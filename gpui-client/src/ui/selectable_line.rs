//! Selectable diff-line text element.
//!
//! GPUI ships StyledText for static text and TextInput for editable
//! text, but no read-only "drag to select, Ctrl+C to copy" primitive.
//! This element fills that gap for diff rows: it shapes the line, paints
//! syntax / word-diff highlights, overlays a selection background when
//! the global TextSelection on `DifitApp` intersects this row, and wires
//! mouse handlers that update that global selection.
//!
//! Mouse-down starts a fresh selection at the byte under the cursor;
//! mouse-move while `active` extends it (possibly into other rows that
//! own their own SelectableLine elements). Mouse-up is handled globally
//! on the diff pane root in `diff_view.rs`.

use std::ops::Range;

use gpui::{
    fill, point, App, Bounds, DispatchPhase, ElementId, Entity, GlobalElementId, HighlightStyle,
    Hsla, InspectorElementId, IntoElement, LayoutId, MouseButton, MouseDownEvent, MouseMoveEvent,
    PaintQuad, Pixels, ShapedLine, SharedString, Style, TextRun, Window,
};

use crate::app::{DifitApp, SelectionColumn, SelectionPoint, TextSelection};
use crate::ui::diff_rows::HighlightSpans;

pub struct SelectableLine {
    text: SharedString,
    highlights: HighlightSpans,
    row_idx: usize,
    column: SelectionColumn,
    app: Entity<DifitApp>,
}

impl SelectableLine {
    pub fn new(
        text: SharedString,
        highlights: HighlightSpans,
        row_idx: usize,
        column: SelectionColumn,
        app: Entity<DifitApp>,
    ) -> Self {
        Self {
            text,
            highlights,
            row_idx,
            column,
            app,
        }
    }
}

pub struct PrepaintState {
    shaped: ShapedLine,
    sel_quad: Option<PaintQuad>,
}

impl IntoElement for SelectableLine {
    type Element = Self;
    fn into_element(self) -> Self::Element {
        self
    }
}

impl gpui::Element for SelectableLine {
    type RequestLayoutState = ();
    type PrepaintState = PrepaintState;

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        let mut style = Style::default();
        style.size.width = gpui::relative(1.0).into();
        style.size.height = window.line_height().into();
        (window.request_layout(style, [], cx), ())
    }

    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) -> Self::PrepaintState {
        let text_style = window.text_style();
        let font_size = text_style.font_size.to_pixels(window.rem_size());
        let base_color = text_style.color;
        let runs = build_runs(&self.text, &self.highlights, &text_style, base_color);
        let shaped =
            window
                .text_system()
                .shape_line(self.text.clone(), font_size, &runs, None);

        let sel_quad = self
            .app
            .read(cx)
            .text_selection
            .and_then(|sel| selection_range_for_row(&sel, self.row_idx, self.column, self.text.len()))
            .map(|range| {
                let x_start = bounds.left() + shaped.x_for_index(range.start);
                let x_end = bounds.left() + shaped.x_for_index(range.end);
                fill(
                    Bounds::from_corners(
                        point(x_start, bounds.top()),
                        point(x_end, bounds.bottom()),
                    ),
                    selection_bg(),
                )
            });

        PrepaintState { shaped, sel_quad }
    }

    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _: &mut Self::RequestLayoutState,
        prepaint: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        if let Some(q) = prepaint.sel_quad.take() {
            window.paint_quad(q);
        }

        let line_height = window.line_height();
        let _ = prepaint.shaped.paint(
            point(bounds.left(), bounds.top()),
            line_height,
            gpui::TextAlign::Left,
            None,
            window,
            cx,
        );

        // Mouse handlers — registered per-frame in paint. They're
        // bounds-filtered so siblings can each own their own handler
        // without stepping on each other.
        let app_down = self.app.clone();
        let shaped_down = prepaint.shaped.clone();
        let row_idx = self.row_idx;
        let column = self.column;
        let bounds_down = bounds;
        window.on_mouse_event(move |evt: &MouseDownEvent, phase, _window, cx| {
            if phase != DispatchPhase::Bubble {
                return;
            }
            if evt.button != MouseButton::Left {
                return;
            }
            if !bounds_down.contains(&evt.position) {
                return;
            }
            let byte = shaped_down.closest_index_for_x(evt.position.x - bounds_down.left());
            app_down.update(cx, |this, cx| {
                this.text_selection = Some(TextSelection {
                    anchor: SelectionPoint {
                        row_idx,
                        column,
                        byte,
                    },
                    cursor: SelectionPoint {
                        row_idx,
                        column,
                        byte,
                    },
                    active: true,
                });
                cx.notify();
            });
        });

        let app_move = self.app.clone();
        let shaped_move = prepaint.shaped.clone();
        let bounds_move = bounds;
        window.on_mouse_event(move |evt: &MouseMoveEvent, phase, _window, cx| {
            if phase != DispatchPhase::Bubble {
                return;
            }
            // Skip cheaply when no active drag is in flight or the drag
            // started in a different column. Cross-column selection is
            // intentionally not supported — it doesn't have a clear
            // semantic in split view.
            let allow = app_move
                .read(cx)
                .text_selection
                .as_ref()
                .map(|s| s.active && s.anchor.column == column)
                .unwrap_or(false);
            if !allow {
                return;
            }
            if !bounds_move.contains(&evt.position) {
                return;
            }
            let byte = shaped_move.closest_index_for_x(evt.position.x - bounds_move.left());
            app_move.update(cx, |this, cx| {
                if let Some(sel) = this.text_selection.as_mut() {
                    if sel.active && sel.anchor.column == column {
                        sel.cursor = SelectionPoint {
                            row_idx,
                            column,
                            byte,
                        };
                        cx.notify();
                    }
                }
            });
        });
    }
}

fn build_runs(
    text: &str,
    highlights: &HighlightSpans,
    style: &gpui::TextStyle,
    base_color: Hsla,
) -> Vec<TextRun> {
    let base_run = |len: usize| TextRun {
        len,
        font: style.font(),
        color: base_color,
        background_color: None,
        underline: None,
        strikethrough: None,
    };

    if highlights.is_empty() {
        return vec![base_run(text.len())];
    }

    let mut spans: Vec<&(Range<usize>, HighlightStyle)> = highlights.iter().collect();
    spans.sort_by_key(|(r, _)| (r.start, r.end));

    let mut runs: Vec<TextRun> = Vec::with_capacity(spans.len() * 2 + 1);
    let mut cursor = 0usize;
    for (range, hl) in spans {
        if range.start >= text.len() {
            break;
        }
        let end = range.end.min(text.len());
        if end <= range.start {
            continue;
        }
        if range.start > cursor {
            runs.push(base_run(range.start - cursor));
        }
        if range.start < cursor {
            // Overlapping highlight — skip (our spans are usually
            // pre-merged via combine_highlights).
            continue;
        }
        runs.push(TextRun {
            len: end - range.start,
            font: style.font(),
            color: hl.color.unwrap_or(base_color),
            background_color: hl.background_color,
            underline: hl.underline,
            strikethrough: hl.strikethrough,
        });
        cursor = end;
    }
    if cursor < text.len() {
        runs.push(base_run(text.len() - cursor));
    }
    runs
}

/// Byte range covered by `sel` on the row identified by (row_idx, column).
/// Returns `None` if this row is outside the selection or the selection's
/// column doesn't match.
fn selection_range_for_row(
    sel: &TextSelection,
    row_idx: usize,
    column: SelectionColumn,
    text_len: usize,
) -> Option<Range<usize>> {
    if sel.anchor.column != column {
        return None;
    }
    if sel.is_empty() {
        return None;
    }
    let (start, end) = sel.ordered();
    if row_idx < start.row_idx || row_idx > end.row_idx {
        return None;
    }
    if start.row_idx == end.row_idx {
        if start.byte == end.byte {
            return None;
        }
        return Some(start.byte.min(text_len)..end.byte.min(text_len));
    }
    if row_idx == start.row_idx {
        return Some(start.byte.min(text_len)..text_len);
    }
    if row_idx == end.row_idx {
        return Some(0..end.byte.min(text_len));
    }
    Some(0..text_len)
}

fn selection_bg() -> Hsla {
    gpui::hsla(0.58, 0.7, 0.5, 0.35)
}
