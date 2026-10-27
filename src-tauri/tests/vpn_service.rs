use serde_json::json;
use service_manager::{
    manager::Manager,
    model::{ServiceDefinition, VpnRoute, VpnService},
    ports::configured_ports,
    vpn::VpnImport,
};
fn command() -> ServiceDefinition {
    serde_json::from_value(
        json!({"id":"api","name":"API","cwd":".","type":"command","command":"node server.js"}),
    )
    .unwrap()
}
#[test]
fn absent_vpn_preserves_existing_wire_format() {
    let definition = command();
    let value = serde_json::to_value(&definition).unwrap();
    assert!(value.get("vpn").is_none());
    assert_eq!(
        value,
        json!({"id":"api","name":"API","cwd":".","type":"command","command":"node server.js"})
    );
}

#[test]
fn command_accepts_multiple_native_vpn_routes() {
    let value = json!({
        "id":"api",
        "name":"API",
        "cwd":".",
        "type":"command",
        "command":"go run .",
        "vpnRoutes":[
            {
                "profileId":"ac9c00d2-4810-43b6-a8e9-b9f07fecf6ec",
                "remoteHost":"sso.internal.example",
                "remotePort":443,
                "localPort":18443
            },
            {
                "profileId":"b99c00d2-4810-43b6-a8e9-b9f07fecf6ec",
                "remoteHost":"database.internal.example",
                "remotePort":5432,
                "localPort":15432
            }
        ]
    });
    let definition: ServiceDefinition = serde_json::from_value(value.clone()).unwrap();
    assert_eq!(serde_json::to_value(definition).unwrap(), value);
}

#[test]
fn native_vpn_routes_reject_unsafe_or_ambiguous_endpoints() {
    let route = json!({
        "profileId":"ac9c00d2-4810-43b6-a8e9-b9f07fecf6ec",
        "remoteHost":"sso.internal.example",
        "remotePort":443,
        "localPort":18443
    });
    let command = |routes| {
        json!({
            "id":"api","name":"API","cwd":".","type":"command","command":"go run .","vpnRoutes":routes
        })
    };

    let mut duplicate = route.clone();
    duplicate["remoteHost"] = "other.internal.example".into();
    assert!(serde_json::from_value::<ServiceDefinition>(command(json!([
        route.clone(),
        duplicate
    ])))
    .is_err());

    for invalid in [
        json!({"profileId":route["profileId"],"remoteHost":"bad\r\nhost","remotePort":443,"localPort":18443}),
        json!({"profileId":route["profileId"],"remoteHost":"https://internal.example","remotePort":443,"localPort":18443}),
        json!({"profileId":route["profileId"],"remoteHost":"internal.example","remotePort":0,"localPort":18443}),
        json!({"profileId":route["profileId"],"remoteHost":"internal.example","remotePort":443,"localPort":0}),
    ] {
        assert!(serde_json::from_value::<ServiceDefinition>(command(json!([invalid]))).is_err());
    }

    let compose = json!({
        "id":"api","name":"API","cwd":".","type":"compose","file":"compose.yml","vpnRoutes":[route]
    });
    assert!(serde_json::from_value::<ServiceDefinition>(compose).is_err());
}

#[test]
fn native_vpn_route_ports_are_reserved_for_loopback_listeners() {
    let mut definition = command();
    definition.ports = vec![8080];
    definition.vpn_routes = vec![VpnRoute {
        profile_id: "ac9c00d2-4810-43b6-a8e9-b9f07fecf6ec".into(),
        remote_host: "database.internal.example".into(),
        remote_port: 5432,
        local_port: 15432,
    }];
    assert_eq!(configured_ports(&definition), vec![8080, 15432]);
}

#[tokio::test]
async fn save_rejects_unknown_native_route_profiles_without_writing_project() {
    let root = tempfile::tempdir().unwrap();
    let state = tempfile::tempdir().unwrap();
    let manager = Manager::new(state.path().into()).unwrap();
    let project = manager.open(root.path().to_str().unwrap()).await.unwrap();
    let mut definition = command();
    definition.vpn_routes = vec![VpnRoute {
        profile_id: "ac9c00d2-4810-43b6-a8e9-b9f07fecf6ec".into(),
        remote_host: "database.internal.example".into(),
        remote_port: 5432,
        local_port: 15432,
    }];
    assert!(manager
        .save(&project.id, definition, &project.revision, false)
        .await
        .is_err());
    assert!(!root.path().join(".service-manager/project.json").exists());
}
#[test]
fn vpn_rejects_compose_unknown_fields_and_image_options() {
    let vpn = json!({"profileId":"ac9c00d2-4810-43b6-a8e9-b9f07fecf6ec","image":"alpine:3.22"});
    let mut compose =
        json!({"id":"api","name":"API","cwd":".","type":"compose","file":"compose.yml","vpn":vpn});
    assert!(serde_json::from_value::<ServiceDefinition>(compose.clone()).is_err());
    compose["type"] = "command".into();
    compose.as_object_mut().unwrap().remove("file");
    compose["command"] = "echo fixture".into();
    assert!(serde_json::from_value::<ServiceDefinition>(compose.clone()).is_ok());
    compose["vpn"]["image"] = "--privileged".into();
    assert!(serde_json::from_value::<ServiceDefinition>(compose.clone()).is_err());
    compose["vpn"]["image"] = "alpine:3.22".into();
    compose["vpn"]["password"] = "synthetic".into();
    assert!(serde_json::from_value::<ServiceDefinition>(compose).is_err());
}
#[tokio::test]
async fn save_rejects_unknown_profiles_without_writing_project() {
    let root = tempfile::tempdir().unwrap();
    let state = tempfile::tempdir().unwrap();
    let manager = Manager::new(state.path().into()).unwrap();
    let project = manager.open(root.path().to_str().unwrap()).await.unwrap();
    let mut definition = command();
    definition.vpn = Some(VpnService {
        profile_id: "ac9c00d2-4810-43b6-a8e9-b9f07fecf6ec".into(),
        image: "alpine:3.22".into(),
    });
    assert!(manager
        .save(&project.id, definition, &project.revision, false)
        .await
        .is_err());
    assert!(!root.path().join(".service-manager/project.json").exists());
}
#[tokio::test]
async fn save_requires_profile_mappings_and_reloads_vpn_definition() {
    let root = tempfile::tempdir().unwrap();
    let state = tempfile::tempdir().unwrap();
    let manager = Manager::new(state.path().into()).unwrap();
    let project = manager.open(root.path().to_str().unwrap()).await.unwrap();
    let config = state.path().join("fixture.ovpn");
    std::fs::write(&config, "client\ndev tun\nauth-user-pass").unwrap();
    let profile = manager
        .vpns
        .import(VpnImport {
            name: "Fixture".into(),
            config_path: config.to_string_lossy().into(),
            key_passphrase: String::new(),
            username: "fixture".into(),
            password: "synthetic-only".into(),
            published_ports: vec![service_manager::vpn::VpnPortMapping {
                host: 18080,
                container: 8080,
            }],
            outbound_subnets: vec![],
            health_target: "example.com:443".into(),
            dns_address: String::new(),
            health_icmp_target: String::new(),
        })
        .await
        .unwrap();
    let mut definition = command();
    definition.vpn = Some(VpnService {
        profile_id: profile.id,
        image: "alpine:3.22".into(),
    });
    definition.ports = vec![18081];
    assert!(manager
        .save(&project.id, definition.clone(), &project.revision, false)
        .await
        .is_err());
    assert!(!root.path().join(".service-manager/project.json").exists());
    definition.ports = vec![18080];
    definition.health_check = Some(service_manager::model::HealthCheck {
        url: "http://127.0.0.1:18081/health".into(),
    });
    assert!(manager
        .save(&project.id, definition.clone(), &project.revision, false)
        .await
        .is_err());
    definition.health_check.as_mut().unwrap().url = "http://127.0.0.1:18080/health".into();
    let saved = manager
        .save(&project.id, definition, &project.revision, false)
        .await
        .unwrap();
    assert!(saved.config.services[0].vpn.is_some());
    let reopened = manager.open(root.path().to_str().unwrap()).await.unwrap();
    assert_eq!(
        reopened.config.services[0].vpn.as_ref().unwrap().image,
        "alpine:3.22"
    );
}
