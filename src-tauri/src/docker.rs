use crate::model::{AppError, Result};
use std::{
    path::{Path, PathBuf},
    process::Stdio,
    time::Duration,
};
use tokio::{io::AsyncReadExt, process::Command};
fn failure(message: &str) -> AppError {
    AppError::new("DOCKER_FAILED", message)
}
#[derive(Clone, Debug)]
pub(crate) struct DockerSession {
    binary: PathBuf,
    host: String,
    identity: String,
}
impl DockerSession {
    pub(crate) async fn local() -> Result<Self> {
        let binary = std::env::var_os("PATH")
            .into_iter()
            .flat_map(|p| std::env::split_paths(&p).collect::<Vec<_>>())
            .map(|p| p.join("docker"))
            .chain([
                PathBuf::from("/opt/homebrew/bin/docker"),
                PathBuf::from("/usr/local/bin/docker"),
            ])
            .find(|p| p.is_file())
            .ok_or_else(|| failure("Install Docker before using container services."))?;
        let mut context = Command::new(&binary);
        context.env_remove("DOCKER_HOST").args([
            "context",
            "inspect",
            "--format",
            "{{.Endpoints.docker.Host}}",
        ]);
        let host = String::from_utf8(capture(context).await?)
            .map_err(|_| failure("Invalid Docker context."))?
            .trim()
            .to_owned();
        if !host.starts_with("unix:///") || host.contains(['\n', '\r']) {
            return Err(failure(
                "Only a local Unix-socket Docker context is supported.",
            ));
        }
        let mut session = Self {
            binary,
            host,
            identity: String::new(),
        };
        session.identity = session.daemon_id().await?;
        Ok(session)
    }
    pub(crate) async fn pinned(dir: &Path) -> Result<Self> {
        let session = Self::local().await?;
        session.bind(dir)?;
        Ok(session)
    }
    fn bind(&self, dir: &Path) -> Result<()> {
        // App-wide instance lock protects ordinary operation; this lock also covers isolated managers.
        static PIN_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
        let _lock = PIN_LOCK
            .lock()
            .map_err(|_| failure("Docker binding lock is unavailable."))?;
        let path = dir.join("docker-binding.json");
        if let Ok(meta) = std::fs::symlink_metadata(&path) {
            if !meta.is_file() || meta.file_type().is_symlink() || meta.len() > 4096 {
                return Err(failure(
                    "Invalid private Docker binding. Existing VPN profiles were preserved.",
                ));
            }
            let value: serde_json::Value = serde_json::from_slice(&std::fs::read(&path)?)
                .map_err(|_| failure("Invalid private Docker binding."))?;
            if value["host"].as_str() != Some(&self.host)
                || value["identity"].as_str() != Some(&self.identity)
            {
                return Err(failure("VPN profiles belong to another local Docker daemon. Switch back to their original Docker context; existing gateways and profile files were preserved."));
            }
        } else {
            crate::project::secure_write(
                &path,
                &serde_json::to_vec(
                    &serde_json::json!({"host":self.host,"identity":self.identity}),
                )
                .map_err(|_| failure("Cannot save private Docker binding."))?,
            )?;
        }
        Ok(())
    }
    pub(crate) fn command(&self) -> Command {
        let mut cmd = Command::new(&self.binary);
        cmd.env_remove("DOCKER_CONTEXT")
            .env_remove("DOCKER_HOST")
            .args(["--host", &self.host]);
        cmd
    }
    async fn daemon_id(&self) -> Result<String> {
        let mut cmd = self.command();
        cmd.args(["info", "--format", "{{.ID}}"]);
        let id = String::from_utf8(capture(cmd).await?)
            .map_err(|_| failure("Invalid Docker daemon identity."))?
            .trim()
            .to_owned();
        if id.is_empty() || id.contains(char::is_whitespace) {
            return Err(failure("Cannot establish Docker daemon identity."));
        }
        Ok(id)
    }
    pub(crate) async fn verify(&self) -> Result<()> {
        if self.daemon_id().await? != self.identity {
            return Err(failure(
                "Docker daemon changed. Existing containers were not modified.",
            ));
        }
        Ok(())
    }
}
pub async fn pull_image(image: &str) -> Result<()> {
    crate::model::validate_vpn_image(image)?;
    let session = DockerSession::local().await?;
    let mut cmd = session.command();
    cmd.args(["pull", image])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .process_group(0)
        .kill_on_drop(true);
    let mut child = cmd
        .spawn()
        .map_err(|_| failure("Docker image download could not start."))?;
    let _guard = GroupGuard(child.id().unwrap());
    let status = tokio::time::timeout(Duration::from_secs(600), child.wait())
        .await
        .map_err(|_| failure("Image download timed out."))??;
    if !status.success() {
        return Err(failure(
            "Image download failed. Check registry access and image name.",
        ));
    }
    Ok(())
}
struct GroupGuard(u32);
impl Drop for GroupGuard {
    fn drop(&mut self) {
        let _ = nix::sys::signal::killpg(
            nix::unistd::Pid::from_raw(self.0 as i32),
            nix::sys::signal::Signal::SIGKILL,
        );
    }
}

pub(crate) async fn capture(mut command: Command) -> Result<Vec<u8>> {
    command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .process_group(0)
        .kill_on_drop(true);
    let mut child = command.spawn().map_err(|_| {
        failure("Docker could not start. Install Docker and make its CLI available.")
    })?;
    let pid = child.id().unwrap();
    let _guard = GroupGuard(pid);
    let mut stdout = child.stdout.take().unwrap();
    let result = tokio::time::timeout(Duration::from_secs(15), async {
        let mut bytes = Vec::new();
        (&mut stdout).take(1_048_577).read_to_end(&mut bytes).await?;
        if bytes.len() > 1_048_576 { return Err(failure("Docker output exceeded the private preflight limit.")); }
        if !child.wait().await?.success() { return Err(failure("Docker rejected the operation. Check the local daemon, Compose file and required local images.")); }
        Ok(bytes)
    }).await.unwrap_or_else(|_| Err(failure("Docker did not respond within 15 seconds.")));
    // The CLI may launch a plugin; never leave preflight/helper descendants behind.
    if nix::sys::signal::killpg(nix::unistd::Pid::from_raw(pid as i32), None).is_ok() {
        let _ = nix::sys::signal::killpg(
            nix::unistd::Pid::from_raw(pid as i32),
            nix::sys::signal::Signal::SIGKILL,
        );
    }
    let _ = child.wait().await;
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn vpn_binding_persists_and_refuses_context_or_daemon_replacement() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let a = DockerSession {
            binary: "docker".into(),
            host: "unix:///a.sock".into(),
            identity: "daemon-a".into(),
        };
        a.bind(dir.path()).unwrap();
        let path = dir.path().join("docker-binding.json");
        let original = std::fs::read(&path).unwrap();
        assert_eq!(
            std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o600
        );
        a.bind(dir.path()).unwrap();
        for changed in [
            DockerSession {
                host: "unix:///b.sock".into(),
                ..a.clone()
            },
            DockerSession {
                identity: "daemon-b".into(),
                ..a.clone()
            },
        ] {
            assert!(changed.bind(dir.path()).is_err());
            assert_eq!(std::fs::read(&path).unwrap(), original);
        }
        std::fs::remove_file(&path).unwrap();
        std::os::unix::fs::symlink(dir.path().join("missing"), &path).unwrap();
        assert!(a.bind(dir.path()).is_err());
        assert!(!dir.path().join("missing").exists());
    }
}
