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

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct ViewedStore {
    #[serde(default)]
    pub repos: HashMap<String, HashSet<String>>,
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
