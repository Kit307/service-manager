use service_manager::logs::LogStore;
#[test]
fn output_is_bounded_and_cursor_reads_do_not_repeat_lines() {
    let mut logs = LogStore::default();
    logs.push("p/web", "run1", "stdout", "first");
    let first = logs.read("p/web", None);
    assert_eq!(first.records.len(), 1);
    logs.push("p/web", "run1", "stderr", "second");
    let next = logs.read("p/web", Some(&first.next_cursor));
    assert_eq!(next.records.len(), 1);
    assert_eq!(next.records[0].text, "second");
    for _ in 0..6000 {
        logs.push("p/web", "run1", "stdout", "bounded");
    }
    assert!(logs.read("p/web", Some("0")).truncated);
}
#[test]
fn huge_single_records_are_truncated_and_clear_does_not_reuse_cursors() {
    let mut logs = LogStore::default();
    logs.push("p/web", "r1", "stdout", &"x".repeat(100000));
    let first = logs.read("p/web", None);
    assert_eq!(first.records.len(), 1);
    assert!(first.records[0].text.len() <= 16384);
    assert!(first.records[0].truncated);
    logs.clear("p/web");
    logs.push("p/web", "r2", "stdout", "after clear");
    assert!(logs.read("p/web", None).records[0].seq > first.records[0].seq);
}
