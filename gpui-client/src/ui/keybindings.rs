//! App-level keyboard actions and bindings.
//!
//! Each variant maps to a method on `DifitApp::on_action_*`. The bindings
//! are scoped to the `"DifitApp"` key context so they don't fire while a
//! `TextInput` is focused (which has its own `"TextInput"` context).

use gpui::{actions, App, KeyBinding};

actions!(
    difit_app,
    [
        NextFile,
        PrevFile,
        NextRow,
        PrevRow,
        Compose,
        ToggleViewMode,
        ToggleIgnoreWhitespace,
        ToggleMergeBase,
        Refresh,
        OpenInEditor,
        ToggleHelp,
        Escape,
    ]
);

pub fn bind_keys(cx: &mut App) {
    let ctx = Some("DifitApp");
    cx.bind_keys([
        KeyBinding::new("j", NextRow, ctx),
        KeyBinding::new("k", PrevRow, ctx),
        KeyBinding::new("n", NextFile, ctx),
        KeyBinding::new("p", PrevFile, ctx),
        KeyBinding::new("c", Compose, ctx),
        KeyBinding::new("v", ToggleViewMode, ctx),
        KeyBinding::new("w", ToggleIgnoreWhitespace, ctx),
        KeyBinding::new("m", ToggleMergeBase, ctx),
        KeyBinding::new("r", Refresh, ctx),
        KeyBinding::new("o", OpenInEditor, ctx),
        KeyBinding::new("shift-/", ToggleHelp, ctx),
        KeyBinding::new("escape", Escape, ctx),
    ]);
}
