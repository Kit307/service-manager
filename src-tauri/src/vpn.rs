use crate::{
    docker::{capture, DockerSession},
    model::{AppError, Result},
    project::{secure_write, ServiceRef},
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    collections::{HashMap, HashSet},
    fs,
    io::Read,
    os::unix::fs::{OpenOptionsExt, PermissionsExt},
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};
use tokio::sync::Mutex as AsyncMutex;

const IMAGE: &str = "qmcgaw/gluetun:v3.41.0";
const OWNER: &str = "io.service-manager.vpn.owner";
const PROFILE: &str = "io.service-manager.vpn.profile";
const FINGERPRINT: &str = "io.service-manager.vpn.fingerprint";
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct VpnPortMapping {
    pub host: u16,
    pub container: u16,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct VpnImport {
    pub name: String,
    pub config_path: String,
    #[serde(default)]
    pub username: String,
    #[serde(default)]
    pub password: String,
    #[serde(default)]
    pub key_passphrase: String,
    #[serde(default)]
    pub published_ports: Vec<VpnPortMapping>,
    #[serde(default)]
    pub health_target: String,
    #[serde(default)]
    pub dns_address: String,
    #[serde(default)]
    pub health_icmp_target: String,
}
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VpnProfileView {
    pub id: String,
    pub name: String,
    pub published_ports: Vec<VpnPortMapping>,
    pub health_target: String,
    #[serde(default)]
    pub dns_address: String,
    #[serde(default)]
    pub health_icmp_target: String,
    pub state: String,
    pub error: Option<String>,
    pub users: Vec<ServiceRef>,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Profile {
    id: String,
    name: String,
    published_ports: Vec<VpnPortMapping>,
    health_target: String,
    #[serde(default)]
    dns_address: String,
    #[serde(default)]
    health_icmp_target: String,
    fingerprint: String,
    #[serde(default = "default_true")]
    requires_credentials: bool,
    #[serde(default)]
    has_key_passphrase: bool,
}
fn default_true() -> bool {
    true
}
#[derive(Default)]
struct ConfigAuth {
    requires_credentials: bool,
    encrypted_key: bool,
}
struct Inner {
    dir: PathBuf,
    owner: String,
    profiles: AsyncMutex<Vec<Profile>>,
    operations: AsyncMutex<()>,
    leases: Mutex<HashMap<String, HashMap<String, ServiceRef>>>,
}
#[derive(Clone)]
pub struct VpnManager {
    inner: Arc<Inner>,
}
pub struct VpnLease {
    pub(crate) session: DockerSession,
    pub gateway_id: String,
    pub published_ports: Vec<VpnPortMapping>,
    pub dns_address: String,
    inner: Arc<Inner>,
    profile: String,
    token: String,
    fingerprint: String,
}
fn error(message: &str) -> AppError {
    AppError::new("VPN_ERROR", message)
}
fn private_dir(path: &Path) -> Result<()> {
    if fs::symlink_metadata(path).is_ok_and(|m| m.file_type().is_symlink()) {
        return Err(error("VPN storage cannot be a symbolic link."));
    }
    fs::create_dir_all(path)?;
    fs::set_permissions(path, fs::Permissions::from_mode(0o700))?;
    Ok(())
}
fn read_private(path: &Path) -> Result<Vec<u8>> {
    let meta = fs::symlink_metadata(path)?;
    if !meta.is_file() || meta.file_type().is_symlink() || meta.len() > 1_048_576 {
        return Err(error(
            "VPN storage is invalid; existing files were preserved.",
        ));
    }
    fs::set_permissions(path, fs::Permissions::from_mode(0o600))?;
    Ok(fs::read(path)?)
}
fn validate_ports(ports: &[VpnPortMapping]) -> Result<()> {
    let mut hosts = HashSet::new();
    let mut containers = HashSet::new();
    if ports.len() > 32
        || ports.iter().any(|p| {
            p.host == 0
                || p.container == 0
                || matches!(p.container, 53 | 8000 | 9999)
                || !hosts.insert(p.host)
                || !containers.insert(p.container)
        })
    {
        return Err(error("Use up to 32 distinct nonzero host and container ports; container ports 53, 8000 and 9999 are reserved for VPN DNS, control and health."));
    }
    Ok(())
}
fn ipv4_target(input: &str, dns: bool) -> Result<String> {
    let input = input.trim();
    if input.is_empty() {
        return Ok(if dns { String::new() } else { "0.0.0.0".into() });
    }
    let ip = input
        .parse::<std::net::Ipv4Addr>()
        .map_err(|_| error("VPN DNS and ICMP targets must be IPv4 addresses."))?;
    if ip.is_loopback() || ip.is_multicast() || ip.is_broadcast() || (dns && ip.is_unspecified()) {
        return Err(error("Use a non-loopback unicast IPv4 DNS or ICMP target. Only ICMP accepts 0.0.0.0 for the VPN server."));
    }
    Ok(ip.to_string())
}
fn health_target(input: &str) -> Result<String> {
    let input = if input.trim().is_empty() {
        "cloudflare.com:443"
    } else {
        input.trim()
    };
    let url = reqwest::Url::parse(&format!("https://{input}"))
        .map_err(|_| error("Use a health target in hostname:port form."))?;
    let port = input
        .rsplit_once(':')
        .and_then(|(_, p)| p.parse::<u16>().ok());
    let host = url.host_str().unwrap_or("");
    if port.is_none_or(|p| p == 0)
        || !url.username().is_empty()
        || url.password().is_some()
        || url.path() != "/"
        || url.query().is_some()
        || url.fragment().is_some()
        || input.contains('/')
        || input.contains('@')
        || host.is_empty()
        || host == "localhost"
        || host.ends_with(".localhost")
        || host
            .trim_matches(['[', ']'])
            .parse::<std::net::IpAddr>()
            .is_ok_and(|a| a.is_loopback() || a.is_unspecified())
        || host == "[::1]"
        || host == "[::]"
        || host.bytes().any(|b| b.is_ascii_whitespace())
    {
        return Err(error(
            "Use a non-loopback health target in hostname:port form.",
        ));
    }
    Ok(input.to_owned())
}
fn remote_host(line: &str) -> Result<Option<(usize, usize, &str)>> {
    let content = line.split(['#', ';']).next().unwrap_or("");
    let mut words = content.split_whitespace();
    let Some(option) = words.next() else {
        return Ok(None);
    };
    if option.trim_start_matches("--") != "remote" {
        return Ok(None);
    }
    let invalid = || {
        error("Use at most 32 OpenVPN remote directives with an IPv4 address or hostname, optional valid port and IPv4 TCP/UDP protocol.")
    };
    let host = words.next().ok_or_else(invalid)?;
    let args: Vec<_> = words.collect();
    if args.len() > 2
        || args
            .first()
            .is_some_and(|p| p.parse::<u16>().map_or(true, |p| p == 0))
        || args.get(1).is_some_and(|p| {
            !["udp", "udp4", "tcp", "tcp4", "tcp-client", "tcp4-client"].contains(p)
        })
    {
        return Err(invalid());
    }
    match host.parse::<std::net::IpAddr>() {
        Ok(std::net::IpAddr::V4(_)) => {}
        Ok(std::net::IpAddr::V6(_)) => {
            return Err(error(
                "IPv6-only OpenVPN remotes are unsupported by this gateway.",
            ))
        }
        Err(_) => {
            let dns = host.strip_suffix('.').unwrap_or(host);
            if dns.is_empty()
                || host.len() > 253
                || dns.split('.').any(|label| {
                    label.is_empty()
                        || label.len() > 63
                        || label.starts_with('-')
                        || label.ends_with('-')
                        || !label
                            .bytes()
                            .all(|b| b.is_ascii_alphanumeric() || b == b'-')
                })
            {
                return Err(invalid());
            }
        }
    }
    let option_start = line.len() - line.trim_start().len();
    let tail = &line[option_start + option.len()..];
    let start = option_start + option.len() + tail.len() - tail.trim_start().len();
    Ok(Some((start, start + host.len(), host)))
}
async fn runtime_config(original: &[u8], askpass: bool) -> Result<Vec<u8>> {
    validate_config(original)?;
    let text = std::str::from_utf8(original)
        .map_err(|_| error("The private OpenVPN configuration is invalid."))?;
    let mut result = String::with_capacity(text.len());
    let mut block: Option<String> = None;
    for line in text.split_inclusive('\n') {
        let trimmed = line.trim();
        if let Some(tag) = block.as_ref() {
            if trimmed == format!("</{tag}>") {
                block = None;
            }
            result.push_str(line);
            continue;
        }
        if trimmed.starts_with('<') {
            block = Some(trimmed.trim_start_matches('<').trim_end_matches('>').into());
            result.push_str(line);
            continue;
        }
        if let Some((start, end, host)) = remote_host(line)? {
            if host.parse::<std::net::Ipv4Addr>().is_ok() {
                result.push_str(line);
                continue;
            }
            let addresses = tokio::time::timeout(
                std::time::Duration::from_secs(5),
                tokio::net::lookup_host((host, 0)),
            )
            .await
            .map_err(|_| error("VPN server name resolution timed out before connecting."))?
            .map_err(|_| error("The VPN server name could not be resolved before connecting."))?;
            let address = addresses
                .filter_map(|address| match address.ip() {
                    std::net::IpAddr::V4(ip) => Some(ip),
                    _ => None,
                })
                .next()
                .ok_or_else(|| error("The VPN server has no supported IPv4 address."))?;
            result.push_str(&line[..start]);
            result.push_str(&address.to_string());
            result.push_str(&line[end..]);
        } else {
            result.push_str(line);
        }
    }
    if askpass {
        if !result.ends_with('\n') {
            result.push('\n');
        }
        result.push_str("askpass /etc/openvpn/askpass\n");
    }
    Ok(result.into_bytes())
}
fn validate_config(bytes: &[u8]) -> Result<ConfigAuth> {
    let text = std::str::from_utf8(bytes)
        .map_err(|_| error("Import a UTF-8 inline-certificate OpenVPN configuration."))?;
    if bytes.len() > 1_048_576 || text.contains('\0') {
        return Err(error(
            "Configurations over 1 MiB and OTP authentication are unsupported.",
        ));
    }
    let mut block: Option<&str> = None;
    let mut remotes = 0;
    let mut auth = ConfigAuth::default();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') || line.starts_with(';') {
            continue;
        }
        if let Some(tag) = block {
            if tag == "key"
                && (line.contains("ENCRYPTED PRIVATE KEY")
                    || line.contains("Proc-Type: 4,ENCRYPTED"))
            {
                auth.encrypted_key = true;
            }
            if line == format!("</{tag}>") {
                block = None;
            } else if line.contains(['<', '>']) {
                return Err(error("Malformed inline certificate or key block."));
            }
            continue;
        }
        if line.starts_with('<') {
            let tag = line
                .strip_prefix('<')
                .and_then(|s| s.strip_suffix('>'))
                .unwrap_or("");
            if !["ca", "cert", "key", "tls-auth", "tls-crypt", "tls-crypt-v2"].contains(&tag) {
                return Err(error(
                    "Only inline certificate and key blocks are supported.",
                ));
            }
            block = Some(tag);
            continue;
        }
        let mut words = line.split_whitespace();
        let option = words.next().unwrap_or("").trim_start_matches("--");
        let args: Vec<_> = words.collect();
        if option == "remote" {
            remotes += 1;
            if remotes > 32 {
                return Err(error("Use at most 32 OpenVPN remote directives."));
            }
            remote_host(line)?;
        }
        if option == "auth-user-pass" {
            auth.requires_credentials = true;
            if !args.is_empty() {
                return Err(error(
                    "Use parameterless auth-user-pass; credentials are stored separately.",
                ));
            }
            continue;
        }
        if matches!(option, "dev" | "dev-type") && args.as_slice() != ["tun"] {
            return Err(error("Only OpenVPN tun devices are supported."));
        }
        if option == "ignore-unknown-option"
            && (args.is_empty()
                || args
                    .iter()
                    .any(|value| !["data-ciphers", "block-outside-dns"].contains(value)))
        {
            return Err(error(
                "Only known safe compatibility options may be ignored.",
            ));
        }
        if matches!(option, "push-peer-info" | "block-outside-dns") && !args.is_empty() {
            return Err(error(
                "This OpenVPN compatibility option does not accept arguments.",
            ));
        }
        if matches!(option, "hand-window" | "server-poll-timeout")
            && (args.len() != 1 || args[0].parse::<u32>().map_or(true, |value| value == 0))
        {
            return Err(error(
                "OpenVPN compatibility timeouts require one positive integer.",
            ));
        }
        if option == "setenv" {
            let key = args.first().copied().unwrap_or("");
            let metadata = key.strip_prefix("UV_").is_some_and(|suffix| {
                !suffix.is_empty()
                    && suffix
                        .bytes()
                        .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == b'_')
            });
            if args.len() < 2 || !(key == "CLIENT_CERT" || metadata) {
                return Err(error(
                    "Only CLIENT_CERT and UV_ metadata environment entries are supported.",
                ));
            }
        }
        if ![
            "client",
            "dev-type",
            "ignore-unknown-option",
            "block-outside-dns",
            "push-peer-info",
            "hand-window",
            "server-poll-timeout",
            "dev",
            "proto",
            "remote",
            "resolv-retry",
            "nobind",
            "persist-key",
            "persist-tun",
            "remote-cert-tls",
            "verify-x509-name",
            "cipher",
            "auth",
            "auth-nocache",
            "verb",
            "mute",
            "explicit-exit-notify",
            "tun-mtu",
            "mssfix",
            "key-direction",
            "tls-client",
            "tls-version-min",
            "tls-cipher",
            "tls-ciphersuites",
            "data-ciphers",
            "data-ciphers-fallback",
            "connect-retry",
            "connect-retry-max",
            "connect-timeout",
            "remote-random",
            "reneg-sec",
            "sndbuf",
            "rcvbuf",
            "float",
            "comp-lzo",
            "compress",
            "topology",
            "route",
            "redirect-gateway",
            "route-delay",
            "ping",
            "ping-restart",
            "keepalive",
            "pull",
            "pull-filter",
            "setenv",
        ]
        .contains(&option)
        {
            return Err(error("Unsupported OpenVPN configuration: use inline certificates and keys, with no external files, scripts, plugins, management, logs or output overrides. OTP is unsupported."));
        }
    }
    if block.is_some() {
        return Err(error("An inline certificate or key block is incomplete."));
    }
    Ok(auth)
}
impl VpnManager {
    pub fn new(state_dir: PathBuf) -> Result<Self> {
        let dir = state_dir.join("vpn");
        private_dir(&dir)?;
        let owner_file = dir.join("owner");
        let owner = if owner_file.exists() {
            String::from_utf8(read_private(&owner_file)?)
                .map_err(|_| error("VPN ownership state is invalid."))?
        } else {
            let id = uuid::Uuid::new_v4().to_string();
            secure_write(&owner_file, id.as_bytes())?;
            id
        };
        if uuid::Uuid::parse_str(&owner).is_err() {
            return Err(error("VPN ownership state is invalid."));
        }
        let file = dir.join("profiles.json");
        let profiles: Vec<Profile> = if file.exists() {
            serde_json::from_slice(&read_private(&file)?)
                .map_err(|_| error("VPN profiles are invalid; existing files were preserved."))?
        } else {
            vec![]
        };
        for p in &profiles {
            if uuid::Uuid::parse_str(&p.id).is_err() {
                return Err(error("VPN profile identity is invalid."));
            }
            validate_ports(&p.published_ports)?;
            health_target(&p.health_target)?;
            ipv4_target(&p.dns_address, true)?;
            ipv4_target(&p.health_icmp_target, false)?;
        }
        Ok(Self {
            inner: Arc::new(Inner {
                dir,
                owner,
                profiles: AsyncMutex::new(profiles),
                operations: AsyncMutex::new(()),
                leases: Mutex::new(HashMap::new()),
            }),
        })
    }
    fn persist(&self, profiles: &[Profile]) -> Result<()> {
        secure_write(
            &self.inner.dir.join("profiles.json"),
            &serde_json::to_vec(profiles).map_err(|_| error("Could not save VPN profiles."))?,
        )
    }
    fn users(&self, id: &str) -> Vec<ServiceRef> {
        let leases = self.inner.leases.lock().unwrap_or_else(|e| e.into_inner());
        let mut users = Vec::new();
        if let Some(runs) = leases.get(id) {
            for user in runs.values() {
                if !users.contains(user) {
                    users.push(user.clone());
                }
            }
        }
        users
    }
    fn view(&self, p: &Profile, state: &str, message: Option<String>) -> VpnProfileView {
        VpnProfileView {
            id: p.id.clone(),
            name: p.name.clone(),
            published_ports: p.published_ports.clone(),
            health_target: p.health_target.clone(),
            dns_address: p.dns_address.clone(),
            health_icmp_target: ipv4_target(&p.health_icmp_target, false)
                .unwrap_or_else(|_| "0.0.0.0".into()),
            state: state.into(),
            error: message,
            users: self.users(&p.id),
        }
    }
    pub(crate) async fn ensure_service_ports(&self, id: &str, ports: &[u16]) -> Result<()> {
        let profiles = self.inner.profiles.lock().await;
        let profile = find(&profiles, id)?;
        if ports.iter().any(|port| {
            !profile
                .published_ports
                .iter()
                .any(|mapping| mapping.host == *port)
        }) {
            return Err(error("Every declared service, preview and health-check host port must be published by its VPN profile."));
        }
        Ok(())
    }
    pub async fn exists(&self, id: &str) -> bool {
        self.inner.profiles.lock().await.iter().any(|p| p.id == id)
    }
    pub async fn import(&self, input: VpnImport) -> Result<VpnProfileView> {
        let name = input.name.trim();
        if name.is_empty() || name.chars().count() > 80 || name.chars().any(char::is_control) {
            return Err(error("Choose a VPN name between 1 and 80 characters."));
        }
        validate_ports(&input.published_ports)?;
        let target = health_target(&input.health_target)?;
        let dns_address = ipv4_target(&input.dns_address, true)?;
        let health_icmp_target = ipv4_target(&input.health_icmp_target, false)?;
        let invalid_source = || {
            error("Choose a regular local OpenVPN file of at most 1 MiB, without symbolic links.")
        };
        // Nonblocking open rejects FIFOs/devices before a read can wait for external input.
        let file = fs::OpenOptions::new()
            .read(true)
            .custom_flags(nix::libc::O_NONBLOCK | nix::libc::O_NOFOLLOW)
            .open(&input.config_path)
            .map_err(|_| invalid_source())?;
        let metadata = file.metadata().map_err(|_| invalid_source())?;
        if !metadata.is_file() || metadata.len() > 1_048_576 {
            return Err(invalid_source());
        }
        let mut config = Vec::new();
        file.take(1_048_577)
            .read_to_end(&mut config)
            .map_err(|_| invalid_source())?;
        let auth = validate_config(&config)?;
        if [&input.username, &input.password, &input.key_passphrase]
            .iter()
            .any(|v| v.len() > 4096 || v.chars().any(char::is_control))
        {
            return Err(error("VPN credentials and key passphrases must be at most 4096 bytes without control characters."));
        }
        if auth.requires_credentials && (input.username.is_empty() || input.password.is_empty()) {
            return Err(error(
                "This OpenVPN configuration requires a username and password. OTP is unsupported.",
            ));
        }
        if auth.encrypted_key && input.key_passphrase.is_empty() {
            return Err(error(
                "This encrypted private key requires its key passphrase.",
            ));
        }
        let mut profiles = self.inner.profiles.lock().await;
        if profiles
            .iter()
            .any(|p| p.name.to_lowercase() == name.to_lowercase())
        {
            return Err(error("Choose a unique VPN profile name."));
        }
        let id = uuid::Uuid::new_v4().to_string();
        let dir = self.inner.dir.join(&id);
        private_dir(&dir)?;
        secure_write(&dir.join("client.ovpn"), &config)?;
        if auth.requires_credentials {
            secure_write(&dir.join("username"), input.username.as_bytes())?;
            secure_write(&dir.join("password"), input.password.as_bytes())?;
        }
        if auth.encrypted_key {
            secure_write(&dir.join("key-passphrase"), input.key_passphrase.as_bytes())?;
        }
        let fingerprint = uuid::Uuid::new_v4().to_string();
        let p = Profile {
            id,
            name: name.into(),
            published_ports: input.published_ports,
            health_target: target,
            dns_address,
            health_icmp_target,
            fingerprint,
            requires_credentials: auth.requires_credentials,
            has_key_passphrase: auth.encrypted_key,
        };
        let mut next = profiles.clone();
        next.push(p.clone());
        self.persist(&next)?;
        *profiles = next;
        Ok(self.view(&p, "disconnected", None))
    }
    pub async fn list(&self) -> Result<Vec<VpnProfileView>> {
        let profiles = self.inner.profiles.lock().await.clone();
        if profiles.is_empty() {
            return Ok(vec![]);
        }
        let session = DockerSession::pinned(&self.inner.dir).await;
        let mut views = Vec::new();
        for p in profiles.iter() {
            let status = match &session {
                Ok(s) => self.gateway(s, p).await,
                Err(e) => Err(e.clone()),
            };
            views.push(match status {
                Ok(Some(v)) => self.view(p, container_state(&v), None),
                Ok(None) => self.view(p, "disconnected", None),
                Err(_) => self.view(
                    p,
                    "unavailable",
                    Some(
                        "The local Docker gateway is unavailable or its ownership changed.".into(),
                    ),
                ),
            });
        }
        Ok(views)
    }
    pub async fn connect(&self, id: &str) -> Result<VpnProfileView> {
        let _operation = self.inner.operations.lock().await;
        let profiles = self.inner.profiles.lock().await.clone();
        let p = find(&profiles, id)?;
        let session = DockerSession::pinned(&self.inner.dir).await?;
        self.ensure(&session, p).await?;
        Ok(self.view(p, "ready", None))
    }
    pub async fn acquire(&self, id: &str, user: &ServiceRef, run_id: &str) -> Result<VpnLease> {
        let _operation = self.inner.operations.lock().await;
        let profiles = self.inner.profiles.lock().await.clone();
        let p = find(&profiles, id)?;
        let session = DockerSession::pinned(&self.inner.dir).await?;
        let gateway_id = self.ensure(&session, p).await?;
        // A unique token prevents an old run's Drop from releasing a newer run's lease.
        let token = format!("{run_id}:{}", uuid::Uuid::new_v4());
        self.inner
            .leases
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .entry(id.into())
            .or_default()
            .insert(token.clone(), user.clone());
        Ok(VpnLease {
            session,
            gateway_id,
            published_ports: p.published_ports.clone(),
            dns_address: if p.dns_address.is_empty() {
                "127.0.0.1".into()
            } else {
                p.dns_address.clone()
            },
            inner: self.inner.clone(),
            profile: id.into(),
            token,
            fingerprint: p.fingerprint.clone(),
        })
    }
    pub async fn disconnect(&self, id: &str) -> Result<()> {
        let _operation = self.inner.operations.lock().await;
        let profiles = self.inner.profiles.lock().await.clone();
        self.stop(find(&profiles, id)?).await
    }
    pub async fn remove(&self, id: &str) -> Result<()> {
        let _operation = self.inner.operations.lock().await;
        let snapshot = self.inner.profiles.lock().await.clone();
        self.stop(find(&snapshot, id)?).await?;
        let mut profiles = self.inner.profiles.lock().await;
        let p = find(&profiles, id)?;
        let session = DockerSession::pinned(&self.inner.dir).await?;
        session.verify().await?;
        if let Some(v) = self.gateway(&session, p).await? {
            let gateway = v["Id"]
                .as_str()
                .ok_or_else(|| error("Gateway identity is missing."))?;
            let mut cmd = session.command();
            cmd.args(["container", "rm", gateway]);
            capture(cmd).await?;
        }
        let network = format!("{}-net", self.name(p));
        let mut cmd = session.command();
        cmd.args([
            "network",
            "ls",
            "--filter",
            &format!("name=^{network}$"),
            "--format",
            "{{.ID}}",
        ]);
        let ids = String::from_utf8(capture(cmd).await?)
            .map_err(|_| error("Docker returned invalid network metadata."))?;
        for network_id in ids.lines() {
            let mut cmd = session.command();
            cmd.args(["network", "inspect", network_id]);
            let v: Value = serde_json::from_slice(&capture(cmd).await?)
                .map_err(|_| error("Docker returned invalid network metadata."))?;
            let labels = &v[0]["Labels"];
            if labels[OWNER].as_str() != Some(&self.inner.owner)
                || labels[PROFILE].as_str() != Some(&p.id)
                || labels[FINGERPRINT].as_str() != Some(&p.fingerprint)
            {
                return Err(error("A foreign or changed VPN network was preserved."));
            }
            let mut cmd = session.command();
            cmd.args(["network", "rm", network_id]);
            capture(cmd).await?;
        }
        let next: Vec<_> = profiles.iter().filter(|p| p.id != id).cloned().collect();
        self.persist(&next)?;
        *profiles = next;
        fs::remove_dir_all(self.inner.dir.join(id))?;
        Ok(())
    }
    pub async fn shutdown(&self) -> Result<()> {
        let _operation = self.inner.operations.lock().await;
        let profiles = self.inner.profiles.lock().await.clone();
        let mut failure = None;
        for p in profiles.iter() {
            if let Err(e) = self.stop(p).await {
                failure = Some(e);
            }
        }
        failure.map_or(Ok(()), Err)
    }
    fn name(&self, p: &Profile) -> String {
        format!("sm-vpn-{}-{}", &self.inner.owner[..8], p.id)
    }
    fn owns(&self, v: &Value, p: &Profile) -> bool {
        let l = &v["Config"]["Labels"];
        l[OWNER].as_str() == Some(&self.inner.owner)
            && l[PROFILE].as_str() == Some(&p.id)
            && l[FINGERPRINT].as_str() == Some(&p.fingerprint)
    }
    async fn gateway(&self, s: &DockerSession, p: &Profile) -> Result<Option<Value>> {
        let name = self.name(p);
        let mut cmd = s.command();
        cmd.args([
            "container",
            "ls",
            "--all",
            "--filter",
            &format!("name=^/{name}$"),
            "--format",
            "{{.ID}}",
        ]);
        let ids = String::from_utf8(capture(cmd).await?)
            .map_err(|_| error("Docker returned invalid gateway metadata."))?;
        let Some(id) = ids.lines().next() else {
            return Ok(None);
        };
        let v = inspect(s, id).await?;
        if !self.owns(&v, p) {
            return Err(error(
                "A gateway name is occupied by a foreign or changed container; it was preserved.",
            ));
        }
        Ok(Some(v))
    }
    async fn ensure(&self, s: &DockerSession, p: &Profile) -> Result<String> {
        s.verify().await?;
        if let Some(v) = self.gateway(s, p).await? {
            let id = v["Id"]
                .as_str()
                .ok_or_else(|| error("Gateway identity is missing."))?
                .to_owned();
            if !v["State"]["Running"].as_bool().unwrap_or(false) {
                let mut cmd = s.command();
                cmd.args(["start", &id]);
                capture(cmd).await?;
            }
            self.wait_healthy(s, &id, p).await?;
            return Ok(id);
        }
        let mut image = s.command();
        image.args(["image", "inspect", IMAGE]);
        capture(image).await.map_err(|_| {
            error("Download the required Gluetun image from VPN settings before connecting.")
        })?;
        let network = format!("{}-net", self.name(p));
        let mut cmd = s.command();
        cmd.args([
            "network",
            "ls",
            "--filter",
            &format!("name=^{network}$"),
            "--format",
            "{{.ID}}",
        ]);
        let existing = String::from_utf8(capture(cmd).await?)
            .map_err(|_| error("Docker returned invalid network metadata."))?;
        if let Some(id) = existing.lines().next() {
            let mut cmd = s.command();
            cmd.args(["network", "inspect", id]);
            let v: Value = serde_json::from_slice(&capture(cmd).await?)
                .map_err(|_| error("Docker returned invalid network metadata."))?;
            let l = &v[0]["Labels"];
            if l[OWNER].as_str() != Some(&self.inner.owner)
                || l[PROFILE].as_str() != Some(&p.id)
                || l[FINGERPRINT].as_str() != Some(&p.fingerprint)
            {
                return Err(error("A foreign or changed VPN network was preserved."));
            }
        } else {
            let mut cmd = s.command();
            cmd.args([
                "network",
                "create",
                "--driver",
                "bridge",
                "--label",
                &format!("{OWNER}={}", self.inner.owner),
                "--label",
                &format!("{PROFILE}={}", p.id),
                "--label",
                &format!("{FINGERPRINT}={}", p.fingerprint),
                &network,
            ]);
            capture(cmd).await?;
        }
        let dir = self.inner.dir.join(&p.id);
        private_dir(&dir)?;
        if dir.to_string_lossy().contains([',', '\n']) {
            return Err(error(
                "VPN storage paths cannot contain commas or line breaks.",
            ));
        }
        read_private(&dir.join("client.ovpn"))?;
        if p.requires_credentials {
            for file in ["username", "password"] {
                read_private(&dir.join(file))?;
            }
        }
        if p.has_key_passphrase {
            read_private(&dir.join("key-passphrase"))?;
        }
        // Gluetun v3.41.0 accepts only numeric remote addresses; retain the imported original.
        let runtime = runtime_config(
            &read_private(&dir.join("client.ovpn"))?,
            p.has_key_passphrase,
        )
        .await?;
        secure_write(&dir.join("gateway.ovpn"), &runtime)?;
        let mut cmd = s.command();
        cmd.args([
            "create",
            "--pull=never",
            "--name",
            &self.name(p),
            "--network",
            &network,
            "--cap-drop=ALL",
            "--cap-add=NET_ADMIN",
            "--cap-add=NET_RAW",
            "--cap-add=CHOWN",
            "--cap-add=DAC_OVERRIDE",
            "--cap-add=SETUID",
            "--cap-add=SETGID",
            "--device=/dev/net/tun:/dev/net/tun",
            "--security-opt=no-new-privileges:true",
            "--restart=no",
            "--label",
            &format!("{OWNER}={}", self.inner.owner),
            "--label",
            &format!("{PROFILE}={}", p.id),
            "--label",
            &format!("{FINGERPRINT}={}", p.fingerprint),
        ]);
        let mut mounts = vec![("gateway.ovpn", "/gluetun/custom.conf")];
        if p.requires_credentials {
            mounts.extend([
                ("username", "/run/secrets/vpn_username"),
                ("password", "/run/secrets/vpn_password"),
            ]);
        }
        if p.has_key_passphrase {
            mounts.push(("key-passphrase", "/run/secrets/vpn_key_passphrase"));
        }
        for (file, target) in mounts {
            cmd.arg("--mount").arg(format!(
                "type=bind,source={},target={target},readonly",
                dir.join(file).display()
            ));
        }
        for env in [
            "VPN_SERVICE_PROVIDER=custom".to_owned(),
            "VPN_TYPE=openvpn".into(),
            "OPENVPN_CUSTOM_CONFIG=/gluetun/custom.conf".into(),
            "FIREWALL=on".into(),
            format!(
                "DNS_ADDRESS={}",
                if p.dns_address.is_empty() {
                    "127.0.0.1"
                } else {
                    &p.dns_address
                }
            ),
            "HEALTH_SMALL_CHECK_TYPE=icmp".into(),
            format!(
                "HEALTH_ICMP_TARGET_IPS={}",
                ipv4_target(&p.health_icmp_target, false)?
            ),
            format!("HEALTH_TARGET_ADDRESSES={}", p.health_target),
        ] {
            cmd.arg("--env").arg(env);
        }
        if p.requires_credentials {
            for setting in [
                "OPENVPN_USER_SECRETFILE=/run/secrets/vpn_username",
                "OPENVPN_PASSWORD_SECRETFILE=/run/secrets/vpn_password",
            ] {
                cmd.arg("--env").arg(setting);
            }
        }
        if p.has_key_passphrase {
            cmd.arg("--env")
                .arg("OPENVPN_KEY_PASSPHRASE_SECRETFILE=/run/secrets/vpn_key_passphrase");
        }
        if !p.dns_address.is_empty() {
            cmd.arg("--env").arg("DNS_SERVER=off");
        }
        if !p.published_ports.is_empty() {
            cmd.arg("--env").arg(format!(
                "FIREWALL_INPUT_PORTS={}",
                p.published_ports
                    .iter()
                    .map(|p| p.container.to_string())
                    .collect::<Vec<_>>()
                    .join(",")
            ));
        }
        for port in &p.published_ports {
            cmd.arg("--publish")
                .arg(format!("127.0.0.1:{}:{}/tcp", port.host, port.container));
        }
        cmd.arg(IMAGE);
        capture(cmd).await?;
        let v = self
            .gateway(s, p)
            .await?
            .ok_or_else(|| error("The VPN gateway was not created."))?;
        let id = v["Id"]
            .as_str()
            .ok_or_else(|| error("Gateway identity is missing."))?
            .to_owned();
        let mut cmd = s.command();
        cmd.args(["start", &id]);
        capture(cmd).await?;
        self.wait_healthy(s, &id, p).await?;
        Ok(id)
    }
    async fn wait_healthy(&self, s: &DockerSession, id: &str, p: &Profile) -> Result<()> {
        let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(90);
        loop {
            s.verify().await?;
            let v = inspect(s, id).await?;
            if !self.owns(&v, p) {
                return Err(error("VPN gateway ownership changed."));
            }
            if container_state(&v) == "ready" {
                return Ok(());
            }
            if !v["State"]["Running"].as_bool().unwrap_or(false)
                || tokio::time::Instant::now() >= deadline
            {
                return Err(error("The VPN did not become healthy. Check profile credentials, VPN reachability and TCP/TLS and ICMP health targets locally."));
            }
            tokio::time::sleep(std::time::Duration::from_secs(1)).await;
        }
    }
    async fn stop(&self, p: &Profile) -> Result<()> {
        if !self.users(&p.id).is_empty() {
            return Err(error(
                "Stop every service using this VPN before disconnecting or removing it.",
            ));
        }
        let s = DockerSession::pinned(&self.inner.dir).await?;
        s.verify().await?;
        let Some(v) = self.gateway(&s, p).await? else {
            return Ok(());
        };
        let id = v["Id"]
            .as_str()
            .ok_or_else(|| error("Gateway identity is missing."))?;
        let mut cmd = s.command();
        cmd.args(["container", "ls", "--quiet", "--no-trunc"]);
        let ids = String::from_utf8(capture(cmd).await?)
            .map_err(|_| error("Docker returned invalid container identities."))?;
        for other in ids.lines().filter(|other| *other != id) {
            let other = inspect(&s, other).await?;
            if namespace_uses(&s, other, id, &self.name(p)).await? {
                return Err(error("A running Docker container still shares this VPN network. Stop it before disconnecting or removing the profile."));
            }
        }
        let current = inspect(&s, id).await?;
        if !self.owns(&current, p) {
            return Err(error("VPN gateway ownership changed; it was preserved."));
        }
        let mut cmd = s.command();
        cmd.args(["stop", "--time", "10", id]);
        capture(cmd).await?;
        let stopped = inspect(&s, id).await?;
        if stopped["State"]["Running"].as_bool() != Some(false) {
            return Err(error(
                "The VPN gateway did not stop; profile files were preserved.",
            ));
        }
        Ok(())
    }
}
fn find<'a>(profiles: &'a [Profile], id: &str) -> Result<&'a Profile> {
    profiles
        .iter()
        .find(|p| p.id == id)
        .ok_or_else(|| error("VPN profile no longer exists."))
}
async fn inspect(s: &DockerSession, id: &str) -> Result<Value> {
    let mut cmd = s.command();
    cmd.args(["container", "inspect", id]);
    let mut v: Vec<Value> = serde_json::from_slice(&capture(cmd).await?)
        .map_err(|_| error("Docker returned invalid gateway metadata."))?;
    v.pop()
        .ok_or_else(|| error("Docker gateway was not found."))
}
async fn namespace_uses(
    s: &DockerSession,
    mut container: Value,
    gateway: &str,
    name: &str,
) -> Result<bool> {
    let mut visited = HashSet::new();
    for _ in 0..128 {
        let Some(target) = container["HostConfig"]["NetworkMode"]
            .as_str()
            .and_then(|mode| mode.strip_prefix("container:"))
            .map(str::to_owned)
        else {
            return Ok(false);
        };
        if target == gateway
            || target == name
            || (!target.is_empty() && gateway.starts_with(&target))
        {
            return Ok(true);
        }
        if target.is_empty() || !visited.insert(target.clone()) {
            return Err(error(
                "Docker network namespace references could not be verified; the VPN was preserved.",
            ));
        }
        container = inspect(s, &target).await?;
    }
    Err(error(
        "Docker network namespace references exceed the verification limit; the VPN was preserved.",
    ))
}
fn container_state(v: &Value) -> &'static str {
    if !v["State"]["Running"].as_bool().unwrap_or(false) {
        "disconnected"
    } else {
        match v["State"]["Health"]["Status"].as_str() {
            Some("healthy") => "ready",
            Some("starting") => "connecting",
            _ => "failed",
        }
    }
}
impl VpnLease {
    pub async fn healthy(&self) -> Result<bool> {
        self.session.verify().await?;
        let v = inspect(&self.session, &self.gateway_id).await?;
        let l = &v["Config"]["Labels"];
        Ok(l[OWNER].as_str() == Some(&self.inner.owner)
            && l[PROFILE].as_str() == Some(&self.profile)
            && l[FINGERPRINT].as_str() == Some(&self.fingerprint)
            && v["Id"].as_str() == Some(&self.gateway_id)
            && container_state(&v) == "ready")
    }
}
fn release_token(inner: &Inner, profile: &str, token: &str) {
    let mut leases = inner.leases.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(runs) = leases.get_mut(profile) {
        runs.remove(token);
        if runs.is_empty() {
            leases.remove(profile);
        }
    }
}
impl Drop for VpnLease {
    fn drop(&mut self) {
        release_token(&self.inner, &self.profile, &self.token);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn remote_normalization_preserves_imported_lines_and_inline_material() {
        let original=b"client\r\n  remote localhost 1194 udp # fixture\r\n<ca>\r\nremote untouched.example 443\r\n</ca>\r\nremote 192.0.2.1 443 tcp\r\n";
        let normalized = runtime_config(original, false).await.unwrap();
        assert_eq!(normalized,b"client\r\n  remote 127.0.0.1 1194 udp # fixture\r\n<ca>\r\nremote untouched.example 443\r\n</ca>\r\nremote 192.0.2.1 443 tcp\r\n");
        assert!(std::str::from_utf8(original)
            .unwrap()
            .contains("remote localhost"));
        assert!(runtime_config(b"remote ::1 1194", false).await.is_err());
    }
    #[test]
    fn remote_parser_bounds_arguments_and_directive_count() {
        for line in [
            "remote",
            "remote host 0",
            "remote host 70000",
            "remote host 443 udp extra",
            "remote host 443 tcp6",
            "remote bad/host 443",
        ] {
            assert!(remote_host(line).is_err(), "{line}");
        }
        assert!(validate_config("remote 192.0.2.1 1194\n".repeat(33).as_bytes()).is_err());
        assert!(validate_config("remote 192.0.2.1 1194\n".repeat(32).as_bytes()).is_ok());
        assert_eq!(
            remote_host("  --remote localhost 1194 # fixture")
                .unwrap()
                .unwrap()
                .2,
            "localhost"
        );
    }
    #[test]
    fn config_detects_authentication_and_accepts_safe_client_compatibility() {
        assert!(
            validate_config(b"client\nauth-user-pass")
                .unwrap()
                .requires_credentials
        );
        let auth=validate_config(b"client\n<key>\n-----BEGIN ENCRYPTED PRIVATE KEY-----\nsynthetic\n-----END ENCRYPTED PRIVATE KEY-----\n</key>").unwrap();
        assert!(!auth.requires_credentials);
        assert!(auth.encrypted_key);
        assert!(validate_config(b"client\nignore-unknown-option data-ciphers block-outside-dns\nblock-outside-dns\nsetenv UV_ID synthetic-id\nsetenv UV_NAME synthetic-name\nsetenv CLIENT_CERT 0\ndev-type tun\npush-peer-info\nhand-window 60\nserver-poll-timeout 30").is_ok());
        for config in [
            "setenv PATH /tmp",
            "setenv LD_PRELOAD /tmp/lib",
            "setenv UV_bad value",
            "ignore-unknown-option script-security",
            "dev-type tap",
            "push-peer-info extra",
            "hand-window 0",
            "server-poll-timeout -1",
            "askpass /tmp/keypass",
        ] {
            assert!(validate_config(config.as_bytes()).is_err());
        }
    }
    #[tokio::test]
    async fn certificate_profiles_omit_auth_secrets_and_keep_key_passphrase_private() {
        let temp = tempfile::tempdir().unwrap();
        let manager = VpnManager::new(temp.path().join("state")).unwrap();
        let plain = temp.path().join("plain.ovpn");
        fs::write(&plain, b"client\n<key>\nsynthetic-private-key\n</key>\n").unwrap();
        let userpass = temp.path().join("userpass.ovpn");
        fs::write(&userpass, b"client\nauth-user-pass\n").unwrap();
        let encrypted = temp.path().join("encrypted.ovpn");
        let encrypted_bytes=b"client\n<key>\n-----BEGIN ENCRYPTED PRIVATE KEY-----\nsynthetic-only\n-----END ENCRYPTED PRIVATE KEY-----\n</key>\n";
        fs::write(&encrypted, encrypted_bytes).unwrap();
        let input = |name: &str, path: &Path| VpnImport {
            name: name.into(),
            config_path: path.to_string_lossy().into(),
            username: String::new(),
            password: String::new(),
            key_passphrase: String::new(),
            published_ports: vec![],
            health_target: String::new(),
            dns_address: String::new(),
            health_icmp_target: String::new(),
        };
        let view = manager.import(input("Certificate", &plain)).await.unwrap();
        let dir = manager.inner.dir.join(&view.id);
        assert!(!dir.join("username").exists());
        assert!(!dir.join("password").exists());
        assert!(!dir.join("key-passphrase").exists());
        assert!(manager
            .import(input("User password", &userpass))
            .await
            .is_err());
        assert!(manager
            .import(input("Encrypted missing", &encrypted))
            .await
            .is_err());
        let mut request = input("Encrypted", &encrypted);
        request.key_passphrase = "synthetic-key-passphrase".into();
        let view = manager.import(request).await.unwrap();
        let dir = manager.inner.dir.join(&view.id);
        assert_eq!(
            read_private(&dir.join("client.ovpn")).unwrap(),
            encrypted_bytes
        );
        assert_eq!(
            read_private(&dir.join("key-passphrase")).unwrap(),
            b"synthetic-key-passphrase"
        );
        assert_eq!(
            fs::metadata(dir.join("key-passphrase"))
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o600
        );
        assert!(!dir.join("username").exists());
        assert!(!serde_json::to_string(&view)
            .unwrap()
            .contains("synthetic-key-passphrase"));
        assert!(!String::from_utf8(
            read_private(&manager.inner.dir.join("profiles.json")).unwrap()
        )
        .unwrap()
        .contains("synthetic-key-passphrase"));
        let runtime = runtime_config(encrypted_bytes, true).await.unwrap();
        assert!(runtime.starts_with(encrypted_bytes));
        assert!(runtime.ends_with(b"askpass /etc/openvpn/askpass\n"));
        assert!(!runtime
            .windows(b"synthetic-key-passphrase".len())
            .any(|part| part == b"synthetic-key-passphrase"));
    }
    #[test]
    fn config_rejects_external_code_and_files() {
        for config in [
            "client\nup /tmp/run",
            "ca ca.pem",
            "auth-user-pass creds",
            "plugin x",
            "management 127.0.0.1 1",
            "log out",
            "config other",
            "<connection>\nremote x\n</connection>",
        ] {
            assert!(validate_config(config.as_bytes()).is_err(), "{config}");
        }
        assert!(validate_config(b"client\ndev tun\nremote vpn.example 1194\nauth-user-pass\n<ca>\ncertificate\n</ca>\n<key>\nprivate\n</key>").is_ok());
    }
    #[test]
    fn private_dns_and_icmp_targets_are_explicit_and_safe() {
        assert_eq!(ipv4_target("", true).unwrap(), "");
        assert_eq!(ipv4_target("", false).unwrap(), "0.0.0.0");
        assert_eq!(ipv4_target("10.30.0.53", true).unwrap(), "10.30.0.53");
        assert_eq!(ipv4_target("0.0.0.0", false).unwrap(), "0.0.0.0");
        for address in [
            "127.0.0.1",
            "224.0.0.1",
            "255.255.255.255",
            "::1",
            "localhost",
        ] {
            assert!(ipv4_target(address, true).is_err());
            assert!(ipv4_target(address, false).is_err());
        }
        assert!(ipv4_target("0.0.0.0", true).is_err());
        assert!(health_target("database.internal:5432").is_ok());
    }
    #[test]
    fn ports_and_tls_targets_are_bounded() {
        assert!(validate_ports(&[
            VpnPortMapping {
                host: 8080,
                container: 80
            },
            VpnPortMapping {
                host: 8081,
                container: 80
            }
        ])
        .is_err());
        assert!(validate_ports(&[VpnPortMapping {
            host: 8080,
            container: 9999
        }])
        .is_err());
        for target in [
            "localhost:443",
            "127.0.0.1:9999",
            "[::1]:443",
            "user@host:443",
            "host:0",
            "host:443/path",
            "host:443?x",
            "host",
        ] {
            assert!(health_target(target).is_err(), "{target}");
        }
        assert_eq!(health_target("").unwrap(), "cloudflare.com:443");
        assert!(health_target("private.example:443").is_ok());
    }
    #[tokio::test]
    async fn import_rejects_symlinks_and_oversized_files_without_saving_profiles() {
        let temp = tempfile::tempdir().unwrap();
        let manager = VpnManager::new(temp.path().join("state")).unwrap();
        let source = temp.path().join("source.ovpn");
        fs::write(&source, b"client\ndev tun\nauth-user-pass").unwrap();
        let link = temp.path().join("link.ovpn");
        std::os::unix::fs::symlink(&source, &link).unwrap();
        let large = temp.path().join("large.ovpn");
        fs::File::create(&large)
            .unwrap()
            .set_len(1_048_577)
            .unwrap();
        for source in [link, large] {
            let result = manager
                .import(VpnImport {
                    name: "Rejected fixture".into(),
                    config_path: source.to_string_lossy().into(),
                    key_passphrase: String::new(),
                    username: "fixture".into(),
                    password: "synthetic-only".into(),
                    published_ports: vec![],
                    health_target: String::new(),
                    dns_address: String::new(),
                    health_icmp_target: String::new(),
                })
                .await;
            assert!(result.is_err());
            assert!(manager.inner.profiles.lock().await.is_empty());
        }
        assert!(!manager.inner.dir.join("profiles.json").exists());
    }
    #[tokio::test]
    async fn import_keeps_secrets_private_and_survives_restart() {
        let temp = tempfile::tempdir().unwrap();
        let config = temp.path().join("input.ovpn");
        fs::write(&config, b"client\ndev tun\nauth-user-pass").unwrap();
        let m = VpnManager::new(temp.path().join("state")).unwrap();
        let view = m
            .import(VpnImport {
                name: "Office".into(),
                config_path: config.to_string_lossy().into(),
                key_passphrase: String::new(),
                username: "private-user".into(),
                password: "private-pass".into(),
                published_ports: vec![],
                health_target: String::new(),
                dns_address: String::new(),
                health_icmp_target: String::new(),
            })
            .await
            .unwrap();
        let serialized = serde_json::to_string(&view).unwrap();
        assert!(!serialized.contains("private-user"));
        assert!(!serialized.contains("private-pass"));
        let dir = m.inner.dir.join(&view.id);
        assert_eq!(
            fs::metadata(&dir).unwrap().permissions().mode() & 0o777,
            0o700
        );
        for file in ["client.ovpn", "username", "password"] {
            assert_eq!(
                fs::metadata(dir.join(file)).unwrap().permissions().mode() & 0o777,
                0o600
            );
        }
        let restored = VpnManager::new(temp.path().join("state")).unwrap();
        assert_eq!(restored.inner.profiles.lock().await[0].id, view.id);
        assert_eq!(restored.inner.owner, m.inner.owner);
    }
    #[test]
    fn releasing_one_run_preserves_other_runs_and_profiles() {
        let temp = tempfile::tempdir().unwrap();
        let m = VpnManager::new(temp.path().into()).unwrap();
        let user = ServiceRef {
            project_id: "project".into(),
            service_id: "service".into(),
        };
        {
            let mut leases = m.inner.leases.lock().unwrap();
            leases
                .entry("a".into())
                .or_default()
                .insert("old".into(), user.clone());
            leases
                .entry("a".into())
                .or_default()
                .insert("new".into(), user.clone());
            leases
                .entry("b".into())
                .or_default()
                .insert("other".into(), user.clone());
        }
        assert_eq!(m.users("a").len(), 1);
        release_token(&m.inner, "a", "old");
        assert_eq!(m.users("a"), vec![user.clone()]);
        release_token(&m.inner, "a", "old");
        assert_eq!(m.users("a").len(), 1);
        release_token(&m.inner, "a", "new");
        assert!(m.users("a").is_empty());
        assert_eq!(m.users("b"), vec![user]);
    }
    #[test]
    fn exact_ownership_is_required() {
        let temp = tempfile::tempdir().unwrap();
        let m = VpnManager::new(temp.path().into()).unwrap();
        let p = Profile {
            id: "p".into(),
            name: "P".into(),
            published_ports: vec![],
            health_target: "host:443".into(),
            dns_address: String::new(),
            health_icmp_target: String::new(),
            fingerprint: "fp".into(),
            requires_credentials: true,
            has_key_passphrase: false,
        };
        let mut labels = serde_json::Map::new();
        labels.insert(OWNER.into(), m.inner.owner.clone().into());
        labels.insert(PROFILE.into(), "p".into());
        labels.insert(FINGERPRINT.into(), "fp".into());
        let mut value = serde_json::json!({"Config":{"Labels":labels}});
        assert!(m.owns(&value, &p));
        value["Config"]["Labels"][OWNER] = "foreign".into();
        assert!(!m.owns(&value, &p));
    }
}
