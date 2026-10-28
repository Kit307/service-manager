use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::Path;

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct AppError {
    pub code: String,
    pub message: String,
}
impl AppError {
    pub fn new(code: &str, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
        }
    }
}
impl std::fmt::Display for AppError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}
impl std::error::Error for AppError {}
impl From<std::io::Error> for AppError {
    fn from(_: std::io::Error) -> Self {
        Self::new(
            "IO_ERROR",
            "Could not access the local file or process. Check its permissions and availability.",
        )
    }
}
pub type Result<T> = std::result::Result<T, AppError>;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct HealthCheck {
    pub url: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum ServiceKind {
    Command { command: String },
    Compose { file: String },
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct VpnService {
    pub profile_id: String,
    pub image: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct VpnRoute {
    pub profile_id: String,
    pub remote_host: String,
    pub remote_port: u16,
    pub local_port: u16,
}

fn validate_vpn_routes(kind: &ServiceKind, routes: &[VpnRoute]) -> Result<()> {
    if routes.is_empty() {
        return Ok(());
    }
    if !matches!(kind, ServiceKind::Command { .. }) || routes.len() > 32 {
        return Err(AppError::new(
            "CONFIG_INVALID",
            "Use up to 32 VPN routes on command services only.",
        ));
    }
    let mut local_ports = HashSet::new();
    for route in routes {
        let host = route.remote_host.as_str();
        if uuid::Uuid::parse_str(&route.profile_id).is_err()
            || route.remote_port == 0
            || route.local_port == 0
            || !local_ports.insert(route.local_port)
            || host.is_empty()
            || host.len() > 253
            || host.starts_with(['.', '-'])
            || host.ends_with(['.', '-'])
            || !host
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'_'))
        {
            return Err(AppError::new(
                "CONFIG_INVALID",
                "VPN routes require a valid profile, hostname, nonzero ports, and distinct local ports.",
            ));
        }
    }
    Ok(())
}
pub fn validate_vpn_image(image: &str) -> Result<()> {
    if image.is_empty()
        || image.len() > 256
        || !image.as_bytes()[0].is_ascii_alphanumeric()
        || !image
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || b"./:_-@".contains(&c))
    {
        return Err(AppError::new(
            "CONFIG_INVALID",
            "Use a valid locally available Docker image name without spaces or options.",
        ));
    }
    Ok(())
}
fn validate_vpn(kind: &ServiceKind, vpn: &Option<VpnService>) -> Result<()> {
    if let Some(vpn) = vpn {
        if !matches!(kind, ServiceKind::Command { .. })
            || uuid::Uuid::parse_str(&vpn.profile_id).is_err()
        {
            return Err(AppError::new(
                "CONFIG_INVALID",
                "VPN profiles are supported only for command services with a valid profile ID.",
            ));
        }
        validate_vpn_image(&vpn.image)?;
    }
    Ok(())
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", try_from = "ServiceWire")]
pub struct ServiceDefinition {
    pub id: String,
    pub name: String,
    pub cwd: String,
    #[serde(flatten)]
    pub kind: ServiceKind,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub env_file: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub health_check: Option<HealthCheck>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub ports: Vec<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub depends_on: Vec<crate::project::ServiceRef>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vpn: Option<VpnService>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub vpn_routes: Vec<VpnRoute>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ServiceWire {
    id: String,
    name: String,
    cwd: String,
    #[serde(rename = "type")]
    kind: String,
    command: Option<String>,
    file: Option<String>,
    env_file: Option<String>,
    health_check: Option<HealthCheck>,
    #[serde(default)]
    ports: Vec<u16>,
    url: Option<String>,
    #[serde(default)]
    depends_on: Vec<crate::project::ServiceRef>,
    vpn: Option<VpnService>,
    #[serde(default)]
    vpn_routes: Vec<VpnRoute>,
}
impl TryFrom<ServiceWire> for ServiceDefinition {
    type Error = String;
    fn try_from(w: ServiceWire) -> std::result::Result<Self, String> {
        let kind = match (w.kind.as_str(), w.command, w.file) {
            ("command", Some(command), None) => ServiceKind::Command { command },
            ("compose", None, Some(file)) => ServiceKind::Compose { file },
            _ => return Err("Choose command with command text, or compose with a file; unknown and mixed fields are not supported.".into()),
        };
        validate_vpn(&kind, &w.vpn).map_err(|e| e.message)?;
        validate_vpn_routes(&kind, &w.vpn_routes).map_err(|e| e.message)?;
        if w.vpn.is_some() && !w.vpn_routes.is_empty() {
            return Err("Choose either the legacy VPN worker or native VPN routes, not both.".into());
        }
        Ok(Self {
            id: w.id,
            name: w.name,
            cwd: w.cwd,
            kind,
            env_file: w.env_file,
            health_check: w.health_check,
            ports: w.ports,
            url: w.url,
            depends_on: w.depends_on,
            vpn: w.vpn,
            vpn_routes: w.vpn_routes,
        })
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectConfig {
    pub schema_version: u32,
    pub services: Vec<ServiceDefinition>,
}
impl Default for ProjectConfig {
    fn default() -> Self {
        Self {
            schema_version: 1,
            services: vec![],
        }
    }
}

pub fn inside(root: &Path, relative: &str) -> Result<std::path::PathBuf> {
    let path = Path::new(relative);
    if path.is_absolute()
        || path
            .components()
            .any(|c| matches!(c, std::path::Component::ParentDir))
    {
        return Err(AppError::new(
            "CONFIG_INVALID",
            "Paths must stay inside the project folder.",
        ));
    }
    let canonical = root.join(path).canonicalize().map_err(|_| {
        AppError::new(
            "CONFIG_INVALID",
            "A configured folder or file does not exist.",
        )
    })?;
    if !canonical.starts_with(root) {
        return Err(AppError::new(
            "CONFIG_INVALID",
            "A symbolic link points outside the project.",
        ));
    }
    Ok(canonical)
}
pub fn validate(root: &Path, config: &ProjectConfig) -> Result<()> {
    if config.schema_version != 1 {
        return Err(AppError::new(
            "CONFIG_INVALID",
            "This configuration version is not supported. The file was preserved.",
        ));
    }
    let mut ids = std::collections::HashSet::new();
    for service in &config.services {
        validate_vpn(&service.kind, &service.vpn)?;
        if service.id.is_empty()
            || service.id.len() > 80
            || !service
                .id
                .bytes()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'-' || c == b'_')
            || service.name.trim().is_empty()
            || service.name.len() > 120
        {
            return Err(AppError::new("CONFIG_INVALID", "Use a name and a short lowercase service ID with letters, numbers, hyphens or underscores."));
        }
        if !ids.insert(&service.id) {
            return Err(AppError::new(
                "CONFIG_INVALID",
                "Service IDs must be unique within the project.",
            ));
        }
        if !inside(root, &service.cwd)?.is_dir() {
            return Err(AppError::new(
                "CONFIG_INVALID",
                "The working directory must be a folder.",
            ));
        }
        match &service.kind {
            ServiceKind::Command { command }
                if command.trim().is_empty() || command.len() > 32768 || command.contains('\0') =>
            {
                return Err(AppError::new(
                    "CONFIG_INVALID",
                    "Enter a nonempty run command.",
                ))
            }
            ServiceKind::Compose { file } if !inside(root, file)?.is_file() => {
                return Err(AppError::new(
                    "CONFIG_INVALID",
                    "The Compose file must be a file.",
                ));
            }
            _ => {}
        }
        if let Some(file) = &service.env_file {
            if !inside(root, file)?.is_file() {
                return Err(AppError::new(
                    "CONFIG_INVALID",
                    "The environment file must be an existing file.",
                ));
            }
        }
        if service.ports.len() > 16
            || service.ports.contains(&0)
            || service
                .ports
                .iter()
                .collect::<std::collections::HashSet<_>>()
                .len()
                != service.ports.len()
            || service.depends_on.len() > 32
            || service.depends_on.iter().any(|d| {
                d.project_id.is_empty()
                    || d.service_id.is_empty()
                    || d.project_id.len() > 128
                    || d.service_id.len() > 80
            })
        {
            return Err(AppError::new(
                "CONFIG_INVALID",
                "Use up to 16 distinct TCP ports (1–65535) and 32 valid dependencies.",
            ));
        }
        for address in service
            .health_check
            .as_ref()
            .map(|h| h.url.as_str())
            .into_iter()
            .chain(service.url.as_deref())
        {
            let url = reqwest::Url::parse(address).map_err(|_| {
                AppError::new("CONFIG_INVALID", "Enter a valid loopback health-check URL.")
            })?;
            if !matches!(url.scheme(), "http" | "https")
                || !matches!(url.host_str(), Some("127.0.0.1" | "[::1]" | "localhost"))
                || !url.username().is_empty()
                || url.password().is_some()
            {
                return Err(AppError::new(
                    "CONFIG_INVALID",
                    "Health checks must use loopback HTTP/HTTPS without credentials.",
                ));
            }
        }
    }
    Ok(())
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NetworkTraffic {
    pub project_id: String,
    pub service_id: String,
    pub run_id: String,
    pub profile_id: String,
    pub local_port: u16,
    pub remote_host: String,
    pub remote_port: u16,
    pub tx_bytes: u64,
    pub rx_bytes: u64,
    pub active_connections: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    pub state: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub run_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pid: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exit_code: Option<i32>,
    pub health: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    pub activity: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub started_at: Option<String>,
}
impl Default for Snapshot {
    fn default() -> Self {
        Self {
            state: "stopped".into(),
            run_id: None,
            pid: None,
            exit_code: None,
            health: "notConfigured".into(),
            error: None,
            activity: "Ready when you are".into(),
            started_at: None,
        }
    }
}
impl Snapshot {
    pub fn active(&self) -> bool {
        matches!(
            self.state.as_str(),
            "starting" | "running" | "ready" | "stopping"
        )
    }
}
