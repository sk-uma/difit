//! Per-line syntax highlighting backed by syntect.
//!
//! For each diff line we run `HighlightLines` standalone (no cross-line
//! state) and convert syntect's runs into `(Range<usize>, gpui::Rgba)`
//! spans that `StyledText::with_highlights` can consume. This means
//! multi-line constructs (block comments, raw strings) won't always be
//! colored correctly — acceptable trade-off for the diff viewer, since
//! lines are already shown out of their original sequence anyway.

use std::ops::Range;
use std::sync::OnceLock;

use gpui::Rgba;
use syntect::easy::HighlightLines;
use syntect::highlighting::{Style, Theme, ThemeSet};
use syntect::parsing::SyntaxSet;

static SYNTAX_SET: OnceLock<SyntaxSet> = OnceLock::new();
static THEME: OnceLock<Theme> = OnceLock::new();

fn syntax_set() -> &'static SyntaxSet {
    SYNTAX_SET.get_or_init(SyntaxSet::load_defaults_newlines)
}

fn theme() -> &'static Theme {
    THEME.get_or_init(|| {
        let mut set = ThemeSet::load_defaults();
        // base16-ocean.dark sits closest to the React frontend's palette;
        // fall back to whatever default is available.
        set.themes
            .remove("base16-ocean.dark")
            .or_else(|| set.themes.remove("base16-eighties.dark"))
            .or_else(|| set.themes.remove("Solarized (dark)"))
            .unwrap_or_default()
    })
}

/// Result of highlighting a single line.
#[derive(Debug, Clone, Default)]
pub struct HighlightedLine {
    pub spans: Vec<(Range<usize>, Rgba)>,
}

/// Highlight `content` as a single line, picking the syntax by `extension`
/// (which may be empty). Tabs in `content` must already be expanded — the
/// resulting byte ranges are indices into the string you pass in.
pub fn highlight_line(content: &str, extension: &str) -> HighlightedLine {
    if content.is_empty() {
        return HighlightedLine::default();
    }

    let set = syntax_set();
    let syntax = set
        .find_syntax_by_extension(extension)
        .unwrap_or_else(|| set.find_syntax_plain_text());

    let mut highlighter = HighlightLines::new(syntax, theme());
    let ranges = match highlighter.highlight_line(content, set) {
        Ok(r) => r,
        Err(_) => return HighlightedLine::default(),
    };

    let mut spans = Vec::with_capacity(ranges.len());
    let mut offset = 0;
    for (style, text) in ranges {
        let len = text.len();
        if len > 0 {
            spans.push((offset..offset + len, syntect_color(style)));
        }
        offset += len;
    }
    HighlightedLine { spans }
}

fn syntect_color(style: Style) -> Rgba {
    let c = style.foreground;
    Rgba {
        r: f32::from(c.r) / 255.0,
        g: f32::from(c.g) / 255.0,
        b: f32::from(c.b) / 255.0,
        a: f32::from(c.a) / 255.0,
    }
}

/// Extract the file extension (lowercase, without the leading dot) from a
/// path. Returns `""` if there is none.
pub fn extension_for_path(path: &str) -> String {
    path.rsplit_once('.')
        .map(|(_, ext)| ext.to_ascii_lowercase())
        .filter(|ext| !ext.contains('/') && !ext.contains('\\'))
        .unwrap_or_default()
}
