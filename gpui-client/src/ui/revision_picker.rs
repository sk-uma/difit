use std::sync::Arc;

use gpui::{
    anchored, deferred, div, prelude::*, px, Anchor, App, ElementId, IntoElement, ParentElement,
    SharedString, Styled,
};

use crate::api::types::RevisionsResponse;
use crate::ui::theme::Theme;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RevisionRole {
    Base,
    Target,
}

impl RevisionRole {
    fn label(self) -> &'static str {
        match self {
            RevisionRole::Base => "Base",
            RevisionRole::Target => "Target",
        }
    }

    fn id_prefix(self) -> &'static str {
        match self {
            RevisionRole::Base => "base-picker",
            RevisionRole::Target => "target-picker",
        }
    }
}

pub fn render_revision_picker(
    role: RevisionRole,
    current: Option<&str>,
    revisions: Option<&Arc<RevisionsResponse>>,
    is_open: bool,
    on_toggle: impl Fn(&mut App) + 'static,
    on_select: impl Fn(String, &mut App) + 'static + Clone,
    on_dismiss: impl Fn(&mut App) + 'static,
) -> impl IntoElement {
    let display_value = current.unwrap_or("—");
    let display = format!("{}: {} \u{25BE}", role.label(), display_value);

    let button = div()
        .id(ElementId::Name(SharedString::from(role.id_prefix())))
        .px_3()
        .py_1()
        .rounded_sm()
        .border_1()
        .border_color(Theme::BORDER)
        .text_size(px(12.0))
        .text_color(Theme::TEXT)
        .cursor_pointer()
        .hover(|s| s.bg(Theme::BG_HOVER))
        .on_click(move |_event, _window, cx| on_toggle(cx))
        .child(SharedString::from(display));

    if !is_open {
        return button.into_any_element();
    }

    let Some(revisions) = revisions else {
        return button
            .child(deferred(
                anchored().anchor(Anchor::TopLeft).child(
                    panel().child(
                        div()
                            .px_3()
                            .py_2()
                            .text_color(Theme::TEXT_MUTED)
                            .child(SharedString::from("Loading revisions…")),
                    ),
                ),
            ))
            .into_any_element();
    };

    let items = collect_items(revisions);
    let on_select = on_select.clone();
    button
        .child(
            deferred(
                anchored()
                    .anchor(Anchor::TopLeft)
                    .snap_to_window_with_margin(px(8.0))
                    .child(
                        panel()
                            .on_mouse_down_out(move |_event, _window, cx| on_dismiss(cx))
                            .child(picker_list(role, items, on_select)),
                    ),
            )
            .priority(1),
        )
        .into_any_element()
}

fn panel() -> gpui::Div {
    div()
        .mt_1()
        .w(px(320.0))
        .max_h(px(420.0))
        .bg(Theme::BG_ELEVATED)
        .border_1()
        .border_color(Theme::BORDER)
        .rounded_md()
        .shadow_lg()
        .text_color(Theme::TEXT)
        .text_size(px(12.0))
        .flex()
        .flex_col()
}

struct Item {
    value: String,
    label: String,
    hint: Option<String>,
    section: &'static str,
}

fn collect_items(revisions: &RevisionsResponse) -> Vec<Item> {
    let mut items = Vec::new();
    for opt in &revisions.special_options {
        items.push(Item {
            value: opt.value.clone(),
            label: opt.label.clone(),
            hint: Some(opt.value.clone()),
            section: "Special",
        });
    }
    for b in &revisions.branches {
        items.push(Item {
            value: b.name.clone(),
            label: if b.current {
                format!("* {}", b.name)
            } else {
                b.name.clone()
            },
            hint: None,
            section: "Branches",
        });
    }
    for c in &revisions.commits {
        items.push(Item {
            value: c.hash.clone(),
            label: format!("{} {}", c.short_hash, truncate(&c.message, 64)),
            hint: None,
            section: "Commits",
        });
    }
    items
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() > max {
        let mut out: String = s.chars().take(max).collect();
        out.push('…');
        out
    } else {
        s.to_string()
    }
}

fn picker_list(
    role: RevisionRole,
    items: Vec<Item>,
    on_select: impl Fn(String, &mut App) + 'static + Clone,
) -> impl IntoElement {
    let mut last_section: Option<&'static str> = None;
    let mut children: Vec<gpui::AnyElement> = Vec::with_capacity(items.len());

    for (idx, item) in items.into_iter().enumerate() {
        if last_section != Some(item.section) {
            children.push(section_header(item.section).into_any_element());
            last_section = Some(item.section);
        }
        let value = item.value.clone();
        let cb = on_select.clone();
        let row_id =
            ElementId::Name(SharedString::from(format!("{}-item-{}", role.id_prefix(), idx)));
        let row = div()
            .id(row_id)
            .px_3()
            .py_1()
            .flex()
            .flex_row()
            .gap_2()
            .cursor_pointer()
            .hover(|s| s.bg(Theme::BG_HOVER))
            .on_click(move |_event, _window, cx| cb(value.clone(), cx))
            .child(
                div()
                    .flex_1()
                    .child(SharedString::from(item.label.clone())),
            );
        let row = if let Some(hint) = item.hint {
            row.child(
                div()
                    .text_color(Theme::TEXT_MUTED)
                    .child(SharedString::from(hint)),
            )
        } else {
            row
        };
        children.push(row.into_any_element());
    }

    div()
        .id("revision-list")
        .flex()
        .flex_col()
        .overflow_y_scroll()
        .children(children)
}

fn section_header(name: &'static str) -> impl IntoElement {
    div()
        .px_3()
        .py_1()
        .text_color(Theme::TEXT_MUTED)
        .text_size(px(11.0))
        .border_b_1()
        .border_color(Theme::BORDER)
        .child(SharedString::from(name))
}
