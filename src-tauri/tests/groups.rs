use serde_json::json;
use service_manager::project::ProjectStore;

#[test]
fn virtual_groups_survive_loading_and_saving_private_state() {
    let state = tempfile::tempdir().unwrap();
    std::fs::write(state.path().join("state.json"), serde_json::to_vec(&json!({"projects":{},"trust":{},"dirty":false,"groups":[{"id":"fixture","name":"Workspace group","services":[]}]})).unwrap()).unwrap();
    let store = ProjectStore::new(state.path().into()).unwrap();
    store.persist().unwrap();
    let saved: serde_json::Value =
        serde_json::from_slice(&std::fs::read(state.path().join("state.json")).unwrap()).unwrap();
    assert_eq!(saved["groups"][0]["name"], "Workspace group");
}

use service_manager::{
    model::{ProjectConfig, ServiceDefinition, Snapshot},
    project::ServiceRef,
};
fn service() -> ServiceDefinition {
    serde_json::from_value(
        json!({"id":"web","name":"Web","cwd":".","type":"command","command":"sleep 60"}),
    )
    .unwrap()
}
fn register(store: &mut ProjectStore, root: &std::path::Path) -> ServiceRef {
    let project = store.open(root.to_str().unwrap()).unwrap();
    store
        .save(
            &project.id,
            ProjectConfig {
                schema_version: 1,
                services: vec![service()],
            },
            &project.revision,
        )
        .unwrap();
    ServiceRef {
        project_id: project.id,
        service_id: "web".into(),
    }
}
#[test]
fn groups_move_services_across_roots_without_changing_definitions_or_runtime() {
    let state = tempfile::tempdir().unwrap();
    let root = tempfile::tempdir().unwrap();
    let other = tempfile::tempdir().unwrap();
    let mut store = ProjectStore::new(state.path().into()).unwrap();
    assert!(store.private.groups.is_empty());
    let first = register(&mut store, root.path());
    let second = register(&mut store, other.path());
    let bytes = std::fs::read(root.path().join(".service-manager/project.json")).unwrap();
    let snapshot = Snapshot {
        state: "running".into(),
        ..Default::default()
    };
    store
        .projects
        .get_mut(&first.project_id)
        .unwrap()
        .runtime
        .insert("web".into(), snapshot);
    let a = store
        .save_group(
            None,
            "First",
            vec![first.clone(), first.clone(), second.clone()],
        )
        .unwrap();
    assert_eq!(a.services.len(), 2);
    let b = store
        .save_group(None, "Second", vec![first.clone()])
        .unwrap();
    assert_eq!(store.private.groups[0].services, vec![second.clone()]);
    store
        .save_group(Some(&b.id), "Renamed", vec![first.clone()])
        .unwrap();
    assert_eq!(
        std::fs::read(root.path().join(".service-manager/project.json")).unwrap(),
        bytes
    );
    assert!(store.projects[&first.project_id].runtime["web"].active());
    store.remove_group(&b.id).unwrap();
    assert_eq!(store.projects[&first.project_id].config.services.len(), 1);
    let loaded = ProjectStore::new(state.path().into()).unwrap();
    assert_eq!(loaded.private.groups.len(), 1);
    assert_eq!(loaded.private.groups[0].services, vec![second]);
}
#[test]
fn invalid_members_names_and_failed_writes_preserve_groups() {
    let state = tempfile::tempdir().unwrap();
    let root = tempfile::tempdir().unwrap();
    let mut store = ProjectStore::new(state.path().into()).unwrap();
    let member = register(&mut store, root.path());
    let group = store
        .save_group(None, "Demo", vec![member.clone()])
        .unwrap();
    assert!(store.save_group(None, " demo ", vec![]).is_err());
    assert!(store.save_group(None, " ", vec![]).is_err());
    assert!(store.save_group(Some("missing"), "New", vec![]).is_err());
    assert!(store
        .save_group(
            None,
            "Bad",
            vec![ServiceRef {
                project_id: "missing".into(),
                service_id: "web".into()
            }]
        )
        .is_err());
    let before = std::fs::read(state.path().join("state.json")).unwrap();
    assert_eq!(
        serde_json::to_value(&store.private.groups).unwrap()[0]["name"],
        "Demo"
    );
    let target = state.path().join("preserved.json");
    std::fs::rename(state.path().join("state.json"), &target).unwrap();
    std::os::unix::fs::symlink(&target, state.path().join("state.json")).unwrap();
    assert!(store
        .save_group(Some(&group.id), "Changed", vec![])
        .is_err());
    assert!(store.remove_group(&group.id).is_err());
    assert_eq!(store.private.groups[0].services, vec![member]);
    assert_eq!(store.private.groups[0].name, "Demo");
    assert_eq!(std::fs::read(target).unwrap(), before);
}
#[test]
fn explicit_service_and_root_removal_clean_membership() {
    let state = tempfile::tempdir().unwrap();
    let root = tempfile::tempdir().unwrap();
    let mut store = ProjectStore::new(state.path().into()).unwrap();
    let member = register(&mut store, root.path());
    let group = store
        .save_group(None, "Demo", vec![member.clone()])
        .unwrap();
    store
        .remove_service_from_groups(&member.project_id, &member.service_id)
        .unwrap();
    assert!(store.private.groups[0].services.is_empty());
    store
        .save_group(Some(&group.id), "Demo", vec![member.clone()])
        .unwrap();
    store.remove_project(&member.project_id).unwrap();
    assert!(store.private.groups[0].services.is_empty());
    assert!(root.path().join(".service-manager/project.json").exists());
}

#[test]
fn direct_moves_preserve_other_members_and_reject_failed_or_foreign_moves() {
    let state = tempfile::tempdir().unwrap();
    let root = tempfile::tempdir().unwrap();
    let other = tempfile::tempdir().unwrap();
    let mut store = ProjectStore::new(state.path().into()).unwrap();
    let a = register(&mut store, root.path());
    let b = register(&mut store, other.path());
    let one = store
        .save_group(None, "One", vec![a.clone(), b.clone()])
        .unwrap();
    let two = store.save_group(None, "Two", vec![]).unwrap();
    let bytes = std::fs::read(root.path().join(".service-manager/project.json")).unwrap();
    store
        .projects
        .get_mut(&a.project_id)
        .unwrap()
        .runtime
        .insert(
            "web".into(),
            Snapshot {
                state: "running".into(),
                ..Default::default()
            },
        );
    store.move_service(a.clone(), Some(&two.id)).unwrap();
    store.move_service(a.clone(), Some(&two.id)).unwrap();
    assert_eq!(store.private.groups[0].services, vec![b]);
    assert_eq!(store.private.groups[1].services, vec![a.clone()]);
    assert!(store.move_service(a.clone(), Some("missing")).is_err());
    assert!(store
        .move_service(
            ServiceRef {
                project_id: "foreign".into(),
                service_id: "web".into()
            },
            None
        )
        .is_err());
    let target = state.path().join("backup.json");
    std::fs::rename(state.path().join("state.json"), &target).unwrap();
    std::os::unix::fs::symlink(&target, state.path().join("state.json")).unwrap();
    assert!(store.move_service(a.clone(), Some(&one.id)).is_err());
    assert_eq!(store.private.groups[1].services, vec![a.clone()]);
    std::fs::remove_file(state.path().join("state.json")).unwrap();
    std::fs::rename(target, state.path().join("state.json")).unwrap();
    store.move_service(a.clone(), None).unwrap();
    let loaded = ProjectStore::new(state.path().into()).unwrap();
    assert!(loaded.private.groups[1].services.is_empty());
    assert!(store.projects[&a.project_id].runtime["web"].active());
    assert_eq!(
        std::fs::read(root.path().join(".service-manager/project.json")).unwrap(),
        bytes
    );
}
