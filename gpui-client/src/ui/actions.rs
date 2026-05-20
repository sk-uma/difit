//! Callback plumbing for diff row interactions. Letting the diff/comment
//! views talk back to `DifitApp` through an opaque `DiffActions` keeps
//! the UI layer from depending on the app type directly.

use std::sync::Arc;

use gpui::{App, Window};

use crate::api::types::DiffSide;
use crate::ui::diff_rows::CommentAnchor;

#[derive(Debug, Clone)]
pub enum DiffAction {
    /// User clicked the "+" affordance on a diff row.
    StartComposeAt(CommentAnchor),
    /// User clicked "Reply" on a thread.
    StartReply { thread_id: String, anchor: CommentAnchor },
    /// User clicked "Edit" on a message.
    StartEdit {
        thread_id: String,
        message_id: String,
        body: String,
        anchor: CommentAnchor,
    },
    /// User clicked "Delete" on a message. If it's the last message in the
    /// thread, the thread itself is removed.
    DeleteMessage {
        thread_id: String,
        message_id: String,
    },
    /// Remove the entire thread.
    DeleteThread { thread_id: String },
    /// Copy a single thread as a prompt.
    CopyPromptThread { thread_id: String },
    /// Open the active file in the configured editor at this line.
    OpenInEditor { side: DiffSide, line: u32 },
    /// Expand more surrounding context for the given chunk in the
    /// currently visible file.
    ExpandContext {
        chunk_idx: usize,
        direction: ExpandDirection,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExpandDirection {
    Above,
    Below,
}

pub type DiffActions = Arc<dyn Fn(DiffAction, &mut Window, &mut App) + 'static>;
