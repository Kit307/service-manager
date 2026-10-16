use crate::{
    model::*,
    project::{hash, secure_write},
};
use serde_json::Value;
use std::{
    path::{Path, PathBuf},
    process::Stdio,
    time::Duration,
};
use tokio::{io::AsyncReadExt, process::Command};

pub struct ComposeLaunch {
    docker: PathBuf,
    host: String,
    pub name: String,
    owner: String,
    cwd: PathBuf,
    file: PathBuf,
    expected: Vec<String>,
    private_snapshot: bool,
}

pub struct ComposeJob {
    pub project: String,
    pub service: String,
    pub run: String,
    pub root: PathBuf,
    pub cwd: PathBuf,
    pub definition: ServiceDefinition,
    pub state_dir: PathBuf,
    pub owner: String,
}

fn failure(message: &str) -> AppError {
    AppError::new("COMPOSE_FAILED", message)
}

// Expanded configuration can contain credentials; never return Docker output in errors.
async fn capture(mut command: Command) -> Result<Vec<u8>> {
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

impl ComposeLaunch {
    pub async fn prepare(
        root: &Path,
        cwd: &Path,
        definition: &ServiceDefinition,
        state: &Path,
        project: &str,
        owner: &str,
    ) -> Result<Self> {
        let docker = std::env::var_os("PATH")
            .into_iter()
            .flat_map(|p| std::env::split_paths(&p).collect::<Vec<_>>())
            .map(|p| p.join("docker"))
            .chain([
                PathBuf::from("/opt/homebrew/bin/docker"),
                PathBuf::from("/usr/local/bin/docker"),
            ])
            .find(|p| p.is_file())
            .ok_or_else(|| failure("Install the Docker CLI before running Compose."))?;
        let mut context = Command::new(&docker);
        context.args([
            "context",
            "inspect",
            "--format",
            "{{.Endpoints.docker.Host}}",
        ]);
        let host = String::from_utf8(capture(context).await?)
            .map_err(|_| failure("Invalid Docker context."))?
            .trim()
            .to_owned();
        if !host.starts_with("unix:///") {
            return Err(failure("Only a local Unix-socket Docker context is supported. Remote contexts are never controlled."));
        }
        let ServiceKind::Compose { file } = &definition.kind else {
            return Err(failure("A Compose definition is required."));
        };
        let name = format!(
            "sm-{}",
            &hash(format!("{project}/{}", definition.id).as_bytes())[..24]
        );
        let source = inside(root, file)?;
        let mut launch = Self {
            docker,
            host,
            name,
            owner: owner.into(),
            cwd: cwd.into(),
            file: source,
            expected: Vec::new(),
            private_snapshot: false,
        };
        let mut config = launch.command();
        if let Some(file) = &definition.env_file {
            config.arg("--env-file").arg(inside(root, file)?);
        }
        config.args(["config", "--format", "json"]);
        let mut value: Value = serde_json::from_slice(&capture(config).await?)
            .map_err(|_| failure("Compose did not return a valid configuration."))?;
        isolate(&mut value, owner)?;
        launch.expected = value["services"]
            .as_object()
            .unwrap()
            .keys()
            .cloned()
            .collect();
        for (section, kind) in [("volumes", "volume"), ("networks", "network")] {
            if let Some(items) = value[section].as_object() {
                for resource in items.values() {
                    if let Some(name) = resource["name"].as_str() {
                        let mut list = launch.docker();
                        list.args([
                            kind,
                            "ls",
                            "--filter",
                            &format!("name={name}"),
                            "--format",
                            "{{.Name}}",
                        ]);
                        let names = String::from_utf8(capture(list).await?)
                            .map_err(|_| failure("Invalid resource identities."))?;
                        if names.lines().any(|n| n == name) {
                            let mut inspect = launch.docker();
                            inspect.args([kind, "inspect", name]);
                            let existing: Vec<Value> =
                                serde_json::from_slice(&capture(inspect).await?)
                                    .map_err(|_| failure("Cannot inspect Compose resources."))?;
                            if existing.len() != 1
                                || existing[0]["Labels"]["dev.service-manager.owner"] != owner
                            {
                                return Err(failure("A Compose resource exists without this service's ownership label. No resources were changed."));
                            }
                        }
                    }
                }
            }
        }

        // Freeze resolved paths and environment for Stop, independent of later project edits.
        launch.file = state.join(format!("compose-{}.json", uuid::Uuid::new_v4()));
        secure_write(
            &launch.file,
            &serde_json::to_vec(&value)
                .map_err(|_| failure("Cannot save the private Compose launch snapshot."))?,
        )?;
        launch.private_snapshot = true;
        let existing = launch.containers().await?;
        if existing.iter().any(|c| {
            matches!(
                c["State"]["Status"].as_str(),
                Some("running" | "restarting" | "paused")
            )
        }) {
            return Err(failure("Owned containers are already active from another session. Stop them locally before starting a new run."));
        }
        Ok(launch)
    }
    fn docker(&self) -> Command {
        let mut cmd = Command::new(&self.docker);
        cmd.env_remove("DOCKER_CONTEXT")
            .env_remove("DOCKER_HOST")
            .arg("--host")
            .arg(&self.host)
            .current_dir(&self.cwd);
        cmd
    }
    pub fn command(&self) -> Command {
        let mut cmd = self.docker();
        cmd.args([
            "compose",
            "--project-name",
            &self.name,
            "--project-directory",
        ])
        .arg(&self.cwd)
        .arg("--file")
        .arg(&self.file);
        cmd
    }
    pub fn up(&self) -> Command {
        let mut cmd = self.command();
        cmd.args(["up", "--no-color", "--no-build", "--pull", "never"]);
        cmd
    }
    pub async fn containers(&self) -> Result<Vec<Value>> {
        let mut ps = self.docker();
        ps.args([
            "ps",
            "--all",
            "--quiet",
            "--filter",
            &format!("label=com.docker.compose.project={}", self.name),
        ]);
        let ids = String::from_utf8(capture(ps).await?)
            .map_err(|_| failure("Invalid container identities."))?;
        let ids: Vec<_> = ids.split_whitespace().collect();
        if ids.is_empty() {
            return Ok(vec![]);
        }
        let mut inspect = self.docker();
        inspect.arg("inspect").args(ids);
        let values: Vec<Value> = serde_json::from_slice(&capture(inspect).await?)
            .map_err(|_| failure("Cannot inspect owned containers."))?;
        if values.iter().any(|v| {
            v["Config"]["Labels"]["dev.service-manager.owner"].as_str() != Some(&self.owner)
                || v["Config"]["Labels"]["com.docker.compose.project"].as_str() != Some(&self.name)
        }) {
            return Err(failure("Container ownership does not match this local service. No containers were changed."));
        }
        Ok(values)
    }
    pub async fn state(&self) -> Result<(&'static str, &'static str)> {
        let containers = self.containers().await?;
        if containers.len() < self.expected.len() {
            return Ok(
                if containers.iter().any(|c| {
                    c["State"]["Running"] == true
                        || c["State"]["Restarting"] == true
                        || c["State"]["Paused"] == true
                }) {
                    ("running", "unhealthy")
                } else {
                    ("starting", "checking")
                },
            );
        }
        let containers: Vec<_> = containers
            .iter()
            .filter(|c| {
                self.expected
                    .iter()
                    .any(|name| c["Config"]["Labels"]["com.docker.compose.service"] == *name)
            })
            .collect();
        let running = self.expected.iter().all(|name| {
            containers
                .iter()
                .filter(|c| {
                    c["Config"]["Labels"]["com.docker.compose.service"] == *name
                        && c["State"]["Status"] == "running"
                })
                .count()
                == 1
        });
        if running {
            let healthy = containers
                .iter()
                .all(|c| c["State"]["Health"]["Status"] == "healthy");
            let health = if healthy {
                "healthy"
            } else if containers
                .iter()
                .any(|c| c["State"]["Health"]["Status"] == "unhealthy")
            {
                "unhealthy"
            } else if containers
                .iter()
                .any(|c| c["State"]["Health"]["Status"] == "starting")
            {
                "checking"
            } else {
                "notConfigured"
            };
            return Ok((if healthy { "ready" } else { "running" }, health));
        }
        if containers.iter().any(|c| {
            c["State"]["Running"] == true
                || c["State"]["Paused"] == true
                || c["State"]["Restarting"] == true
        }) {
            return Ok(("running", "unhealthy"));
        }
        Ok(("failed", "unhealthy"))
    }
    pub async fn stop(&self) -> Result<()> {
        self.containers().await?;
        let mut stop = self.command();
        stop.args(["stop", "--timeout", "5"]);
        capture(stop).await?;
        if self.containers().await?.iter().any(|c| {
            c["State"]["Running"] == true
                || c["State"]["Paused"] == true
                || c["State"]["Restarting"] == true
        }) {
            return Err(failure(
                "Owned containers are still active. Stop has not completed.",
            ));
        }
        Ok(())
    }
}

impl Drop for ComposeLaunch {
    fn drop(&mut self) {
        if self.private_snapshot {
            let _ = std::fs::remove_file(&self.file);
        }
    }
}

fn isolate(value: &mut Value, owner: &str) -> Result<()> {
    for section in ["volumes", "networks", "secrets", "configs"] {
        if matches!(section, "secrets" | "configs")
            && value[section].as_object().is_some_and(|v| !v.is_empty())
        {
            return Err(failure(
                "Compose secrets and configs require separate management.",
            ));
        }
        if value[section].as_object().is_some_and(|items| {
            items
                .values()
                .any(|v| v["external"].as_bool() == Some(true))
        }) {
            return Err(failure("External Compose resources require separate management. Use project-owned resources."));
        }
        if let Some(items) = value[section].as_object() {
            for resource in items.values() {
                if let Some(name) = resource["name"].as_str() {
                    let project = value["name"].as_str().unwrap_or("");
                    if !name.starts_with(&format!("{project}_")) {
                        return Err(failure("Explicit shared resource names are not supported. Remove the name override."));
                    }
                }
            }
        }
    }
    for section in ["volumes", "networks"] {
        if let Some(items) = value[section].as_object_mut() {
            for resource in items.values_mut() {
                if let Some(object) = resource.as_object_mut() {
                    let labels = object
                        .entry("labels")
                        .or_insert_with(|| serde_json::json!({}));
                    labels
                        .as_object_mut()
                        .ok_or_else(|| failure("Resource labels must be a mapping."))?
                        .insert("dev.service-manager.owner".into(), owner.into());
                }
            }
        }
    }
    let services = value["services"]
        .as_object_mut()
        .filter(|s| !s.is_empty())
        .ok_or_else(|| failure("The Compose file has no enabled services."))?;
    for service in services.values_mut() {
        if service["scale"].as_u64().is_some_and(|n| n != 1)
            || service["deploy"]["replicas"]
                .as_u64()
                .is_some_and(|n| n != 1)
        {
            return Err(failure("Managed Compose supports one long-running container per service. Remove scaling overrides."));
        }

        if service.get("container_name").is_some()
            || service.get("profiles").is_some()
            || service.get("extends").is_some()
        {
            return Err(failure("container_name, profiles and extends are not supported in managed Compose services."));
        }
        let labels = service
            .as_object_mut()
            .unwrap()
            .entry("labels")
            .or_insert_with(|| serde_json::json!({}));
        labels
            .as_object_mut()
            .ok_or_else(|| failure("Compose labels must be a mapping."))?
            .insert("dev.service-manager.owner".into(), owner.into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn refuses_shared_resources_and_injects_ownership() {
        let mut good = serde_json::json!({"name":"sm-test","services":{"web":{"image":"fixture"}},"volumes":{"data":{"name":"sm-test_data"}}});
        isolate(&mut good, "owner").unwrap();
        assert_eq!(
            good["services"]["web"]["labels"]["dev.service-manager.owner"],
            "owner"
        );
        for bad in [
            serde_json::json!({"services":{"web":{"container_name":"shared"}}}),
            serde_json::json!({"services":{"web":{}},"volumes":{"data":{"external":true}}}),
            serde_json::json!({"services":{"web":{}},"networks":{"default":{"name":"shared"}}}),
        ] {
            assert!(isolate(&mut bad.clone(), "owner").is_err());
        }
    }
}
