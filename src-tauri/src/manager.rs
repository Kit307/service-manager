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
    path::{Path, PathBuf},
    process::Stdio,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::Duration,
};
use tokio::sync::Mutex;
use tokio::{
    io::{AsyncRead, AsyncReadExt},
    process::Command,
};
#[derive(Clone)]
pub struct Manager {
    pub store: Arc<Mutex<ProjectStore>>,
    pub logs: Arc<Mutex<LogStore>>,
    shutting_down: Arc<AtomicBool>,
}
impl Manager {
    pub fn new(path: PathBuf) -> Result<Self> {
        Ok(Self {
            store: Arc::new(Mutex::new(ProjectStore::new(path)?)),
            logs: Arc::new(Mutex::new(LogStore::default())),
            shutting_down: Arc::new(AtomicBool::new(false)),
        })
    }
    pub async fn open(&self, path: &str) -> Result<Project> {
        self.store.lock().await.open(path)
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
    pub async fn save(
        &self,
        project: &str,
        definition: ServiceDefinition,
        revision: &str,
        trust: bool,
    ) -> Result<Project> {
        let mut store = self.store.lock().await;
        let mut p = store.refresh(project)?;
        if let Some(existing) = p.config.services.iter_mut().find(|s| s.id == definition.id) {
            *existing = definition.clone();
        } else {
            p.config.services.push(definition.clone());
        }
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
        Ok(())
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
    pub async fn start(&self, project: &str, service: &str, actor: &str) -> Result<Snapshot> {
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
        let cwd = inside(Path::new(&p.path), &definition.cwd)?;
        if matches!(definition.kind, ServiceKind::Compose { .. }) {
            return Err(AppError::new("COMPOSE_PENDING", "Docker Compose lifecycle support is not available in this build yet. Command services are available."));
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
        store.private.dirty = true;
        store.persist()?;
        let mut child = cmd.spawn().map_err(|_| {
            AppError::new(
                "PROCESS_START_FAILED",
                "Could not start the command. Check its working folder and shell.",
            )
        })?;
        let pid = child.id().ok_or_else(|| {
            AppError::new(
                "PROCESS_START_FAILED",
                "The process exited before its identity was available.",
            )
        })?;
        let run = uuid::Uuid::new_v4().to_string();
        let key = format!("{project}/{service}");
        let snapshot = Snapshot {
            state: "running".into(),
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
        self.logs.lock().await.push(
            &key,
            &run,
            "system",
            "Started service. Output is shared with the desktop and authorized AI clients.",
        );
        let stdout = child.stdout.take();
        let stderr = child.stderr.take();
        let a = stdout.map(|pipe| {
            tokio::spawn(drain(
                pipe,
                self.logs.clone(),
                key.clone(),
                run.clone(),
                "stdout",
            ))
        });
        let b = stderr.map(|pipe| {
            tokio::spawn(drain(
                pipe,
                self.logs.clone(),
                key.clone(),
                run.clone(),
                "stderr",
            ))
        });
        let has_health = definition.health_check.is_some();
        let manager = self.clone();
        let project_id = project.to_string();
        let service_id = service.to_string();
        let run_id = run.clone();
        tokio::spawn(async move {
            let result = child.wait().await;
            // The shell may exit before children. Clean the owned group before releasing this run.
            if group_alive(pid) {
                let _ = killpg(Pid::from_raw(pid as i32), Signal::SIGTERM);
                tokio::time::sleep(Duration::from_millis(150)).await;
                if group_alive(pid) {
                    let _ = killpg(Pid::from_raw(pid as i32), Signal::SIGKILL);
                }
            }
            if let Some(mut a) = a {
                if tokio::time::timeout(Duration::from_secs(2), &mut a)
                    .await
                    .is_err()
                {
                    a.abort();
                }
            }
            if let Some(mut b) = b {
                if tokio::time::timeout(Duration::from_secs(2), &mut b)
                    .await
                    .is_err()
                {
                    b.abort();
                }
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
                let stopped = current.state == "stopping";
                if group_alive(pid) {
                    current.state = "stopping".into();
                    current.error = Some("Owned child processes are still present after cleanup. Quit and new runs are blocked until cleanup can be verified.".into());
                    return;
                }
                current.state = if stopped || (code == Some(0) && !has_health) {
                    "stopped"
                } else {
                    "failed"
                }
                .into();
                current.exit_code = code;
                current.pid = None;
                if !stopped && current.state == "failed" {
                    current.error = Some(format!("Command exited {}. Check stderr and confirm the required tools are available in your login shell.", code.map(|n| format!("with code {n}")).unwrap_or_else(|| "after a signal".into())));
                }
                current.activity = format!(
                    "{} · {}",
                    if stopped { "Stopped" } else { "Process exited" },
                    chrono::Local::now().format("%H:%M:%S")
                );
            }
            drop(store);
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
    pub async fn stop(&self, project: &str, service: &str) -> Result<Snapshot> {
        let mut store = self.store.lock().await;
        let p = store
            .projects
            .get_mut(project)
            .ok_or_else(|| AppError::new("PROJECT_NOT_AUTHORIZED", "Project is not open."))?;
        let Some(current) = p.runtime.get_mut(service) else {
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
        for _ in 0..80 {
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
        for _ in 0..90 {
            let mut active = false;
            for (p, s) in &keys {
                active |= self.status(p, s).await?.active();
            }
            if !active {
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
        loop {
            let state = self.status(&project, &service).await.ok();
            if !state.is_some_and(|s| {
                s.run_id.as_deref() == Some(&run) && matches!(s.state.as_str(), "running" | "ready")
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
                    && matches!(current.state.as_str(), "running" | "ready")
                {
                    current.state = if healthy { "ready" } else { "running" }.into();
                    current.health = if healthy { "healthy" } else { "unhealthy" }.into();
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
) {
    let mut chunk = [0u8; 4096];
    let mut line = Vec::with_capacity(16384);
    loop {
        match pipe.read(&mut chunk).await {
            Ok(0) | Err(_) => break,
            Ok(n) => {
                for byte in &chunk[..n] {
                    if *byte == b'\n' || line.len() >= 16380 {
                        let text = String::from_utf8_lossy(&line).into_owned();
                        logs.lock().await.push(&key, &run, stream, &text);
                        line.clear();
                    }
                    if *byte != b'\n' {
                        line.push(*byte);
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
}
