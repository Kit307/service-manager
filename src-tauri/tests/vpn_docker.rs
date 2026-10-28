use serde_json::Value;
use service_manager::{
    manager::Manager,
    mcp::AiBridge,
    model::*,
    project::{hash, secure_write},
    proxy::{call, connect, Credentials},
    vpn::{VpnImport, VpnManager},
};
use std::{
    net::{Ipv4Addr, TcpListener},
    path::{Path, PathBuf},
    process::Command,
    time::Duration,
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
};

fn docker(args: &[&str]) -> String {
    let result = Command::new("docker").args(args).output().unwrap();
    assert!(
        result.status.success(),
        "synthetic Docker fixture operation failed: {}",
        args[0]
    );
    String::from_utf8(result.stdout).unwrap().trim().into()
}
struct Cleanup {
    stacks: Vec<String>,
    gateways: Vec<String>,
    networks: Vec<String>,
    server: String,
}
impl Drop for Cleanup {
    fn drop(&mut self) {
        if std::thread::panicking() {
            for name in &self.gateways {
                if let Ok(out) = Command::new("docker").args(["logs", name]).output() {
                    for line in String::from_utf8_lossy(&out.stdout)
                        .lines()
                        .chain(String::from_utf8_lossy(&out.stderr).lines())
                    {
                        if line.contains("ERROR")
                            || line.contains("Options error")
                            || line.contains("Exiting due")
                        {
                            let safe = line
                                .replace("424242", "[redacted]")
                                .replace("synthetic-password", "[redacted]")
                                .replace("synthetic-key-passphrase-only", "[redacted]")
                                .replace("synthetic-wrong-passphrase", "[redacted]");
                            eprintln!("synthetic gateway init: {safe}");
                        }
                    }
                }
            }
        }
        for stack in &self.stacks {
            let result = Command::new("docker")
                .args([
                    "ps",
                    "-aq",
                    "--filter",
                    &format!("label=com.docker.compose.project={stack}"),
                ])
                .output();
            if let Ok(result) = result {
                for id in String::from_utf8_lossy(&result.stdout).lines() {
                    let _ = Command::new("docker").args(["rm", "-f", id]).output();
                }
            }
        }
        for name in &self.gateways {
            let _ = Command::new("docker").args(["rm", "-f", name]).output();
        }
        for name in &self.networks {
            let _ = Command::new("docker")
                .args(["network", "disconnect", "-f", name, &self.server])
                .output();
            let _ = Command::new("docker")
                .args(["network", "rm", name])
                .output();
        }
    }
}
async fn wait_marker(manager: &Manager, project: &str, service: &str) {
    for _ in 0..900 {
        if manager
            .read_logs(project, service, None)
            .await
            .unwrap()
            .records
            .iter()
            .any(|r| r.text.contains("vpn-worker-marker"))
        {
            return;
        }
        let status = manager.status(project, service).await.unwrap();
        assert_ne!(
            status.state, "failed",
            "synthetic worker failed: {:?}",
            status.error
        );
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    panic!("synthetic VPN worker did not start");
}
async fn wait_terminal(manager: &Manager, project: &str, service: &str, expected: &str) {
    for _ in 0..400 {
        let status = manager.status(project, service).await.unwrap();
        if !status.active() {
            assert_eq!(status.state, expected);
            return;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    panic!("synthetic worker cleanup was not verified");
}
fn worker_id(stack: &str) -> String {
    docker(&[
        "ps",
        "-aq",
        "--filter",
        &format!("label=com.docker.compose.project={stack}"),
    ])
}
fn reachable(worker: &str) -> bool {
    let result = Command::new("docker")
        .args([
            "exec",
            worker,
            "wget",
            "-T",
            "3",
            "-qO-",
            "http://10.66.0.1:5432",
        ])
        .output()
        .unwrap();
    result.status.success() && String::from_utf8_lossy(&result.stdout).trim() == "synthetic-ok"
}

fn free_port() -> u16 {
    TcpListener::bind((Ipv4Addr::LOCALHOST, 0))
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}

async fn relay_reachable(port: u16) -> bool {
    let Ok(Ok(mut stream)) = tokio::time::timeout(
        Duration::from_secs(3),
        TcpStream::connect((Ipv4Addr::LOCALHOST, port)),
    )
    .await
    else {
        return false;
    };
    if stream
        .write_all(b"GET / HTTP/1.1\r\nHost: synthetic\r\nConnection: close\r\n\r\n")
        .await
        .is_err()
    {
        return false;
    }
    let mut response = String::new();
    tokio::time::timeout(Duration::from_secs(3), stream.read_to_string(&mut response))
        .await
        .is_ok_and(|result| result.is_ok())
        && response.contains("synthetic-ok")
}

// Explicit opt-in: creates a local synthetic OpenVPN server and only uniquely owned resources.
#[tokio::test]
#[ignore = "requires local Docker, OpenSSL, Gluetun v3.41.0 and alpine:3.23.3"]
async fn vpn_manager_shares_gateways_and_verifies_loss_cleanup() {
    let fixture = match std::env::var_os("SM_VPN_FIXTURE_DIR") {
        Some(path) => PathBuf::from(path),
        None => {
            let script = Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../.agents/evidence/TASK-0018/vpn-connected-proof.py");
            let status = Command::new("python3")
                .arg(script)
                .arg(std::env::current_exe().unwrap())
                .status()
                .unwrap();
            assert!(status.success(), "synthetic VPN manager proof failed");
            return;
        }
    };
    let server = std::env::var("SM_VPN_FIXTURE_SERVER").unwrap();
    let state = fixture.join("state");
    let root = fixture.join("project");
    std::fs::create_dir(&root).unwrap();
    let manager = Manager::new(state.clone()).unwrap();
    let project = manager.open(root.to_str().unwrap()).await.unwrap();
    let mut profiles = Vec::new();
    let octet = 2 + uuid::Uuid::new_v4().as_bytes()[0] % 240;
    let endpoints = [
        format!("10.231.{octet}.2"),
        format!("10.231.{}.2", octet + 1),
    ];
    for (index, name) in ["Synthetic A", "Synthetic B"].iter().enumerate() {
        let config = std::fs::read_to_string(fixture.join("client.ovpn"))
            .unwrap()
            .replace(
                "remote synthetic-vpn-server 1194",
                &format!("remote {} 1194", endpoints[index]),
            );
        let config_path = fixture.join(format!("client-{index}.ovpn"));
        secure_write(&config_path, config.as_bytes()).unwrap();
        profiles.push(
            manager
                .vpns
                .import(VpnImport {
                    name: (*name).into(),
                    config_path: config_path.to_str().unwrap().into(),
                    key_passphrase: String::new(),
                    username: "synthetic-user".into(),
                    password: "synthetic-password".into(),
                    published_ports: vec![],
                    outbound_subnets: vec![],
                    health_target: "10.66.0.1:5432".into(),
                    dns_address: String::new(),
                    health_icmp_target: "10.66.0.1".into(),
                })
                .await
                .unwrap(),
        );
    }
    let owner = std::fs::read_to_string(state.join("vpn/owner")).unwrap();
    let metadata: Vec<Value> =
        serde_json::from_slice(&std::fs::read(state.join("vpn/profiles.json")).unwrap()).unwrap();
    let mut cleanup = Cleanup {
        stacks: vec![],
        gateways: vec![],
        networks: vec![],
        server,
    };
    // A synthetic server joins each owned private bridge; no host/public ports are exposed.
    for (index, p) in metadata.iter().enumerate() {
        let id = p["id"].as_str().unwrap();
        let fingerprint = p["fingerprint"].as_str().unwrap();
        let name = format!("sm-vpn-{}-{id}", &owner[..8]);
        let network = format!("{name}-net");
        cleanup.gateways.push(name);
        cleanup.networks.push(network.clone());
        docker(&[
            "network",
            "create",
            "--driver",
            "bridge",
            "--subnet",
            &format!("10.231.{}.0/24", octet + index as u8),
            "--label",
            &format!("io.service-manager.vpn.owner={owner}"),
            "--label",
            &format!("io.service-manager.vpn.profile={id}"),
            "--label",
            &format!("io.service-manager.vpn.fingerprint={fingerprint}"),
            &network,
        ]);
        docker(&[
            "network",
            "connect",
            "--ip",
            &endpoints[index],
            &network,
            &cleanup.server,
        ]);
    }
    for (id, profile) in [
        ("a1", &profiles[0]),
        ("a2", &profiles[0]),
        ("b1", &profiles[1]),
    ] {
        let definition = ServiceDefinition {
            id: id.into(),
            name: id.into(),
            cwd: ".".into(),
            kind: ServiceKind::Command {
                command: "echo vpn-worker-marker; exec sleep 300".into(),
            },
            env_file: None,
            vpn: Some(VpnService {
                profile_id: profile.id.clone(),
                image: "alpine:3.23.3".into(),
            }),
            vpn_routes: vec![],
            health_check: None,
            ports: vec![],
            url: None,
            depends_on: vec![],
        };
        let revision = manager.store.lock().await.projects[&project.id]
            .revision
            .clone();
        manager
            .save(&project.id, definition, &revision, false)
            .await
            .unwrap();
        cleanup.stacks.push(format!(
            "sm-{}",
            &hash(format!("{}/{id}", project.id).as_bytes())[..24]
        ));
    }
    for id in ["a1", "a2", "b1"] {
        let revision = manager.store.lock().await.projects[&project.id]
            .revision
            .clone();
        manager
            .store
            .lock()
            .await
            .trust(&project.id, id, &revision)
            .unwrap();
    }
    let first = manager.start(&project.id, "a1", "test").await.unwrap();
    assert_eq!(
        first.run_id,
        manager
            .start(&project.id, "a1", "test")
            .await
            .unwrap()
            .run_id
    );
    manager.start(&project.id, "a2", "test").await.unwrap();
    manager.start(&project.id, "b1", "test").await.unwrap();
    for id in ["a1", "a2", "b1"] {
        wait_marker(&manager, &project.id, id).await;
    }
    let workers: Vec<_> = cleanup.stacks.iter().map(|s| worker_id(s)).collect();
    assert!(
        workers.iter().all(|id| reachable(id)),
        "all workers must reach the synthetic VPN target"
    );
    assert_eq!(
        docker(&[
            "ps",
            "-q",
            "--filter",
            &format!("label=io.service-manager.vpn.owner={owner}")
        ])
        .lines()
        .count(),
        2
    );
    assert!(manager.vpns.disconnect(&profiles[0].id).await.is_err());
    let reopened = VpnManager::new(state.clone()).unwrap();
    assert!(
        reopened.disconnect(&profiles[0].id).await.is_err(),
        "post-restart namespace users must block disconnect"
    );
    manager.stop(&project.id, "a1").await.unwrap();
    wait_terminal(&manager, &project.id, "a1", "stopped").await;
    assert!(reachable(&workers[1]));
    assert!(reachable(&workers[2]));
    docker(&["pause", &cleanup.gateways[0]]);
    let mut unhealthy = false;
    for _ in 0..40 {
        if docker(&[
            "inspect",
            "--format",
            "{{.State.Health.Status}}",
            &cleanup.gateways[0],
        ]) == "unhealthy"
        {
            unhealthy = true;
            break;
        }
        tokio::time::sleep(Duration::from_secs(1)).await;
    }
    assert!(unhealthy, "paused gateway must become unhealthy");
    tokio::time::sleep(Duration::from_secs(2)).await;
    assert!(
        manager.status(&project.id, "a2").await.unwrap().active(),
        "a transiently unhealthy owned gateway must preserve its worker"
    );
    docker(&["unpause", &cleanup.gateways[0]]);
    docker(&["stop", "--time", "1", &cleanup.gateways[0]]);
    wait_terminal(&manager, &project.id, "a2", "failed").await;
    assert_eq!(
        docker(&["inspect", "--format", "{{.State.Running}}", &workers[1]]),
        "false"
    );
    assert!(manager.status(&project.id, "b1").await.unwrap().active());
    assert!(reachable(&workers[2]));
    manager.shutdown().await.unwrap();
    wait_terminal(&manager, &project.id, "b1", "stopped").await;
    println!("synthetic VPN manager proof passed: two gateways, shared workers, transient health preservation, run reuse, post-restart guard, verified gateway-stop cleanup and unaffected other VPN");
}

#[tokio::test]
#[ignore = "requires local Docker, OpenSSL and Gluetun v3.41.0"]
async fn native_vpn_routes_keep_commands_on_host_and_share_gateways() {
    let fixture = match std::env::var_os("SM_VPN_FIXTURE_DIR") {
        Some(path) => PathBuf::from(path),
        None => {
            let script = Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../.agents/evidence/TASK-0018/vpn-connected-proof.py");
            let status = Command::new("python3")
                .arg(script)
                .arg(std::env::current_exe().unwrap())
                .arg("native_vpn_routes_keep_commands_on_host_and_share_gateways")
                .status()
                .unwrap();
            assert!(status.success(), "synthetic native VPN relay proof failed");
            return;
        }
    };
    let server = std::env::var("SM_VPN_FIXTURE_SERVER").unwrap();
    let state = fixture.join("state-native");
    let root = fixture.join("project-native");
    std::fs::create_dir(&root).unwrap();
    let manager = Manager::new(state.clone()).unwrap();
    let project = manager.open(root.to_str().unwrap()).await.unwrap();
    let bridge = AiBridge::new(manager.clone(), state.clone(), free_port()).unwrap();
    bridge.set_enabled(true).await.unwrap();
    let created = bridge
        .create(
            "synthetic-vpn-client".into(),
            vec![],
            true,
            true,
            vec![],
            std::path::PathBuf::from(env!("CARGO_BIN_EXE_service-manager")),
        )
        .await
        .unwrap();
    let client = connect(Credentials {
        endpoint: bridge.endpoint.clone(),
        token: created.token,
    })
    .await
    .unwrap();
    let owner = std::fs::read_to_string(state.join("vpn/owner")).unwrap();
    let octet = 2 + uuid::Uuid::new_v4().as_bytes()[0] % 240;
    let endpoints = [
        format!("10.232.{octet}.2"),
        format!("10.232.{}.2", octet + 1),
    ];
    let mut profiles = Vec::new();
    for (index, name) in ["Native A", "Native B"].iter().enumerate() {
        let config = std::fs::read_to_string(fixture.join("client.ovpn"))
            .unwrap()
            .replace(
                "remote synthetic-vpn-server 1194",
                &format!("remote {} 1194", endpoints[index]),
            );
        let config_path = fixture.join(format!("native-client-{index}.ovpn"));
        secure_write(&config_path, config.as_bytes()).unwrap();
        profiles.push(
            manager
                .vpns
                .import(VpnImport {
                    name: (*name).into(),
                    config_path: config_path.to_str().unwrap().into(),
                    key_passphrase: String::new(),
                    username: "synthetic-user".into(),
                    password: "synthetic-password".into(),
                    published_ports: vec![],
                    outbound_subnets: vec![],
                    health_target: "10.66.0.1:5432".into(),
                    dns_address: String::new(),
                    health_icmp_target: "10.66.0.1".into(),
                })
                .await
                .unwrap(),
        );
    }
    let metadata: Vec<Value> =
        serde_json::from_slice(&std::fs::read(state.join("vpn/profiles.json")).unwrap()).unwrap();
    let mut cleanup = Cleanup {
        stacks: vec![],
        gateways: vec![],
        networks: vec![],
        server,
    };
    for (index, profile) in metadata.iter().enumerate() {
        let id = profile["id"].as_str().unwrap();
        let fingerprint = profile["fingerprint"].as_str().unwrap();
        let name = format!("sm-vpn-{}-{id}", &owner[..8]);
        let network = format!("{name}-net");
        cleanup.gateways.push(name);
        cleanup.networks.push(network.clone());
        docker(&[
            "network",
            "create",
            "--driver",
            "bridge",
            "--subnet",
            &format!("10.232.{}.0/24", octet + index as u8),
            "--label",
            &format!("io.service-manager.vpn.owner={owner}"),
            "--label",
            &format!("io.service-manager.vpn.profile={id}"),
            "--label",
            &format!("io.service-manager.vpn.fingerprint={fingerprint}"),
            &network,
        ]);
        docker(&[
            "network",
            "connect",
            "--ip",
            &endpoints[index],
            &network,
            &cleanup.server,
        ]);
    }
    let ports = [free_port(), free_port(), free_port()];
    for (index, id) in ["native-a1", "native-a2", "native-b1"].iter().enumerate() {
        let profile = if index < 2 {
            &profiles[0]
        } else {
            &profiles[1]
        };
        let command = format!(
            "exec /usr/bin/python3 -u -c 'import urllib.request,time; print(\"vpn-worker-marker\",urllib.request.urlopen(\"http://127.0.0.1:{}\",timeout=5).read().decode(),flush=True); time.sleep(300)'",
            ports[index]
        );
        let definition = ServiceDefinition {
            id: (*id).into(),
            name: (*id).into(),
            cwd: ".".into(),
            kind: ServiceKind::Command { command },
            env_file: None,
            health_check: None,
            ports: vec![],
            url: None,
            depends_on: vec![],
            vpn: None,
            vpn_routes: vec![VpnRoute {
                profile_id: profile.id.clone(),
                remote_host: "10.66.0.1".into(),
                remote_port: 5432,
                local_port: ports[index],
            }],
        };
        let config = call(
            &client,
            "get_project_config",
            serde_json::json!({"projectId": project.id}),
        )
        .await
        .unwrap();
        call(
            &client,
            "upsert_service",
            serde_json::json!({
                "projectId": project.id,
                "expectedRevision": config["revision"],
                "definition": definition,
            }),
        )
        .await
        .unwrap();
    }
    for id in ["native-a1", "native-a2", "native-b1"] {
        call(
            &client,
            "start_service",
            serde_json::json!({"projectId": project.id, "serviceId": id}),
        )
        .await
        .unwrap();
        wait_marker(&manager, &project.id, id).await;
        let logs = call(
            &client,
            "read_logs",
            serde_json::json!({"projectId": project.id, "serviceId": id}),
        )
        .await
        .unwrap();
        assert!(logs["records"].as_array().unwrap().iter().any(|record| {
            record["text"]
                .as_str()
                .unwrap_or_default()
                .contains("vpn-worker-marker")
        }));
    }
    // A successful routed preflight followed by an OS spawn failure must release
    // the relay and publish no traffic for that failed run.
    let failure_port = free_port();
    std::fs::write(
        root.join("oversized.env"),
        format!("LARGE={}\n", "x".repeat(4 * 1024 * 1024)),
    )
    .unwrap();
    let revision = manager
        .projects()
        .await
        .into_iter()
        .find(|p| p.id == project.id)
        .unwrap()
        .revision;
    manager
        .save(
            &project.id,
            ServiceDefinition {
                id: "routed-spawn-failure".into(),
                name: "Routed spawn failure".into(),
                cwd: ".".into(),
                kind: ServiceKind::Command {
                    command: "true".into(),
                },
                env_file: Some("oversized.env".into()),
                health_check: None,
                ports: vec![],
                url: None,
                depends_on: vec![],
                vpn: None,
                vpn_routes: vec![VpnRoute {
                    profile_id: profiles[0].id.clone(),
                    remote_host: "10.66.0.1".into(),
                    remote_port: 5432,
                    local_port: failure_port,
                }],
            },
            &revision,
            true,
        )
        .await
        .unwrap();
    let error = manager
        .start(&project.id, "routed-spawn-failure", "test")
        .await
        .unwrap_err();
    assert_eq!(error.code, "PROCESS_START_FAILED");
    assert!(!manager
        .network_traffic()
        .iter()
        .any(|row| row.service_id == "routed-spawn-failure"));
    let rebound = TcpListener::bind((Ipv4Addr::LOCALHOST, failure_port)).unwrap();
    drop(rebound);

    for port in ports {
        assert!(
            relay_reachable(port).await,
            "relay {port} did not reach target"
        );
    }
    let traffic = || {
        manager
            .network_traffic()
            .into_iter()
            .find(|row| row.service_id == "native-a1")
            .unwrap()
    };
    let before = traffic();
    assert!(relay_reachable(ports[0]).await);
    let after = traffic();
    assert_eq!(
        after.tx_bytes - before.tx_bytes,
        b"GET / HTTP/1.1\r\nHost: synthetic\r\nConnection: close\r\n\r\n".len() as u64
    );
    assert_eq!(
        after.rx_bytes - before.rx_bytes,
        b"HTTP/1.1 200 OK\r\nContent-Length: 12\r\nConnection: close\r\n\r\nsynthetic-ok".len()
            as u64
    );
    tokio::time::sleep(Duration::from_millis(200)).await;
    let idle = traffic();
    assert_eq!(
        (idle.tx_bytes, idle.rx_bytes),
        (after.tx_bytes, after.rx_bytes)
    );
    println!("live relay telemetry proof: exact TX/RX payload deltas and idle stability");
    assert_eq!(
        docker(&[
            "ps",
            "-q",
            "--filter",
            &format!("label=io.service-manager.vpn.owner={owner}")
        ])
        .lines()
        .count(),
        2,
        "only the two shared gateways should run in Docker"
    );
    let pid = manager
        .status(&project.id, "native-a1")
        .await
        .unwrap()
        .pid
        .unwrap();
    let process = Command::new("ps")
        .args(["-p", &pid.to_string(), "-o", "comm="])
        .output()
        .unwrap();
    assert!(String::from_utf8_lossy(&process.stdout).contains("Python"));
    call(
        &client,
        "stop_service",
        serde_json::json!({"projectId": project.id, "serviceId": "native-a1"}),
    )
    .await
    .unwrap();
    wait_terminal(&manager, &project.id, "native-a1", "stopped").await;
    assert!(!manager
        .network_traffic()
        .iter()
        .any(|row| row.service_id == "native-a1"));
    let rebound = TcpListener::bind((Ipv4Addr::LOCALHOST, ports[0])).unwrap();
    drop(rebound);
    manager
        .start(&project.id, "native-a1", "test")
        .await
        .unwrap();
    wait_marker(&manager, &project.id, "native-a1").await;
    manager.stop(&project.id, "native-a1").await.unwrap();
    wait_terminal(&manager, &project.id, "native-a1", "stopped").await;
    assert!(relay_reachable(ports[1]).await);
    assert!(relay_reachable(ports[2]).await);
    docker(&["stop", "--time", "1", &cleanup.gateways[0]]);
    wait_terminal(&manager, &project.id, "native-a2", "failed").await;
    assert!(!manager
        .network_traffic()
        .iter()
        .any(|row| row.service_id == "native-a2"));
    let rebound = TcpListener::bind((Ipv4Addr::LOCALHOST, ports[1])).unwrap();
    drop(rebound);
    assert!(manager
        .status(&project.id, "native-b1")
        .await
        .unwrap()
        .active());
    assert!(relay_reachable(ports[2]).await);
    manager.shutdown().await.unwrap();
    wait_terminal(&manager, &project.id, "native-b1", "stopped").await;
    bridge.set_enabled(false).await.unwrap();
    let _ = client.cancel().await;
    println!("native VPN relay proof passed: native processes, two isolated profiles, shared gateway, loopback cleanup and fail-closed gateway loss");
}

#[tokio::test]
#[ignore = "requires local Docker, OpenSSL, Gluetun v3.41.0 and alpine:3.23.3"]
async fn vpn_manager_uses_certificate_only_encrypted_key() {
    let fixture = match std::env::var_os("SM_VPN_FIXTURE_DIR") {
        Some(path) => PathBuf::from(path),
        None => {
            let script = Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../.agents/evidence/TASK-0018/vpn-connected-proof.py");
            let status = Command::new("python3")
                .arg(script)
                .arg(std::env::current_exe().unwrap())
                .arg("vpn_manager_uses_certificate_only_encrypted_key")
                .status()
                .unwrap();
            assert!(
                status.success(),
                "synthetic encrypted certificate proof failed"
            );
            return;
        }
    };
    let server = std::env::var("SM_VPN_FIXTURE_SERVER").unwrap();
    let state = fixture.join("state");
    let root = fixture.join("project");
    std::fs::create_dir(&root).unwrap();
    let manager = Manager::new(state.clone()).unwrap();
    let project = manager.open(root.to_str().unwrap()).await.unwrap();
    let passphrase = std::fs::read_to_string(fixture.join("key-passphrase")).unwrap();
    let octet = 2 + uuid::Uuid::new_v4().as_bytes()[0] % 240;
    let endpoints = [
        format!("10.232.{octet}.2"),
        format!("10.232.{}.2", octet + 1),
    ];
    let mut profiles = Vec::new();
    for (index, name) in ["Encrypted certificate", "Wrong key passphrase"]
        .iter()
        .enumerate()
    {
        let config = std::fs::read_to_string(fixture.join("client-certonly.ovpn"))
            .unwrap()
            .replace(
                "remote synthetic-vpn-server 1194",
                &format!("remote {} 1194", endpoints[index]),
            );
        assert!(!config.contains("auth-user-pass"));
        assert!(config.contains("BEGIN ENCRYPTED PRIVATE KEY"));
        let path = fixture.join(format!("certonly-{index}.ovpn"));
        secure_write(&path, config.as_bytes()).unwrap();
        profiles.push(
            manager
                .vpns
                .import(VpnImport {
                    name: (*name).into(),
                    config_path: path.to_str().unwrap().into(),
                    username: String::new(),
                    password: String::new(),
                    key_passphrase: if index == 0 {
                        passphrase.clone()
                    } else {
                        "synthetic-wrong-passphrase".into()
                    },
                    published_ports: vec![],
                    outbound_subnets: vec![],
                    health_target: "10.66.0.1:5432".into(),
                    dns_address: String::new(),
                    health_icmp_target: "10.66.0.1".into(),
                })
                .await
                .unwrap(),
        );
    }
    for profile in &profiles {
        let public = serde_json::to_string(profile).unwrap();
        assert!(!public.contains(&passphrase));
        assert!(!public.contains("PRIVATE KEY"));
        assert!(!public.contains("synthetic-wrong-passphrase"));
        assert!(!state
            .join("vpn")
            .join(&profile.id)
            .join("username")
            .exists());
        assert!(!state
            .join("vpn")
            .join(&profile.id)
            .join("password")
            .exists());
    }
    let owner = std::fs::read_to_string(state.join("vpn/owner")).unwrap();
    let metadata: Vec<Value> =
        serde_json::from_slice(&std::fs::read(state.join("vpn/profiles.json")).unwrap()).unwrap();
    let mut cleanup = Cleanup {
        stacks: vec![],
        gateways: vec![],
        networks: vec![],
        server,
    };
    for (index, p) in metadata.iter().enumerate() {
        let id = p["id"].as_str().unwrap();
        let fingerprint = p["fingerprint"].as_str().unwrap();
        let name = format!("sm-vpn-{}-{id}", &owner[..8]);
        let network = format!("{name}-net");
        cleanup.gateways.push(name);
        cleanup.networks.push(network.clone());
        docker(&[
            "network",
            "create",
            "--driver",
            "bridge",
            "--subnet",
            &format!("10.232.{}.0/24", octet + index as u8),
            "--label",
            &format!("io.service-manager.vpn.owner={owner}"),
            "--label",
            &format!("io.service-manager.vpn.profile={id}"),
            "--label",
            &format!("io.service-manager.vpn.fingerprint={fingerprint}"),
            &network,
        ]);
        docker(&[
            "network",
            "connect",
            "--ip",
            &endpoints[index],
            &network,
            &cleanup.server,
        ]);
    }
    let definition = ServiceDefinition {
        id: "encrypted".into(),
        name: "Encrypted certificate worker".into(),
        cwd: ".".into(),
        kind: ServiceKind::Command {
            command: "echo vpn-worker-marker; exec sleep 300".into(),
        },
        env_file: None,
        vpn: Some(VpnService {
            profile_id: profiles[0].id.clone(),
            image: "alpine:3.23.3".into(),
        }),
        vpn_routes: vec![],
        health_check: None,
        ports: vec![],
        url: None,
        depends_on: vec![],
    };
    let saved = manager
        .save(&project.id, definition, &project.revision, false)
        .await
        .unwrap();
    manager
        .store
        .lock()
        .await
        .trust(&project.id, "encrypted", &saved.revision)
        .unwrap();
    cleanup.stacks.push(format!(
        "sm-{}",
        &hash(format!("{}/encrypted", project.id).as_bytes())[..24]
    ));
    manager
        .start(&project.id, "encrypted", "test")
        .await
        .unwrap();
    wait_marker(&manager, &project.id, "encrypted").await;
    let worker = worker_id(&cleanup.stacks[0]);
    assert!(reachable(&worker));
    let inspected: Value =
        serde_json::from_str(&docker(&["inspect", &cleanup.gateways[0]])).unwrap();
    let process_public = serde_json::to_string(&inspected[0]["Config"]).unwrap();
    assert!(!process_public.contains(&passphrase));
    assert!(!process_public.contains("synthetic-wrong-passphrase"));
    assert!(!process_public.contains("synthetic-user"));
    assert!(!process_public.contains("synthetic-password"));
    assert!(process_public.contains("OPENVPN_KEY_PASSPHRASE_SECRETFILE"));
    assert!(!process_public.contains("/run/secrets/vpn_username"));
    assert!(!process_public.contains("/run/secrets/vpn_password"));
    let mounts = serde_json::to_string(&inspected[0]["Mounts"]).unwrap();
    assert!(!mounts.contains("/run/secrets/vpn_username"));
    assert!(!mounts.contains("/run/secrets/vpn_password"));
    let rejected = manager.vpns.connect(&profiles[1].id).await;
    assert!(
        rejected.is_err(),
        "wrong passphrase must not establish a gateway"
    );
    let rejected = match rejected {
        Err(e) => e,
        Ok(_) => unreachable!(),
    };
    assert!(!rejected.message.contains(&passphrase));
    assert!(!rejected.message.contains("synthetic-wrong-passphrase"));
    assert!(
        reachable(&worker),
        "failed other-profile auth must not interrupt the ready profile"
    );
    manager.shutdown().await.unwrap();
    wait_terminal(&manager, &project.id, "encrypted", "stopped").await;
    println!("encrypted certificate-only VPN proof passed: private passphrase, no username/password, worker reachable, public metadata redacted, wrong passphrase rejected");
}

#[tokio::test]
#[ignore = "requires local Docker, OpenSSL, Gluetun v3.41.0 and alpine:3.23.3"]
async fn vpn_manager_handles_real_crv1_mfa() {
    let fixture = match std::env::var_os("SM_VPN_FIXTURE_DIR") {
        Some(path) => PathBuf::from(path),
        None => {
            let script = Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../.agents/evidence/TASK-0018/vpn-connected-proof.py");
            let status = Command::new("python3")
                .arg(script)
                .arg(std::env::current_exe().unwrap())
                .arg("vpn_manager_handles_real_crv1_mfa")
                .status()
                .unwrap();
            assert!(status.success(), "synthetic real CRV1 MFA proof failed");
            return;
        }
    };
    let server = std::env::var("SM_VPN_FIXTURE_SERVER").unwrap();
    let state = fixture.join("state");
    let root = fixture.join("project");
    std::fs::create_dir(&root).unwrap();
    let manager = Manager::new(state.clone()).unwrap();
    let project = manager.open(root.to_str().unwrap()).await.unwrap();
    let octet = 2 + uuid::Uuid::new_v4().as_bytes()[0] % 238;
    let mut profiles = Vec::new();
    for (index, name) in ["Unrelated certificate", "CRV1 MFA", "Other MFA profile"]
        .iter()
        .enumerate()
    {
        let base = if index == 0 {
            "client-certonly.ovpn"
        } else {
            "client.ovpn"
        };
        let config = std::fs::read_to_string(fixture.join(base))
            .unwrap()
            .replace(
                "remote synthetic-vpn-server 1194",
                &format!(
                    "remote 10.233.{}.2 {}",
                    octet + index as u8,
                    if index == 0 { 1194 } else { 1195 }
                ),
            );
        let path = fixture.join(format!("mfa-client-{index}.ovpn"));
        secure_write(&path, config.as_bytes()).unwrap();
        profiles.push(
            manager
                .vpns
                .import(VpnImport {
                    name: (*name).into(),
                    config_path: path.to_str().unwrap().into(),
                    username: if index == 0 {
                        String::new()
                    } else {
                        "synthetic-user".into()
                    },
                    password: if index == 0 {
                        String::new()
                    } else {
                        "synthetic-password".into()
                    },
                    key_passphrase: if index == 0 {
                        std::fs::read_to_string(fixture.join("key-passphrase")).unwrap()
                    } else {
                        String::new()
                    },
                    published_ports: vec![],
                    outbound_subnets: vec![],
                    health_target: format!("10.{}.0.1:5432", if index == 0 { 66 } else { 67 }),
                    dns_address: String::new(),
                    health_icmp_target: format!("10.{}.0.1", if index == 0 { 66 } else { 67 }),
                })
                .await
                .unwrap(),
        );
    }
    let owner = std::fs::read_to_string(state.join("vpn/owner")).unwrap();
    let metadata: Vec<Value> =
        serde_json::from_slice(&std::fs::read(state.join("vpn/profiles.json")).unwrap()).unwrap();
    let mut cleanup = Cleanup {
        stacks: vec![],
        gateways: vec![],
        networks: vec![],
        server,
    };
    for (index, p) in metadata.iter().enumerate() {
        let id = p["id"].as_str().unwrap();
        let fingerprint = p["fingerprint"].as_str().unwrap();
        let name = format!("sm-vpn-{}-{id}", &owner[..8]);
        let network = format!("{name}-net");
        cleanup.gateways.push(name);
        cleanup.networks.push(network.clone());
        docker(&[
            "network",
            "create",
            "--driver",
            "bridge",
            "--subnet",
            &format!("10.233.{}.0/24", octet + index as u8),
            "--label",
            &format!("io.service-manager.vpn.owner={owner}"),
            "--label",
            &format!("io.service-manager.vpn.profile={id}"),
            "--label",
            &format!("io.service-manager.vpn.fingerprint={fingerprint}"),
            &network,
        ]);
        docker(&[
            "network",
            "connect",
            "--ip",
            &format!("10.233.{}.2", octet + index as u8),
            &network,
            &cleanup.server,
        ]);
    }
    for (index, id) in ["cert", "mfa"].iter().enumerate() {
        let definition = ServiceDefinition {
            id: (*id).into(),
            name: (*id).into(),
            cwd: ".".into(),
            kind: ServiceKind::Command {
                command: "echo vpn-worker-marker; exec sleep 300".into(),
            },
            env_file: None,
            vpn: Some(VpnService {
                profile_id: profiles[index].id.clone(),
                image: "alpine:3.23.3".into(),
            }),
            vpn_routes: vec![],
            health_check: None,
            ports: vec![],
            url: None,
            depends_on: vec![],
        };
        let revision = manager.store.lock().await.projects[&project.id]
            .revision
            .clone();
        let saved = manager
            .save(&project.id, definition, &revision, false)
            .await
            .unwrap();
        manager
            .store
            .lock()
            .await
            .trust(&project.id, id, &saved.revision)
            .unwrap();
        cleanup.stacks.push(format!(
            "sm-{}",
            &hash(format!("{}/{id}", project.id).as_bytes())[..24]
        ));
    }
    let bridge = manager.vpns.clone();
    let mfa_profile = profiles[1].id.clone();
    let connect = tokio::spawn(async move { bridge.connect(&mfa_profile).await });
    let mut challenge = None;
    for _ in 0..600 {
        let views = manager.vpns.list().await.unwrap();
        let mfa = views.iter().find(|p| p.id == profiles[1].id).unwrap();
        if let Some(value) = mfa.auth_challenge.clone() {
            assert_eq!(mfa.state, "authRequired");
            challenge = Some(value);
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    let challenge = challenge.expect("real OpenVPN CRV1 challenge must reach the manager");
    assert_eq!(challenge.prompt, "Enter your VPN MFA/PIN code.");
    assert_ne!(challenge.id, "synthetic-crv1-state");
    manager.start(&project.id, "cert", "test").await.unwrap();
    wait_marker(&manager, &project.id, "cert").await;
    let cert_worker = worker_id(&cleanup.stacks[0]);
    assert!(reachable(&cert_worker));

    assert!(
        reachable(&cert_worker),
        "unrelated VPN must work while waiting for MFA"
    );
    assert!(manager
        .vpns
        .respond_challenge(&profiles[1].id, "stale-challenge", "424242".into())
        .await
        .is_err());
    let other_bridge = manager.vpns.clone();
    let other_profile = profiles[2].id.clone();
    let other_connect = tokio::spawn(async move { other_bridge.connect(&other_profile).await });
    let mut other_challenge = None;
    for _ in 0..600 {
        let views = manager.vpns.list().await.unwrap();
        let mfa = views.iter().find(|p| p.id == profiles[2].id).unwrap();
        if let Some(value) = mfa.auth_challenge.clone() {
            other_challenge = Some(value);
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    let other_challenge =
        other_challenge.expect("second profile must independently reach MFA challenge");
    assert_ne!(other_challenge.id, challenge.id);
    assert!(manager
        .vpns
        .respond_challenge(&profiles[2].id, &challenge.id, "424242".into())
        .await
        .is_err());
    tokio::time::timeout(
        Duration::from_secs(10),
        manager.vpns.disconnect(&profiles[2].id),
    )
    .await
    .expect("pending MFA cancellation must not wait for the PIN deadline")
    .unwrap();
    assert!(tokio::time::timeout(Duration::from_secs(5), other_connect)
        .await
        .unwrap()
        .unwrap()
        .is_err());
    assert!(manager
        .vpns
        .respond_challenge(&profiles[2].id, &other_challenge.id, "424242".into())
        .await
        .is_err());
    assert!(reachable(&cert_worker));
    let public = serde_json::to_string(&manager.vpns.list().await.unwrap()).unwrap();
    for secret in [
        "424242",
        "synthetic-password",
        "synthetic-user",
        "synthetic-crv1-state",
    ] {
        assert!(
            !public.contains(secret),
            "public challenge metadata must exclude auth secrets"
        );
    }
    manager
        .vpns
        .respond_challenge(&profiles[1].id, &challenge.id, "424242".into())
        .await
        .unwrap();
    assert!(
        manager
            .vpns
            .respond_challenge(&profiles[1].id, &challenge.id, "424242".into())
            .await
            .is_err(),
        "challenge response must be single use"
    );
    let connected = tokio::time::timeout(Duration::from_secs(90), connect)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    assert_eq!(connected.state, "ready");
    assert!(connected.auth_challenge.is_none());
    manager.start(&project.id, "mfa", "test").await.unwrap();
    wait_marker(&manager, &project.id, "mfa").await;
    let mfa_worker = worker_id(&cleanup.stacks[1]);
    let response = Command::new("docker")
        .args([
            "exec",
            &mfa_worker,
            "wget",
            "-T",
            "3",
            "-qO-",
            "http://10.67.0.1:5432",
        ])
        .output()
        .unwrap();
    assert!(response.status.success());
    assert_eq!(
        String::from_utf8_lossy(&response.stdout).trim(),
        "synthetic-ok"
    );
    let inspected = docker(&["inspect", &cleanup.gateways[1]]);
    assert!(!inspected.contains("424242"));
    assert!(!inspected.contains("synthetic-password"));
    let logs = Command::new("docker")
        .args(["logs", &cleanup.gateways[1]])
        .output()
        .unwrap();
    assert!(!String::from_utf8_lossy(&logs.stdout).contains("424242"));
    assert!(!String::from_utf8_lossy(&logs.stderr).contains("424242"));
    assert!(reachable(&cert_worker));
    manager.shutdown().await.unwrap();
    wait_terminal(&manager, &project.id, "cert", "stopped").await;
    wait_terminal(&manager, &project.id, "mfa", "stopped").await;
    println!("real CRV1 MFA proof passed: safe challenge, explicit PIN response, stale/cross-profile/replayed rejection, no PIN in public metadata or Docker logs, unrelated VPN starts while awaiting PIN, second pending MFA cancels immediately");
}
