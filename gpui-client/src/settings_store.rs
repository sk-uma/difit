//! User-tweakable settings, persisted to a JSON file in the OS config
//! directory. Loaded once at startup and re-saved on every change.

use std::fs;
use std::path::PathBuf;
use std::sync::RwLock;

use anyhow::Result;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    /// Diff body font size, in pixels.
    #[serde(default = "default_font_size")]
    pub font_size: f32,
    /// Name of a syntect theme to use for syntax highlighting. Must be one
    /// of the bundled themes (`base16-ocean.dark`, `base16-eighties.dark`,
    /// `base16-mocha.dark`, `Solarized (dark)`, `InspiredGitHub`).
    #[serde(default = "default_syntax_theme")]
    pub syntax_theme: String,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            font_size: default_font_size(),
            syntax_theme: default_syntax_theme(),
        }
    }
}

fn default_font_size() -> f32 {
    14.0
}

fn default_syntax_theme() -> String {
    "base16-ocean.dark".to_string()
}

/// Known-good theme names exposed in the settings UI. Anything else still
/// loads (syntect falls back to plain text), but these are guaranteed
/// available since they ship with the syntect default theme set.
pub const SYNTAX_THEMES: &[&str] = &[
    "base16-ocean.dark",
    "base16-eighties.dark",
    "base16-mocha.dark",
    "Solarized (dark)",
    "InspiredGitHub",
];

pub const FONT_SIZES: &[(&str, f32)] = &[
    ("Small", 12.0),
    ("Medium", 14.0),
    ("Large", 16.0),
    ("Extra Large", 18.0),
];

impl Settings {
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
        fs::write(path, serde_json::to_vec_pretty(self)?)?;
        Ok(())
    }
}

fn store_path() -> Option<PathBuf> {
    Some(config_dir()?.join("difit-gpui").join("settings.json"))
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

/// A process-wide, lock-protected settings handle. Lets the highlighter
/// (which doesn't have access to `DifitApp`) read the current syntax
/// theme without threading it through everything.
pub static CURRENT_SETTINGS: RwLock<Settings> = RwLock::new(Settings {
    font_size: 14.0,
    syntax_theme: String::new(),
});

pub fn install(settings: Settings) {
    *CURRENT_SETTINGS.write().unwrap() = settings;
}

pub fn snapshot() -> Settings {
    CURRENT_SETTINGS.read().unwrap().clone()
}
