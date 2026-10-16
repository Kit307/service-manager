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
    };
    manager.save(&p.id, def, "absent", false).await.unwrap();
    let mut store = manager.store.lock().await;
    let revision = store.refresh(&p.id).unwrap().revision;
    store.trust(&p.id, "web", &revision).unwrap();
    drop(store);
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    drop(listener);
    let bridge = AiBridge::new(manager.clone(), state.path().into(), port).unwrap();
    bridge.set_enabled(true).await.unwrap();
    let created = bridge
        .create(
            "test-client".into(),
            vec![p.id.clone()],
            true,
            false,
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
    let list = call(&client, "list_projects", json!({})).await.unwrap();
    assert_eq!(list["projects"][0]["id"], p.id);
    let key = json!({"projectId":p.id,"serviceId":"web"});
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
        "PERMISSION_DENIED"
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
