use crate::model::*;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::HashMap,
    fs::{self, OpenOptions},
    io::{Read, Write},
    os::unix::fs::{OpenOptionsExt, PermissionsExt},
    path::{Path, PathBuf},
};

pub fn hash(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
pub fn fingerprint(service: &ServiceDefinition) -> String {
    hash(&serde_json::to_vec(service).unwrap_or_default())
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Project {
    pub id: String,
    pub name: String,
    pub path: String,
    pub revision: String,
    pub config: ProjectConfig,
    pub trusted: Vec<String>,
    pub runtime: HashMap<String, Snapshot>,
}
#[derive(Clone, Default, Serialize, Deserialize)]
pub struct PrivateState {
    pub projects: HashMap<String, String>,
    pub trust: HashMap<String, HashMap<String, String>>,
    pub dirty: bool,
    #[serde(default)]
    pub compose_owners: HashMap<String, String>,
    #[serde(default)]
    pub groups: Vec<WorkspaceGroup>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceGroup {
    pub id: String,
    pub name: String,
    pub services: Vec<ServiceRef>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServiceRef {
    pub project_id: String,
    pub service_id: String,
}
pub struct ProjectStore {
    pub state_dir: PathBuf,
    pub private: PrivateState,
    pub projects: HashMap<String, Project>,
    pub unclean: bool,
}

pub fn secure_write(path: &Path, bytes: &[u8]) -> Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| AppError::new("IO_ERROR", "Missing parent directory."))?;
    if fs::symlink_metadata(parent).is_ok_and(|m| m.file_type().is_symlink())
        || fs::symlink_metadata(path).is_ok_and(|m| m.file_type().is_symlink())
    {
        return Err(AppError::new(
            "CONFIG_INVALID",
            "Refusing to write through a symbolic link.",
        ));
    }
    fs::create_dir_all(parent)?;
    let temp = parent.join(format!(".service-manager-{}.tmp", uuid::Uuid::new_v4()));
    let result = (|| {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&temp)?;
        file.write_all(bytes)?;
        file.sync_all()?;
        fs::rename(&temp, path)?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(temp);
    }
    result
}
pub fn read_config(root: &Path) -> Result<(ProjectConfig, String)> {
    let dir = root.join(".service-manager");
    let path = dir.join("project.json");
    if fs::symlink_metadata(&dir).is_ok_and(|m| m.file_type().is_symlink())
        || fs::symlink_metadata(&path).is_ok_and(|m| m.file_type().is_symlink())
    {
        return Err(AppError::new(
            "CONFIG_INVALID",
            "Configuration cannot be a symbolic link.",
        ));
    }
    if !path.exists() {
        return Ok((ProjectConfig::default(), "absent".into()));
    }
    let mut bytes = Vec::new();
    fs::File::open(path)?
        .take(1_048_577)
        .read_to_end(&mut bytes)?;
    if bytes.len() > 1_048_576 {
        return Err(AppError::new(
            "CONFIG_INVALID",
            "Configuration exceeds 1 MiB.",
        ));
    }
    let value: serde_json::Value = serde_json::from_slice(&bytes).map_err(|_| {
        AppError::new(
            "CONFIG_INVALID",
            "project.json is not valid JSON. Its contents were preserved.",
        )
    })?;
    if let Some(services) = value.get("services").and_then(|s| s.as_array()) {
        for service in services {
            if service.as_object().is_some_and(|o| {
                o.keys().any(|k| {
                    ![
                        "id",
                        "name",
                        "cwd",
                        "type",
                        "command",
                        "file",
                        "envFile",
                        "healthCheck",
                        "ports",
                        "url",
                        "dependsOn",
                    ]
                    .contains(&k.as_str())
                })
            }) {
                return Err(AppError::new(
                    "CONFIG_INVALID",
                    "A service contains unsupported configuration fields.",
                ));
            }
        }
    }
    let config: ProjectConfig = serde_json::from_value(value).map_err(|_| {
        AppError::new(
            "CONFIG_INVALID",
            "Configuration fields are invalid. The file was preserved.",
        )
    })?;
    validate(root, &config)?;
    Ok((config, hash(&bytes)))
}
impl ProjectStore {
    pub fn new(state_dir: PathBuf) -> Result<Self> {
        fs::create_dir_all(&state_dir)?;
        fs::set_permissions(&state_dir, fs::Permissions::from_mode(0o700))?;
        let state_file = state_dir.join("state.json");
        let private: PrivateState = if state_file.exists() {
            serde_json::from_slice(&fs::read(&state_file)?).map_err(|_| {
                AppError::new(
                    "CONFIG_INVALID",
                    "The private application state is invalid; it was preserved.",
                )
            })?
        } else {
            PrivateState::default()
        };
        let unclean = private.dirty;
        let mut store = Self {
            state_dir,
            private,
            projects: HashMap::new(),
            unclean,
        };
        let recent: Vec<_> = store.private.projects.values().cloned().collect();
        for root in recent {
            let _ = store.open(&root);
        }
        store.persist()?;
        Ok(store)
    }
    pub fn persist(&self) -> Result<()> {
        self.write_private(&self.private)
    }
    fn write_private(&self, private: &PrivateState) -> Result<()> {
        secure_write(
            &self.state_dir.join("state.json"),
            &serde_json::to_vec_pretty(private).map_err(|_| {
                AppError::new("IO_ERROR", "Could not serialize application settings.")
            })?,
        )
    }
    pub fn save_group(
        &mut self,
        id: Option<&str>,
        name: &str,
        services: Vec<ServiceRef>,
    ) -> Result<WorkspaceGroup> {
        let name = name.trim();
        if name.is_empty()
            || name.chars().count() > 80
            || self
                .private
                .groups
                .iter()
                .any(|g| Some(g.id.as_str()) != id && g.name.to_lowercase() == name.to_lowercase())
        {
            return Err(AppError::new(
                "CONFIG_INVALID",
                "Choose a unique group name between 1 and 80 characters.",
            ));
        }
        if id.is_some_and(|id| !self.private.groups.iter().any(|g| g.id == id)) {
            return Err(AppError::new(
                "GROUP_NOT_FOUND",
                "This group no longer exists.",
            ));
        }
        if !services.iter().all(|s| {
            self.projects
                .get(&s.project_id)
                .is_some_and(|p| p.config.services.iter().any(|d| d.id == s.service_id))
        }) {
            return Err(AppError::new(
                "SERVICE_NOT_FOUND",
                "Only registered services can be added to a group.",
            ));
        }
        let mut members = Vec::new();
        for service in services {
            if !members.contains(&service) {
                members.push(service);
            }
        }
        let group = WorkspaceGroup {
            id: id
                .map(str::to_owned)
                .unwrap_or_else(|| uuid::Uuid::new_v4().to_string()),
            name: name.into(),
            services: members,
        };
        let mut next = self.private.clone();
        for other in &mut next.groups {
            other.services.retain(|s| !group.services.contains(s));
        }
        if let Some(existing) = next.groups.iter_mut().find(|g| g.id == group.id) {
            *existing = group.clone();
        } else {
            next.groups.push(group.clone());
        }
        self.write_private(&next)?;
        self.private = next;
        Ok(group)
    }
    pub fn move_service(&mut self, service: ServiceRef, group_id: Option<&str>) -> Result<()> {
        if !self
            .projects
            .get(&service.project_id)
            .is_some_and(|p| p.config.services.iter().any(|s| s.id == service.service_id))
        {
            return Err(AppError::new(
                "SERVICE_NOT_FOUND",
                "Only registered services can be moved.",
            ));
        }
        if let Some(id) = group_id {
            let mut group = self
                .private
                .groups
                .iter()
                .find(|g| g.id == id)
                .cloned()
                .ok_or_else(|| AppError::new("GROUP_NOT_FOUND", "This group no longer exists."))?;
            if group.services.contains(&service) {
                return Ok(());
            }
            group.services.push(service);
            self.save_group(Some(id), &group.name, group.services)?;
        } else if self
            .private
            .groups
            .iter()
            .any(|g| g.services.contains(&service))
        {
            self.remove_service_from_groups(&service.project_id, &service.service_id)?;
        }
        Ok(())
    }
    pub fn remove_service_from_groups(&mut self, project: &str, service: &str) -> Result<()> {
        let mut next = self.private.clone();
        for group in &mut next.groups {
            group
                .services
                .retain(|s| s.project_id != project || s.service_id != service);
        }
        self.write_private(&next)?;
        self.private = next;
        Ok(())
    }
    pub fn remove_group(&mut self, id: &str) -> Result<()> {
        if !self.private.groups.iter().any(|g| g.id == id) {
            return Err(AppError::new(
                "GROUP_NOT_FOUND",
                "This group no longer exists.",
            ));
        }
        let mut next = self.private.clone();
        next.groups.retain(|g| g.id != id);
        self.write_private(&next)?;
        self.private = next;
        Ok(())
    }
    pub fn open(&mut self, path: &str) -> Result<Project> {
        let root = Path::new(path).canonicalize()?;
        self.open_root(root)
    }
    pub fn open_within(
        &mut self,
        allowed: &Path,
        relative: &Path,
        create: bool,
    ) -> Result<Project> {
        use std::path::Component;
        if !allowed.is_absolute() || allowed.canonicalize()? != allowed || !allowed.is_dir() {
            return Err(AppError::new(
                "PERMISSION_DENIED",
                "The allowed folder changed. Choose it again locally.",
            ));
        }
        if relative.as_os_str().is_empty()
            || relative.components().any(|c| {
                matches!(
                    c,
                    Component::ParentDir | Component::RootDir | Component::Prefix(_)
                )
            })
        {
            return Err(AppError::new(
                "CONFIG_INVALID",
                "Use a relative project path inside the allowed folder.",
            ));
        }
        let target = allowed.join(relative);
        if create {
            if relative.components().count() != 1
                || !matches!(relative.components().next(), Some(Component::Normal(_)))
            {
                return Err(AppError::new(
                    "CONFIG_INVALID",
                    "A new project needs a single folder name.",
                ));
            }
            std::fs::create_dir(&target).map_err(|_| AppError::new("CONFIG_INVALID", "Could not create the folder. It may already exist; existing files were preserved."))?;
        }
        let root = target.canonicalize()?;
        if !root.starts_with(allowed) {
            return Err(AppError::new(
                "PERMISSION_DENIED",
                "The project resolves outside the allowed folder.",
            ));
        }
        self.open_root(root)
    }
    fn open_root(&mut self, root: PathBuf) -> Result<Project> {
        if !root.is_dir() {
            return Err(AppError::new(
                "CONFIG_INVALID",
                "Open a folder, not a file.",
            ));
        }
        let path = root.to_string_lossy().into_owned();
        let id = self
            .private
            .projects
            .iter()
            .find(|(_, p)| **p == path)
            .map(|(id, _)| id.clone())
            .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
        let (config, revision) = read_config(&root)?;
        if self.projects.get(&id).is_some_and(|p| {
            p.runtime
                .iter()
                .any(|(sid, s)| s.active() && !config.services.iter().any(|d| d.id == *sid))
        }) {
            return Err(AppError::new("CONFIG_CONFLICT", "An active service was removed externally. Stop the service before reloading this project's configuration."));
        }
        let trust = self.private.trust.get(&id);
        let trusted = config
            .services
            .iter()
            .filter(|s| {
                trust
                    .and_then(|t| t.get(&s.id))
                    .is_some_and(|h| *h == fingerprint(s))
            })
            .map(|s| s.id.clone())
            .collect();
        let runtime = self
            .projects
            .get(&id)
            .map(|p| p.runtime.clone())
            .unwrap_or_default();
        let project = Project {
            id: id.clone(),
            name: root
                .file_name()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_else(|| "Project".into()),
            path: path.clone(),
            config,
            revision,
            trusted,
            runtime,
        };
        let is_new = !self.private.projects.contains_key(&id);
        self.private.projects.insert(id.clone(), path);
        self.projects.insert(id, project.clone());
        if is_new {
            self.persist()?;
        }
        Ok(project)
    }
    pub fn refresh(&mut self, id: &str) -> Result<Project> {
        let path = self
            .projects
            .get(id)
            .ok_or_else(|| {
                AppError::new("PROJECT_NOT_AUTHORIZED", "Open this project locally first.")
            })?
            .path
            .clone();
        self.open(&path)
    }
    pub fn remove_project(&mut self, id: &str) -> Result<()> {
        let project = self
            .projects
            .get(id)
            .ok_or_else(|| AppError::new("PROJECT_NOT_AUTHORIZED", "Project is not open."))?;
        if project.runtime.values().any(Snapshot::active) {
            return Err(AppError::new(
                "OPERATION_IN_PROGRESS",
                "Stop all services successfully before removing this project.",
            ));
        }
        let previous = self.private.clone();
        self.private.projects.remove(id);
        self.private.trust.remove(id);
        for group in &mut self.private.groups {
            group.services.retain(|service| service.project_id != id);
        }
        let prefix = format!("{id}/");
        self.private
            .compose_owners
            .retain(|key, _| !key.starts_with(&prefix));
        if let Err(error) = self.persist() {
            self.private = previous;
            return Err(error);
        }
        self.projects.remove(id);
        Ok(())
    }
    pub fn save(&mut self, id: &str, config: ProjectConfig, revision: &str) -> Result<Project> {
        let project = self.refresh(id)?;
        if project.revision != revision {
            return Err(AppError::new("CONFIG_CONFLICT", "Configuration changed externally. Reload the project before saving; your draft was not written."));
        }
        let root = Path::new(&project.path);
        validate(root, &config)?;
        let bytes = serde_json::to_vec_pretty(&config)
            .map_err(|_| AppError::new("CONFIG_INVALID", "Invalid configuration."))?;
        if bytes.len() > 1_048_576 {
            return Err(AppError::new(
                "CONFIG_INVALID",
                "Configuration exceeds 1 MiB.",
            ));
        }
        secure_write(&root.join(".service-manager/project.json"), &bytes)?;
        self.refresh(id)
    }
    pub fn trust(&mut self, id: &str, service: &str, expected_revision: &str) -> Result<()> {
        let project = self.refresh(id)?;
        if project.revision != expected_revision {
            return Err(AppError::new("CONFIG_CONFLICT", "Configuration changed while you reviewed it. Review the updated command before trusting it."));
        }
        let definition = project
            .config
            .services
            .iter()
            .find(|s| s.id == service)
            .ok_or_else(|| AppError::new("SERVICE_NOT_FOUND", "Service no longer exists."))?;
        self.private
            .trust
            .entry(id.into())
            .or_default()
            .insert(service.into(), fingerprint(definition));
        self.persist()?;
        self.refresh(id)?;
        Ok(())
    }
}
