//! Embedded SVG icon set. Compiled into the binary as `&'static [u8]`
//! and exposed to GPUI via an `AssetSource`. The icons are simplified
//! lucide.dev paths so the GPUI client visually matches the React UI.

use std::borrow::Cow;

use anyhow::Result;
use gpui::{AssetSource, SharedString};

pub struct EmbeddedAssets;

impl AssetSource for EmbeddedAssets {
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        if let Some((_, bytes)) = ICONS.iter().find(|(name, _)| *name == path) {
            Ok(Some(Cow::Borrowed(bytes.as_bytes())))
        } else {
            Ok(None)
        }
    }

    fn list(&self, _path: &str) -> Result<Vec<SharedString>> {
        Ok(ICONS
            .iter()
            .map(|(name, _)| SharedString::from(*name))
            .collect())
    }
}

/// (path, body). The path is what callers pass to `svg().path("…")`.
const ICONS: &[(&str, &str)] = &[
    ("icons/chevron-right.svg", CHEVRON_RIGHT),
    ("icons/chevron-down.svg", CHEVRON_DOWN),
    ("icons/columns.svg", COLUMNS),
    ("icons/align-left.svg", ALIGN_LEFT),
    ("icons/settings.svg", SETTINGS),
    ("icons/keyboard.svg", KEYBOARD),
    ("icons/refresh-cw.svg", REFRESH_CW),
    ("icons/search.svg", SEARCH),
    ("icons/plus.svg", PLUS),
    ("icons/check.svg", CHECK),
    ("icons/folder.svg", FOLDER),
    ("icons/folder-open.svg", FOLDER_OPEN),
    ("icons/file.svg", FILE),
    ("icons/file-plus.svg", FILE_PLUS),
    ("icons/file-x.svg", FILE_X),
    ("icons/file-pen.svg", FILE_PEN),
    ("icons/file-diff.svg", FILE_DIFF),
    ("icons/message-square.svg", MESSAGE_SQUARE),
    ("icons/copy.svg", COPY),
    ("icons/external-link.svg", EXTERNAL_LINK),
    ("icons/x.svg", X_MARK),
    ("icons/eye.svg", EYE),
    ("icons/edit.svg", EDIT),
    ("icons/trash.svg", TRASH),
    ("icons/reply.svg", REPLY),
    ("icons/info.svg", INFO),
    ("icons/help.svg", HELP_CIRCLE),
    ("icons/panel-left.svg", PANEL_LEFT),
    ("icons/arrow-up-from-line.svg", ARROW_UP_FROM_LINE),
    ("icons/arrow-down-from-line.svg", ARROW_DOWN_FROM_LINE),
    ("icons/unfold-vertical.svg", UNFOLD_VERTICAL),
    ("icons/minus.svg", MINUS),
    ("icons/square.svg", SQUARE),
    ("icons/restore.svg", RESTORE),
    ("icons/github.svg", GITHUB),
    ("icons/difit-logo.svg", DIFIT_LOGO),
];

/// GitHub octocat logo, lifted verbatim from
/// `src/client/components/GitHubIcon.tsx` (which credits
/// github.com/logos).
const GITHUB: &str = concat!(
    r#"<svg xmlns="http://www.w3.org/2000/svg" width="98" height="96" viewBox="0 0 98 96" aria-label="GitHub" role="img">"#,
    r#"<path fill-rule="evenodd" clip-rule="evenodd" d="M48.854 0C21.839 0 0 22 0 49.217c0 21.756 13.993 40.172 33.405 46.69 2.427.49 3.316-1.059 3.316-2.362 0-1.141-.08-5.052-.08-9.127-13.59 2.934-16.42-5.867-16.42-5.867-2.184-5.704-5.42-7.17-5.42-7.17-4.448-3.015.324-3.015.324-3.015 4.934.326 7.523 5.052 7.523 5.052 4.367 7.496 11.404 5.378 14.235 4.074.404-3.178 1.699-5.378 3.074-6.6-10.839-1.141-22.243-5.378-22.243-24.283 0-5.378 1.94-9.778 5.014-13.2-.485-1.222-2.184-6.275.486-13.038 0 0 4.125-1.304 13.426 5.052a46.97 46.97 0 0 1 12.214-1.63c4.125 0 8.33.571 12.213 1.63 9.302-6.356 13.427-5.052 13.427-5.052 2.67 6.763.97 11.816.485 13.038 3.155 3.422 5.015 7.822 5.015 13.2 0 18.905-11.404 23.06-22.324 24.283 1.78 1.548 3.316 4.481 3.316 9.126 0 6.6-.08 11.897-.08 13.526 0 1.304.89 2.853 3.316 2.364 19.412-6.52 33.405-24.935 33.405-46.691C97.707 22 75.788 0 48.854 0z" fill="currentColor"/></svg>"#
);

const MINUS: &str = concat!(
    r#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">"#,
    r#"<path d="M5 12h14"/></svg>"#
);
const SQUARE: &str = concat!(
    r#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">"#,
    r#"<rect width="18" height="18" x="3" y="3" rx="2"/></svg>"#
);
/// "Restore" — two overlapping squares (Windows' un-maximize glyph).
const RESTORE: &str = concat!(
    r#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">"#,
    r#"<rect width="14" height="14" x="3" y="7" rx="2"/><path d="M7 7V5a2 2 0 0 1 2-2h10a2 2 0 0 1 2 2v10a2 2 0 0 1-2 2h-2"/></svg>"#
);

const ARROW_UP_FROM_LINE: &str = concat!(
    r#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">"#,
    r#"<path d="m18 9-6-6-6 6"/><path d="M12 3v14"/><path d="M5 21h14"/></svg>"#
);
const ARROW_DOWN_FROM_LINE: &str = concat!(
    r#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">"#,
    r#"<path d="M19 3H5"/><path d="M12 21V7"/><path d="m6 15 6 6 6-6"/></svg>"#
);
const UNFOLD_VERTICAL: &str = concat!(
    r#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">"#,
    r#"<path d="M12 22v-6"/><path d="M12 8V2"/><path d="M4 12H2"/><path d="M10 12H8"/><path d="M16 12h-2"/><path d="M22 12h-2"/><path d="m15 19-3 3-3-3"/><path d="m15 5-3-3-3 3"/></svg>"#
);

const SVG_HEADER: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">"#;

const CHEVRON_RIGHT: &str = concat!(
    r#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">"#,
    r#"<path d="m9 18 6-6-6-6"/></svg>"#
);
const CHEVRON_DOWN: &str = concat!(
    r#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">"#,
    r#"<path d="m6 9 6 6 6-6"/></svg>"#
);
const COLUMNS: &str = concat!(
    r#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">"#,
    r#"<rect width="18" height="18" x="3" y="3" rx="2"/><path d="M12 3v18"/></svg>"#
);
const ALIGN_LEFT: &str = concat!(
    r#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">"#,
    r#"<line x1="21" x2="3" y1="6" y2="6"/><line x1="15" x2="3" y1="12" y2="12"/><line x1="17" x2="3" y1="18" y2="18"/></svg>"#
);
const SETTINGS: &str = concat!(
    r#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">"#,
    r#"<path d="M12.22 2h-.44a2 2 0 0 0-2 2v.18a2 2 0 0 1-1 1.73l-.43.25a2 2 0 0 1-2 0l-.15-.08a2 2 0 0 0-2.73.73l-.22.38a2 2 0 0 0 .73 2.73l.15.1a2 2 0 0 1 1 1.72v.51a2 2 0 0 1-1 1.74l-.15.09a2 2 0 0 0-.73 2.73l.22.38a2 2 0 0 0 2.73.73l.15-.08a2 2 0 0 1 2 0l.43.25a2 2 0 0 1 1 1.73V20a2 2 0 0 0 2 2h.44a2 2 0 0 0 2-2v-.18a2 2 0 0 1 1-1.73l.43-.25a2 2 0 0 1 2 0l.15.08a2 2 0 0 0 2.73-.73l.22-.39a2 2 0 0 0-.73-2.73l-.15-.08a2 2 0 0 1-1-1.74v-.5a2 2 0 0 1 1-1.74l.15-.09a2 2 0 0 0 .73-2.73l-.22-.38a2 2 0 0 0-2.73-.73l-.15.08a2 2 0 0 1-2 0l-.43-.25a2 2 0 0 1-1-1.73V4a2 2 0 0 0-2-2z"/><circle cx="12" cy="12" r="3"/></svg>"#
);
const KEYBOARD: &str = concat!(
    r#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">"#,
    r#"<path d="M10 8h.01"/><path d="M12 12h.01"/><path d="M14 8h.01"/><path d="M16 12h.01"/><path d="M18 8h.01"/><path d="M6 8h.01"/><path d="M7 16h10"/><path d="M8 12h.01"/><rect width="20" height="16" x="2" y="4" rx="2"/></svg>"#
);
const REFRESH_CW: &str = concat!(
    r#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">"#,
    r#"<path d="M3 12a9 9 0 0 1 9-9 9.75 9.75 0 0 1 6.74 2.74L21 8"/><path d="M21 3v5h-5"/><path d="M21 12a9 9 0 0 1-9 9 9.75 9.75 0 0 1-6.74-2.74L3 16"/><path d="M3 21v-5h5"/></svg>"#
);
const SEARCH: &str = concat!(
    r#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">"#,
    r#"<circle cx="11" cy="11" r="8"/><path d="m21 21-4.3-4.3"/></svg>"#
);
const PLUS: &str = concat!(
    r#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">"#,
    r#"<path d="M5 12h14"/><path d="M12 5v14"/></svg>"#
);
const CHECK: &str = concat!(
    r#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">"#,
    r#"<path d="M20 6 9 17l-5-5"/></svg>"#
);
const FOLDER: &str = concat!(
    r#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">"#,
    r#"<path d="M20 20a2 2 0 0 0 2-2V8a2 2 0 0 0-2-2h-7.93a2 2 0 0 1-1.66-.9l-.82-1.2A2 2 0 0 0 7.93 3H4a2 2 0 0 0-2 2v13a2 2 0 0 0 2 2z"/></svg>"#
);
const FOLDER_OPEN: &str = concat!(
    r#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">"#,
    r#"<path d="m6 14 1.45-2.9A2 2 0 0 1 9.24 10H20a2 2 0 0 1 1.94 2.5l-1.55 6a2 2 0 0 1-1.94 1.5H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h3.9a2 2 0 0 1 1.69.9l.81 1.2a2 2 0 0 0 1.67.9H18a2 2 0 0 1 2 2v2"/></svg>"#
);
const FILE: &str = concat!(
    r#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">"#,
    r#"<path d="M15 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V7Z"/><path d="M14 2v4a2 2 0 0 0 2 2h4"/></svg>"#
);
const FILE_PLUS: &str = concat!(
    r#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">"#,
    r#"<path d="M15 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V7Z"/><path d="M14 2v4a2 2 0 0 0 2 2h4"/><path d="M9 15h6"/><path d="M12 18v-6"/></svg>"#
);
const FILE_X: &str = concat!(
    r#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">"#,
    r#"<path d="M15 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V7Z"/><path d="M14 2v4a2 2 0 0 0 2 2h4"/><path d="m9.5 12.5 5 5"/><path d="m14.5 12.5-5 5"/></svg>"#
);
const FILE_PEN: &str = concat!(
    r#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">"#,
    r#"<path d="M12.5 22H6a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h8.5L20 7.5V11"/><path d="M14 2v4a2 2 0 0 0 2 2h4"/><path d="M13.378 15.626a1 1 0 1 0-3.004-3.004l-5.01 5.012a2 2 0 0 0-.506.854l-.837 2.87a.5.5 0 0 0 .62.62l2.87-.837a2 2 0 0 0 .854-.506z"/></svg>"#
);
const FILE_DIFF: &str = concat!(
    r#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">"#,
    r#"<path d="M15 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V7Z"/><path d="M14 2v4a2 2 0 0 0 2 2h4"/><path d="M9 13h6"/><path d="M12 10v6"/><path d="M9 19h6"/></svg>"#
);
const MESSAGE_SQUARE: &str = concat!(
    r#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">"#,
    r#"<path d="M21 15a2 2 0 0 1-2 2H7l-4 4V5a2 2 0 0 1 2-2h14a2 2 0 0 1 2 2z"/></svg>"#
);
const COPY: &str = concat!(
    r#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">"#,
    r#"<rect width="14" height="14" x="8" y="8" rx="2" ry="2"/><path d="M4 16c-1.1 0-2-.9-2-2V4c0-1.1.9-2 2-2h10c1.1 0 2 .9 2 2"/></svg>"#
);
const EXTERNAL_LINK: &str = concat!(
    r#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">"#,
    r#"<path d="M15 3h6v6"/><path d="M10 14 21 3"/><path d="M18 13v6a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V8a2 2 0 0 1 2-2h6"/></svg>"#
);
const X_MARK: &str = concat!(
    r#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">"#,
    r#"<path d="M18 6 6 18"/><path d="m6 6 12 12"/></svg>"#
);
const EYE: &str = concat!(
    r#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">"#,
    r#"<path d="M2.062 12.348a1 1 0 0 1 0-.696 10.75 10.75 0 0 1 19.876 0 1 1 0 0 1 0 .696 10.75 10.75 0 0 1-19.876 0"/><circle cx="12" cy="12" r="3"/></svg>"#
);
const EDIT: &str = concat!(
    r#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">"#,
    r#"<path d="M21.174 6.812a1 1 0 0 0-3.986-3.987L3.842 16.174a2 2 0 0 0-.5.83l-1.321 4.352a.5.5 0 0 0 .623.622l4.353-1.32a2 2 0 0 0 .83-.497z"/></svg>"#
);
const TRASH: &str = concat!(
    r#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">"#,
    r#"<path d="M3 6h18"/><path d="M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6"/><path d="M8 6V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2"/></svg>"#
);
const REPLY: &str = concat!(
    r#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">"#,
    r#"<polyline points="9 17 4 12 9 7"/><path d="M20 18v-2a4 4 0 0 0-4-4H4"/></svg>"#
);
const INFO: &str = concat!(
    r#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">"#,
    r#"<circle cx="12" cy="12" r="10"/><path d="M12 16v-4"/><path d="M12 8h.01"/></svg>"#
);
const HELP_CIRCLE: &str = concat!(
    r#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">"#,
    r#"<circle cx="12" cy="12" r="10"/><path d="M9.09 9a3 3 0 0 1 5.83 1c0 2-3 3-3 3"/><path d="M12 17h.01"/></svg>"#
);
const PANEL_LEFT: &str = concat!(
    r#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">"#,
    r#"<rect width="18" height="18" x="3" y="3" rx="2"/><path d="M9 3v18"/></svg>"#
);

// Compile-time assertion so future edits to SVG_HEADER stay in sync.
const _: &str = SVG_HEADER;

/// The difit logotype (text + boxed glyph). Lifted verbatim from
/// `src/client/components/Logo.tsx`.
const DIFIT_LOGO: &str = r##"<svg viewBox="0 0 720 190" fill="none" xmlns="http://www.w3.org/2000/svg"><path fill-rule="evenodd" clip-rule="evenodd" d="M440.487 5.90627C432.164 9.01695 427.318 16.2817 427.318 25.649C427.318 35.9474 433.206 43.6458 442.714 45.7819C451.336 47.7188 459.606 44.3893 464.84 36.8737C466.923 33.8843 467.35 32.4073 467.606 27.3105C467.773 23.9957 467.492 20.0154 466.983 18.4654C465.699 14.5629 460.869 9.13088 456.854 7.07513C452.57 4.88086 444.73 4.32102 440.487 5.90627ZM596.658 6.653C588.927 9.55794 584.572 15.4908 583.887 24.0465C583.17 33.0212 586.83 39.8868 594.382 43.7278C603.739 48.4877 615.574 44.5081 620.482 34.9515C625.35 25.4728 622.15 13.4916 613.37 8.32678C608.986 5.74808 601.154 4.96365 596.658 6.653ZM368.585 7.68006C367.464 8.80629 367.302 12.903 367.302 40.2286V71.4878L363.832 68.1329C353.432 58.0787 335.595 52.668 319.246 54.6082C310.542 55.641 299.81 59.1369 293.3 63.0607C278.56 71.9427 266.453 89.5576 263.139 106.945C261.765 114.155 261.602 129.431 262.825 136.365C265.723 152.796 274.795 168.36 286.716 177.355C298.275 186.077 310.237 190 325.274 190C340.687 190 353.943 185.273 362.869 176.594L367.302 172.284V178.215C367.302 186.902 366.946 186.724 384.303 186.709C392.243 186.702 399.29 186.347 399.964 185.919C401.04 185.236 401.189 174.47 401.189 97.1224C401.189 23.7236 401.003 8.87842 400.068 7.74809C398.433 5.77021 370.547 5.71037 368.585 7.68006ZM526.937 7.94153C519.783 10.0325 515.359 12.5834 510.029 17.6867C501.959 25.4129 498.766 34.4597 498.766 49.5975V58.8517H490.659C484.378 58.8517 482.263 59.1418 481.268 60.1394C480.214 61.1976 479.986 63.6558 479.986 73.91C479.986 82.5264 480.289 86.6977 480.966 87.3764C481.601 88.0141 484.903 88.36 490.356 88.36H498.766V136.692C498.766 183.638 498.811 185.048 500.345 185.873C502.433 186.994 529.394 186.994 531.482 185.873C533.016 185.048 533.061 183.638 533.061 136.692V88.36H549.334C563.038 88.36 565.809 88.1568 566.89 87.0723C568.782 85.1723 568.782 62.0394 566.89 60.1394C565.808 59.0541 563.029 58.8517 549.231 58.8517H532.856L533.216 51.9721C533.604 44.5499 534.721 41.9147 538.546 39.3958C540.377 38.1901 543.049 37.904 554.661 37.6737C562.626 37.5155 569.018 37.0343 569.602 36.5483C570.892 35.4761 571.046 10.3579 569.777 7.9776C568.978 6.47923 567.918 6.39562 550.384 6.45054C535.269 6.49808 530.93 6.77431 526.937 7.94153ZM654.931 26.4834C653.136 27.7662 653.094 28.1572 653.094 43.3245V58.8517H644.567C633.342 58.8517 633.497 58.6443 633.497 73.6059C633.497 88.5674 633.342 88.36 644.567 88.36H653.094V122.636C653.094 159.998 653.439 163.935 657.362 171.354C661.848 179.838 669.111 184.99 680.857 188.022C687.996 189.866 707.647 189.269 714.309 187.007C719.756 185.158 719.86 184.87 719.969 171.298C720.082 157.364 720.441 157.791 709.174 158.48C695.386 159.324 689.341 156.938 687.388 149.883C686.927 148.214 686.58 134.435 686.577 117.663L686.572 88.36H702.068H717.564L718.87 86.3592C719.946 84.7108 720.129 82.3461 719.91 72.9575C719.719 64.8279 719.321 61.1689 718.522 60.2041C717.553 59.0345 715.315 58.8517 701.987 58.8517H686.572L686.546 43.8926C686.526 31.8556 686.28 28.613 685.288 27.2941C684.156 25.7884 682.945 25.6351 670.412 25.4129C658.94 25.2088 656.476 25.3793 654.931 26.4834ZM431.459 60.1394C430.316 61.2877 430.178 68.0001 430.188 121.82C430.197 168.838 430.429 182.667 431.232 184.262L432.264 186.311L446.657 186.535C458.941 186.725 461.242 186.567 462.353 185.452C463.521 184.279 463.655 177.845 463.655 122.786C463.655 68.0985 463.515 61.2877 462.372 60.1394C460.469 58.2287 433.362 58.2287 431.459 60.1394ZM587.799 60.4369C587.238 61.4894 586.954 82.56 586.954 123.084C586.954 177.502 587.093 184.285 588.236 185.434C590.138 187.342 616.431 187.342 618.333 185.434C619.476 184.285 619.615 177.447 619.615 122.482C619.615 75.6149 619.38 60.5828 618.636 59.8353C617.947 59.1443 613.344 58.8517 603.15 58.8517C589.644 58.8517 588.586 58.9607 587.799 60.4369ZM347.956 87.3469C362.023 93.9543 369.138 107.306 367.775 124.535C366.623 139.097 359.678 149.866 347.83 155.463C337.073 160.544 325.541 160.523 315.049 155.403C293.381 144.83 288.765 112.315 306.417 94.5961C313.92 87.0641 322.021 84.1412 334.301 84.5354C341.49 84.7666 343.224 85.124 347.956 87.3469Z" fill="currentColor"/><path fill-rule="evenodd" clip-rule="evenodd" d="M9.42299 0.744751C4.61699 2.15275 2.03999 5.66375 0.956994 12.2768C-0.369006 20.3728 -0.294006 170.791 1.03899 176.572C2.29699 182.031 4.81699 185.308 9.65399 187.774C13.265 189.615 16.865 189.684 108.63 189.684C163.129 189.684 204.095 189.304 204.41 188.795C206 188.5 206 188.795 207.5 188C211.167 187.079 216.779 180.138 218.005 175.584C218.682 173.068 218.939 143.952 218.756 90.4718C218.487 11.7478 218.417 9.10275 216.535 6.62675C215.466 5.22075 213.441 3.19575 212.035 2.12675C209.546 0.23575 206.839 0.17975 110.978 0.0167504C55.373 -0.0782496 11.148 0.239751 9.42299 0.744751ZM203.778 14.8838C205.558 16.6638 205.509 172.577 203.728 175.005C202.577 176.574 198.865 176.688 156.978 176.446L111.478 176.184L111.226 138.684L110.974 101.184H84.059C69.255 101.184 56.808 101.52 56.397 101.931C55.986 102.343 60.674 107.82 66.814 114.104C72.954 120.388 77.978 126.202 77.978 127.026C77.978 129.577 74.769 134.267 71.896 135.916C69.609 137.229 68.75 137.278 66.704 136.217C63.533 134.572 28.705 99.1448 27.757 96.5998C27.36 95.5348 27.491 93.4308 28.049 91.9238C28.955 89.4768 68.498 49.6838 70.024 49.6838C70.366 49.6838 72.356 51.1627 74.447 52.9697C77.589 55.6867 78.118 56.6658 77.498 58.6198C77.086 59.9198 72.075 65.8308 66.363 71.7568C60.651 77.6818 55.978 83.0148 55.978 83.6068C55.978 84.3488 64.068 84.6838 81.952 84.6838H107.927H110.666C111.144 82.5 110.978 73.4537 110.978 46.2337V13.6838H156.778C191.067 13.6838 202.88 13.9858 203.778 14.8838ZM148.803 52.7217C146.51 54.0597 144.845 59.5997 145.74 62.9107C146.146 64.4107 150.657 69.9308 155.764 75.1788C161.411 80.9788 164.547 84.8988 163.764 85.1788C163.057 85.4318 151.549 85.7608 138.19 85.9108C117.896 86.1388 113.747 86.4308 112.956 87.6838C112.435 88.5088 112.002 91.7228 111.993 94.8268C111.982 99.0598 112.373 100.622 113.56 101.077C114.43 101.411 126.13 101.684 139.56 101.684C156.121 101.684 163.978 102.026 163.978 102.746C163.978 103.33 160.187 107.607 155.554 112.251C146.244 121.582 144.276 125.371 146.069 130.514C147.321 134.107 149.576 135.684 153.463 135.684C155.522 135.684 159.843 131.902 173.778 117.904C191.699 99.9008 194.978 96.0678 194.978 93.1208C194.978 91.1338 157.422 53.8238 154.163 52.5728C151.322 51.4818 150.908 51.4937 148.803 52.5728Z" fill="currentColor"/></svg>"##;
