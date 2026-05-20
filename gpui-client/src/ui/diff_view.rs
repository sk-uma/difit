use gpui::{
    div, prelude::*, px, IntoElement, ParentElement, SharedString, Styled,
};

use crate::api::types::{DiffFile, DiffLine, LineType};
use crate::ui::theme::{Theme, MONO_FONT};

pub fn render_diff(file: Option<&DiffFile>) -> impl IntoElement {
    let mut container = div()
        .id("diff-scroll")
        .flex_1()
        .h_full()
        .bg(Theme::BG)
        .text_color(Theme::TEXT)
        .font_family(MONO_FONT)
        .text_size(px(12.5))
        .overflow_y_scroll();

    let Some(file) = file else {
        return container.child(empty_placeholder("Select a file to see its diff"));
    };

    container = container.child(file_header(file));

    if file.chunks.is_empty() {
        container = container.child(empty_placeholder(
            if file.is_generated.unwrap_or(false) {
                "Generated file — collapsed by default."
            } else {
                "No textual diff."
            },
        ));
        return container;
    }

    for chunk in &file.chunks {
        container = container.child(
            div()
                .w_full()
                .bg(Theme::DIFF_HUNK_BG)
                .px_3()
                .py_1()
                .text_color(Theme::DIFF_HUNK_TEXT)
                .child(SharedString::from(chunk.header.clone())),
        );
        for line in &chunk.lines {
            container = container.child(diff_line_row(line));
        }
    }

    container
}

fn file_header(file: &DiffFile) -> impl IntoElement {
    let path_display = match &file.old_path {
        Some(old) if old != &file.path => format!("{old} → {}", file.path),
        _ => file.path.clone(),
    };

    div()
        .w_full()
        .px_4()
        .py_2()
        .bg(Theme::BG_ELEVATED)
        .border_b_1()
        .border_color(Theme::BORDER)
        .flex()
        .items_center()
        .gap_3()
        .child(
            div()
                .text_color(Theme::TEXT)
                .text_size(px(13.0))
                .child(SharedString::from(path_display)),
        )
        .child(
            div()
                .text_color(Theme::FILE_STATUS_ADD)
                .text_size(px(11.0))
                .child(SharedString::from(format!("+{}", file.additions))),
        )
        .child(
            div()
                .text_color(Theme::FILE_STATUS_DEL)
                .text_size(px(11.0))
                .child(SharedString::from(format!("-{}", file.deletions))),
        )
}

fn diff_line_row(line: &DiffLine) -> impl IntoElement {
    let (bg, marker) = match line.kind {
        LineType::Add => (Theme::DIFF_ADD_BG, "+"),
        LineType::Delete | LineType::Remove => (Theme::DIFF_DEL_BG, "-"),
        LineType::Hunk | LineType::Header => (Theme::DIFF_HUNK_BG, " "),
        LineType::Normal | LineType::Context => (Theme::BG, " "),
    };

    div()
        .w_full()
        .flex()
        .flex_row()
        .bg(bg)
        .child(gutter(line.old_line_number))
        .child(gutter(line.new_line_number))
        .child(
            div()
                .w(px(18.0))
                .text_color(Theme::TEXT_MUTED)
                .child(SharedString::from(marker)),
        )
        .child(
            div()
                .flex_1()
                .px_1()
                .whitespace_nowrap()
                .child(SharedString::from(expand_tabs(&line.content))),
        )
}

fn gutter(value: Option<u32>) -> impl IntoElement {
    div()
        .w(px(56.0))
        .px_2()
        .text_color(Theme::TEXT_MUTED)
        .child(SharedString::from(
            value.map(|n| n.to_string()).unwrap_or_default(),
        ))
}

fn empty_placeholder(msg: &'static str) -> impl IntoElement {
    div()
        .w_full()
        .p_8()
        .text_color(Theme::TEXT_MUTED)
        .child(SharedString::from(msg))
}

fn expand_tabs(s: &str) -> String {
    // GPUI does not render tab characters with any width; expand to 4 spaces
    // so indented code lines up.
    s.replace('\t', "    ")
}
