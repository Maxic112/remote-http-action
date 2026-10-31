//! JSON Lines logs per category with size-based rotation.
//!
//! Files: `<data>/logs/<category>.jsonl`, rotated to `<category>.1.jsonl` … `<category>.N.jsonl`
//! (`.1` is the newest rotated file). Secrets are never passed to the logger: HTTP entries carry the
//! API key *name*, never the key itself.

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, RwLock};

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::error::AppResult;
use crate::model::now;
use crate::settings::{LogCategorySettings, LoggingSettings};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, clap::ValueEnum)]
#[serde(rename_all = "snake_case")]
pub enum LogCategory {
    Http,
    Cli,
    Flow,
}

impl LogCategory {
    pub const ALL: [LogCategory; 3] = [LogCategory::Http, LogCategory::Cli, LogCategory::Flow];

    pub fn as_str(self) -> &'static str {
        match self {
            LogCategory::Http => "http",
            LogCategory::Cli => "cli",
            LogCategory::Flow => "flow",
        }
    }
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct LogFileInfo {
    pub category: LogCategory,
    pub enabled: bool,
    pub files: usize,
    pub size_bytes: u64,
}

pub struct Logger {
    dir: PathBuf,
    settings: RwLock<LoggingSettings>,
    write_lock: Mutex<()>,
}

impl Logger {
    pub fn new(dir: PathBuf, settings: LoggingSettings) -> Self {
        Self {
            dir,
            settings: RwLock::new(settings),
            write_lock: Mutex::new(()),
        }
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    pub fn set_settings(&self, settings: LoggingSettings) {
        *self.settings.write().unwrap() = settings;
    }

    pub fn output_limit(&self) -> usize {
        self.settings.read().unwrap().output_limit as usize
    }

    fn category(&self, cat: LogCategory) -> LogCategorySettings {
        let s = self.settings.read().unwrap();
        match cat {
            LogCategory::Http => s.http.clone(),
            LogCategory::Cli => s.cli.clone(),
            LogCategory::Flow => s.flow.clone(),
        }
    }

    fn file(&self, cat: LogCategory, index: u32) -> PathBuf {
        if index == 0 {
            self.dir.join(format!("{}.jsonl", cat.as_str()))
        } else {
            self.dir.join(format!("{}.{index}.jsonl", cat.as_str()))
        }
    }

    /// Appends an entry (a JSON object) with a `ts` field. Errors are ignored: logging must
    /// never break request handling.
    pub fn write(&self, cat: LogCategory, entry: Value) {
        let cfg = self.category(cat);
        if !cfg.enabled {
            return;
        }
        let mut obj = serde_json::Map::new();
        obj.insert("ts".into(), json!(now()));
        if let Value::Object(fields) = entry {
            obj.extend(fields);
        }
        let mut line = serde_json::to_string(&Value::Object(obj)).unwrap_or_default();
        line.push('\n');

        let _guard = self.write_lock.lock().unwrap_or_else(|p| p.into_inner());
        if fs::create_dir_all(&self.dir).is_err() {
            return;
        }
        let path = self.file(cat, 0);
        let written = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .and_then(|mut f| f.write_all(line.as_bytes()));
        if written.is_err() {
            return;
        }
        let max_bytes = u64::from(cfg.max_file_mb) * 1024 * 1024;
        if fs::metadata(&path).map(|m| m.len()).unwrap_or(0) >= max_bytes {
            self.rotate(cat, cfg.max_files);
        }
    }

    fn rotate(&self, cat: LogCategory, max_files: u32) {
        let _ = fs::remove_file(self.file(cat, max_files));
        for i in (1..max_files).rev() {
            let from = self.file(cat, i);
            if from.exists() {
                let _ = fs::rename(&from, self.file(cat, i + 1));
            }
        }
        let _ = fs::rename(self.file(cat, 0), self.file(cat, 1));
        // Files beyond a lowered limit.
        let mut i = max_files + 1;
        while fs::remove_file(self.file(cat, i)).is_ok() {
            i += 1;
        }
    }

    /// Current file (may be absent right after a rotation) and rotated files, newest first.
    fn existing_files(&self, cat: LogCategory) -> Vec<PathBuf> {
        let mut files: Vec<PathBuf> = Some(self.file(cat, 0))
            .filter(|p| p.exists())
            .into_iter()
            .collect();
        let mut i = 1;
        while self.file(cat, i).exists() {
            files.push(self.file(cat, i));
            i += 1;
        }
        files
    }

    /// Newest entries first, optionally filtered by a case-insensitive substring.
    pub fn read(
        &self,
        cat: LogCategory,
        limit: usize,
        query: Option<&str>,
    ) -> AppResult<Vec<Value>> {
        let query = query.map(str::to_lowercase).filter(|q| !q.is_empty());
        let mut out = Vec::new();
        for path in self.existing_files(cat) {
            let text = fs::read_to_string(&path)?;
            for line in text.lines().rev() {
                if line.trim().is_empty() {
                    continue;
                }
                if let Some(q) = &query
                    && !line.to_lowercase().contains(q)
                {
                    continue;
                }
                if let Ok(v) = serde_json::from_str::<Value>(line) {
                    out.push(v);
                    if out.len() >= limit {
                        return Ok(out);
                    }
                }
            }
        }
        Ok(out)
    }

    pub fn clear(&self, cat: LogCategory) -> AppResult<()> {
        let _guard = self.write_lock.lock().unwrap_or_else(|p| p.into_inner());
        for p in self.existing_files(cat) {
            fs::remove_file(p)?;
        }
        Ok(())
    }

    pub fn info(&self) -> Vec<LogFileInfo> {
        LogCategory::ALL
            .iter()
            .map(|&cat| {
                let files = self.existing_files(cat);
                LogFileInfo {
                    category: cat,
                    enabled: self.category(cat).enabled,
                    files: files.len(),
                    size_bytes: files
                        .iter()
                        .filter_map(|p| fs::metadata(p).ok())
                        .map(|m| m.len())
                        .sum(),
                }
            })
            .collect()
    }
}

/// Shortens command output for the log.
pub fn clip(s: &str, limit: usize) -> String {
    if s.chars().count() <= limit {
        return s.to_string();
    }
    let mut t: String = s.chars().take(limit).collect();
    t.push('…');
    t
}

#[cfg(test)]
mod tests {
    use super::*;

    fn logger(dir: &Path, max_files: u32) -> Logger {
        let mut s = LoggingSettings::default();
        s.http.max_file_mb = 1;
        s.http.max_files = max_files;
        Logger::new(dir.to_path_buf(), s)
    }

    #[test]
    fn writes_and_reads_newest_first() {
        let dir = tempfile::tempdir().unwrap();
        let log = logger(dir.path(), 3);
        for i in 0..5 {
            log.write(
                LogCategory::Http,
                json!({ "n": i, "key": if i % 2 == 0 { "phone" } else { "tv" } }),
            );
        }
        let all = log.read(LogCategory::Http, 100, None).unwrap();
        assert_eq!(all.len(), 5);
        assert_eq!(all[0]["n"], 4);
        assert!(all[0]["ts"].is_string());
        let phone = log.read(LogCategory::Http, 100, Some("PHONE")).unwrap();
        assert_eq!(phone.len(), 3);
        assert_eq!(log.read(LogCategory::Http, 2, None).unwrap().len(), 2);
        assert!(log.read(LogCategory::Cli, 10, None).unwrap().is_empty());
    }

    #[test]
    fn rotates_by_size_and_keeps_limit() {
        let dir = tempfile::tempdir().unwrap();
        let log = logger(dir.path(), 2);
        let big = "x".repeat(300 * 1024);
        for i in 0..12 {
            log.write(LogCategory::Http, json!({ "n": i, "pad": big }));
        }
        let names: Vec<String> = fs::read_dir(dir.path())
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        assert!(names.contains(&"http.1.jsonl".to_string()));
        assert!(names.contains(&"http.2.jsonl".to_string()));
        assert!(!names.contains(&"http.3.jsonl".to_string()));
        // Reading continues into rotated files, newest first.
        let entries = log.read(LogCategory::Http, 100, None).unwrap();
        assert_eq!(entries[0]["n"], 11);
        let ns: Vec<i64> = entries.iter().map(|e| e["n"].as_i64().unwrap()).collect();
        assert!(ns.windows(2).all(|w| w[0] > w[1]));
        let info = log.info();
        // The 12th entry triggered a rotation, so only `.1` and `.2` exist right now.
        assert_eq!(info[0].files, 2);
    }

    #[test]
    fn disabled_category_is_not_written_and_clear_removes_files() {
        let dir = tempfile::tempdir().unwrap();
        let mut s = LoggingSettings::default();
        s.cli.enabled = false;
        let log = Logger::new(dir.path().to_path_buf(), s);
        log.write(LogCategory::Cli, json!({ "a": 1 }));
        log.write(LogCategory::Flow, json!({ "a": 1 }));
        assert!(log.read(LogCategory::Cli, 10, None).unwrap().is_empty());
        assert_eq!(log.read(LogCategory::Flow, 10, None).unwrap().len(), 1);
        log.clear(LogCategory::Flow).unwrap();
        assert!(log.read(LogCategory::Flow, 10, None).unwrap().is_empty());
    }

    #[test]
    fn clip_shortens_long_text() {
        assert_eq!(clip("abc", 5), "abc");
        assert_eq!(clip("abcdef", 3), "abc…");
    }
}
