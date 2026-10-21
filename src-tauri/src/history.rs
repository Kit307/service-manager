use crate::{
    logs::{LogPage, LogRecord},
    model::{AppError, Result, Snapshot},
    project::secure_write,
};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    fs::{self, OpenOptions},
    io::Read,
    os::unix::fs::OpenOptionsExt,
    path::{Path, PathBuf},
};

const MAX_RUNS: usize = 100;
const MAX_PER_SERVICE: usize = 20;
const LOG_BYTES: usize = 64 * 1024;
const FILE_BYTES: usize = 16 * 1024 * 1024;

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RunSummary {
    pub project_id: String,
    pub service_id: String,
    pub run_id: String,
    pub command: String,
    pub started_at: String,
    pub ended_at: Option<String>,
    pub state: String,
    pub exit_code: Option<i32>,
    pub error: Option<String>,
    pub truncated: bool,
}
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RunRecord {
    #[serde(flatten)]
    pub summary: RunSummary,
    pub records: Vec<LogRecord>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct StoredLog {
    seq: u64,
    run_id: String,
    timestamp: String,
    stream: String,
    text: String,
    truncated: bool,
}
struct StoredRun {
    summary: RunSummary,
    records: Vec<StoredLog>,
}
impl<'de> Deserialize<'de> for StoredRun {
    fn deserialize<D: serde::Deserializer<'de>>(
        deserializer: D,
    ) -> std::result::Result<Self, D::Error> {
        let mut value = serde_json::Value::deserialize(deserializer)?;
        let records = value
            .as_object_mut()
            .and_then(|v| v.remove("records"))
            .ok_or_else(|| serde::de::Error::custom("missing records"))?;
        Ok(Self {
            summary: serde_json::from_value(value).map_err(serde::de::Error::custom)?,
            records: serde_json::from_value(records).map_err(serde::de::Error::custom)?,
        })
    }
}
impl From<StoredRun> for RunRecord {
    fn from(r: StoredRun) -> Self {
        Self {
            summary: r.summary,
            records: r
                .records
                .into_iter()
                .map(|l| LogRecord {
                    seq: l.seq,
                    run_id: l.run_id,
                    timestamp: l.timestamp,
                    stream: l.stream,
                    text: l.text,
                    truncated: l.truncated,
                })
                .collect(),
        }
    }
}
pub struct HistoryStore {
    path: PathBuf,
    runs: Vec<RunRecord>,
}
fn invalid() -> AppError {
    AppError::new(
        "HISTORY_INVALID",
        "Run history is invalid or exceeds its limits. The file was preserved.",
    )
}
fn now() -> String {
    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
}
impl HistoryStore {
    pub fn open(state_dir: &Path) -> Result<Self> {
        let path = state_dir.join("history.json");
        if fs::symlink_metadata(state_dir).is_ok_and(|m| m.file_type().is_symlink()) {
            return Err(invalid());
        }
        let mut runs: Vec<RunRecord> = match fs::symlink_metadata(&path) {
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Vec::new(),
            Err(e) => return Err(e.into()),
            Ok(meta) => {
                if !meta.is_file()
                    || meta.file_type().is_symlink()
                    || meta.len() > FILE_BYTES as u64
                {
                    return Err(invalid());
                }
                let mut bytes = Vec::new();
                OpenOptions::new()
                    .read(true)
                    .custom_flags(nix::libc::O_NOFOLLOW)
                    .open(&path)?
                    .take(FILE_BYTES as u64 + 1)
                    .read_to_end(&mut bytes)?;
                if bytes.len() > FILE_BYTES {
                    return Err(invalid());
                }
                let stored: Vec<StoredRun> =
                    serde_json::from_slice(&bytes).map_err(|_| invalid())?;
                stored.into_iter().map(Into::into).collect()
            }
        };
        let mut counts = HashMap::new();
        let mut keys = std::collections::HashSet::new();
        if runs.len() > MAX_RUNS {
            return Err(invalid());
        }
        for run in &runs {
            let s = &run.summary;
            let count = counts.entry((&s.project_id, &s.service_id)).or_insert(0);
            *count += 1;
            if *count > MAX_PER_SERVICE
                || !keys.insert((&s.project_id, &s.service_id, &s.run_id))
                || !valid_summary(s)
                || run
                    .records
                    .iter()
                    .map(record_size)
                    .sum::<usize>()
                    .saturating_add(2)
                    > LOG_BYTES
                || run
                    .records
                    .iter()
                    .any(|r| r.run_id != s.run_id || r.timestamp.len() > 100 || r.stream.len() > 32)
            {
                return Err(invalid());
            }
        }
        let interrupted = runs.iter().any(|r| r.summary.ended_at.is_none());
        for run in &mut runs {
            if run.summary.ended_at.is_none() {
                run.summary.ended_at = Some(now());
                run.summary.state = "interrupted".into();
                run.summary.error =
                    Some("Manager stopped before a final result was recorded.".into());
            }
        }
        let mut store = Self {
            path,
            runs: Vec::new(),
        };
        if interrupted {
            store.persist(runs)?;
        } else {
            store.runs = runs;
        }
        Ok(store)
    }
    fn persist(&mut self, runs: Vec<RunRecord>) -> Result<()> {
        let bytes = serde_json::to_vec(&runs).map_err(|_| invalid())?;
        if bytes.len() > FILE_BYTES {
            return Err(invalid());
        }
        secure_write(&self.path, &bytes)?;
        self.runs = runs;
        Ok(())
    }
    pub fn begin(&mut self, project: &str, service: &str, run: &str, command: &str) -> Result<()> {
        if self
            .runs
            .iter()
            .any(|r| matches_run(r, project, service, run))
        {
            return Ok(());
        }
        let summary = RunSummary {
            project_id: project.into(),
            service_id: service.into(),
            run_id: run.into(),
            command: redact(command),
            started_at: now(),
            ended_at: None,
            state: "starting".into(),
            exit_code: None,
            error: None,
            truncated: false,
        };
        if !valid_summary(&summary) {
            return Err(invalid());
        }
        let mut runs = self.runs.clone();
        runs.insert(
            0,
            RunRecord {
                summary,
                records: Vec::new(),
            },
        );
        let mut counts = HashMap::new();
        runs.retain(|r| {
            let count = counts
                .entry((r.summary.project_id.clone(), r.summary.service_id.clone()))
                .or_insert(0);
            *count += 1;
            *count <= MAX_PER_SERVICE
        });
        runs.truncate(MAX_RUNS);
        self.persist(runs)
    }
    pub fn finish(
        &mut self,
        project: &str,
        service: &str,
        run: &str,
        snapshot: &Snapshot,
        logs: LogPage,
    ) -> Result<()> {
        if snapshot.run_id.as_deref() != Some(run) {
            return Ok(());
        }
        let Some(index) = self
            .runs
            .iter()
            .position(|r| matches_run(r, project, service, run) && r.summary.ended_at.is_none())
        else {
            return Ok(());
        };
        let mut runs = self.runs.clone();
        let record = &mut runs[index];
        record.summary.ended_at = Some(now());
        record.summary.state = snapshot.state.clone();
        record.summary.exit_code = snapshot.exit_code;
        record.summary.error = snapshot.error.as_deref().map(redact);
        let (records, truncated) = bounded_records(
            logs.records
                .into_iter()
                .filter(|r| r.run_id == run)
                .collect(),
            LOG_BYTES,
            500,
        );
        record.records = records;
        record.summary.truncated = logs.truncated || truncated;
        if !valid_summary(&record.summary) {
            return Err(invalid());
        }
        self.persist(runs)
    }
    pub fn list(&self, project: &str, service: &str) -> Vec<RunSummary> {
        self.runs
            .iter()
            .filter(|r| r.summary.project_id == project && r.summary.service_id == service)
            .map(|r| r.summary.clone())
            .collect()
    }
    pub fn get(&self, project: &str, service: &str, run: &str) -> Result<RunRecord> {
        self.runs
            .iter()
            .find(|r| matches_run(r, project, service, run))
            .cloned()
            .ok_or_else(|| AppError::new("RUN_NOT_FOUND", "No saved history exists for this run."))
    }
}
fn matches_run(r: &RunRecord, project: &str, service: &str, run: &str) -> bool {
    r.summary.project_id == project && r.summary.service_id == service && r.summary.run_id == run
}
fn valid_summary(s: &RunSummary) -> bool {
    [&s.project_id, &s.service_id, &s.run_id]
        .iter()
        .all(|v| !v.is_empty() && v.len() <= 256)
        && s.command.len() <= 32768
        && s.started_at.len() <= 100
        && s.ended_at.as_ref().is_none_or(|v| v.len() <= 100)
        && s.state.len() <= 100
        && s.error.as_ref().is_none_or(|v| v.len() <= 32768)
}
fn record_size(r: &LogRecord) -> usize {
    serde_json::to_vec(r).map_or(usize::MAX, |bytes| bytes.len().saturating_add(1))
}
fn bounded_records(records: Vec<LogRecord>, limit: usize, count: usize) -> (Vec<LogRecord>, bool) {
    let original = records.len();
    let mut private_key = false;
    let records = records
        .into_iter()
        .map(|mut record| {
            record.text = redact_with_state(&record.text, &mut private_key);
            record
        })
        .collect::<Vec<_>>();
    let mut output = Vec::new();
    let mut bytes = 2;
    for record in records.into_iter().rev() {
        let size = record_size(&record);
        if output.len() >= count || bytes + size > limit {
            break;
        }
        bytes += size;
        output.push(record);
    }
    let truncated = output.len() < original || output.iter().any(|r| r.truncated);
    output.reverse();
    (output, truncated)
}

// Redaction is best effort for unknown application-specific secrets; never reads env files.
fn redact(text: &str) -> String {
    redact_with_state(text, &mut false)
}
fn redact_with_state(text: &str, private_key: &mut bool) -> String {
    let sensitive = [
        "password",
        "passwd",
        "secret",
        "token",
        "api_key",
        "api-key",
        "apikey",
        "authorization",
        "credential",
        "private_key",
        "private-key",
        "client_secret",
        "access_key",
        "cookie",
    ];
    text.lines()
        .map(|line| {
            let folded = line.to_ascii_lowercase();
            if folded.contains("-----begin") && folded.contains("private key") {
                *private_key = true;
            }
            if *private_key {
                if folded.contains("-----end") {
                    *private_key = false;
                }
                return "[REDACTED private key]".to_string();
            }
            if sensitive.iter().any(|key| folded.contains(key))
                || folded.contains("bearer ")
                || folded.contains("basic ")
            {
                "[REDACTED credential-bearing line]".to_string()
            } else {
                line.split_whitespace()
                    .map(|word| {
                        let w = word.to_ascii_lowercase();
                        if (w.contains("://")
                            && w.split("://").nth(1).is_some_and(|v| v.contains('@')))
                            || word.starts_with("eyJ")
                            || word.starts_with("AKIA")
                            || word.starts_with("ASIA")
                            || word.starts_with("sk-")
                            || word.starts_with("ghp_")
                            || word.starts_with("github_pat_")
                        {
                            "[REDACTED]"
                        } else {
                            word
                        }
                    })
                    .collect::<Vec<_>>()
                    .join(" ")
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

pub fn redacted_diagnostics(
    project: &str,
    service: &str,
    snapshot: &Snapshot,
    page: LogPage,
) -> serde_json::Value {
    let mut snapshot = snapshot.clone();
    snapshot.error = snapshot.error.as_deref().map(redact);
    snapshot.activity = redact(&snapshot.activity);
    let (records, bounded) = bounded_records(
        page.records
            .into_iter()
            .filter(|r| snapshot.run_id.as_deref() == Some(r.run_id.as_str()))
            .collect(),
        32 * 1024,
        100,
    );
    let mut summary = vec![format!(
        "Observation: service state is {} and health is {}.",
        snapshot.state, snapshot.health
    )];
    if let Some(code) = snapshot.exit_code {
        summary.push(format!("Observation: the process exited with code {code}."));
    }
    if records.iter().any(|r| {
        r.text
            .to_ascii_lowercase()
            .contains("address already in use")
    }) {
        summary.push("Hint: recent output reports an address already in use; check the configured listener and existing process ownership.".into());
    }
    if records
        .iter()
        .any(|r| r.text.to_ascii_lowercase().contains("permission denied"))
    {
        summary.push("Hint: recent output reports permission denied; inspect access for the configured command or file.".into());
    }
    serde_json::json!({"projectId":project,"serviceId":service,"runId":snapshot.run_id,"state":snapshot,"summary":summary,"records":records,"nextCursor":page.next_cursor,"truncated":page.truncated || bounded,"redacted":true})
}
