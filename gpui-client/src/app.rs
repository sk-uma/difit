use std::sync::Arc;

use gpui::{
    div, prelude::*, px, App, Context, Entity, IntoElement, ListAlignment, ListState,
    ParentElement, SharedString, Styled, Window,
};

use crate::api::client::{CommentSelectionQuery, DiffQuery, WatchEvent};
use crate::api::types::{DiffCommentThread, DiffResponse, RevisionsResponse};
use crate::api::ApiClient;
use crate::ui::diff_rows::{build_rows, DiffRow};
use crate::ui::diff_view::{count_threads_for_file, render_diff, DiffViewMode, RenderedDiff};
use crate::ui::file_list::render_file_list;
use crate::ui::revision_picker::{render_revision_picker, RevisionRole};
use crate::ui::theme::{Theme, UI_FONT};

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
    pub fn new(api: Arc<ApiClient>, _window: &mut Window, cx: &mut App) -> Entity<Self> {
        let view = cx.new(|_cx| Self {
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
        });

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
        let query = DiffQuery::from_selection(
            self.selected_base.as_deref(),
            self.selected_target.as_deref(),
        );
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
                        this.diff = Some(Arc::new(diff));
                        this.diff_generation = this.diff_generation.wrapping_add(1);
                        this.rendered_cache = None;
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

impl Render for DifitApp {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // Snapshot lightweight state up-front; heavy data stays behind Arcs.
        let view_mode = self.view_mode;
        let revisions = self.revisions.clone();
        let selected_base = self.selected_base.clone();
        let selected_target = self.selected_target.clone();
        let base_open = self.base_picker_open;
        let target_open = self.target_picker_open;
        let status = self.status.clone();

        let rendered = self.ensure_rendered();
        let diff = self.diff.clone();
        let selected = self.selected;
        let comments = self.comments.clone();

        let entity = cx.entity();

        // Avoid cloning the entire file list every frame — borrow it for
        // file_list rendering and active_file lookup via the same Arc.
        let files: &[crate::api::types::DiffFile] = diff
            .as_ref()
            .map(|d| d.files.as_slice())
            .unwrap_or(&[]);
        let active_file = selected.and_then(|i| files.get(i));
        let thread_count = active_file
            .map(|f| count_threads_for_file(&f.path, comments.iter()))
            .unwrap_or(0);

        div()
            .size_full()
            .flex()
            .flex_col()
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
                entity: entity.clone(),
            }))
            .child(
                div()
                    .flex()
                    .flex_row()
                    .flex_1()
                    .min_h_0()
                    .child(render_file_list(files, selected, move |idx, cx| {
                        entity.update(cx, |this, cx| {
                            this.selected = Some(idx);
                            this.rendered_cache = None;
                            cx.notify();
                        });
                    }))
                    .child(render_diff(active_file, rendered, thread_count)),
            )
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
        entity,
    } = inputs;

    let entity_a = entity.clone();
    let entity_b = entity.clone();
    let entity_c = entity.clone();
    let entity_d = entity.clone();
    let entity_e = entity.clone();
    let entity_f = entity.clone();
    let entity_g = entity.clone();

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
