use gpui::{div, prelude::*, px, App, ElementId, IntoElement, ParentElement, SharedString, Styled};

use crate::settings_store::{Settings, FONT_SIZES, SYNTAX_THEMES};
use crate::ui::theme::{Theme, UI_FONT};

pub fn render_settings_modal(
    settings: Settings,
    on_apply: impl Fn(Settings, &mut App) + 'static + Clone,
    on_close: impl Fn(&mut App) + 'static,
) -> impl IntoElement {
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
            move |_e, _w, cx| on_close(cx),
        )
        .child(
            div()
                .w(px(420.0))
                .max_h(px(560.0))
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
                .gap_3()
                .child(
                    div()
                        .text_size(px(14.0))
                        .child(SharedString::from("Settings")),
                )
                .child(font_size_row(&settings, on_apply.clone()))
                .child(theme_row(&settings, on_apply))
                .child(
                    div()
                        .mt_2()
                        .text_color(Theme::TEXT_MUTED)
                        .text_size(px(11.0))
                        .child(SharedString::from(
                            "Saved to settings.json in the OS config dir.",
                        )),
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
    div()
        .flex()
        .flex_col()
        .gap_1()
        .child(
            div()
                .text_color(Theme::TEXT_MUTED)
                .text_size(px(11.0))
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
