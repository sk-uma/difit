use std::sync::Arc;

use gpui::{
    div, prelude::*, px, App, Context, Entity, IntoElement, ParentElement, SharedString, Styled,
    Window,
};

use crate::api::client::DiffQuery;
use crate::api::types::DiffResponse;
use crate::api::ApiClient;
use crate::ui::diff_view::render_diff;
use crate::ui::file_list::render_file_list;
use crate::ui::theme::{Theme, UI_FONT};

pub struct DifitApp {
    api: Arc<ApiClient>,
    diff: Option<DiffResponse>,
    selected: Option<usize>,
    status: SharedString,
}

impl DifitApp {
    pub fn new(api: Arc<ApiClient>, _window: &mut Window, cx: &mut App) -> Entity<Self> {
        let view = cx.new(|_cx| Self {
            api: api.clone(),
            diff: None,
            selected: None,
            status: SharedString::from("Loading…"),
        });

        // Kick off the initial diff fetch immediately after construction.
        view.update(cx, |this, cx| this.refresh_diff(cx));
        view
    }

    fn refresh_diff(&mut self, cx: &mut Context<Self>) {
        self.status = SharedString::from("Fetching diff…");
        let rx = self.api.fetch_diff(DiffQuery::default());
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
                        this.diff = Some(diff);
                        if this.selected.is_none()
                            && this
                                .diff
                                .as_ref()
                                .map(|d| !d.files.is_empty())
                                .unwrap_or(false)
                        {
                            this.selected = Some(0);
                        }
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
        let files = self
            .diff
            .as_ref()
            .map(|d| d.files.clone())
            .unwrap_or_default();
        let selected = self.selected;
        let active_file = selected.and_then(|i| files.get(i).cloned());

        let entity = cx.entity();
        let entity_for_refresh = entity.clone();

        div()
            .size_full()
            .flex()
            .flex_col()
            .bg(Theme::BG)
            .text_color(Theme::TEXT)
            .font_family(UI_FONT)
            .child(header(
                self.status.clone(),
                {
                    let entity = entity_for_refresh.clone();
                    move |cx: &mut App| {
                        entity.update(cx, |this, cx| this.refresh_diff(cx));
                    }
                },
            ))
            .child(
                div()
                    .flex()
                    .flex_row()
                    .flex_1()
                    .child(render_file_list(&files, selected, move |idx, cx| {
                        entity.update(cx, |this, cx| {
                            this.selected = Some(idx);
                            cx.notify();
                        });
                    }))
                    .child(render_diff(active_file.as_ref())),
            )
    }
}

fn header(
    status: SharedString,
    on_refresh: impl Fn(&mut App) + 'static,
) -> impl IntoElement {
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
        .child(
            div()
                .flex_1()
                .text_size(px(12.0))
                .text_color(Theme::TEXT_MUTED)
                .child(status),
        )
        .child(
            div()
                .id("refresh")
                .px_3()
                .py_1()
                .rounded_sm()
                .border_1()
                .border_color(Theme::BORDER)
                .text_size(px(12.0))
                .cursor_pointer()
                .hover(|s| s.bg(Theme::BG_HOVER))
                .on_click(move |_event, _window, cx| on_refresh(cx))
                .child(SharedString::from("Refresh")),
        )
}
