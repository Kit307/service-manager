use crate::{
    logs::{LogPage, LogStore},
    model::*,
    project::{Project, ProjectStore},
};
use nix::{
    sys::signal::{killpg, Signal},
    unistd::Pid,
};
use std::{
    collections::HashMap,
    net::{IpAddr, Ipv4Addr, SocketAddr},
    path::{Path, PathBuf},
    process::Stdio,
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Arc,
    },
    time::Duration,
};
use tokio::sync::Mutex;
use tokio::{
    io::{AsyncRead, AsyncReadExt},
    process::Command,
};

fn uses_compose_runner(definition: &ServiceDefinition) -> bool {
    matches!(definition.kind, ServiceKind::Compose { .. }) || definition.vpn.is_some()
}

struct NativeVpnSession {
    _relays: crate::vpn_relay::VpnRelaySet,
    leases: Vec<crate::vpn::VpnLease>,
    #[cfg(test)]
    shutdown_barrier: Option<(
        tokio::sync::oneshot::Sender<()>,
        tokio::sync::oneshot::Receiver<()>,
    )>,
}

impl NativeVpnSession {
    async fn shutdown(self) {
        #[cfg(test)]
        if let Some((ready, release)) = self.shutdown_barrier {
            let _ = ready.send(());
            let _ = release.await;
        }
        self._relays.shutdown().await;
    }

    async fn safe(&self) -> bool {
        for lease in &self.leases {
            if !lease.worker_safe().await.unwrap_or(false) {
                return false;
            }
        }
        true
    }
}
type TrafficRegistry =
    Arc<std::sync::Mutex<HashMap<(String, String), (String, Vec<crate::vpn_relay::RelayTraffic>)>>>;

struct TrafficGuard {
    registry: TrafficRegistry,
    key: (String, String),
    run: String,
}

impl Drop for TrafficGuard {
    fn drop(&mut self) {
        let mut registry = self.registry.lock().unwrap_or_else(|e| e.into_inner());
        if registry
            .get(&self.key)
            .is_some_and(|(run, _)| run == &self.run)
        {
            registry.remove(&self.key);
        }
    }
}

#[derive(Clone)]
pub struct Manager {
    pub store: Arc<Mutex<ProjectStore>>,
    pub logs: Arc<Mutex<LogStore>>,
    pub vpns: crate::vpn::VpnManager,
    shutting_down: Arc<AtomicBool>,
    traffic: TrafficRegistry,
    #[cfg(test)]
    prepared_test_session: Arc<std::sync::Mutex<Option<Vec<crate::vpn_relay::RelayTarget>>>>,
    #[cfg(test)]
    preflight_test_barrier: Arc<
        std::sync::Mutex<
            Option<(
                tokio::sync::oneshot::Sender<()>,
                tokio::sync::oneshot::Receiver<()>,
            )>,
        >,
    >,
    #[cfg(test)]
    shutdown_test_barrier: Arc<
        std::sync::Mutex<
            Option<(
                tokio::sync::oneshot::Sender<()>,
                tokio::sync::oneshot::Receiver<()>,
            )>,
        >,
    >,
    pub(crate) history: Arc<Mutex<crate::history::HistoryStore>>,
    pub(crate) sequence_lock: Arc<Mutex<()>>,
    pub(crate) sequence_generation: Arc<AtomicU64>,
}
impl Manager {
    pub fn new(path: PathBuf) -> Result<Self> {
        let store = ProjectStore::new(path.clone())?;
        let history = crate::history::HistoryStore::open(&path)?;
        Ok(Self {
            vpns: crate::vpn::VpnManager::new(path)?,
            store: Arc::new(Mutex::new(store)),
            history: Arc::new(Mutex::new(history)),
            sequence_lock: Arc::new(Mutex::new(())),
            sequence_generation: Arc::new(AtomicU64::new(0)),
            logs: Arc::new(Mutex::new(LogStore::default())),
            shutting_down: Arc::new(AtomicBool::new(false)),
            traffic: Arc::new(std::sync::Mutex::new(HashMap::new())),
            #[cfg(test)]
            prepared_test_session: Arc::new(std::sync::Mutex::new(None)),
            #[cfg(test)]
            preflight_test_barrier: Arc::new(std::sync::Mutex::new(None)),
            #[cfg(test)]
            shutdown_test_barrier: Arc::new(std::sync::Mutex::new(None)),
        })
    }
    fn register_network_traffic(
        &self,
        project: &str,
        service: &str,
        run: &str,
        traffic: Vec<crate::vpn_relay::RelayTraffic>,
    ) -> TrafficGuard {
        let key = (project.to_owned(), service.to_owned());
        self.traffic
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .insert(key.clone(), (run.to_owned(), traffic));
        TrafficGuard {
            registry: self.traffic.clone(),
            key,
            run: run.to_owned(),
        }
    }

    pub fn network_traffic(&self) -> Vec<NetworkTraffic> {
        let entries = self
            .traffic
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone();
        let mut rows = Vec::new();
        for ((project_id, service_id), (run_id, traffic)) in entries {
            for relay in traffic {
                let snapshot = relay.snapshot();
                rows.push(NetworkTraffic {
                    project_id: project_id.clone(),
                    service_id: service_id.clone(),
                    run_id: run_id.clone(),
                    profile_id: snapshot.route.profile_id,
                    local_port: snapshot.route.local_port,
                    remote_host: snapshot.route.remote_host,
                    remote_port: snapshot.route.remote_port,
                    tx_bytes: snapshot.tx_bytes,
                    rx_bytes: snapshot.rx_bytes,
                    active_connections: snapshot.active_connections,
                });
            }
        }
        rows.sort_by(|a, b| {
            (
                &a.project_id,
                &a.service_id,
                a.local_port,
                &a.profile_id,
                &a.remote_host,
                a.remote_port,
            )
                .cmp(&(
                    &b.project_id,
                    &b.service_id,
                    b.local_port,
                    &b.profile_id,
                    &b.remote_host,
                    b.remote_port,
                ))
        });
        rows
    }

    pub async fn open(&self, path: &str) -> Result<Project> {
        self.store.lock().await.open(path)
    }
    pub async fn remove_project(&self, project: &str) -> Result<()> {
        self.store.lock().await.remove_project(project)
    }
    pub async fn projects(&self) -> Vec<Project> {
        let mut store = self.store.lock().await;
        let ids: Vec<_> = store.projects.keys().cloned().collect();
        for id in ids {
            let _ = store.refresh(&id);
        }
        let mut projects: Vec<_> = store.projects.values().cloned().collect();
        projects.sort_by(|a, b| a.name.cmp(&b.name));
        projects
    }
    pub async fn update_vpn_network(
        &self,
        id: &str,
        published_ports: Vec<crate::vpn::VpnPortMapping>,
        outbound_subnets: Vec<String>,
    ) -> Result<crate::vpn::VpnProfileView> {
        let owners = self
            .store
            .lock()
            .await
            .private
            .compose_owners
            .values()
            .cloned()
            .collect();
        self.vpns
            .update_network(id, published_ports, outbound_subnets, &owners)
            .await
    }
    pub async fn save(
        &self,
        project: &str,
        definition: ServiceDefinition,
        revision: &str,
        trust: bool,
    ) -> Result<Project> {
        if let Some(vpn) = &definition.vpn {
            self.vpns
                .ensure_service_ports(
                    &vpn.profile_id,
                    &crate::ports::configured_ports(&definition),
                )
                .await?;
        }
        for route in &definition.vpn_routes {
            if !self.vpns.exists(&route.profile_id).await {
                return Err(AppError::new(
                    "VPN_ERROR",
                    "A selected VPN profile no longer exists.",
                ));
            }
        }
        let mut store = self.store.lock().await;
        let mut p = store.refresh(project)?;
        if let Some(existing) = p.config.services.iter_mut().find(|s| s.id == definition.id) {
            *existing = definition.clone();
        } else {
            p.config.services.push(definition.clone());
        }
        let mut graph: Vec<_> = store.projects.values().cloned().collect();
        if let Some(current) = graph.iter_mut().find(|p| p.id == project) {
            current.config = p.config.clone();
        }
        crate::operations::validate_dependencies(
            &graph,
            crate::project::ServiceRef {
                project_id: project.into(),
                service_id: definition.id.clone(),
            },
        )?;
        let saved = store.save(project, p.config, revision)?;
        if trust {
            store.trust(project, &definition.id, &saved.revision)?;
            return store.refresh(project);
        }
        Ok(saved)
    }
    pub async fn remove(&self, project: &str, service: &str, revision: &str) -> Result<()> {
        let mut store = self.store.lock().await;
        let mut p = store.refresh(project)?;
        if p.runtime.get(service).is_some_and(|s| s.active()) {
            return Err(AppError::new(
                "OPERATION_IN_PROGRESS",
                "Stop the service successfully before removing its definition.",
            ));
        }
        p.config.services.retain(|s| s.id != service);
        store.save(project, p.config, revision)?;
        store.remove_service_from_groups(project, service)
    }
    pub async fn status(&self, project: &str, service: &str) -> Result<Snapshot> {
        let store = self.store.lock().await;
        let p = store
            .projects
            .get(project)
            .ok_or_else(|| AppError::new("PROJECT_NOT_AUTHORIZED", "Project is not open."))?;
        if !p.config.services.iter().any(|s| s.id == service) {
            return Err(AppError::new(
                "SERVICE_NOT_FOUND",
                "Service does not exist.",
            ));
        }
        Ok(p.runtime.get(service).cloned().unwrap_or_default())
    }
    pub async fn read_logs(
        &self,
        project: &str,
        service: &str,
        cursor: Option<&str>,
    ) -> Result<LogPage> {
        self.status(project, service).await?;
        Ok(self
            .logs
            .lock()
            .await
            .read(&format!("{project}/{service}"), cursor))
    }

    async fn prepare_native_vpn(
        &self,
        routes: &[VpnRoute],
        user: &crate::project::ServiceRef,
        run: &str,
    ) -> Result<NativeVpnSession> {
        #[cfg(test)]
        let targets = self.prepared_test_session.lock().unwrap().take();
        #[cfg(test)]
        if let Some(targets) = targets {
            let session = NativeVpnSession {
                _relays: crate::vpn_relay::VpnRelaySet::start(targets).await?,
                leases: vec![],
                shutdown_barrier: self.shutdown_test_barrier.lock().unwrap().take(),
            };
            let barrier = self.preflight_test_barrier.lock().unwrap().take();
            if let Some((ready, release)) = barrier {
                let _ = ready.send(());
                let _ = release.await;
            }
            return Ok(session);
        }
        let mut profiles = HashMap::new();
        let mut leases = Vec::new();
        for route in routes {
            if profiles.contains_key(&route.profile_id) {
                continue;
            }
            let lease = self.vpns.acquire(&route.profile_id, user, run).await?;
            if !lease.healthy().await? {
                return Err(AppError::new(
                    "VPN_ERROR",
                    "A selected VPN is not ready. Reconnect it before starting the service.",
                ));
            }
            profiles.insert(route.profile_id.clone(), lease.proxy_port);
            leases.push(lease);
        }
        let targets = routes
            .iter()
            .map(|route| crate::vpn_relay::RelayTarget {
                route: route.clone(),
                proxy: SocketAddr::new(
                    IpAddr::V4(Ipv4Addr::LOCALHOST),
                    profiles[&route.profile_id],
                ),
            })
            .collect();
        let relays = crate::vpn_relay::VpnRelaySet::start(targets).await?;
        Ok(NativeVpnSession {
            _relays: relays,
            leases,
            #[cfg(test)]
            shutdown_barrier: None,
        })
    }

    async fn finish_native_preflight(
        &self,
        project: &str,
        service: &str,
        terminal: Snapshot,
    ) -> Result<Snapshot> {
        // Keep the run active until history finalization finishes so shutdown cannot race it.
        self.finish_history(project, service, &terminal).await;
        let mut store = self.store.lock().await;
        let current = store
            .projects
            .get_mut(project)
            .and_then(|project| project.runtime.get_mut(service))
            .ok_or_else(|| AppError::new("SERVICE_NOT_FOUND", "Service does not exist."))?;
        if current.run_id != terminal.run_id || !current.active() {
            return Err(AppError::new(
                "OPERATION_IN_PROGRESS",
                "The managed run changed during VPN setup.",
            ));
        }
        current.state = terminal.state;
        current.activity = terminal.activity;
        Ok(current.clone())
    }

    pub async fn start(&self, project: &str, service: &str, actor: &str) -> Result<Snapshot> {
        let (caller, cancelled) = tokio::sync::oneshot::channel::<()>();
        let manager = self.clone();
        let project = project.to_owned();
        let service = service.to_owned();
        let actor = actor.to_owned();
        let task = tokio::spawn(async move {
            manager
                .start_owned(&project, &service, &actor, cancelled)
                .await
        });
        let result = task.await.map_err(|_| {
            AppError::new(
                "PROCESS_START_FAILED",
                "The service start task could not complete.",
            )
        });
        drop(caller);
        result?
    }

    async fn start_owned(
        &self,
        project: &str,
        service: &str,
        actor: &str,
        mut cancelled: tokio::sync::oneshot::Receiver<()>,
    ) -> Result<Snapshot> {
        let mut store = self.store.lock().await;
        if self.shutting_down.load(Ordering::SeqCst) {
            return Err(AppError::new(
                "OPERATION_IN_PROGRESS",
                "The manager is stopping services before quit.",
            ));
        }
        let p = store.refresh(project)?;
        let definition = p
            .config
            .services
            .iter()
            .find(|s| s.id == service)
            .cloned()
            .ok_or_else(|| AppError::new("SERVICE_NOT_FOUND", "Service does not exist."))?;
        if let Some(current) = p.runtime.get(service) {
            if current.active() {
                return Ok(current.clone());
            }
        }
        if !p.trusted.contains(&service.to_string()) {
            return Err(AppError::new(
                "TRUST_REQUIRED",
                "Review and trust this service locally before running it.",
            ));
        }
        for dependency in &definition.depends_on {
            if !store
                .projects
                .get(&dependency.project_id)
                .and_then(|p| p.runtime.get(&dependency.service_id))
                .is_some_and(|s| s.state == "ready")
            {
                return Err(AppError::new(
                    "DEPENDENCY_NOT_READY",
                    "A dependency is not Ready. Start its health-checked service first.",
                ));
            }
        }
        if let Some(vpn) = &definition.vpn {
            self.vpns
                .ensure_service_ports(
                    &vpn.profile_id,
                    &crate::ports::configured_ports(&definition),
                )
                .await?;
        } else {
            let conflicts =
                crate::ports::inspect(&crate::ports::configured_ports(&definition)).await?;
            if let Some(conflict) = conflicts.first() {
                return Err(AppError::new("PORT_IN_USE", format!("TCP port {} is used by {} (PID {}). Open Diagnostics to inspect or stop its owner.",conflict.port,conflict.name,conflict.pid)));
            }
        }
        let cwd = inside(Path::new(&p.path), &definition.cwd)?;
        if matches!(
            cancelled.try_recv(),
            Err(tokio::sync::oneshot::error::TryRecvError::Closed)
        ) {
            return Err(AppError::new(
                "OPERATION_CANCELLED",
                "Service start was cancelled.",
            ));
        }
        if uses_compose_runner(&definition) {
            let key = format!("{project}/{service}");
            let owner = store
                .private
                .compose_owners
                .entry(key)
                .or_insert_with(|| uuid::Uuid::new_v4().to_string())
                .clone();
            store.private.dirty = true;
            store.persist()?;
            let snapshot = Snapshot {
                state: "starting".into(),
                started_at: Some(chrono::Utc::now().to_rfc3339()),
                run_id: Some(uuid::Uuid::new_v4().to_string()),
                activity: format!("Docker preflight via {actor}"),
                ..Snapshot::default()
            };
            self.history.lock().await.begin(
                project,
                service,
                snapshot.run_id.as_deref().unwrap(),
                &match &definition.kind {
                    ServiceKind::Compose { file } => format!("docker compose -f {file} up"),
                    ServiceKind::Command { command } => command.clone(),
                },
            )?;
            store
                .projects
                .get_mut(project)
                .unwrap()
                .runtime
                .insert(service.into(), snapshot.clone());
            let state_dir = store.state_dir.clone();
            drop(store);
            let manager = self.clone();
            let project = project.to_string();
            let service = service.to_string();
            let run = snapshot.run_id.clone().unwrap();
            tokio::spawn(async move {
                manager
                    .run_compose(crate::compose::ComposeJob {
                        project,
                        service,
                        run,
                        root: p.path.into(),
                        cwd,
                        definition,
                        state_dir,
                        owner,
                    })
                    .await;
            });
            return Ok(snapshot);
        }
        let ServiceKind::Command { command } = &definition.kind else {
            unreachable!()
        };
        let mut cmd = Command::new("/bin/zsh");
        cmd.args(["-lc", command])
            .current_dir(cwd)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .process_group(0);
        if let Some(file) = &definition.env_file {
            let env = inside(Path::new(&p.path), file)?;
            let iter = dotenvy::from_path_iter(env).map_err(|_| {
                AppError::new(
                    "CONFIG_INVALID",
                    "Could not parse the environment file. Its contents were not logged.",
                )
            })?;
            for item in iter {
                let (key, value) = item.map_err(|_| {
                    AppError::new(
                        "CONFIG_INVALID",
                        "Invalid environment file syntax. Its contents were not logged.",
                    )
                })?;
                cmd.env(key, value);
            }
        }
        let run = uuid::Uuid::new_v4().to_string();
        let mut route_session = None;
        if !definition.vpn_routes.is_empty() {
            store.private.dirty = true;
            store.persist()?;
            self.history.lock().await.begin(
                project,
                service,
                &run,
                match &definition.kind {
                    ServiceKind::Command { command } => command,
                    ServiceKind::Compose { .. } => unreachable!(),
                },
            )?;
            let starting = Snapshot {
                state: "starting".into(),
                started_at: Some(chrono::Utc::now().to_rfc3339()),
                run_id: Some(run.clone()),
                activity: format!("VPN relay preflight via {actor}"),
                ..Snapshot::default()
            };
            store
                .projects
                .get_mut(project)
                .unwrap()
                .runtime
                .insert(service.into(), starting);
            drop(store);
            let user = crate::project::ServiceRef {
                project_id: project.into(),
                service_id: service.into(),
            };
            let prepared = self.prepare_native_vpn(&definition.vpn_routes, &user, &run);
            tokio::pin!(prepared);
            let (prepared, cancelled_preflight) = tokio::select! {
                biased;
                _ = &mut cancelled => {
                    self.stop(project, service).await?;
                    (prepared.await, true)
                }
                result = &mut prepared => (result, false),
            };
            store = self.store.lock().await;
            let current = store
                .projects
                .get_mut(project)
                .and_then(|project| project.runtime.get_mut(service))
                .ok_or_else(|| AppError::new("SERVICE_NOT_FOUND", "Service does not exist."))?;
            if current.run_id.as_deref() != Some(&run) {
                return Err(AppError::new(
                    "OPERATION_IN_PROGRESS",
                    "The managed run changed during VPN setup.",
                ));
            }
            if !cancelled_preflight
                && matches!(
                    cancelled.try_recv(),
                    Err(tokio::sync::oneshot::error::TryRecvError::Closed)
                )
            {
                current.state = "stopping".into();
            }
            route_session = match prepared {
                Ok(session) if current.state != "stopping" => Some(session),
                Ok(session) => {
                    drop(store);
                    session.shutdown().await;
                    let mut store = self.store.lock().await;
                    let current = store
                        .projects
                        .get_mut(project)
                        .and_then(|project| project.runtime.get_mut(service))
                        .ok_or_else(|| {
                            AppError::new("SERVICE_NOT_FOUND", "Service does not exist.")
                        })?;
                    if current.run_id.as_deref() != Some(&run) || current.state != "stopping" {
                        return Err(AppError::new(
                            "OPERATION_IN_PROGRESS",
                            "The managed run changed during VPN setup.",
                        ));
                    }
                    let mut terminal = current.clone();
                    terminal.state = "stopped".into();
                    terminal.activity = "Stopped during VPN setup".into();
                    drop(store);
                    return self
                        .finish_native_preflight(project, service, terminal)
                        .await;
                }
                Err(error) => {
                    current.error = Some(error.message.clone());
                    let mut terminal = current.clone();
                    terminal.state = "failed".into();
                    terminal.activity = "VPN relay setup failed".into();
                    drop(store);
                    self.finish_native_preflight(project, service, terminal)
                        .await?;
                    return Err(error);
                }
            };
        }
        if route_session.is_none() {
            store.private.dirty = true;
            store.persist()?;
            self.history
                .lock()
                .await
                .begin(project, service, &run, command)?;
        }
        let mut child = match cmd.spawn() {
            Ok(child) => child,
            Err(_) => {
                let error = AppError::new(
                    "PROCESS_START_FAILED",
                    "Could not start the command. Check its working folder and shell.",
                );
                if let Some(session) = route_session.take() {
                    drop(store);
                    session.shutdown().await;
                    let mut store = self.store.lock().await;
                    let current = store
                        .projects
                        .get_mut(project)
                        .and_then(|project| project.runtime.get_mut(service))
                        .ok_or_else(|| {
                            AppError::new("SERVICE_NOT_FOUND", "Service does not exist.")
                        })?;
                    if current.run_id.as_deref() != Some(&run) {
                        return Err(AppError::new(
                            "OPERATION_IN_PROGRESS",
                            "The managed run changed during VPN setup.",
                        ));
                    }
                    current.error = Some(error.message.clone());
                    let mut terminal = current.clone();
                    terminal.state = "failed".into();
                    terminal.activity = "Command could not start".into();
                    drop(store);
                    self.finish_native_preflight(project, service, terminal)
                        .await?;
                    return Err(error);
                }
                let snapshot = Snapshot {
                    state: "failed".into(),
                    run_id: Some(run),
                    error: Some(error.message.clone()),
                    activity: "Command could not start".into(),
                    ..Snapshot::default()
                };
                store
                    .projects
                    .get_mut(project)
                    .unwrap()
                    .runtime
                    .insert(service.into(), snapshot.clone());
                drop(store);
                self.finish_history(project, service, &snapshot).await;
                return Err(error);
            }
        };
        let pid = child.id().ok_or_else(|| {
            AppError::new(
                "PROCESS_START_FAILED",
                "The process exited before its identity was available.",
            )
        })?;
        let traffic_guard = route_session.as_ref().map(|session| {
            self.register_network_traffic(project, service, &run, session._relays.traffic())
        });
        let key = format!("{project}/{service}");
        let snapshot = Snapshot {
            state: if definition.health_check.is_some() {
                "starting"
            } else {
                "running"
            }
            .into(),
            started_at: Some(chrono::Utc::now().to_rfc3339()),
            run_id: Some(run.clone()),
            pid: Some(pid),
            exit_code: None,
            health: if definition.health_check.is_some() {
                "checking".into()
            } else {
                "notConfigured".into()
            },
            error: None,
            activity: format!(
                "Started via {actor} · {}",
                chrono::Local::now().format("%H:%M:%S")
            ),
        };
        store
            .projects
            .get_mut(project)
            .unwrap()
            .runtime
            .insert(service.into(), snapshot.clone());
        drop(store);
        let stdout = child.stdout.take();
        let stderr = child.stderr.take();
        let (stdout_ready, stdout_gate) = tokio::sync::oneshot::channel::<()>();
        let (stderr_ready, stderr_gate) = tokio::sync::oneshot::channel::<()>();
        let (drain_deadline, drain_deadline_rx) = tokio::sync::watch::channel(None);
        let has_health = definition.health_check.is_some();
        let manager = self.clone();
        let project_id = project.to_string();
        let service_id = service.to_string();
        let run_id = run.clone();
        let relay_count = definition.vpn_routes.len();
        let log_store = self.logs.clone();
        let log_key = key.clone();
        let log_run = run.clone();
        let started_log = tokio::spawn(async move {
            log_store.lock().await.push(
                &log_key,
                &log_run,
                "system",
                "Started service. Output is shared with the desktop and authorized AI clients.",
            );
            if relay_count != 0 {
                log_store.lock().await.push(
                    &log_key,
                    &log_run,
                    "system",
                    &format!(
                        "Opened {} loopback VPN destination relay(s). The service process remains native.",
                        relay_count
                    ),
                );
            }
            let _ = stdout_ready.send(());
            let _ = stderr_ready.send(());
        });
        tokio::spawn(async move {
            let stdout_deadline = drain_deadline_rx.clone();
            let stderr_deadline = drain_deadline_rx;
            let a = stdout.map(|pipe| {
                let logs = manager.logs.clone();
                let key = key.clone();
                let run = run_id.clone();
                tokio::spawn(async move {
                    let _ = stdout_gate.await;
                    drain(pipe, logs, key, run, "stdout", Some(stdout_deadline)).await
                })
            });
            let b = stderr.map(|pipe| {
                let logs = manager.logs.clone();
                let key = key.clone();
                let run = run_id.clone();
                tokio::spawn(async move {
                    let _ = stderr_gate.await;
                    drain(pipe, logs, key, run, "stderr", Some(stderr_deadline)).await
                })
            });
            let mut traffic_guard = traffic_guard;
            let mut route_session = route_session;
            let mut wait = Box::pin(child.wait());
            let mut vpn_failure = None;
            let result = if route_session.is_some() {
                loop {
                    tokio::select! {
                        result = &mut wait => break result,
                        _ = tokio::time::sleep(Duration::from_secs(1)) => {
                            if vpn_failure.is_none() && !route_session.as_ref().unwrap().safe().await {
                                drop(traffic_guard.take());
                                if let Some(session) = route_session.take() {
                                    session.shutdown().await;
                                }
                                let message = "VPN gateway stopped or ownership was lost. The native service and its loopback relays were stopped.".to_string();
                                vpn_failure = Some(message.clone());
                                if let Some(current) = manager
                                    .store
                                    .lock()
                                    .await
                                    .projects
                                    .get_mut(&project_id)
                                    .and_then(|project| project.runtime.get_mut(&service_id))
                                {
                                    if current.run_id.as_deref() == Some(&run_id) {
                                        current.state = "stopping".into();
                                        current.error = Some(message);
                                    }
                                }
                                let _ = killpg(Pid::from_raw(pid as i32), Signal::SIGTERM);
                                let force_pid = pid;
                                tokio::spawn(async move {
                                    tokio::time::sleep(Duration::from_secs(5)).await;
                                    if group_alive(force_pid) {
                                        let _ = killpg(Pid::from_raw(force_pid as i32), Signal::SIGKILL);
                                    }
                                });
                            }
                        }
                    }
                }
            } else {
                wait.await
            };
            // The shell may exit before children. Clean the owned group before releasing this run.
            if group_alive(pid) {
                let _ = killpg(Pid::from_raw(pid as i32), Signal::SIGTERM);
                for _ in 0..50 {
                    if !group_alive(pid) {
                        break;
                    }
                    tokio::time::sleep(Duration::from_millis(100)).await;
                }
                if group_alive(pid) {
                    let _ = killpg(Pid::from_raw(pid as i32), Signal::SIGKILL);
                }
            }
            let _ = drain_deadline.send(Some(tokio::time::Instant::now() + Duration::from_secs(2)));
            drop(traffic_guard.take());
            if let Some(session) = route_session.take() {
                session.shutdown().await;
            }
            let code = result.as_ref().ok().and_then(|s| s.code());
            let mut store = manager.store.lock().await;
            if let Some(current) = store
                .projects
                .get_mut(&project_id)
                .and_then(|p| p.runtime.get_mut(&service_id))
            {
                if current.run_id.as_deref() != Some(&run_id) {
                    return;
                }
                if group_alive(pid) {
                    current.state = "stopping".into();
                    current.error = Some("Owned child processes are still present after cleanup. Waiting for their exit.".into());
                    drop(store);
                    while group_alive(pid) {
                        tokio::time::sleep(Duration::from_millis(100)).await;
                    }
                    store = manager.store.lock().await;
                }
            }
            let terminal = if let Some(current) = store
                .projects
                .get_mut(&project_id)
                .and_then(|p| p.runtime.get_mut(&service_id))
            {
                if current.run_id.as_deref() != Some(&run_id) {
                    return;
                }
                let stopped = current.state == "stopping";
                current.error = vpn_failure.clone();
                current.state = if vpn_failure.is_some() {
                    "failed"
                } else if stopped || (code == Some(0) && !has_health) {
                    "stopped"
                } else {
                    "failed"
                }
                .into();
                current.exit_code = code;
                current.pid = None;
                if vpn_failure.is_none() && !stopped && current.state == "failed" {
                    current.error = Some(format!("Command exited {}. Check stderr and confirm the required tools are available in your login shell.", code.map(|n| format!("with code {n}")).unwrap_or_else(|| "after a signal".into())));
                }
                current.activity = format!(
                    "{} · {}",
                    if vpn_failure.is_some() {
                        "VPN route stopped"
                    } else if stopped {
                        "Stopped"
                    } else {
                        "Process exited"
                    },
                    chrono::Local::now().format("%H:%M:%S")
                );
                current.clone()
            } else {
                return;
            };
            drop(store);
            let _ = started_log.await;
            let incomplete_stdout = if let Some(a) = a {
                a.await.unwrap_or(true)
            } else {
                false
            };
            let incomplete_stderr = if let Some(b) = b {
                b.await.unwrap_or(true)
            } else {
                false
            };
            if incomplete_stdout || incomplete_stderr {
                manager.logs.lock().await.push(&key, &run_id, "system", "Service output may be incomplete because a pipe remained open after process exit.");
            }
            manager.logs.lock().await.push(
                &key,
                &run_id,
                "system",
                &format!(
                    "Process exited. Code: {}",
                    code.map(|n| n.to_string())
                        .unwrap_or_else(|| "signal".into())
                ),
            );
            manager
                .finish_history(&project_id, &service_id, &terminal)
                .await;
        });
        if let Some(health) = definition.health_check.clone() {
            let manager = self.clone();
            let project = project.to_string();
            let service = service.to_string();
            tokio::spawn(async move {
                manager
                    .check_health(project, service, run, health.url)
                    .await;
            });
        }
        Ok(snapshot)
    }
    async fn run_compose(&self, job: crate::compose::ComposeJob) {
        let project = job.project.clone();
        let service = job.service.clone();
        if let Some(terminal) = self.run_compose_inner(job).await {
            self.finish_history(&project, &service, &terminal).await;
        }
    }
    async fn run_compose_inner(&self, job: crate::compose::ComposeJob) -> Option<Snapshot> {
        use crate::compose::ComposeLaunch;
        let key = format!("{}/{}", job.project, job.service);
        let mut lease = None;
        let prepared = if let Some(vpn) = &job.definition.vpn {
            let user = crate::project::ServiceRef {
                project_id: job.project.clone(),
                service_id: job.service.clone(),
            };
            let acquisition = self.vpns.acquire(&vpn.profile_id, &user, &job.run);
            tokio::pin!(acquisition);
            let acquired = loop {
                tokio::select! {
                    result=&mut acquisition => break result,
                    _=tokio::time::sleep(Duration::from_millis(100)) => {
                        let mut store=self.store.lock().await;
                        let snapshot=store.projects.get_mut(&job.project)?.runtime.get_mut(&job.service)?;
                        if snapshot.run_id.as_deref()!=Some(&job.run) {return None;}
                        if snapshot.state=="stopping" {snapshot.state="stopped".into();return Some(snapshot.clone());}
                    }
                }
            };
            match acquired {
                Ok(acquired) => {
                    lease = Some(acquired);
                    ComposeLaunch::prepare_vpn(
                        &job.root,
                        &job.cwd,
                        &job.definition,
                        &job.state_dir,
                        &job.project,
                        &job.owner,
                        lease.as_ref().unwrap(),
                    )
                    .await
                }
                Err(error) => Err(error),
            }
        } else {
            ComposeLaunch::prepare(
                &job.root,
                &job.cwd,
                &job.definition,
                &job.state_dir,
                &job.project,
                &job.owner,
            )
            .await
        };
        let launch = match prepared {
            Ok(launch) => launch,
            Err(error) => {
                let mut store = self.store.lock().await;
                let snapshot = store
                    .projects
                    .get_mut(&job.project)?
                    .runtime
                    .get_mut(&job.service)?;
                if snapshot.run_id.as_deref() != Some(&job.run) {
                    return None;
                }
                snapshot.state = "failed".into();
                snapshot.error = Some(error.message);
                return Some(snapshot.clone());
            }
        };
        // Stop may arrive while preflight is awaiting Docker.
        {
            let mut store = self.store.lock().await;
            let snapshot = store
                .projects
                .get_mut(&job.project)?
                .runtime
                .get_mut(&job.service)?;
            if snapshot.run_id.as_deref() != Some(&job.run) {
                return None;
            }
            if snapshot.state == "stopping" {
                snapshot.state = "stopped".into();
                return Some(snapshot.clone());
            }
        }
        if let Some(vpn) = lease.as_ref() {
            if !vpn.healthy().await.unwrap_or(false) {
                let mut store = self.store.lock().await;
                let snapshot = store
                    .projects
                    .get_mut(&job.project)?
                    .runtime
                    .get_mut(&job.service)?;
                if snapshot.run_id.as_deref() != Some(&job.run) {
                    return None;
                }
                snapshot.state = "failed".into();
                snapshot.error=Some("VPN health or ownership changed during worker preparation. No command was started.".into());
                return Some(snapshot.clone());
            }
        }
        // Health inspection can await Docker; honor Stop again before spawning its creator.
        {
            let mut store = self.store.lock().await;
            let snapshot = store
                .projects
                .get_mut(&job.project)?
                .runtime
                .get_mut(&job.service)?;
            if snapshot.run_id.as_deref() != Some(&job.run) {
                return None;
            }
            if snapshot.state == "stopping" {
                snapshot.state = "stopped".into();
                return Some(snapshot.clone());
            }
        }
        let mut command = launch.up();
        command
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .process_group(0);
        let mut child = match command.spawn() {
            Ok(child) => child,
            Err(_) => {
                let mut store = self.store.lock().await;
                let snapshot = store
                    .projects
                    .get_mut(&job.project)?
                    .runtime
                    .get_mut(&job.service)?;
                if snapshot.run_id.as_deref() != Some(&job.run) {
                    return None;
                }
                snapshot.state = "failed".into();
                snapshot.error = Some("Could not launch Docker Compose.".into());
                return Some(snapshot.clone());
            }
        };
        let runner_pid = child.id().expect("spawned Compose runner has an identity");
        // Container stop is owned by this worker, never by the generic process-group stop.
        let mut readers = Vec::new();
        if let Some(pipe) = child.stdout.take() {
            readers.push(tokio::spawn(drain(
                pipe,
                self.logs.clone(),
                key.clone(),
                job.run.clone(),
                "stdout",
                None,
            )));
        }
        if let Some(pipe) = child.stderr.take() {
            readers.push(tokio::spawn(drain(
                pipe,
                self.logs.clone(),
                key.clone(),
                job.run.clone(),
                "stderr",
                None,
            )));
        }
        self.logs.lock().await.push(
            &key,
            &job.run,
            "system",
            "Started an owned local Compose project. Stop preserves containers and volumes.",
        );
        if let Some(health) = job.definition.health_check.clone() {
            let manager = self.clone();
            let project = job.project.clone();
            let service = job.service.clone();
            let run = job.run.clone();
            tokio::spawn(async move {
                manager
                    .check_health(project, service, run, health.url)
                    .await;
            });
        }
        let mut exited = false;
        let mut terminal = None;
        let mut vpn_failure: Option<String> = None;
        loop {
            let mut current = match self.status(&job.project, &job.service).await {
                Ok(s) if s.run_id.as_deref() == Some(&job.run) => s,
                _ => break,
            };
            exited |= child.try_wait().ok().flatten().is_some();
            if current.state != "stopping" {
                if let Some(vpn) = lease.as_ref() {
                    if !vpn.worker_safe().await.unwrap_or(false) {
                        vpn_failure=Some("VPN gateway stopped or ownership was lost. The worker was stopped; reconnect the profile before restarting.".into());
                        if let Some(snapshot) = self
                            .store
                            .lock()
                            .await
                            .projects
                            .get_mut(&job.project)
                            .and_then(|p| p.runtime.get_mut(&job.service))
                        {
                            if snapshot.run_id.as_deref() != Some(&job.run) {
                                break;
                            }
                            snapshot.state = "stopping".into();
                            snapshot.error = vpn_failure.clone();
                        }
                        current.state = "stopping".into();
                    }
                }
            }
            let stopping = current.state == "stopping";
            let result = if stopping {
                // The Compose plugin is a child of Docker: reap and verify the whole creator group.
                if group_alive(runner_pid) {
                    let _ = killpg(Pid::from_raw(runner_pid as i32), Signal::SIGINT);
                    for _ in 0..70 {
                        let _ = child.try_wait();
                        if !group_alive(runner_pid) {
                            break;
                        }
                        tokio::time::sleep(Duration::from_millis(100)).await;
                    }
                    if group_alive(runner_pid) {
                        let _ = killpg(Pid::from_raw(runner_pid as i32), Signal::SIGKILL);
                    }
                    for _ in 0..20 {
                        let _ = child.try_wait();
                        if !group_alive(runner_pid) {
                            break;
                        }
                        tokio::time::sleep(Duration::from_millis(100)).await;
                    }
                }
                exited = child.try_wait().ok().flatten().is_some();
                if group_alive(runner_pid) {
                    if let Some(s) = self
                        .store
                        .lock()
                        .await
                        .projects
                        .get_mut(&job.project)
                        .and_then(|p| p.runtime.get_mut(&job.service))
                    {
                        if s.run_id.as_deref() != Some(&job.run) {
                            break;
                        }
                        s.error = Some(
                            "Compose runner cleanup is not yet verified. Stop remains in progress."
                                .into(),
                        );
                    }
                    tokio::time::sleep(Duration::from_secs(1)).await;
                    continue;
                }
                launch.stop().await.map(|_| ("stopped", "notConfigured"))
            } else {
                launch.state().await.map(|(state, health)| {
                    if state == "failed" && (!exited || group_alive(runner_pid)) {
                        ("starting", health)
                    } else {
                        (state, health)
                    }
                })
            };
            // A VPN worker releases its lease only after the creator group and all containers stop.
            if lease.is_some()
                && !stopping
                && (result.as_ref().is_err()
                    || result.as_ref().is_ok_and(|(state, _)| {
                        *state == "failed"
                            || *state == "stopped"
                            || (*state == "starting" && exited && !group_alive(runner_pid))
                    }))
            {
                vpn_failure=Some("The VPN worker exited or its Docker state could not be verified. Cleanup is required before restarting.".into());
                if let Some(snapshot) = self
                    .store
                    .lock()
                    .await
                    .projects
                    .get_mut(&job.project)
                    .and_then(|p| p.runtime.get_mut(&job.service))
                {
                    if snapshot.run_id.as_deref() != Some(&job.run) {
                        break;
                    }
                    snapshot.state = "stopping".into();
                    snapshot.error = vpn_failure.clone();
                }
                continue;
            }
            if stopping && result.as_ref().is_ok_and(|(state, _)| *state == "stopped") {
                drop(lease.take());
            }
            let mut store = self.store.lock().await;
            if let Some(s) = store
                .projects
                .get_mut(&job.project)
                .and_then(|p| p.runtime.get_mut(&job.service))
            {
                // Never overwrite a stop admitted while Docker was being inspected.
                if s.run_id.as_deref() != Some(&job.run) {
                    break;
                }
                match result {
                    Ok((state, health)) if stopping || s.state != "stopping" => {
                        s.state = if job.definition.health_check.is_some()
                            && state == "running"
                            && s.health == "healthy"
                        {
                            "ready"
                        } else {
                            state
                        }
                        .into();
                        if job.definition.health_check.is_none() {
                            s.health = health.into();
                        }
                        if state == "failed" {
                            s.error = Some(
                                "Compose containers exited. Inspect the service output.".into(),
                            )
                        } else if job.definition.health_check.is_none() {
                            s.error = None;
                        }
                        if state == "stopped" {
                            if let Some(message) = vpn_failure.as_ref() {
                                s.state = "failed".into();
                                s.error = Some(message.clone());
                            }
                            terminal = Some(s.clone());
                            break;
                        }
                        if exited && !group_alive(runner_pid) && state == "starting" {
                            s.state = "failed".into();
                            s.error = Some(
                                "Compose exited before the expected containers started.".into(),
                            );
                            terminal = Some(s.clone());
                            break;
                        }
                        if state == "failed" {
                            terminal = Some(s.clone());
                            break;
                        }
                    }
                    Err(error) => {
                        // Inspection failures do not prove containers stopped. Keep Stop available.
                        s.state = if stopping { "stopping" } else { "running" }.into();
                        s.error = Some(error.message);
                    }
                    _ => {}
                }
            }
            drop(store);
            exited = child.try_wait().ok().flatten().is_some();
            tokio::time::sleep(Duration::from_secs(1)).await;
        }
        if child.try_wait().ok().flatten().is_none() {
            let _ = child.kill().await;
        }
        let _ = child.wait().await;
        for mut reader in readers {
            if tokio::time::timeout(Duration::from_secs(2), &mut reader)
                .await
                .is_err()
            {
                reader.abort();
            }
        }
        self.logs.lock().await.push(
            &key,
            &job.run,
            "system",
            "Compose observer finished. Containers and volumes were preserved.",
        );
        terminal
    }
    pub async fn stop(&self, project: &str, service: &str) -> Result<Snapshot> {
        self.stop_inner(project, service, None).await
    }
    pub(crate) async fn stop_confirmed(
        &self,
        project: &str,
        service: &str,
        run_id: &str,
        pid: u32,
    ) -> Result<Snapshot> {
        self.stop_inner(project, service, Some((run_id, pid))).await
    }
    async fn stop_inner(
        &self,
        project: &str,
        service: &str,
        expected: Option<(&str, u32)>,
    ) -> Result<Snapshot> {
        let mut store = self.store.lock().await;
        let p = store
            .projects
            .get_mut(project)
            .ok_or_else(|| AppError::new("PROJECT_NOT_AUTHORIZED", "Project is not open."))?;
        let current = p.runtime.get_mut(service);
        if let Some((run_id, pid)) = expected {
            if !current.as_ref().is_some_and(|s| {
                s.run_id.as_deref() == Some(run_id) && s.pid == Some(pid) && s.active()
            }) {
                return Err(AppError::new(
                    "PROCESS_CHANGED",
                    "The managed run changed. Refresh and confirm its current process again.",
                ));
            }
        }
        let Some(current) = current else {
            return Ok(Snapshot::default());
        };
        if !current.active() {
            return Ok(current.clone());
        }
        if current.state == "stopping" {
            return Ok(current.clone());
        }
        current.state = "stopping".into();
        let snapshot = current.clone();
        let pid = current.pid;
        let run = current.run_id.clone();
        drop(store);
        if let Some(pid) = pid {
            let result = killpg(Pid::from_raw(pid as i32), Signal::SIGTERM);
            if result.is_err() && group_alive(pid) {
                return Err(AppError::new(
                    "STOP_FAILED",
                    "Could not signal the owned process group.",
                ));
            }
            let manager = self.clone();
            let project = project.to_string();
            let service = service.to_string();
            tokio::spawn(async move {
                tokio::time::sleep(Duration::from_secs(5)).await;
                let current = manager.status(&project, &service).await.ok();
                if current.is_some_and(|s| s.run_id == run && s.state == "stopping")
                    && group_alive(pid)
                {
                    let _ = killpg(Pid::from_raw(pid as i32), Signal::SIGKILL);
                }
            });
        }
        Ok(snapshot)
    }
    pub async fn restart(&self, project: &str, service: &str, actor: &str) -> Result<Snapshot> {
        if self.status(project, service).await?.state == "stopping" {
            return Err(AppError::new(
                "OPERATION_IN_PROGRESS",
                "A stop or restart is already in progress.",
            ));
        }
        self.stop(project, service).await?;
        for _ in 0..600 {
            if !self.status(project, service).await?.active() {
                return self.start(project, service, actor).await;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        Err(AppError::new(
            "STOP_FAILED",
            "The previous process has not stopped. Restart was cancelled.",
        ))
    }
    pub async fn shutdown(&self) -> Result<()> {
        if self.shutting_down.swap(true, Ordering::SeqCst) {
            return Err(AppError::new(
                "OPERATION_IN_PROGRESS",
                "Shutdown is already in progress.",
            ));
        }
        let result = self.shutdown_inner().await;
        if result.is_err() {
            self.shutting_down.store(false, Ordering::SeqCst);
        }
        result
    }
    async fn shutdown_inner(&self) -> Result<()> {
        let keys: Vec<_> = self
            .projects()
            .await
            .into_iter()
            .flat_map(|p| {
                p.runtime
                    .into_iter()
                    .filter(|(_, s)| s.active())
                    .map(move |(s, _)| (p.id.clone(), s))
            })
            .collect();
        for (p, s) in &keys {
            self.stop(p, s).await?;
        }
        for _ in 0..600 {
            let mut active = false;
            for (p, s) in &keys {
                active |= self.status(p, s).await?.active();
            }
            if !active {
                self.vpns.shutdown().await?;
                let mut store = self.store.lock().await;
                store.private.dirty = false;
                return store.persist();
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        Err(AppError::new(
            "STOP_FAILED",
            "Some services have not stopped. Quit was cancelled.",
        ))
    }
    async fn check_health(&self, project: String, service: String, run: String, url: String) {
        let parsed = match reqwest::Url::parse(&url) {
            Ok(u) => u,
            Err(_) => return,
        };
        let port = parsed.port_or_known_default().unwrap_or(80);
        let client = match reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .timeout(Duration::from_secs(2))
            .no_proxy()
            .resolve(
                "localhost",
                std::net::SocketAddr::from(([127, 0, 0, 1], port)),
            )
            .build()
        {
            Ok(c) => c,
            Err(_) => return,
        };
        let started = tokio::time::Instant::now();
        loop {
            let state = self.status(&project, &service).await.ok();
            if !state.is_some_and(|s| {
                s.run_id.as_deref() == Some(&run)
                    && matches!(s.state.as_str(), "starting" | "running" | "ready")
            }) {
                break;
            }
            let healthy = client
                .get(&url)
                .send()
                .await
                .is_ok_and(|r| r.status().is_success());
            let mut store = self.store.lock().await;
            if let Some(current) = store
                .projects
                .get_mut(&project)
                .and_then(|p| p.runtime.get_mut(&service))
            {
                if current.run_id.as_deref() == Some(&run)
                    && matches!(current.state.as_str(), "starting" | "running" | "ready")
                {
                    current.state = if healthy {
                        "ready"
                    } else if started.elapsed() < Duration::from_secs(30) {
                        "starting"
                    } else {
                        "running"
                    }
                    .into();
                    current.health = if healthy { "healthy" } else { "unhealthy" }.into();
                    if healthy {
                        current.error = None;
                    } else if started.elapsed() >= Duration::from_secs(30) {
                        current.error = Some("Health check has not succeeded within 30 seconds. The service remains running and checks continue.".into());
                    }
                }
            }
            drop(store);
            tokio::time::sleep(Duration::from_secs(1)).await;
        }
    }
}
fn group_alive(pid: u32) -> bool {
    matches!(
        killpg(Pid::from_raw(pid as i32), None),
        Ok(()) | Err(nix::errno::Errno::EPERM)
    )
}
async fn drain(
    mut pipe: impl AsyncRead + Unpin,
    logs: Arc<Mutex<LogStore>>,
    key: String,
    run: String,
    stream: &'static str,
    mut deadline: Option<tokio::sync::watch::Receiver<Option<tokio::time::Instant>>>,
) -> bool {
    let mut chunk = [0u8; 4096];
    let mut line = Vec::with_capacity(16384);
    let mut after_deadline = 0usize;
    let mut incomplete = false;
    loop {
        let read = tokio::select! {
            biased;
            result = pipe.read(&mut chunk) => result,
            _ = async {
                if let Some(deadline) = &mut deadline {
                    loop {
                        let value = *deadline.borrow_and_update();
                        if let Some(value) = value {
                            tokio::time::sleep_until(value).await;
                            break;
                        }
                        if deadline.changed().await.is_err() { break; }
                    }
                } else {
                    std::future::pending::<()>().await;
                }
            } => {
                incomplete = true;
                break;
            }
        };
        let read_at = tokio::time::Instant::now();
        match read {
            Ok(0) | Err(_) => break,
            Ok(n) => {
                for byte in &chunk[..n] {
                    if *byte == b'\n' || line.len() >= 16380 {
                        let continued = *byte != b'\n';
                        let end = if continued {
                            std::str::from_utf8(&line)
                                .err()
                                .filter(|e| e.error_len().is_none())
                                .map_or(line.len(), |e| e.valid_up_to())
                        } else {
                            line.len()
                        };
                        let text = String::from_utf8_lossy(&line[..end]).into_owned();
                        logs.lock()
                            .await
                            .push_chunk(&key, &run, stream, &text, continued);
                        line.drain(..end);
                    }
                    if *byte != b'\n' {
                        line.push(*byte);
                    }
                }
                if deadline
                    .as_ref()
                    .and_then(|d| *d.borrow())
                    .is_some_and(|d| read_at >= d)
                {
                    after_deadline += n;
                    // shortcut: cap post-deadline output at 1 MiB, increase if pipe capacities exceed this.
                    if after_deadline >= 1024 * 1024 {
                        incomplete = true;
                        break;
                    }
                }
            }
        }
    }
    if !line.is_empty() {
        logs.lock()
            .await
            .push(&key, &run, stream, &String::from_utf8_lossy(&line));
    }
    incomplete
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::AsyncWriteExt;

    struct TestProcess(Option<u32>);
    impl Drop for TestProcess {
        fn drop(&mut self) {
            if let Some(pid) = self.0 {
                let _ = killpg(Pid::from_raw(pid as i32), Signal::SIGKILL);
            }
        }
    }

    async fn routed_fixture() -> (
        tempfile::TempDir,
        tempfile::TempDir,
        Manager,
        String,
        VpnRoute,
    ) {
        let root = tempfile::tempdir().unwrap();
        let state = tempfile::tempdir().unwrap();
        let manager = Manager::new(state.path().into()).unwrap();
        let config = root.path().join("fixture.ovpn");
        std::fs::write(&config, b"client\n").unwrap();
        let profile = manager
            .vpns
            .import(crate::vpn::VpnImport {
                name: "fixture".into(),
                config_path: config.to_string_lossy().into(),
                username: String::new(),
                password: String::new(),
                key_passphrase: String::new(),
                published_ports: vec![],
                outbound_subnets: vec![],
                health_target: String::new(),
                dns_address: String::new(),
                health_icmp_target: String::new(),
            })
            .await
            .unwrap();
        let project = manager.open(root.path().to_str().unwrap()).await.unwrap();
        let port = std::net::TcpListener::bind("127.0.0.1:0")
            .unwrap()
            .local_addr()
            .unwrap()
            .port();
        let route = VpnRoute {
            profile_id: profile.id,
            remote_host: "example.test".into(),
            remote_port: 443,
            local_port: port,
        };
        let definition = ServiceDefinition {
            id: "fixture".into(),
            name: "Fixture".into(),
            cwd: ".".into(),
            kind: ServiceKind::Command {
                command: "exec sleep 30".into(),
            },
            env_file: None,
            health_check: None,
            ports: vec![],
            url: None,
            depends_on: vec![],
            vpn: None,
            vpn_routes: vec![route.clone()],
        };
        manager
            .save(&project.id, definition, "absent", true)
            .await
            .unwrap();
        (root, state, manager, project.id, route)
    }

    fn prepare_fixture(manager: &Manager, route: &VpnRoute, proxy: SocketAddr) {
        *manager.prepared_test_session.lock().unwrap() =
            Some(vec![crate::vpn_relay::RelayTarget {
                route: route.clone(),
                proxy,
            }]);
    }

    async fn wait_stopped_and_archived(manager: &Manager, project: &str, run: &str) {
        tokio::time::timeout(Duration::from_secs(3), async {
            loop {
                let status = manager.status(project, "fixture").await.unwrap();
                let history = manager.run_history(project, "fixture").await.unwrap();
                if status.state == "stopped"
                    && history
                        .iter()
                        .any(|r| r.run_id == run && r.state == "stopped" && r.ended_at.is_some())
                {
                    break;
                }
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("run did not stop and finish history");
    }

    #[tokio::test]
    async fn other_local_client_moves_counters_while_managed_worker_is_idle() {
        use tokio::io::AsyncReadExt;
        let (_root, _state, manager, project, route) = routed_fixture().await;
        let proxy = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let proxy_address = proxy.local_addr().unwrap();
        let proxy_task = tokio::spawn(async move {
            let (mut stream, _) = proxy.accept().await.unwrap();
            let mut request = Vec::new();
            loop {
                request.push(stream.read_u8().await.unwrap());
                if request.ends_with(b"\r\n\r\n") {
                    break;
                }
            }
            stream
                .write_all(b"HTTP/1.1 200 Connection Established\r\n\r\n")
                .await
                .unwrap();
            let mut bytes = [0; 4];
            stream.read_exact(&mut bytes).await.unwrap();
            assert_eq!(&bytes, b"ping");
            stream.write_all(b"pong").await.unwrap();
        });
        prepare_fixture(&manager, &route, proxy_address);
        let started = manager.start(&project, "fixture", "test").await.unwrap();
        let mut process = TestProcess(started.pid);
        let run = started.run_id.unwrap();
        let initial = manager.network_traffic();
        assert_eq!((initial[0].tx_bytes, initial[0].rx_bytes), (0, 0));
        let project_view = manager.projects().await.remove(0);
        let mut changed = project_view.config.services[0].clone();
        changed.vpn_routes[0].remote_host = "changed.test".into();
        assert!(manager
            .save(&project, changed.clone(), &project_view.revision, false)
            .await
            .is_err());
        changed.vpn_routes.clear();
        assert!(manager
            .save(&project, changed, &project_view.revision, false)
            .await
            .is_err());
        let mut client = tokio::net::TcpStream::connect((Ipv4Addr::LOCALHOST, route.local_port))
            .await
            .unwrap();
        client.write_all(b"ping").await.unwrap();
        let mut reply = [0; 4];
        client.read_exact(&mut reply).await.unwrap();
        assert_eq!(&reply, b"pong");
        proxy_task.await.unwrap();
        let sample = manager.network_traffic();
        assert_eq!((sample[0].tx_bytes, sample[0].rx_bytes), (4, 4));
        assert_eq!(sample[0].remote_host, "example.test");
        assert_eq!(sample[0].run_id, run);
        assert_eq!(
            manager.status(&project, "fixture").await.unwrap().state,
            "running"
        );
        manager.stop(&project, "fixture").await.unwrap();
        wait_stopped_and_archived(&manager, &project, &run).await;
        process.0 = None;
        manager.shutdown().await.unwrap();
    }

    #[tokio::test]
    async fn active_routes_reject_api_and_external_edits_until_stopped() {
        let (root, _state, manager, project, route) = routed_fixture().await;
        prepare_fixture(&manager, &route, "127.0.0.1:1".parse().unwrap());
        let started = manager.start(&project, "fixture", "test").await.unwrap();
        let mut process = TestProcess(started.pid);
        let run = started.run_id.unwrap();
        let project_view = manager.projects().await.remove(0);
        let mut edited = project_view.config.services[0].clone();
        edited.vpn_routes[0].remote_host = "changed.test".into();
        assert_eq!(
            manager
                .save(&project, edited.clone(), &project_view.revision, false)
                .await
                .err()
                .unwrap()
                .code,
            "OPERATION_IN_PROGRESS"
        );
        edited.vpn_routes.clear();
        assert_eq!(
            manager
                .save(&project, edited, &project_view.revision, false)
                .await
                .err()
                .unwrap()
                .code,
            "OPERATION_IN_PROGRESS"
        );
        let mut unrelated = project_view.config.services[0].clone();
        unrelated.name = "Renamed".into();
        let saved = manager
            .save(&project, unrelated, &project_view.revision, false)
            .await
            .unwrap();
        assert_eq!(saved.config.services[0].name, "Renamed");
        assert_eq!(manager.network_traffic()[0].remote_host, "example.test");
        let config_path = root.path().join(".service-manager/project.json");
        let mut external = saved.config.clone();
        external.services[0].vpn_routes[0].remote_host = "external.test".into();
        std::fs::write(&config_path, serde_json::to_vec(&external).unwrap()).unwrap();
        assert_eq!(
            manager
                .store
                .lock()
                .await
                .refresh(&project)
                .err()
                .unwrap()
                .code,
            "CONFIG_CONFLICT"
        );
        assert_eq!(
            manager.projects().await[0].config.services[0].vpn_routes[0].remote_host,
            "example.test"
        );
        external.services[0].vpn_routes.clear();
        std::fs::write(&config_path, serde_json::to_vec(&external).unwrap()).unwrap();
        assert_eq!(
            manager
                .store
                .lock()
                .await
                .refresh(&project)
                .err()
                .unwrap()
                .code,
            "CONFIG_CONFLICT"
        );
        manager.stop(&project, "fixture").await.unwrap();
        wait_stopped_and_archived(&manager, &project, &run).await;
        process.0 = None;
        let loaded = manager.store.lock().await.refresh(&project).unwrap();
        assert!(loaded.config.services[0].vpn_routes.is_empty());
        let mut restored = loaded.config.services[0].clone();
        restored.vpn_routes.push(route);
        let saved = manager
            .save(&project, restored.clone(), &loaded.revision, false)
            .await
            .unwrap();
        restored.vpn_routes[0].remote_host = "post-stop.test".into();
        manager
            .save(&project, restored, &saved.revision, false)
            .await
            .unwrap();
        manager.shutdown().await.unwrap();
    }

    async fn cancelled_preflight_recovers(cancel_during_shutdown: bool) {
        let (_root, _state, manager, project, route) = routed_fixture().await;
        let proxy = "127.0.0.1:1".parse().unwrap();
        prepare_fixture(&manager, &route, proxy);
        let (ready_tx, ready_rx) = tokio::sync::oneshot::channel();
        let (release_tx, release_rx) = tokio::sync::oneshot::channel();
        *manager.preflight_test_barrier.lock().unwrap() = Some((ready_tx, release_rx));
        let (shutdown_ready_tx, shutdown_ready_rx) = tokio::sync::oneshot::channel();
        let (shutdown_release_tx, shutdown_release_rx) = tokio::sync::oneshot::channel();
        *manager.shutdown_test_barrier.lock().unwrap() =
            Some((shutdown_ready_tx, shutdown_release_rx));
        let worker = manager.clone();
        let id = project.clone();
        let starting = tokio::spawn(async move { worker.start(&id, "fixture", "test").await });
        ready_rx.await.unwrap();
        let run = manager
            .status(&project, "fixture")
            .await
            .unwrap()
            .run_id
            .unwrap();
        if cancel_during_shutdown {
            manager.stop(&project, "fixture").await.unwrap();
            let _ = release_tx.send(());
            shutdown_ready_rx.await.unwrap();
            starting.abort();
        } else {
            starting.abort();
            tokio::time::timeout(Duration::from_secs(1), async {
                while manager.status(&project, "fixture").await.unwrap().state != "stopping" {
                    tokio::task::yield_now().await;
                }
            })
            .await
            .expect("cancellation did not stop preflight");
            assert_eq!(
                manager.stop(&project, "fixture").await.unwrap().state,
                "stopping"
            );
            let _ = release_tx.send(());
            let _ = shutdown_ready_rx.await;
        }
        let _ = starting.await;
        let _ = shutdown_release_tx.send(());
        wait_stopped_and_archived(&manager, &project, &run).await;
        assert!(std::net::TcpListener::bind((Ipv4Addr::LOCALHOST, route.local_port)).is_ok());
        prepare_fixture(&manager, &route, proxy);
        let restarted = manager.restart(&project, "fixture", "test").await.unwrap();
        let mut process = TestProcess(restarted.pid);
        let next = restarted.run_id.unwrap();
        assert_ne!(next, run);
        manager.stop(&project, "fixture").await.unwrap();
        wait_stopped_and_archived(&manager, &project, &next).await;
        process.0 = None;
        tokio::time::timeout(Duration::from_secs(3), manager.shutdown())
            .await
            .unwrap()
            .unwrap();
    }

    #[tokio::test]
    async fn cancelled_native_preflight_still_finishes_lifecycle() {
        cancelled_preflight_recovers(false).await;
    }

    #[tokio::test]
    async fn cancelled_stopped_preflight_shutdown_still_finishes_lifecycle() {
        cancelled_preflight_recovers(true).await;
    }

    #[tokio::test]
    async fn shutdown_waits_for_cancelled_preflight_history_to_finish() {
        let (_root, _state, manager, project, route) = routed_fixture().await;
        prepare_fixture(&manager, &route, "127.0.0.1:1".parse().unwrap());
        let (ready_tx, ready_rx) = tokio::sync::oneshot::channel();
        let (release_tx, release_rx) = tokio::sync::oneshot::channel();
        *manager.preflight_test_barrier.lock().unwrap() = Some((ready_tx, release_rx));
        let (shutdown_ready_tx, shutdown_ready_rx) = tokio::sync::oneshot::channel();
        let (shutdown_release_tx, shutdown_release_rx) = tokio::sync::oneshot::channel();
        *manager.shutdown_test_barrier.lock().unwrap() =
            Some((shutdown_ready_tx, shutdown_release_rx));
        let worker = manager.clone();
        let id = project.clone();
        let starting = tokio::spawn(async move { worker.start(&id, "fixture", "test").await });
        ready_rx.await.unwrap();
        let run = manager
            .status(&project, "fixture")
            .await
            .unwrap()
            .run_id
            .unwrap();
        manager.stop(&project, "fixture").await.unwrap();
        release_tx.send(()).unwrap();
        shutdown_ready_rx.await.unwrap();
        starting.abort();
        let _ = starting.await;
        let logs = manager.logs.lock().await;
        let closing = manager.clone();
        let shutdown = tokio::spawn(async move { closing.shutdown().await });
        shutdown_release_tx.send(()).unwrap();
        tokio::time::timeout(Duration::from_secs(3), async {
            while std::net::TcpListener::bind((Ipv4Addr::LOCALHOST, route.local_port)).is_err() {
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
        assert_eq!(
            manager.status(&project, "fixture").await.unwrap().state,
            "stopping"
        );
        assert!(!shutdown.is_finished());
        drop(logs);
        tokio::time::timeout(Duration::from_secs(3), shutdown)
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        wait_stopped_and_archived(&manager, &project, &run).await;
    }

    #[tokio::test]
    async fn shutdown_waits_for_cancelled_spawn_failure_history_to_finish() {
        let (root, _state, manager, project, route) = routed_fixture().await;
        std::fs::write(
            root.path().join("fixture.env"),
            format!("LARGE={}\n", "x".repeat(4 * 1024 * 1024)),
        )
        .unwrap();
        let saved = manager.projects().await.remove(0);
        let mut definition = saved.config.services[0].clone();
        definition.env_file = Some("fixture.env".into());
        manager
            .save(&project, definition, &saved.revision, true)
            .await
            .unwrap();
        prepare_fixture(&manager, &route, "127.0.0.1:1".parse().unwrap());
        let (shutdown_ready_tx, shutdown_ready_rx) = tokio::sync::oneshot::channel();
        let (shutdown_release_tx, shutdown_release_rx) = tokio::sync::oneshot::channel();
        *manager.shutdown_test_barrier.lock().unwrap() =
            Some((shutdown_ready_tx, shutdown_release_rx));
        let worker = manager.clone();
        let id = project.clone();
        let starting = tokio::spawn(async move { worker.start(&id, "fixture", "test").await });
        shutdown_ready_rx.await.unwrap();
        starting.abort();
        let _ = starting.await;
        let logs = manager.logs.lock().await;
        let closing = manager.clone();
        let shutdown = tokio::spawn(async move { closing.shutdown().await });
        shutdown_release_tx.send(()).unwrap();
        tokio::time::timeout(Duration::from_secs(3), async {
            while std::net::TcpListener::bind((Ipv4Addr::LOCALHOST, route.local_port)).is_err() {
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
        assert!(manager.status(&project, "fixture").await.unwrap().active());
        assert!(!shutdown.is_finished());
        drop(logs);
        tokio::time::timeout(Duration::from_secs(3), shutdown)
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        let history = manager.run_history(&project, "fixture").await.unwrap();
        assert_eq!(history[0].state, "failed");
        assert!(history[0].ended_at.is_some());
    }

    #[tokio::test]
    async fn network_traffic_tracks_current_runs_and_serializes_camel_case() {
        use crate::vpn_relay::{RelayTarget, VpnRelaySet};
        let state = tempfile::tempdir().unwrap();
        let manager = Manager::new(state.path().into()).unwrap();
        let relay = VpnRelaySet::start(vec![RelayTarget {
            route: VpnRoute {
                profile_id: "profile".into(),
                local_port: 0,
                remote_host: "example.test".into(),
                remote_port: 5432,
            },
            proxy: "127.0.0.1:1".parse().unwrap(),
        }])
        .await
        .unwrap();
        assert!(manager.network_traffic().is_empty());
        let old = manager.register_network_traffic("z", "service", "old", relay.traffic());
        let first = manager.register_network_traffic("a", "service", "first", relay.traffic());
        let replacement = manager.register_network_traffic("z", "service", "new", relay.traffic());
        drop(old);
        let rows = manager.network_traffic();
        assert_eq!(rows.len(), 2);
        assert_eq!((&*rows[0].project_id, &*rows[1].run_id), ("a", "new"));
        assert_eq!(
            serde_json::to_value(&rows[1]).unwrap(),
            serde_json::json!({
                "projectId":"z", "serviceId":"service", "runId":"new", "profileId":"profile",
                "localPort":0, "remoteHost":"example.test", "remotePort":5432,
                "txBytes":0, "rxBytes":0, "activeConnections":0
            })
        );
        drop(first);
        assert_eq!(manager.network_traffic().len(), 1);
        drop(replacement);
        assert!(manager.network_traffic().is_empty());
    }

    #[tokio::test]
    async fn network_traffic_stays_empty_on_setup_and_spawn_failure() {
        let root = tempfile::tempdir().unwrap();
        let state = tempfile::tempdir().unwrap();
        let manager = Manager::new(state.path().into()).unwrap();
        let config = root.path().join("fixture.ovpn");
        std::fs::write(&config, b"client\n").unwrap();
        let profile = manager
            .vpns
            .import(crate::vpn::VpnImport {
                name: "fixture".into(),
                config_path: config.to_string_lossy().into(),
                username: String::new(),
                password: String::new(),
                key_passphrase: String::new(),
                published_ports: vec![],
                outbound_subnets: vec![],
                health_target: String::new(),
                dns_address: String::new(),
                health_icmp_target: String::new(),
            })
            .await
            .unwrap();
        let project = manager.open(root.path().to_str().unwrap()).await.unwrap();
        let local_port = std::net::TcpListener::bind("127.0.0.1:0")
            .unwrap()
            .local_addr()
            .unwrap()
            .port();
        let mut definition = ServiceDefinition {
            id: "fixture".into(),
            name: "Fixture".into(),
            cwd: ".".into(),
            kind: ServiceKind::Command {
                command: "true".into(),
            },
            env_file: None,
            health_check: None,
            ports: vec![],
            url: None,
            depends_on: vec![],
            vpn: None,
            vpn_routes: vec![VpnRoute {
                profile_id: profile.id,
                remote_host: "example.test".into(),
                remote_port: 5432,
                local_port,
            }],
        };
        let saved = manager
            .save(&project.id, definition.clone(), "absent", true)
            .await
            .unwrap();
        assert!(manager.start(&project.id, "fixture", "test").await.is_err());
        assert_eq!(
            manager
                .status(&project.id, "fixture")
                .await
                .unwrap()
                .activity,
            "VPN relay setup failed"
        );
        assert!(manager.network_traffic().is_empty());

        std::fs::write(
            root.path().join("fixture.env"),
            format!("LARGE={}\n", "x".repeat(4 * 1024 * 1024)),
        )
        .unwrap();
        let route = definition.vpn_routes[0].clone();
        *manager.prepared_test_session.lock().unwrap() =
            Some(vec![crate::vpn_relay::RelayTarget {
                route,
                proxy: "127.0.0.1:1".parse().unwrap(),
            }]);
        definition.env_file = Some("fixture.env".into());
        manager
            .save(&project.id, definition, &saved.revision, true)
            .await
            .unwrap();
        let error = manager
            .start(&project.id, "fixture", "test")
            .await
            .unwrap_err();
        assert_eq!(error.code, "PROCESS_START_FAILED");
        assert!(manager.network_traffic().is_empty());
        assert!(std::net::TcpListener::bind((Ipv4Addr::LOCALHOST, local_port)).is_ok());
    }

    #[tokio::test]
    async fn invalid_routed_env_fails_before_preflight_and_runtime() {
        let root = tempfile::tempdir().unwrap();
        let state = tempfile::tempdir().unwrap();
        let manager = Manager::new(state.path().into()).unwrap();
        let config = root.path().join("fixture.ovpn");
        std::fs::write(&config, b"client\n").unwrap();
        let profile = manager
            .vpns
            .import(crate::vpn::VpnImport {
                name: "fixture".into(),
                config_path: config.to_string_lossy().into(),
                username: String::new(),
                password: String::new(),
                key_passphrase: String::new(),
                published_ports: vec![],
                outbound_subnets: vec![],
                health_target: String::new(),
                dns_address: String::new(),
                health_icmp_target: String::new(),
            })
            .await
            .unwrap();
        let project = manager.open(root.path().to_str().unwrap()).await.unwrap();
        let local_port = std::net::TcpListener::bind("127.0.0.1:0")
            .unwrap()
            .local_addr()
            .unwrap()
            .port();
        let mut definition = ServiceDefinition {
            id: "fixture".into(),
            name: "Fixture".into(),
            cwd: ".".into(),
            kind: ServiceKind::Command {
                command: "true".into(),
            },
            env_file: None,
            health_check: None,
            ports: vec![],
            url: None,
            depends_on: vec![],
            vpn: None,
            vpn_routes: vec![VpnRoute {
                profile_id: profile.id,
                remote_host: "example.test".into(),
                remote_port: 5432,
                local_port,
            }],
        };
        std::fs::write(root.path().join("invalid.env"), "BROKEN='unterminated\n").unwrap();
        definition.env_file = Some("invalid.env".into());
        *manager.prepared_test_session.lock().unwrap() =
            Some(vec![crate::vpn_relay::RelayTarget {
                route: definition.vpn_routes[0].clone(),
                proxy: "127.0.0.1:1".parse().unwrap(),
            }]);
        let _saved = manager
            .save(&project.id, definition.clone(), "absent", true)
            .await
            .unwrap();
        let error = manager
            .start(&project.id, "fixture", "test")
            .await
            .unwrap_err();
        assert_eq!(error.code, "CONFIG_INVALID");
        assert!(!manager
            .status(&project.id, "fixture")
            .await
            .unwrap()
            .active());
        assert!(manager.network_traffic().is_empty());
        assert!(std::net::TcpListener::bind((Ipv4Addr::LOCALHOST, local_port)).is_ok());
        assert!(manager
            .run_history(&project.id, "fixture")
            .await
            .unwrap()
            .is_empty());
    }

    #[tokio::test]
    async fn stopped_preflight_releases_port_before_terminal_and_restart() {
        use crate::vpn_relay::RelayTarget;
        let root = tempfile::tempdir().unwrap();
        let state = tempfile::tempdir().unwrap();
        let manager = Manager::new(state.path().into()).unwrap();
        let config = root.path().join("fixture.ovpn");
        std::fs::write(&config, b"client\n").unwrap();
        let profile = manager
            .vpns
            .import(crate::vpn::VpnImport {
                name: "fixture".into(),
                config_path: config.to_string_lossy().into(),
                username: String::new(),
                password: String::new(),
                key_passphrase: String::new(),
                published_ports: vec![],
                outbound_subnets: vec![],
                health_target: String::new(),
                dns_address: String::new(),
                health_icmp_target: String::new(),
            })
            .await
            .unwrap();
        let project = manager.open(root.path().to_str().unwrap()).await.unwrap();
        let port = std::net::TcpListener::bind("127.0.0.1:0")
            .unwrap()
            .local_addr()
            .unwrap()
            .port();
        let route = VpnRoute {
            profile_id: profile.id,
            remote_host: "example.test".into(),
            remote_port: 443,
            local_port: port,
        };
        let definition = ServiceDefinition {
            id: "fixture".into(),
            name: "Fixture".into(),
            cwd: ".".into(),
            kind: ServiceKind::Command {
                command: "exec sleep 300".into(),
            },
            env_file: None,
            health_check: None,
            ports: vec![],
            url: None,
            depends_on: vec![],
            vpn: None,
            vpn_routes: vec![route.clone()],
        };
        manager
            .save(&project.id, definition, "absent", true)
            .await
            .unwrap();
        let targets = || {
            vec![RelayTarget {
                route: route.clone(),
                proxy: "127.0.0.1:1".parse().unwrap(),
            }]
        };
        *manager.prepared_test_session.lock().unwrap() = Some(targets());
        let (ready_tx, ready_rx) = tokio::sync::oneshot::channel();
        let (release_tx, release_rx) = tokio::sync::oneshot::channel();
        *manager.preflight_test_barrier.lock().unwrap() = Some((ready_tx, release_rx));
        let (shutdown_ready_tx, shutdown_ready_rx) = tokio::sync::oneshot::channel();
        let (shutdown_release_tx, shutdown_release_rx) = tokio::sync::oneshot::channel();
        *manager.shutdown_test_barrier.lock().unwrap() =
            Some((shutdown_ready_tx, shutdown_release_rx));
        let worker = manager.clone();
        let id = project.id.clone();
        let starting = tokio::spawn(async move { worker.start(&id, "fixture", "test").await });
        ready_rx.await.unwrap();
        manager.stop(&project.id, "fixture").await.unwrap();
        assert_eq!(
            manager.status(&project.id, "fixture").await.unwrap().state,
            "stopping"
        );
        release_tx.send(()).unwrap();
        shutdown_ready_rx.await.unwrap();
        assert_eq!(
            manager.status(&project.id, "fixture").await.unwrap().state,
            "stopping"
        );
        assert!(std::net::TcpListener::bind((Ipv4Addr::LOCALHOST, port)).is_err());
        shutdown_release_tx.send(()).unwrap();
        assert_eq!(starting.await.unwrap().unwrap().state, "stopped");
        let rebound = std::net::TcpListener::bind((Ipv4Addr::LOCALHOST, port)).unwrap();
        drop(rebound);
        *manager.prepared_test_session.lock().unwrap() = Some(targets());
        manager.start(&project.id, "fixture", "test").await.unwrap();
        manager.stop(&project.id, "fixture").await.unwrap();
        tokio::time::timeout(Duration::from_secs(5), async {
            while manager
                .status(&project.id, "fixture")
                .await
                .unwrap()
                .active()
            {
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn terminal_state_waits_for_route_release_even_when_logs_block() {
        use crate::vpn_relay::RelayTarget;
        let root = tempfile::tempdir().unwrap();
        let state = tempfile::tempdir().unwrap();
        let manager = Manager::new(state.path().into()).unwrap();
        let config = root.path().join("fixture.ovpn");
        std::fs::write(&config, b"client\n").unwrap();
        let profile = manager
            .vpns
            .import(crate::vpn::VpnImport {
                name: "fixture".into(),
                config_path: config.to_string_lossy().into(),
                username: String::new(),
                password: String::new(),
                key_passphrase: String::new(),
                published_ports: vec![],
                outbound_subnets: vec![],
                health_target: String::new(),
                dns_address: String::new(),
                health_icmp_target: String::new(),
            })
            .await
            .unwrap();
        let project = manager.open(root.path().to_str().unwrap()).await.unwrap();
        let port = std::net::TcpListener::bind("127.0.0.1:0")
            .unwrap()
            .local_addr()
            .unwrap()
            .port();
        let route = VpnRoute {
            profile_id: profile.id,
            remote_host: "example.test".into(),
            remote_port: 443,
            local_port: port,
        };
        let definition = ServiceDefinition {
            id: "fixture".into(),
            name: "Fixture".into(),
            cwd: ".".into(),
            kind: ServiceKind::Command {
                command: "exec sleep 300".into(),
            },
            env_file: None,
            health_check: None,
            ports: vec![],
            url: None,
            depends_on: vec![],
            vpn: None,
            vpn_routes: vec![route.clone()],
        };
        manager
            .save(&project.id, definition, "absent", true)
            .await
            .unwrap();
        let make_targets = || {
            vec![RelayTarget {
                route: route.clone(),
                proxy: "127.0.0.1:1".parse().unwrap(),
            }]
        };
        *manager.prepared_test_session.lock().unwrap() = Some(make_targets());
        let started = manager.start(&project.id, "fixture", "test").await.unwrap();
        tokio::time::timeout(Duration::from_secs(3), async {
            loop {
                if !manager
                    .read_logs(&project.id, "fixture", None)
                    .await
                    .unwrap()
                    .records
                    .is_empty()
                {
                    break;
                }
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
        let logs = manager.logs.lock().await;
        manager.stop(&project.id, "fixture").await.unwrap();
        tokio::time::timeout(Duration::from_secs(5), async {
            while manager
                .status(&project.id, "fixture")
                .await
                .unwrap()
                .active()
            {
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
        assert!(manager.network_traffic().is_empty());
        let rebound = std::net::TcpListener::bind((Ipv4Addr::LOCALHOST, port)).unwrap();
        drop(rebound);
        *manager.prepared_test_session.lock().unwrap() = Some(make_targets());
        let restarted = manager.start(&project.id, "fixture", "test").await.unwrap();
        assert_ne!(started.run_id, restarted.run_id);
        drop(logs);
        manager.stop(&project.id, "fixture").await.unwrap();
        tokio::time::timeout(Duration::from_secs(5), async {
            while manager
                .status(&project.id, "fixture")
                .await
                .unwrap()
                .active()
            {
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
    }

    #[test]
    fn destination_routes_keep_command_services_on_native_runner() {
        let definition: ServiceDefinition = serde_json::from_value(serde_json::json!({
            "id":"api",
            "name":"API",
            "cwd":".",
            "type":"command",
            "command":"go run .",
            "vpnRoutes":[{
                "profileId":"ac9c00d2-4810-43b6-a8e9-b9f07fecf6ec",
                "remoteHost":"database.internal.example",
                "remotePort":5432,
                "localPort":15432
            }]
        }))
        .unwrap();
        assert!(!uses_compose_runner(&definition));
    }
    #[tokio::test]
    async fn no_newline_flood_remains_bounded_and_preserves_utf8() {
        let logs = Arc::new(Mutex::new(LogStore::default()));
        let (mut writer, reader) = tokio::io::duplex(4096);
        let consumer = tokio::spawn(drain(
            reader,
            logs.clone(),
            "flood".into(),
            "run".into(),
            "stdout",
            None,
        ));
        let chunk = "ก".repeat(1024);
        for _ in 0..7000 {
            writer.write_all(chunk.as_bytes()).await.unwrap();
        }
        drop(writer);
        consumer.await.unwrap();
        let page = logs.lock().await.read("flood", Some("0"));
        assert!(page.truncated);
        assert!(!page.records.is_empty());
        assert!(page
            .records
            .iter()
            .all(|r| !r.text.contains('\u{fffd}') && r.text.len() <= 16384));
        assert!(page.records.iter().any(|r| r.truncated));
    }
    #[tokio::test]
    async fn health_deadline_recovers_and_stopped_run_cannot_become_ready() {
        use std::sync::atomic::AtomicU16;
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let response = Arc::new(AtomicU16::new(503));
        let server_response = response.clone();
        let server = tokio::spawn(async move {
            loop {
                let (mut socket, _) = listener.accept().await.unwrap();
                let status = server_response.load(Ordering::SeqCst);
                let text = format!("HTTP/1.1 {status} Fixture\r\nLocation: http://127.0.0.1:{}/healthy\r\nContent-Length: 0\r\nConnection: close\r\n\r\n", address.port());
                let _ = socket.write_all(text.as_bytes()).await;
            }
        });
        let root = tempfile::tempdir().unwrap();
        let state = tempfile::tempdir().unwrap();
        let manager = Manager::new(state.path().into()).unwrap();
        let project = manager.open(root.path().to_str().unwrap()).await.unwrap();
        let definition = ServiceDefinition {
            id: "health".into(),
            name: "Health fixture".into(),
            cwd: ".".into(),
            kind: ServiceKind::Command {
                command: "fixture".into(),
            },
            env_file: None,
            health_check: None,
            ports: vec![],
            url: None,
            depends_on: vec![],
            vpn: None,
            vpn_routes: vec![],
        };
        manager
            .save(&project.id, definition, "absent", false)
            .await
            .unwrap();
        manager
            .store
            .lock()
            .await
            .projects
            .get_mut(&project.id)
            .unwrap()
            .runtime
            .insert(
                "health".into(),
                Snapshot {
                    state: "running".into(),
                    run_id: Some("fixture-run".into()),
                    ..Snapshot::default()
                },
            );
        let checking = manager.clone();
        let id = project.id.clone();
        let checker = tokio::spawn(async move {
            checking
                .check_health(
                    id,
                    "health".into(),
                    "fixture-run".into(),
                    format!("http://{address}/health"),
                )
                .await;
        });
        tokio::time::sleep(Duration::from_secs(32)).await;
        let failed_health = manager.status(&project.id, "health").await.unwrap();
        assert_eq!(failed_health.state, "running");
        assert!(failed_health.error.unwrap().contains("30 seconds"));
        response.store(302, Ordering::SeqCst);
        tokio::time::sleep(Duration::from_millis(1200)).await;
        assert_ne!(
            manager.status(&project.id, "health").await.unwrap().state,
            "ready"
        );
        response.store(200, Ordering::SeqCst);
        for _ in 0..40 {
            if manager.status(&project.id, "health").await.unwrap().state == "ready" {
                break;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        assert_eq!(
            manager.status(&project.id, "health").await.unwrap().state,
            "ready"
        );
        assert!(manager
            .status(&project.id, "health")
            .await
            .unwrap()
            .error
            .is_none());
        manager
            .store
            .lock()
            .await
            .projects
            .get_mut(&project.id)
            .unwrap()
            .runtime
            .get_mut("health")
            .unwrap()
            .state = "stopped".into();
        tokio::time::timeout(Duration::from_secs(3), checker)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            manager.status(&project.id, "health").await.unwrap().state,
            "stopped"
        );
        server.abort();
    }
}
