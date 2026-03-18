use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

use axe_tree::icons::{self, FileIcon};
use axe_tree::{FileTree, NodeKind, TreeAction};

use crate::editor_panel::render_scrollbar;
use crate::theme::Theme;

/// Indentation width per nesting level in the file tree.
const TREE_INDENT: usize = 2;
/// Prefix for collapsed directories.
const DIR_COLLAPSED_PREFIX: &str = "\u{25B8} ";
/// Prefix for expanded directories.
const DIR_EXPANDED_PREFIX: &str = "\u{25BE} ";
/// Prefix for files (space for alignment with directory arrows).
const FILE_PREFIX: &str = "  ";

/// Renders an inline input line for tree actions (create/rename).
fn render_inline_input_line(
    indent: &str,
    input: &str,
    area_width: usize,
    theme: &Theme,
) -> Line<'static> {
    let text = format!("{indent}  > {input}|");
    let padded = format!("{:<width$}", text, width = area_width);
    let style = Style::default()
        .fg(theme.panel_border_active)
        .bg(theme.tree_selection_bg);
    Line::from(Span::styled(padded, style))
}

// IMPACT ANALYSIS — icon_for_node
// Parents: build_icon_line() uses this to pick the icon for each tree node.
// Children: None — purely visual.
// Siblings: icon_for_file() in axe-tree::icons — delegates to it for file nodes.

/// Returns the icon for a tree node when icons are enabled.
fn icon_for_node(node: &axe_tree::TreeNode) -> FileIcon {
    match &node.kind {
        NodeKind::Directory { .. } => {
            if node.expanded {
                icons::DIR_OPEN_ICON
            } else {
                icons::DIR_CLOSED_ICON
            }
        }
        NodeKind::Symlink { .. } => icons::SYMLINK_ICON,
        NodeKind::File { .. } => icons::icon_for_file(&node.name),
    }
}

/// Builds a multi-span tree line with icon when icons are enabled.
#[allow(clippy::too_many_arguments)]
fn build_icon_line(
    node: &axe_tree::TreeNode,
    indent: &str,
    display_name: &str,
    name_style: Style,
    is_selected: bool,
    area_width: usize,
    scroll_col: usize,
    theme: &Theme,
) -> Line<'static> {
    let icon = if node.depth == 0 {
        icons::DIR_OPEN_ICON
    } else {
        icon_for_node(node)
    };

    // Build the full logical line, then apply horizontal scroll.
    let full_text = format!("{}{}{}", indent, icon.icon, display_name);
    let visible: String = full_text.chars().skip(scroll_col).collect();
    let padded = format!("{:<width$}", visible, width = area_width);

    // Reconstruct styled spans from the scrolled view.
    let indent_chars = indent.chars().count();
    let icon_chars = icon.icon.chars().count();

    // How many chars of each section remain after scroll.
    let indent_visible = indent_chars.saturating_sub(scroll_col);
    let icon_start = indent_chars.saturating_sub(scroll_col).min(area_width);
    let icon_visible = if scroll_col > indent_chars {
        icon_chars.saturating_sub(scroll_col - indent_chars)
    } else {
        icon_chars
    }
    .min(area_width.saturating_sub(icon_start));
    let name_start = icon_start + icon_visible;

    let mut icon_style = Style::default().fg(icon.color);
    if is_selected {
        icon_style = icon_style.bg(theme.tree_selection_bg);
    }

    // Split padded string into three styled spans.
    let chars: Vec<char> = padded.chars().collect();
    let indent_str: String = chars[..indent_visible.min(chars.len())].iter().collect();
    let icon_str: String = chars[icon_start..name_start.min(chars.len())]
        .iter()
        .collect();
    let name_str: String = chars[name_start.min(chars.len())..].iter().collect();

    Line::from(vec![
        Span::styled(indent_str, name_style),
        Span::styled(icon_str, icon_style),
        Span::styled(name_str, name_style),
    ])
}

/// Builds a plain-text tree line without icons.
fn build_plain_line(
    node: &axe_tree::TreeNode,
    indent: &str,
    display_name: &str,
    name_style: Style,
    area_width: usize,
    scroll_col: usize,
) -> Line<'static> {
    let text = if node.depth == 0 {
        format!("{indent}{display_name}")
    } else {
        let prefix = match &node.kind {
            NodeKind::Directory { .. } => {
                if node.expanded {
                    DIR_EXPANDED_PREFIX
                } else {
                    DIR_COLLAPSED_PREFIX
                }
            }
            NodeKind::File { .. } | NodeKind::Symlink { .. } => FILE_PREFIX,
        };
        format!("{indent}{prefix}{display_name}")
    };
    let visible: String = text.chars().skip(scroll_col).collect();
    let padded = format!("{:<width$}", visible, width = area_width);
    Line::from(Span::styled(padded, name_style))
}

// IMPACT ANALYSIS — render_tree_content
// Parents: render() calls this for tree panel content (normal and zoomed views).
// Children: build_icon_line(), build_plain_line(), render_inline_input_line().
// Siblings: Tree actions (create/rename) inject inline input lines;
//           delete confirmation is handled by the unified confirm dialog overlay.
//           show_icons toggle changes rendering path.

/// Renders file tree content into the given area, with selection highlight and scrolling.
pub(crate) fn render_tree_content(
    file_tree: &FileTree,
    area: Rect,
    frame: &mut Frame,
    theme: &Theme,
    modified_files: &std::collections::HashSet<std::path::PathBuf>,
) {
    let nodes = file_tree.visible_nodes();
    let scroll = file_tree.scroll();
    let scroll_col = file_tree.scroll_col();
    let selected = file_tree.selected();
    let visible_count = area.height as usize;
    let action = file_tree.action();
    let use_icons = file_tree.show_icons();

    // Reserve 1 column for scrollbar when content overflows.
    let needs_scrollbar = nodes.len() > visible_count;
    let (content_area, scrollbar_area) = if needs_scrollbar && area.width > 1 {
        let content = Rect::new(area.x, area.y, area.width - 1, area.height);
        let scrollbar = Rect::new(area.x + area.width - 1, area.y, 1, area.height);
        (content, Some(scrollbar))
    } else {
        (area, None)
    };

    let area_width = content_area.width as usize;
    let mut lines: Vec<Line> = Vec::with_capacity(visible_count);

    for (i, node) in nodes.iter().enumerate().skip(scroll) {
        if lines.len() >= visible_count {
            break;
        }

        let indent = " ".repeat(TREE_INDENT * node.depth);
        let is_selected = i == selected;

        let display_name = if let TreeAction::Renaming { node_idx, input } = action {
            if i == *node_idx {
                format!("{input}|")
            } else {
                node.name.clone()
            }
        } else {
            node.name.clone()
        };

        let mut name_style = if node.depth == 0 {
            Style::default()
                .fg(theme.foreground)
                .add_modifier(Modifier::BOLD)
        } else {
            match &node.kind {
                NodeKind::Directory { .. } => Style::default().fg(theme.panel_border_active),
                NodeKind::File { .. } | NodeKind::Symlink { .. } => {
                    Style::default().fg(theme.foreground)
                }
            }
        };

        // Tint files with uncommitted changes (orange).
        if node.depth > 0 {
            let is_modified = match &node.kind {
                NodeKind::File { .. } | NodeKind::Symlink { .. } => {
                    modified_files.contains(&node.path)
                }
                NodeKind::Directory { .. } => false,
            };
            if is_modified {
                name_style = name_style.fg(theme.tree_modified_fg);
            }
        }

        if is_selected {
            name_style = name_style.bg(theme.tree_selection_bg);
        }

        if let TreeAction::Renaming { node_idx, .. } = action {
            if i == *node_idx {
                name_style = name_style
                    .fg(theme.panel_border_active)
                    .bg(theme.tree_selection_bg);
            }
        }

        let line = if use_icons {
            build_icon_line(
                node,
                &indent,
                &display_name,
                name_style,
                is_selected,
                area_width,
                scroll_col,
                theme,
            )
        } else {
            build_plain_line(
                node,
                &indent,
                &display_name,
                name_style,
                area_width,
                scroll_col,
            )
        };

        lines.push(line);

        if is_selected && lines.len() < visible_count {
            if let TreeAction::Creating { input, .. } = action {
                lines.push(render_inline_input_line(&indent, input, area_width, theme));
            }
        }
    }

    let paragraph = Paragraph::new(lines);
    frame.render_widget(paragraph, content_area);

    // Render scrollbar when content overflows.
    if let Some(sb_area) = scrollbar_area {
        render_scrollbar(
            nodes.len(),
            visible_count,
            scroll,
            sb_area,
            frame.buffer_mut(),
            theme,
        );
    }
}
