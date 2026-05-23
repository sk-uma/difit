//! Custom client-side titlebar.
//!
//! On Windows the OS title bar is hidden (via
//! `TitlebarOptions::appears_transparent = true`) and we paint our own
//! 32px strip. Each control area is published to Win32 via
//! `Div::window_control_area(...)` — the regular hitbox path the div
//! emits during prepaint also gets registered as a `WindowControlArea`,
//! so Win32's WM_NCHITTEST → WM_NCLBUTTONDOWN sequence keeps driving
//! native drag, double-click maximize, snap layouts, etc. without us
//! having to reimplement any of it.

use gpui::{
    div, prelude::*, px, ElementId, IntoElement, ParentElement, Rgba, SharedString, Styled,
    WindowControlArea,
};

use crate::ui::theme::{Theme, UI_FONT};
use crate::ui::widgets::{icon, logo};

pub fn render_titlebar(is_maximized: bool) -> impl IntoElement {
    div()
        .w_full()
        .h(px(32.0))
        .flex()
        .flex_row()
        .items_stretch()
        .bg(Theme::BG_ELEVATED)
        .border_b_1()
        .border_color(Theme::BORDER)
        .font_family(UI_FONT())
        .child(drag_region())
        .child(control_button(
            "tb-min",
            "minus",
            Theme::BG_HOVER,
            WindowControlArea::Min,
        ))
        .child(control_button(
            "tb-max",
            if is_maximized { "restore" } else { "square" },
            Theme::BG_HOVER,
            WindowControlArea::Max,
        ))
        .child(control_button(
            "tb-close",
            "x",
            // Windows-style red close-button hover color.
            Rgba {
                r: 0.9,
                g: 0.18,
                b: 0.2,
                a: 1.0,
            },
            WindowControlArea::Close,
        ))
}

/// Body of the title bar: logo + "difit" text. The whole row publishes
/// itself as the OS drag region.
fn drag_region() -> impl IntoElement {
    div()
        .id(ElementId::Name(SharedString::from("titlebar-drag")))
        .flex_1()
        .min_w_0()
        .h_full()
        .flex()
        .items_center()
        .gap(px(8.0))
        .px(px(12.0))
        .window_control_area(WindowControlArea::Drag)
        .child(logo())
        .child(
            div()
                .text_size(px(11.0))
                .text_color(Theme::TEXT_MUTED)
                .child(SharedString::from("difit")),
        )
}

fn control_button(
    id: &'static str,
    icon_name: &'static str,
    hover_bg: Rgba,
    area: WindowControlArea,
) -> impl IntoElement {
    div()
        .id(ElementId::Name(SharedString::from(id)))
        .w(px(46.0))
        .h_full()
        .flex()
        .items_center()
        .justify_center()
        .hover(move |s| s.bg(hover_bg))
        .window_control_area(area)
        .child(icon(icon_name, 14.0, Theme::TEXT_MUTED))
}
