use service_manager::{manager::Manager, model::ServiceDefinition};
use std::time::Duration;

async fn fixture(
    kind: serde_json::Value,
) -> (tempfile::TempDir, tempfile::TempDir, Manager, String) {
    let root = tempfile::tempdir().unwrap();
    let state = tempfile::tempdir().unwrap();
    let manager = Manager::new(state.path().into()).unwrap();
    let project = manager.open(root.path().to_str().unwrap()).await.unwrap();
    let definition: ServiceDefinition = serde_json::from_value(kind).unwrap();
    manager
        .save(&project.id, definition, &project.revision, true)
        .await
        .unwrap();
    (root, state, manager, project.id)
}
async fn archived(manager: &Manager, project: &str, run: &str) {
    tokio::time::timeout(Duration::from_secs(8), async {
        loop {
            if manager
                .run_details(project, "once", run)
                .await
                .unwrap()
                .summary
                .ended_at
                .is_some()
            {
                return;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("run was not archived");
}
#[tokio::test]
async fn exiting_run_is_archived_even_when_next_run_replaces_runtime() {
    let (_root,_state,manager,project) = fixture(serde_json::json!({"id":"once","name":"once","cwd":".","type":"command","command":"exit 0"})).await;
    let first = manager
        .start(&project, "once", "test")
        .await
        .unwrap()
        .run_id
        .unwrap();
    // Block the final system log after process completion, then start the next run.
    let logs = manager.logs.lock().await;
    tokio::time::timeout(Duration::from_secs(8), async {
        while manager.status(&project, "once").await.unwrap().active() {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .unwrap();
    let m = manager.clone();
    let p = project.clone();
    let second = tokio::spawn(async move { m.start(&p, "once", "test").await.unwrap() });
    tokio::time::timeout(Duration::from_secs(8), async {
        while manager
            .status(&project, "once")
            .await
            .unwrap()
            .run_id
            .as_deref()
            == Some(&first)
        {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .unwrap();
    drop(logs);
    let second = second.await.unwrap().run_id.unwrap();
    archived(&manager, &project, &first).await;
    archived(&manager, &project, &second).await;
    for run in [&first, &second] {
        let record = manager.run_details(&project, "once", run).await.unwrap();
        assert_eq!(record.summary.state, "stopped");
        assert!(record.records.iter().all(|r| r.run_id == *run));
    }
}
#[tokio::test]
async fn failed_history_write_does_not_publish_compose_runtime() {
    let root = tempfile::tempdir().unwrap();
    let state = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("compose.yaml"), "services: {}\n").unwrap();
    let manager = Manager::new(state.path().into()).unwrap();
    let project = manager.open(root.path().to_str().unwrap()).await.unwrap();
    let definition:ServiceDefinition=serde_json::from_value(serde_json::json!({"id":"once","name":"once","cwd":".","type":"compose","file":"compose.yaml"})).unwrap();
    manager
        .save(&project.id, definition, &project.revision, true)
        .await
        .unwrap();
    std::fs::create_dir(state.path().join("history.json")).unwrap();
    assert!(manager.start(&project.id, "once", "test").await.is_err());
    assert!(!manager.status(&project.id, "once").await.unwrap().active());
    assert!(manager
        .run_history(&project.id, "once")
        .await
        .unwrap()
        .is_empty());
}
#[tokio::test]
async fn failed_command_exit_has_terminal_history() {
    let (_root,_state,manager,project) = fixture(serde_json::json!({"id":"once","name":"once","cwd":".","type":"command","command":"exit 23"})).await;
    let run = manager
        .start(&project, "once", "test")
        .await
        .unwrap()
        .run_id
        .unwrap();
    archived(&manager, &project, &run).await;
    let record = manager.run_details(&project, "once", &run).await.unwrap();
    assert_eq!(record.summary.state, "failed");
    assert_eq!(record.summary.exit_code, Some(23));
}

#[tokio::test]
async fn blocked_logs_preserve_start_order_and_final_output_in_history() {
    let (root, _state, manager, project) = fixture(serde_json::json!({
        "id":"once", "name":"once", "cwd":".", "type":"command",
        "command":"printf 'final-output\\n'; touch completed"
    }))
    .await;
    let logs = manager.logs.lock().await;
    let run = manager
        .start(&project, "once", "test")
        .await
        .unwrap()
        .run_id
        .unwrap();
    tokio::time::timeout(Duration::from_secs(5), async {
        while !root.path().join("completed").exists() {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    tokio::time::sleep(Duration::from_millis(2500)).await;
    drop(logs);
    archived(&manager, &project, &run).await;
    let record = manager.run_details(&project, "once", &run).await.unwrap();
    let lines: Vec<_> = record.records.iter().map(|r| r.text.as_str()).collect();
    let started = lines
        .iter()
        .position(|s| s.contains("Started service"))
        .unwrap();
    let output = lines
        .iter()
        .position(|s| s.contains("final-output"))
        .unwrap();
    assert!(started < output);
}

#[tokio::test]
async fn escaped_process_holding_stdout_does_not_strand_history() {
    struct Escaped(std::path::PathBuf);
    impl Drop for Escaped {
        fn drop(&mut self) {
            if let Ok(pid) = std::fs::read_to_string(&self.0) {
                if let Ok(pid) = pid.parse::<i32>() {
                    let _ = nix::sys::signal::kill(
                        nix::unistd::Pid::from_raw(pid),
                        nix::sys::signal::Signal::SIGKILL,
                    );
                }
            }
        }
    }
    let (root, _state, manager, project) = fixture(serde_json::json!({
        "id":"once", "name":"once", "cwd":".", "type":"command",
        "command":"/usr/bin/python3 -c 'import os,time; r,w=os.pipe(); p=os.fork(); p and (os.close(w),os.read(r,1),os._exit(0)); os.close(r); os.setsid(); open(\"escaped.pid\",\"w\").write(str(os.getpid())); print(\"before-cutoff\",end=\"\",flush=True); os.write(w,b\"x\"); os.close(w); time.sleep(60)'"
    })).await;
    let _escaped = Escaped(root.path().join("escaped.pid"));
    let logs = manager.logs.lock().await;
    let run = manager
        .start(&project, "once", "test")
        .await
        .unwrap()
        .run_id
        .unwrap();
    tokio::time::timeout(Duration::from_secs(5), async {
        while !root.path().join("escaped.pid").exists() {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    tokio::time::sleep(Duration::from_millis(2500)).await;
    drop(logs);
    archived(&manager, &project, &run).await;
    let record = manager.run_details(&project, "once", &run).await.unwrap();
    let lines: Vec<_> = record.records.iter().map(|r| r.text.as_str()).collect();
    let output = lines
        .iter()
        .position(|s| s.contains("before-cutoff"))
        .unwrap();
    let exited = lines
        .iter()
        .position(|s| s.contains("Process exited"))
        .unwrap();
    assert!(output < exited);
    assert!(lines
        .iter()
        .any(|s| s.contains("output") && s.contains("incomplete")));
}
