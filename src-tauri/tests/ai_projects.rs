use serde_json::json;
use service_manager::{
    manager::Manager,
    mcp::AiBridge,
    project::{hash, secure_write},
    proxy::{call, connect, Credentials},
};

#[tokio::test]
async fn all_tokens_create_and_open_projects_without_folder_grants() {
    let root = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    let state = tempfile::tempdir().unwrap();
    let root_path = root.path().canonicalize().unwrap();
    let existing = root_path.join("existing");
    std::fs::create_dir(&existing).unwrap();
    std::fs::write(existing.join("source.txt"), "keep this").unwrap();
    std::os::unix::fs::symlink(outside.path(), root_path.join("escape")).unwrap();
    secure_write(&state.path().join("clients.json"), &serde_json::to_vec(&json!([
        {"id":"builder","name":"Builder","projects":[],"projectRoots":[root_path],"control":false,"edit":true,"tokenHash":hash(b"fixture-token")},
        {"id":"legacy","name":"Legacy","projects":[],"control":false,"edit":false,"tokenHash":hash(b"legacy-token")}
    ])).unwrap()).unwrap();
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    drop(listener);
    let manager = Manager::new(state.path().into()).unwrap();
    let bridge = AiBridge::new(manager.clone(), state.path().into(), port).unwrap();
    bridge.set_enabled(true).await.unwrap();
    let client = connect(Credentials {
        endpoint: bridge.endpoint.clone(),
        token: "fixture-token".into(),
    })
    .await
    .unwrap();
    let tools = client.list_tools(None).await.unwrap().tools;
    for name in ["list_project_roots", "create_project", "open_project"] {
        assert!(tools.iter().any(|tool| tool.name == name));
    }
    let legacy = connect(Credentials {
        endpoint: bridge.endpoint.clone(),
        token: "legacy-token".into(),
    })
    .await
    .unwrap();
    let roots = call(&client, "list_project_roots", json!({}))
        .await
        .unwrap();
    assert_eq!(roots["roots"], json!([{"path":"/"}]));
    assert_eq!(
        call(&legacy, "list_project_roots", json!({}))
            .await
            .unwrap(),
        roots
    );
    assert_eq!(bridge.status().await["clients"][1]["scope"], "filesystem");
    assert!(call(
        &legacy,
        "create_project",
        json!({"root":"relative-root","name":"invalid"})
    )
    .await
    .is_err());
    for name in ["../escape", "/absolute", ".", "a/b", ""] {
        assert!(call(
            &client,
            "create_project",
            json!({"root":root_path,"name":name})
        )
        .await
        .is_err());
    }
    for relative in ["../", "/", "escape", "existing/source.txt"] {
        assert!(call(
            &client,
            "open_project",
            json!({"root":root_path,"relativePath":relative})
        )
        .await
        .is_err());
    }
    assert!(manager.projects().await.is_empty());
    let legacy_created = call(
        &legacy,
        "create_project",
        json!({"root":outside.path(),"name":"legacy-project"}),
    )
    .await
    .unwrap();
    assert!(outside.path().join("legacy-project").is_dir());
    let legacy_opened = call(
        &legacy,
        "open_project",
        json!({"root":outside.path(),"relativePath":"legacy-project"}),
    )
    .await
    .unwrap();
    assert_eq!(legacy_opened["id"], legacy_created["id"]);
    // Absolute parent aliases are normalized before the per-request path guard.
    let alias = root_path.join("parent-alias");
    std::os::unix::fs::symlink(outside.path(), &alias).unwrap();
    assert_eq!(
        call(
            &legacy,
            "open_project",
            json!({"root":alias,"relativePath":"legacy-project"})
        )
        .await
        .unwrap()["id"],
        legacy_created["id"]
    );
    let created = call(
        &client,
        "create_project",
        json!({"root":root_path,"name":"new-project"}),
    )
    .await
    .unwrap();
    let id = created["id"].as_str().unwrap();
    assert!(root_path.join("new-project").is_dir());
    assert!(!root_path.join("new-project/.service-manager").exists());
    assert!(created["runtime"].as_object().unwrap().is_empty());
    assert!(call(
        &client,
        "create_project",
        json!({"root":root_path,"name":"new-project"})
    )
    .await
    .is_err());
    let opened = call(
        &client,
        "open_project",
        json!({"root":root_path,"relativePath":"existing"}),
    )
    .await
    .unwrap();
    assert_eq!(
        std::fs::read_to_string(existing.join("source.txt")).unwrap(),
        "keep this"
    );
    assert_eq!(manager.projects().await.len(), 3);
    assert_eq!(
        call(&client, "list_projects", json!({})).await.unwrap()["projects"]
            .as_array()
            .unwrap()
            .len(),
        3
    );
    assert_eq!(
        call(&legacy, "list_projects", json!({})).await.unwrap()["projects"]
            .as_array()
            .unwrap()
            .len(),
        3
    );
    let saved = call(&legacy,"upsert_service",json!({"projectId":id,"expectedRevision":created["revision"],"definition":{"id":"web","name":"Web","cwd":".","type":"command","command":"printf marker"}})).await.unwrap();
    assert_eq!(saved["config"]["services"][0]["id"], "web");
    assert_eq!(manager.status(id, "web").await.unwrap().state, "stopped");
    assert!(call(
        &client,
        "start_service",
        json!({"projectId":id,"serviceId":"web"})
    )
    .await
    .is_ok());
    let reopened = call(
        &client,
        "open_project",
        json!({"root":root_path,"relativePath":"existing"}),
    )
    .await
    .unwrap();
    assert_eq!(reopened["id"], opened["id"]);
    let fresh = bridge
        .create(
            "Fresh token".into(),
            vec![],
            false,
            false,
            vec![],
            std::path::PathBuf::from(env!("CARGO_BIN_EXE_service-manager")),
        )
        .await
        .unwrap();
    let fresh_client = connect(Credentials {
        endpoint: bridge.endpoint.clone(),
        token: fresh.token.clone(),
    })
    .await
    .unwrap();
    let fresh_project = call(
        &fresh_client,
        "create_project",
        json!({"root":outside.path(),"name":"fresh-project"}),
    )
    .await
    .unwrap();
    assert_eq!(
        call(
            &fresh_client,
            "open_project",
            json!({"root":outside.path(),"relativePath":"fresh-project"})
        )
        .await
        .unwrap()["id"],
        fresh_project["id"]
    );
    assert!(fresh_project["runtime"].as_object().unwrap().is_empty());
    let stored = std::fs::read(state.path().join("clients.json")).unwrap();
    let loaded = AiBridge::new(manager.clone(), state.path().into(), port).unwrap();
    assert!(loaded.authenticate_token("legacy-token").await.is_some());
    assert_eq!(
        std::fs::read(state.path().join("clients.json")).unwrap(),
        stored
    );
    bridge.revoke("builder").await.unwrap();
    assert!(call(
        &client,
        "create_project",
        json!({"root":root_path,"name":"revoked"})
    )
    .await
    .is_err());
    assert!(!root_path.join("revoked").exists());
    assert!(call(
        &client,
        "open_project",
        json!({"root":outside.path(),"relativePath":"legacy-project"})
    )
    .await
    .is_err());
    assert!(call(&client, "list_project_roots", json!({}))
        .await
        .is_err());
    let _ = fresh_client.cancel().await;

    let _ = client.cancel().await;
    let _ = legacy.cancel().await;
    bridge.set_enabled(false).await.unwrap();
}

#[tokio::test]
async fn new_profiles_have_full_access_and_preserve_token_compatibility() {
    let state = tempfile::tempdir().unwrap();
    let manager = Manager::new(state.path().into()).unwrap();
    let bridge = AiBridge::new(manager, state.path().into(), 47831).unwrap();
    let executable = std::path::PathBuf::from(env!("CARGO_BIN_EXE_service-manager"));
    assert!(bridge
        .create("".into(), vec![], false, false, vec![], executable.clone())
        .await
        .is_err());
    assert!(bridge.clients.lock().await.is_empty());
    let created = bridge
        .create(
            "Full access".into(),
            vec![],
            false,
            false,
            vec![],
            executable,
        )
        .await
        .unwrap();
    let loaded = AiBridge::new(bridge.manager.clone(), state.path().into(), 47831).unwrap();
    let profile = loaded.authenticate_token(&created.token).await.unwrap();
    assert!(profile.edit && profile.control);
    let status = loaded.status().await;
    assert_eq!(status["clients"][0]["projects"], json!([]));
    assert_eq!(status["clients"][0]["projectRoots"], json!(["/"]));
    assert_eq!(status["clients"][0]["scope"], "filesystem");
}

#[test]
fn replaced_root_cannot_create_a_project_outside_its_grant() {
    let parent = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    let state = tempfile::tempdir().unwrap();
    let root = parent.path().join("approved");
    std::fs::create_dir(&root).unwrap();
    let allowed = root.canonicalize().unwrap();
    std::fs::rename(&root, parent.path().join("moved")).unwrap();
    std::os::unix::fs::symlink(outside.path(), &root).unwrap();
    let mut store = service_manager::project::ProjectStore::new(state.path().into()).unwrap();
    assert!(store
        .open_within(&allowed, std::path::Path::new("should-not-exist"), true)
        .is_err());
    assert!(!outside.path().join("should-not-exist").exists());
    assert!(store.projects.is_empty());
}

#[tokio::test]
async fn registration_does_not_rewrite_tokens_or_project_grants() {
    let root = tempfile::tempdir().unwrap();
    let state = tempfile::tempdir().unwrap();
    let manager = Manager::new(state.path().into()).unwrap();
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    drop(listener);
    let bridge = AiBridge::new(manager.clone(), state.path().into(), port).unwrap();
    let created = bridge
        .create(
            "Fixture".into(),
            vec![],
            false,
            false,
            vec![root.path().to_string_lossy().into_owned()],
            std::path::PathBuf::from(env!("CARGO_BIN_EXE_service-manager")),
        )
        .await
        .unwrap();
    bridge.set_enabled(true).await.unwrap();
    let client = connect(Credentials {
        endpoint: bridge.endpoint.clone(),
        token: created.token.clone(),
    })
    .await
    .unwrap();
    let file = state.path().join("clients.json");
    let backup = state.path().join("clients-backup.json");
    let original = std::fs::read(&file).unwrap();
    std::fs::rename(&file, &backup).unwrap();
    std::os::unix::fs::symlink(&backup, &file).unwrap();
    assert!(call(
        &client,
        "create_project",
        json!({"root":root.path().canonicalize().unwrap(),"name":"preserved"})
    )
    .await
    .is_ok());
    assert!(root.path().join("preserved").is_dir());
    assert!(bridge
        .authenticate_token(&created.token)
        .await
        .unwrap()
        .projects
        .is_empty());
    assert_eq!(std::fs::read(backup).unwrap(), original);
    assert_eq!(
        call(&client, "list_projects", json!({})).await.unwrap()["projects"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    let _ = client.cancel().await;
    bridge.set_enabled(false).await.unwrap();
}
