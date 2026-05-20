//! Intra-line word-level diff. Used to highlight the specific tokens that
//! changed between a delete/add pair, so reviewers can see at a glance
//! which word(s) were actually edited.

use std::ops::Range;

use gpui::{HighlightStyle, Hsla};
use similar::{ChangeTag, TextDiff};

/// Returns `(removed_ranges_in_old, added_ranges_in_new)` byte ranges
/// pointing into each input string respectively.
pub fn word_changes(old: &str, new: &str) -> (Vec<Range<usize>>, Vec<Range<usize>>) {
    let diff = TextDiff::configure().diff_words(old, new);

    let mut old_ranges: Vec<Range<usize>> = Vec::new();
    let mut new_ranges: Vec<Range<usize>> = Vec::new();
    let mut old_pos = 0usize;
    let mut new_pos = 0usize;

    for change in diff.iter_all_changes() {
        let len = change.value().len();
        match change.tag() {
            ChangeTag::Equal => {
                old_pos += len;
                new_pos += len;
            }
            ChangeTag::Delete => {
                merge_or_push(&mut old_ranges, old_pos..old_pos + len);
                old_pos += len;
            }
            ChangeTag::Insert => {
                merge_or_push(&mut new_ranges, new_pos..new_pos + len);
                new_pos += len;
            }
        }
    }

    (old_ranges, new_ranges)
}

fn merge_or_push(out: &mut Vec<Range<usize>>, r: Range<usize>) {
    if let Some(last) = out.last_mut() {
        if last.end == r.start {
            last.end = r.end;
            return;
        }
    }
    out.push(r);
}

pub fn word_highlight(bg: Hsla) -> HighlightStyle {
    HighlightStyle {
        background_color: Some(bg),
        ..HighlightStyle::default()
    }
}
