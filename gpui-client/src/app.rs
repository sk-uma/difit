use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use gpui::{
    div, prelude::*, px, App, ClipboardItem, Context, Entity, FocusHandle, Focusable, IntoElement,
    ListAlignment, ListOffset, ListState, ParentElement, SharedString, Styled, Window,
};

use crate::api::client::{CommentSelectionQuery, DiffQuery, WatchEvent};
use crate::api::types::{
    DiffCommentMessage, DiffCommentPosition, DiffCommentThread, DiffLineRange, DiffResponse,
    DiffSide, RevisionsResponse,
};
use crate::api::ApiClient;
use crate::ui::actions::{DiffAction, DiffActions, ExpandDirection};
use crate::ui::comments_list_modal::render_comments_list_modal;
use crate::ui::compose_bar::render_compose_bar;
use crate::ui::diff_rows::{build_all_rows, expand_step, BuildContext, CommentAnchor, DiffRow, ExpansionMap};
use crate::ui::diff_view::{render_main_pane, DiffViewMode, RenderedDiff};
use crate::ui::file_list::render_file_list;
use crate::ui::help_modal::render_help_modal;
use crate::ui::image_viewer::is_image_ext;
use crate::ui::notebook_view::is_notebook_ext;
use crate::ui::keybindings::{
    Compose, Escape, NextFile, NextRow, OpenInEditor, PrevFile, PrevRow, Refresh, ToggleHelp,
    ToggleIgnoreWhitespace, ToggleMergeBase, ToggleViewMode,
};
use crate::ui::revision_modal::render_revision_modal;
use crate::ui::revision_picker::{render_revision_picker, RevisionRole};
use crate::settings_store::{self, Settings};
use crate::ui::settings_modal::render_settings_modal;
use crate::ui::text_input::{InputMode, TextInput};
use crate::ui::theme::{Theme, UI_FONT};
use crate::ui::quick_menu::{render_quick_menu, Preset as QuickPreset};
use crate::ui::widgets::{
    checkbox, icon_button, label_tooltip, logo, pill_toggle, viewed_progress,
};
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
    quick_menu_open: bool,
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
    show_settings_modal: bool,
    settings: Settings,
    settings_version: u64,
    /// Per-file expansion counts (above, below) for each chunk.
    expansions: HashMap<String, ExpansionMap>,
    /// Bumped whenever expansions change; part of the rendered-rows
    /// cache key.
    expansion_version: u64,
    /// Bumped whenever any UI-shape state (collapsed, viewed,
    /// preview_paths, blob_cache) changes.
    ui_version: u64,
    /// In-progress drag of the diff scrollbar thumb.
    scrollbar_drag: Option<ScrollbarDragState>,
    /// File-tree sidebar visibility (PanelLeft toggle).
    sidebar_open: bool,
    /// Paths the user has collapsed (in addition to auto-collapsed
    /// generated files).
    collapsed: HashSet<String>,
    /// Directories collapsed in the sidebar tree (default = all
    /// expanded).
    collapsed_dirs: HashSet<String>,
    /// Text input for the sidebar filter.
    file_filter: Entity<TextInput>,
    /// `/api/generated-status` results cached per (path, ref).
    generated_cache: HashMap<(String, String), bool>,
    /// Paths for which we've already evaluated the auto-collapse rule
    /// against the generated-status result.
    auto_collapse_done: HashSet<String>,
    /// Raw bytes for image / notebook / markdown / context-expansion
    /// blobs, keyed by (path, ref).
    blob_cache: HashMap<(String, String), Arc<Vec<u8>>>,
    /// (path, ref) pairs for which a blob fetch is already in flight.
    /// Prevents render() from re-issuing the same request.
    pending_blob_fetches: HashSet<(String, String)>,
    /// Markdown files the user has toggled into preview mode.
    preview_paths: HashSet<String>,
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

#[derive(Debug, Clone, Copy)]
pub struct ScrollbarDragState {
    start_mouse_y: f32,
    start_top_item: f32,
    /// scroll_range_items / track_space — converts mouse delta to a
    /// number of items to skip.
    items_per_drag_px: f32,
}

#[derive(Debug, Clone)]
enum ComposeMode {
    New,
    Reply { thread_id: String },
    Edit { thread_id: String, message_id: String },
}

#[derive(PartialEq, Eq, Clone)]
struct RenderedCacheKey {
    view_mode: DiffViewMode,
    diff_generation: u64,
    comments_version: u64,
    settings_version: u64,
    expansion_version: u64,
    ui_version: u64,
}

struct RenderedCacheEntry {
    key: RenderedCacheKey,
    rows: Arc<Vec<DiffRow>>,
    list_state: ListState,
    /// Starting row index of each file's content in `rows`. Sidebar
    /// click → `scroll_to_reveal_item(file_starts[path])`.
    file_starts: HashMap<String, usize>,
}

impl DifitApp {
    pub fn new(api: Arc<ApiClient>, window: &mut Window, cx: &mut App) -> Entity<Self> {
        let filter_input = cx.new(|cx| {
            TextInput::new(InputMode::SingleLine, "Filter files…", cx)
        });
        let view = cx.new(|cx| Self {
            api: api.clone(),
            diff: None,
            diff_generation: 0,
            selected: None,
            status: SharedString::from("Loading…"),
            view_mode: DiffViewMode::Unified,
            revisions: None,
            base_picker_open: false,
            quick_menu_open: false,
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
            show_settings_modal: false,
            settings: settings_store::snapshot(),
            settings_version: 0,
            expansions: HashMap::new(),
            expansion_version: 0,
            ui_version: 0,
            scrollbar_drag: None,
            sidebar_open: true,
            collapsed: HashSet::new(),
            collapsed_dirs: HashSet::new(),
            file_filter: filter_input.clone(),
            generated_cache: HashMap::new(),
            auto_collapse_done: HashSet::new(),
            blob_cache: HashMap::new(),
            pending_blob_fetches: HashSet::new(),
            preview_paths: HashSet::new(),
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
                        this.fetch_generated_statuses(cx);
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
        // The inline ComposeSlot row needs the cache rebuild to appear.
        self.bump_ui();
        window.focus(&body_handle, cx);
        cx.notify();
    }

    fn cancel_compose(&mut self, cx: &mut Context<Self>) {
        self.composing = None;
        self.bump_ui();
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
                        if this.composing.take().is_some() {
                            this.bump_ui();
                        }
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

    fn copy_all_prompts_for_file(&self, file_path: &str, cx: &mut Context<Self>) {
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
        self.open_in_editor_for(file_path, line, cx);
    }

    fn open_in_editor_for(&self, file_path: String, line: Option<u32>, cx: &mut Context<Self>) {
        let rx = self.api.open_in_editor(file_path, line);
        cx.spawn(async move |_this, _cx| {
            if let Ok(Err(e)) = rx.await {
                log::error!("open in editor failed: {e:#}");
            }
        })
        .detach();
    }

    fn start_compose_at_for(
        &mut self,
        file_path: String,
        anchor: CommentAnchor,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        // Move "selected" so the header / sidebar reflect the file the
        // user is commenting in, even if their last click was elsewhere.
        if let Some(diff) = self.diff.as_ref() {
            if let Some(idx) = diff.files.iter().position(|f| f.path == file_path) {
                self.selected = Some(idx);
            }
        }
        // Re-use the existing single-file compose path.
        let line_text = anchor.line.to_string();
        self.open_compose_with(
            ComposeMode::New,
            anchor.side,
            &line_text,
            "",
            "Write a comment…",
            window,
            cx,
        );
        if let Some(state) = &mut self.composing {
            state.file_path = file_path;
        }
    }

    fn toggle_viewed_for(&mut self, file_path: String, cx: &mut Context<Self>) {
        let Some(repo_id) = self
            .diff
            .as_ref()
            .and_then(|d| d.repository_id.clone())
        else {
            return;
        };
        let new_state = !self.viewed.is_viewed(&repo_id, &file_path);
        self.viewed.set_viewed(&repo_id, &file_path, new_state);
        if let Err(e) = self.viewed.save() {
            log::warn!("viewed save failed: {e:#}");
        }
        self.bump_ui();
        cx.notify();
    }

    fn toggle_dir_collapsed(&mut self, dir_path: String, cx: &mut Context<Self>) {
        if self.collapsed_dirs.contains(&dir_path) {
            self.collapsed_dirs.remove(&dir_path);
        } else {
            self.collapsed_dirs.insert(dir_path);
        }
        cx.notify();
    }

    fn toggle_collapsed_for(&mut self, file_path: String, cx: &mut Context<Self>) {
        if self.collapsed.contains(&file_path) {
            self.collapsed.remove(&file_path);
        } else {
            self.collapsed.insert(file_path);
        }
        self.bump_ui();
        cx.notify();
    }

    fn toggle_preview_for(&mut self, file_path: String, cx: &mut Context<Self>) {
        if !is_markdown_path(&file_path) {
            return;
        }
        if self.preview_paths.contains(&file_path) {
            self.preview_paths.remove(&file_path);
        } else {
            self.preview_paths.insert(file_path.clone());
            let ref_name = self.expansion_ref();
            self.ensure_blob(file_path, ref_name, cx);
        }
        self.bump_ui();
        cx.notify();
    }

    fn scroll_to_file(&mut self, file_path: String, cx: &mut Context<Self>) {
        if let Some(diff) = self.diff.as_ref() {
            if let Some(idx) = diff.files.iter().position(|f| f.path == file_path) {
                self.selected = Some(idx);
            }
        }
        if let Some(cache) = self.rendered_cache.as_ref() {
            if let Some(&row) = cache.file_starts.get(&file_path) {
                // `scroll_to_reveal_item` only ensures visibility, which
                // can land the row at the *bottom* of the viewport when
                // the user clicked from far away. We want the FileHeader
                // pinned to the top, so use `scroll_to` directly.
                cache.list_state.scroll_to(gpui::ListOffset {
                    item_ix: row,
                    offset_in_item: px(0.0),
                });
            }
        }
        cx.notify();
    }

    fn expansion_ref(&self) -> String {
        self.selected_target
            .clone()
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "HEAD".to_string())
    }

    fn expand_context(
        &mut self,
        file_path: String,
        chunk_idx: usize,
        direction: ExpandDirection,
        cx: &mut Context<Self>,
    ) {
        let ref_name = self.expansion_ref();
        let inc = expand_step();
        let entry = self.expansions.entry(file_path.clone()).or_default();
        let counts = entry.entry(chunk_idx).or_insert((0, 0));
        match direction {
            ExpandDirection::Above => counts.0 = counts.0.saturating_add(inc),
            ExpandDirection::Below => counts.1 = counts.1.saturating_add(inc),
        }
        self.expansion_version = self.expansion_version.wrapping_add(1);
        self.ensure_blob(file_path, ref_name, cx);
        cx.notify();
    }

    fn apply_settings(&mut self, settings: Settings, cx: &mut Context<Self>) {
        self.settings = settings.clone();
        settings_store::install(settings.clone());
        if let Err(e) = settings.save() {
            log::warn!("settings save failed: {e:#}");
        }
        self.settings_version = self.settings_version.wrapping_add(1);
        cx.notify();
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
        if let Some((file_path, anchor)) = self.selected_row_anchor() {
            self.start_compose_at_for(file_path, anchor, window, cx);
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
        let line = self.selected_row_anchor().map(|(_, a)| a.line);
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
        } else if self.show_settings_modal {
            self.show_settings_modal = false;
            cx.notify();
        } else if self.composing.is_some() {
            self.cancel_compose(cx);
        } else if self.base_picker_open || self.target_picker_open {
            self.base_picker_open = false;
            self.target_picker_open = false;
            cx.notify();
        }
    }

    fn selected_row_anchor(&self) -> Option<(String, CommentAnchor)> {
        let cache = self.rendered_cache.as_ref()?;
        let idx = self.selected_row?;
        let row = cache.rows.get(idx)?;
        match row {
            DiffRow::Unified { file_path, cell } => {
                cell.anchor.map(|a| (file_path.to_string(), a))
            }
            DiffRow::Split { file_path, left, right } => left
                .as_ref()
                .and_then(|c| c.anchor)
                .or_else(|| right.as_ref().and_then(|c| c.anchor))
                .map(|a| (file_path.to_string(), a)),
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
                DiffRow::Unified { cell, .. } if cell.anchor.is_some() => Some(i),
                DiffRow::Split { left, right, .. }
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
            if self.composing.take().is_some() {
                self.bump_ui();
            }
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
            if self.composing.take().is_some() {
                self.bump_ui();
            }
        }
        self.show_comments_list = false;

        if let Some(rendered) = self.ensure_rendered(cx) {
            for (i, row) in rendered.rows.iter().enumerate() {
                let matches = match row {
                    DiffRow::Unified { cell, .. } => cell
                        .anchor
                        .map(|a| a.side == side && a.line == anchor_line)
                        .unwrap_or(false),
                    DiffRow::Split { left, right, .. } => {
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

    /// Kick off blob fetches for every file whose special viewer wants
    /// the bytes: images need both old and new, notebooks always need
    /// new, markdown only when preview is enabled.
    fn kick_off_special_blob_fetches(&mut self, cx: &mut Context<Self>) {
        let Some(diff) = self.diff.clone() else { return };
        let new_ref = self.expansion_ref();
        let old_ref = self.selected_base.clone().filter(|s| !s.is_empty());
        for file in diff.files.iter() {
            let ext = file
                .path
                .rsplit_once('.')
                .map(|(_, e)| e.to_ascii_lowercase())
                .unwrap_or_default();
            if is_image_ext(&ext) {
                if let Some(r) = &old_ref {
                    self.ensure_blob(file.path.clone(), r.clone(), cx);
                }
                self.ensure_blob(file.path.clone(), new_ref.clone(), cx);
                continue;
            }
            if is_notebook_ext(&ext) {
                self.ensure_blob(file.path.clone(), new_ref.clone(), cx);
                continue;
            }
            if is_markdown_path(&file.path) && self.preview_paths.contains(&file.path) {
                self.ensure_blob(file.path.clone(), new_ref.clone(), cx);
            }
        }
    }

    fn ensure_blob(&mut self, path: String, git_ref: String, cx: &mut Context<Self>) {
        if git_ref.is_empty()
            || git_ref == "working"
            || git_ref == "staged"
            || git_ref == "."
        {
            return;
        }
        let key = (path.clone(), git_ref.clone());
        if self.blob_cache.contains_key(&key)
            || !self.pending_blob_fetches.insert(key.clone())
        {
            return;
        }
        let rx = self.api.fetch_blob(path, git_ref);
        cx.spawn(async move |this, cx| {
            match rx.await {
                Ok(Ok(bytes)) => {
                    let _ = this.update(cx, |this, cx| {
                        this.blob_cache.insert(key.clone(), Arc::new(bytes));
                        this.pending_blob_fetches.remove(&key);
                        // The expand renderer reads from blob_cache;
                        // bump the cache version so ensure_rendered
                        // rebuilds with the freshly-arrived bytes.
                        // Otherwise the first Expand click looks
                        // silent — the row is added but no context
                        // lines appear until the *next* click triggers
                        // a rebuild.
                        this.expansion_version =
                            this.expansion_version.wrapping_add(1);
                        cx.notify();
                    });
                }
                _ => {
                    let _ = this.update(cx, |this, _cx| {
                        this.pending_blob_fetches.remove(&key);
                    });
                }
            }
        })
        .detach();
    }

    fn toggle_preview_for_active(&mut self, cx: &mut Context<Self>) {
        let Some(path) = self.current_file_path() else { return };
        if !is_markdown_path(&path) {
            return;
        }
        if self.preview_paths.contains(&path) {
            self.preview_paths.remove(&path);
        } else {
            self.preview_paths.insert(path.clone());
            // Pull the new blob so the renderer has content to show.
            let ref_name = self.expansion_ref();
            self.ensure_blob(path, ref_name, cx);
        }
        cx.notify();
    }

    fn toggle_collapsed_at(&mut self, idx: usize, cx: &mut Context<Self>) {
        let Some(diff) = self.diff.clone() else { return };
        let Some(file) = diff.files.get(idx) else { return };
        if self.collapsed.contains(&file.path) {
            self.collapsed.remove(&file.path);
        } else {
            self.collapsed.insert(file.path.clone());
        }
        cx.notify();
    }

    fn fetch_generated_statuses(&mut self, cx: &mut Context<Self>) {
        let Some(diff) = self.diff.clone() else { return };
        let ref_name = self.expansion_ref();
        let repo_id = diff.repository_id.clone();
        for file in diff.files.iter() {
            let key = (file.path.clone(), ref_name.clone());
            if self.generated_cache.contains_key(&key) {
                continue;
            }
            // Heuristic short-circuit: if the server has already marked
            // the file as generated by path, treat it that way without
            // calling the network.
            if file.is_generated.unwrap_or(false) {
                self.generated_cache.insert(key.clone(), true);
                self.apply_generated(&file.path, true, repo_id.as_deref());
                continue;
            }
            let rx = self.api.fetch_generated_status(
                file.path.clone(),
                ref_name.clone(),
            );
            let key_clone = key.clone();
            let repo_id_clone = repo_id.clone();
            cx.spawn(async move |this, cx| {
                match rx.await {
                    Ok(Ok(resp)) => {
                        let _ = this.update(cx, |this, cx| {
                            this.generated_cache.insert(key_clone.clone(), resp.is_generated);
                            this.apply_generated(
                                &key_clone.0,
                                resp.is_generated,
                                repo_id_clone.as_deref(),
                            );
                            cx.notify();
                        });
                    }
                    Ok(Err(e)) => log::debug!("generated-status: {e:#}"),
                    Err(_) => {}
                }
            })
            .detach();
        }
    }

    fn apply_generated(&mut self, path: &str, is_generated: bool, repo_id: Option<&str>) {
        if !self.auto_collapse_done.insert(path.to_string()) {
            return;
        }
        if !is_generated {
            return;
        }
        self.collapsed.insert(path.to_string());
        if let Some(repo_id) = repo_id {
            if !self.viewed.is_viewed(repo_id, path) {
                self.viewed.set_viewed(repo_id, path, true);
                if let Err(e) = self.viewed.save() {
                    log::warn!("viewed save failed: {e:#}");
                }
            }
        }
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
                DiffAction::StartComposeAt { file_path, anchor } => {
                    this.start_compose_at_for(file_path, anchor, window, cx)
                }
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
                DiffAction::OpenInEditor { file_path, side: _, line } => {
                    this.open_in_editor_for(file_path, Some(line), cx)
                }
                DiffAction::ExpandContext {
                    file_path,
                    chunk_idx,
                    direction,
                } => this.expand_context(file_path, chunk_idx, direction, cx),
                DiffAction::ToggleViewed { file_path } => {
                    this.toggle_viewed_for(file_path, cx)
                }
                DiffAction::ToggleCollapsed { file_path } => {
                    this.toggle_collapsed_for(file_path, cx)
                }
                DiffAction::TogglePreview { file_path } => {
                    this.toggle_preview_for(file_path, cx)
                }
                DiffAction::SelectFile { file_path } => {
                    this.scroll_to_file(file_path, cx)
                }
                DiffAction::OpenFileInEditor { file_path } => {
                    this.open_in_editor_for(file_path, None, cx)
                }
                DiffAction::CopyAllPromptForFile { file_path } => {
                    this.copy_all_prompts_for_file(&file_path, cx);
                }
                DiffAction::ScrollbarDragStart {
                    mouse_y,
                    start_top_item,
                    scroll_range_items,
                    track_space_px,
                } => {
                    if track_space_px > 0.0 && scroll_range_items > 0.0 {
                        this.scrollbar_drag = Some(ScrollbarDragState {
                            start_mouse_y: mouse_y,
                            start_top_item,
                            items_per_drag_px: scroll_range_items / track_space_px,
                        });
                        if let Some(cache) = this.rendered_cache.as_ref() {
                            cache.list_state.scrollbar_drag_started();
                        }
                    }
                }
                DiffAction::ScrollbarDragMove { mouse_y } => {
                    let Some(drag) = this.scrollbar_drag else { return };
                    let Some(cache) = this.rendered_cache.as_ref() else { return };
                    let item_count = cache.rows.len();
                    if item_count == 0 {
                        return;
                    }
                    let target_item_f = drag.start_top_item
                        + (mouse_y - drag.start_mouse_y) * drag.items_per_drag_px;
                    let target = target_item_f
                        .round()
                        .clamp(0.0, item_count.saturating_sub(1) as f32)
                        as usize;
                    let current = cache.list_state.logical_scroll_top().item_ix;
                    if target != current {
                        cache.list_state.scroll_to(ListOffset {
                            item_ix: target,
                            offset_in_item: px(0.0),
                        });
                        cx.notify();
                    }
                }
                DiffAction::ScrollbarDragEnd => {
                    if this.scrollbar_drag.take().is_some() {
                        if let Some(cache) = this.rendered_cache.as_ref() {
                            cache.list_state.scrollbar_drag_ended();
                        }
                    }
                }
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

    /// Return the RenderedDiff for the whole repo, building (or refreshing)
    /// the cache if needed. This is the hot path's only heavy step, and
    /// only runs when the cache key changes.
    fn ensure_rendered(&mut self, cx: &mut Context<Self>) -> Option<RenderedDiff> {
        let diff = self.diff.clone()?;
        let key = RenderedCacheKey {
            view_mode: self.view_mode,
            diff_generation: self.diff_generation,
            comments_version: self.comments_version,
            settings_version: self.settings_version,
            expansion_version: self.expansion_version,
            ui_version: self.ui_version,
        };

        let needs_rebuild = self
            .rendered_cache
            .as_ref()
            .map(|c| c.key != key)
            .unwrap_or(true);

        if needs_rebuild {
            // Snapshot the prior scroll position so cache invalidations
            // (Expand, toggle collapsed/viewed, view-mode change, …)
            // don't yank the viewport back to the top.
            let prev_scroll = self
                .rendered_cache
                .as_ref()
                .map(|c| c.list_state.logical_scroll_top());

            let viewed = self.viewed_paths_for_current_repo();
            let compose_anchor =
                self.composing
                    .as_ref()
                    .map(|c| crate::ui::diff_rows::ComposeAnchor {
                        file_path: c.file_path.clone(),
                        anchor: CommentAnchor {
                            side: c.side,
                            line: c
                                .line_input
                                .read(cx)
                                .content()
                                .parse::<u32>()
                                .unwrap_or(0),
                        },
                    });
            let ctx = BuildContext {
                mode: self.view_mode,
                comments: self.comments.as_ref(),
                viewed: &viewed,
                collapsed: &self.collapsed,
                preview_paths: &self.preview_paths,
                expansions: &self.expansions,
                blob_bytes: &self.blob_cache,
                old_ref: self
                    .selected_base
                    .clone()
                    .filter(|s| !s.is_empty()),
                new_ref: Some(self.expansion_ref()),
                compose_anchor,
            };
            let (rows, file_starts) = build_all_rows(&diff, &ctx);
            let item_count = rows.len();
            // We deliberately do NOT call `measure_all` here — for big
            // diffs it's noticeably slow. The scrollbar instead estimates
            // total height from item count, which keeps the thumb at a
            // stable size as the user scrolls (the trade-off being that
            // the thumb position is a per-item approximation rather than
            // pixel-perfect).
            let list_state = ListState::new(item_count, ListAlignment::Top, px(400.0));
            if let Some(prev) = prev_scroll {
                let clamped_ix = prev.item_ix.min(item_count.saturating_sub(1));
                list_state.scroll_to(gpui::ListOffset {
                    item_ix: clamped_ix,
                    offset_in_item: prev.offset_in_item,
                });
            }
            self.rendered_cache = Some(RenderedCacheEntry {
                key,
                rows: Arc::new(rows),
                list_state,
                file_starts,
            });
        }

        self.rendered_cache.as_ref().map(|c| RenderedDiff {
            rows: c.rows.clone(),
            list_state: c.list_state.clone(),
        })
    }

    fn bump_ui(&mut self) {
        // Just bump the counter — `ensure_rendered` notices the cache
        // key mismatch and rebuilds while preserving scroll position.
        // Clearing the cache here would drop the ListState (and with it
        // the scroll offset) before `ensure_rendered` could read it.
        self.ui_version = self.ui_version.wrapping_add(1);
    }
}

fn is_markdown_path(path: &str) -> bool {
    let ext = path
        .rsplit_once('.')
        .map(|(_, e)| e.to_ascii_lowercase())
        .unwrap_or_default();
    matches!(ext.as_str(), "md" | "markdown")
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
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let can_compose = self.current_file_path().is_some();
        let composing_active = self.composing.is_some();
        let show_help = self.show_help;
        let show_revision_modal = self.show_revision_modal;
        let show_comments_list = self.show_comments_list;
        let show_settings_modal = self.show_settings_modal;
        let settings_snapshot = self.settings.clone();
        let font_size = self.settings.font_size;
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

        let rendered = self.ensure_rendered(cx);
        let diff = self.diff.clone();
        let selected = self.selected;
        let comments = self.comments.clone();

        let entity = cx.entity();
        let actions = self.build_actions(&entity);
        let viewed_paths: HashSet<String> = self.viewed_paths_for_current_repo();
        let collapsed_snapshot: HashSet<String> = self.collapsed.clone();
        let collapsed_dirs_snapshot: HashSet<String> = self.collapsed_dirs.clone();
        let filter_text: String = self.file_filter.read(cx).content().to_string();

        // Fire off blob fetches for every special-viewer file the build
        // surfaced (image/notebook/markdown-preview). pending_blob_fetches
        // dedupes inflight requests so this is cheap to re-run.
        self.kick_off_special_blob_fetches(cx);

        // Avoid cloning the entire file list every frame — borrow it for
        // sidebar rendering. The main pane consumes the all-files rows
        // through `rendered`.
        let files: &[crate::api::types::DiffFile] = diff
            .as_ref()
            .map(|d| d.files.as_slice())
            .unwrap_or(&[]);
        let active_file = selected.and_then(|i| files.get(i));
        let active_path = active_file.map(|f| f.path.clone());

        let file_count = diff.as_ref().map(|d| d.files.len()).unwrap_or(0);
        let reviewing_text = match (selected_base.as_deref(), selected_target.as_deref()) {
            (Some(b), Some(t)) => format!("Reviewing: {} ← {}", t, b),
            _ => String::new(),
        };
        let is_maximized = window.is_maximized();
        let root = div()
            .size_full()
            .flex()
            .flex_col()
            .key_context("DifitApp")
            .track_focus(&self.focus_handle)
            .child(crate::ui::titlebar::render_titlebar(is_maximized))
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
            .font_family(UI_FONT())
            .child(render_header(HeaderInputs {
                view_mode,
                ignore_whitespace,
                viewed_count: viewed_paths.len(),
                total_files: file_count,
                thread_count: comments.len(),
                commit_text: SharedString::from(
                    self.diff
                        .as_ref()
                        .map(|d| d.commit.clone())
                        .unwrap_or_default(),
                ),
                sidebar_open: self.sidebar_open,
                sidebar_width: 280.0,
                revisions: self.revisions.clone(),
                selected_base: self.selected_base.clone(),
                selected_target: self.selected_target.clone(),
                quick_menu_open: self.quick_menu_open,
                entity: entity.clone(),
            }))
            .child({
                let mut row = div().flex().flex_row().flex_1().min_h_0();
                if self.sidebar_open {
                    row = row.child(render_file_list(
                        files,
                        selected,
                        &viewed_paths,
                        &collapsed_snapshot,
                        &collapsed_dirs_snapshot,
                        Some(self.file_filter.clone()),
                        &filter_text,
                        {
                            let entity = entity.clone();
                            move |idx, cx| {
                                entity.update(cx, |this, cx| {
                                    let path = this
                                        .diff
                                        .as_ref()
                                        .and_then(|d| d.files.get(idx))
                                        .map(|f| f.path.clone());
                                    this.selected = Some(idx);
                                    this.selected_row = None;
                                    this.composing = None;
                                    if let Some(p) = path {
                                        this.scroll_to_file(p, cx);
                                    } else {
                                        cx.notify();
                                    }
                                });
                            }
                        },
                        {
                            let entity = entity.clone();
                            move |idx, cx| {
                                entity.update(cx, |this, cx| {
                                    if let Some(path) = this
                                        .diff
                                        .as_ref()
                                        .and_then(|d| d.files.get(idx))
                                        .map(|f| f.path.clone())
                                    {
                                        this.toggle_viewed_for(path, cx);
                                    }
                                });
                            }
                        },
                        {
                            let entity = entity.clone();
                            move |idx, cx| {
                                entity.update(cx, |this, cx| {
                                    if let Some(path) = this
                                        .diff
                                        .as_ref()
                                        .and_then(|d| d.files.get(idx))
                                        .map(|f| f.path.clone())
                                    {
                                        this.toggle_collapsed_for(path, cx);
                                    }
                                });
                            }
                        },
                        {
                            let entity = entity.clone();
                            move |dir_path, cx| {
                                entity.update(cx, |this, cx| {
                                    this.toggle_dir_collapsed(dir_path, cx)
                                });
                            }
                        },
                        {
                            let entity = entity.clone();
                            move |cx: &mut App| {
                                entity.update(cx, |this, cx| {
                                    this.show_help = !this.show_help;
                                    cx.notify();
                                });
                            }
                        },
                    ));
                }
                row.child(self.render_main_column(rendered, font_size, actions, &entity))
            });

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
            let revs_for_modal = self.revisions.clone();
            let base_now = self.selected_base.clone();
            let target_now = self.selected_target.clone();
            let base_open_now = self.base_picker_open;
            let target_open_now = self.target_picker_open;

            let mk = |f: fn(&mut DifitApp, &mut Context<DifitApp>)| {
                let entity = entity.clone();
                move |cx: &mut App| {
                    entity.update(cx, |this, cx| f(this, cx));
                }
            };
            let mk_pick =
                |role: RevisionRole| {
                    let entity = entity.clone();
                    move |value: String, cx: &mut App| {
                        entity.update(cx, |this, cx| this.pick_revision(role, value, cx));
                    }
                };
            let entity_close = entity.clone();
            let entity_apply = entity.clone();
            root.child(render_revision_modal(
                revs_for_modal.as_ref(),
                base_now.as_deref(),
                target_now.as_deref(),
                base_open_now,
                target_open_now,
                mk(|this, cx| {
                    this.base_picker_open = !this.base_picker_open;
                    this.target_picker_open = false;
                    cx.notify();
                }),
                mk_pick(RevisionRole::Base),
                mk(|this, cx| {
                    this.base_picker_open = false;
                    cx.notify();
                }),
                mk(|this, cx| {
                    this.target_picker_open = !this.target_picker_open;
                    this.base_picker_open = false;
                    cx.notify();
                }),
                mk_pick(RevisionRole::Target),
                mk(|this, cx| {
                    this.target_picker_open = false;
                    cx.notify();
                }),
                move |cx| {
                    entity_apply.update(cx, |this, cx| {
                        this.show_revision_modal = false;
                        this.refresh_diff(cx);
                    });
                },
                move |cx| {
                    entity_close.update(cx, |this, cx| {
                        this.show_revision_modal = false;
                        cx.notify();
                    });
                },
            ))
        } else {
            root
        };

        let root = if show_comments_list {
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
        };

        if show_settings_modal {
            let entity_apply = entity.clone();
            let entity_close = entity.clone();
            root.child(render_settings_modal(
                settings_snapshot,
                move |new_settings, cx| {
                    entity_apply.update(cx, |this, cx| this.apply_settings(new_settings, cx));
                },
                move |cx| {
                    entity_close.update(cx, |this, cx| {
                        this.show_settings_modal = false;
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
    /// Wraps the all-files virtualized list. The compose form is now
    /// inlined as a `ComposeSlot` row underneath the diff line being
    /// commented on, matching React's CommentForm placement.
    fn render_main_column(
        &self,
        rendered: Option<RenderedDiff>,
        font_size: f32,
        actions: DiffActions,
        entity: &Entity<DifitApp>,
    ) -> impl IntoElement {
        let compose_renderer: Option<crate::ui::diff_view::ComposeRenderer> =
            self.composing.as_ref().map(|state| {
                let file_path = SharedString::from(state.file_path.clone());
                let side = state.side;
                let line_input = state.line_input.clone();
                let body_input = state.body_input.clone();
                let entity_s = entity.clone();
                let entity_c = entity.clone();
                let entity_t = entity.clone();
                std::sync::Arc::new(move || {
                    let toggle = {
                        let entity_t = entity_t.clone();
                        move |side: DiffSide, cx: &mut App| {
                            entity_t
                                .update(cx, |this, cx| this.toggle_compose_side(side, cx));
                        }
                    };
                    let submit = {
                        let entity_s = entity_s.clone();
                        move |cx: &mut App| {
                            entity_s.update(cx, |this, cx| this.submit_compose(cx));
                        }
                    };
                    let cancel = {
                        let entity_c = entity_c.clone();
                        move |cx: &mut App| {
                            entity_c.update(cx, |this, cx| this.cancel_compose(cx));
                        }
                    };
                    render_compose_bar(
                        file_path.clone(),
                        side,
                        line_input.clone(),
                        body_input.clone(),
                        toggle,
                        submit,
                        cancel,
                    )
                    .into_any_element()
                }) as crate::ui::diff_view::ComposeRenderer
            });

        div()
            .flex_1()
            .min_h_0()
            .min_w_0()
            .flex()
            .flex_col()
            .child(render_main_pane(
                rendered,
                font_size,
                actions,
                compose_renderer,
            ))
    }
}

struct HeaderInputs {
    view_mode: DiffViewMode,
    ignore_whitespace: bool,
    viewed_count: usize,
    total_files: usize,
    thread_count: usize,
    commit_text: SharedString,
    sidebar_open: bool,
    sidebar_width: f32,
    revisions: Option<Arc<RevisionsResponse>>,
    selected_base: Option<String>,
    selected_target: Option<String>,
    quick_menu_open: bool,
    entity: Entity<DifitApp>,
}

fn render_header(inputs: HeaderInputs) -> impl IntoElement {
    let HeaderInputs {
        view_mode,
        ignore_whitespace,
        viewed_count,
        total_files,
        thread_count,
        commit_text,
        sidebar_open,
        sidebar_width,
        revisions,
        selected_base,
        selected_target,
        quick_menu_open,
        entity,
    } = inputs;

    let entity_panel = entity.clone();
    let entity_settings = entity.clone();
    let entity_help = entity.clone();
    let entity_view = entity.clone();
    let entity_ws = entity.clone();
    let entity_refresh = entity.clone();
    let entity_threads = entity.clone();
    let entity_review = entity.clone();

    let panel_icon = if sidebar_open {
        "panel-left"
    } else {
        "panel-left"
    };

    // Left section — sidebar-width column with logo + panel-left + settings.
    let left_section = div()
        .px(px(16.0))
        .py(px(10.0))
        .flex()
        .flex_row()
        .items_center()
        .justify_between()
        .gap(px(16.0))
        .w(px(sidebar_width))
        .flex_shrink_0()
        .child(logo())
        .child(
            div()
                .flex()
                .flex_row()
                .items_center()
                .gap(px(2.0))
                .child(icon_button(
                    "panel-left-toggle",
                    panel_icon,
                    if sidebar_open {
                        "Collapse file tree"
                    } else {
                        "Expand file tree"
                    },
                    move |cx: &mut App| {
                        entity_panel.update(cx, |this, cx| {
                            this.sidebar_open = !this.sidebar_open;
                            cx.notify();
                        });
                    },
                ))
                .child(icon_button(
                    "settings-btn",
                    "settings",
                    "Settings",
                    move |cx: &mut App| {
                        entity_settings.update(cx, |this, cx| {
                            this.show_settings_modal = !this.show_settings_modal;
                            cx.notify();
                        });
                    },
                )),
            // Help button removed — Shortcuts now lives in the sidebar
            // footer, matching React's App.tsx layout.
        );
        let _ = entity_help;

    // Vertical divider matching React's 4px / inset-8px bar.
    let divider = div()
        .w(px(1.0))
        .my(px(8.0))
        .bg(Theme::BORDER);

    // Right section — split into left cluster (view mode / WS / reload)
    // and right cluster (threads / viewed progress / reviewing label).
    let view_is_split = view_mode == DiffViewMode::Split;
    let entity_view_pill = entity_view.clone();
    let left_cluster = div()
        .flex()
        .flex_row()
        .flex_wrap()
        .items_center()
        .gap(px(12.0))
        .child(pill_toggle(
            "view-mode-pill",
            "columns",
            "Split",
            "align-left",
            "Unified",
            !view_is_split,
            move |cx: &mut App| {
                entity_view_pill.update(cx, |this, cx| {
                    this.view_mode = this.view_mode.toggle();
                    cx.notify();
                });
            },
        ))
        .child(checkbox(
            "ws-checkbox",
            ignore_whitespace,
            "Ignore Whitespace",
            move |cx: &mut App| {
                entity_ws.update(cx, |this, cx| this.toggle_ignore_whitespace(cx));
            },
        ))
        .child(icon_button(
            "refresh-btn",
            "refresh-cw",
            "Refresh diff",
            move |cx: &mut App| {
                entity_refresh.update(cx, |this, cx| this.refresh_diff(cx));
            },
        ));

    let right_cluster_threads: gpui::AnyElement = if thread_count > 0 {
        icon_button(
            "threads-btn",
            "message-square",
            "All comments",
            move |cx: &mut App| {
                entity_threads.update(cx, |this, cx| {
                    this.show_comments_list = !this.show_comments_list;
                    cx.notify();
                });
            },
        )
        .into_any_element()
    } else {
        div().into_any_element()
    };

    let right_cluster = div()
        .flex()
        .flex_row()
        .flex_wrap()
        .items_center()
        .gap(px(16.0))
        .child(right_cluster_threads)
        .child(viewed_progress(viewed_count, total_files))
        .child({
            let entity_toggle = entity_review.clone();
            let entity_apply = entity_review.clone();
            let entity_detailed = entity_review.clone();
            let entity_dismiss = entity_review.clone();
            render_quick_menu(
                commit_text,
                revisions.as_ref(),
                selected_base.as_deref(),
                selected_target.as_deref(),
                quick_menu_open,
                move |cx: &mut App| {
                    entity_toggle.update(cx, |this, cx| {
                        this.quick_menu_open = !this.quick_menu_open;
                        cx.notify();
                    });
                },
                move |preset: QuickPreset, cx: &mut App| {
                    entity_apply.update(cx, |this, cx| {
                        this.selected_base = Some(preset.base);
                        this.selected_target = Some(preset.target);
                        this.quick_menu_open = false;
                        this.refresh_diff(cx);
                    });
                },
                move |cx: &mut App| {
                    entity_detailed.update(cx, |this, cx| {
                        this.quick_menu_open = false;
                        this.show_revision_modal = true;
                        cx.notify();
                    });
                },
                move |cx: &mut App| {
                    entity_dismiss.update(cx, |this, cx| {
                        this.quick_menu_open = false;
                        cx.notify();
                    });
                },
            )
        });

    // The two clusters lay out on a single row when the window is wide
    // and wrap to a second row when it's narrow (React uses
    // `flex-wrap`).
    let right_section = div()
        .flex_1()
        .min_w_0()
        .px(px(16.0))
        .py(px(10.0))
        .flex()
        .flex_row()
        .flex_wrap()
        .items_center()
        .justify_between()
        .gap(px(16.0))
        .child(left_cluster)
        .child(right_cluster);

    div()
        .w_full()
        .flex()
        .flex_row()
        .items_center()
        .bg(Theme::BG_ELEVATED)
        .border_b_1()
        .border_color(Theme::BORDER)
        .child(left_section)
        .child(divider)
        .child(right_section)
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
        .tooltip(label_tooltip(label))
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
        .tooltip(label_tooltip(label))
        .child(SharedString::from(label))
}
