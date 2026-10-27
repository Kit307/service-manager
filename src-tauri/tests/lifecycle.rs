use service_manager::{manager::Manager, model::*, project::ProjectStore};
#[tokio::test]
async fn trusted_service_starts_once_and_collects_output() {
    let root = tempfile::tempdir().unwrap();
    let state = tempfile::tempdir().unwrap();
    let mut store = ProjectStore::new(state.path().into()).unwrap();
    let p = store.open(root.path().to_str().unwrap()).unwrap();
    let definition = ServiceDefinition {
        id: "web".into(),
        name: "Web".into(),
        cwd: ".".into(),
        kind: ServiceKind::Command {
            command: "printf 'fixture-ready\\n'; sleep 60".into(),
        },
        env_file: None,
        health_check: None,
        ports: vec![],
        url: None,
        depends_on: vec![],
        vpn: None,
        vpn_routes: vec![],
    };
    store
        .save(
            &p.id,
            ProjectConfig {
                schema_version: 1,
                services: vec![definition],
            },
            "absent",
        )
        .unwrap();
    let revision = store.refresh(&p.id).unwrap().revision;
    store.trust(&p.id, "web", &revision).unwrap();
    drop(store);
    let manager = Manager::new(state.path().into()).unwrap();
    let (a, b) = tokio::join!(
        manager.start(&p.id, "web", "test"),
        manager.start(&p.id, "web", "test")
    );
    let first = a.unwrap();
    let second = b.unwrap();
    struct Cleanup(u32);
    impl Drop for Cleanup {
        fn drop(&mut self) {
            let _ = nix::sys::signal::killpg(
                nix::unistd::Pid::from_raw(self.0 as i32),
                nix::sys::signal::Signal::SIGKILL,
            );
        }
    }
    let _cleanup = Cleanup(first.pid.unwrap());
    assert_eq!(first.run_id, second.run_id);
    assert_eq!(
        manager.remove_project(&p.id).await.unwrap_err().code,
        "OPERATION_IN_PROGRESS"
    );
    let mut found = false;
    for _ in 0..40 {
        if manager
            .read_logs(&p.id, "web", None)
            .await
            .unwrap()
            .records
            .iter()
            .any(|l| l.text == "fixture-ready")
        {
            found = true;
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
    manager.stop(&p.id, "web").await.unwrap();
    for _ in 0..80 {
        if !manager.status(&p.id, "web").await.unwrap().active() {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
    assert!(!manager.status(&p.id, "web").await.unwrap().active());
    assert!(
        found,
        "the real process output must reach the shared log store"
    );
    manager.remove_project(&p.id).await.unwrap();
    assert!(manager.projects().await.is_empty());
    assert_eq!(
        manager
            .start(&p.id, "web", "test")
            .await
            .err()
            .unwrap()
            .code,
        "PROJECT_NOT_AUTHORIZED"
    );
}

#[tokio::test]
async fn shutdown_blocks_new_runs_and_escalates_a_process_that_ignores_sigterm() {
    let root = tempfile::tempdir().unwrap();
    let state = tempfile::tempdir().unwrap();
    let manager = Manager::new(state.path().into()).unwrap();
    let p = manager.open(root.path().to_str().unwrap()).await.unwrap();
    let definition = ServiceDefinition {
        id: "slow".into(),
        name: "Slow stop fixture".into(),
        cwd: ".".into(),
        kind: ServiceKind::Command {
            command: "trap '' TERM; printf 'ignore-term-ready\\n'; sleep 60".into(),
        },
        env_file: None,
        health_check: None,
        ports: vec![],
        url: None,
        depends_on: vec![],
        vpn: None,
        vpn_routes: vec![],
    };
    let saved = manager
        .save(&p.id, definition, "absent", false)
        .await
        .unwrap();
    manager
        .store
        .lock()
        .await
        .trust(&p.id, "slow", &saved.revision)
        .unwrap();
    let first = manager.start(&p.id, "slow", "test").await.unwrap();
    struct Cleanup(u32);
    impl Drop for Cleanup {
        fn drop(&mut self) {
            let _ = nix::sys::signal::killpg(
                nix::unistd::Pid::from_raw(self.0 as i32),
                nix::sys::signal::Signal::SIGKILL,
            );
        }
    }
    let _cleanup = Cleanup(first.pid.unwrap());
    for _ in 0..40 {
        if manager
            .read_logs(&p.id, "slow", None)
            .await
            .unwrap()
            .records
            .iter()
            .any(|l| l.text == "ignore-term-ready")
        {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
    let closing = manager.clone();
    let task = tokio::spawn(async move { closing.shutdown().await });
    for _ in 0..40 {
        if manager.status(&p.id, "slow").await.unwrap().state == "stopping" {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(25)).await;
    }
    assert_eq!(
        manager
            .start(&p.id, "slow", "test")
            .await
            .err()
            .unwrap()
            .code,
        "OPERATION_IN_PROGRESS"
    );
    tokio::time::timeout(std::time::Duration::from_secs(10), task)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    assert!(!manager.status(&p.id, "slow").await.unwrap().active());
    assert!(
        nix::sys::signal::killpg(nix::unistd::Pid::from_raw(first.pid.unwrap() as i32), None)
            .is_err()
    );
}

#[tokio::test]
async fn readiness_requires_an_actual_successful_loopback_response() {
    let available = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = available.local_addr().unwrap().port();
    drop(available);
    let root = tempfile::tempdir().unwrap();
    std::fs::write(
        root.path().join("health_fixture.py"),
        r#"import http.server
import pathlib
import signal
import sys

signal.alarm(20)
class Health(http.server.BaseHTTPRequestHandler):
    def do_GET(self):
        status = 200 if pathlib.Path("ready").exists() else 503
        self.send_response(status)
        self.end_headers()
        self.wfile.write(b"fixture")
        print(f"health-status-{status}", flush=True)

http.server.HTTPServer(("127.0.0.1", int(sys.argv[1])), Health).serve_forever()
"#,
    )
    .unwrap();
    let state = tempfile::tempdir().unwrap();
    let manager = Manager::new(state.path().into()).unwrap();
    let p = manager.open(root.path().to_str().unwrap()).await.unwrap();
    let definition = ServiceDefinition {
        id: "health".into(),
        name: "Health fixture".into(),
        cwd: ".".into(),
        kind: ServiceKind::Command {
            command: format!("exec /usr/bin/python3 -u health_fixture.py {port}"),
        },
        env_file: None,
        ports: vec![],
        url: None,
        depends_on: vec![],
        vpn: None,
        vpn_routes: vec![],
        health_check: Some(HealthCheck {
            url: format!("http://127.0.0.1:{port}/health"),
        }),
    };
    let saved = manager
        .save(&p.id, definition, "absent", false)
        .await
        .unwrap();
    manager
        .store
        .lock()
        .await
        .trust(&p.id, "health", &saved.revision)
        .unwrap();
    let initial = manager.start(&p.id, "health", "test").await.unwrap();
    struct Cleanup(u32);
    impl Drop for Cleanup {
        fn drop(&mut self) {
            let _ = nix::sys::signal::killpg(
                nix::unistd::Pid::from_raw(self.0 as i32),
                nix::sys::signal::Signal::SIGKILL,
            );
        }
    }
    let _cleanup = Cleanup(initial.pid.unwrap());
    assert_eq!(initial.state, "starting");
    let mut served_unhealthy = false;
    for _ in 0..80 {
        if manager
            .read_logs(&p.id, "health", None)
            .await
            .unwrap()
            .records
            .iter()
            .any(|line| line.text == "health-status-503")
        {
            served_unhealthy = true;
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
    assert!(
        served_unhealthy,
        "the service must actually serve an unsuccessful health response"
    );
    assert_eq!(
        manager.status(&p.id, "health").await.unwrap().state,
        "starting"
    );
    std::fs::write(root.path().join("ready"), "ready").unwrap();
    let mut ready = false;
    for _ in 0..80 {
        if manager.status(&p.id, "health").await.unwrap().state == "ready" {
            ready = true;
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
    manager.shutdown().await.unwrap();
    assert!(
        ready,
        "running must become ready only after the real HTTP response"
    );
}
