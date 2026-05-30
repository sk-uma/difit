//! Selectable diff-line text element.
//!
//! GPUI ships StyledText for static text and TextInput for editable
//! text, but no read-only "drag to select, Ctrl+C to copy" primitive.
//! This element fills that gap for diff rows: it shapes the line with
//! `shape_text(wrap_width = column_width)` so long lines wrap inside
//! the diff column, paints syntax / word-diff highlights, overlays a
//! selection background when the global TextSelection on `DifitApp`
//! intersects this row, and wires mouse handlers that update that
//! global selection.
//!
//! Layout: a `request_measured_layout` closure shapes the line at the
//! column's available width and reports the wrapped size back to taffy.
//! The shaped `WrappedLine` is cached on a `Rc<RefCell<…>>` so prepaint
//! / paint / mouse handlers can all see the same layout from the same
//! frame.
//!
//! Mouse-down starts a fresh selection at the byte under the cursor;
//! mouse-move while `active` extends it (possibly into other rows that
//! own their own SelectableLine elements). Mouse-up is handled globally
//! on the diff pane root in `diff_view.rs`.

use std::cell::RefCell;
use std::ops::Range;
use std::rc::Rc;

use gpui::{
    fill, point, size, App, AvailableSpace, Bounds, DispatchPhase, ElementId, Entity,
    GlobalElementId, HighlightStyle, Hsla, InspectorElementId, IntoElement, LayoutId, MouseButton,
    MouseDownEvent, MouseMoveEvent, Pixels, Size, Style, TextRun, Window, WrappedLine,
};

use crate::app::{DifitApp, SelectionColumn, SelectionPoint, TextSelection};
use crate::ui::diff_rows::HighlightSpans;

pub struct SelectableLine {
    text: SharedString,
    highlights: HighlightSpans,
    row_idx: usize,
    column: SelectionColumn,
    app: Entity<DifitApp>,
    state: Rc<RefCell<Option<LineState>>>,
}

// Re-export to avoid an extra `use` in callers.
use gpui::SharedString;

struct LineState {
    wrapped: WrappedLine,
    /// Wrap width passed to `shape_text`. Used to skip re-shaping when
    /// taffy re-measures with the same constraints.
    wrap_width: Option<Pixels>,
    line_height: Pixels,
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
            state: Rc::new(RefCell::new(None)),
        }
    }
}

impl IntoElement for SelectableLine {
    type Element = Self;
    fn into_element(self) -> Self::Element {
        self
    }
}

impl gpui::Element for SelectableLine {
    type RequestLayoutState = ();
    type PrepaintState = ();

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
        let _ = cx;
        let state_ref = self.state.clone();
        let text = self.text.clone();
        let highlights = self.highlights.clone();
        let layout_id = window.request_measured_layout(
            Style::default(),
            move |known_dims, available, window, cx| {
                let text_style = window.text_style();
                let font_size = text_style.font_size.to_pixels(window.rem_size());
                let line_height = window.line_height();
                // Match `Text`'s logic: prefer the width taffy already
                // resolved; otherwise fall back to the definite portion
                // of `available_space`. None disables wrapping.
                let wrap_width = known_dims.width.or(match available.width {
                    AvailableSpace::Definite(x) => Some(x),
                    _ => None,
                });

                if let Some(s) = state_ref.borrow().as_ref() {
                    if s.wrap_width == wrap_width && s.line_height == line_height {
                        return s.wrapped.size(line_height);
                    }
                }

                let base_color = text_style.color;
                let runs = build_runs(&text, &highlights, &text_style, base_color);
                let _ = cx;
                let mut lines = window
                    .text_system()
                    .shape_text(text.clone(), font_size, &runs, wrap_width, None)
                    .unwrap_or_default();
                let Some(wrapped) = lines.drain(..).next() else {
                    return Size::default();
                };
                let measured = wrapped.size(line_height);
                state_ref.borrow_mut().replace(LineState {
                    wrapped,
                    wrap_width,
                    line_height,
                });
                measured
            },
        );
        (layout_id, ())
    }

    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _bounds: Bounds<Pixels>,
        _: &mut Self::RequestLayoutState,
        _window: &mut Window,
        _cx: &mut App,
    ) -> Self::PrepaintState {
    }

    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _: &mut Self::RequestLayoutState,
        _: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        // 1. Selection overlay (one quad per visual row in range).
        let selection_range = {
            let app = self.app.read(cx);
            app.text_selection.and_then(|sel| {
                selection_range_for_row(&sel, self.row_idx, self.column, self.text.len())
            })
        };
        if let Some(range) = selection_range {
            let state_ref = self.state.borrow();
            if let Some(state) = state_ref.as_ref() {
                paint_selection_overlay(
                    &state.wrapped,
                    bounds,
                    state.line_height,
                    range,
                    window,
                );
            }
        }

        // 2. Paint the wrapped text. WrappedLine::paint handles wrap
        //    boundaries internally — we just hand it the box origin and
        //    line height.
        let line_height = self
            .state
            .borrow()
            .as_ref()
            .map(|s| s.line_height)
            .unwrap_or_else(|| window.line_height());
        if let Some(state) = self.state.borrow().as_ref() {
            let _ = state.wrapped.paint(
                bounds.origin,
                line_height,
                gpui::TextAlign::Left,
                Some(bounds),
                window,
                cx,
            );
        }

        // 3. Mouse handlers — registered per-frame. Bounds-filter so
        //    each row owns its own handler without trampling siblings.
        let app_down = self.app.clone();
        let state_down = self.state.clone();
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
            let Some(byte) = byte_at_position(&state_down, bounds_down, evt.position) else {
                return;
            };
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
        let state_move = self.state.clone();
        let bounds_move = bounds;
        window.on_mouse_event(move |evt: &MouseMoveEvent, phase, _window, cx| {
            if phase != DispatchPhase::Bubble {
                return;
            }
            // Skip cheaply when no drag is in flight or it started in a
            // different column. Cross-column selection isn't supported.
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
            let Some(byte) = byte_at_position(&state_move, bounds_move, evt.position) else {
                return;
            };
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

fn byte_at_position(
    state: &Rc<RefCell<Option<LineState>>>,
    bounds: Bounds<Pixels>,
    position: gpui::Point<Pixels>,
) -> Option<usize> {
    let s = state.borrow();
    let state = s.as_ref()?;
    let rel = position - bounds.origin;
    let line_height = state.line_height;
    match state
        .wrapped
        .closest_index_for_position(rel, line_height)
    {
        Ok(b) => Some(b),
        Err(b) => Some(b),
    }
}

fn paint_selection_overlay(
    wrapped: &WrappedLine,
    bounds: Bounds<Pixels>,
    line_height: Pixels,
    range: Range<usize>,
    window: &mut Window,
) {
    let len = wrapped.len();
    let start = range.start.min(len);
    let end = range.end.min(len);
    if start >= end {
        return;
    }
    let Some(start_pos) = wrapped.position_for_index(start, line_height) else {
        return;
    };
    let Some(end_pos) = wrapped.position_for_index(end, line_height) else {
        return;
    };
    let line_w = wrapped.width();

    if start_pos.y == end_pos.y {
        let y0 = bounds.origin.y + start_pos.y;
        window.paint_quad(fill(
            Bounds {
                origin: point(bounds.origin.x + start_pos.x, y0),
                size: size(end_pos.x - start_pos.x, line_height),
            },
            selection_bg(),
        ));
        return;
    }

    // First visual row — start.x to end of the line.
    let y0_first = bounds.origin.y + start_pos.y;
    window.paint_quad(fill(
        Bounds {
            origin: point(bounds.origin.x + start_pos.x, y0_first),
            size: size(line_w - start_pos.x, line_height),
        },
        selection_bg(),
    ));

    // Middle full-width visual rows.
    let mut y = y0_first + line_height;
    let y_last = bounds.origin.y + end_pos.y;
    while y < y_last {
        window.paint_quad(fill(
            Bounds {
                origin: point(bounds.origin.x, y),
                size: size(line_w, line_height),
            },
            selection_bg(),
        ));
        y += line_height;
    }

    // Last visual row — left edge to end.x.
    window.paint_quad(fill(
        Bounds {
            origin: point(bounds.origin.x, y_last),
            size: size(end_pos.x, line_height),
        },
        selection_bg(),
    ));
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
