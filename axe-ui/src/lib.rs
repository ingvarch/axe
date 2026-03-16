pub mod layout;
pub mod theme;

use alacritty_terminal::term::cell::Flags as CellFlags;
use alacritty_terminal::vte::ansi::{Color as AnsiColor, CursorShape, NamedColor};
use ratatui::layout::{Alignment, Constraint, Flex, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Clear, Paragraph};
use ratatui::Frame;

use axe_core::{AppState, FocusTarget};
use axe_terminal::TerminalManager;
use axe_tree::icons::{self, FileIcon};
use axe_tree::{FileTree, NodeKind, TreeAction};

use layout::LayoutManager;
use theme::Theme;

/// Width of the help overlay in columns.
const HELP_OVERLAY_WIDTH: u16 = 40;
/// Vertical padding added to the help overlay height (border + title + spacing).
const HELP_OVERLAY_PADDING: u16 = 4;
/// Minimum horizontal margin around the help overlay.
const HELP_OVERLAY_MARGIN: u16 = 4;
/// Minimum vertical margin around the help overlay.
const HELP_OVERLAY_VERTICAL_MARGIN: u16 = 2;
/// Width of the key column in help overlay lines.
const HELP_KEY_COLUMN_WIDTH: usize = 14;
/// Top offset for help content within the overlay inner area.
const HELP_CONTENT_TOP_OFFSET: u16 = 1;

/// Returns the border style for a panel based on whether it has focus and resize mode.
fn border_style_for(
    focus: &FocusTarget,
    panel: &FocusTarget,
    theme: &Theme,
    resize_active: bool,
) -> Style {
    if resize_active && focus == panel {
        Style::default().fg(theme.resize_border)
    } else if focus == panel {
        Style::default().fg(theme.panel_border_active)
    } else {
        Style::default().fg(theme.panel_border)
    }
}

/// Returns the title style for a panel — bold when focused. Uses resize color in resize mode.
fn title_style_for(
    focus: &FocusTarget,
    panel: &FocusTarget,
    theme: &Theme,
    resize_active: bool,
) -> Style {
    if resize_active && focus == panel {
        Style::default()
            .fg(theme.resize_border)
            .add_modifier(Modifier::BOLD)
    } else if focus == panel {
        Style::default()
            .fg(theme.panel_border_active)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(theme.panel_border)
    }
}

/// Creates a styled panel block with the given title and focus state.
fn panel_block<'a>(
    title: &'a str,
    focus: &FocusTarget,
    panel: &FocusTarget,
    theme: &Theme,
    resize_active: bool,
) -> Block<'a> {
    let panel_style = Style::default().bg(theme.background).fg(theme.foreground);

    Block::default()
        .title(title)
        .title_style(title_style_for(focus, panel, theme, resize_active))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(border_style_for(focus, panel, theme, resize_active))
        .style(panel_style)
}

/// Builds the status bar line with hotkey hints.
fn build_status_bar<'a>(app: &AppState, theme: &Theme) -> Line<'a> {
    let version = axe_core::version();
    let focus_label = app.focus.label();
    let key_style = Style::default().fg(theme.status_bar_key);
    let text_style = Style::default().fg(theme.status_bar_fg);
    let resize_style = Style::default()
        .fg(theme.resize_border)
        .add_modifier(Modifier::BOLD);

    let mut spans = vec![Span::styled(format!(" Axe v{version}"), text_style)];

    if app.file_tree.as_ref().is_some_and(|t| t.show_ignored()) {
        spans.push(Span::styled(" | ", key_style));
        spans.push(Span::styled("[SHOW IGNORED]", text_style));
    }

    if app.file_tree.as_ref().is_some_and(|t| t.show_icons()) {
        spans.push(Span::styled(" | ", key_style));
        spans.push(Span::styled("[ICONS]", text_style));
    }

    if app.zoomed_panel.is_some() {
        spans.push(Span::styled(" | ", key_style));
        spans.push(Span::styled("[ZOOM]", resize_style));
    }

    if app.resize_mode.active {
        spans.push(Span::styled(" | ", key_style));
        spans.push(Span::styled("-- RESIZE --", resize_style));
    }

    spans.extend([
        Span::styled(" | ", key_style),
        Span::styled(format!("Focus: {focus_label}"), text_style),
        Span::styled(" | ", key_style),
        Span::styled("^Q", text_style.add_modifier(Modifier::BOLD)),
        Span::styled(" Quit ", key_style),
        Span::styled("^B", text_style.add_modifier(Modifier::BOLD)),
        Span::styled(" Tree ", key_style),
        Span::styled("^T", text_style.add_modifier(Modifier::BOLD)),
        Span::styled(" Term ", key_style),
        Span::styled("Tab", text_style.add_modifier(Modifier::BOLD)),
        Span::styled(" Focus ", key_style),
        Span::styled("^H", text_style.add_modifier(Modifier::BOLD)),
        Span::styled(" Help", key_style),
    ]);

    Line::from(spans)
}

/// Help text lines for the help overlay.
const HELP_LINES: &[(&str, &str)] = &[
    ("Ctrl+Q", "Quit"),
    ("Shift+Tab", "Previous panel"),
    ("Ctrl+1", "Focus Files"),
    ("Ctrl+2", "Focus Editor"),
    ("Ctrl+3", "Focus Terminal"),
    ("Ctrl+B", "Toggle file tree"),
    ("Ctrl+T", "Toggle terminal"),
    ("Ctrl+R", "Resize mode"),
    ("Ctrl+Z", "Zoom panel"),
    ("Click panel", "Focus panel"),
    ("Drag border", "Resize panel"),
    ("", ""),
    ("--- Tree ---", ""),
    ("\u{2191}/\u{2193}", "Navigate tree"),
    ("Enter", "Expand/collapse dir"),
    ("\u{2190}/\u{2192}", "Collapse/expand"),
    ("Home/End", "First/last item"),
    ("Ctrl+G", "Toggle ignored files"),
    ("Ctrl+I", "Toggle file icons"),
    ("n", "New file"),
    ("N", "New directory"),
    ("r", "Rename"),
    ("d", "Delete"),
    ("", ""),
    ("Ctrl+H", "Toggle this help"),
    ("Esc", "Close overlay"),
];

/// Renders the help overlay centered on the screen.
fn render_help_overlay(frame: &mut Frame, theme: &Theme) {
    let area = frame.area();

    let overlay_width = HELP_OVERLAY_WIDTH.min(area.width.saturating_sub(HELP_OVERLAY_MARGIN));
    let overlay_height = (HELP_LINES.len() as u16 + HELP_OVERLAY_PADDING)
        .min(area.height.saturating_sub(HELP_OVERLAY_VERTICAL_MARGIN));

    let horizontal = Layout::horizontal([Constraint::Length(overlay_width)])
        .flex(Flex::Center)
        .split(area);
    let vertical = Layout::vertical([Constraint::Length(overlay_height)])
        .flex(Flex::Center)
        .split(horizontal[0]);
    let overlay_area = vertical[0];

    frame.render_widget(Clear, overlay_area);

    let block = Block::default()
        .title(" Help ")
        .title_style(
            Style::default()
                .fg(theme.overlay_border)
                .add_modifier(Modifier::BOLD),
        )
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(theme.overlay_border))
        .style(Style::default().bg(theme.overlay_bg).fg(theme.foreground));

    let inner = block.inner(overlay_area);

    let lines: Vec<Line> = HELP_LINES
        .iter()
        .map(|(key, desc)| {
            Line::from(vec![
                Span::styled(
                    format!("  {key:<HELP_KEY_COLUMN_WIDTH$}"),
                    Style::default()
                        .fg(theme.panel_border_active)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(desc.to_string(), Style::default().fg(theme.foreground)),
            ])
        })
        .collect();

    frame.render_widget(block, overlay_area);

    let help_text = Paragraph::new(lines).alignment(Alignment::Left);
    let content_area = Rect {
        y: inner.y + HELP_CONTENT_TOP_OFFSET,
        height: inner.height.saturating_sub(HELP_CONTENT_TOP_OFFSET),
        ..inner
    };
    frame.render_widget(help_text, content_area);
}

/// Width of the quit confirmation overlay in columns.
const QUIT_OVERLAY_WIDTH: u16 = 30;
/// Height of the quit confirmation overlay in rows.
const QUIT_OVERLAY_HEIGHT: u16 = 5;

/// Renders a centered quit confirmation dialog.
fn render_quit_overlay(frame: &mut Frame, theme: &Theme) {
    let area = frame.area();

    let overlay_width = QUIT_OVERLAY_WIDTH.min(area.width.saturating_sub(4));
    let overlay_height = QUIT_OVERLAY_HEIGHT.min(area.height.saturating_sub(2));

    let horizontal = Layout::horizontal([Constraint::Length(overlay_width)])
        .flex(Flex::Center)
        .split(area);
    let vertical = Layout::vertical([Constraint::Length(overlay_height)])
        .flex(Flex::Center)
        .split(horizontal[0]);
    let overlay_area = vertical[0];

    frame.render_widget(Clear, overlay_area);

    let block = Block::default()
        .title(" Quit ")
        .title_style(
            Style::default()
                .fg(theme.overlay_border)
                .add_modifier(Modifier::BOLD),
        )
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(theme.overlay_border))
        .style(Style::default().bg(theme.overlay_bg).fg(theme.foreground));

    let inner = block.inner(overlay_area);
    frame.render_widget(block, overlay_area);

    let text = Line::from(vec![
        Span::raw("Are you sure? "),
        Span::styled(
            "(y/N)",
            Style::default()
                .fg(theme.panel_border_active)
                .add_modifier(Modifier::BOLD),
        ),
    ]);

    let paragraph = Paragraph::new(text).alignment(Alignment::Center);
    let content_area = Rect {
        y: inner.y + inner.height / 2,
        height: 1,
        ..inner
    };
    frame.render_widget(paragraph, content_area);
}

/// Indentation width per nesting level in the file tree.
const TREE_INDENT: usize = 2;
/// Prefix for collapsed directories.
const DIR_COLLAPSED_PREFIX: &str = "▸ ";
/// Prefix for expanded directories.
const DIR_EXPANDED_PREFIX: &str = "▾ ";
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

/// Renders a delete confirmation line.
fn render_delete_confirm_line(area_width: usize, theme: &Theme) -> Line<'static> {
    let text = "  Delete? [y/N]";
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
fn build_icon_line(
    node: &axe_tree::TreeNode,
    indent: &str,
    display_name: &str,
    name_style: Style,
    is_selected: bool,
    area_width: usize,
    theme: &Theme,
) -> Line<'static> {
    let icon = if node.depth == 0 {
        icons::DIR_OPEN_ICON
    } else {
        icon_for_node(node)
    };

    let indent_span = Span::styled(indent.to_owned(), name_style);
    let mut icon_style = Style::default().fg(icon.color);
    if is_selected {
        icon_style = icon_style.bg(theme.tree_selection_bg);
    }
    let icon_span = Span::styled(icon.icon, icon_style);

    let used = indent.len() + icon.icon.chars().count();
    let remaining = area_width.saturating_sub(used);
    let name_padded = format!("{:<width$}", display_name, width = remaining);
    let name_span = Span::styled(name_padded, name_style);

    Line::from(vec![indent_span, icon_span, name_span])
}

/// Builds a plain-text tree line without icons.
fn build_plain_line(
    node: &axe_tree::TreeNode,
    indent: &str,
    display_name: &str,
    name_style: Style,
    area_width: usize,
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
    let padded = format!("{:<width$}", text, width = area_width);
    Line::from(Span::styled(padded, name_style))
}

// IMPACT ANALYSIS — render_tree_content
// Parents: render() calls this for tree panel content (normal and zoomed views).
// Children: build_icon_line(), build_plain_line(), render_inline_input_line(),
//           render_delete_confirm_line().
// Siblings: Tree actions (create/rename/delete) inject inline input lines.
//           show_icons toggle changes rendering path.

/// Renders file tree content into the given area, with selection highlight and scrolling.
fn render_tree_content(file_tree: &FileTree, area: Rect, frame: &mut Frame, theme: &Theme) {
    let nodes = file_tree.visible_nodes();
    let scroll = file_tree.scroll();
    let selected = file_tree.selected();
    let visible_count = area.height as usize;
    let action = file_tree.action();
    let area_width = area.width as usize;
    let use_icons = file_tree.show_icons();
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
                theme,
            )
        } else {
            build_plain_line(node, &indent, &display_name, name_style, area_width)
        };

        lines.push(line);

        if is_selected && lines.len() < visible_count {
            match action {
                TreeAction::Creating { input, .. } => {
                    lines.push(render_inline_input_line(&indent, input, area_width, theme));
                }
                TreeAction::ConfirmDelete { .. } => {
                    lines.push(render_delete_confirm_line(area_width, theme));
                }
                _ => {}
            }
        }
    }

    let paragraph = Paragraph::new(lines);
    frame.render_widget(paragraph, area);
}

// IMPACT ANALYSIS — convert_ansi_color
// Parents: render_terminal_content() uses this to convert cell colors.
// Children: None.
// Siblings: Theme colors — terminal colors are independent from theme.

/// Converts an alacritty_terminal ANSI color to a ratatui color.
fn convert_ansi_color(color: &AnsiColor) -> Color {
    match color {
        AnsiColor::Named(named) => match named {
            NamedColor::Black | NamedColor::DimBlack => Color::Black,
            NamedColor::Red | NamedColor::DimRed => Color::Red,
            NamedColor::Green | NamedColor::DimGreen => Color::Green,
            NamedColor::Yellow | NamedColor::DimYellow => Color::Yellow,
            NamedColor::Blue | NamedColor::DimBlue => Color::Blue,
            NamedColor::Magenta | NamedColor::DimMagenta => Color::Magenta,
            NamedColor::Cyan | NamedColor::DimCyan => Color::Cyan,
            NamedColor::White | NamedColor::DimWhite => Color::White,
            NamedColor::BrightBlack => Color::DarkGray,
            NamedColor::BrightRed => Color::LightRed,
            NamedColor::BrightGreen => Color::LightGreen,
            NamedColor::BrightYellow => Color::LightYellow,
            NamedColor::BrightBlue => Color::LightBlue,
            NamedColor::BrightMagenta => Color::LightMagenta,
            NamedColor::BrightCyan => Color::LightCyan,
            NamedColor::BrightWhite => Color::White,
            NamedColor::Foreground | NamedColor::BrightForeground | NamedColor::DimForeground => {
                Color::Reset
            }
            NamedColor::Background => Color::Reset,
            NamedColor::Cursor => Color::Reset,
        },
        AnsiColor::Spec(rgb) => Color::Rgb(rgb.r, rgb.g, rgb.b),
        AnsiColor::Indexed(idx) => Color::Indexed(*idx),
    }
}

/// Converts alacritty cell flags to ratatui style modifiers.
fn cell_flags_to_modifier(flags: CellFlags) -> Modifier {
    let mut modifier = Modifier::empty();
    if flags.contains(CellFlags::BOLD) {
        modifier |= Modifier::BOLD;
    }
    if flags.contains(CellFlags::ITALIC) {
        modifier |= Modifier::ITALIC;
    }
    if flags.intersects(CellFlags::UNDERLINE | CellFlags::DOUBLE_UNDERLINE) {
        modifier |= Modifier::UNDERLINED;
    }
    if flags.contains(CellFlags::DIM) {
        modifier |= Modifier::DIM;
    }
    if flags.contains(CellFlags::INVERSE) {
        modifier |= Modifier::REVERSED;
    }
    if flags.contains(CellFlags::STRIKEOUT) {
        modifier |= Modifier::CROSSED_OUT;
    }
    if flags.contains(CellFlags::HIDDEN) {
        modifier |= Modifier::HIDDEN;
    }
    modifier
}

// IMPACT ANALYSIS — render_terminal_content
// Parents: render_right_panels() and zoomed view call this.
// Children: convert_ansi_color(), cell_flags_to_modifier().
// Siblings: Terminal panel block is rendered separately — this only fills the inner area.

/// Renders terminal content from the active tab into the given area.
fn render_terminal_content(mgr: &TerminalManager, area: Rect, frame: &mut Frame, theme: &Theme) {
    let tab = match mgr.active_tab() {
        Some(tab) => tab,
        None => return,
    };

    let term = tab.term();
    let content = term.renderable_content();
    let buf = frame.buffer_mut();

    // Render each cell from the terminal grid into the ratatui buffer.
    for indexed in content.display_iter {
        let point = indexed.point;
        let cell = &indexed.cell;

        // Convert grid coordinates to buffer coordinates within the area.
        let x = area.x + point.column.0 as u16;
        let y = area.y + point.line.0 as u16;

        // Skip cells outside the visible area.
        if x >= area.x + area.width || y >= area.y + area.height {
            continue;
        }

        // Skip wide char spacers — the main wide char cell covers them.
        if cell.flags.contains(CellFlags::WIDE_CHAR_SPACER)
            || cell.flags.contains(CellFlags::LEADING_WIDE_CHAR_SPACER)
        {
            continue;
        }

        let fg = convert_ansi_color(&cell.fg);
        let bg = convert_ansi_color(&cell.bg);
        let modifier = cell_flags_to_modifier(cell.flags);

        let style = Style::default().fg(fg).bg(bg).add_modifier(modifier);

        if let Some(buf_cell) = buf.cell_mut((x, y)) {
            buf_cell.set_char(cell.c);
            buf_cell.set_style(style);
        }
    }

    // Render cursor.
    if content.cursor.shape != CursorShape::Hidden {
        let cursor_point = content.cursor.point;
        let cx = area.x + cursor_point.column.0 as u16;
        let cy = area.y + cursor_point.line.0 as u16;

        if cx < area.x + area.width && cy < area.y + area.height {
            if let Some(buf_cell) = buf.cell_mut((cx, cy)) {
                let cursor_style = Style::default()
                    .fg(theme.background)
                    .bg(theme.foreground)
                    .add_modifier(Modifier::REVERSED);
                buf_cell.set_style(cursor_style);
            }
        }
    }
}

/// Returns the inner area of the terminal panel, if terminal is visible.
///
/// Used by the main loop to sync PTY size with panel dimensions.
pub fn terminal_inner_rect(app: &AppState, area: Rect) -> Option<Rect> {
    let layout_mgr = LayoutManager {
        show_tree: app.show_tree,
        show_terminal: app.show_terminal,
        tree_width_pct: app.tree_width_pct,
        editor_height_pct: app.editor_height_pct,
    };

    if !layout_mgr.show_terminal {
        return None;
    }

    if let Some(ref zoomed) = app.zoomed_panel {
        if matches!(zoomed, FocusTarget::Terminal(_)) {
            let vertical =
                Layout::vertical([Constraint::Min(0), Constraint::Length(1)]).split(area);
            let block = panel_block(
                " Terminal (zoomed) ",
                &app.focus,
                &FocusTarget::Terminal(0),
                &Theme::default(),
                false,
            );
            return Some(block.inner(vertical[0]));
        }
        return None;
    }

    let vertical = Layout::vertical([Constraint::Min(0), Constraint::Length(1)]).split(area);
    let main_area = vertical[0];

    let right_area = if layout_mgr.show_tree {
        let horizontal = Layout::horizontal([
            Constraint::Percentage(layout_mgr.tree_width_pct),
            Constraint::Percentage(100 - layout_mgr.tree_width_pct),
        ])
        .split(main_area);
        horizontal[1]
    } else {
        main_area
    };

    let right_split = Layout::vertical([
        Constraint::Percentage(layout_mgr.editor_height_pct),
        Constraint::Percentage(100 - layout_mgr.editor_height_pct),
    ])
    .split(right_area);

    let term_block = panel_block(
        " Terminal ",
        &app.focus,
        &FocusTarget::Terminal(0),
        &Theme::default(),
        false,
    );
    Some(term_block.inner(right_split[1]))
}

/// Renders the full IDE interface with conditional panel visibility and a status bar.
pub fn render(app: &AppState, frame: &mut Frame) {
    let theme = Theme::default();
    let layout_mgr = LayoutManager {
        show_tree: app.show_tree,
        show_terminal: app.show_terminal,
        tree_width_pct: app.tree_width_pct,
        editor_height_pct: app.editor_height_pct,
    };
    let area = frame.area();

    // Split vertically: main area + status bar
    let vertical = Layout::vertical([Constraint::Min(0), Constraint::Length(1)]).split(area);
    let main_area = vertical[0];
    let status_area = vertical[1];

    let resize_active = app.resize_mode.active;

    if let Some(ref zoomed) = app.zoomed_panel {
        let (title, panel_target) = match zoomed {
            FocusTarget::Tree => (" Files (zoomed) ", FocusTarget::Tree),
            FocusTarget::Editor => (" Editor (zoomed) ", FocusTarget::Editor),
            FocusTarget::Terminal(id) => (" Terminal (zoomed) ", FocusTarget::Terminal(*id)),
        };
        let block = panel_block(title, &app.focus, &panel_target, &theme, resize_active);
        let inner = block.inner(main_area);
        frame.render_widget(block, main_area);
        match zoomed {
            FocusTarget::Tree => {
                if let Some(ref tree) = app.file_tree {
                    render_tree_content(tree, inner, frame, &theme);
                }
            }
            FocusTarget::Terminal(_) => {
                if let Some(ref mgr) = app.terminal_manager {
                    render_terminal_content(mgr, inner, frame, &theme);
                }
            }
            FocusTarget::Editor => {}
        }
    } else if layout_mgr.show_tree {
        let horizontal = Layout::horizontal([
            Constraint::Percentage(layout_mgr.tree_width_pct),
            Constraint::Percentage(100 - layout_mgr.tree_width_pct),
        ])
        .split(main_area);

        let tree_area = horizontal[0];
        let right_area = horizontal[1];

        let tree_block = panel_block(
            " Files ",
            &app.focus,
            &FocusTarget::Tree,
            &theme,
            resize_active,
        );
        let tree_inner = tree_block.inner(tree_area);
        frame.render_widget(tree_block, tree_area);
        if let Some(ref tree) = app.file_tree {
            render_tree_content(tree, tree_inner, frame, &theme);
        }

        render_right_panels(app, frame, right_area, &layout_mgr, &theme, resize_active);
    } else {
        render_right_panels(app, frame, main_area, &layout_mgr, &theme, resize_active);
    }

    // Status bar with hotkey hints
    let status_line = build_status_bar(app, &theme);
    let status_bar = Paragraph::new(status_line).style(
        Style::default()
            .bg(theme.status_bar_bg)
            .fg(theme.status_bar_fg),
    );
    frame.render_widget(status_bar, status_area);

    // Overlays (on top of everything)
    if app.confirm_quit {
        render_quit_overlay(frame, &theme);
    } else if app.show_help {
        render_help_overlay(frame, &theme);
    }
}

/// Renders the right-side panels (editor and optionally terminal) in the given area.
fn render_right_panels(
    app: &AppState,
    frame: &mut Frame,
    area: ratatui::layout::Rect,
    layout_mgr: &LayoutManager,
    theme: &Theme,
    resize_active: bool,
) {
    if layout_mgr.show_terminal {
        let right_split = Layout::vertical([
            Constraint::Percentage(layout_mgr.editor_height_pct),
            Constraint::Percentage(100 - layout_mgr.editor_height_pct),
        ])
        .split(area);

        frame.render_widget(
            panel_block(
                " Editor ",
                &app.focus,
                &FocusTarget::Editor,
                theme,
                resize_active,
            ),
            right_split[0],
        );

        let term_block = panel_block(
            " Terminal ",
            &app.focus,
            &FocusTarget::Terminal(0),
            theme,
            resize_active,
        );
        let term_inner = term_block.inner(right_split[1]);
        frame.render_widget(term_block, right_split[1]);
        if let Some(ref mgr) = app.terminal_manager {
            render_terminal_content(mgr, term_inner, frame, theme);
        }
    } else {
        frame.render_widget(
            panel_block(
                " Editor ",
                &app.focus,
                &FocusTarget::Editor,
                theme,
                resize_active,
            ),
            area,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axe_core::{AppState, FocusTarget};
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    fn render_app_to_string(app: &AppState, width: u16, height: u16) -> String {
        let backend = TestBackend::new(width, height);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal.draw(|frame| render(app, frame)).unwrap();

        terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|cell| cell.symbol().chars().next().unwrap_or(' '))
            .collect()
    }

    fn render_to_string(width: u16, height: u16) -> String {
        let app = AppState::new();
        render_app_to_string(&app, width, height)
    }

    #[test]
    fn render_shows_files_panel_title() {
        let content = render_to_string(80, 24);
        assert!(
            content.contains("Files"),
            "expected 'Files' in rendered output"
        );
    }

    #[test]
    fn render_shows_editor_panel_title() {
        let content = render_to_string(80, 24);
        assert!(
            content.contains("Editor"),
            "expected 'Editor' in rendered output"
        );
    }

    #[test]
    fn render_shows_terminal_panel_title() {
        let content = render_to_string(80, 24);
        assert!(
            content.contains("Terminal"),
            "expected 'Terminal' in rendered output"
        );
    }

    #[test]
    fn render_shows_status_bar_version() {
        let content = render_to_string(100, 24);
        assert!(content.contains("Axe v"), "expected 'Axe v' in status bar");
    }

    #[test]
    fn render_status_bar_shows_hotkeys() {
        let content = render_to_string(100, 24);
        assert!(content.contains("Quit"), "expected 'Quit' hotkey hint");
        assert!(content.contains("Tree"), "expected 'Tree' hotkey hint");
        assert!(content.contains("Term"), "expected 'Term' hotkey hint");
        assert!(content.contains("Help"), "expected 'Help' hotkey hint");
    }

    #[test]
    fn render_status_bar_shows_ctrl_q() {
        let content = render_to_string(100, 24);
        assert!(content.contains("^Q"), "expected '^Q' in status bar");
    }

    #[test]
    fn render_works_with_small_terminal() {
        let content = render_to_string(40, 10);
        assert!(!content.is_empty());
    }

    #[test]
    fn render_tree_has_active_border_by_default() {
        let content = render_to_string(100, 24);
        assert!(
            content.contains("Focus: Files"),
            "expected 'Focus: Files' in status bar"
        );
    }

    #[test]
    fn render_status_bar_shows_focus_files() {
        let mut app = AppState::new();
        app.focus = FocusTarget::Tree;
        let content = render_app_to_string(&app, 100, 24);
        assert!(
            content.contains("Focus: Files"),
            "expected 'Focus: Files' in status bar"
        );
    }

    #[test]
    fn render_status_bar_shows_focus_terminal() {
        let mut app = AppState::new();
        app.focus = FocusTarget::Terminal(0);
        let content = render_app_to_string(&app, 100, 24);
        assert!(
            content.contains("Focus: Terminal"),
            "expected 'Focus: Terminal' in status bar"
        );
    }

    #[test]
    fn render_hides_tree_when_show_tree_false() {
        let mut app = AppState::new();
        app.show_tree = false;
        app.focus = FocusTarget::Editor;
        let content = render_app_to_string(&app, 80, 24);
        assert!(
            !content.contains("Files"),
            "expected 'Files' to be absent when tree is hidden"
        );
    }

    #[test]
    fn render_hides_terminal_when_show_terminal_false() {
        let mut app = AppState::new();
        app.show_terminal = false;
        let content = render_app_to_string(&app, 80, 24);
        assert!(
            !content.contains("Terminal"),
            "expected 'Terminal' to be absent when terminal is hidden"
        );
    }

    #[test]
    fn render_editor_fills_width_when_tree_hidden() {
        let mut app = AppState::new();
        app.show_tree = false;
        app.focus = FocusTarget::Editor;
        let content = render_app_to_string(&app, 80, 24);
        assert!(
            content.contains("Editor"),
            "expected 'Editor' visible when tree is hidden"
        );
        assert!(
            !content.contains("Files"),
            "expected 'Files' absent when tree is hidden"
        );
    }

    #[test]
    fn render_editor_fills_height_when_terminal_hidden() {
        let mut app = AppState::new();
        app.show_terminal = false;
        let content = render_app_to_string(&app, 80, 24);
        assert!(
            content.contains("Editor"),
            "expected 'Editor' visible when terminal is hidden"
        );
        assert!(
            !content.contains("Terminal"),
            "expected 'Terminal' absent when terminal is hidden"
        );
    }

    #[test]
    fn render_help_overlay_when_show_help_true() {
        let mut app = AppState::new();
        app.show_help = true;
        let content = render_app_to_string(&app, 80, 24);
        assert!(content.contains("Help"), "expected 'Help' title in overlay");
        assert!(content.contains("Quit"), "expected 'Quit' in help content");
    }

    #[test]
    fn render_no_help_overlay_by_default() {
        let content = render_to_string(80, 24);
        // "Help" appears in status bar hints, but the overlay key list items
        // like "Toggle this help" should NOT appear unless overlay is shown.
        assert!(
            !content.contains("Toggle this help"),
            "expected help overlay content absent by default"
        );
    }

    #[test]
    fn render_help_overlay_shows_keybindings() {
        let mut app = AppState::new();
        app.show_help = true;
        let content = render_app_to_string(&app, 80, 36);
        assert!(content.contains("Ctrl+Q"), "expected 'Ctrl+Q' in help");
        assert!(content.contains("Ctrl+B"), "expected 'Ctrl+B' in help");
        assert!(content.contains("Ctrl+T"), "expected 'Ctrl+T' in help");
        assert!(content.contains("Ctrl+H"), "expected 'Ctrl+H' in help");
        assert!(content.contains("Ctrl+R"), "expected 'Ctrl+R' in help");
        assert!(content.contains("Esc"), "expected 'Esc' in help");
    }

    #[test]
    fn render_quit_overlay_when_confirm_quit() {
        let mut app = AppState::new();
        app.confirm_quit = true;
        let content = render_app_to_string(&app, 80, 24);
        assert!(
            content.contains("y/N"),
            "expected 'y/N' in quit confirmation overlay"
        );
    }

    #[test]
    fn render_no_quit_overlay_by_default() {
        let app = AppState::new();
        let content = render_app_to_string(&app, 80, 24);
        assert!(
            !content.contains("y/N"),
            "quit overlay should not appear by default"
        );
    }

    #[test]
    fn render_shows_resize_indicator_when_resize_mode_active() {
        let mut app = AppState::new();
        app.resize_mode.active = true;
        let content = render_app_to_string(&app, 100, 24);
        assert!(
            content.contains("-- RESIZE --"),
            "expected '-- RESIZE --' in status bar"
        );
    }

    #[test]
    fn render_no_resize_indicator_by_default() {
        let content = render_to_string(100, 24);
        assert!(
            !content.contains("-- RESIZE --"),
            "expected no '-- RESIZE --' by default"
        );
    }

    // --- Zoom rendering tests ---

    #[test]
    fn render_zoomed_shows_only_focused_panel() {
        let mut app = AppState::new();
        app.focus = FocusTarget::Editor;
        app.zoomed_panel = Some(FocusTarget::Editor);
        let content = render_app_to_string(&app, 100, 24);
        assert!(content.contains("Editor"), "expected 'Editor' panel");
        assert!(
            !content.contains("Files"),
            "expected 'Files' hidden when editor is zoomed"
        );
        assert!(
            !content.contains("Terminal"),
            "expected 'Terminal' hidden when editor is zoomed"
        );
    }

    #[test]
    fn render_zoomed_tree_shows_only_tree() {
        let mut app = AppState::new();
        app.focus = FocusTarget::Tree;
        app.zoomed_panel = Some(FocusTarget::Tree);
        let content = render_app_to_string(&app, 100, 24);
        assert!(content.contains("Files"), "expected 'Files' panel");
        assert!(
            !content.contains("Editor"),
            "expected 'Editor' hidden when tree is zoomed"
        );
        assert!(
            !content.contains("Terminal"),
            "expected 'Terminal' hidden when tree is zoomed"
        );
    }

    #[test]
    fn render_zoomed_terminal_shows_only_terminal() {
        let mut app = AppState::new();
        app.focus = FocusTarget::Terminal(0);
        app.zoomed_panel = Some(FocusTarget::Terminal(0));
        let content = render_app_to_string(&app, 100, 24);
        assert!(content.contains("Terminal"), "expected 'Terminal' panel");
        assert!(
            !content.contains("Files"),
            "expected 'Files' hidden when terminal is zoomed"
        );
        assert!(
            !content.contains("Editor"),
            "expected 'Editor' hidden when terminal is zoomed"
        );
    }

    #[test]
    fn render_zoom_indicator_in_status_bar() {
        let mut app = AppState::new();
        app.zoomed_panel = Some(FocusTarget::Editor);
        let content = render_app_to_string(&app, 100, 24);
        assert!(
            content.contains("[ZOOM]"),
            "expected '[ZOOM]' in status bar when zoomed"
        );
    }

    #[test]
    fn render_no_zoom_indicator_by_default() {
        let content = render_to_string(100, 24);
        assert!(
            !content.contains("[ZOOM]"),
            "expected no '[ZOOM]' by default"
        );
    }

    #[test]
    fn render_zoomed_panel_title_has_suffix() {
        let mut app = AppState::new();
        app.focus = FocusTarget::Editor;
        app.zoomed_panel = Some(FocusTarget::Editor);
        let content = render_app_to_string(&app, 100, 24);
        assert!(
            content.contains("(zoomed)"),
            "expected '(zoomed)' suffix in panel title"
        );
    }

    #[test]
    fn render_uses_app_tree_width_pct() {
        let mut app = AppState::new();
        app.tree_width_pct = 40;
        // Just ensure it renders without panic; the visual difference
        // is verified by the layout using the custom percentage.
        let content = render_app_to_string(&app, 100, 24);
        assert!(content.contains("Files"), "expected 'Files' panel");
        assert!(content.contains("Editor"), "expected 'Editor' panel");
    }

    // --- File tree rendering tests ---

    fn app_with_tree() -> (AppState, tempfile::TempDir) {
        let tmp = tempfile::TempDir::new().unwrap();
        std::fs::create_dir(tmp.path().join("src")).unwrap();
        std::fs::write(tmp.path().join("Cargo.toml"), "").unwrap();
        std::fs::write(tmp.path().join("README.md"), "").unwrap();
        let app = AppState::new_with_root(tmp.path().to_path_buf());
        (app, tmp)
    }

    #[test]
    fn render_tree_shows_root_name() {
        let (app, tmp) = app_with_tree();
        let root_name = tmp
            .path()
            .canonicalize()
            .unwrap()
            .file_name()
            .unwrap()
            .to_string_lossy()
            .into_owned();
        let content = render_app_to_string(&app, 100, 24);
        assert!(
            content.contains(&root_name),
            "expected root name '{root_name}' in rendered output"
        );
    }

    #[test]
    fn render_tree_shows_directory_prefix_without_icons() {
        let (mut app, _tmp) = app_with_tree();
        // Disable icons to get plain text prefixes.
        if let Some(ref mut tree) = app.file_tree {
            tree.toggle_show_icons();
        }
        let content = render_app_to_string(&app, 100, 24);
        assert!(
            content.contains('\u{25B8}'),
            "expected collapsed dir prefix '▸' in rendered output when icons disabled"
        );
    }

    #[test]
    fn render_tree_shows_file_entries() {
        let (app, _tmp) = app_with_tree();
        let content = render_app_to_string(&app, 100, 24);
        assert!(
            content.contains("Cargo.toml"),
            "expected 'Cargo.toml' in tree"
        );
        assert!(
            content.contains("README.md"),
            "expected 'README.md' in tree"
        );
    }

    #[test]
    fn render_tree_shows_directory_name() {
        let (app, _tmp) = app_with_tree();
        let content = render_app_to_string(&app, 100, 24);
        assert!(content.contains("src"), "expected 'src' directory in tree");
    }
}
