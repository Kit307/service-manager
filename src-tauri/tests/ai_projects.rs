use serde_json::json;
use service_manager::{
    manager::Manager,
    mcp::AiBridge,
    project::{hash, secure_write},
    proxy::{call, connect, Credentials},
};

#[tokio::test]
async fn mcp_creates_and_opens_projects_only_inside_approved_roots() {
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
    assert_eq!(roots["roots"][0]["path"], root_path.to_str().unwrap());
    assert_eq!(
        call(&legacy, "list_project_roots", json!({}))
            .await
            .unwrap()["roots"],
        json!([])
    );
    assert_eq!(
        call(
            &legacy,
            "create_project",
            json!({"root":root_path,"name":"denied"})
        )
        .await
        .unwrap_err()
        .code,
        "PERMISSION_DENIED"
    );
    assert!(!root_path.join("denied").exists());
    for name in ["../escape", "/absolute", ".", "a/b", ""] {
        assert!(call(
            &client,
            "create_project",
            json!({"root":root_path,"name":name})
        )
        .await
        .is_err());
    }
    assert!(call(
        &client,
        "create_project",
        json!({"root":outside.path(),"name":"denied"})
    )
    .await
    .is_err());
    assert!(!outside.path().join("denied").exists());
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
    assert_eq!(manager.projects().await.len(), 2);
    assert_eq!(
        call(&client, "list_projects", json!({})).await.unwrap()["projects"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    assert_eq!(
        call(&legacy, "list_projects", json!({})).await.unwrap()["projects"],
        json!([])
    );
    let saved = call(&client,"upsert_service",json!({"projectId":id,"expectedRevision":created["revision"],"definition":{"id":"web","name":"Web","cwd":".","type":"command","command":"printf marker"}})).await.unwrap();
    assert_eq!(saved["config"]["services"][0]["id"], "web");
    assert_eq!(manager.status(id, "web").await.unwrap().state, "stopped");
    assert_eq!(
        call(
            &client,
            "start_service",
            json!({"projectId":id,"serviceId":"web"})
        )
        .await
        .unwrap_err()
        .code,
        "PERMISSION_DENIED"
    );
    let reopened = call(
        &client,
        "open_project",
        json!({"root":root_path,"relativePath":"existing"}),
    )
    .await
    .unwrap();
    assert_eq!(reopened["id"], opened["id"]);
    let loaded = AiBridge::new(manager.clone(), state.path().into(), port).unwrap();
    assert!(loaded.authorize("builder", id, "edit").await.is_ok());
    bridge.revoke("builder").await.unwrap();
    assert!(call(
        &client,
        "create_project",
        json!({"root":root_path,"name":"revoked"})
    )
    .await
    .is_err());
    assert!(!root_path.join("revoked").exists());
    let _ = client.cancel().await;
    let _ = legacy.cancel().await;
    bridge.set_enabled(false).await.unwrap();
}

#[tokio::test]
async fn profiles_can_start_with_only_roots_and_reject_non_directory_grants() {
    let root = tempfile::tempdir().unwrap();
    let state = tempfile::tempdir().unwrap();
    let manager = Manager::new(state.path().into()).unwrap();
    let bridge = AiBridge::new(manager, state.path().into(), 47831).unwrap();
    let executable = std::path::PathBuf::from(env!("CARGO_BIN_EXE_service-manager"));
    assert!(bridge
        .create(
            "No scope".into(),
            vec![],
            false,
            false,
            vec![],
            executable.clone()
        )
        .await
        .is_err());
    let file = root.path().join("file.txt");
    std::fs::write(&file, "keep").unwrap();
    assert!(bridge
        .create(
            "File scope".into(),
            vec![],
            false,
            false,
            vec![file.to_string_lossy().into_owned()],
            executable.clone()
        )
        .await
        .is_err());
    assert!(bridge.clients.lock().await.is_empty());
    let created = bridge
        .create(
            "Root client".into(),
            vec![],
            false,
            false,
            vec![root.path().to_string_lossy().into_owned()],
            executable,
        )
        .await
        .unwrap();
    let loaded = AiBridge::new(bridge.manager.clone(), state.path().into(), 47831).unwrap();
    let status = loaded.status().await;
    assert_eq!(status["clients"][0]["projects"], json!([]));
    assert_eq!(
        status["clients"][0]["projectRoots"][0],
        root.path().canonicalize().unwrap().to_str().unwrap()
    );
    assert!(loaded.authenticate_token(&created.token).await.is_some());
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
async fn failed_grant_write_preserves_created_folder_without_granting_access() {
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
    .is_err());
    assert!(root.path().join("preserved").is_dir());
    assert!(bridge
        .authenticate_token(&created.token)
        .await
        .unwrap()
        .projects
        .is_empty());
    assert_eq!(std::fs::read(backup).unwrap(), original);
    assert_eq!(
        call(&client, "list_projects", json!({})).await.unwrap()["projects"],
        json!([])
    );
    let _ = client.cancel().await;
    bridge.set_enabled(false).await.unwrap();
}
