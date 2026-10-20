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
                run_id: Some(uuid::Uuid::new_v4().to_string()),
                activity: format!("Compose preflight via {actor}"),
                ..Snapshot::default()
            };
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
            if let Some(current) = store
                .projects
                .get_mut(&project_id)
                .and_then(|p| p.runtime.get_mut(&service_id))
            {
                if current.run_id.as_deref() != Some(&run_id) {
                    return;
                }
                let stopped = current.state == "stopping";
                current.error = None;
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
    async fn run_compose(&self, job: crate::compose::ComposeJob) {
        use crate::compose::ComposeLaunch;
        let key = format!("{}/{}", job.project, job.service);
        let prepared = ComposeLaunch::prepare(
            &job.root,
            &job.cwd,
            &job.definition,
            &job.state_dir,
            &job.project,
            &job.owner,
        )
        .await;
        let launch = match prepared {
            Ok(launch) => launch,
            Err(error) => {
                if let Some(s) = self
                    .store
                    .lock()
                    .await
                    .projects
                    .get_mut(&job.project)
                    .and_then(|p| p.runtime.get_mut(&job.service))
                {
                    s.state = "failed".into();
                    s.error = Some(error.message);
                }
                return;
            }
        };
        // Stop may arrive while preflight is awaiting Docker.
        if self
            .status(&job.project, &job.service)
            .await
            .is_ok_and(|s| s.state == "stopping")
        {
            if let Some(s) = self
                .store
                .lock()
                .await
                .projects
                .get_mut(&job.project)
                .and_then(|p| p.runtime.get_mut(&job.service))
            {
                s.state = "stopped".into();
            }
            return;
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
                if let Some(s) = self
                    .store
                    .lock()
                    .await
                    .projects
                    .get_mut(&job.project)
                    .and_then(|p| p.runtime.get_mut(&job.service))
                {
                    s.state = "failed".into();
                    s.error = Some("Could not launch Docker Compose.".into());
                }
                return;
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
            )));
        }
        if let Some(pipe) = child.stderr.take() {
            readers.push(tokio::spawn(drain(
                pipe,
                self.logs.clone(),
                key.clone(),
                job.run.clone(),
                "stderr",
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
        loop {
            let current = match self.status(&job.project, &job.service).await {
                Ok(s) => s,
                Err(_) => break,
            };
            exited |= child.try_wait().ok().flatten().is_some();
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
                            break;
                        }
                        if exited && !group_alive(runner_pid) && state == "starting" {
                            s.state = "failed".into();
                            s.error = Some(
                                "Compose exited before the expected containers started.".into(),
                            );
                            break;
                        }
                        if state == "failed" {
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
                    && matches!(current.state.as_str(), "running" | "ready")
                {
                    current.state = if healthy { "ready" } else { "running" }.into();
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
) {
    let mut chunk = [0u8; 4096];
    let mut line = Vec::with_capacity(16384);
    loop {
        match pipe.read(&mut chunk).await {
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
            }
        }
    }
    if !line.is_empty() {
        logs.lock()
            .await
            .push(&key, &run, stream, &String::from_utf8_lossy(&line));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::AsyncWriteExt;
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
