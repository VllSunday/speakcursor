//! Small local history store for dictated source and processed text.

use fs2::FileExt;
use serde::{Deserialize, Serialize};
use speakcursor_core::DictationStyle;
use std::{
    fs::{self, File, OpenOptions},
    io,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

const RETENTION_DAYS: u64 = 30;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Entry {
    pub id: u64,
    pub created_at: u64,
    pub original: String,
    pub processed: Option<String>,
    pub style: Option<DictationStyle>,
    pub error: Option<String>,
}

#[derive(Default, Serialize, Deserialize)]
struct Data {
    entries: Vec<Entry>,
}

pub struct History {
    path: PathBuf,
    data: Data,
    _lock: File,
}

impl History {
    pub fn open(path: impl AsRef<Path>, keep_forever: bool) -> io::Result<Self> {
        let path = path.as_ref().to_path_buf();
        if let Some(dir) = path.parent() {
            fs::create_dir_all(dir)?;
        }
        let lock = OpenOptions::new()
            .create(true)
            .read(true)
            .write(true)
            .open(path.with_extension("lock"))?;
        lock.lock_exclusive()?;
        let data = match fs::read(&path) {
            Ok(bytes) => serde_json::from_slice(&bytes).map_err(io::Error::other)?,
            Err(error) if error.kind() == io::ErrorKind::NotFound => Data::default(),
            Err(error) => return Err(error),
        };
        let mut history = Self {
            path,
            data,
            _lock: lock,
        };
        if history.prune(keep_forever) {
            history.save()?;
        }
        Ok(history)
    }

    pub fn entries(&self) -> &[Entry] {
        &self.data.entries
    }

    pub fn search(&self, query: &str) -> Vec<&Entry> {
        let needle = query.to_lowercase();
        self.data
            .entries
            .iter()
            .rev()
            .filter(|entry| {
                entry.original.to_lowercase().contains(&needle)
                    || entry
                        .processed
                        .as_deref()
                        .unwrap_or("")
                        .to_lowercase()
                        .contains(&needle)
            })
            .collect()
    }

    pub fn append(
        &mut self,
        original: String,
        processed: Option<String>,
        style: Option<DictationStyle>,
        error: Option<String>,
        keep_forever: bool,
    ) -> io::Result<u64> {
        self.prune(keep_forever);
        let now = now_seconds()?;
        let id = self
            .data
            .entries
            .last()
            .map(|entry| entry.id + 1)
            .unwrap_or(1);
        self.data.entries.push(Entry {
            id,
            created_at: now,
            original,
            processed,
            style,
            error,
        });
        self.save()?;
        Ok(id)
    }

    /// A retry updates the same row; callers retain the source until it succeeds.
    pub fn update_result(&mut self, id: u64, processed: String) -> io::Result<bool> {
        if let Some(entry) = self.data.entries.iter_mut().find(|entry| entry.id == id) {
            entry.processed = Some(processed);
            entry.error = None;
            self.save()?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    pub fn update_error(&mut self, id: u64, error: String) -> io::Result<bool> {
        if let Some(entry) = self.data.entries.iter_mut().find(|entry| entry.id == id) {
            entry.error = Some(error);
            self.save()?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    pub fn clear(&mut self) -> io::Result<()> {
        self.data.entries.clear();
        self.save()
    }

    fn prune(&mut self, keep_forever: bool) -> bool {
        if keep_forever {
            return false;
        }
        let Ok(now) = now_seconds() else {
            return false;
        };
        let cutoff = now.saturating_sub(RETENTION_DAYS * 24 * 60 * 60);
        let len = self.data.entries.len();
        self.data.entries.retain(|entry| entry.created_at >= cutoff);
        len != self.data.entries.len()
    }

    fn save(&self) -> io::Result<()> {
        if let Some(dir) = self.path.parent() {
            fs::create_dir_all(dir)?;
        }
        let content = serde_json::to_vec_pretty(&self.data).map_err(io::Error::other)?;
        let temp = self.path.with_extension("json.tmp");
        fs::write(&temp, content)?;
        if cfg!(windows) && self.path.exists() {
            fs::remove_file(&self.path)?;
        }
        fs::rename(temp, &self.path)
    }
}

fn now_seconds() -> io::Result<u64> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .map_err(io::Error::other)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn search_and_retention_keep_both_versions_until_expiry() {
        let path = std::env::temp_dir().join(format!(
            "speakcursor-history-{}-{}.json",
            std::process::id(),
            now_seconds().unwrap()
        ));
        let mut history = History::open(&path, false).unwrap();
        history.clear().unwrap();
        let id = history
            .append(
                "первый вариант".into(),
                None,
                Some(DictationStyle::Proofread),
                Some("сбой".into()),
                false,
            )
            .unwrap();
        history
            .update_result(id, "готовый результат".into())
            .unwrap();
        assert_eq!(history.search("первый").len(), 1);
        assert_eq!(history.search("ГОТОВЫЙ").len(), 1);
        assert!(history.search("готовый")[0].error.is_none());
        history.data.entries[0].created_at = 1;
        history.save().unwrap();
        drop(history);
        assert!(History::open(&path, false).unwrap().entries().is_empty());
        fs::remove_file(path.with_extension("lock")).unwrap();
        fs::remove_file(path).unwrap();
    }
}
