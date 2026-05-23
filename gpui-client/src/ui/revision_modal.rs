use std::sync::Arc;

use gpui::{div, prelude::*, px, App, IntoElement, ParentElement, SharedString, Styled};

use crate::api::types::{DiffResponse, RevisionsResponse};
use crate::ui::theme::{Theme, UI_FONT};

pub fn render_revision_modal(
    diff: Option<&DiffResponse>,
    revisions: Option<&Arc<RevisionsResponse>>,
    ignore_whitespace: bool,
    use_merge_base: bool,
    on_close: impl Fn(&mut App) + 'static,
) -> impl IntoElement {
    let rows = build_rows(diff, revisions, ignore_whitespace, use_merge_base);

    div()
        .id("revision-modal")
        .absolute()
        .inset_0()
        .flex()
        .items_center()
        .justify_center()
        .bg(gpui::hsla(0.0, 0.0, 0.0, 0.5))
        .on_mouse_down(
            gpui::MouseButton::Left,
            move |_e, _w, cx| on_close(cx),
        )
        .child(
            div()
                .w(px(540.0))
                .max_h(px(540.0))
                .p_4()
                .bg(Theme::BG_ELEVATED)
                .border_1()
                .border_color(Theme::BORDER)
                .rounded_md()
                .shadow_lg()
                .font_family(UI_FONT())
                .text_size(px(12.5))
                .text_color(Theme::TEXT)
                .flex()
                .flex_col()
                .gap_2()
                .child(
                    div()
                        .text_size(px(14.0))
                        .text_color(Theme::TEXT)
                        .child(SharedString::from("Revision details")),
                )
                .children(rows.into_iter().map(|(k, v)| {
                    div()
                        .flex()
                        .flex_row()
                        .gap_3()
                        .child(
                            div()
                                .w(px(160.0))
                                .text_color(Theme::TEXT_MUTED)
                                .child(SharedString::from(k)),
                        )
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .text_color(Theme::TEXT)
                                .child(SharedString::from(v)),
                        )
                }))
                .child(
                    div()
                        .mt_2()
                        .text_color(Theme::TEXT_MUTED)
                        .text_size(px(11.0))
                        .child(SharedString::from(
                            "Click outside to close.",
                        )),
                ),
        )
}

fn build_rows(
    diff: Option<&DiffResponse>,
    revisions: Option<&Arc<RevisionsResponse>>,
    ignore_whitespace: bool,
    use_merge_base: bool,
) -> Vec<(&'static str, String)> {
    let mut rows: Vec<(&'static str, String)> = Vec::new();
    if let Some(d) = diff {
        rows.push(("Commit", d.commit.clone()));
        rows.push((
            "Base",
            d.base_commitish.clone().unwrap_or_else(|| "—".to_string()),
        ));
        rows.push((
            "Target",
            d.target_commitish
                .clone()
                .unwrap_or_else(|| "—".to_string()),
        ));
        if let Some(rb) = d.requested_base_commitish.as_ref() {
            rows.push(("Requested base", rb.clone()));
        }
        if let Some(rt) = d.requested_target_commitish.as_ref() {
            rows.push(("Requested target", rt.clone()));
        }
        rows.push(("Files changed", d.files.len().to_string()));
        if let Some(repo_id) = d.repository_id.as_ref() {
            rows.push(("Repository id", short(repo_id)));
        }
    } else {
        rows.push(("Diff", "Loading…".to_string()));
    }
    rows.push((
        "Ignore whitespace",
        if ignore_whitespace { "on" } else { "off" }.to_string(),
    ));
    rows.push((
        "Merge-base mode",
        if use_merge_base { "on" } else { "off" }.to_string(),
    ));
    if let Some(r) = revisions {
        rows.push((
            "Origin default branch",
            r.origin_default_branch
                .clone()
                .unwrap_or_else(|| "—".to_string()),
        ));
        rows.push((
            "Resolved base",
            r.resolved_base.clone().unwrap_or_else(|| "—".to_string()),
        ));
        rows.push((
            "Resolved target",
            r.resolved_target
                .clone()
                .unwrap_or_else(|| "—".to_string()),
        ));
    }
    rows
}

fn short(s: &str) -> String {
    if s.len() > 16 {
        format!("{}…", &s[..16])
    } else {
        s.to_string()
    }
}
