use crate::{
    history::{RunRecord, RunSummary},
    manager::Manager,
    model::*,
    ports::{self, PortListener},
    project::{Project, ServiceRef},
};
use serde_json::Value;
use std::{
    collections::{HashMap, HashSet},
    sync::atomic::Ordering,
    time::Duration,
};
fn key(r: &ServiceRef) -> String {
    format!("{}/{}", r.project_id, r.service_id)
}

#[derive(Clone, Copy)]
enum DependencyScope {
    All,
    Selected { require_external_ready: bool },
}

fn ordered(
    projects: &[Project],
    targets: &[ServiceRef],
    scope: DependencyScope,
) -> Result<Vec<ServiceRef>> {
    let definitions: HashMap<_, _> = projects
        .iter()
        .flat_map(|p| {
            p.config
                .services
                .iter()
                .map(move |s| (format!("{}/{}", p.id, s.id), (p, s)))
        })
        .collect();
    let selected: HashSet<_> = targets.iter().map(key).collect();
    if targets.len() > 256 || selected.len() != targets.len() {
        return Err(AppError::new(
            "CONFIG_INVALID",
            "Select up to 256 distinct services.",
        ));
    }
    fn visit(
        r: &ServiceRef,
        defs: &HashMap<String, (&Project, &ServiceDefinition)>,
        selected: &HashSet<String>,
        visiting: &mut HashSet<String>,
        done: &mut HashSet<String>,
        out: &mut Vec<ServiceRef>,
        scope: DependencyScope,
    ) -> Result<()> {
        let k = key(r);
        if done.contains(&k) {
            return Ok(());
        }
        let (p, s) = defs.get(&k).ok_or_else(|| {
            AppError::new(
                "DEPENDENCY_NOT_FOUND",
                "A selected service or dependency no longer exists.",
            )
        })?;
        if let DependencyScope::Selected {
            require_external_ready,
        } = scope
        {
            if !selected.contains(&k) {
                if !require_external_ready
                    || p.runtime.get(&s.id).is_some_and(|s| s.state == "ready")
                {
                    return Ok(());
                }
                return Err(AppError::new("DEPENDENCY_OUTSIDE_SELECTION","A dependency outside this selection is not Ready. Start it explicitly or include it in this group."));
            }
        }
        if !visiting.insert(k.clone()) {
            return Err(AppError::new(
                "DEPENDENCY_CYCLE",
                "Service dependencies contain a cycle.",
            ));
        }
        for dependency in &s.depends_on {
            visit(dependency, defs, selected, visiting, done, out, scope)?;
        }
        visiting.remove(&k);
        done.insert(k);
        out.push(r.clone());
        Ok(())
    }
    let mut out = vec![];
    let mut visiting = HashSet::new();
    let mut done = HashSet::new();
    for r in targets {
        visit(
            r,
            &definitions,
            &selected,
            &mut visiting,
            &mut done,
            &mut out,
            scope,
        )?;
    }
    Ok(out)
}
pub fn validate_dependencies(projects: &[Project], target: ServiceRef) -> Result<()> {
    ordered(projects, &[target], DependencyScope::All).map(|_| ())
}

impl Manager {
    pub async fn inspect_ports(&self, project: &str, service: &str) -> Result<Vec<PortListener>> {
        self.status(project, service).await?;
        let store = self.store.lock().await;
        let definition = store
            .projects
            .get(project)
            .and_then(|p| p.config.services.iter().find(|s| s.id == service))
            .cloned()
            .ok_or_else(|| AppError::new("SERVICE_NOT_FOUND", "Service does not exist."))?;
        let owned: Vec<_> = store
            .projects
            .values()
            .flat_map(|p| {
                p.runtime
                    .iter()
                    .filter(|(_, s)| s.active())
                    .filter_map(move |(id, s)| {
                        s.pid.map(|pid| {
                            (
                                pid,
                                ServiceRef {
                                    project_id: p.id.clone(),
                                    service_id: id.clone(),
                                },
                                s.run_id.clone(),
                            )
                        })
                    })
            })
            .collect();
        drop(store);
        let mut listeners = ports::inspect(&ports::configured_ports(&definition)).await?;
        for listener in &mut listeners {
            if let Some((pid, reference, run)) = owned
                .iter()
                .find(|(pid, _, _)| *pid == listener.pgid || *pid == listener.pid)
            {
                listener.managed = Some(reference.clone());
                listener.managed_run = run.clone();
                listener.managed_pid = Some(*pid);
            }
        }
        Ok(listeners)
    }
    pub async fn kill_port(
        &self,
        project: &str,
        service: &str,
        port: u16,
        pid: u32,
        identity: &str,
    ) -> Result<()> {
        let listener = self
            .inspect_ports(project, service)
            .await?
            .into_iter()
            .find(|l| {
                l.port == port && l.pid == pid && l.identity == identity && !identity.is_empty()
            })
            .ok_or_else(|| {
                AppError::new(
                    "PROCESS_CHANGED",
                    "The port owner changed. Refresh and confirm the current process again.",
                )
            })?;
        let (_, uid, _) = ports::process_identity(std::process::id()).await?;
        if pid <= 1 || listener.uid != uid || ports::protected(pid).await? {
            return Err(AppError::new(
                "PERMISSION_DENIED",
                "Refusing to terminate a protected process or another user’s process.",
            ));
        }
        if let Some(owned) = &listener.managed {
            let run = listener.managed_run.as_deref().ok_or_else(|| {
                AppError::new("PROCESS_CHANGED", "The managed run cannot be verified.")
            })?;
            let group_pid = listener.managed_pid.ok_or_else(|| {
                AppError::new("PROCESS_CHANGED", "The managed process cannot be verified.")
            })?;
            self.stop_confirmed(&owned.project_id, &owned.service_id, run, group_pid)
                .await?;
        } else {
            // Revalidate the captured start-time/owner immediately before signalling the PID.
            if ports::process_identity(pid).await?.0 != identity {
                return Err(AppError::new(
                    "PROCESS_CHANGED",
                    "The process identity changed. Refresh before retrying.",
                ));
            }
            nix::sys::signal::kill(
                nix::unistd::Pid::from_raw(pid as i32),
                nix::sys::signal::Signal::SIGTERM,
            )
            .map_err(|_| AppError::new("STOP_FAILED", "The process could not be terminated."))?;
        }
        let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
        while tokio::time::Instant::now() < deadline {
            if !self
                .inspect_ports(project, service)
                .await?
                .iter()
                .any(|l| l.port == port && l.pid == pid && l.identity == identity)
            {
                return Ok(());
            }
            tokio::time::sleep(Duration::from_millis(150)).await;
        }
        if listener.managed.is_none() {
            let current = self.inspect_ports(project, service).await?;
            if current
                .iter()
                .any(|l| l.port == port && l.pid == pid && l.identity == identity)
                && ports::process_identity(pid).await?.0 == identity
            {
                nix::sys::signal::kill(
                    nix::unistd::Pid::from_raw(pid as i32),
                    nix::sys::signal::Signal::SIGKILL,
                )
                .map_err(|_| {
                    AppError::new(
                        "STOP_FAILED",
                        "The same confirmed process could not be killed.",
                    )
                })?;
            }
        }
        for _ in 0..20 {
            if !self
                .inspect_ports(project, service)
                .await?
                .iter()
                .any(|l| l.port == port && l.pid == pid && l.identity == identity)
            {
                return Ok(());
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        Err(AppError::new(
            "STOP_FAILED",
            "Port release is not yet verified. Refresh before taking further action.",
        ))
    }
    pub fn cancel_sequence(&self) {
        self.sequence_generation.fetch_add(1, Ordering::SeqCst);
    }
    pub async fn control_sequence(
        &self,
        targets: Vec<ServiceRef>,
        action: &str,
        actor: &str,
    ) -> Result<()> {
        let _guard = self.sequence_lock.try_lock().map_err(|_| {
            AppError::new(
                "OPERATION_IN_PROGRESS",
                "Another group sequence is in progress.",
            )
        })?;
        let epoch = self.sequence_generation.load(Ordering::SeqCst);
        if !matches!(action, "start" | "stop" | "restart") {
            return Err(AppError::new("CONFIG_INVALID", "Unknown lifecycle action."));
        }
        if targets.len() > 256
            || targets.iter().map(key).collect::<HashSet<_>>().len() != targets.len()
        {
            return Err(AppError::new(
                "CONFIG_INVALID",
                "Select up to 256 distinct services.",
            ));
        }
        let projects = self.projects().await;
        for r in &targets {
            self.status(&r.project_id, &r.service_id).await?;
        }
        let order = match ordered(
            &projects,
            &targets,
            DependencyScope::Selected {
                require_external_ready: action != "stop",
            },
        ) {
            Ok(p) => p,
            Err(_) if action == "stop" => targets.clone(),
            Err(e) => return Err(e),
        };
        let check = || {
            if epoch == self.sequence_generation.load(Ordering::SeqCst) {
                Ok(())
            } else {
                Err(AppError::new(
                    "SEQUENCE_CANCELLED",
                    "Sequence cancelled; services already started remain running.",
                ))
            }
        };
        let prerequisites: HashSet<_> = order
            .iter()
            .filter_map(|r| {
                projects
                    .iter()
                    .find(|p| p.id == r.project_id)
                    .and_then(|p| p.config.services.iter().find(|s| s.id == r.service_id))
            })
            .flat_map(|s| s.depends_on.iter().map(key))
            .collect();
        if action != "stop" {
            for r in &order {
                let prerequisite = prerequisites.contains(&key(r));
                let d = projects
                    .iter()
                    .find(|p| p.id == r.project_id)
                    .and_then(|p| p.config.services.iter().find(|s| s.id == r.service_id))
                    .unwrap();
                if prerequisite
                    && d.health_check.is_none()
                    && matches!(d.kind, ServiceKind::Command { .. })
                {
                    return Err(AppError::new(
                        "DEPENDENCY_HEALTH_REQUIRED",
                        format!(
                            "Configure a health check for dependency {} before starting the group.",
                            d.name
                        ),
                    ));
                }
            }
        }
        if action != "start" {
            for r in order.iter().rev() {
                check()?;
                self.stop(&r.project_id, &r.service_id).await?;
                let deadline = tokio::time::Instant::now() + Duration::from_secs(60);
                while self.status(&r.project_id, &r.service_id).await?.active() {
                    check()?;
                    if tokio::time::Instant::now() >= deadline {
                        return Err(AppError::new(
                            "STOP_FAILED",
                            "A group service has not stopped.",
                        ));
                    }
                    tokio::time::sleep(Duration::from_millis(100)).await;
                }
            }
        }
        if action == "stop" {
            return Ok(());
        }
        for r in &order {
            check()?;
            self.start(&r.project_id, &r.service_id, actor).await?;
            let needed = prerequisites.contains(&key(r));
            if needed {
                let deadline = tokio::time::Instant::now() + Duration::from_secs(30);
                loop {
                    check()?;
                    let status = self.status(&r.project_id, &r.service_id).await?;
                    if status.state == "ready" {
                        break;
                    }
                    if !status.active() || tokio::time::Instant::now() >= deadline {
                        return Err(AppError::new("DEPENDENCY_NOT_READY",format!("Dependency {} is not Ready. Later services were not started; inspect its health and output.",r.service_id)));
                    }
                    tokio::time::sleep(Duration::from_millis(100)).await;
                }
            }
        }
        Ok(())
    }
    pub async fn run_history(&self, project: &str, service: &str) -> Result<Vec<RunSummary>> {
        self.status(project, service).await?;
        Ok(self.history.lock().await.list(project, service))
    }
    pub async fn run_details(&self, project: &str, service: &str, run: &str) -> Result<RunRecord> {
        self.status(project, service).await?;
        self.history.lock().await.get(project, service, run)
    }
    pub async fn diagnose(&self, project: &str, service: &str) -> Result<Value> {
        let snapshot = self.status(project, service).await?;
        let page = self.read_logs(project, service, None).await?;
        Ok(crate::history::redacted_diagnostics(
            project, service, &snapshot, page,
        ))
    }
    pub async fn checked_url(&self, project: &str, service: &str) -> Result<String> {
        let state = self.status(project, service).await?;
        if !matches!(state.state.as_str(), "starting" | "running" | "ready") {
            return Err(AppError::new(
                "URL_NOT_READY",
                "Start this service before opening its URL.",
            ));
        }
        let store = self.store.lock().await;
        let d = store
            .projects
            .get(project)
            .and_then(|p| p.config.services.iter().find(|s| s.id == service))
            .ok_or_else(|| AppError::new("SERVICE_NOT_FOUND", "Service does not exist."))?;
        let address = d.url.clone().ok_or_else(|| {
            AppError::new(
                "CONFIG_INVALID",
                "Configure an Open URL in service settings.",
            )
        })?;
        drop(store);
        let parsed = reqwest::Url::parse(&address)
            .map_err(|_| AppError::new("CONFIG_INVALID", "Invalid service URL."))?;
        if !matches!(parsed.scheme(), "http" | "https")
            || !matches!(parsed.host_str(), Some("localhost" | "127.0.0.1" | "[::1]"))
            || !parsed.username().is_empty()
            || parsed.password().is_some()
        {
            return Err(AppError::new(
                "CONFIG_INVALID",
                "Only loopback HTTP/HTTPS URLs without credentials can be opened.",
            ));
        }
        let port = parsed.port_or_known_default().unwrap_or(80);
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(2))
            .no_proxy()
            .redirect(reqwest::redirect::Policy::none())
            .resolve("localhost", ([127, 0, 0, 1], port).into())
            .build()
            .map_err(|_| AppError::new("URL_NOT_READY", "Could not initialize URL check."))?;
        let response =
            client.get(&address).send().await.map_err(|_| {
                AppError::new("URL_NOT_READY", "The service URL is not reachable yet.")
            })?;
        if !response.status().is_success() && !response.status().is_redirection() {
            return Err(AppError::new(
                "URL_NOT_READY",
                format!(
                    "The service URL returned HTTP {}.",
                    response.status().as_u16()
                ),
            ));
        }
        Ok(address)
    }
    pub(crate) async fn finish_history(&self, project: &str, service: &str, snapshot: &Snapshot) {
        if snapshot.active() {
            return;
        }
        if let Some(run) = &snapshot.run_id {
            let page = self
                .logs
                .lock()
                .await
                .read_run(&format!("{project}/{service}"), run);
            // Release history before acquiring store; start takes store before history.
            let result = {
                self.history
                    .lock()
                    .await
                    .finish(project, service, run, snapshot, page)
            };
            if let Err(error) = result {
                if let Some(current) = self
                    .store
                    .lock()
                    .await
                    .projects
                    .get_mut(project)
                    .and_then(|p| p.runtime.get_mut(service))
                {
                    if current.run_id == snapshot.run_id {
                        current.error = Some(error.message);
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn cancellation_during_preflight_prevents_any_start() {
        let root = tempfile::tempdir().unwrap();
        let state = tempfile::tempdir().unwrap();
        let manager = Manager::new(state.path().into()).unwrap();
        let project = manager.open(root.path().to_str().unwrap()).await.unwrap();
        let definition: ServiceDefinition = serde_json::from_value(serde_json::json!({
            "id":"fixture", "name":"fixture", "cwd":".", "type":"command", "command":"sleep 10"
        }))
        .unwrap();
        manager
            .save(&project.id, definition, "absent", true)
            .await
            .unwrap();
        let target = ServiceRef {
            project_id: project.id.clone(),
            service_id: "fixture".into(),
        };
        let held_store = manager.store.lock().await;
        let controller = manager.clone();
        let sequence = tokio::spawn(async move {
            controller
                .control_sequence(vec![target], "start", "test")
                .await
        });
        // Block preflight until the sequence owns its lock, then cancel before its first store read.
        tokio::time::timeout(Duration::from_secs(2), async {
            loop {
                if manager.sequence_lock.try_lock().is_err() {
                    break;
                }
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
        manager.cancel_sequence();
        drop(held_store);
        assert_eq!(
            sequence.await.unwrap().unwrap_err().code,
            "SEQUENCE_CANCELLED"
        );
        assert!(manager
            .status(&project.id, "fixture")
            .await
            .unwrap()
            .run_id
            .is_none());
        assert!(manager
            .run_history(&project.id, "fixture")
            .await
            .unwrap()
            .is_empty());
        manager.shutdown().await.unwrap();
    }

    #[tokio::test]
    async fn stop_order_ignores_unselected_dependency_readiness() {
        let root = tempfile::tempdir().unwrap();
        let state = tempfile::tempdir().unwrap();
        let manager = Manager::new(state.path().into()).unwrap();
        let p = manager.open(root.path().to_str().unwrap()).await.unwrap();
        let external = ServiceRef {
            project_id: p.id.clone(),
            service_id: "external".into(),
        };
        let db = ServiceRef {
            project_id: p.id.clone(),
            service_id: "db".into(),
        };
        let api = ServiceRef {
            project_id: p.id.clone(),
            service_id: "api".into(),
        };
        for (id, dependencies) in [
            ("external", vec![]),
            ("db", vec![external]),
            ("api", vec![db.clone()]),
        ] {
            let definition = serde_json::from_value(serde_json::json!({
                "id":id,"name":id,"cwd":".","type":"command","command":"sleep 10","dependsOn":dependencies
            })).unwrap();
            let revision = manager.store.lock().await.refresh(&p.id).unwrap().revision;
            manager
                .save(&p.id, definition, &revision, true)
                .await
                .unwrap();
        }
        let projects = manager.projects().await;
        let order = ordered(
            &projects,
            &[api.clone(), db.clone()],
            DependencyScope::Selected {
                require_external_ready: false,
            },
        )
        .unwrap();
        assert_eq!(
            order
                .iter()
                .rev()
                .map(|r| r.service_id.as_str())
                .collect::<Vec<_>>(),
            vec!["api", "db"]
        );
        assert_eq!(
            ordered(
                &projects,
                &[api, db],
                DependencyScope::Selected {
                    require_external_ready: true
                }
            )
            .err()
            .unwrap()
            .code,
            "DEPENDENCY_OUTSIDE_SELECTION"
        );
    }

    #[tokio::test]
    async fn confirmed_stop_rejects_previous_run_and_pid() {
        let root = tempfile::tempdir().unwrap();
        let state = tempfile::tempdir().unwrap();
        let manager = Manager::new(state.path().into()).unwrap();
        let project = manager.open(root.path().to_str().unwrap()).await.unwrap();
        let definition: ServiceDefinition = serde_json::from_value(serde_json::json!({
            "id":"fixture", "name":"fixture", "cwd":".", "type":"command", "command":"sleep 10"
        }))
        .unwrap();
        manager
            .save(&project.id, definition, "absent", true)
            .await
            .unwrap();
        let old = manager.start(&project.id, "fixture", "test").await.unwrap();
        manager.stop(&project.id, "fixture").await.unwrap();
        for _ in 0..100 {
            if !manager
                .status(&project.id, "fixture")
                .await
                .unwrap()
                .active()
            {
                break;
            }
            tokio::time::sleep(Duration::from_millis(30)).await;
        }
        let current = manager.start(&project.id, "fixture", "test").await.unwrap();
        assert_ne!(old.run_id, current.run_id);
        let error = manager
            .stop_confirmed(
                &project.id,
                "fixture",
                old.run_id.as_deref().unwrap(),
                old.pid.unwrap(),
            )
            .await
            .unwrap_err();
        assert_eq!(error.code, "PROCESS_CHANGED");
        assert_eq!(
            manager.status(&project.id, "fixture").await.unwrap().run_id,
            current.run_id
        );
        assert!(manager
            .status(&project.id, "fixture")
            .await
            .unwrap()
            .active());
        manager.shutdown().await.unwrap();
    }
}
