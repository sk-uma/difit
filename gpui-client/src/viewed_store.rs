//! Per-repo "viewed file" persistence.
//!
//! Mirrors the React frontend's localStorage-based viewed state. Stored as
//! one JSON file under the OS config directory; keys are the
//! `DiffResponse.repository_id` SHA, values are the set of repo-relative
//! file paths the user has marked viewed.

use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::PathBuf;

use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::api::types::{DiffFile, FileStatus, LineType};

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct ViewedStore {
    #[serde(default)]
    pub repos: HashMap<String, HashSet<String>>,
    /// Per-repo, per-path set of diff-content hashes the user has marked
    /// viewed at some point — across any comparison range. Used to power
    /// the "Changed since you viewed" badge, mirroring React's
    /// `viewedHashIndex` on `StorageService`.
    #[serde(default)]
    pub hash_index: HashMap<String, HashMap<String, HashSet<String>>>,
}

impl ViewedStore {
    pub fn load() -> Self {
        let Some(path) = store_path() else {
            return Self::default();
        };
        let Ok(bytes) = fs::read(&path) else {
            return Self::default();
        };
        serde_json::from_slice(&bytes).unwrap_or_default()
    }

    pub fn save(&self) -> Result<()> {
        let Some(path) = store_path() else {
            return Ok(());
        };
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let bytes = serde_json::to_vec_pretty(self)?;
        fs::write(path, bytes)?;
        Ok(())
    }

    pub fn is_viewed(&self, repo_id: &str, path: &str) -> bool {
        self.repos
            .get(repo_id)
            .map(|set| set.contains(path))
            .unwrap_or(false)
    }

    pub fn set_viewed(&mut self, repo_id: &str, path: &str, viewed: bool) {
        let entry = self.repos.entry(repo_id.to_string()).or_default();
        if viewed {
            entry.insert(path.to_string());
        } else {
            entry.remove(path);
        }
    }

    /// Record that `path` was viewed at the given content `hash`. Adds to
    /// both the current viewed set and the long-lived hash index.
    pub fn set_viewed_with_hash(
        &mut self,
        repo_id: &str,
        path: &str,
        hash: &str,
        viewed: bool,
    ) {
        self.set_viewed(repo_id, path, viewed);
        let path_index = self
            .hash_index
            .entry(repo_id.to_string())
            .or_default()
            .entry(path.to_string())
            .or_default();
        if viewed {
            path_index.insert(hash.to_string());
        } else {
            path_index.remove(hash);
            if path_index.is_empty() {
                if let Some(repo_entry) = self.hash_index.get_mut(repo_id) {
                    repo_entry.remove(path);
                }
            }
        }
    }

    /// Has the user ever viewed `path` at *any* hash other than `current_hash`?
    /// True ⇒ surface a "Changed since you viewed" indicator. False when
    /// the file is currently viewed at this exact hash, or has never been
    /// touched.
    pub fn is_changed_since_viewed(
        &self,
        repo_id: &str,
        path: &str,
        current_hash: &str,
    ) -> bool {
        let Some(hashes) = self
            .hash_index
            .get(repo_id)
            .and_then(|m| m.get(path))
        else {
            return false;
        };
        !hashes.is_empty() && !hashes.contains(current_hash)
    }
}

/// Stable hash of a file's diff content. Matches React's
/// `getDiffContentForHashing` in shape (path + status + chunks/lines) so
/// the change-detection semantics line up — but uses FNV-1a instead of
/// SHA-256 since the hash is only ever compared against other entries
/// from the same client, never sent over the wire.
pub fn diff_content_hash(file: &DiffFile) -> String {
    let mut payload = String::with_capacity(256);
    payload.push_str(&file.path);
    payload.push('\n');
    payload.push_str(file_status_str(&file.status));
    payload.push('\n');
    for (i, chunk) in file.chunks.iter().enumerate() {
        if i > 0 {
            payload.push_str("\n\n");
        }
        payload.push_str(&chunk.header);
        for line in &chunk.lines {
            if matches!(line.kind, LineType::Hunk | LineType::Header) {
                continue;
            }
            payload.push('\n');
            payload.push_str(line_type_str(line.kind));
            payload.push(':');
            payload.push_str(&line.content);
        }
    }
    format!("{:016x}", fnv1a(payload.as_bytes()))
}

fn fnv1a(bytes: &[u8]) -> u64 {
    const OFFSET: u64 = 0xcbf29ce484222325;
    const PRIME: u64 = 0x100000001b3;
    let mut h = OFFSET;
    for &b in bytes {
        h ^= b as u64;
        h = h.wrapping_mul(PRIME);
    }
    h
}

fn file_status_str(s: &FileStatus) -> &'static str {
    match s {
        FileStatus::Modified => "modified",
        FileStatus::Added => "added",
        FileStatus::Deleted => "deleted",
        FileStatus::Renamed => "renamed",
    }
}

fn line_type_str(t: LineType) -> &'static str {
    match t {
        LineType::Add => "add",
        LineType::Delete => "delete",
        LineType::Normal => "normal",
        LineType::Hunk => "hunk",
        LineType::Remove => "remove",
        LineType::Context => "context",
        LineType::Header => "header",
    }
}

fn store_path() -> Option<PathBuf> {
    let base = config_dir()?;
    Some(base.join("difit-gpui").join("viewed.json"))
}

fn config_dir() -> Option<PathBuf> {
    #[cfg(windows)]
    {
        std::env::var_os("APPDATA").map(PathBuf::from)
    }
    #[cfg(not(windows))]
    {
        std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))
    }
}

/// Returns `true` if the file should be considered "viewed" automatically
/// (lockfiles, minified bundles, source maps, …). Mirrors a subset of the
/// patterns the React frontend auto-collapses.
pub fn is_auto_viewed(path: &str) -> bool {
    let lower = path.to_ascii_lowercase();
    const LOCKFILES: &[&str] = &[
        "package-lock.json",
        "pnpm-lock.yaml",
        "yarn.lock",
        "cargo.lock",
        "gemfile.lock",
        "go.sum",
        "composer.lock",
        "pipfile.lock",
        "poetry.lock",
        "uv.lock",
        "pdm.lock",
        "gradle.lockfile",
    ];

    let base = lower.rsplit_once('/').map(|(_, b)| b).unwrap_or(&lower);
    if LOCKFILES.contains(&base) {
        return true;
    }
    if lower.ends_with(".min.js")
        || lower.ends_with(".min.css")
        || lower.ends_with(".map")
        || lower.ends_with(".g.dart")
        || lower.ends_with(".freezed.dart")
        || lower.ends_with(".pb.go")
    {
        return true;
    }
    false
}
