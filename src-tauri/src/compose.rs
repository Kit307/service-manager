use crate::{
    model::*,
    project::{hash, secure_write},
};
use serde_json::Value;
use std::path::{Path, PathBuf};
use tokio::process::Command;

pub struct ComposeLaunch {
    session: crate::docker::DockerSession,
    pub name: String,
    owner: String,
    cwd: PathBuf,
    file: PathBuf,
    expected: Vec<String>,
    private_snapshot: bool,
    private_files: Vec<PathBuf>,
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

// Expanded configuration can contain credentials; never return Docker output.
async fn capture(command: Command) -> Result<Vec<u8>> {
    crate::docker::capture(command)
        .await
        .map_err(|e| failure(&e.message))
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
        let session = crate::docker::DockerSession::local().await?;
        let ServiceKind::Compose { file } = &definition.kind else {
            return Err(failure("A Compose definition is required."));
        };
        let name = format!(
            "sm-{}",
            &hash(format!("{project}/{}", definition.id).as_bytes())[..24]
        );
        let source = inside(root, file)?;
        let mut launch = Self {
            session,
            name,
            owner: owner.into(),
            cwd: cwd.into(),
            file: source,
            expected: Vec::new(),
            private_snapshot: false,
            private_files: Vec::new(),
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
    pub async fn prepare_vpn(
        root: &Path,
        cwd: &Path,
        definition: &ServiceDefinition,
        state: &Path,
        project: &str,
        owner: &str,
        lease: &crate::vpn::VpnLease,
    ) -> Result<Self> {
        let vpn = definition
            .vpn
            .as_ref()
            .ok_or_else(|| failure("VPN selection is required."))?;
        crate::model::validate_vpn_image(&vpn.image)?;
        let ServiceKind::Command { command } = &definition.kind else {
            return Err(failure("VPN supports command services only."));
        };
        lease.session.verify().await?;
        if !lease.healthy().await? {
            return Err(failure("VPN gateway is not healthy."));
        }
        let mut inspect = lease.session.command();
        inspect.args(["image", "inspect", "--format", "{{.Id}}", &vpn.image]);
        let image = String::from_utf8(capture(inspect).await?)
            .map_err(|_| failure("Invalid local container image."))?
            .trim()
            .to_owned();
        if !image.starts_with("sha256:")
            || image.len() != 71
            || !image[7..].bytes().all(|b| b.is_ascii_hexdigit())
        {
            return Err(failure(
                "Download this service image in VPN settings before Run.",
            ));
        }
        let relative = cwd
            .strip_prefix(root)
            .map_err(|_| failure("Container cwd must remain inside the project."))?;
        let name = format!(
            "sm-{}",
            &hash(format!("{project}/{}", definition.id).as_bytes())[..24]
        );
        let resolver = state.join(format!("vpn-resolver-{}", uuid::Uuid::new_v4()));
        let mut launch = Self {
            session: lease.session.clone(),
            name,
            owner: owner.into(),
            cwd: cwd.into(),
            file: state.join(format!("compose-{}.json", uuid::Uuid::new_v4())),
            expected: vec!["worker".into()],
            private_snapshot: true,
            private_files: vec![resolver.clone()],
        };
        secure_write(
            &resolver,
            format!("nameserver {}\noptions ndots:0\n", lease.dns_address).as_bytes(),
        )?;
        let mut environment = serde_json::Map::new();
        if let Some(file) = &definition.env_file {
            let entries = dotenvy::from_path_iter(inside(root, file)?)
                .map_err(|_| failure("Cannot read service env file."))?;
            for entry in entries {
                let (key, value) = entry.map_err(|_| failure("Invalid service env file."))?;
                environment.insert(key, Value::String(value));
            }
        }
        let mut value = vpn_worker_config(
            root,
            relative,
            &image,
            command,
            owner,
            &lease.gateway_id,
            &resolver,
            environment,
        );
        // Compose interpolation must never expand host secrets or alter literal service commands.
        escape_compose_values(&mut value);
        secure_write(
            &launch.file,
            &serde_json::to_vec(&value).map_err(|_| failure("Cannot save VPN worker snapshot."))?,
        )?;
        if launch.containers().await?.iter().any(|c| {
            c["State"]["Running"] == true
                || c["State"]["Restarting"] == true
                || c["State"]["Paused"] == true
        }) {
            return Err(failure(
                "Owned service containers are already active. Stop them before starting a VPN run.",
            ));
        }
        launch.expected = vec!["worker".into()];
        Ok(launch)
    }
    fn docker(&self) -> Command {
        let mut cmd = self.session.command();
        cmd.current_dir(&self.cwd);
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
        self.session.verify().await?;
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
        for path in &self.private_files {
            let _ = std::fs::remove_file(path);
        }
        if self.private_snapshot {
            let _ = std::fs::remove_file(&self.file);
        }
    }
}

fn vpn_worker_config(
    root: &Path,
    relative: &Path,
    image: &str,
    command: &str,
    owner: &str,
    gateway: &str,
    resolver: &Path,
    environment: serde_json::Map<String, Value>,
) -> Value {
    serde_json::json!({"services":{"worker":{
        "image":image,"pull_policy":"never","restart":"no",
        "entrypoint":["/bin/sh","-lc"],"command":[command],
        "working_dir":Path::new("/workspace").join(relative),
        "network_mode":format!("container:{gateway}"),
        "cap_drop":["ALL"],"security_opt":["no-new-privileges:true"],
        "labels":{"dev.service-manager.owner":owner},
        "environment":environment,
        "volumes":[{"type":"bind","source":root,"target":"/workspace"},
            {"type":"bind","source":resolver,"target":"/etc/resolv.conf","read_only":true}]
    }}})
}
fn escape_compose_values(value: &mut Value) {
    match value {
        Value::String(s) => *s = s.replace('$', "$$"),
        Value::Array(values) => values.iter_mut().for_each(escape_compose_values),
        Value::Object(values) => values.values_mut().for_each(escape_compose_values),
        _ => {}
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
    fn vpn_worker_shares_only_verified_namespace_and_literal_command() {
        let mut env = serde_json::Map::new();
        env.insert("SECRET".into(), "$PRIVATE".into());
        let mut config = vpn_worker_config(
            Path::new("/project"),
            Path::new("api"),
            "sha256:test",
            "echo $SECRET",
            "owner",
            "immutable-id",
            Path::new("/private/resolver"),
            env,
        );
        escape_compose_values(&mut config);
        let worker = &config["services"]["worker"];
        assert_eq!(worker["network_mode"], "container:immutable-id");
        assert_eq!(worker["command"][0], "echo $$SECRET");
        assert_eq!(worker["environment"]["SECRET"], "$$PRIVATE");
        assert_eq!(worker["cap_drop"], serde_json::json!(["ALL"]));
        assert_eq!(worker["volumes"][1]["read_only"], true);
        assert!(worker.get("ports").is_none());
        assert!(worker.get("devices").is_none());
        assert_eq!(worker["working_dir"], "/workspace/api");
    }
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
