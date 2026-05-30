//! Tree-style sidebar matching the React FileList component.
//!
//! Files are grouped by their path segments into a hierarchy of
//! directories. Each directory row collapses on click; each file row
//! shows status badge + viewed checkbox + comment counter and scrolls
//! the main pane to the file on click.

use std::collections::{HashMap, HashSet};

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
    comment_counts: &HashMap<String, usize>,
    changed_since_viewed: &HashSet<String>,
    filter_input: Option<Entity<TextInput>>,
    filter_text: &str,
    on_select: impl Fn(usize, &mut App) + 'static + Clone,
    on_toggle_viewed: impl Fn(usize, &mut App) + 'static + Clone,
    on_toggle_collapsed: impl Fn(usize, &mut App) + 'static + Clone,
    on_toggle_dir: impl Fn(String, &mut App) + 'static + Clone,
    on_open_shortcuts: impl Fn(&mut App) + 'static,
) -> impl IntoElement {
    let total = files.len();
    let filter_lc = filter_text.to_ascii_lowercase();
    let matches: Vec<(usize, &DiffFile)> = files
        .iter()
        .enumerate()
        .filter(|(_, f)| filter_lc.is_empty() || f.path.to_ascii_lowercase().contains(&filter_lc))
        .collect();

    let total_additions: u32 = files.iter().map(|f| f.additions).sum();
    let total_deletions: u32 = files.iter().map(|f| f.deletions).sum();

    let tree = build_tree(&matches);

    let mut rows: Vec<gpui::AnyElement> = Vec::new();
    render_nodes(
        &tree,
        0,
        collapsed_dirs,
        viewed,
        collapsed_files,
        comment_counts,
        changed_since_viewed,
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
        .font_family(UI_FONT())
        .text_color(Theme::TEXT)
        .child(sidebar_header(
            total,
            total_additions,
            total_deletions,
            filter_input,
        ))
        .child(
            div()
                .id("file-list-scroll")
                .flex_1()
                .min_h_0()
                .overflow_y_scroll()
                .children(rows),
        )
        .child(sidebar_footer(on_open_shortcuts))
}

/// React's bottom-of-sidebar bar: "[Keyboard] Shortcuts" on the left,
/// "Star on GitHub [octocat]" on the right.
fn sidebar_footer(on_open_shortcuts: impl Fn(&mut App) + 'static) -> impl IntoElement {
    use crate::ui::widgets::icon;
    div()
        .w_full()
        .px(px(16.0))
        .py(px(14.0))
        .border_t_1()
        .border_color(Theme::BORDER)
        .flex()
        .flex_row()
        .items_center()
        .justify_between()
        .child(
            div()
                .id("sidebar-shortcuts")
                .flex()
                .flex_row()
                .items_center()
                .gap(px(6.0))
                .text_size(px(13.0))
                .text_color(Theme::TEXT_MUTED)
                .cursor_pointer()
                .hover(|s| s.text_color(Theme::TEXT))
                .on_click(move |_e, _w, cx| on_open_shortcuts(cx))
                .child(icon("keyboard", 16.0, Theme::TEXT_MUTED))
                .child(SharedString::from("Shortcuts")),
        )
        .child(
            div()
                .id("sidebar-github")
                .flex()
                .flex_row()
                .items_center()
                .gap(px(8.0))
                .text_size(px(13.0))
                .text_color(Theme::TEXT_MUTED)
                .cursor_pointer()
                .hover(|s| s.text_color(Theme::TEXT))
                .on_click(|_e, _w, cx| {
                    cx.open_url("https://github.com/yoshiko-pg/difit");
                })
                .child(SharedString::from("Star on GitHub"))
                .child(icon("github", 18.0, Theme::TEXT_MUTED)),
        )
}

fn sidebar_header(
    total: usize,
    total_additions: u32,
    total_deletions: u32,
    filter_input: Option<Entity<TextInput>>,
) -> impl IntoElement {
    use crate::ui::widgets::icon;
    let mut header = div()
        .px(px(16.0))
        .py(px(12.0))
        .border_b_1()
        .border_color(Theme::BORDER)
        .bg(Theme::BG_HOVER)
        .flex()
        .flex_col()
        .gap(px(12.0))
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
                        .text_size(px(13.0))
                        .font_weight(gpui::FontWeight::SEMIBOLD)
                        .child(SharedString::from(format!("Files changed ({})", total))),
                )
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .gap_1()
                        .text_size(px(11.0))
                        .child(
                            div()
                                .text_color(Theme::FILE_STATUS_ADD)
                                .child(SharedString::from(format!("+{total_additions}"))),
                        )
                        .child(
                            div()
                                .text_color(Theme::FILE_STATUS_DEL)
                                .child(SharedString::from(format!("-{total_deletions}"))),
                        ),
                ),
        );

    if let Some(input) = filter_input {
        // React wraps the input with a Search icon absolutely-positioned
        // on the left. Replicate with a relative wrapper.
        let _ = icon;
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
    // Preserve the server's file order (don't sort) so the sidebar and
    // the all-files-stacked main pane stay in lockstep. Then collapse
    // chains of single-child directories the way React does.
    collapse_chains(&mut roots);
    roots
}

fn collapse_chains(nodes: &mut Vec<Node>) {
    for node in nodes.iter_mut() {
        if let Node::Dir { children, .. } = node {
            collapse_chains(children);
        }
    }
    let mut i = 0;
    while i < nodes.len() {
        let (combined_name, child_path, grandchildren) = match &nodes[i] {
            Node::Dir { name, children, .. } if children.len() == 1 => {
                if let Some(Node::Dir {
                    name: cn,
                    path: cp,
                    children: gc,
                }) = children.first()
                {
                    (format!("{name}/{cn}"), cp.clone(), gc.clone())
                } else {
                    i += 1;
                    continue;
                }
            }
            _ => {
                i += 1;
                continue;
            }
        };
        nodes[i] = Node::Dir {
            name: combined_name,
            path: child_path,
            children: grandchildren,
        };
        // Stay on the same index so we keep collapsing further chains.
    }
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


#[allow(clippy::too_many_arguments)]
fn render_nodes(
    nodes: &[Node],
    depth: usize,
    collapsed_dirs: &HashSet<String>,
    viewed: &HashSet<String>,
    collapsed_files: &HashSet<String>,
    comment_counts: &HashMap<String, usize>,
    changed_since_viewed: &HashSet<String>,
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
                        .h(px(36.0))
                        .px(px(16.0))
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap_2()
                        .pl(px(depth as f32 * 12.0 + 16.0))
                        .bg(Theme::BG_ELEVATED)
                        .cursor_pointer()
                        .hover(|s| s.bg(Theme::BG_HOVER))
                        .on_click(move |_e, _w, cx| cb(path_owned.clone(), cx))
                        .child(icon(chevron_name, 16.0, Theme::TEXT_MUTED))
                        .child(icon(folder_name, 16.0, Theme::TEXT_MUTED))
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .overflow_hidden()
                                .whitespace_nowrap()
                                .text_color(Theme::TEXT)
                                .text_size(px(13.0))
                                .font_weight(gpui::FontWeight::MEDIUM)
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
                        comment_counts,
                        changed_since_viewed,
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
                let comment_count = comment_counts.get(path).copied().unwrap_or(0);
                let is_changed = changed_since_viewed.contains(path);
                out.push(
                    file_row(
                        *file_idx,
                        name,
                        path,
                        status,
                        *additions,
                        *deletions,
                        comment_count,
                        is_viewed,
                        is_changed,
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
    comment_count: usize,
    viewed: bool,
    changed_since_viewed: bool,
    selected: bool,
    collapsed: bool,
    depth: usize,
    on_select: impl Fn(usize, &mut App) + 'static,
    on_toggle_viewed: impl Fn(usize, &mut App) + 'static,
    on_toggle_collapsed: impl Fn(usize, &mut App) + 'static,
) -> impl IntoElement {
    // React uses the same `bg-github-bg-tertiary` for selected and
    // hover, and there's no border between file rows. Reviewed files
    // drop to opacity-70 and the filename gets a strikethrough.
    let bg = if selected {
        Theme::BG_HOVER
    } else {
        Theme::BG_ELEVATED
    };
    let id: ElementId = ElementId::Integer(file_idx as u64);
    let toggle_v_id = ElementId::Name(SharedString::from(format!("file-viewed-{file_idx}")));
    let toggle_c_id = ElementId::Name(SharedString::from(format!("file-collapsed-{file_idx}")));
    let text_color = if viewed { Theme::TEXT_MUTED } else { Theme::TEXT };
    let mut name_text = div()
        .flex_1()
        .min_w_0()
        .overflow_hidden()
        .whitespace_nowrap()
        .text_size(px(13.0))
        .text_color(text_color)
        .child(SharedString::from(name.to_string()));
    if viewed {
        name_text = name_text.line_through();
    }
    let row_opacity = if viewed { 0.7 } else { 1.0 };

    div()
        .id(id)
        .px(px(16.0))
        .py(px(6.0))
        .flex()
        .flex_row()
        .items_center()
        .gap_2()
        .pl(px(depth as f32 * 12.0 + 8.0))
        .bg(bg)
        .opacity(row_opacity)
        .hover(|s| s.bg(Theme::BG_HOVER))
        .cursor_pointer()
        .on_click(move |_event, _window, cx| on_select(file_idx, cx))
        .child(collapse_chevron(toggle_c_id, collapsed, move |cx| {
            on_toggle_collapsed(file_idx, cx)
        }))
        .child(viewed_checkbox(toggle_v_id, viewed, move |cx| {
            on_toggle_viewed(file_idx, cx)
        }))
        .child(status_badge(status))
        .child(name_text)
        .child(changed_badge(changed_since_viewed))
        .child(comment_count_badge(comment_count))
        .child(
            div()
                .flex()
                .flex_shrink_0()
                .gap_1()
                .text_size(px(10.0))
                .child(
                    div()
                        .whitespace_nowrap()
                        .text_color(Theme::FILE_STATUS_ADD)
                        .child(SharedString::from(format!("+{additions}"))),
                )
                .child(
                    div()
                        .whitespace_nowrap()
                        .text_color(Theme::FILE_STATUS_DEL)
                        .child(SharedString::from(format!("-{deletions}"))),
                ),
        )
}

fn changed_badge(show: bool) -> impl IntoElement {
    let mut wrap = div().flex().flex_shrink_0().items_center();
    if show {
        wrap = wrap
            .px(px(6.0))
            .py(px(1.0))
            .rounded_full()
            .bg(Theme::FILE_STATUS_MOD)
            .text_color(Theme::BG)
            .text_size(px(10.0))
            .font_weight(gpui::FontWeight::SEMIBOLD)
            .child(SharedString::from("Changed"));
    }
    wrap
}

fn comment_count_badge(count: usize) -> impl IntoElement {
    let mut wrap = div()
        .flex()
        .flex_shrink_0()
        .items_center()
        .gap_1()
        .text_size(px(12.0));
    if count > 0 {
        wrap = wrap
            .text_color(Theme::FILE_STATUS_MOD)
            .child(icon("message-square", 12.0, Theme::FILE_STATUS_MOD))
            .child(SharedString::from(count.to_string()));
    }
    wrap
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
