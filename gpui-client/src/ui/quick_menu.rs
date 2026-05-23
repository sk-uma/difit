//! Compact "Reviewing: <commit>" dropdown — React's `DiffQuickMenu`.
//!
//! The trigger is the small `Reviewing: <code>{commit}</code>` chip
//! that lives at the end of the header. Clicking it pops up a menu
//! with a few preset diff selections (HEAD, HEAD…Uncommitted, etc.),
//! plus a "Detailed…" entry that opens the full RevisionDetailModal.

use std::sync::Arc;

use gpui::{
    anchored, deferred, div, prelude::*, px, Anchor, App, ElementId, IntoElement, ParentElement,
    SharedString, Styled,
};

use crate::api::types::RevisionsResponse;
use crate::ui::theme::{Theme, MONO_FONT};

/// (base, target) selection a Quick Diffs preset will apply.
#[derive(Clone, Debug)]
pub struct Preset {
    pub label: String,
    pub base: String,
    pub target: String,
}

#[allow(clippy::too_many_arguments)]
pub fn render_quick_menu(
    commit_text: SharedString,
    revisions: Option<&Arc<RevisionsResponse>>,
    current_base: Option<&str>,
    current_target: Option<&str>,
    is_open: bool,
    on_toggle: impl Fn(&mut App) + 'static,
    on_apply: impl Fn(Preset, &mut App) + 'static + Clone,
    on_open_detailed: impl Fn(&mut App) + 'static,
    on_dismiss: impl Fn(&mut App) + 'static,
) -> impl IntoElement {
    let trigger = div()
        .id(ElementId::Name(SharedString::from("quick-menu-trigger")))
        .flex()
        .flex_row()
        .items_center()
        .gap(px(6.0))
        .text_size(px(11.0))
        .text_color(Theme::TEXT_MUTED)
        .cursor_pointer()
        .hover(|s| s.text_color(Theme::TEXT))
        .on_click(move |_e, _w, cx| on_toggle(cx))
        .child(SharedString::from("Reviewing:"))
        .child(
            div()
                .px(px(6.0))
                .py(px(1.0))
                .bg(Theme::BG_HOVER)
                .border_1()
                .border_color(Theme::BORDER)
                .rounded(px(3.0))
                .font_family(MONO_FONT())
                .text_color(Theme::TEXT)
                .child(commit_text),
        )
        .child(SharedString::from("\u{25BE}"));

    if !is_open {
        return trigger.into_any_element();
    }

    let presets = build_presets(revisions, current_target);
    let on_apply = on_apply.clone();

    trigger
        .child(
            deferred(
                anchored()
                    .anchor(Anchor::TopRight)
                    .snap_to_window_with_margin(px(8.0))
                    .child(
                        panel()
                            .on_mouse_down_out(move |_e, _w, cx| on_dismiss(cx))
                            .child(section_header("Quick Diffs"))
                            .children(presets.iter().cloned().map(|p| {
                                let active = is_active(&p, current_base, current_target);
                                let cb = on_apply.clone();
                                preset_item(p, active, move |preset, cx| cb(preset, cx))
                            }))
                            .child(divider())
                            .child(
                                div()
                                    .id(ElementId::Name(SharedString::from("qm-detailed")))
                                    .px(px(12.0))
                                    .py(px(8.0))
                                    .text_color(Theme::TEXT)
                                    .text_size(px(12.0))
                                    .cursor_pointer()
                                    .hover(|s| s.bg(Theme::BG_HOVER))
                                    .on_click(move |_e, _w, cx| on_open_detailed(cx))
                                    .child(SharedString::from("Detailed…")),
                            ),
                    ),
            )
            .priority(1),
        )
        .into_any_element()
}

fn panel() -> gpui::Div {
    div()
        .mt_1()
        .w(px(260.0))
        .max_h(px(360.0))
        .bg(Theme::BG_ELEVATED)
        .border_1()
        .border_color(Theme::BORDER)
        .rounded(px(6.0))
        .shadow_lg()
        .overflow_hidden()
        .text_color(Theme::TEXT)
        .text_size(px(12.0))
        .flex()
        .flex_col()
}

fn section_header(label: &'static str) -> impl IntoElement {
    div()
        .px(px(12.0))
        .py(px(6.0))
        .bg(Theme::BG_HOVER)
        .border_b_1()
        .border_color(Theme::BORDER)
        .text_color(Theme::TEXT_MUTED)
        .text_size(px(11.0))
        .font_weight(gpui::FontWeight::SEMIBOLD)
        .child(SharedString::from(label))
}

fn divider() -> impl IntoElement {
    div().h(px(1.0)).w_full().bg(Theme::BORDER)
}

fn preset_item(
    preset: Preset,
    active: bool,
    on_click: impl Fn(Preset, &mut App) + 'static,
) -> impl IntoElement {
    let id = ElementId::Name(SharedString::from(format!(
        "qm-{}-{}",
        preset.base, preset.target
    )));
    let label = preset.label.clone();
    let preset_for_click = preset.clone();
    div()
        .id(id)
        .px(px(12.0))
        .py(px(8.0))
        .text_color(if active {
            Theme::TEXT
        } else {
            Theme::TEXT_MUTED
        })
        .font_weight(if active {
            gpui::FontWeight::SEMIBOLD
        } else {
            gpui::FontWeight::NORMAL
        })
        .bg(if active {
            Theme::BG_HOVER
        } else {
            Theme::BG_ELEVATED
        })
        .border_l(px(if active { 3.0 } else { 0.0 }))
        .border_color(Theme::TEXT_LINK)
        .cursor_pointer()
        .hover(|s| s.bg(Theme::BG_HOVER).text_color(Theme::TEXT))
        .on_click(move |_e, _w, cx| on_click(preset_for_click.clone(), cx))
        .child(SharedString::from(label))
}

fn is_active(preset: &Preset, current_base: Option<&str>, current_target: Option<&str>) -> bool {
    current_base == Some(preset.base.as_str())
        && current_target == Some(preset.target.as_str())
}

fn build_presets(
    revisions: Option<&Arc<RevisionsResponse>>,
    current_target: Option<&str>,
) -> Vec<Preset> {
    let mut out = vec![
        Preset {
            label: "HEAD".into(),
            base: "HEAD^".into(),
            target: "HEAD".into(),
        },
        Preset {
            label: "HEAD…Uncommitted (merge-base)".into(),
            base: "HEAD".into(),
            target: ".".into(),
        },
    ];
    if let Some(revs) = revisions {
        if let Some(b) = revs.branches.iter().find(|b| b.current) {
            out.push(Preset {
                label: format!("{}…Uncommitted (merge-base)", b.name),
                base: b.name.clone(),
                target: ".".into(),
            });
        }
        if let Some(o) = revs.origin_default_branch.as_ref() {
            out.push(Preset {
                label: format!("{}…Uncommitted (merge-base)", o),
                base: o.clone(),
                target: ".".into(),
            });
        }
    }
    if let Some(t) = current_target {
        out.push(Preset {
            label: "Previous commit".into(),
            base: format!("{}^^", t.trim_end_matches('^')),
            target: format!("{}^", t.trim_end_matches('^')),
        });
    }
    out
}
