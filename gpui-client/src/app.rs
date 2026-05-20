use std::collections::HashSet;
use std::sync::Arc;

use gpui::{
    div, prelude::*, px, App, ClipboardItem, Context, Entity, FocusHandle, Focusable, IntoElement,
    ListAlignment, ListState, ParentElement, SharedString, Styled, Window,
};

use crate::api::client::{CommentSelectionQuery, DiffQuery, WatchEvent};
use crate::api::types::{
    DiffCommentMessage, DiffCommentPosition, DiffCommentThread, DiffLineRange, DiffResponse,
    DiffSide, RevisionsResponse,
};
use crate::api::ApiClient;
use crate::ui::actions::{DiffAction, DiffActions};
use crate::ui::comments_list_modal::render_comments_list_modal;
use crate::ui::compose_bar::render_compose_bar;
use crate::ui::diff_rows::{build_rows, CommentAnchor, DiffRow};
use crate::ui::diff_view::{count_threads_for_file, render_diff, DiffViewMode, RenderedDiff};
use crate::ui::file_list::render_file_list;
use crate::ui::help_modal::render_help_modal;
use crate::ui::keybindings::{
    Compose, Escape, NextFile, NextRow, OpenInEditor, PrevFile, PrevRow, Refresh, ToggleHelp,
    ToggleIgnoreWhitespace, ToggleMergeBase, ToggleViewMode,
};
use crate::ui::revision_modal::render_revision_modal;
use crate::ui::revision_picker::{render_revision_picker, RevisionRole};
use crate::ui::text_input::{InputMode, TextInput};
use crate::ui::theme::{Theme, UI_FONT};
use crate::viewed_store::{is_auto_viewed, ViewedStore};

pub struct DifitApp {
    api: Arc<ApiClient>,
    diff: Option<Arc<DiffResponse>>,
    /// Bumped every time `diff` is replaced. Lets the rendered-rows cache
    /// notice that the underlying diff has changed even when the file path
    /// and view mode haven't.
    diff_generation: u64,
    selected: Option<usize>,
    status: SharedString,
    view_mode: DiffViewMode,
    revisions: Option<Arc<RevisionsResponse>>,
    base_picker_open: bool,
    target_picker_open: bool,
    selected_base: Option<String>,
    selected_target: Option<String>,
    comments: Arc<Vec<DiffCommentThread>>,
    comments_version: u64,
    rendered_cache: Option<RenderedCacheEntry>,
    composing: Option<ComposeState>,
    ignore_whitespace: bool,
    use_merge_base: bool,
    focus_handle: FocusHandle,
    show_help: bool,
    show_revision_modal: bool,
    show_comments_list: bool,
    /// Index into the current `rendered_cache.rows` for the keyboard-
    /// focused row. Skips non-anchorable rows during j/k navigation.
    selected_row: Option<usize>,
    viewed: ViewedStore,
    /// Repos for which we've already run the auto-viewed pass this
    /// process. Prevents re-marking on every diff refresh.
    auto_viewed_done: HashSet<String>,
}

struct ComposeState {
    file_path: String,
    side: DiffSide,
    line_input: Entity<TextInput>,
    body_input: Entity<TextInput>,
    mode: ComposeMode,
}

#[derive(Debug, Clone)]
enum ComposeMode {
    New,
    Reply { thread_id: String },
    Edit { thread_id: String, message_id: String },
}

#[derive(PartialEq, Eq, Clone)]
struct RenderedCacheKey {
    file_path: String,
    view_mode: DiffViewMode,
    diff_generation: u64,
    comments_version: u64,
}

struct RenderedCacheEntry {
    key: RenderedCacheKey,
    rows: Arc<Vec<DiffRow>>,
    list_state: ListState,
}

impl DifitApp {
    pub fn new(api: Arc<ApiClient>, window: &mut Window, cx: &mut App) -> Entity<Self> {
        let view = cx.new(|cx| Self {
            api: api.clone(),
            diff: None,
            diff_generation: 0,
            selected: None,
            status: SharedString::from("Loading…"),
            view_mode: DiffViewMode::Unified,
            revisions: None,
            base_picker_open: false,
            target_picker_open: false,
            selected_base: None,
            selected_target: None,
            comments: Arc::new(Vec::new()),
            comments_version: 0,
            rendered_cache: None,
            composing: None,
            ignore_whitespace: false,
            use_merge_base: false,
            focus_handle: cx.focus_handle(),
            show_help: false,
            show_revision_modal: false,
            show_comments_list: false,
            selected_row: None,
            viewed: ViewedStore::load(),
            auto_viewed_done: HashSet::new(),
        });

        let handle = view.read(cx).focus_handle.clone();
        window.focus(&handle, cx);

        view.update(cx, |this, cx| {
            this.refresh_diff(cx);
            this.refresh_revisions(cx);
            this.refresh_comments(cx);
            this.start_live_updates(cx);
        });
        view
    }

    fn refresh_diff(&mut self, cx: &mut Context<Self>) {
        self.status = SharedString::from("Fetching diff…");
        let mut query = DiffQuery::from_selection(
            self.selected_base.as_deref(),
            self.selected_target.as_deref(),
        );
        if self.ignore_whitespace {
            query.ignore_whitespace = Some(true);
        }
        if self.use_merge_base {
            query.base_mode = Some("merge-base".to_string());
        }
        let rx = self.api.fetch_diff(query);
        cx.spawn(async move |this, cx| {
            let result = rx.await;
            this.update(cx, |this, cx| {
                match result {
                    Ok(Ok(diff)) => {
                        let summary = format!(
                            "{} • {} file(s)",
                            short_commit(&diff),
                            diff.files.len()
                        );
                        if let Some(base) = diff
                            .base_commitish
                            .clone()
                            .or_else(|| diff.requested_base_commitish.clone())
                        {
                            this.selected_base = Some(base);
                        }
                        if let Some(target) = diff
                            .target_commitish
                            .clone()
                            .or_else(|| diff.requested_target_commitish.clone())
                        {
                            this.selected_target = Some(target);
                        }
                        let file_count = diff.files.len();
                        let repo_id = diff.repository_id.clone();
                        this.diff = Some(Arc::new(diff));
                        this.diff_generation = this.diff_generation.wrapping_add(1);
                        this.rendered_cache = None;
                        this.apply_auto_viewed(repo_id.as_deref());
                        this.refresh_comments(cx);
                        this.selected = match this.selected {
                            Some(i) if i < file_count => Some(i),
                            _ if file_count > 0 => Some(0),
                            _ => None,
                        };
                        this.status = SharedString::from(summary);
                    }
                    Ok(Err(e)) => {
                        log::error!("diff fetch failed: {e:#}");
                        this.status = SharedString::from(format!("Error: {e}"));
                    }
                    Err(_) => {
                        this.status = SharedString::from("Diff fetch cancelled");
                    }
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn refresh_revisions(&mut self, cx: &mut Context<Self>) {
        let rx = self.api.fetch_revisions();
        cx.spawn(async move |this, cx| {
            let result = rx.await;
            this.update(cx, |this, cx| {
                match result {
                    Ok(Ok(revisions)) => {
                        this.revisions = Some(Arc::new(revisions));
                    }
                    Ok(Err(e)) => {
                        log::warn!("revisions fetch failed: {e:#}");
                    }
                    Err(_) => {}
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn refresh_comments(&mut self, cx: &mut Context<Self>) {
        let query = CommentSelectionQuery {
            base: self.selected_base.clone(),
            target: self.selected_target.clone(),
            base_mode: None,
        };
        let rx = self.api.fetch_comments(&query);
        cx.spawn(async move |this, cx| {
            let result = rx.await;
            this.update(cx, |this, cx| {
                match result {
                    Ok(Ok(payload)) => {
                        this.comments = Arc::new(payload.threads);
                        this.comments_version = payload.version;
                        this.rendered_cache = None;
                    }
                    Ok(Err(e)) => {
                        log::warn!("comments fetch failed: {e:#}");
                    }
                    Err(_) => {}
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn start_live_updates(&mut self, cx: &mut Context<Self>) {
        self.api.start_heartbeat();
        let mut rx = self.api.watch_stream();
        cx.spawn(async move |this, cx| {
            while let Some(event) = rx.recv().await {
                let updated = this.update(cx, |this, cx| match event {
                    WatchEvent::FilesChanged => {
                        log::info!("watch: filesChanged → refreshing diff");
                        this.refresh_diff(cx);
                    }
                    WatchEvent::CommentsChanged { version } => {
                        if version > this.comments_version {
                            log::info!("watch: commentsChanged v={version} → refetching");
                            this.refresh_comments(cx);
                        }
                    }
                    WatchEvent::Other(payload) => {
                        log::debug!("watch: unhandled event {payload}");
                    }
                });
                if updated.is_err() {
                    break;
                }
            }
        })
        .detach();
    }

    fn start_compose(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.open_compose_with(
            ComposeMode::New,
            DiffSide::New,
            "",
            "",
            "Write a comment…",
            window,
            cx,
        );
    }

    fn start_compose_at(
        &mut self,
        anchor: CommentAnchor,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.open_compose_with(
            ComposeMode::New,
            anchor.side,
            &anchor.line.to_string(),
            "",
            "Write a comment…",
            window,
            cx,
        );
    }

    fn start_reply(
        &mut self,
        thread_id: String,
        anchor: CommentAnchor,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.open_compose_with(
            ComposeMode::Reply { thread_id },
            anchor.side,
            &anchor.line.to_string(),
            "",
            "Write a reply…",
            window,
            cx,
        );
    }

    fn start_edit(
        &mut self,
        thread_id: String,
        message_id: String,
        body: String,
        anchor: CommentAnchor,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.open_compose_with(
            ComposeMode::Edit {
                thread_id,
                message_id,
            },
            anchor.side,
            &anchor.line.to_string(),
            &body,
            "Edit comment…",
            window,
            cx,
        );
    }

    fn open_compose_with(
        &mut self,
        mode: ComposeMode,
        side: DiffSide,
        initial_line: &str,
        initial_body: &str,
        placeholder: &'static str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(file_path) = self.current_file_path() else {
            return;
        };
        let initial_line_placeholder = if initial_line.is_empty() {
            "1"
        } else {
            initial_line
        };
        let line_input = cx.new(|cx| {
            let mut input = TextInput::new(
                InputMode::SingleLine,
                SharedString::from(initial_line_placeholder.to_string()),
                cx,
            );
            if !initial_line.is_empty() {
                input.set_content(initial_line);
            }
            input
        });
        let body_input = cx.new(|cx| {
            let mut input = TextInput::new(InputMode::MultiLine, placeholder, cx);
            if !initial_body.is_empty() {
                input.set_content(initial_body);
            }
            input
        });
        let body_handle = body_input.read(cx).focus_handle();
        self.composing = Some(ComposeState {
            file_path,
            side,
            line_input,
            body_input,
            mode,
        });
        window.focus(&body_handle, cx);
        cx.notify();
    }

    fn cancel_compose(&mut self, cx: &mut Context<Self>) {
        self.composing = None;
        cx.notify();
    }

    fn toggle_compose_side(&mut self, side: DiffSide, cx: &mut Context<Self>) {
        if let Some(state) = &mut self.composing {
            state.side = side;
            cx.notify();
        }
    }

    fn submit_compose(&mut self, cx: &mut Context<Self>) {
        let Some(state) = &self.composing else {
            return;
        };
        let line_text = state.line_input.read(cx).content().trim().to_string();
        let body_text = state.body_input.read(cx).content().trim().to_string();

        let Ok(line) = line_text.parse::<u32>() else {
            log::warn!("compose: invalid line number {line_text:?}");
            return;
        };
        if line == 0 {
            log::warn!("compose: line must be ≥ 1");
            return;
        }
        if body_text.is_empty() {
            log::warn!("compose: empty body");
            return;
        }

        let file_path = state.file_path.clone();
        let side = state.side;
        let mode = state.mode.clone();
        let now = String::new(); // server fills in

        let mut threads: Vec<DiffCommentThread> = (*self.comments).clone();
        match mode {
            ComposeMode::New => {
                let id = next_thread_id();
                threads.push(DiffCommentThread {
                    id: id.clone(),
                    file_path,
                    created_at: now.clone(),
                    updated_at: now.clone(),
                    position: DiffCommentPosition {
                        side,
                        line: DiffLineRange::Single(line),
                    },
                    code_snapshot: None,
                    messages: vec![DiffCommentMessage {
                        id,
                        body: body_text,
                        author: None,
                        created_at: now.clone(),
                        updated_at: now,
                    }],
                });
            }
            ComposeMode::Reply { thread_id } => {
                let Some(thread) = threads.iter_mut().find(|t| t.id == thread_id) else {
                    log::warn!("compose reply: thread {thread_id} not found");
                    return;
                };
                let id = next_thread_id();
                thread.updated_at = now.clone();
                thread.messages.push(DiffCommentMessage {
                    id,
                    body: body_text,
                    author: None,
                    created_at: now.clone(),
                    updated_at: now,
                });
            }
            ComposeMode::Edit {
                thread_id,
                message_id,
            } => {
                let Some(thread) = threads.iter_mut().find(|t| t.id == thread_id) else {
                    log::warn!("compose edit: thread {thread_id} not found");
                    return;
                };
                let Some(msg) = thread.messages.iter_mut().find(|m| m.id == message_id) else {
                    log::warn!("compose edit: message {message_id} not found");
                    return;
                };
                msg.body = body_text;
                msg.updated_at = now.clone();
                thread.updated_at = now;
            }
        }

        let query = self.comment_query();
        let rx = self.api.post_comments(&query, threads);
        cx.spawn(async move |this, cx| {
            let result = rx.await;
            this.update(cx, |this, cx| {
                match result {
                    Ok(Ok(())) => {
                        this.composing = None;
                    }
                    Ok(Err(e)) => {
                        log::error!("compose post failed: {e:#}");
                    }
                    Err(_) => {}
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn delete_thread(&mut self, thread_id: String, cx: &mut Context<Self>) {
        let mut threads: Vec<DiffCommentThread> = (*self.comments).clone();
        let before = threads.len();
        threads.retain(|t| t.id != thread_id);
        if threads.len() == before {
            return;
        }
        self.post_threads(threads, cx);
    }

    fn delete_message(
        &mut self,
        thread_id: String,
        message_id: String,
        cx: &mut Context<Self>,
    ) {
        let mut threads: Vec<DiffCommentThread> = (*self.comments).clone();
        let Some(thread) = threads.iter_mut().find(|t| t.id == thread_id) else {
            return;
        };
        thread.messages.retain(|m| m.id != message_id);
        if thread.messages.is_empty() {
            threads.retain(|t| t.id != thread_id);
        }
        self.post_threads(threads, cx);
    }

    fn post_threads(&mut self, threads: Vec<DiffCommentThread>, cx: &mut Context<Self>) {
        let query = self.comment_query();
        let rx = self.api.post_comments(&query, threads);
        cx.spawn(async move |this, cx| {
            let result = rx.await;
            this.update(cx, |_this, cx| {
                if let Ok(Err(e)) = result {
                    log::error!("post_threads failed: {e:#}");
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn comment_query(&self) -> CommentSelectionQuery {
        CommentSelectionQuery {
            base: self.selected_base.clone(),
            target: self.selected_target.clone(),
            base_mode: None,
        }
    }

    fn copy_prompt_thread(&self, thread_id: String, cx: &mut App) {
        let Some(thread) = self.comments.iter().find(|t| t.id == thread_id) else {
            return;
        };
        let text = format_thread_prompt(thread);
        cx.write_to_clipboard(ClipboardItem::new_string(text));
    }

    fn copy_all_prompts_for_file(&self, file_path: &str, cx: &mut App) {
        let mut blocks: Vec<String> = Vec::new();
        for thread in self.comments.iter().filter(|t| t.file_path == file_path) {
            blocks.push(format_thread_prompt(thread));
        }
        if blocks.is_empty() {
            return;
        }
        cx.write_to_clipboard(ClipboardItem::new_string(blocks.join("\n\n")));
    }

    fn open_in_editor(&self, line: Option<u32>, cx: &mut Context<Self>) {
        let Some(file_path) = self.current_file_path() else {
            return;
        };
        let rx = self.api.open_in_editor(file_path, line);
        cx.spawn(async move |_this, _cx| {
            if let Ok(Err(e)) = rx.await {
                log::error!("open in editor failed: {e:#}");
            }
        })
        .detach();
    }

    fn toggle_ignore_whitespace(&mut self, cx: &mut Context<Self>) {
        self.ignore_whitespace = !self.ignore_whitespace;
        self.refresh_diff(cx);
    }

    fn toggle_merge_base(&mut self, cx: &mut Context<Self>) {
        self.use_merge_base = !self.use_merge_base;
        self.refresh_diff(cx);
    }

    // -- Keyboard action handlers ---------------------------------------

    fn on_next_row(&mut self, _: &NextRow, _window: &mut Window, cx: &mut Context<Self>) {
        self.move_selected_row(1, cx);
    }
    fn on_prev_row(&mut self, _: &PrevRow, _window: &mut Window, cx: &mut Context<Self>) {
        self.move_selected_row(-1, cx);
    }
    fn on_next_file(&mut self, _: &NextFile, _window: &mut Window, cx: &mut Context<Self>) {
        self.move_selected_file(1, cx);
    }
    fn on_prev_file(&mut self, _: &PrevFile, _window: &mut Window, cx: &mut Context<Self>) {
        self.move_selected_file(-1, cx);
    }
    fn on_compose_key(&mut self, _: &Compose, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(anchor) = self.selected_row_anchor() {
            self.start_compose_at(anchor, window, cx);
        } else {
            self.start_compose(window, cx);
        }
    }
    fn on_toggle_view_mode(
        &mut self,
        _: &ToggleViewMode,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.view_mode = self.view_mode.toggle();
        self.rendered_cache = None;
        cx.notify();
    }
    fn on_toggle_ignore_whitespace(
        &mut self,
        _: &ToggleIgnoreWhitespace,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.toggle_ignore_whitespace(cx);
    }
    fn on_toggle_merge_base(
        &mut self,
        _: &ToggleMergeBase,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.toggle_merge_base(cx);
    }
    fn on_refresh_key(&mut self, _: &Refresh, _window: &mut Window, cx: &mut Context<Self>) {
        self.refresh_diff(cx);
    }
    fn on_open_in_editor_key(
        &mut self,
        _: &OpenInEditor,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let line = self.selected_row_anchor().map(|a| a.line);
        self.open_in_editor(line, cx);
    }
    fn on_toggle_help(
        &mut self,
        _: &ToggleHelp,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.show_help = !self.show_help;
        cx.notify();
    }
    fn on_escape(&mut self, _: &Escape, _window: &mut Window, cx: &mut Context<Self>) {
        if self.show_help {
            self.show_help = false;
            cx.notify();
        } else if self.show_revision_modal {
            self.show_revision_modal = false;
            cx.notify();
        } else if self.show_comments_list {
            self.show_comments_list = false;
            cx.notify();
        } else if self.composing.is_some() {
            self.cancel_compose(cx);
        } else if self.base_picker_open || self.target_picker_open {
            self.base_picker_open = false;
            self.target_picker_open = false;
            cx.notify();
        }
    }

    fn selected_row_anchor(&self) -> Option<CommentAnchor> {
        let cache = self.rendered_cache.as_ref()?;
        let idx = self.selected_row?;
        let row = cache.rows.get(idx)?;
        match row {
            DiffRow::Unified(cell) => cell.anchor,
            DiffRow::Split { left, right } => {
                left.as_ref().and_then(|c| c.anchor)
                    .or_else(|| right.as_ref().and_then(|c| c.anchor))
            }
            _ => None,
        }
    }

    fn move_selected_row(&mut self, dir: i32, cx: &mut Context<Self>) {
        let Some(cache) = self.rendered_cache.as_ref() else {
            return;
        };
        if cache.rows.is_empty() {
            return;
        }
        let anchorable: Vec<usize> = cache
            .rows
            .iter()
            .enumerate()
            .filter_map(|(i, row)| match row {
                DiffRow::Unified(cell) if cell.anchor.is_some() => Some(i),
                DiffRow::Split { left, right }
                    if left.as_ref().and_then(|c| c.anchor).is_some()
                        || right.as_ref().and_then(|c| c.anchor).is_some() =>
                {
                    Some(i)
                }
                _ => None,
            })
            .collect();
        if anchorable.is_empty() {
            return;
        }
        let current_pos = self
            .selected_row
            .and_then(|sel| anchorable.iter().position(|&i| i == sel));
        let next_pos = match (current_pos, dir) {
            (None, d) if d > 0 => 0,
            (None, _) => anchorable.len() - 1,
            (Some(p), d) if d > 0 => (p + 1).min(anchorable.len() - 1),
            (Some(p), _) => p.saturating_sub(1),
        };
        let idx = anchorable[next_pos];
        self.selected_row = Some(idx);
        cache.list_state.scroll_to_reveal_item(idx);
        cx.notify();
    }

    fn move_selected_file(&mut self, dir: i32, cx: &mut Context<Self>) {
        let Some(diff) = self.diff.as_ref() else {
            return;
        };
        let n = diff.files.len();
        if n == 0 {
            return;
        }
        let next = match (self.selected, dir) {
            (None, d) if d > 0 => 0,
            (None, _) => n - 1,
            (Some(i), d) if d > 0 => (i + 1).min(n - 1),
            (Some(i), _) => i.saturating_sub(1),
        };
        if self.selected != Some(next) {
            self.selected = Some(next);
            self.selected_row = None;
            self.rendered_cache = None;
            self.composing = None;
            cx.notify();
        }
    }

    fn jump_to_thread(&mut self, thread_id: String, cx: &mut Context<Self>) {
        let Some(thread) = self.comments.iter().find(|t| t.id == thread_id).cloned() else {
            return;
        };
        let path = thread.file_path;
        let anchor_line = match thread.position.line {
            DiffLineRange::Single(n) => n,
            DiffLineRange::Range { end, .. } => end,
        };
        let side = thread.position.side;

        let Some(diff) = self.diff.as_ref() else { return };
        let Some(idx) = diff.files.iter().position(|f| f.path == path) else {
            return;
        };
        if self.selected != Some(idx) {
            self.selected = Some(idx);
            self.rendered_cache = None;
            self.composing = None;
        }
        self.show_comments_list = false;

        if let Some(rendered) = self.ensure_rendered() {
            for (i, row) in rendered.rows.iter().enumerate() {
                let matches = match row {
                    DiffRow::Unified(cell) => cell
                        .anchor
                        .map(|a| a.side == side && a.line == anchor_line)
                        .unwrap_or(false),
                    DiffRow::Split { left, right } => {
                        let in_left = left
                            .as_ref()
                            .and_then(|c| c.anchor)
                            .map(|a| a.side == side && a.line == anchor_line)
                            .unwrap_or(false);
                        let in_right = right
                            .as_ref()
                            .and_then(|c| c.anchor)
                            .map(|a| a.side == side && a.line == anchor_line)
                            .unwrap_or(false);
                        in_left || in_right
                    }
                    _ => false,
                };
                if matches {
                    self.selected_row = Some(i);
                    rendered.list_state.scroll_to_reveal_item(i);
                    break;
                }
            }
        }
        cx.notify();
    }

    fn apply_auto_viewed(&mut self, repo_id: Option<&str>) {
        let Some(repo_id) = repo_id else { return };
        if !self.auto_viewed_done.insert(repo_id.to_string()) {
            return;
        }
        let Some(diff) = self.diff.as_ref() else { return };
        let mut changed = false;
        for f in &diff.files {
            if is_auto_viewed(&f.path) && !self.viewed.is_viewed(repo_id, &f.path) {
                self.viewed.set_viewed(repo_id, &f.path, true);
                changed = true;
            }
        }
        if changed {
            if let Err(e) = self.viewed.save() {
                log::warn!("viewed save failed: {e:#}");
            }
        }
    }

    fn toggle_viewed(&mut self, idx: usize, cx: &mut Context<Self>) {
        let Some(diff) = self.diff.clone() else { return };
        let Some(file) = diff.files.get(idx) else { return };
        let Some(repo_id) = diff.repository_id.as_deref() else { return };
        let new_state = !self.viewed.is_viewed(repo_id, &file.path);
        self.viewed.set_viewed(repo_id, &file.path, new_state);
        if let Err(e) = self.viewed.save() {
            log::warn!("viewed save failed: {e:#}");
        }
        cx.notify();
    }

    fn viewed_paths_for_current_repo(&self) -> HashSet<String> {
        self.diff
            .as_ref()
            .and_then(|d| d.repository_id.as_deref())
            .and_then(|repo_id| self.viewed.repos.get(repo_id).cloned())
            .unwrap_or_default()
    }

    fn build_actions(&self, entity: &Entity<DifitApp>) -> DiffActions {
        let entity = entity.clone();
        Arc::new(move |action, window, cx| {
            entity.update(cx, |this, cx| match action {
                DiffAction::StartComposeAt(anchor) => this.start_compose_at(anchor, window, cx),
                DiffAction::StartReply { thread_id, anchor } => {
                    this.start_reply(thread_id, anchor, window, cx)
                }
                DiffAction::StartEdit {
                    thread_id,
                    message_id,
                    body,
                    anchor,
                } => this.start_edit(thread_id, message_id, body, anchor, window, cx),
                DiffAction::DeleteMessage {
                    thread_id,
                    message_id,
                } => this.delete_message(thread_id, message_id, cx),
                DiffAction::DeleteThread { thread_id } => this.delete_thread(thread_id, cx),
                DiffAction::CopyPromptThread { thread_id } => this.copy_prompt_thread(thread_id, cx),
                DiffAction::OpenInEditor { side: _, line } => this.open_in_editor(Some(line), cx),
            });
        })
    }

    fn current_file_path(&self) -> Option<String> {
        let diff = self.diff.as_ref()?;
        let idx = self.selected?;
        diff.files.get(idx).map(|f| f.path.clone())
    }

    fn pick_revision(&mut self, role: RevisionRole, value: String, cx: &mut Context<Self>) {
        match role {
            RevisionRole::Base => self.selected_base = Some(value),
            RevisionRole::Target => self.selected_target = Some(value),
        }
        self.base_picker_open = false;
        self.target_picker_open = false;
        self.refresh_diff(cx);
        cx.notify();
    }

    /// Return the RenderedDiff for the currently selected file, building (or
    /// refreshing) the cache if needed. This is the hot path's only heavy
    /// step, and only runs when the cache key changes.
    fn ensure_rendered(&mut self) -> Option<RenderedDiff> {
        let diff = self.diff.as_ref()?;
        let idx = self.selected?;
        let file = diff.files.get(idx)?;

        let key = RenderedCacheKey {
            file_path: file.path.clone(),
            view_mode: self.view_mode,
            diff_generation: self.diff_generation,
            comments_version: self.comments_version,
        };

        let needs_rebuild = self
            .rendered_cache
            .as_ref()
            .map(|c| c.key != key)
            .unwrap_or(true);

        if needs_rebuild {
            let rows = build_rows(file, self.view_mode, self.comments.as_ref());
            let item_count = rows.len();
            let list_state = ListState::new(item_count, ListAlignment::Top, px(400.0));
            self.rendered_cache = Some(RenderedCacheEntry {
                key,
                rows: Arc::new(rows),
                list_state,
            });
        }

        self.rendered_cache.as_ref().map(|c| RenderedDiff {
            rows: c.rows.clone(),
            list_state: c.list_state.clone(),
        })
    }
}

fn format_thread_prompt(thread: &DiffCommentThread) -> String {
    let line_label = match thread.position.line {
        DiffLineRange::Single(n) => format!("L{n}"),
        DiffLineRange::Range { start, end } => format!("L{start}-L{end}"),
    };
    let header = format!("{}:{}", thread.file_path, line_label);
    let mut bodies = String::new();
    for (i, msg) in thread.messages.iter().enumerate() {
        if i > 0 {
            bodies.push_str("\n\n");
        }
        bodies.push_str(msg.body.trim());
    }
    format!("{header}\n{bodies}")
}

fn next_thread_id() -> String {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    format!("gpui-{nanos:x}")
}

fn short_commit(diff: &DiffResponse) -> String {
    let target = diff
        .target_commitish
        .clone()
        .unwrap_or_else(|| diff.commit.clone());
    let base = diff
        .base_commitish
        .clone()
        .unwrap_or_else(|| "—".to_string());
    let trunc = |s: &str| -> String {
        if s.len() > 12 {
            format!("{}…", &s[..12])
        } else {
            s.to_string()
        }
    };
    format!("{} ← {}", trunc(&target), trunc(&base))
}

impl Focusable for DifitApp {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for DifitApp {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let can_compose = self.current_file_path().is_some();
        let composing_active = self.composing.is_some();
        let show_help = self.show_help;
        let show_revision_modal = self.show_revision_modal;
        let show_comments_list = self.show_comments_list;
        // Snapshot lightweight state up-front; heavy data stays behind Arcs.
        let view_mode = self.view_mode;
        let revisions = self.revisions.clone();
        let selected_base = self.selected_base.clone();
        let selected_target = self.selected_target.clone();
        let base_open = self.base_picker_open;
        let target_open = self.target_picker_open;
        let status = self.status.clone();
        let ignore_whitespace = self.ignore_whitespace;
        let use_merge_base = self.use_merge_base;

        let rendered = self.ensure_rendered();
        let diff = self.diff.clone();
        let selected = self.selected;
        let comments = self.comments.clone();

        let entity = cx.entity();
        let actions = self.build_actions(&entity);
        let viewed_paths: HashSet<String> = self.viewed_paths_for_current_repo();

        // Avoid cloning the entire file list every frame — borrow it for
        // file_list rendering and active_file lookup via the same Arc.
        let files: &[crate::api::types::DiffFile] = diff
            .as_ref()
            .map(|d| d.files.as_slice())
            .unwrap_or(&[]);
        let active_file = selected.and_then(|i| files.get(i));
        let active_path = active_file.map(|f| f.path.clone());
        let thread_count = active_file
            .map(|f| count_threads_for_file(&f.path, comments.iter()))
            .unwrap_or(0);

        let root = div()
            .size_full()
            .flex()
            .flex_col()
            .key_context("DifitApp")
            .track_focus(&self.focus_handle)
            .on_action(cx.listener(Self::on_next_row))
            .on_action(cx.listener(Self::on_prev_row))
            .on_action(cx.listener(Self::on_next_file))
            .on_action(cx.listener(Self::on_prev_file))
            .on_action(cx.listener(Self::on_compose_key))
            .on_action(cx.listener(Self::on_toggle_view_mode))
            .on_action(cx.listener(Self::on_toggle_ignore_whitespace))
            .on_action(cx.listener(Self::on_toggle_merge_base))
            .on_action(cx.listener(Self::on_refresh_key))
            .on_action(cx.listener(Self::on_open_in_editor_key))
            .on_action(cx.listener(Self::on_toggle_help))
            .on_action(cx.listener(Self::on_escape))
            .bg(Theme::BG)
            .text_color(Theme::TEXT)
            .font_family(UI_FONT)
            .child(render_header(HeaderInputs {
                status,
                view_mode,
                revisions,
                selected_base,
                selected_target,
                base_open,
                target_open,
                can_compose,
                composing_active,
                ignore_whitespace,
                use_merge_base,
                active_path,
                entity: entity.clone(),
            }))
            .child(
                div()
                    .flex()
                    .flex_row()
                    .flex_1()
                    .min_h_0()
                    .child(render_file_list(
                        files,
                        selected,
                        &viewed_paths,
                        {
                            let entity = entity.clone();
                            move |idx, cx| {
                                entity.update(cx, |this, cx| {
                                    this.selected = Some(idx);
                                    this.selected_row = None;
                                    this.rendered_cache = None;
                                    this.composing = None;
                                    cx.notify();
                                });
                            }
                        },
                        {
                            let entity = entity.clone();
                            move |idx, cx| {
                                entity.update(cx, |this, cx| this.toggle_viewed(idx, cx));
                            }
                        },
                    ))
                    .child(self.render_diff_pane(
                        active_file,
                        rendered,
                        thread_count,
                        &entity,
                        actions,
                        cx,
                    )),
            );

        let root = if show_help {
            let entity_for_close = entity.clone();
            root.child(render_help_modal(move |cx| {
                entity_for_close.update(cx, |this, cx| {
                    this.show_help = false;
                    cx.notify();
                });
            }))
        } else {
            root
        };

        let root = if show_revision_modal {
            let entity_for_close = entity.clone();
            let diff_for_modal = self.diff.clone();
            let revs_for_modal = self.revisions.clone();
            let ignore_ws = self.ignore_whitespace;
            let merge_base = self.use_merge_base;
            root.child(render_revision_modal(
                diff_for_modal.as_deref(),
                revs_for_modal.as_ref(),
                ignore_ws,
                merge_base,
                move |cx| {
                    entity_for_close.update(cx, |this, cx| {
                        this.show_revision_modal = false;
                        cx.notify();
                    });
                },
            ))
        } else {
            root
        };

        if show_comments_list {
            let entity_for_jump = entity.clone();
            let entity_for_close = entity.clone();
            let threads = comments.clone();
            root.child(render_comments_list_modal(
                threads,
                move |_path, cx| {
                    // The on_jump callback gets a file_path string in the
                    // current API; we need a thread id, so do the lookup
                    // through the most recent thread for that file. The
                    // comments_list closure passes the file_path directly
                    // (see the row builder).
                    entity_for_jump.update(cx, |this, cx| {
                        // Find the first thread whose file matches and jump
                        // to it. Good enough since rows already include the
                        // file column.
                        if let Some(thread_id) = this
                            .comments
                            .iter()
                            .find(|t| t.file_path == _path)
                            .map(|t| t.id.clone())
                        {
                            this.jump_to_thread(thread_id, cx);
                        }
                    });
                },
                move |cx| {
                    entity_for_close.update(cx, |this, cx| {
                        this.show_comments_list = false;
                        cx.notify();
                    });
                },
            ))
        } else {
            root
        }
    }
}

impl DifitApp {
    fn render_diff_pane(
        &self,
        active_file: Option<&crate::api::types::DiffFile>,
        rendered: Option<RenderedDiff>,
        thread_count: usize,
        entity: &Entity<DifitApp>,
        actions: DiffActions,
        _cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let mut col = div()
            .flex_1()
            .min_h_0()
            .min_w_0()
            .flex()
            .flex_col()
            .child(render_diff(active_file, rendered, thread_count, actions));

        if let Some(state) = self.composing.as_ref() {
            let entity_s = entity.clone();
            let entity_c = entity.clone();
            let entity_t = entity.clone();
            col = col.child(render_compose_bar(
                SharedString::from(state.file_path.clone()),
                state.side,
                state.line_input.clone(),
                state.body_input.clone(),
                move |side, cx| {
                    entity_t.update(cx, |this, cx| this.toggle_compose_side(side, cx));
                },
                move |cx| {
                    entity_s.update(cx, |this, cx| this.submit_compose(cx));
                },
                move |cx| {
                    entity_c.update(cx, |this, cx| this.cancel_compose(cx));
                },
            ));
        }

        col
    }
}

struct HeaderInputs {
    status: SharedString,
    view_mode: DiffViewMode,
    revisions: Option<Arc<RevisionsResponse>>,
    selected_base: Option<String>,
    selected_target: Option<String>,
    base_open: bool,
    target_open: bool,
    can_compose: bool,
    composing_active: bool,
    ignore_whitespace: bool,
    use_merge_base: bool,
    active_path: Option<String>,
    entity: Entity<DifitApp>,
}

fn render_header(inputs: HeaderInputs) -> impl IntoElement {
    let HeaderInputs {
        status,
        view_mode,
        revisions,
        selected_base,
        selected_target,
        base_open,
        target_open,
        can_compose,
        composing_active,
        ignore_whitespace,
        use_merge_base,
        active_path,
        entity,
    } = inputs;

    let entity_a = entity.clone();
    let entity_b = entity.clone();
    let entity_c = entity.clone();
    let entity_d = entity.clone();
    let entity_e = entity.clone();
    let entity_f = entity.clone();
    let entity_g = entity.clone();
    let entity_h = entity.clone();
    let entity_ws = entity.clone();
    let entity_mb = entity.clone();
    let entity_open = entity.clone();
    let entity_copy = entity.clone();
    let entity_info = entity.clone();
    let entity_help = entity.clone();
    let entity_threads = entity.clone();
    let active_path_for_open = active_path.clone();
    let active_path_for_copy = active_path;

    div()
        .w_full()
        .h(px(40.0))
        .px_4()
        .flex()
        .flex_row()
        .items_center()
        .gap_3()
        .bg(Theme::BG_ELEVATED)
        .border_b_1()
        .border_color(Theme::BORDER)
        .child(
            div()
                .text_color(Theme::TEXT)
                .text_size(px(13.0))
                .font_weight(gpui::FontWeight::SEMIBOLD)
                .child(SharedString::from("difit")),
        )
        .child(render_revision_picker(
            RevisionRole::Base,
            selected_base.as_deref(),
            revisions.as_ref(),
            base_open,
            move |cx| {
                entity_a.update(cx, |this, cx| {
                    this.base_picker_open = !this.base_picker_open;
                    this.target_picker_open = false;
                    cx.notify();
                });
            },
            move |value, cx| {
                entity_b.update(cx, |this, cx| this.pick_revision(RevisionRole::Base, value, cx));
            },
            move |cx| {
                entity_c.update(cx, |this, cx| {
                    this.base_picker_open = false;
                    cx.notify();
                });
            },
        ))
        .child(render_revision_picker(
            RevisionRole::Target,
            selected_target.as_deref(),
            revisions.as_ref(),
            target_open,
            move |cx| {
                entity_d.update(cx, |this, cx| {
                    this.target_picker_open = !this.target_picker_open;
                    this.base_picker_open = false;
                    cx.notify();
                });
            },
            move |value, cx| {
                entity_e.update(cx, |this, cx| {
                    this.pick_revision(RevisionRole::Target, value, cx)
                });
            },
            move |cx| {
                entity_f.update(cx, |this, cx| {
                    this.target_picker_open = false;
                    cx.notify();
                });
            },
        ))
        .child(
            div()
                .flex_1()
                .text_size(px(12.0))
                .text_color(Theme::TEXT_MUTED)
                .child(status),
        )
        .child(toggle_header_button(
            "ws",
            "WS",
            ignore_whitespace,
            move |cx: &mut App| {
                entity_ws.update(cx, |this, cx| this.toggle_ignore_whitespace(cx));
            },
        ))
        .child(toggle_header_button(
            "mb",
            "merge-base",
            use_merge_base,
            move |cx: &mut App| {
                entity_mb.update(cx, |this, cx| this.toggle_merge_base(cx));
            },
        ))
        .child(header_button("view-mode", view_mode.label(), {
            let entity = entity_g.clone();
            move |cx: &mut App| {
                entity.update(cx, |this, cx| {
                    this.view_mode = this.view_mode.toggle();
                    this.rendered_cache = None;
                    cx.notify();
                });
            }
        }))
        .child(header_button("refresh", "Refresh", {
            let entity = entity_g;
            move |cx: &mut App| {
                entity.update(cx, |this, cx| this.refresh_diff(cx));
            }
        }))
        .child(header_button_enabled(
            "open-editor",
            "Open in editor",
            active_path_for_open.is_some(),
            move |cx: &mut App| {
                entity_open.update(cx, |this, cx| this.open_in_editor(None, cx));
            },
        ))
        .child(header_button_enabled(
            "copy-all",
            "Copy all",
            active_path_for_copy.is_some(),
            move |cx: &mut App| {
                if let Some(path) = active_path_for_copy.clone() {
                    entity_copy.update(cx, |this, _cx| this.copy_all_prompts_for_file(&path, _cx));
                }
            },
        ))
        .child(compose_header_button(
            can_compose,
            composing_active,
            move |window, cx| {
                entity_h.update(cx, |this, cx| {
                    if this.composing.is_some() {
                        this.cancel_compose(cx);
                    } else {
                        this.start_compose(window, cx);
                    }
                });
            },
        ))
        .child(header_button("comments-list", "Threads", {
            let entity = entity_threads;
            move |cx: &mut App| {
                entity.update(cx, |this, cx| {
                    this.show_comments_list = !this.show_comments_list;
                    cx.notify();
                });
            }
        }))
        .child(header_button("info", "Info", {
            let entity = entity_info;
            move |cx: &mut App| {
                entity.update(cx, |this, cx| {
                    this.show_revision_modal = !this.show_revision_modal;
                    cx.notify();
                });
            }
        }))
        .child(header_button("help", "?", {
            let entity = entity_help;
            move |cx: &mut App| {
                entity.update(cx, |this, cx| {
                    this.show_help = !this.show_help;
                    cx.notify();
                });
            }
        }))
}

fn toggle_header_button(
    id: &'static str,
    label: &'static str,
    active: bool,
    on_click: impl Fn(&mut App) + 'static,
) -> impl IntoElement {
    div()
        .id(id)
        .px_3()
        .py_1()
        .rounded_sm()
        .border_1()
        .border_color(if active { Theme::TEXT_LINK } else { Theme::BORDER })
        .bg(if active { Theme::BG_SELECTED } else { Theme::BG_ELEVATED })
        .text_color(if active { Theme::TEXT } else { Theme::TEXT_MUTED })
        .text_size(px(12.0))
        .cursor_pointer()
        .hover(|s| s.bg(Theme::BG_HOVER))
        .on_click(move |_e, _w, cx| on_click(cx))
        .child(SharedString::from(label))
}

fn header_button_enabled(
    id: &'static str,
    label: &'static str,
    enabled: bool,
    on_click: impl Fn(&mut App) + 'static,
) -> impl IntoElement {
    let mut btn = div()
        .id(id)
        .px_3()
        .py_1()
        .rounded_sm()
        .border_1()
        .border_color(Theme::BORDER)
        .text_size(px(12.0))
        .text_color(if enabled { Theme::TEXT } else { Theme::TEXT_MUTED })
        .child(SharedString::from(label));
    if enabled {
        btn = btn
            .cursor_pointer()
            .hover(|s| s.bg(Theme::BG_HOVER))
            .on_click(move |_e, _w, cx| on_click(cx));
    }
    btn
}

fn compose_header_button(
    enabled: bool,
    active: bool,
    on_click: impl Fn(&mut Window, &mut App) + 'static,
) -> impl IntoElement {
    let label = if active { "Close" } else { "+ Comment" };
    let mut btn = div()
        .id("compose-toggle")
        .px_3()
        .py_1()
        .rounded_sm()
        .border_1()
        .border_color(if active { Theme::TEXT_LINK } else { Theme::BORDER })
        .text_size(px(12.0))
        .text_color(if enabled { Theme::TEXT } else { Theme::TEXT_MUTED })
        .child(SharedString::from(label));
    if enabled {
        btn = btn
            .cursor_pointer()
            .hover(|s| s.bg(Theme::BG_HOVER))
            .on_click(move |_e, window, cx| on_click(window, cx));
    }
    btn
}

fn header_button(
    id: &'static str,
    label: &'static str,
    on_click: impl Fn(&mut App) + 'static,
) -> impl IntoElement {
    div()
        .id(id)
        .px_3()
        .py_1()
        .rounded_sm()
        .border_1()
        .border_color(Theme::BORDER)
        .text_size(px(12.0))
        .cursor_pointer()
        .hover(|s| s.bg(Theme::BG_HOVER))
        .on_click(move |_event, _window, cx| on_click(cx))
        .child(SharedString::from(label))
}
