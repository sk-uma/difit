use gpui::Rgba;

/// GitHub-like dark palette. Colors are kept close to the React frontend's
/// Tailwind defaults so the two clients feel like the same product.
pub struct Theme;

impl Theme {
    pub const BG: Rgba = rgb(0x0d1117);
    pub const BG_ELEVATED: Rgba = rgb(0x161b22);
    pub const BG_HOVER: Rgba = rgb(0x1f242c);
    pub const BG_SELECTED: Rgba = rgb(0x21262d);
    pub const BORDER: Rgba = rgb(0x30363d);

    pub const TEXT: Rgba = rgb(0xc9d1d9);
    pub const TEXT_MUTED: Rgba = rgb(0x8b949e);
    pub const TEXT_LINK: Rgba = rgb(0x58a6ff);

    // Diff line backgrounds (slightly transparent in the React app; opaque is
    // fine for a native renderer).
    pub const DIFF_ADD_BG: Rgba = rgb(0x033a16);
    pub const DIFF_DEL_BG: Rgba = rgb(0x3c0f12);
    /// Intra-line word-diff highlight, layered on top of the line bg.
    pub const DIFF_ADD_WORD_BG: Rgba = rgb(0x125c2c);
    pub const DIFF_DEL_WORD_BG: Rgba = rgb(0x6d1b22);
    pub const DIFF_HUNK_BG: Rgba = rgb(0x1d2733);
    pub const DIFF_HUNK_TEXT: Rgba = rgb(0x7d8590);

    pub const FILE_STATUS_ADD: Rgba = rgb(0x3fb950);
    pub const FILE_STATUS_DEL: Rgba = rgb(0xf85149);
    pub const FILE_STATUS_MOD: Rgba = rgb(0xd29922);
}

const fn rgb(hex: u32) -> Rgba {
    Rgba {
        r: ((hex >> 16) & 0xff) as f32 / 255.0,
        g: ((hex >> 8) & 0xff) as f32 / 255.0,
        b: (hex & 0xff) as f32 / 255.0,
        a: 1.0,
    }
}

use std::sync::RwLock;

use gpui::{App, SharedString};

/// Ordered list of preferred monospace fonts. `resolve_fonts` picks the
/// first one that's actually installed on the system.
const MONO_CANDIDATES: &[&str] = &[
    "Zed Plex Mono",
    "IBM Plex Mono",
    "JetBrains Mono",
    "JetBrainsMono Nerd Font",
    "Cascadia Code",
    "Cascadia Mono",
    "Fira Code",
    "Source Code Pro",
    "Menlo",
    "Consolas",
];
/// Ordered list of preferred UI sans fonts.
const UI_CANDIDATES: &[&str] = &[
    "Inter",
    "Zed Sans",
    "IBM Plex Sans",
    "Segoe UI Variable",
    "Segoe UI",
    "SF Pro Display",
    "Helvetica Neue",
    "Arial",
];

const MONO_FALLBACK: &str = "Consolas";
const UI_FALLBACK: &str = "Segoe UI";

static MONO_FONT_RESOLVED: RwLock<Option<SharedString>> = RwLock::new(None);
static UI_FONT_RESOLVED: RwLock<Option<SharedString>> = RwLock::new(None);

/// Inspect the platform's installed font list and pick the first match
/// from each preferred ordering. GPUI's `font_family` takes a single
/// family name (not a CSS-style stack), so we have to do the fallback
/// ourselves at startup.
pub fn resolve_fonts(cx: &App) {
    let installed: std::collections::HashSet<String> =
        cx.text_system().all_font_names().into_iter().collect();
    let pick = |candidates: &[&str], fallback: &str| -> SharedString {
        for name in candidates {
            if installed.contains(*name) {
                return SharedString::from(name.to_string());
            }
        }
        SharedString::from(fallback.to_string())
    };
    *MONO_FONT_RESOLVED.write().unwrap() = Some(pick(MONO_CANDIDATES, MONO_FALLBACK));
    *UI_FONT_RESOLVED.write().unwrap() = Some(pick(UI_CANDIDATES, UI_FALLBACK));
}

/// Resolved monospace font name; call sites pass it directly to
/// `font_family(...)`. Keeps SHOUT_CASE for source-compat with prior
/// const callers.
#[allow(non_snake_case)]
pub fn MONO_FONT() -> SharedString {
    MONO_FONT_RESOLVED
        .read()
        .unwrap()
        .clone()
        .unwrap_or_else(|| SharedString::from(MONO_FALLBACK))
}

#[allow(non_snake_case)]
pub fn UI_FONT() -> SharedString {
    UI_FONT_RESOLVED
        .read()
        .unwrap()
        .clone()
        .unwrap_or_else(|| SharedString::from(UI_FALLBACK))
}
