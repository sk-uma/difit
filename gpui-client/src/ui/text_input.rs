//! A reusable text input widget for GPUI.
//!
//! Adapted from `crates/gpui/examples/input.rs`, generalized to support
//! both single-line and multi-line editing so it can back comment forms
//! (multi-line) and future settings fields (single-line).
//!
//! Multi-line specifics:
//! - `\n` splits the content into shaped lines that are stacked vertically.
//! - Cursor offsets remain byte offsets into the full string.
//! - Up/Down arrows preserve the horizontal pixel position when jumping
//!   between lines.
//! - Click anywhere positions the caret in the corresponding line/column.
//!
//! Keybindings are registered via [`bind_keys`] at startup. They use the
//! `"TextInput"` key context so they only fire while an input is focused.

use std::ops::Range;

use gpui::{
    actions, div, fill, point, prelude::*, px, App, Bounds, ClipboardItem, Context, CursorStyle,
    Element, ElementId, ElementInputHandler, Entity, EntityInputHandler, FocusHandle, Focusable,
    GlobalElementId, IntoElement, KeyBinding, LayoutId, MouseButton, MouseDownEvent,
    MouseMoveEvent, MouseUpEvent, PaintQuad, Pixels, Point, ShapedLine, SharedString, Style,
    TextRun, UTF16Selection, UnderlineStyle, Window,
};
use unicode_segmentation::UnicodeSegmentation;

use crate::ui::theme::Theme;

actions!(
    difit_text_input,
    [
        Backspace,
        Delete,
        Left,
        Right,
        Up,
        Down,
        SelectLeft,
        SelectRight,
        SelectAll,
        Home,
        End,
        Newline,
        Paste,
        Copy,
        Cut,
        ShowCharacterPalette,
    ]
);

/// Register the input keymap. Call once at startup.
pub fn bind_keys(cx: &mut App) {
    let ctx = Some("TextInput");
    cx.bind_keys([
        KeyBinding::new("backspace", Backspace, ctx),
        KeyBinding::new("delete", Delete, ctx),
        KeyBinding::new("left", Left, ctx),
        KeyBinding::new("right", Right, ctx),
        KeyBinding::new("up", Up, ctx),
        KeyBinding::new("down", Down, ctx),
        KeyBinding::new("shift-left", SelectLeft, ctx),
        KeyBinding::new("shift-right", SelectRight, ctx),
        KeyBinding::new("home", Home, ctx),
        KeyBinding::new("end", End, ctx),
        KeyBinding::new("enter", Newline, ctx),
        KeyBinding::new("cmd-a", SelectAll, ctx),
        KeyBinding::new("ctrl-a", SelectAll, ctx),
        KeyBinding::new("cmd-c", Copy, ctx),
        KeyBinding::new("ctrl-c", Copy, ctx),
        KeyBinding::new("cmd-v", Paste, ctx),
        KeyBinding::new("ctrl-v", Paste, ctx),
        KeyBinding::new("cmd-x", Cut, ctx),
        KeyBinding::new("ctrl-x", Cut, ctx),
        KeyBinding::new("ctrl-cmd-space", ShowCharacterPalette, ctx),
    ]);
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputMode {
    SingleLine,
    MultiLine,
}

pub struct TextInput {
    focus_handle: FocusHandle,
    mode: InputMode,
    content: SharedString,
    placeholder: SharedString,
    selected_range: Range<usize>,
    selection_reversed: bool,
    marked_range: Option<Range<usize>>,
    last_layout: Vec<LineLayout>,
    last_bounds: Option<Bounds<Pixels>>,
    is_selecting: bool,
}

struct LineLayout {
    /// Byte range of this line within the content (excludes the trailing
    /// '\n' for all but the last line).
    range: Range<usize>,
    line: ShapedLine,
}

impl TextInput {
    pub fn new(
        mode: InputMode,
        placeholder: impl Into<SharedString>,
        cx: &mut Context<Self>,
    ) -> Self {
        Self {
            focus_handle: cx.focus_handle(),
            mode,
            content: SharedString::from(""),
            placeholder: placeholder.into(),
            selected_range: 0..0,
            selection_reversed: false,
            marked_range: None,
            last_layout: Vec::new(),
            last_bounds: None,
            is_selecting: false,
        }
    }

    pub fn content(&self) -> &str {
        &self.content
    }

    pub fn is_empty(&self) -> bool {
        self.content.is_empty()
    }

    pub fn reset(&mut self) {
        self.content = SharedString::from("");
        self.selected_range = 0..0;
        self.selection_reversed = false;
        self.marked_range = None;
        self.last_layout.clear();
        self.last_bounds = None;
        self.is_selecting = false;
    }

    /// Set the editor content programmatically. Cursor is moved to the end.
    pub fn set_content(&mut self, text: &str) {
        let text = if self.mode == InputMode::SingleLine {
            text.replace('\n', " ")
        } else {
            text.to_string()
        };
        let len = text.len();
        self.content = SharedString::from(text);
        self.selected_range = len..len;
        self.selection_reversed = false;
        self.marked_range = None;
    }

    pub fn focus(&self, window: &mut Window, cx: &mut App) {
        window.focus(&self.focus_handle, cx);
    }

    pub fn focus_handle(&self) -> FocusHandle {
        self.focus_handle.clone()
    }

    // ---- cursor / selection helpers ------------------------------------

    fn cursor_offset(&self) -> usize {
        if self.selection_reversed {
            self.selected_range.start
        } else {
            self.selected_range.end
        }
    }

    fn move_to(&mut self, offset: usize, cx: &mut Context<Self>) {
        self.selected_range = offset..offset;
        cx.notify();
    }

    fn select_to(&mut self, offset: usize, cx: &mut Context<Self>) {
        if self.selection_reversed {
            self.selected_range.start = offset;
        } else {
            self.selected_range.end = offset;
        }
        if self.selected_range.end < self.selected_range.start {
            self.selection_reversed = !self.selection_reversed;
            self.selected_range = self.selected_range.end..self.selected_range.start;
        }
        cx.notify();
    }

    fn previous_boundary(&self, offset: usize) -> usize {
        self.content
            .grapheme_indices(true)
            .rev()
            .find_map(|(idx, _)| (idx < offset).then_some(idx))
            .unwrap_or(0)
    }

    fn next_boundary(&self, offset: usize) -> usize {
        self.content
            .grapheme_indices(true)
            .find_map(|(idx, _)| (idx > offset).then_some(idx))
            .unwrap_or(self.content.len())
    }

    /// Index into `self.last_layout` for the line containing the given byte
    /// offset. Falls back to the last line if the offset is past the end.
    fn line_index_for_offset(&self, offset: usize) -> usize {
        if self.last_layout.is_empty() {
            return 0;
        }
        for (i, ll) in self.last_layout.iter().enumerate() {
            if offset <= ll.range.end {
                return i;
            }
        }
        self.last_layout.len() - 1
    }

    // ---- action handlers -----------------------------------------------

    fn left(&mut self, _: &Left, _: &mut Window, cx: &mut Context<Self>) {
        if self.selected_range.is_empty() {
            self.move_to(self.previous_boundary(self.cursor_offset()), cx);
        } else {
            self.move_to(self.selected_range.start, cx);
        }
    }

    fn right(&mut self, _: &Right, _: &mut Window, cx: &mut Context<Self>) {
        if self.selected_range.is_empty() {
            self.move_to(self.next_boundary(self.selected_range.end), cx);
        } else {
            self.move_to(self.selected_range.end, cx);
        }
    }

    fn up(&mut self, _: &Up, window: &mut Window, cx: &mut Context<Self>) {
        if self.mode == InputMode::SingleLine || self.last_layout.is_empty() {
            self.home(&Home, window, cx);
            return;
        }
        let cursor = self.cursor_offset();
        let line_idx = self.line_index_for_offset(cursor);
        if line_idx == 0 {
            self.move_to(0, cx);
            return;
        }
        let target_line = &self.last_layout[line_idx];
        let in_line_offset = cursor.saturating_sub(target_line.range.start);
        let x = target_line.line.x_for_index(in_line_offset);

        let prev = &self.last_layout[line_idx - 1];
        let new_in_line = prev.line.closest_index_for_x(x);
        let new_offset = prev.range.start + new_in_line;
        self.move_to(new_offset.min(prev.range.end), cx);
    }

    fn down(&mut self, _: &Down, window: &mut Window, cx: &mut Context<Self>) {
        if self.mode == InputMode::SingleLine || self.last_layout.is_empty() {
            self.end(&End, window, cx);
            return;
        }
        let cursor = self.cursor_offset();
        let line_idx = self.line_index_for_offset(cursor);
        if line_idx + 1 >= self.last_layout.len() {
            self.move_to(self.content.len(), cx);
            return;
        }
        let cur = &self.last_layout[line_idx];
        let in_line_offset = cursor.saturating_sub(cur.range.start);
        let x = cur.line.x_for_index(in_line_offset);

        let next = &self.last_layout[line_idx + 1];
        let new_in_line = next.line.closest_index_for_x(x);
        let new_offset = next.range.start + new_in_line;
        self.move_to(new_offset.min(next.range.end), cx);
    }

    fn select_left(&mut self, _: &SelectLeft, _: &mut Window, cx: &mut Context<Self>) {
        self.select_to(self.previous_boundary(self.cursor_offset()), cx);
    }

    fn select_right(&mut self, _: &SelectRight, _: &mut Window, cx: &mut Context<Self>) {
        self.select_to(self.next_boundary(self.cursor_offset()), cx);
    }

    fn select_all(&mut self, _: &SelectAll, _: &mut Window, cx: &mut Context<Self>) {
        self.move_to(0, cx);
        self.select_to(self.content.len(), cx);
    }

    fn home(&mut self, _: &Home, _: &mut Window, cx: &mut Context<Self>) {
        if self.mode == InputMode::MultiLine && !self.last_layout.is_empty() {
            let cursor = self.cursor_offset();
            let line_idx = self.line_index_for_offset(cursor);
            self.move_to(self.last_layout[line_idx].range.start, cx);
        } else {
            self.move_to(0, cx);
        }
    }

    fn end(&mut self, _: &End, _: &mut Window, cx: &mut Context<Self>) {
        if self.mode == InputMode::MultiLine && !self.last_layout.is_empty() {
            let cursor = self.cursor_offset();
            let line_idx = self.line_index_for_offset(cursor);
            self.move_to(self.last_layout[line_idx].range.end, cx);
        } else {
            self.move_to(self.content.len(), cx);
        }
    }

    fn backspace(&mut self, _: &Backspace, window: &mut Window, cx: &mut Context<Self>) {
        if self.selected_range.is_empty() {
            let prev = self.previous_boundary(self.cursor_offset());
            if self.cursor_offset() == prev {
                window.play_system_bell();
                return;
            }
            self.select_to(prev, cx);
        }
        self.replace_text_in_range(None, "", window, cx);
    }

    fn delete(&mut self, _: &Delete, window: &mut Window, cx: &mut Context<Self>) {
        if self.selected_range.is_empty() {
            let next = self.next_boundary(self.cursor_offset());
            if self.cursor_offset() == next {
                window.play_system_bell();
                return;
            }
            self.select_to(next, cx);
        }
        self.replace_text_in_range(None, "", window, cx);
    }

    fn newline(&mut self, _: &Newline, window: &mut Window, cx: &mut Context<Self>) {
        if self.mode == InputMode::SingleLine {
            // Parents that want submit-on-enter can listen via key bindings
            // outside the input itself.
            return;
        }
        self.replace_text_in_range(None, "\n", window, cx);
    }

    fn paste(&mut self, _: &Paste, window: &mut Window, cx: &mut Context<Self>) {
        let Some(text) = cx.read_from_clipboard().and_then(|c| c.text()) else {
            return;
        };
        let to_insert = if self.mode == InputMode::SingleLine {
            text.replace('\n', " ")
        } else {
            text
        };
        self.replace_text_in_range(None, &to_insert, window, cx);
    }

    fn copy(&mut self, _: &Copy, _: &mut Window, cx: &mut Context<Self>) {
        if self.selected_range.is_empty() {
            return;
        }
        cx.write_to_clipboard(ClipboardItem::new_string(
            self.content[self.selected_range.clone()].to_string(),
        ));
    }

    fn cut(&mut self, _: &Cut, window: &mut Window, cx: &mut Context<Self>) {
        if self.selected_range.is_empty() {
            return;
        }
        cx.write_to_clipboard(ClipboardItem::new_string(
            self.content[self.selected_range.clone()].to_string(),
        ));
        self.replace_text_in_range(None, "", window, cx);
    }

    fn show_character_palette(
        &mut self,
        _: &ShowCharacterPalette,
        window: &mut Window,
        _: &mut Context<Self>,
    ) {
        window.show_character_palette();
    }

    fn on_mouse_down(
        &mut self,
        event: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.is_selecting = true;
        let offset = self.index_for_mouse_position(event.position);
        if event.modifiers.shift {
            self.select_to(offset, cx);
        } else {
            self.move_to(offset, cx);
        }
        window.focus(&self.focus_handle, cx);
    }

    fn on_mouse_up(&mut self, _: &MouseUpEvent, _: &mut Window, _: &mut Context<Self>) {
        self.is_selecting = false;
    }

    fn on_mouse_move(&mut self, event: &MouseMoveEvent, _: &mut Window, cx: &mut Context<Self>) {
        if self.is_selecting {
            let offset = self.index_for_mouse_position(event.position);
            self.select_to(offset, cx);
        }
    }

    fn index_for_mouse_position(&self, position: Point<Pixels>) -> usize {
        let (Some(bounds), false) = (self.last_bounds.as_ref(), self.last_layout.is_empty()) else {
            return 0;
        };
        if self.content.is_empty() {
            return 0;
        }
        if position.y < bounds.top() {
            return 0;
        }
        if position.y > bounds.bottom() {
            return self.content.len();
        }
        let rel_y = f32::from(position.y - bounds.top());
        let total_h = f32::from(bounds.bottom() - bounds.top());
        let line_h = total_h / self.last_layout.len() as f32;
        let line_idx = ((rel_y / line_h).floor() as usize).min(self.last_layout.len() - 1);
        let layout = &self.last_layout[line_idx];
        let in_line = layout.line.closest_index_for_x(position.x - bounds.left());
        (layout.range.start + in_line).min(layout.range.end)
    }

    // ---- UTF-16 conversions (IME) --------------------------------------

    fn offset_from_utf16(&self, offset: usize) -> usize {
        let mut utf8_offset = 0;
        let mut utf16_count = 0;
        for ch in self.content.chars() {
            if utf16_count >= offset {
                break;
            }
            utf16_count += ch.len_utf16();
            utf8_offset += ch.len_utf8();
        }
        utf8_offset
    }

    fn offset_to_utf16(&self, offset: usize) -> usize {
        let mut utf16_offset = 0;
        let mut utf8_count = 0;
        for ch in self.content.chars() {
            if utf8_count >= offset {
                break;
            }
            utf8_count += ch.len_utf8();
            utf16_offset += ch.len_utf16();
        }
        utf16_offset
    }

    fn range_to_utf16(&self, range: &Range<usize>) -> Range<usize> {
        self.offset_to_utf16(range.start)..self.offset_to_utf16(range.end)
    }

    fn range_from_utf16(&self, range_utf16: &Range<usize>) -> Range<usize> {
        self.offset_from_utf16(range_utf16.start)..self.offset_from_utf16(range_utf16.end)
    }
}

impl EntityInputHandler for TextInput {
    fn text_for_range(
        &mut self,
        range_utf16: Range<usize>,
        actual_range: &mut Option<Range<usize>>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<String> {
        let range = self.range_from_utf16(&range_utf16);
        actual_range.replace(self.range_to_utf16(&range));
        Some(self.content[range].to_string())
    }

    fn selected_text_range(
        &mut self,
        _: bool,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<UTF16Selection> {
        Some(UTF16Selection {
            range: self.range_to_utf16(&self.selected_range),
            reversed: self.selection_reversed,
        })
    }

    fn marked_text_range(&self, _: &mut Window, _: &mut Context<Self>) -> Option<Range<usize>> {
        self.marked_range
            .as_ref()
            .map(|range| self.range_to_utf16(range))
    }

    fn unmark_text(&mut self, _: &mut Window, _: &mut Context<Self>) {
        self.marked_range = None;
    }

    fn replace_text_in_range(
        &mut self,
        range_utf16: Option<Range<usize>>,
        new_text: &str,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let range = range_utf16
            .as_ref()
            .map(|r| self.range_from_utf16(r))
            .or(self.marked_range.clone())
            .unwrap_or(self.selected_range.clone());

        let mut new = String::with_capacity(self.content.len() - (range.end - range.start) + new_text.len());
        new.push_str(&self.content[..range.start]);
        new.push_str(new_text);
        new.push_str(&self.content[range.end..]);
        self.content = SharedString::from(new);
        self.selected_range = range.start + new_text.len()..range.start + new_text.len();
        self.marked_range.take();
        cx.notify();
    }

    fn replace_and_mark_text_in_range(
        &mut self,
        range_utf16: Option<Range<usize>>,
        new_text: &str,
        new_selected_range_utf16: Option<Range<usize>>,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let range = range_utf16
            .as_ref()
            .map(|r| self.range_from_utf16(r))
            .or(self.marked_range.clone())
            .unwrap_or(self.selected_range.clone());

        let mut new = String::with_capacity(self.content.len() - (range.end - range.start) + new_text.len());
        new.push_str(&self.content[..range.start]);
        new.push_str(new_text);
        new.push_str(&self.content[range.end..]);
        self.content = SharedString::from(new);

        if !new_text.is_empty() {
            self.marked_range = Some(range.start..range.start + new_text.len());
        } else {
            self.marked_range = None;
        }
        self.selected_range = new_selected_range_utf16
            .as_ref()
            .map(|r| self.range_from_utf16(r))
            .map(|nr| nr.start + range.start..nr.end + range.start)
            .unwrap_or_else(|| range.start + new_text.len()..range.start + new_text.len());

        cx.notify();
    }

    fn bounds_for_range(
        &mut self,
        range_utf16: Range<usize>,
        bounds: Bounds<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        if self.last_layout.is_empty() {
            return None;
        }
        let range = self.range_from_utf16(&range_utf16);
        let line_idx = self.line_index_for_offset(range.start);
        let layout = &self.last_layout[line_idx];
        let in_line_start = range.start.saturating_sub(layout.range.start);
        let in_line_end = (range.end.min(layout.range.end)).saturating_sub(layout.range.start);
        let line_h = (bounds.bottom() - bounds.top()) / self.last_layout.len() as f32;
        let top = bounds.top() + line_h * (line_idx as f32);
        let bottom = top + line_h;
        Some(Bounds::from_corners(
            point(bounds.left() + layout.line.x_for_index(in_line_start), top),
            point(bounds.left() + layout.line.x_for_index(in_line_end), bottom),
        ))
    }

    fn character_index_for_point(
        &mut self,
        position: gpui::Point<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<usize> {
        let offset = self.index_for_mouse_position(position);
        Some(self.offset_to_utf16(offset))
    }
}

// ---- Custom element doing the actual shaping/painting ------------------

struct TextElement {
    input: Entity<TextInput>,
}

struct PrepaintState {
    lines: Vec<LineLayout>,
    cursor: Option<PaintQuad>,
    selections: Vec<PaintQuad>,
}

impl IntoElement for TextElement {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl Element for TextElement {
    type RequestLayoutState = usize; // number of lines (so paint can match prepaint)
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
        _: Option<&gpui::InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        let input = self.input.read(cx);
        let line_count = if input.content.is_empty() {
            1
        } else {
            input.content.split('\n').count()
        };
        let mut style = Style::default();
        style.size.width = gpui::relative(1.0).into();
        style.size.height = (window.line_height() * line_count as f32).into();
        (window.request_layout(style, [], cx), line_count)
    }

    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&gpui::InspectorElementId>,
        bounds: Bounds<Pixels>,
        _: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) -> Self::PrepaintState {
        let input = self.input.read(cx);
        let content = input.content.clone();
        let selected_range = input.selected_range.clone();
        let cursor_offset = input.cursor_offset();
        let placeholder = input.placeholder.clone();
        let marked_range = input.marked_range.clone();
        let style = window.text_style();
        let line_height = window.line_height();
        let font_size = style.font_size.to_pixels(window.rem_size());

        let (display_text, text_color, use_placeholder) = if content.is_empty() {
            (placeholder, hsla_muted(), true)
        } else {
            (content.clone(), style.color, false)
        };

        // Shape every line. Lines own their byte ranges so we can map
        // global offsets back to (line_index, in_line_offset).
        let mut layouts: Vec<LineLayout> = Vec::new();
        let mut cursor_so_far = 0usize;
        for (line_idx, line_text) in display_text.split('\n').enumerate() {
            if line_idx > 0 {
                cursor_so_far += 1; // for the '\n'
            }
            let start = cursor_so_far;
            let end = cursor_so_far + line_text.len();
            cursor_so_far = end;

            let runs = build_runs(line_text, text_color, &style, marked_range.as_ref(), start);
            let shaped = window.text_system().shape_line(
                SharedString::from(line_text.to_string()),
                font_size,
                &runs,
                None,
            );
            layouts.push(LineLayout {
                range: start..end,
                line: shaped,
            });
        }

        // Build cursor + selection quads.
        let mut selections = Vec::new();
        // Draw the caret even when the placeholder is showing — an empty
        // focused input still needs a visible insertion point. Selection
        // overlay only makes sense for real content.
        if !use_placeholder && !selected_range.is_empty() {
            for (idx, ll) in layouts.iter().enumerate() {
                let line_top = bounds.top() + line_height * (idx as f32);
                let line_bottom = line_top + line_height;
                let start = selected_range.start.max(ll.range.start);
                let end = selected_range.end.min(ll.range.end);
                if start >= end {
                    continue;
                }
                let x_start = bounds.left() + ll.line.x_for_index(start - ll.range.start);
                let x_end = bounds.left() + ll.line.x_for_index(end - ll.range.start);
                selections.push(fill(
                    Bounds::from_corners(point(x_start, line_top), point(x_end, line_bottom)),
                    selection_color(),
                ));
            }
        }

        let cursor = {
            // Cursor caret. For empty / placeholder content the layout
            // is still a single empty line, so position 0 lands at
            // bounds.left() + 0.
            let (line_idx, in_line) = locate(&layouts, cursor_offset);
            let line_top = bounds.top() + line_height * (line_idx as f32);
            let line_bottom = line_top + line_height;
            let layout = &layouts[line_idx];
            let cursor_x = if use_placeholder {
                bounds.left()
            } else {
                bounds.left() + layout.line.x_for_index(in_line)
            };
            Some(fill(
                Bounds::new(
                    point(cursor_x, line_top),
                    gpui::size(px(2.0), line_bottom - line_top),
                ),
                cursor_color(),
            ))
        };

        PrepaintState {
            lines: layouts,
            cursor,
            selections,
        }
    }

    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&gpui::InspectorElementId>,
        bounds: Bounds<Pixels>,
        _: &mut Self::RequestLayoutState,
        prepaint: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        let focus_handle = self.input.read(cx).focus_handle.clone();
        window.handle_input(
            &focus_handle,
            ElementInputHandler::new(bounds, self.input.clone()),
            cx,
        );

        for sel in prepaint.selections.drain(..) {
            window.paint_quad(sel);
        }

        let line_height = window.line_height();
        let mut taken_lines = std::mem::take(&mut prepaint.lines);
        for (idx, layout) in taken_lines.iter().enumerate() {
            let origin = point(bounds.left(), bounds.top() + line_height * (idx as f32));
            let _ = layout
                .line
                .paint(origin, line_height, gpui::TextAlign::Left, None, window, cx);
        }

        if focus_handle.is_focused(window) {
            if let Some(cursor) = prepaint.cursor.take() {
                window.paint_quad(cursor);
            }
        }

        // Stash layout for click/cursor lookups on the next interaction.
        self.input.update(cx, |input, _| {
            input.last_layout = taken_lines.drain(..).collect();
            input.last_bounds = Some(bounds);
        });
    }
}

fn build_runs(
    text: &str,
    color: gpui::Hsla,
    style: &gpui::TextStyle,
    marked_range: Option<&Range<usize>>,
    line_start_offset: usize,
) -> Vec<TextRun> {
    let base = TextRun {
        len: text.len(),
        font: style.font(),
        color,
        background_color: None,
        underline: None,
        strikethrough: None,
    };

    let Some(marked) = marked_range else {
        return vec![base];
    };

    let line_end = line_start_offset + text.len();
    if marked.start >= line_end || marked.end <= line_start_offset {
        return vec![base];
    }

    let local_start = marked.start.saturating_sub(line_start_offset);
    let local_end = (marked.end - line_start_offset).min(text.len());

    let mut runs = Vec::with_capacity(3);
    if local_start > 0 {
        runs.push(TextRun {
            len: local_start,
            ..base.clone()
        });
    }
    if local_end > local_start {
        runs.push(TextRun {
            len: local_end - local_start,
            underline: Some(UnderlineStyle {
                color: Some(color),
                thickness: px(1.0),
                wavy: false,
            }),
            ..base.clone()
        });
    }
    if local_end < text.len() {
        runs.push(TextRun {
            len: text.len() - local_end,
            ..base
        });
    }
    runs
}

fn locate(layouts: &[LineLayout], offset: usize) -> (usize, usize) {
    for (i, ll) in layouts.iter().enumerate() {
        if offset <= ll.range.end {
            return (i, offset.saturating_sub(ll.range.start));
        }
    }
    let last = layouts.len().saturating_sub(1);
    let layout = layouts.last().unwrap();
    (last, layout.range.end - layout.range.start)
}

fn hsla_muted() -> gpui::Hsla {
    gpui::hsla(0.0, 0.0, 0.6, 0.6)
}

fn cursor_color() -> gpui::Hsla {
    Theme::TEXT_LINK.into()
}

fn selection_color() -> gpui::Hsla {
    gpui::hsla(0.58, 0.7, 0.5, 0.35)
}

impl Render for TextInput {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let focused = self.focus_handle.is_focused(_window);
        let border_color = if focused {
            Theme::TEXT_LINK
        } else {
            Theme::BORDER
        };
        div()
            .key_context("TextInput")
            .track_focus(&self.focus_handle)
            .cursor(CursorStyle::IBeam)
            .on_action(cx.listener(Self::backspace))
            .on_action(cx.listener(Self::delete))
            .on_action(cx.listener(Self::left))
            .on_action(cx.listener(Self::right))
            .on_action(cx.listener(Self::up))
            .on_action(cx.listener(Self::down))
            .on_action(cx.listener(Self::select_left))
            .on_action(cx.listener(Self::select_right))
            .on_action(cx.listener(Self::select_all))
            .on_action(cx.listener(Self::home))
            .on_action(cx.listener(Self::end))
            .on_action(cx.listener(Self::newline))
            .on_action(cx.listener(Self::paste))
            .on_action(cx.listener(Self::copy))
            .on_action(cx.listener(Self::cut))
            .on_action(cx.listener(Self::show_character_palette))
            .on_mouse_down(MouseButton::Left, cx.listener(Self::on_mouse_down))
            .on_mouse_up(MouseButton::Left, cx.listener(Self::on_mouse_up))
            .on_mouse_up_out(MouseButton::Left, cx.listener(Self::on_mouse_up))
            .on_mouse_move(cx.listener(Self::on_mouse_move))
            .w_full()
            .p_2()
            .bg(Theme::BG)
            .border_1()
            .border_color(border_color)
            .rounded_sm()
            .text_color(Theme::TEXT)
            .text_size(px(12.5))
            .child(TextElement {
                input: cx.entity(),
            })
    }
}

impl Focusable for TextInput {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}
