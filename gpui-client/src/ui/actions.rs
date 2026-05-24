//! Callback plumbing for diff row interactions. Letting the diff /
//! file-list / comment views talk back to `DifitApp` through an opaque
//! `DiffActions` keeps the UI layer from depending on the app type.

use std::sync::Arc;

use gpui::{App, Window};

use crate::api::types::DiffSide;
use crate::ui::diff_rows::CommentAnchor;

#[derive(Debug, Clone)]
pub enum DiffAction {
    /// User clicked the "+" affordance on a diff row.
    StartComposeAt {
        file_path: String,
        anchor: CommentAnchor,
    },
    /// User clicked "Reply" on a thread.
    StartReply {
        thread_id: String,
        anchor: CommentAnchor,
    },
    /// User clicked "Edit" on a message.
    StartEdit {
        thread_id: String,
        message_id: String,
        body: String,
        anchor: CommentAnchor,
    },
    /// User clicked "Delete" on a message.
    DeleteMessage {
        thread_id: String,
        message_id: String,
    },
    /// Remove the entire thread.
    DeleteThread { thread_id: String },
    /// Copy a single thread as a prompt.
    CopyPromptThread { thread_id: String },
    /// Open `file_path` at a specific line.
    OpenInEditor {
        file_path: String,
        side: DiffSide,
        line: u32,
    },
    /// Expand more surrounding context for a chunk in a file.
    ExpandContext {
        file_path: String,
        chunk_idx: usize,
        direction: ExpandDirection,
    },
    /// Toggle file-level state from a FileHeader row.
    ToggleViewed { file_path: String },
    ToggleCollapsed { file_path: String },
    TogglePreview { file_path: String },
    /// Scroll the main diff list to a file's first row.
    SelectFile { file_path: String },
    /// Open the file in the editor (no specific line).
    OpenFileInEditor { file_path: String },
    /// Copy all comments for a file.
    CopyAllPromptForFile { file_path: String },
    /// Mouse went down on the scrollbar thumb. Carries the snapshot
    /// needed to translate later DragMove events into row indices.
    ScrollbarDragStart {
        mouse_y: f32,
        start_top_item: f32,
        scroll_range_items: f32,
        track_space_px: f32,
    },
    /// Mouse moved while the thumb is being dragged.
    ScrollbarDragMove { mouse_y: f32 },
    /// Mouse released; clear drag state.
    ScrollbarDragEnd,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExpandDirection {
    Above,
    Below,
    /// Used only by the Expand row that sits between two chunks. The
    /// renderer shows both an up and a down arrow; clicks dispatch
    /// separate `Above` / `Below` actions to the respective chunks.
    Both,
}

pub type DiffActions = Arc<dyn Fn(DiffAction, &mut Window, &mut App) + 'static>;
