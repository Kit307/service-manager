use service_manager::{manager::Manager, model::*, project::ServiceRef};
use std::time::Duration;

async fn fixture() -> (tempfile::TempDir, tempfile::TempDir, Manager, String) {
    let root = tempfile::tempdir().unwrap();
    let state = tempfile::tempdir().unwrap();
    let manager = Manager::new(state.path().into()).unwrap();
    let p = manager.open(root.path().to_str().unwrap()).await.unwrap();
    (root, state, manager, p.id)
}
fn definition(id: &str) -> ServiceDefinition {
    serde_json::from_value(
        serde_json::json!({"id":id,"name":id,"cwd":".","type":"command","command":"sleep 30"}),
    )
    .unwrap()
}
async fn save(m: &Manager, p: &str, d: ServiceDefinition) {
    let rev = m.store.lock().await.refresh(p).unwrap().revision;
    m.save(p, d, &rev, true).await.unwrap();
}
#[tokio::test]
async fn occupied_port_prevents_spawn_and_stale_kill_is_rejected() {
    let (_root, _state, m, p) = fixture().await;
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let mut d = definition("web");
    d.ports = vec![port];
    save(&m, &p, d).await;
    assert_eq!(
        m.start(&p, "web", "test").await.unwrap_err().code,
        "PORT_IN_USE"
    );
    assert!(!m.status(&p, "web").await.unwrap().active());
    let found = m.inspect_ports(&p, "web").await.unwrap();
    assert!(found.iter().any(|l| l.port == port));
    assert_eq!(
        m.kill_port(&p, "web", port, std::process::id(), "stale")
            .await
            .unwrap_err()
            .code,
        "PROCESS_CHANGED"
    );
    drop(listener);
    assert!(m.inspect_ports(&p, "web").await.unwrap().is_empty());
}
#[tokio::test]
async fn group_rejects_cycles_before_start_and_dependency_requires_ready() {
    let (_root, _state, m, p) = fixture().await;
    let a = definition("db");
    save(&m, &p, a).await;
    let mut b = definition("api");
    b.depends_on = vec![ServiceRef {
        project_id: p.clone(),
        service_id: "db".into(),
    }];
    save(&m, &p, b).await;
    assert_eq!(
        m.start(&p, "api", "test").await.unwrap_err().code,
        "DEPENDENCY_NOT_READY"
    );
    let mut a = definition("db");
    a.depends_on = vec![ServiceRef {
        project_id: p.clone(),
        service_id: "api".into(),
    }];
    let rev = m.store.lock().await.refresh(&p).unwrap().revision;
    assert_eq!(
        m.save(&p, a, &rev, true).await.err().unwrap().code,
        "DEPENDENCY_CYCLE"
    );
    assert!(!m.status(&p, "db").await.unwrap().active());
}
#[tokio::test]
async fn command_history_survives_reopening_without_env_contents() {
    let (_root, state, m, p) = fixture().await;
    let mut d = definition("once");
    d.kind = ServiceKind::Command {
        command: "printf 'history-fixture\\n'".into(),
    };
    save(&m, &p, d).await;
    let run = m.start(&p, "once", "test").await.unwrap().run_id.unwrap();
    for _ in 0..100 {
        if !m.run_history(&p, "once").await.unwrap().is_empty()
            && m.run_history(&p, "once").await.unwrap()[0]
                .ended_at
                .is_some()
        {
            break;
        }
        tokio::time::sleep(Duration::from_millis(30)).await;
    }
    let record = m.run_details(&p, "once", &run).await.unwrap();
    assert_eq!(record.summary.state, "stopped");
    assert!(record.records.iter().any(|r| r.text == "history-fixture"));
    drop(m);
    let reopened = Manager::new(state.path().into()).unwrap();
    assert_eq!(reopened.run_history(&p, "once").await.unwrap().len(), 1);
}
#[test]
fn legacy_definition_keeps_its_serialized_shape() {
    let old = serde_json::json!({"id":"web","name":"web","cwd":".","type":"command","command":"sleep 30"});
    let definition: ServiceDefinition = serde_json::from_value(old.clone()).unwrap();
    assert_eq!(serde_json::to_value(definition).unwrap(), old);
}

fn reference(project: &str, service: &str) -> ServiceRef {
    ServiceRef {
        project_id: project.into(),
        service_id: service.into(),
    }
}
fn free_port() -> u16 {
    std::net::TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}
fn web_definition(id: &str, port: u16) -> ServiceDefinition {
    let mut d = definition(id);
    // Alarm bounds the disposable fixture lifetime even if an assertion fails.
    d.kind = ServiceKind::Command { command: format!("exec /usr/bin/python3 -c 'import signal,http.server; signal.alarm(15); http.server.HTTPServer((\"127.0.0.1\",{port}),http.server.SimpleHTTPRequestHandler).serve_forever()'") };
    d.health_check = Some(HealthCheck {
        url: format!("http://127.0.0.1:{port}/"),
    });
    d.url = Some(format!("http://127.0.0.1:{port}/"));
    d
}
async fn wait_terminal(m: &Manager, p: &str, service: &str) {
    for _ in 0..100 {
        if !m.status(p, service).await.unwrap().active() {
            return;
        }
        tokio::time::sleep(Duration::from_millis(30)).await;
    }
    panic!("fixture did not reach terminal state");
}

#[tokio::test]
async fn confirmed_external_listener_can_be_killed_but_self_is_protected() {
    let (_root, _state, m, p) = fixture().await;
    let port = free_port();
    let mut d = definition("web");
    d.ports = vec![port];
    save(&m, &p, d).await;
    let mut child=std::process::Command::new("/usr/bin/python3")
        .args(["-c",&format!("import signal,socket,time; signal.alarm(15); s=socket.socket(); s.bind(('127.0.0.1',{port})); s.listen(); time.sleep(15)")])
        .stdin(std::process::Stdio::null()).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null()).spawn().unwrap();
    let mut found = None;
    for _ in 0..40 {
        found = m
            .inspect_ports(&p, "web")
            .await
            .unwrap()
            .into_iter()
            .find(|l| l.pid == child.id());
        if found.is_some() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(30)).await;
    }
    let listener = found.expect("external fixture listened");
    assert!(listener.managed.is_none());
    assert_eq!(
        m.kill_port(&p, "web", port, child.id(), "stale")
            .await
            .unwrap_err()
            .code,
        "PROCESS_CHANGED"
    );
    assert!(child.try_wait().unwrap().is_none());
    m.kill_port(&p, "web", port, listener.pid, &listener.identity)
        .await
        .unwrap();
    assert!(!child.wait().unwrap().success());
    assert!(m.inspect_ports(&p, "web").await.unwrap().is_empty());
    let socket = std::net::TcpListener::bind(("127.0.0.1", port)).unwrap();
    let own = m
        .inspect_ports(&p, "web")
        .await
        .unwrap()
        .into_iter()
        .find(|l| l.pid == std::process::id())
        .unwrap();
    assert_eq!(
        m.kill_port(&p, "web", port, own.pid, &own.identity)
            .await
            .unwrap_err()
            .code,
        "PERMISSION_DENIED"
    );
    drop(socket);
}

#[tokio::test]
async fn sequence_waits_for_ready_and_checked_url_requires_reachability() {
    let (_root, _state, m, p) = fixture().await;
    let port = free_port();
    save(&m, &p, web_definition("db", port)).await;
    let mut api = definition("api");
    api.kind = ServiceKind::Command {
        command: "sleep 10".into(),
    };
    api.depends_on = vec![reference(&p, "db")];
    save(&m, &p, api).await;
    assert_eq!(
        m.checked_url(&p, "db").await.unwrap_err().code,
        "URL_NOT_READY"
    );
    m.control_sequence(
        vec![reference(&p, "api"), reference(&p, "db")],
        "start",
        "test",
    )
    .await
    .unwrap();
    assert_eq!(m.status(&p, "db").await.unwrap().state, "ready");
    assert!(m.status(&p, "api").await.unwrap().active());
    assert_eq!(
        m.checked_url(&p, "db").await.unwrap(),
        format!("http://127.0.0.1:{port}/")
    );
    let previous = m.status(&p, "db").await.unwrap().run_id;
    m.control_sequence(
        vec![reference(&p, "api"), reference(&p, "db")],
        "restart",
        "test",
    )
    .await
    .unwrap();
    assert_ne!(m.status(&p, "db").await.unwrap().run_id, previous);
    assert_eq!(m.status(&p, "db").await.unwrap().state, "ready");
    let owner = m
        .inspect_ports(&p, "db")
        .await
        .unwrap()
        .into_iter()
        .find(|l| l.port == port)
        .unwrap();
    assert_eq!(owner.managed.as_ref().unwrap().service_id, "db");
    m.kill_port(&p, "db", port, owner.pid, &owner.identity)
        .await
        .unwrap();
    wait_terminal(&m, &p, "db").await;
    assert_eq!(
        m.checked_url(&p, "db").await.unwrap_err().code,
        "URL_NOT_READY"
    );
    m.shutdown().await.unwrap();
}

#[tokio::test]
async fn failed_prerequisite_does_not_start_dependent() {
    let (_root, _state, m, p) = fixture().await;
    let mut db = web_definition("db", free_port());
    db.kind = ServiceKind::Command {
        command: "exit 7".into(),
    };
    save(&m, &p, db).await;
    let mut api = definition("api");
    api.depends_on = vec![reference(&p, "db")];
    save(&m, &p, api).await;
    assert_eq!(
        m.control_sequence(
            vec![reference(&p, "api"), reference(&p, "db")],
            "start",
            "test"
        )
        .await
        .unwrap_err()
        .code,
        "DEPENDENCY_NOT_READY"
    );
    assert!(m.status(&p, "api").await.unwrap().run_id.is_none());
    m.shutdown().await.unwrap();
}

#[tokio::test]
async fn cancellation_prevents_later_starts_and_retains_current_run() {
    let (_root, _state, m, p) = fixture().await;
    let mut db = web_definition("db", free_port());
    db.kind = ServiceKind::Command {
        command: "sleep 10".into(),
    };
    save(&m, &p, db).await;
    let mut api = definition("api");
    api.depends_on = vec![reference(&p, "db")];
    save(&m, &p, api).await;
    let controller = m.clone();
    let project = p.clone();
    let running = tokio::spawn(async move {
        controller
            .control_sequence(
                vec![reference(&project, "api"), reference(&project, "db")],
                "start",
                "test",
            )
            .await
    });
    for _ in 0..100 {
        if m.status(&p, "db").await.unwrap().active() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    assert!(m.status(&p, "db").await.unwrap().active());
    assert_eq!(
        m.checked_url(&p, "db").await.unwrap_err().code,
        "URL_NOT_READY"
    );
    m.cancel_sequence();
    assert_eq!(
        running.await.unwrap().unwrap_err().code,
        "SEQUENCE_CANCELLED"
    );
    assert!(m.status(&p, "db").await.unwrap().active());
    assert!(m.status(&p, "api").await.unwrap().run_id.is_none());
    m.shutdown().await.unwrap();
}

#[tokio::test]
async fn unrelated_dependents_do_not_require_health_for_selected_service() {
    let (_root, _state, m, p) = fixture().await;
    let mut db = definition("db");
    db.kind = ServiceKind::Command {
        command: "sleep 10".into(),
    };
    save(&m, &p, db).await;
    let mut api = definition("api");
    api.depends_on = vec![reference(&p, "db")];
    save(&m, &p, api).await;
    m.control_sequence(vec![reference(&p, "db")], "start", "test")
        .await
        .unwrap();
    assert!(m.status(&p, "db").await.unwrap().active());
    assert_eq!(
        m.control_sequence(
            vec![reference(&p, "db"), reference(&p, "db")],
            "stop",
            "test"
        )
        .await
        .unwrap_err()
        .code,
        "CONFIG_INVALID"
    );
    assert!(m.status(&p, "db").await.unwrap().active());
    m.shutdown().await.unwrap();
}
