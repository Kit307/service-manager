use service_manager::{model::*, project::ProjectStore};
#[test]
fn saves_reopen_and_stale_writes_preserve_external_changes() {
    let root = tempfile::tempdir().unwrap();
    let state = tempfile::tempdir().unwrap();
    let mut store = ProjectStore::new(state.path().into()).unwrap();
    let p = store.open(root.path().to_str().unwrap()).unwrap();
    assert!(!root.path().join(".service-manager").exists());
    let config:ProjectConfig=serde_json::from_value(serde_json::json!({"schemaVersion":1,"services":[{"id":"web","name":"Web","cwd":".","type":"command","command":"sleep 1"}]})).unwrap();
    let saved = store.save(&p.id, config.clone(), "absent").unwrap();
    assert_eq!(store.open(root.path().to_str().unwrap()).unwrap().id, p.id);
    assert_eq!(store.refresh(&p.id).unwrap().config.services.len(), 1);
    let path = root.path().join(".service-manager/project.json");
    let external = br#"{"schemaVersion":1,"services":[]}"#;
    std::fs::write(&path, external).unwrap();
    assert_eq!(
        store
            .save(&p.id, config, &saved.revision)
            .err()
            .unwrap()
            .code,
        "CONFIG_CONFLICT"
    );
    assert_eq!(std::fs::read(path).unwrap(), external);
}
#[test]
fn symlinks_and_malformed_config_do_not_get_overwritten() {
    let root = tempfile::tempdir().unwrap();
    let state = tempfile::tempdir().unwrap();
    let external = tempfile::tempdir().unwrap();
    std::os::unix::fs::symlink(external.path(), root.path().join(".service-manager")).unwrap();
    let mut store = ProjectStore::new(state.path().into()).unwrap();
    assert!(store.open(root.path().to_str().unwrap()).is_err());
    assert!(std::fs::read_dir(external.path()).unwrap().next().is_none());
}
#[test]
fn service_input_rejects_mixed_variant_fields_at_all_boundaries() {
    assert!(serde_json::from_value::<ServiceDefinition>(serde_json::json!({"id":"x","name":"x","cwd":".","type":"command","command":"pwd","file":"compose.yaml"})).is_err());
    assert!(serde_json::from_value::<ServiceDefinition>(serde_json::json!({"id":"x","name":"x","cwd":".","type":"command","command":"pwd","unknown":"bad"})).is_err());
}

#[test]
fn trust_does_not_approve_a_replacement_command_after_review() {
    let root = tempfile::tempdir().unwrap();
    let state = tempfile::tempdir().unwrap();
    let mut store = ProjectStore::new(state.path().into()).unwrap();
    let p = store.open(root.path().to_str().unwrap()).unwrap();
    let config:ProjectConfig=serde_json::from_value(serde_json::json!({"schemaVersion":1,"services":[{"id":"web","name":"Web","cwd":".","type":"command","command":"printf reviewed"}]})).unwrap();
    let reviewed = store.save(&p.id, config, "absent").unwrap();
    let replacement=br#"{"schemaVersion":1,"services":[{"id":"web","name":"Web","cwd":".","type":"command","command":"printf unreviewed"}]}"#;
    std::fs::write(
        root.path().join(".service-manager/project.json"),
        replacement,
    )
    .unwrap();
    assert!(
        store
            .trust(&reviewed.id, "web", &reviewed.revision)
            .is_err(),
        "replacement executable settings were not reviewed"
    );
}
