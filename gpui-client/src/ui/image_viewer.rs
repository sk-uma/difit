//! Side-by-side image diff viewer.
//!
//! Backed by `/api/blob/<path>?ref=…` (raw bytes) and rendered via
//! `gpui::img` from a `gpui::Image` built out of those bytes plus the
//! format inferred from the file extension.

use std::sync::Arc;

use gpui::{div, img, prelude::*, px, Image, ImageFormat, IntoElement, ParentElement, SharedString, Styled};

use crate::api::types::{DiffFile, FileStatus};
use crate::ui::theme::{Theme, UI_FONT};

pub fn is_image_ext(ext: &str) -> bool {
    matches!(
        ext,
        "png" | "jpg" | "jpeg" | "gif" | "webp" | "svg" | "bmp" | "ico"
    )
}

fn image_format_for(ext: &str) -> Option<ImageFormat> {
    Some(match ext {
        "png" => ImageFormat::Png,
        "jpg" | "jpeg" => ImageFormat::Jpeg,
        "gif" => ImageFormat::Gif,
        "webp" => ImageFormat::Webp,
        "svg" => ImageFormat::Svg,
        "bmp" => ImageFormat::Bmp,
        "ico" => ImageFormat::Ico,
        _ => return None,
    })
}

pub fn render_image_diff(
    file: &DiffFile,
    extension: &str,
    old_bytes: Option<Arc<Vec<u8>>>,
    new_bytes: Option<Arc<Vec<u8>>>,
) -> impl IntoElement {
    let format = image_format_for(extension);

    let show_old = !matches!(file.status, FileStatus::Added);
    let show_new = !matches!(file.status, FileStatus::Deleted);

    div()
        .w_full()
        .h_full()
        .flex()
        .flex_row()
        .min_h_0()
        .min_w_0()
        .font_family(UI_FONT)
        .text_color(Theme::TEXT)
        .child(if show_old {
            image_pane(
                "old",
                format,
                old_bytes.as_deref().map(|v| v.as_slice()),
                "Old",
            )
        } else {
            empty_pane("(file added)")
        })
        .child(div().w(px(1.0)).h_full().bg(Theme::BORDER))
        .child(if show_new {
            image_pane(
                "new",
                format,
                new_bytes.as_deref().map(|v| v.as_slice()),
                "New",
            )
        } else {
            empty_pane("(file deleted)")
        })
}

fn image_pane(
    side_tag: &'static str,
    format: Option<ImageFormat>,
    bytes: Option<&[u8]>,
    label: &'static str,
) -> gpui::Div {
    let header = div()
        .px_3()
        .py_1()
        .border_b_1()
        .border_color(Theme::BORDER)
        .text_color(Theme::TEXT_MUTED)
        .text_size(px(11.0))
        .child(SharedString::from(format!(
            "{label}{}",
            bytes
                .map(|b| format!("  •  {} bytes", b.len()))
                .unwrap_or_default()
        )));

    let body = match (format, bytes) {
        (Some(format), Some(bytes)) => {
            let image = Arc::new(Image::from_bytes(format, bytes.to_vec()));
            div()
                .flex_1()
                .min_h_0()
                .flex()
                .items_center()
                .justify_center()
                .p_2()
                .child(img(image).max_h_full().max_w_full())
        }
        (None, _) => placeholder_body("Unsupported image format"),
        (_, None) => placeholder_body("Loading…"),
    };

    let _ = side_tag;
    div()
        .w_1_2()
        .min_w_0()
        .h_full()
        .flex()
        .flex_col()
        .child(header)
        .child(body)
}

fn placeholder_body(msg: &'static str) -> gpui::Div {
    div()
        .flex_1()
        .min_h_0()
        .flex()
        .items_center()
        .justify_center()
        .text_color(Theme::TEXT_MUTED)
        .text_size(px(12.0))
        .child(SharedString::from(msg))
}

fn empty_pane(msg: &'static str) -> gpui::Div {
    div()
        .w_1_2()
        .h_full()
        .flex()
        .items_center()
        .justify_center()
        .text_color(Theme::TEXT_MUTED)
        .text_size(px(12.0))
        .child(SharedString::from(msg))
}
