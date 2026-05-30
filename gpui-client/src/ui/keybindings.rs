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
        ComposeSubmit,
        ToggleViewMode,
        ToggleIgnoreWhitespace,
        ToggleMergeBase,
        Refresh,
        OpenInEditor,
        ToggleHelp,
        Escape,
        CopySelection,
    ]
);

pub fn bind_keys(cx: &mut App) {
    // The unmodified letter shortcuts must NOT fire while a text input
    // is focused — otherwise typing "r" in the file-filter input
    // triggers a Refresh instead of inserting the character. GPUI key
    // dispatch walks the focused element's context stack; predicating
    // on `!TextInput` excludes any keybinding lookup that happens
    // while a TextInput's context is on the stack.
    let app_ctx = Some("DifitApp && !TextInput");
    // Escape stays global on `DifitApp` so the user can still close
    // modals / cancel compose by pressing Escape from inside a focused
    // input.
    let app_or_input = Some("DifitApp");
    cx.bind_keys([
        KeyBinding::new("j", NextRow, app_ctx),
        KeyBinding::new("k", PrevRow, app_ctx),
        KeyBinding::new("n", NextFile, app_ctx),
        KeyBinding::new("p", PrevFile, app_ctx),
        KeyBinding::new("c", Compose, app_ctx),
        KeyBinding::new("v", ToggleViewMode, app_ctx),
        KeyBinding::new("w", ToggleIgnoreWhitespace, app_ctx),
        KeyBinding::new("m", ToggleMergeBase, app_ctx),
        KeyBinding::new("r", Refresh, app_ctx),
        KeyBinding::new("o", OpenInEditor, app_ctx),
        KeyBinding::new("shift-/", ToggleHelp, app_ctx),
        KeyBinding::new("escape", Escape, app_or_input),
        // Cmd/Ctrl+Enter submits the compose form. Matches React's
        // CommentForm shortcut. Bound on `DifitApp` so it still fires
        // while the TextInput is focused.
        KeyBinding::new("cmd-enter", ComposeSubmit, app_or_input),
        KeyBinding::new("ctrl-enter", ComposeSubmit, app_or_input),
        // Cmd/Ctrl+C copies the current diff text selection. Scoped
        // with `!TextInput` so it doesn't shadow the input's own Copy
        // binding when an input is focused.
        KeyBinding::new("cmd-c", CopySelection, app_ctx),
        KeyBinding::new("ctrl-c", CopySelection, app_ctx),
    ]);
}
