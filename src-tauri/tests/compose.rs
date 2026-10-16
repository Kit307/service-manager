use service_manager::{manager::Manager, model::*, project::hash};
use std::{process::Command, time::Duration};

fn docker(args: &[&str]) -> String {
    let output = Command::new("/opt/homebrew/bin/docker")
        .args(args)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "isolated Docker fixture operation failed"
    );
    String::from_utf8(output.stdout).unwrap().trim().into()
}

// Explicit opt-in: this test creates only synthetic, uniquely named local resources.
#[tokio::test]
#[ignore = "requires local Docker and the already installed alpine:3.23.3 image"]
async fn compose_owns_one_stack_and_stop_uses_the_launch_snapshot() {
    let root = tempfile::tempdir().unwrap();
    let state = tempfile::tempdir().unwrap();
    let manager = Manager::new(state.path().into()).unwrap();
    let p = manager.open(root.path().to_str().unwrap()).await.unwrap();
    let name = format!("sm-{}", &hash(format!("{}/stack", p.id).as_bytes())[..24]);
    struct Cleanup(String);
    impl Drop for Cleanup {
        fn drop(&mut self) {
            let ids = docker(&[
                "ps",
                "-aq",
                "--filter",
                &format!("label=com.docker.compose.project={}", self.0),
            ]);
            for id in ids.lines() {
                docker(&["rm", "-f", id]);
            }
            let networks = docker(&[
                "network",
                "ls",
                "-q",
                "--filter",
                &format!("label=com.docker.compose.project={}", self.0),
            ]);
            for id in networks.lines() {
                docker(&["network", "rm", id]);
            }
            let volumes = docker(&[
                "volume",
                "ls",
                "-q",
                "--filter",
                &format!("label=com.docker.compose.project={}", self.0),
            ]);
            for id in volumes.lines() {
                docker(&["volume", "rm", id]);
            }
        }
    }
    let _cleanup = Cleanup(name.clone());
    let compose = "services:\n  worker:\n    image: alpine:3.23.3\n    command: [sh, -c, 'echo compose-marker; test -f /data/persist && echo retained-volume; touch /data/persist; exec sleep 300']\n    volumes: [data:/data]\nvolumes:\n  data: {}\n";
    std::fs::write(root.path().join("compose.yaml"), compose).unwrap();
    let definition = ServiceDefinition {
        id: "stack".into(),
        name: "Isolated fixture".into(),
        cwd: ".".into(),
        kind: ServiceKind::Compose {
            file: "compose.yaml".into(),
        },
        env_file: None,
        health_check: None,
    };
    let saved = manager
        .save(&p.id, definition, "absent", false)
        .await
        .unwrap();
    manager
        .store
        .lock()
        .await
        .trust(&p.id, "stack", &saved.revision)
        .unwrap();
    // Pre-existing data without our private ownership label must never be adopted.
    let volume_name = format!("{name}_data");
    docker(&["volume", "create", &volume_name]);
    manager.start(&p.id, "stack", "test").await.unwrap();
    for _ in 0..150 {
        if manager.status(&p.id, "stack").await.unwrap().state == "failed" {
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    let refused = manager.status(&p.id, "stack").await.unwrap();
    assert_eq!(refused.state, "failed");
    assert!(refused.error.unwrap().contains("ownership label"));
    assert!(docker(&[
        "ps",
        "-aq",
        "--filter",
        &format!("label=com.docker.compose.project={name}")
    ])
    .is_empty());
    docker(&["volume", "rm", &volume_name]);
    // A stop admitted during preflight cannot subsequently start containers.
    manager.start(&p.id, "stack", "test").await.unwrap();
    manager.stop(&p.id, "stack").await.unwrap();
    for _ in 0..200 {
        if !manager.status(&p.id, "stack").await.unwrap().active() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    assert_eq!(
        manager.status(&p.id, "stack").await.unwrap().state,
        "stopped"
    );
    assert!(docker(&[
        "ps",
        "-q",
        "--filter",
        &format!("label=com.docker.compose.project={name}")
    ])
    .is_empty());
    let first = manager.start(&p.id, "stack", "test").await.unwrap();
    let second = manager.start(&p.id, "stack", "test").await.unwrap();
    assert_eq!(first.run_id, second.run_id);
    for _ in 0..100 {
        if manager
            .read_logs(&p.id, "stack", None)
            .await
            .unwrap()
            .records
            .iter()
            .any(|l| l.text.contains("compose-marker"))
        {
            break;
        }
        assert_ne!(
            manager.status(&p.id, "stack").await.unwrap().state,
            "failed"
        );
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    assert!(manager
        .read_logs(&p.id, "stack", None)
        .await
        .unwrap()
        .records
        .iter()
        .any(|l| l.text.contains("compose-marker")));
    let ids = docker(&[
        "ps",
        "-q",
        "--filter",
        &format!("label=com.docker.compose.project={name}"),
    ]);
    assert_eq!(ids.lines().count(), 1);
    // An edited source must not redirect Stop or prevent shutdown.
    std::fs::write(root.path().join("compose.yaml"), "invalid: [compose").unwrap();
    manager.stop(&p.id, "stack").await.unwrap();
    for _ in 0..300 {
        if !manager.status(&p.id, "stack").await.unwrap().active() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    assert_eq!(
        manager.status(&p.id, "stack").await.unwrap().state,
        "stopped"
    );
    assert_eq!(
        docker(&["inspect", "--format", "{{.State.Running}}", &ids]),
        "false"
    );
    assert!(!docker(&[
        "volume",
        "ls",
        "-q",
        "--filter",
        &format!("label=com.docker.compose.project={name}")
    ])
    .is_empty());
    std::fs::write(root.path().join("compose.yaml"), compose).unwrap();
    manager.start(&p.id, "stack", "test").await.unwrap();
    for _ in 0..150 {
        if manager
            .read_logs(&p.id, "stack", None)
            .await
            .unwrap()
            .records
            .iter()
            .any(|l| l.text.contains("retained-volume"))
        {
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    assert!(manager
        .read_logs(&p.id, "stack", None)
        .await
        .unwrap()
        .records
        .iter()
        .any(|l| l.text.contains("retained-volume")));
    manager.shutdown().await.unwrap();
    assert_eq!(
        manager.status(&p.id, "stack").await.unwrap().state,
        "stopped"
    );
}
