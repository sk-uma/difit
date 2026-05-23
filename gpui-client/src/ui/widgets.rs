//! Small reusable UI primitives shared across the app: toggle switches,
//! pill buttons, and the difit logo glyph.

use gpui::{
    div, prelude::*, px, svg, AnyView, App, Context, ElementId, IntoElement, ParentElement,
    Rgba, SharedString, Styled, Window,
};

use crate::ui::theme::Theme;

/// Render an embedded SVG icon. `name` is the filename without
/// extension (e.g. `"chevron-right"`). Color follows `currentColor`
/// via `.text_color`. The icon is locked to its declared size — without
/// `flex_shrink_0` GPUI's flex layout will sometimes collapse the SVG to
/// 0x0 inside crowded rows and the renderer logs "can't render at a
/// zero size" on every frame.
pub fn icon(name: &str, size_px: f32, color: Rgba) -> impl IntoElement {
    svg()
        .path(SharedString::from(format!("icons/{name}.svg")))
        .w(px(size_px))
        .h(px(size_px))
        .flex_shrink_0()
        .text_color(color)
}

/// Icon + label inline, used when replacing text-only header buttons.
pub fn icon_label(name: &str, label: &'static str) -> impl IntoElement {
    div()
        .flex()
        .flex_row()
        .items_center()
        .gap_1()
        .child(icon(name, 14.0, Theme::TEXT_MUTED))
        .child(SharedString::from(label))
}

/// Icon-only header button (18px icon, 8px padding, hover bg).
pub fn icon_button(
    id: impl Into<SharedString>,
    icon_name: &'static str,
    tooltip: &'static str,
    on_click: impl Fn(&mut App) + 'static,
) -> impl IntoElement {
    div()
        .id(ElementId::Name(id.into()))
        .p_2()
        .rounded_sm()
        .text_color(Theme::TEXT_MUTED)
        .cursor_pointer()
        .hover(|s| s.bg(Theme::BG_HOVER).text_color(Theme::TEXT))
        .tooltip(label_tooltip(tooltip))
        .on_click(move |_e, _w, cx| on_click(cx))
        .child(icon(icon_name, 16.0, Theme::TEXT_MUTED))
}

/// Two-segment pill toggle (React's Split | Unified affordance).
pub fn pill_toggle(
    id_prefix: &'static str,
    left_icon: &'static str,
    left_label: &'static str,
    right_icon: &'static str,
    right_label: &'static str,
    is_right: bool,
    on_click: impl Fn(&mut App) + 'static + Clone,
) -> impl IntoElement {
    let on_left = on_click.clone();
    let on_right = on_click;
    div()
        .flex()
        .flex_row()
        .p(px(2.0))
        .bg(Theme::BG_HOVER)
        .border_1()
        .border_color(Theme::BORDER)
        .rounded_sm()
        .gap(px(1.0))
        .child(pill_segment(
            format!("{id_prefix}-left"),
            left_icon,
            left_label,
            !is_right,
            move |cx| {
                if is_right {
                    on_left(cx);
                }
            },
        ))
        .child(pill_segment(
            format!("{id_prefix}-right"),
            right_icon,
            right_label,
            is_right,
            move |cx| {
                if !is_right {
                    on_right(cx);
                }
            },
        ))
}

fn pill_segment(
    id: String,
    icon_name: &'static str,
    label: &'static str,
    active: bool,
    on_click: impl Fn(&mut App) + 'static,
) -> impl IntoElement {
    let bg = if active { Theme::BG } else { Theme::BG_HOVER };
    let fg = if active { Theme::TEXT } else { Theme::TEXT_MUTED };
    div()
        .id(ElementId::Name(SharedString::from(id)))
        .flex()
        .flex_row()
        .items_center()
        .gap(px(6.0))
        .px(px(10.0))
        .py(px(4.0))
        .rounded_sm()
        .bg(bg)
        .text_color(fg)
        .text_size(px(11.0))
        .cursor_pointer()
        .hover(|s| {
            if active {
                s.bg(Theme::BG)
            } else {
                s.text_color(Theme::TEXT)
            }
        })
        .on_click(move |_e, _w, cx| on_click(cx))
        .child(icon(icon_name, 13.0, fg))
        .child(SharedString::from(label))
}

/// Checkbox-style toggle with label on the right (React's Ignore
/// Whitespace control).
pub fn checkbox(
    id: impl Into<SharedString>,
    checked: bool,
    label: &'static str,
    on_click: impl Fn(&mut App) + 'static,
) -> impl IntoElement {
    div()
        .id(ElementId::Name(id.into()))
        .flex()
        .flex_row()
        .items_center()
        .gap(px(6.0))
        .text_size(px(12.0))
        .text_color(Theme::TEXT_MUTED)
        .cursor_pointer()
        .hover(|s| s.text_color(Theme::TEXT))
        .on_click(move |_e, _w, cx| on_click(cx))
        .child(
            div()
                .w(px(14.0))
                .h(px(14.0))
                .rounded_xs()
                .border_1()
                .border_color(if checked { Theme::TEXT_LINK } else { Theme::BORDER })
                .bg(if checked { Theme::TEXT_LINK } else { Theme::BG })
                .flex()
                .items_center()
                .justify_center()
                .child(if checked {
                    icon("check", 10.0, Theme::TEXT).into_any_element()
                } else {
                    div().into_any_element()
                }),
        )
        .child(SharedString::from(label))
}

/// Counter + 90px progress bar showing "viewed / total" files.
pub fn viewed_progress(viewed: usize, total: usize) -> impl IntoElement {
    let label = if total > 0 && viewed == total {
        "All diffs difit-ed!".to_string()
    } else {
        format!("{viewed} / {total} files viewed")
    };
    let remaining_pct = if total > 0 {
        ((total - viewed) as f32) / (total as f32) * 100.0
    } else {
        0.0
    };
    let bar_color = if remaining_pct > 50.0 {
        Theme::FILE_STATUS_ADD
    } else if remaining_pct > 20.0 {
        Theme::FILE_STATUS_MOD
    } else {
        Theme::FILE_STATUS_DEL
    };
    let fill_width = px(remaining_pct / 100.0 * 90.0);

    div()
        .flex()
        .flex_col()
        .items_center()
        .gap(px(4.0))
        .child(
            div()
                .text_size(px(11.0))
                .text_color(Theme::TEXT_MUTED)
                .child(SharedString::from(label)),
        )
        .child(
            div()
                .w(px(90.0))
                .h(px(8.0))
                .bg(Theme::BG_HOVER)
                .border_1()
                .border_color(Theme::BORDER)
                .rounded_full()
                .relative()
                .child(
                    div()
                        .absolute()
                        .top(px(0.0))
                        .right(px(0.0))
                        .h_full()
                        .w(fill_width)
                        .bg(bar_color),
                ),
        )
}

/// "Reviewing: <hash>" badge. Clicking opens the revision detail modal.
pub fn reviewing_label(
    commit_text: SharedString,
    on_click: impl Fn(&mut App) + 'static,
) -> impl IntoElement {
    div()
        .id(ElementId::Name(SharedString::from("reviewing")))
        .flex()
        .flex_row()
        .items_center()
        .gap(px(6.0))
        .text_size(px(11.0))
        .text_color(Theme::TEXT_MUTED)
        .cursor_pointer()
        .hover(|s| s.text_color(Theme::TEXT))
        .on_click(move |_e, _w, cx| on_click(cx))
        .tooltip(label_tooltip("Revision details"))
        .child(SharedString::from("Reviewing:"))
        .child(
            div()
                .px(px(6.0))
                .py(px(1.0))
                .bg(Theme::BG_HOVER)
                .rounded_xs()
                .font_family(crate::ui::theme::MONO_FONT())
                .text_color(Theme::TEXT)
                .child(commit_text),
        )
}

/// Small hover-bubble view. Returned by `label_tooltip`.
pub struct TooltipBubble {
    pub text: SharedString,
}

impl gpui::Render for TooltipBubble {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .px_2()
            .py_1()
            .bg(Theme::BG_ELEVATED)
            .border_1()
            .border_color(Theme::BORDER)
            .rounded_sm()
            .text_size(px(11.0))
            .text_color(Theme::TEXT)
            .child(self.text.clone())
    }
}

/// Returns a closure suitable for `Div::tooltip` that pops up a labelled
/// bubble on hover.
pub fn label_tooltip(label: &'static str) -> impl Fn(&mut Window, &mut App) -> AnyView + 'static {
    move |_window, cx| {
        cx.new(|_cx| TooltipBubble {
            text: SharedString::from(label),
        })
        .into()
    }
}

/// A two-state toggle styled like the React UI (label + sliding pill).
pub fn toggle_switch(
    id: impl Into<SharedString>,
    label: &'static str,
    on: bool,
    on_click: impl Fn(&mut App) + 'static,
) -> impl IntoElement {
    let id = ElementId::Name(id.into());
    let track_bg = if on {
        Theme::TEXT_LINK
    } else {
        Theme::BG_HOVER
    };
    let knob_offset = if on { px(14.0) } else { px(2.0) };

    div()
        .id(id)
        .flex()
        .flex_row()
        .items_center()
        .gap_2()
        .cursor_pointer()
        .on_click(move |_e, _w, cx| on_click(cx))
        .child(
            div()
                .text_size(px(12.0))
                .text_color(Theme::TEXT_MUTED)
                .child(SharedString::from(label)),
        )
        .child(
            div()
                .w(px(28.0))
                .h(px(16.0))
                .bg(track_bg)
                .rounded_full()
                .relative()
                .child(
                    div()
                        .absolute()
                        .top(px(2.0))
                        .left(knob_offset)
                        .w(px(12.0))
                        .h(px(12.0))
                        .bg(Theme::TEXT)
                        .rounded_full(),
                ),
        )
}

/// "difit" wordmark, rendered via the embedded logo SVG (lifted from
/// the React `Logo.tsx`).
pub fn logo() -> impl IntoElement {
    svg()
        .path(SharedString::from("icons/difit-logo.svg"))
        .w(px(72.0))
        .h(px(19.0))
        .flex_shrink_0()
        .text_color(Theme::TEXT)
}
