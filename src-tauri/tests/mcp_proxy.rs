use serde_json::json;
use service_manager::{
    manager::Manager,
    mcp::AiBridge,
    model::*,
    proxy::{call, connect, Credentials},
};
use std::{os::unix::fs::PermissionsExt, time::Duration};

#[tokio::test]
async fn real_sdk_and_proxy_share_one_run_and_revocation_blocks_access() {
    let root = tempfile::tempdir().unwrap();
    let state = tempfile::tempdir().unwrap();
    let manager = Manager::new(state.path().into()).unwrap();
    let p = manager.open(root.path().to_str().unwrap()).await.unwrap();
    let def = ServiceDefinition {
        id: "web".into(),
        name: "Web".into(),
        cwd: ".".into(),
        kind: ServiceKind::Command {
            command: "printf 'shared-fixture-marker\\n'; sleep 60".into(),
        },
        env_file: None,
        health_check: None,
        ports: vec![],
        url: None,
        depends_on: vec![],
        vpn: None,
    };
    manager.save(&p.id, def, "absent", false).await.unwrap();
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    drop(listener);
    let bridge = AiBridge::new(manager.clone(), state.path().into(), port).unwrap();
    bridge.set_enabled(true).await.unwrap();
    let created = bridge
        .create(
            "test-client".into(),
            vec![],
            false,
            false,
            vec![],
            std::path::PathBuf::from(env!("CARGO_BIN_EXE_service-manager")),
        )
        .await
        .unwrap();
    assert!(bridge.authenticate_token("wrong-token").await.is_none());
    let client = connect(Credentials {
        endpoint: bridge.endpoint.clone(),
        token: created.token.clone(),
    })
    .await
    .unwrap();
    let tools = client.list_tools(None).await.unwrap().tools;
    for name in ["inspect_ports", "list_runs", "read_run", "diagnose_service"] {
        let tool = tools.iter().find(|tool| tool.name == name).unwrap();
        assert_eq!(
            serde_json::to_value(tool).unwrap()["annotations"]["readOnlyHint"],
            true
        );
    }
    let read_run = tools.iter().find(|tool| tool.name == "read_run").unwrap();
    assert!(
        serde_json::to_value(read_run).unwrap()["inputSchema"]["required"]
            .as_array()
            .unwrap()
            .contains(&json!("runId"))
    );
    let later_root = tempfile::tempdir().unwrap();
    let later = manager
        .open(later_root.path().to_str().unwrap())
        .await
        .unwrap();
    let list = call(&client, "list_projects", json!({})).await.unwrap();
    assert_eq!(list["projects"].as_array().unwrap().len(), 2);
    assert!(list["projects"]
        .as_array()
        .unwrap()
        .iter()
        .any(|p| p["id"] == later.id));

    let later_config = call(&client, "get_project_config", json!({"projectId":later.id}))
        .await
        .unwrap();
    let saved = call(&client, "upsert_service", json!({"projectId":later.id,"expectedRevision":later_config["revision"],"definition":{"id":"new","name":"New","cwd":".","type":"command","command":"printf later"}})).await.unwrap();
    assert_eq!(saved["config"]["services"][0]["id"], "new");
    assert!(call(&client, "upsert_service", json!({"projectId":later.id,"expectedRevision":"stale","definition":{"id":"new","name":"New","cwd":".","type":"command","command":"printf replacement"}})).await.is_err());
    let key = json!({"projectId":p.id,"serviceId":"web"});
    assert_eq!(
        call(&client, "inspect_ports", key.clone()).await.unwrap(),
        json!([])
    );
    assert_eq!(
        call(&client, "list_runs", key.clone()).await.unwrap(),
        json!([])
    );
    for name in ["inspect_ports", "list_runs", "read_run", "diagnose_service"] {
        let mut invalid_project = json!({"projectId":"unregistered","serviceId":"web"});
        let mut invalid_service = json!({"projectId":p.id,"serviceId":"missing"});
        if name == "read_run" {
            invalid_project["runId"] = json!("absent");
            invalid_service["runId"] = json!("absent");
        }
        assert_eq!(
            call(&client, name, invalid_project).await.unwrap_err().code,
            "PROJECT_NOT_AUTHORIZED"
        );
        assert_eq!(
            call(&client, name, invalid_service).await.unwrap_err().code,
            "SERVICE_NOT_FOUND"
        );
    }
    assert_eq!(
        call(&client, "read_run", key.clone())
            .await
            .unwrap_err()
            .code,
        "CONFIG_INVALID"
    );
    assert_eq!(
        call(
            &client,
            "read_run",
            json!({"projectId":p.id,"serviceId":"web","runId":"unknown-run"})
        )
        .await
        .unwrap_err()
        .code,
        "RUN_NOT_FOUND"
    );
    assert!(
        manager.status(&p.id, "web").await.unwrap().run_id.is_none(),
        "read-only tools must not create a run"
    );
    let first = call(&client, "start_service", key.clone()).await.unwrap();
    let run = first["runId"].as_str().unwrap().to_string();
    struct Cleanup(u32);
    impl Drop for Cleanup {
        fn drop(&mut self) {
            let _ = nix::sys::signal::killpg(
                nix::unistd::Pid::from_raw(self.0 as i32),
                nix::sys::signal::Signal::SIGKILL,
            );
        }
    }
    let _cleanup = Cleanup(first["pid"].as_u64().unwrap() as u32);
    let proxy = tokio::process::Command::new(&created.proxy_path)
        .args([
            "run",
            "--project",
            &p.id,
            "--service",
            "web",
            "--duration",
            "1",
        ])
        .output()
        .await
        .unwrap();
    assert!(
        proxy.status.success(),
        "proxy failed: {}",
        String::from_utf8_lossy(&proxy.stderr)
    );
    assert!(String::from_utf8_lossy(&proxy.stdout).contains("shared-fixture-marker"));
    assert_eq!(
        manager
            .status(&p.id, "web")
            .await
            .unwrap()
            .run_id
            .as_deref(),
        Some(run.as_str())
    );
    assert!(
        manager.status(&p.id, "web").await.unwrap().active(),
        "bounded proxy capture must not stop the service"
    );
    let diagnostic = call(&client, "diagnose_service", key.clone())
        .await
        .unwrap();
    assert_eq!(diagnostic["runId"], run);
    assert_eq!(diagnostic["redacted"], true);
    assert!(diagnostic["records"]
        .as_array()
        .unwrap()
        .iter()
        .any(|record| record["text"]
            .as_str()
            .unwrap_or_default()
            .contains("shared-fixture-marker")));
    let cred = std::path::Path::new(&created.proxy_path).with_file_name("credentials.json");
    assert_eq!(
        std::fs::metadata(cred).unwrap().permissions().mode() & 0o077,
        0
    );
    assert_eq!(
        call(
            &client,
            "upsert_service",
            json!({"projectId":p.id,"definition":{},"expectedRevision":"x"})
        )
        .await
        .unwrap_err()
        .code,
        "CONFIG_INVALID"
    );
    assert_eq!(
        call(
            &client,
            "read_logs",
            json!({"projectId":"ungranted","serviceId":"web"})
        )
        .await
        .unwrap_err()
        .code,
        "PROJECT_NOT_AUTHORIZED"
    );
    let restarted = call(&client, "restart_service", key.clone()).await.unwrap();
    assert_ne!(restarted["runId"].as_str(), Some(run.as_str()));
    let _restart_cleanup = Cleanup(restarted["pid"].as_u64().unwrap() as u32);
    let historical_key = json!({"projectId":p.id,"serviceId":"web","runId":run});
    let mut historical = json!(null);
    for _ in 0..80 {
        historical = call(&client, "read_run", historical_key.clone())
            .await
            .unwrap();
        if !historical["endedAt"].is_null() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    assert_eq!(historical["runId"], run);
    assert!(!historical["endedAt"].is_null());
    assert!(historical["records"]
        .as_array()
        .unwrap()
        .iter()
        .any(|record| record["text"]
            .as_str()
            .unwrap_or_default()
            .contains("shared-fixture-marker")));
    let history = call(&client, "list_runs", key.clone()).await.unwrap();
    let runs = history.as_array().unwrap();
    assert_eq!(runs.len(), 2);
    assert_eq!(runs[0]["runId"], restarted["runId"]);
    assert_eq!(runs[1]["runId"], run);
    assert_eq!(
        call(&client, "diagnose_service", key.clone())
            .await
            .unwrap()["runId"],
        restarted["runId"]
    );
    let http = reqwest::Client::builder().no_proxy().build().unwrap();
    let unauth = http
        .post(&bridge.endpoint)
        .json(&json!({}))
        .send()
        .await
        .unwrap();
    assert_eq!(unauth.status(), 401);
    let origin = http
        .post(&bridge.endpoint)
        .bearer_auth(&created.token)
        .header("Origin", "https://untrusted.invalid")
        .json(&json!({}))
        .send()
        .await
        .unwrap();
    assert_eq!(origin.status(), 403);
    bridge.revoke(&created.id).await.unwrap();
    assert!(call(&client, "list_projects", json!({})).await.is_err());
    for name in ["inspect_ports", "list_runs", "read_run", "diagnose_service"] {
        let args = if name == "read_run" {
            historical_key.clone()
        } else {
            key.clone()
        };
        assert!(
            call(&client, name, args).await.is_err(),
            "{name} must reject a revoked client"
        );
    }
    manager.stop(&p.id, "web").await.unwrap();
    for _ in 0..80 {
        if !manager.status(&p.id, "web").await.unwrap().active() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    assert!(!manager.status(&p.id, "web").await.unwrap().active());
    let _ = client.cancel().await;
    bridge.set_enabled(false).await.unwrap();
}
