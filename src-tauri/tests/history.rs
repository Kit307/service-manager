use service_manager::{
    history::{redacted_diagnostics, HistoryStore},
    logs::{LogPage, LogRecord},
    model::Snapshot,
};
use std::fs;
fn snapshot(run: &str) -> Snapshot {
    serde_json::from_value(serde_json::json!({"state":"failed","runId":run,"exitCode":2,"health":"unknown","activity":"idle"})).unwrap()
}
fn page(run: &str, count: usize, text: &str) -> LogPage {
    LogPage {
        records: (0..count)
            .map(|seq| LogRecord {
                seq: seq as u64,
                run_id: run.into(),
                timestamp: "2026-01-01T00:00:00Z".into(),
                stream: "stderr".into(),
                text: text.into(),
                truncated: false,
            })
            .collect(),
        next_cursor: count.to_string(),
        truncated: false,
    }
}
#[test]
fn persists_completed_and_interrupts_pending_without_process_claims() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = HistoryStore::open(dir.path()).unwrap();
    store.begin("p", "s", "one", "run").unwrap();
    store
        .finish("p", "s", "one", &snapshot("one"), page("one", 1, "error"))
        .unwrap();
    store.begin("p", "s", "two", "run").unwrap();
    let store = HistoryStore::open(dir.path()).unwrap();
    let runs = store.list("p", "s");
    assert_eq!(runs[0].run_id, "two");
    assert_eq!(runs[0].state, "interrupted");
    assert!(runs[0].ended_at.is_some());
    assert_eq!(
        store.get("p", "s", "one").unwrap().summary.exit_code,
        Some(2)
    );
    assert_eq!(store.get("p", "s", "one").unwrap().records.len(), 1);
    use std::os::unix::fs::PermissionsExt;
    assert_eq!(
        fs::metadata(dir.path().join("history.json"))
            .unwrap()
            .permissions()
            .mode()
            & 0o777,
        0o600
    );
}
#[test]
fn limits_history_and_log_bytes() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = HistoryStore::open(dir.path()).unwrap();
    for i in 0..25 {
        store.begin("p", "s", &format!("r{i}"), "run").unwrap();
    }
    assert_eq!(store.list("p", "s").len(), 20);
    store
        .finish(
            "p",
            "s",
            "r24",
            &snapshot("r24"),
            page("r24", 100, &"x".repeat(2000)),
        )
        .unwrap();
    let run = store.get("p", "s", "r24").unwrap();
    assert!(run.summary.truncated);
    assert!(serde_json::to_vec(&run.records).unwrap().len() < 65536);
    for i in 0..110 {
        store.begin("p", &format!("s{i}"), "r", "run").unwrap();
    }
    let all: serde_json::Value =
        serde_json::from_slice(&fs::read(dir.path().join("history.json")).unwrap()).unwrap();
    assert_eq!(all.as_array().unwrap().len(), 100);
    assert!(HistoryStore::open(dir.path()).is_ok());
}
#[test]
fn stale_finish_never_overwrites_results_or_other_runs() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = HistoryStore::open(dir.path()).unwrap();
    store.begin("p", "s", "one", "run").unwrap();
    store.begin("p", "s", "two", "run").unwrap();
    store
        .finish("p", "s", "one", &snapshot("two"), page("two", 1, "wrong"))
        .unwrap();
    assert!(store
        .get("p", "s", "one")
        .unwrap()
        .summary
        .ended_at
        .is_none());
    store
        .finish("p", "s", "one", &snapshot("one"), page("one", 1, "first"))
        .unwrap();
    store
        .finish(
            "p",
            "s",
            "one",
            &snapshot("one"),
            page("one", 1, "overwritten"),
        )
        .unwrap();
    assert_eq!(store.get("p", "s", "one").unwrap().records[0].text, "first");
    assert!(store
        .get("p", "s", "two")
        .unwrap()
        .summary
        .ended_at
        .is_none());
}
#[test]
fn failed_persistence_is_transactional_and_corruption_preserved() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = HistoryStore::open(dir.path()).unwrap();
    store.begin("p", "s", "one", "run").unwrap();
    fs::remove_file(dir.path().join("history.json")).unwrap();
    fs::create_dir(dir.path().join("history.json")).unwrap();
    assert!(store.begin("p", "s", "two", "run").is_err());
    assert_eq!(store.list("p", "s").len(), 1);
    assert!(store
        .finish("p", "s", "one", &snapshot("one"), page("one", 1, "error"))
        .is_err());
    assert!(store
        .get("p", "s", "one")
        .unwrap()
        .summary
        .ended_at
        .is_none());
    fs::remove_dir(dir.path().join("history.json")).unwrap();
    fs::write(dir.path().join("history.json"), "SENSITIVE broken json").unwrap();
    let err = HistoryStore::open(dir.path()).err().unwrap();
    assert!(!err.message.contains("SENSITIVE"));
    assert_eq!(
        fs::read_to_string(dir.path().join("history.json")).unwrap(),
        "SENSITIVE broken json"
    );
}
#[test]
fn rejects_symbolic_links_and_oversized_history() {
    let dir = tempfile::tempdir().unwrap();
    let target = dir.path().join("target");
    fs::write(&target, "[]").unwrap();
    std::os::unix::fs::symlink(&target, dir.path().join("history.json")).unwrap();
    assert!(HistoryStore::open(dir.path()).is_err());
    fs::remove_file(dir.path().join("history.json")).unwrap();
    let f = fs::File::create(dir.path().join("history.json")).unwrap();
    f.set_len(17 * 1024 * 1024).unwrap();
    assert!(HistoryStore::open(dir.path()).is_err());
}
#[test]
fn evidence_redacts_secrets_bounds_recent_output_and_offers_factual_hints() {
    let secret="Authorization: Bearer abc123\nPASSWORD=hunter2\nhttps://user:pass@example.test/path\neyJabc.def.ghi\n-----BEGIN PRIVATE KEY-----\nPRIVATEBODY\n-----END PRIVATE KEY-----";
    let mut state = snapshot("r");
    state.error = Some(secret.into());
    let out = redacted_diagnostics("p", "s", &state, page("r", 1, secret));
    let json = out.to_string();
    for secret in ["abc123", "hunter2", "user:pass", "eyJabc", "PRIVATEBODY"] {
        assert!(!json.contains(secret));
    }
    assert_eq!(out["redacted"], true);
    let out = redacted_diagnostics("p", "s", &state, page("r", 250, "permission denied"));
    assert_eq!(out["records"].as_array().unwrap().len(), 100);
    assert_eq!(out["truncated"], true);
    assert!(out["summary"].to_string().contains("Hint:"));
    let out = redacted_diagnostics("p", "s", &state, page("r", 100, &"x".repeat(2000)));
    assert!(serde_json::to_vec(&out["records"]).unwrap().len() < 32 * 1024);
}

#[test]
fn multiline_key_chunks_and_escaped_output_stay_redacted_and_bounded() {
    let mut logs = page("r", 3, "");
    logs.records[0].text = "-----BEGIN PRIVATE KEY-----".into();
    logs.records[1].text = "TOPSECRETKEYBODY".into();
    logs.records[2].text = "-----END PRIVATE KEY-----".into();
    let out = redacted_diagnostics("p", "s", &snapshot("r"), logs);
    assert!(!out.to_string().contains("TOPSECRETKEYBODY"));
    let out = redacted_diagnostics(
        "p",
        "s",
        &snapshot("r"),
        page("r", 100, &"\\\"".repeat(2000)),
    );
    assert!(serde_json::to_vec(&out["records"]).unwrap().len() <= 32 * 1024);
}
#[test]
fn invalid_schema_does_not_get_repaired_or_overwritten() {
    let dir = tempfile::tempdir().unwrap();
    let text = r#"[{"projectId":"p","serviceId":"s","runId":"r","command":"run","startedAt":"now","endedAt":null,"state":"starting","exitCode":null,"error":null,"truncated":false,"records":[],"unknown":"sensitive"}]"#;
    fs::write(dir.path().join("history.json"), text).unwrap();
    assert!(HistoryStore::open(dir.path()).is_err());
    assert_eq!(
        fs::read_to_string(dir.path().join("history.json")).unwrap(),
        text
    );
}

#[test]
fn persisted_commands_errors_and_records_redact_credentials() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = HistoryStore::open(dir.path()).unwrap();
    store
        .begin("p", "s", "r", "API_KEY=COMMANDSECRET run")
        .unwrap();
    let mut state = snapshot("r");
    state.error = Some("password=ERRORSECRET".into());
    store
        .finish(
            "p",
            "s",
            "r",
            &state,
            page("r", 1, "Authorization: Bearer LOGSECRET"),
        )
        .unwrap();
    let bytes = fs::read_to_string(dir.path().join("history.json")).unwrap();
    for secret in ["COMMANDSECRET", "ERRORSECRET", "LOGSECRET"] {
        assert!(!bytes.contains(secret));
    }
    assert!(HistoryStore::open(dir.path()).is_ok());
}
