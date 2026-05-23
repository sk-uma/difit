use gpui::{div, prelude::*, px, App, ElementId, IntoElement, ParentElement, SharedString, Styled};

use crate::settings_store::{Settings, FONT_SIZES, SYNTAX_THEMES};
use crate::ui::theme::{Theme, UI_FONT};
use crate::ui::widgets::icon;

pub fn render_settings_modal(
    settings: Settings,
    on_apply: impl Fn(Settings, &mut App) + 'static + Clone,
    on_close: impl Fn(&mut App) + 'static + Clone,
) -> impl IntoElement {
    let close_backdrop = on_close.clone();
    let close_button = on_close;
    div()
        .id("settings-modal")
        .absolute()
        .inset_0()
        .flex()
        .items_center()
        .justify_center()
        .bg(gpui::hsla(0.0, 0.0, 0.0, 0.5))
        .on_mouse_down(
            gpui::MouseButton::Left,
            move |_e, _w, cx| close_backdrop(cx),
        )
        .child(
            div()
                .id("settings-modal-card")
                .w(px(520.0))
                .max_h(px(640.0))
                .bg(Theme::BG)
                .border_1()
                .border_color(Theme::BORDER)
                .rounded_md()
                .shadow_lg()
                .font_family(UI_FONT())
                .text_color(Theme::TEXT)
                .on_mouse_down(gpui::MouseButton::Left, |_e, _w, _cx| {})
                .flex()
                .flex_col()
                .child(
                    div()
                        .px(px(24.0))
                        .py(px(16.0))
                        .border_b_1()
                        .border_color(Theme::BORDER)
                        .flex()
                        .flex_row()
                        .items_center()
                        .justify_between()
                        .child(
                            div()
                                .text_size(px(18.0))
                                .font_weight(gpui::FontWeight::SEMIBOLD)
                                .child(SharedString::from("Settings")),
                        )
                        .child(
                            div()
                                .id("settings-close")
                                .p_1()
                                .rounded_sm()
                                .cursor_pointer()
                                .hover(|s| s.bg(Theme::BG_HOVER))
                                .on_click(move |_e, _w, cx| close_button(cx))
                                .child(icon("x", 18.0, Theme::TEXT_MUTED)),
                        ),
                )
                .child(
                    div()
                        .px(px(24.0))
                        .py(px(20.0))
                        .flex()
                        .flex_col()
                        .gap(px(20.0))
                        .child(font_size_row(&settings, on_apply.clone()))
                        .child(theme_row(&settings, on_apply))
                        .child(
                            div()
                                .text_color(Theme::TEXT_MUTED)
                                .text_size(px(11.0))
                                .child(SharedString::from(
                                    "Saved to settings.json in the OS config dir.",
                                )),
                        ),
                ),
        )
}

fn font_size_row(
    settings: &Settings,
    on_apply: impl Fn(Settings, &mut App) + 'static + Clone,
) -> impl IntoElement {
    let current = settings.font_size;
    let theme_name = settings.syntax_theme.clone();
    section("Font size").child(
        div()
            .flex()
            .flex_row()
            .gap_2()
            .children(FONT_SIZES.iter().copied().map(|(label, size)| {
                let active = (current - size).abs() < 0.1;
                let cb = on_apply.clone();
                let theme_name = theme_name.clone();
                pill(
                    format!("font-{label}"),
                    label,
                    active,
                    move |cx| {
                        cb(
                            Settings {
                                font_size: size,
                                syntax_theme: theme_name.clone(),
                            },
                            cx,
                        );
                    },
                )
            })),
    )
}

fn theme_row(
    settings: &Settings,
    on_apply: impl Fn(Settings, &mut App) + 'static + Clone,
) -> impl IntoElement {
    let current = settings.syntax_theme.clone();
    let font_size = settings.font_size;
    section("Syntax theme").child(
        div()
            .flex()
            .flex_col()
            .gap_1()
            .children(SYNTAX_THEMES.iter().copied().map(|name| {
                let active = current == name;
                let cb = on_apply.clone();
                pill(
                    format!("theme-{name}"),
                    name,
                    active,
                    move |cx| {
                        cb(
                            Settings {
                                font_size,
                                syntax_theme: name.to_string(),
                            },
                            cx,
                        );
                    },
                )
            })),
    )
}

fn section(label: &'static str) -> gpui::Div {
    div().flex().flex_col().gap(px(8.0)).child(
        div()
            .text_color(Theme::TEXT)
            .text_size(px(13.0))
            .font_weight(gpui::FontWeight::SEMIBOLD)
            .child(SharedString::from(label)),
    )
}

fn pill(
    id: String,
    label: &'static str,
    active: bool,
    on_click: impl Fn(&mut App) + 'static,
) -> impl IntoElement {
    div()
        .id(ElementId::Name(SharedString::from(id)))
        .px_3()
        .py_1()
        .rounded_sm()
        .border_1()
        .border_color(if active { Theme::TEXT_LINK } else { Theme::BORDER })
        .bg(if active { Theme::BG_SELECTED } else { Theme::BG_ELEVATED })
        .text_color(if active { Theme::TEXT } else { Theme::TEXT_MUTED })
        .cursor_pointer()
        .hover(|s| s.bg(Theme::BG_HOVER))
        .on_click(move |_e, _w, cx| on_click(cx))
        .child(SharedString::from(label))
}
