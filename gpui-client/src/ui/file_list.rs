//! Tree-style sidebar matching the React FileList component.
//!
//! Files are grouped by their path segments into a hierarchy of
//! directories. Each directory row collapses on click; each file row
//! shows status badge + viewed checkbox + comment counter and scrolls
//! the main pane to the file on click.

use std::collections::HashSet;

use gpui::{
    div, prelude::*, px, App, ElementId, Entity, IntoElement, ParentElement, SharedString, Styled,
};

use crate::api::types::{DiffFile, FileStatus};
use crate::ui::text_input::TextInput;
use crate::ui::theme::{Theme, UI_FONT};
use crate::ui::widgets::icon;

#[allow(clippy::too_many_arguments)]
pub fn render_file_list(
    files: &[DiffFile],
    selected: Option<usize>,
    viewed: &HashSet<String>,
    collapsed_files: &HashSet<String>,
    collapsed_dirs: &HashSet<String>,
    filter_input: Option<Entity<TextInput>>,
    filter_text: &str,
    on_select: impl Fn(usize, &mut App) + 'static + Clone,
    on_toggle_viewed: impl Fn(usize, &mut App) + 'static + Clone,
    on_toggle_collapsed: impl Fn(usize, &mut App) + 'static + Clone,
    on_toggle_dir: impl Fn(String, &mut App) + 'static + Clone,
) -> impl IntoElement {
    let total = files.len();
    let filter_lc = filter_text.to_ascii_lowercase();
    let matches: Vec<(usize, &DiffFile)> = files
        .iter()
        .enumerate()
        .filter(|(_, f)| filter_lc.is_empty() || f.path.to_ascii_lowercase().contains(&filter_lc))
        .collect();

    let viewed_count = matches
        .iter()
        .filter(|(_, f)| viewed.contains(&f.path))
        .count();

    let tree = build_tree(&matches);

    let mut rows: Vec<gpui::AnyElement> = Vec::new();
    render_nodes(
        &tree,
        0,
        collapsed_dirs,
        viewed,
        collapsed_files,
        selected,
        &on_select,
        &on_toggle_viewed,
        &on_toggle_collapsed,
        &on_toggle_dir,
        &mut rows,
    );

    div()
        .w(px(280.0))
        .h_full()
        .flex()
        .flex_col()
        .bg(Theme::BG_ELEVATED)
        .border_r_1()
        .border_color(Theme::BORDER)
        .font_family(UI_FONT)
        .text_color(Theme::TEXT)
        .child(sidebar_header(total, viewed_count, filter_input))
        .child(
            div()
                .id("file-list-scroll")
                .flex_1()
                .min_h_0()
                .overflow_y_scroll()
                .children(rows),
        )
}

fn sidebar_header(
    total: usize,
    viewed: usize,
    filter_input: Option<Entity<TextInput>>,
) -> impl IntoElement {
    let mut header = div()
        .px_3()
        .py_2()
        .border_b_1()
        .border_color(Theme::BORDER)
        .flex()
        .flex_col()
        .gap_2()
        .child(
            div()
                .flex()
                .flex_row()
                .items_center()
                .gap_2()
                .child(
                    div()
                        .flex_1()
                        .text_color(Theme::TEXT)
                        .text_size(px(12.0))
                        .child(SharedString::from(format!("Files changed ({})", total))),
                )
                .child(
                    div()
                        .text_color(Theme::TEXT_MUTED)
                        .text_size(px(11.0))
                        .child(SharedString::from(format!("{viewed}/{total} viewed"))),
                ),
        );

    if let Some(input) = filter_input {
        header = header.child(div().w_full().child(input));
    }

    header
}

#[derive(Clone)]
enum Node {
    Dir {
        name: String,
        path: String,
        children: Vec<Node>,
    },
    File {
        name: String,
        file_idx: usize,
        status: FileStatus,
        additions: u32,
        deletions: u32,
        path: String,
    },
}

fn build_tree<'a>(files: &[(usize, &'a DiffFile)]) -> Vec<Node> {
    let mut roots: Vec<Node> = Vec::new();
    for (idx, file) in files {
        let parts: Vec<&str> = file.path.split('/').collect();
        insert_path(&mut roots, &parts, *idx, file, "");
    }
    sort_nodes(&mut roots);
    roots
}

fn insert_path(
    siblings: &mut Vec<Node>,
    parts: &[&str],
    file_idx: usize,
    file: &DiffFile,
    parent_prefix: &str,
) {
    if parts.is_empty() {
        return;
    }
    if parts.len() == 1 {
        siblings.push(Node::File {
            name: parts[0].to_string(),
            file_idx,
            status: file.status.clone(),
            additions: file.additions,
            deletions: file.deletions,
            path: file.path.clone(),
        });
        return;
    }
    let head = parts[0];
    let new_prefix = if parent_prefix.is_empty() {
        head.to_string()
    } else {
        format!("{parent_prefix}/{head}")
    };
    if let Some(Node::Dir { children, .. }) =
        siblings.iter_mut().find(|n| match n {
            Node::Dir { name, .. } => name == head,
            _ => false,
        })
    {
        insert_path(children, &parts[1..], file_idx, file, &new_prefix);
        return;
    }
    let mut children = Vec::new();
    insert_path(&mut children, &parts[1..], file_idx, file, &new_prefix);
    siblings.push(Node::Dir {
        name: head.to_string(),
        path: new_prefix,
        children,
    });
}

fn sort_nodes(nodes: &mut [Node]) {
    nodes.sort_by(|a, b| {
        let key = |n: &Node| match n {
            Node::Dir { name, .. } => (0u8, name.clone()),
            Node::File { name, .. } => (1u8, name.clone()),
        };
        key(a).cmp(&key(b))
    });
    for n in nodes {
        if let Node::Dir { children, .. } = n {
            sort_nodes(children);
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn render_nodes(
    nodes: &[Node],
    depth: usize,
    collapsed_dirs: &HashSet<String>,
    viewed: &HashSet<String>,
    collapsed_files: &HashSet<String>,
    selected: Option<usize>,
    on_select: &(impl Fn(usize, &mut App) + 'static + Clone),
    on_toggle_viewed: &(impl Fn(usize, &mut App) + 'static + Clone),
    on_toggle_collapsed: &(impl Fn(usize, &mut App) + 'static + Clone),
    on_toggle_dir: &(impl Fn(String, &mut App) + 'static + Clone),
    out: &mut Vec<gpui::AnyElement>,
) {
    for node in nodes {
        match node {
            Node::Dir { name, path, children } => {
                let open = !collapsed_dirs.contains(path);
                let chevron_name = if open { "chevron-down" } else { "chevron-right" };
                let folder_name = if open { "folder-open" } else { "folder" };
                let path_owned = path.clone();
                let cb = on_toggle_dir.clone();
                out.push(
                    div()
                        .id(ElementId::Name(SharedString::from(format!("dir-{path}"))))
                        .px_3()
                        .py_1()
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap_2()
                        .pl(px(depth as f32 * 12.0 + 8.0))
                        .cursor_pointer()
                        .hover(|s| s.bg(Theme::BG_HOVER))
                        .on_click(move |_e, _w, cx| cb(path_owned.clone(), cx))
                        .child(icon(chevron_name, 12.0, Theme::TEXT_MUTED))
                        .child(icon(folder_name, 14.0, Theme::TEXT_MUTED))
                        .child(
                            div()
                                .text_color(Theme::TEXT_MUTED)
                                .text_size(px(12.5))
                                .child(SharedString::from(name.clone())),
                        )
                        .into_any_element(),
                );
                if open {
                    render_nodes(
                        children,
                        depth + 1,
                        collapsed_dirs,
                        viewed,
                        collapsed_files,
                        selected,
                        on_select,
                        on_toggle_viewed,
                        on_toggle_collapsed,
                        on_toggle_dir,
                        out,
                    );
                }
            }
            Node::File {
                name,
                file_idx,
                status,
                additions,
                deletions,
                path,
            } => {
                let is_viewed = viewed.contains(path);
                let is_selected = selected == Some(*file_idx);
                let is_collapsed = collapsed_files.contains(path);
                out.push(
                    file_row(
                        *file_idx,
                        name,
                        path,
                        status,
                        *additions,
                        *deletions,
                        is_viewed,
                        is_selected,
                        is_collapsed,
                        depth,
                        on_select.clone(),
                        on_toggle_viewed.clone(),
                        on_toggle_collapsed.clone(),
                    )
                    .into_any_element(),
                );
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn file_row(
    file_idx: usize,
    name: &str,
    _path: &str,
    status: &FileStatus,
    additions: u32,
    deletions: u32,
    viewed: bool,
    selected: bool,
    collapsed: bool,
    depth: usize,
    on_select: impl Fn(usize, &mut App) + 'static,
    on_toggle_viewed: impl Fn(usize, &mut App) + 'static,
    on_toggle_collapsed: impl Fn(usize, &mut App) + 'static,
) -> impl IntoElement {
    let bg = if selected {
        Theme::BG_SELECTED
    } else {
        Theme::BG_ELEVATED
    };
    let id: ElementId = ElementId::Integer(file_idx as u64);
    let toggle_v_id = ElementId::Name(SharedString::from(format!("file-viewed-{file_idx}")));
    let toggle_c_id = ElementId::Name(SharedString::from(format!("file-collapsed-{file_idx}")));
    let text_color = if viewed { Theme::TEXT_MUTED } else { Theme::TEXT };

    div()
        .id(id)
        .px_2()
        .py_1()
        .flex()
        .flex_row()
        .items_center()
        .gap_2()
        .pl(px(depth as f32 * 12.0 + 4.0))
        .bg(bg)
        .hover(|s| s.bg(Theme::BG_HOVER))
        .border_b_1()
        .border_color(Theme::BORDER)
        .cursor_pointer()
        .on_click(move |_event, _window, cx| on_select(file_idx, cx))
        .child(collapse_chevron(toggle_c_id, collapsed, move |cx| {
            on_toggle_collapsed(file_idx, cx)
        }))
        .child(viewed_checkbox(toggle_v_id, viewed, move |cx| {
            on_toggle_viewed(file_idx, cx)
        }))
        .child(status_badge(status))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .text_size(px(12.5))
                .text_color(text_color)
                .child(SharedString::from(name.to_string())),
        )
        .child(
            div()
                .flex()
                .gap_1()
                .text_size(px(10.0))
                .child(
                    div()
                        .text_color(Theme::FILE_STATUS_ADD)
                        .child(SharedString::from(format!("+{additions}"))),
                )
                .child(
                    div()
                        .text_color(Theme::FILE_STATUS_DEL)
                        .child(SharedString::from(format!("-{deletions}"))),
                ),
        )
}

fn collapse_chevron(
    id: ElementId,
    collapsed: bool,
    on_toggle: impl Fn(&mut App) + 'static,
) -> impl IntoElement {
    let name = if collapsed { "chevron-right" } else { "chevron-down" };
    div()
        .id(id)
        .cursor_pointer()
        .hover(|s| s.opacity(0.7))
        .on_click(move |_e, _w, cx| on_toggle(cx))
        .child(icon(name, 12.0, Theme::TEXT_MUTED))
}

fn viewed_checkbox(
    id: ElementId,
    checked: bool,
    on_toggle: impl Fn(&mut App) + 'static,
) -> impl IntoElement {
    let bg = if checked { Theme::FILE_STATUS_ADD } else { Theme::BG_ELEVATED };
    div()
        .id(id)
        .w(px(16.0))
        .h(px(16.0))
        .flex()
        .items_center()
        .justify_center()
        .bg(bg)
        .border_1()
        .border_color(if checked { Theme::FILE_STATUS_ADD } else { Theme::BORDER })
        .rounded_xs()
        .cursor_pointer()
        .hover(|s| s.bg(Theme::BG_HOVER))
        .on_click(move |_e, _w, cx| on_toggle(cx))
        .child(if checked {
            icon("check", 10.0, Theme::TEXT).into_any_element()
        } else {
            div().into_any_element()
        })
}

fn status_badge(status: &FileStatus) -> impl IntoElement {
    let (icon_name, color) = match status {
        FileStatus::Added => ("file-plus", Theme::FILE_STATUS_ADD),
        FileStatus::Deleted => ("file-x", Theme::FILE_STATUS_DEL),
        FileStatus::Modified => ("file-pen", Theme::FILE_STATUS_MOD),
        FileStatus::Renamed => ("file-diff", Theme::TEXT_LINK),
    };
    icon(icon_name, 14.0, color)
}
