use super::{error, VpnChallengeView};
use crate::{docker::DockerSession, model::Result};
use std::{
    process::Stdio,
    sync::{Arc, Mutex},
    time::Duration,
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt, BufReader},
    process::{Child, ChildStdout},
    sync::{oneshot, watch},
    task::JoinHandle,
};

pub(super) const PORT: u16 = 11940;
pub(super) const PASSWORD_PATH: &str = "/run/secrets/vpn_management_password";
pub(super) const MANAGEMENT_LABEL: &str = "io.service-manager.vpn.management";
pub(super) const MANAGEMENT_VERSION: &str = "crv1-v1";
struct Pending {
    id: String,
    sender: oneshot::Sender<String>,
}
#[derive(Default)]
struct State {
    pending: Option<Pending>,
    failure: Option<String>,
}
pub(super) struct Bridge {
    pub gateway_id: String,
    pub session: DockerSession,
    state: Arc<Mutex<State>>,
    cancel: watch::Sender<bool>,
    task: Mutex<Option<JoinHandle<()>>>,
}
pub(super) struct Secrets {
    pub management: String,
    pub username: String,
    pub password: String,
    pub key_passphrase: Option<String>,
}
impl Bridge {
    pub fn spawn(session: DockerSession, gateway_id: String, secrets: Secrets) -> Arc<Self> {
        let state = Arc::new(Mutex::new(State::default()));
        let (cancel, mut receiver) = watch::channel(false);
        let bridge = Arc::new(Self {
            gateway_id: gateway_id.clone(),
            session: session.clone(),
            state: state.clone(),
            cancel,
            task: Mutex::new(None),
        });
        let task = tokio::spawn(async move {
            let outcome = run(session, gateway_id, secrets, state.clone(), &mut receiver).await;
            let mut state = state.lock().unwrap_or_else(|e| e.into_inner());
            state.pending = None;
            if let Err(failure) = outcome {
                if !*receiver.borrow() {
                    state.failure = Some(failure.message);
                }
            }
        });
        *bridge.task.lock().unwrap_or_else(|e| e.into_inner()) = Some(task);
        bridge
    }
    pub fn challenge(&self) -> Option<VpnChallengeView> {
        self.state
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .pending
            .as_ref()
            .map(|pending| VpnChallengeView {
                id: pending.id.clone(),
                prompt: "Enter your VPN MFA/PIN code.".into(),
            })
    }
    pub fn failure(&self) -> Option<String> {
        self.state
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .failure
            .clone()
    }
    pub fn respond(&self, id: &str, response: String) -> Result<()> {
        if *self.cancel.borrow() {
            return Err(error(
                "This VPN challenge was cancelled. Reconnect the profile.",
            ));
        }
        respond(&self.state, id, response)
    }
    pub fn cancel(&self) {
        let _ = self.cancel.send(true);
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        state.pending = None;
        if state.failure.is_none() {
            state.failure = Some("VPN authentication was cancelled. Reconnect the profile.".into());
        }
    }
    pub async fn stop(&self) {
        self.cancel();
        let task = self.task.lock().unwrap_or_else(|e| e.into_inner()).take();
        if let Some(mut task) = task {
            if tokio::time::timeout(Duration::from_secs(3), &mut task)
                .await
                .is_err()
            {
                task.abort();
                let _ = task.await;
            }
        }
        self.state.lock().unwrap_or_else(|e| e.into_inner()).pending = None;
    }
}
impl Drop for Bridge {
    fn drop(&mut self) {
        let _ = self.cancel.send(true);
    }
}

fn respond(state: &Mutex<State>, id: &str, response: String) -> Result<()> {
    if response.is_empty() || response.len() > 4096 || response.chars().any(char::is_control) {
        return Err(error(
            "Enter an MFA/PIN response without control characters, up to 4096 bytes.",
        ));
    }
    let mut state = state.lock().unwrap_or_else(|e| e.into_inner());
    if !state
        .pending
        .as_ref()
        .is_some_and(|pending| pending.id == id)
    {
        return Err(error(
            "This VPN challenge expired or belongs to another profile. Refresh the profile.",
        ));
    }
    state
        .pending
        .take()
        .unwrap()
        .sender
        .send(response)
        .map_err(|_| error("This VPN challenge expired. Reconnect the profile."))
}
async fn line(reader: &mut BufReader<ChildStdout>) -> Result<Option<String>> {
    let mut bytes = Vec::new();
    loop {
        match reader.read_u8().await {
            Ok(b'\n') => {
                if bytes.last() == Some(&b'\r') {
                    bytes.pop();
                }
                return String::from_utf8(bytes)
                    .map(Some)
                    .map_err(|_| error("The VPN authentication channel returned invalid data."));
            }
            Ok(byte) => {
                if bytes.len() >= 16_384 {
                    return Err(error(
                        "The VPN authentication message exceeded its safe limit.",
                    ));
                }
                bytes.push(byte);
            }
            Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => return Ok(None),
            Err(_) => return Err(error("The VPN authentication channel closed unexpectedly.")),
        }
    }
}
async fn close(child: &mut Child, writer: &mut tokio::process::ChildStdin) {
    // Close the authenticated management connection before reaping its Docker observer.
    let _ = writer.write_all(b"quit\n").await;
    let _ = writer.shutdown().await;
    if tokio::time::timeout(Duration::from_secs(1), child.wait())
        .await
        .is_err()
    {
        let _ = child.kill().await;
        let _ = child.wait().await;
    }
}
async fn run(
    session: DockerSession,
    gateway: String,
    secrets: Secrets,
    state: Arc<Mutex<State>>,
    cancel: &mut watch::Receiver<bool>,
) -> Result<()> {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(20);
    let (mut child, mut writer, mut reader) = loop {
        if *cancel.borrow() {
            return Ok(());
        }
        session.verify().await?;
        let mut command = session.command();
        command
            .args(["exec", "-i", &gateway, "nc", "127.0.0.1", &PORT.to_string()])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .kill_on_drop(true);
        let mut child = command
            .spawn()
            .map_err(|_| error("Could not open the private VPN authentication channel."))?;
        let mut writer = child
            .stdin
            .take()
            .ok_or_else(|| error("The VPN authentication channel is unavailable."))?;
        let mut reader = BufReader::new(
            child
                .stdout
                .take()
                .ok_or_else(|| error("The VPN authentication channel is unavailable."))?,
        );
        let sent = writer
            .write_all(format!("{}\n", secrets.management).as_bytes())
            .await
            .is_ok();
        let handshake = async {
            for _ in 0..8 {
                let Some(message) = line(&mut reader).await? else {
                    return Ok::<bool, crate::model::AppError>(false);
                };
                if message.contains("SUCCESS: password is correct") {
                    return Ok(true);
                }
                if message.contains("ERROR:") {
                    return Ok(false);
                }
            }
            Ok(false)
        };
        let first = tokio::select! { _=cancel.changed()=>{close(&mut child,&mut writer).await;return Ok(())}, result=tokio::time::timeout(Duration::from_secs(2),handshake)=>result };
        if sent && matches!(first, Ok(Ok(true))) {
            break (child, writer, reader);
        }
        close(&mut child, &mut writer).await;
        if tokio::time::Instant::now() >= deadline {
            return Err(error("The private VPN authentication channel could not be established. Reconnect the profile."));
        }
        tokio::select! {_=cancel.changed()=>return Ok(()),_=tokio::time::sleep(Duration::from_millis(250))=>{}}
    };
    let conversation = async {
        writer
            .write_all(b"state on\nhold off\nhold release\n")
            .await
            .map_err(|_| error("The VPN authentication channel closed."))?;
        let mut initial_sent = false;
        let mut dynamic: Option<DynamicChallenge> = None;
        let mut challenge_seen = false;
        loop {
            let message = line(&mut reader).await?.ok_or_else(|| {
                error("The VPN authentication channel closed. Reconnect the profile.")
            })?;
            if connected(&message) {
                initial_sent = false;
                challenge_seen = false;
                dynamic = None;
                continue;
            }
            if message.starts_with(">PASSWORD:Verification Failed:") {
                if challenge_seen {
                    return Err(error(
                        "The VPN MFA/PIN response was rejected. Reconnect to try again.",
                    ));
                }
                dynamic = Some(parse_dynamic(&message)?);
                challenge_seen = true;
            } else if message.starts_with(">PASSWORD:Need 'Auth' username/password") {
                if message.contains(" SC:") {
                    return Err(error(
                        "This VPN uses an unsupported static authentication challenge.",
                    ));
                }
                let (username, password) = if let Some(dynamic) = dynamic.take() {
                    let (sender, receiver) = oneshot::channel();
                    let id = uuid::Uuid::new_v4().to_string();
                    state.lock().unwrap_or_else(|e| e.into_inner()).pending =
                        Some(Pending { id, sender });
                    let response = tokio::time::timeout(Duration::from_secs(300), receiver)
                        .await
                        .map_err(|_| {
                            error("The VPN MFA/PIN challenge expired. Reconnect to try again.")
                        })?
                        .map_err(|_| error("The VPN MFA/PIN challenge was cancelled."))?;
                    (
                        dynamic.username,
                        format!("CRV1::{}::{response}", dynamic.state_id),
                    )
                } else if !initial_sent {
                    initial_sent = true;
                    (secrets.username.clone(), secrets.password.clone())
                } else {
                    return Err(error(
                        "VPN authentication was rejected. Reconnect to try again.",
                    ));
                };
                writer
                    .write_all(credentials(&username, &password)?.as_bytes())
                    .await
                    .map_err(|_| {
                        error("The VPN authentication response could not be delivered.")
                    })?;
            } else if message.starts_with(">PASSWORD:Need 'Private Key' password") {
                let value = secrets
                    .key_passphrase
                    .as_ref()
                    .ok_or_else(|| error("This VPN private key requires a saved passphrase."))?;
                writer
                    .write_all(format!("password \"Private Key\" {}\n", quote(value)?).as_bytes())
                    .await
                    .map_err(|_| error("The VPN private-key response could not be delivered."))?;
            } else if message.starts_with("ERROR:") {
                return Err(error(
                    "The private VPN authentication command was rejected.",
                ));
            }
        }
    };
    let outcome = tokio::select! {_=cancel.changed()=>Ok(()),result=conversation=>result};
    close(&mut child, &mut writer).await;
    outcome
}
fn connected(message: &str) -> bool {
    let Some(state) = message.strip_prefix(">STATE:") else {
        return false;
    };
    let mut fields = state.split(',');
    fields
        .next()
        .is_some_and(|timestamp| timestamp.parse::<u64>().is_ok())
        && fields.next() == Some("CONNECTED")
        && fields.next() == Some("SUCCESS")
}
struct DynamicChallenge {
    state_id: String,
    username: String,
}
fn parse_dynamic(message: &str) -> Result<DynamicChallenge> {
    let invalid = || {
        error("VPN authentication failed or returned an unsupported challenge. Reconnect and check your credentials.")
    };
    let (_, challenge) = message.split_once("['CRV1:").ok_or_else(invalid)?;
    let challenge = challenge.strip_suffix("']").ok_or_else(invalid)?;
    let mut parts = challenge.splitn(4, ':');
    let flags = parts.next().ok_or_else(invalid)?;
    let state_id = parts.next().ok_or_else(invalid)?;
    let user = parts.next().ok_or_else(invalid)?;
    let _prompt = parts.next().ok_or_else(invalid)?;
    if !flags.split(',').any(|flag| flag == "R")
        || flags.split(',').any(|flag| !matches!(flag, "R" | "E"))
        || state_id.is_empty()
        || state_id.len() > 1024
        || state_id.chars().any(char::is_control)
    {
        return Err(invalid());
    }
    let username = decode_username(user)?;
    quote(&username)?;
    Ok(DynamicChallenge {
        state_id: state_id.into(),
        username,
    })
}
fn decode_username(encoded: &str) -> Result<String> {
    let invalid = || error("The VPN challenge username was invalid.");
    if encoded.is_empty() || encoded.len() > 4096 || encoded.len() % 4 != 0 {
        return Err(invalid());
    }
    let mut output = Vec::new();
    let mut ended = false;
    for chunk in encoded.as_bytes().chunks_exact(4) {
        if ended {
            return Err(invalid());
        }
        let mut value = 0u32;
        let mut padding = 0;
        for &byte in chunk {
            let digit = match byte {
                b'A'..=b'Z' => byte - b'A',
                b'a'..=b'z' => byte - b'a' + 26,
                b'0'..=b'9' => byte - b'0' + 52,
                b'+' => 62,
                b'/' => 63,
                b'=' => {
                    padding += 1;
                    0
                }
                _ => return Err(invalid()),
            };
            if padding > 0 && byte != b'=' {
                return Err(invalid());
            }
            value = (value << 6) | u32::from(digit);
        }
        if padding > 2
            || (padding == 1 && value & 0xff != 0)
            || (padding == 2 && value & 0xffff != 0)
        {
            return Err(invalid());
        }
        output.push((value >> 16) as u8);
        if padding < 2 {
            output.push((value >> 8) as u8);
        }
        if padding == 0 {
            output.push(value as u8);
        }
        ended = padding > 0;
    }
    let username = String::from_utf8(output).map_err(|_| invalid())?;
    if username.is_empty() {
        return Err(invalid());
    }
    Ok(username)
}
fn quote(value: &str) -> Result<String> {
    if value.len() > 8192 || value.chars().any(char::is_control) {
        return Err(error("The VPN authentication value is invalid."));
    }
    Ok(format!(
        "\"{}\"",
        value.replace('\\', "\\\\").replace('"', "\\\"")
    ))
}
fn credentials(username: &str, password: &str) -> Result<String> {
    Ok(format!(
        "username \"Auth\" {}\npassword \"Auth\" {}\n",
        quote(username)?,
        quote(password)?
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn crv1_uses_server_state_and_decoded_username_without_exposing_prompt() {
        let parsed=parse_dynamic(">PASSWORD:Verification Failed: 'Auth' ['CRV1:R,E:opaque-state:dXNlcg==:untrusted:prompt']").unwrap();
        assert_eq!(parsed.username, "user");
        assert_eq!(parsed.state_id, "opaque-state");
        assert_eq!(
            credentials(
                &parsed.username,
                &format!("CRV1::{}::123456", parsed.state_id)
            )
            .unwrap(),
            "username \"Auth\" \"user\"\npassword \"Auth\" \"CRV1::opaque-state::123456\"\n"
        );
    }
    #[test]
    fn unsupported_auth_and_command_injection_are_rejected() {
        for message in [
            ">PASSWORD:Verification Failed: 'Auth'",
            ">PASSWORD:Verification Failed: ['SCRV1:x']",
            ">PASSWORD:Verification Failed: ['CRV1:R:state:!!!=:prompt']",
        ] {
            assert!(parse_dynamic(message).is_err());
        }
        assert!(credentials("user", "pin\nquit").is_err());
        assert_eq!(quote("a\\\"b").unwrap(), "\"a\\\\\\\"b\"");
    }
    #[test]
    fn only_successful_management_state_starts_a_fresh_auth_cycle() {
        assert!(connected(
            ">STATE:1700000000,CONNECTED,SUCCESS,10.0.0.2,192.0.2.1,1194"
        ));
        for message in [
            ">STATE:1700000000,AUTH,,,,",
            ">STATE:1700000000,RECONNECTING,auth-failure",
            ">PASSWORD:Verification Failed: ['CRV1:R:state:dXNlcg==:CONNECTED,SUCCESS']",
            ">STATE:invalid,CONNECTED,SUCCESS",
        ] {
            assert!(!connected(message));
        }
    }
    #[test]
    fn base64_usernames_validate_padding_and_utf8() {
        assert_eq!(decode_username("dXNlcg==").unwrap(), "user");
        assert_eq!(decode_username("Y3Ix").unwrap(), "cr1");
        for value in ["====", "dX=l", "dXNlcg=Z", "/w==", "YQ=Z"] {
            assert!(decode_username(value).is_err());
        }
    }
    #[tokio::test]
    async fn challenge_ids_are_single_use_and_profile_scoped() {
        let state = Arc::new(Mutex::new(State::default()));
        let (sender, receiver) = oneshot::channel();
        state.lock().unwrap().pending = Some(Pending {
            id: "owned-challenge".into(),
            sender,
        });
        assert!(respond(&state, "other-profile-challenge", "123456".into()).is_err());
        respond(&state, "owned-challenge", "123456".into()).unwrap();
        assert_eq!(receiver.await.unwrap(), "123456");
        assert!(respond(&state, "owned-challenge", "123456".into()).is_err());
    }
}
