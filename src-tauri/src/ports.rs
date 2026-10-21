use crate::{
    model::*,
    project::{hash, ServiceRef},
};
use serde::Serialize;
use std::{collections::BTreeSet, process::Stdio, time::Duration};
use tokio::process::Command;

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PortListener {
    pub port: u16,
    pub pid: u32,
    pub name: String,
    pub identity: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub managed: Option<ServiceRef>,
    #[serde(skip)]
    pub managed_run: Option<String>,
    #[serde(skip)]
    pub managed_pid: Option<u32>,
    #[serde(skip)]
    pub uid: u32,
    #[serde(skip)]
    pub pgid: u32,
}
pub fn configured_ports(definition: &ServiceDefinition) -> Vec<u16> {
    let mut ports: BTreeSet<_> = definition.ports.iter().copied().collect();
    for address in definition
        .url
        .iter()
        .chain(definition.health_check.iter().map(|h| &h.url))
    {
        if let Ok(url) = reqwest::Url::parse(address) {
            if let Some(port) = url.port_or_known_default() {
                ports.insert(port);
            }
        }
    }
    ports.into_iter().collect()
}
async fn output(program: &str, args: &[String]) -> Result<std::process::Output> {
    let mut command = Command::new(program);
    command.args(args).stdin(Stdio::null()).kill_on_drop(true);
    tokio::time::timeout(Duration::from_secs(3), command.output())
        .await
        .map_err(|_| AppError::new("PORT_CHECK_FAILED", "Local process inspection timed out."))?
        .map_err(|_| {
            AppError::new(
                "PORT_CHECK_FAILED",
                "Local process inspection is unavailable.",
            )
        })
}
pub async fn process_identity(pid: u32) -> Result<(String, u32, u32)> {
    let data = output(
        "/bin/ps",
        &[
            "-p".into(),
            pid.to_string(),
            "-o".into(),
            "uid=,lstart=,pgid=,comm=".into(),
        ],
    )
    .await?;
    let text = String::from_utf8_lossy(&data.stdout);
    let fields: Vec<_> = text.split_whitespace().collect();
    if !data.status.success() || fields.len() < 8 {
        return Err(AppError::new(
            "PROCESS_CHANGED",
            "The process exited or its identity cannot be verified.",
        ));
    }
    let uid = fields[0]
        .parse()
        .map_err(|_| AppError::new("PROCESS_CHANGED", "Cannot verify process owner."))?;
    let pgid = fields[6]
        .parse()
        .map_err(|_| AppError::new("PROCESS_CHANGED", "Cannot verify process group."))?;
    Ok((hash(format!("{pid}:{}", text.trim()).as_bytes()), uid, pgid))
}
pub async fn inspect(ports: &[u16]) -> Result<Vec<PortListener>> {
    if ports.is_empty() {
        return Ok(vec![]);
    }
    let data = output(
        "/usr/sbin/lsof",
        &[
            "-nP".into(),
            "-iTCP".into(),
            "-sTCP:LISTEN".into(),
            "-Fpcn".into(),
        ],
    )
    .await?;
    if (!data.status.success() && data.status.code() != Some(1)) || data.stdout.len() > 512 * 1024 {
        return Err(AppError::new(
            "PORT_CHECK_FAILED",
            "Could not inspect local TCP listeners.",
        ));
    }
    let mut pid = 0;
    let mut name = String::new();
    let mut rows = vec![];
    let mut seen = BTreeSet::new();
    for line in String::from_utf8_lossy(&data.stdout).lines() {
        match line.as_bytes().first() {
            Some(b'p') => pid = line[1..].parse().unwrap_or(0),
            Some(b'c') => name = line[1..].chars().take(120).collect(),
            Some(b'n') => {
                let port = line.rsplit(':').next().and_then(|s| s.parse::<u16>().ok());
                if let Some(port) = port.filter(|p| ports.contains(p)) {
                    if pid > 0 && seen.insert((port, pid)) {
                        if let Ok((identity, uid, pgid)) = process_identity(pid).await {
                            rows.push(PortListener {
                                port,
                                pid,
                                name: name.clone(),
                                identity,
                                managed: None,
                                managed_run: None,
                                managed_pid: None,
                                uid,
                                pgid,
                            });
                        }
                    }
                }
            }
            _ => {}
        }
    }
    // Inspection is advisory; the service still performs the final bind after preflight.
    for port in ports {
        if rows.iter().any(|l| l.port == *port) {
            continue;
        }
        match std::net::TcpListener::bind((std::net::Ipv4Addr::UNSPECIFIED, *port)) {
            Ok(listener) => drop(listener),
            Err(error) if error.kind() == std::io::ErrorKind::AddrInUse => {
                rows.push(PortListener {
                    port: *port,
                    pid: 0,
                    name: "Unknown listener".into(),
                    identity: String::new(),
                    managed: None,
                    managed_run: None,
                    managed_pid: None,
                    uid: 0,
                    pgid: 0,
                })
            }
            Err(_) => {
                return Err(AppError::new(
                    "PORT_CHECK_FAILED",
                    "Could not verify a configured port. Check local permissions.",
                ))
            }
        }
    }
    Ok(rows)
}
pub async fn protected(pid: u32) -> Result<bool> {
    let mut ancestor = std::process::id();
    for _ in 0..128 {
        if pid == ancestor || pid <= 1 {
            return Ok(true);
        }
        let data = output(
            "/bin/ps",
            &[
                "-p".into(),
                ancestor.to_string(),
                "-o".into(),
                "ppid=".into(),
            ],
        )
        .await?;
        if !data.status.success() {
            return Err(AppError::new(
                "PROCESS_CHANGED",
                "Could not verify the process ancestry.",
            ));
        }
        ancestor = String::from_utf8_lossy(&data.stdout)
            .trim()
            .parse()
            .map_err(|_| {
                AppError::new("PROCESS_CHANGED", "Could not verify the process ancestry.")
            })?;
        if ancestor <= 1 {
            return Ok(false);
        }
    }
    Err(AppError::new(
        "PROCESS_CHANGED",
        "Could not verify the process ancestry.",
    ))
}
