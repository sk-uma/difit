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

/// Monospace font stack matched to what the React side uses for code.
pub const MONO_FONT: &str = "Consolas, ui-monospace, SFMono-Regular, Menlo, monospace";
pub const UI_FONT: &str = "Segoe UI, system-ui, sans-serif";
