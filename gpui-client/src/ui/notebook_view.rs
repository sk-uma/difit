//! Jupyter notebook viewer.
//!
//! Parses an .ipynb JSON blob and renders each cell: markdown cells
//! flow through the existing `markdown_view::render_markdown`, code
//! cells render as monospace blocks with their text/plain outputs
//! (other MIME types — images, HTML, latex — are listed by type but
//! not rendered).

use gpui::{div, prelude::*, px, IntoElement, ParentElement, SharedString, Styled};
use serde::Deserialize;
use serde_json::Value;

use crate::ui::markdown_view::render_markdown;
use crate::ui::theme::{Theme, MONO_FONT, UI_FONT};

#[derive(Debug, Deserialize)]
struct Notebook {
    #[serde(default)]
    cells: Vec<Cell>,
}

#[derive(Debug, Deserialize)]
struct Cell {
    cell_type: String,
    #[serde(default)]
    source: Value,
    #[serde(default)]
    outputs: Vec<Value>,
    #[serde(default)]
    execution_count: Option<u32>,
}

pub fn is_notebook_ext(ext: &str) -> bool {
    ext == "ipynb"
}

pub fn render_notebook(blob_bytes: &[u8], font_size: f32) -> impl IntoElement {
    let body = div()
        .id("notebook-scroll")
        .flex_1()
        .min_h_0()
        .min_w_0()
        .overflow_y_scroll()
        .px_4()
        .py_3()
        .font_family(UI_FONT)
        .text_color(Theme::TEXT)
        .text_size(px(font_size))
        .flex()
        .flex_col()
        .gap_3();

    let notebook = match serde_json::from_slice::<Notebook>(blob_bytes) {
        Ok(n) => n,
        Err(e) => {
            return body.child(error_box(&format!("Failed to parse notebook: {e}")));
        }
    };

    notebook
        .cells
        .into_iter()
        .enumerate()
        .fold(body, |acc, (i, cell)| acc.child(render_cell(i, cell, font_size)))
}

fn render_cell(idx: usize, cell: Cell, font_size: f32) -> gpui::AnyElement {
    let source = source_to_string(&cell.source);
    match cell.cell_type.as_str() {
        "markdown" => div()
            .border_1()
            .border_color(Theme::BORDER)
            .rounded_sm()
            .p_2()
            .child(
                div()
                    .text_color(Theme::TEXT_MUTED)
                    .text_size(px(font_size * 0.8))
                    .child(SharedString::from(format!("Cell {idx} • markdown"))),
            )
            .child(render_markdown(&source, font_size))
            .into_any_element(),
        "code" => {
            let exec = cell
                .execution_count
                .map(|n| format!("[{n}]"))
                .unwrap_or_else(|| "[ ]".to_string());
            let mut block = div()
                .border_1()
                .border_color(Theme::BORDER)
                .rounded_sm()
                .p_2()
                .flex()
                .flex_col()
                .gap_2()
                .child(
                    div()
                        .text_color(Theme::TEXT_MUTED)
                        .text_size(px(font_size * 0.8))
                        .child(SharedString::from(format!("Cell {idx} • code {exec}"))),
                )
                .child(
                    div()
                        .px_2()
                        .py_1()
                        .bg(Theme::BG_ELEVATED)
                        .font_family(MONO_FONT)
                        .text_size(px(font_size * 0.95))
                        .text_color(Theme::TEXT)
                        .child(SharedString::from(source)),
                );
            for out in cell.outputs {
                block = block.child(render_output(&out, font_size));
            }
            block.into_any_element()
        }
        other => div()
            .text_color(Theme::TEXT_MUTED)
            .child(SharedString::from(format!(
                "Cell {idx} • {other} (not rendered)"
            )))
            .into_any_element(),
    }
}

fn render_output(out: &Value, font_size: f32) -> gpui::AnyElement {
    let output_type = out
        .get("output_type")
        .and_then(Value::as_str)
        .unwrap_or("?");

    let text = match output_type {
        "stream" => out
            .get("text")
            .map(source_to_string)
            .unwrap_or_default(),
        "execute_result" | "display_data" => out
            .get("data")
            .and_then(Value::as_object)
            .and_then(|d| d.get("text/plain").map(source_to_string))
            .unwrap_or_else(|| {
                // Other MIME types we don't render.
                out.get("data")
                    .and_then(Value::as_object)
                    .map(|d| {
                        let mime_types: Vec<&str> = d.keys().map(String::as_str).collect();
                        format!("[non-text output: {}]", mime_types.join(", "))
                    })
                    .unwrap_or_default()
            }),
        "error" => {
            let ename = out.get("ename").and_then(Value::as_str).unwrap_or("Error");
            let evalue = out
                .get("evalue")
                .and_then(Value::as_str)
                .unwrap_or_default();
            format!("{ename}: {evalue}")
        }
        _ => format!("[unsupported output: {output_type}]"),
    };

    if text.is_empty() {
        return div().into_any_element();
    }

    let is_error = output_type == "error";
    div()
        .px_2()
        .py_1()
        .border_l_1()
        .border_color(if is_error {
            Theme::FILE_STATUS_DEL
        } else {
            Theme::BORDER
        })
        .font_family(MONO_FONT)
        .text_size(px(font_size * 0.95))
        .text_color(if is_error {
            Theme::FILE_STATUS_DEL
        } else {
            Theme::TEXT_MUTED
        })
        .child(SharedString::from(text))
        .into_any_element()
}

fn source_to_string(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Array(items) => items
            .iter()
            .map(|i| i.as_str().unwrap_or_default())
            .collect::<String>(),
        _ => String::new(),
    }
}

fn error_box(msg: &str) -> impl IntoElement {
    div()
        .p_4()
        .text_color(Theme::FILE_STATUS_DEL)
        .child(SharedString::from(msg.to_string()))
}
