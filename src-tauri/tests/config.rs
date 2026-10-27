use service_manager::model::{validate, ProjectConfig, ServiceDefinition, ServiceKind};

fn command(cwd: &str) -> ServiceDefinition {
    ServiceDefinition {
        id: "web".into(),
        name: "Web".into(),
        cwd: cwd.into(),
        kind: ServiceKind::Command {
            command: "pnpm dev".into(),
        },
        env_file: None,
        health_check: None,
        ports: vec![],
        url: None,
        depends_on: vec![],
        vpn: None,
        vpn_routes: vec![],
    }
}
#[test]
fn cannot_execute_outside_the_opened_project() {
    let root = tempfile::tempdir().unwrap();
    let config = ProjectConfig {
        schema_version: 1,
        services: vec![command("..")],
    };
    assert!(validate(root.path(), &config).is_err());
}
#[test]
fn duplicate_ids_cannot_redirect_service_operations() {
    let root = tempfile::tempdir().unwrap();
    let config = ProjectConfig {
        schema_version: 1,
        services: vec![command("."), command(".")],
    };
    assert!(validate(root.path(), &config).is_err());
}
#[test]
fn unsupported_schema_is_not_replaced_with_empty_defaults() {
    let root = tempfile::tempdir().unwrap();
    assert!(validate(
        root.path(),
        &ProjectConfig {
            schema_version: 999,
            services: vec![]
        }
    )
    .is_err());
}
