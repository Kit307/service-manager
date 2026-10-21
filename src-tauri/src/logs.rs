use serde::Serialize;
use std::collections::{HashMap, VecDeque};
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LogRecord {
    pub seq: u64,
    pub run_id: String,
    pub timestamp: String,
    pub stream: String,
    pub text: String,
    pub truncated: bool,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LogPage {
    pub records: Vec<LogRecord>,
    pub next_cursor: String,
    pub truncated: bool,
}
#[derive(Default)]
pub struct LogStore {
    entries: HashMap<String, VecDeque<LogRecord>>,
    seq: u64,
    evicted: HashMap<String, u64>,
    sizes: HashMap<String, usize>,
    total: usize,
}
impl LogStore {
    pub fn push(&mut self, key: &str, run: &str, stream: &str, text: &str) {
        self.push_chunk(key, run, stream, text, false);
    }
    pub fn push_chunk(&mut self, key: &str, run: &str, stream: &str, text: &str, continued: bool) {
        self.seq += 1;
        let mut cleaned = String::new();
        let mut escape = false;
        for ch in text.chars() {
            if ch == '\u{1b}' {
                escape = true;
                continue;
            }
            if escape {
                if ch.is_ascii_alphabetic() {
                    escape = false;
                }
                continue;
            }
            if !ch.is_control() || ch == '\t' {
                cleaned.push(ch);
            }
            if cleaned.len() >= 16380 {
                break;
            }
        }
        let record = LogRecord {
            seq: self.seq,
            run_id: run.into(),
            timestamp: chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
            stream: stream.into(),
            truncated: continued || text.len() > 16380,
            text: cleaned,
        };
        let size = record.text.len() + 160;
        *self.sizes.entry(key.into()).or_default() += size;
        self.total += size;
        self.entries
            .entry(key.into())
            .or_default()
            .push_back(record);
        while self.entries.get(key).is_some_and(|v| v.len() > 5000)
            || self.sizes.get(key).copied().unwrap_or(0) > 2 * 1024 * 1024
        {
            self.evict(key);
        }
        while self.total > 32 * 1024 * 1024 {
            let oldest = self
                .entries
                .iter()
                .filter_map(|(k, v)| v.front().map(|r| (k.clone(), r.seq)))
                .min_by_key(|(_, seq)| *seq)
                .map(|(k, _)| k);
            if let Some(oldest) = oldest {
                self.evict(&oldest);
            } else {
                break;
            }
        }
    }
    fn evict(&mut self, key: &str) {
        if let Some(record) = self.entries.get_mut(key).and_then(|v| v.pop_front()) {
            let size = record.text.len() + 160;
            self.total = self.total.saturating_sub(size);
            if let Some(bytes) = self.sizes.get_mut(key) {
                *bytes = bytes.saturating_sub(size);
            }
            self.evicted.insert(key.into(), record.seq);
        }
    }
    pub fn read(&self, key: &str, cursor: Option<&str>) -> LogPage {
        let all = self.entries.get(key);
        let cursor_num = cursor.and_then(|s| s.parse::<u64>().ok());
        let truncated = cursor.is_some()
            && cursor_num.unwrap_or(0) < self.evicted.get(key).copied().unwrap_or(0);
        let mut bytes = 0;
        let mut records = Vec::new();
        if let Some(all) = all {
            let skip = if cursor.is_none() {
                all.len().saturating_sub(200)
            } else {
                0
            };
            for record in all
                .iter()
                .skip(skip)
                .filter(|r| r.seq > cursor_num.unwrap_or(0))
            {
                let size = record.text.len() + 160;
                if records.len() >= 1000 || bytes + size > 128 * 1024 {
                    break;
                }
                bytes += size;
                records.push(record.clone());
            }
        }
        let next_cursor = records
            .last()
            .map(|r| r.seq)
            .or(cursor_num)
            .unwrap_or(self.seq)
            .to_string();
        LogPage {
            records,
            next_cursor,
            truncated,
        }
    }
    pub fn clear(&mut self, key: &str) {
        while self.entries.get(key).is_some_and(|v| !v.is_empty()) {
            self.evict(key);
        }
    }
    pub fn read_run(&self, key: &str, run: &str) -> LogPage {
        let records: Vec<_> = self
            .entries
            .get(key)
            .into_iter()
            .flatten()
            .filter(|r| r.run_id == run)
            .cloned()
            .collect();
        let next_cursor = records
            .last()
            .map(|r| r.seq)
            .unwrap_or(self.seq)
            .to_string();
        LogPage {
            records,
            next_cursor,
            truncated: self.evicted.contains_key(key),
        }
    }
    pub fn revision(&self) -> u64 {
        self.seq
    }
}
