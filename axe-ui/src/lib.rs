pub mod layout;
pub mod theme;

use alacritty_terminal::term::cell::Flags as CellFlags;
use alacritty_terminal::vte::ansi::{Color as AnsiColor, CursorShape, NamedColor};
use ratatui::layout::{Alignment, Constraint, Flex, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Clear, Paragraph};
use ratatui::Frame;

use axe_core::project_search::{DisplayItem, SearchField};
use axe_core::{AppState, CommandPalette, FileFinder, FocusTarget, ProjectSearch, SearchState};
use axe_editor::EditorBuffer;
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

    if let Some(buffer) = app.buffer_manager.active_buffer() {
        let name = buffer.file_name().unwrap_or("[untitled]");
        let lines = buffer.line_count();
        let ftype = buffer.file_type();
        spans.push(Span::styled(" | ", key_style));
        spans.push(Span::styled(name.to_string(), text_style));
        if buffer.modified {
            spans.push(Span::styled(" [+]", key_style));
        }
        spans.push(Span::styled(" | ", key_style));
        spans.push(Span::styled(format!("{lines} lines"), text_style));
        spans.push(Span::styled(" | ", key_style));
        spans.push(Span::styled(ftype.to_string(), text_style));
        spans.push(Span::styled(" | ", key_style));
        spans.push(Span::styled(
            format!(
                "Ln {}, Col {}",
                buffer.cursor.row + 1,
                buffer.cursor.col + 1
            ),
            text_style,
        ));
    }

    if let Some((ref msg, _)) = app.status_message {
        spans.push(Span::styled(" | ", key_style));
        spans.push(Span::styled(
            msg.clone(),
            Style::default()
                .fg(theme.panel_border_active)
                .add_modifier(Modifier::BOLD),
        ));
    }

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

/// Returns the editor panel title, including a modified indicator if needed.
fn editor_title(app: &AppState, zoomed: bool) -> &'static str {
    let modified = app
        .buffer_manager
        .active_buffer()
        .is_some_and(|b| b.modified);
    match (zoomed, modified) {
        (true, true) => " Editor (zoomed) [+] ",
        (true, false) => " Editor (zoomed) ",
        (false, true) => " Editor [+] ",
        (false, false) => " Editor ",
    }
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
    ("Alt+Z", "Zoom panel"),
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
    ("--- Tabs ---", ""),
    ("Alt+T", "New tab (terminal)"),
    ("Alt+W / Ctrl+W", "Close tab"),
    ("Alt+]/[", "Next/prev tab"),
    ("Alt+1-9", "Terminal tab N"),
    ("", ""),
    ("--- Editor ---", ""),
    ("Ctrl+F", "Find in file"),
    ("Shift+Arrows", "Select text"),
    ("Ctrl+A", "Select all"),
    ("Ctrl+C", "Copy"),
    ("Ctrl+X", "Cut"),
    ("Ctrl+V", "Paste"),
    ("Ctrl+Z", "Undo"),
    ("Ctrl+Shift+Z", "Redo"),
    ("Ctrl+Y", "Redo"),
    ("Ctrl+S", "Save"),
    ("Ctrl+P", "Open file finder"),
    ("F1 / Ctrl+Shift+P", "Command palette"),
    ("F2 / Ctrl+Shift+F", "Find in project"),
    ("", ""),
    ("--- Terminal ---", ""),
    ("Shift+PgUp", "Scroll up"),
    ("Shift+PgDn", "Scroll down"),
    ("Shift+Home", "Scroll to top"),
    ("Shift+End", "Scroll to bottom"),
    ("Mouse drag", "Select text (copy)"),
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

/// Minimum width of the confirmation dialog in columns.
const CONFIRM_DIALOG_MIN_WIDTH: u16 = 30;
/// Padding for button labels (spaces around text).
const CONFIRM_BUTTON_PADDING: usize = 2;

/// Renders a centered confirmation dialog with navigable [Yes] / [No] buttons.
fn render_confirm_dialog(dialog: &axe_core::ConfirmDialog, frame: &mut Frame, theme: &Theme) {
    use axe_core::ConfirmButton;

    let area = frame.area();

    // Calculate width based on longest message line.
    let max_message_width = dialog
        .message
        .iter()
        .map(|l| l.len() as u16)
        .max()
        .unwrap_or(0);
    // Button row: "  [ Yes ]  [ No ]  " = ~20 chars
    let button_row_width: u16 = 20;
    let content_width = max_message_width.max(button_row_width);
    // +4 for border (2) + inner padding (2)
    let overlay_width = (content_width + 4)
        .max(CONFIRM_DIALOG_MIN_WIDTH)
        .min(area.width.saturating_sub(4));
    // Height: border(2) + top padding(1) + message lines + gap(1) + button row(1) + bottom padding(1)
    let overlay_height =
        (2 + 1 + dialog.message.len() as u16 + 1 + 1 + 1).min(area.height.saturating_sub(2));

    let horizontal = Layout::horizontal([Constraint::Length(overlay_width)])
        .flex(Flex::Center)
        .split(area);
    let vertical = Layout::vertical([Constraint::Length(overlay_height)])
        .flex(Flex::Center)
        .split(horizontal[0]);
    let overlay_area = vertical[0];

    frame.render_widget(Clear, overlay_area);

    let title = format!(" {} ", dialog.title);
    let block = Block::default()
        .title(title)
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

    // Build message lines.
    let mut lines: Vec<Line<'_>> = dialog
        .message
        .iter()
        .map(|msg| {
            if msg.is_empty() {
                Line::from("")
            } else {
                Line::from(Span::styled(
                    msg.clone(),
                    Style::default().fg(theme.foreground),
                ))
            }
        })
        .collect();

    // Empty line before buttons.
    lines.push(Line::from(""));

    // Button row.
    let yes_label = " Yes ";
    let no_label = " No ";
    let (yes_style, no_style) = match dialog.selected {
        ConfirmButton::Yes => (
            Style::default()
                .fg(theme.overlay_bg)
                .bg(theme.panel_border_active)
                .add_modifier(Modifier::BOLD),
            Style::default().fg(theme.foreground).bg(theme.overlay_bg),
        ),
        ConfirmButton::No => (
            Style::default().fg(theme.foreground).bg(theme.overlay_bg),
            Style::default()
                .fg(theme.overlay_bg)
                .bg(theme.panel_border_active)
                .add_modifier(Modifier::BOLD),
        ),
    };

    let button_line = Line::from(vec![
        Span::styled(
            format!("{:>pad$}", "[", pad = CONFIRM_BUTTON_PADDING),
            Style::default(),
        ),
        Span::styled(yes_label, yes_style),
        Span::styled(" ]  [ ", Style::default()),
        Span::styled(no_label, no_style),
        Span::styled(
            format!("{:<pad$}", "]", pad = CONFIRM_BUTTON_PADDING),
            Style::default(),
        ),
    ]);
    lines.push(button_line);

    let paragraph = Paragraph::new(lines).alignment(Alignment::Center);
    let content_area = Rect {
        y: inner.y + 1,
        height: inner.height.saturating_sub(1),
        ..inner
    };
    frame.render_widget(paragraph, content_area);
}

/// Width of the project search overlay as a percentage of screen width.
const PROJECT_SEARCH_WIDTH_PCT: u16 = 80;
/// Height of the project search overlay as a percentage of screen height.
const PROJECT_SEARCH_HEIGHT_PCT: u16 = 70;
/// Minimum width of the project search overlay.
const PROJECT_SEARCH_MIN_WIDTH: u16 = 50;
/// Minimum height of the project search overlay.
const PROJECT_SEARCH_MIN_HEIGHT: u16 = 12;

/// Renders the project-wide search overlay centered on the screen.
fn render_project_search(search: &ProjectSearch, frame: &mut Frame, theme: &Theme) {
    let area = frame.area();

    let overlay_width = (area.width * PROJECT_SEARCH_WIDTH_PCT / 100)
        .max(PROJECT_SEARCH_MIN_WIDTH)
        .min(area.width.saturating_sub(4));
    let overlay_height = (area.height * PROJECT_SEARCH_HEIGHT_PCT / 100)
        .max(PROJECT_SEARCH_MIN_HEIGHT)
        .min(area.height.saturating_sub(2));

    let horizontal = Layout::horizontal([Constraint::Length(overlay_width)])
        .flex(Flex::Center)
        .split(area);
    let vertical = Layout::vertical([Constraint::Length(overlay_height)])
        .flex(Flex::Center)
        .split(horizontal[0]);
    let overlay_area = vertical[0];

    frame.render_widget(Clear, overlay_area);

    let block = Block::default()
        .title(" Project Search ")
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

    if inner.width == 0 || inner.height == 0 {
        return;
    }

    // Row 1: query input with toggle indicators
    let case_indicator = if search.case_sensitive {
        Span::styled(
            "[Aa]",
            Style::default()
                .fg(theme.panel_border_active)
                .add_modifier(Modifier::BOLD),
        )
    } else {
        Span::styled("[Aa]", Style::default().fg(theme.panel_border))
    };

    let regex_indicator = if search.regex_mode {
        Span::styled(
            "[.*]",
            Style::default()
                .fg(theme.panel_border_active)
                .add_modifier(Modifier::BOLD),
        )
    } else {
        Span::styled("[.*]", Style::default().fg(theme.panel_border))
    };

    let query_style = if search.active_field == SearchField::Query {
        Style::default().fg(theme.foreground)
    } else {
        Style::default().fg(theme.panel_border)
    };

    let cursor = if search.active_field == SearchField::Query {
        Span::styled("|", Style::default().fg(theme.panel_border_active))
    } else {
        Span::raw("")
    };

    let input_line = Line::from(vec![
        Span::styled(
            " > ",
            Style::default()
                .fg(theme.panel_border_active)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(&search.query, query_style),
        cursor,
        Span::raw(" "),
        case_indicator,
        Span::raw(" "),
        regex_indicator,
    ]);
    let input_area = Rect { height: 1, ..inner };
    frame.render_widget(Paragraph::new(input_line), input_area);

    // Row 2: Include/Exclude fields
    if inner.height > 1 {
        let include_style = if search.active_field == SearchField::Include {
            Style::default().fg(theme.foreground)
        } else {
            Style::default().fg(theme.panel_border)
        };
        let exclude_style = if search.active_field == SearchField::Exclude {
            Style::default().fg(theme.foreground)
        } else {
            Style::default().fg(theme.panel_border)
        };

        let include_cursor = if search.active_field == SearchField::Include {
            Span::styled("|", Style::default().fg(theme.panel_border_active))
        } else {
            Span::raw("")
        };
        let exclude_cursor = if search.active_field == SearchField::Exclude {
            Span::styled("|", Style::default().fg(theme.panel_border_active))
        } else {
            Span::raw("")
        };

        let filter_line = Line::from(vec![
            Span::styled(" Include: ", include_style),
            Span::styled(&search.include_pattern, include_style),
            include_cursor,
            Span::raw("  "),
            Span::styled("Exclude: ", exclude_style),
            Span::styled(&search.exclude_pattern, exclude_style),
            exclude_cursor,
        ]);
        let filter_area = Rect {
            y: inner.y + 1,
            height: 1,
            ..inner
        };
        frame.render_widget(Paragraph::new(filter_line), filter_area);
    }

    // Separator
    if inner.height > 2 {
        let sep = Line::from(Span::styled(
            "\u{2500}".repeat(inner.width as usize),
            Style::default().fg(theme.panel_border),
        ));
        let sep_area = Rect {
            y: inner.y + 2,
            height: 1,
            ..inner
        };
        frame.render_widget(Paragraph::new(sep), sep_area);
    }

    // Results list
    let results_start_y = inner.y + 3;
    let results_height = inner.height.saturating_sub(4); // input + filter + sep + footer
    let max_visible = results_height as usize;

    // Adjust scroll offset to keep selection visible.
    let scroll_offset = if search.selected < search.scroll_offset {
        search.selected
    } else if search.selected >= search.scroll_offset + max_visible {
        search
            .selected
            .saturating_sub(max_visible.saturating_sub(1))
    } else {
        search.scroll_offset
    };

    for (i, display_item) in search
        .display_items
        .iter()
        .skip(scroll_offset)
        .take(max_visible)
        .enumerate()
    {
        let is_selected = scroll_offset + i == search.selected;
        let row_y = results_start_y + i as u16;
        let row_area = Rect {
            y: row_y,
            height: 1,
            ..inner
        };

        let bg = if is_selected {
            theme.tree_selection_bg
        } else {
            theme.overlay_bg
        };

        match display_item {
            DisplayItem::FileHeader {
                relative_path,
                match_count,
            } => {
                let header_text = format!(" {} ({} matches)", relative_path, match_count);
                let mut spans = vec![Span::styled(
                    header_text,
                    Style::default()
                        .fg(theme.panel_border_active)
                        .bg(bg)
                        .add_modifier(Modifier::BOLD),
                )];
                // Fill remaining width.
                if is_selected {
                    let used: usize = spans.iter().map(|s| s.content.chars().count()).sum();
                    let remaining = (inner.width as usize).saturating_sub(used);
                    if remaining > 0 {
                        spans.push(Span::styled(" ".repeat(remaining), Style::default().bg(bg)));
                    }
                }
                frame.render_widget(Paragraph::new(Line::from(spans)), row_area);
            }
            DisplayItem::MatchLine { result_index } => {
                if let Some(result) = search.results.get(*result_index) {
                    let line_num = format!("   {:>4}: ", result.line_number);
                    let mut spans = vec![Span::styled(
                        line_num,
                        Style::default().fg(theme.panel_border).bg(bg),
                    )];

                    // Render line text with match highlighting.
                    let text = &result.line_text;
                    let start = result.match_start.min(text.len());
                    let end = result.match_end.min(text.len());

                    if start > 0 {
                        spans.push(Span::styled(
                            &text[..start],
                            Style::default().fg(theme.foreground).bg(bg),
                        ));
                    }
                    if start < end {
                        spans.push(Span::styled(
                            &text[start..end],
                            Style::default()
                                .fg(theme.panel_border_active)
                                .bg(bg)
                                .add_modifier(Modifier::BOLD),
                        ));
                    }
                    if end < text.len() {
                        spans.push(Span::styled(
                            &text[end..],
                            Style::default().fg(theme.foreground).bg(bg),
                        ));
                    }

                    // Fill remaining width for selected row.
                    if is_selected {
                        let used: usize = spans.iter().map(|s| s.content.chars().count()).sum();
                        let remaining = (inner.width as usize).saturating_sub(used);
                        if remaining > 0 {
                            spans
                                .push(Span::styled(" ".repeat(remaining), Style::default().bg(bg)));
                        }
                    }
                    frame.render_widget(Paragraph::new(Line::from(spans)), row_area);
                }
            }
        }
    }

    // Footer: result count or status
    if inner.height > 3 {
        let footer_y = inner.y + inner.height - 1;
        let footer_text = if search.searching {
            format!(
                " Searching... ({} files, {} results)",
                search.files_searched,
                search.total_matches()
            )
        } else if search.results.is_empty() {
            if search.query.is_empty() {
                " Type to search".to_string()
            } else {
                " No results".to_string()
            }
        } else {
            format!(
                " {} results in {} files",
                search.total_matches(),
                search.files_with_matches
            )
        };
        let footer_line = Line::from(Span::styled(
            footer_text,
            Style::default().fg(theme.panel_border),
        ));
        let footer_area = Rect {
            y: footer_y,
            height: 1,
            ..inner
        };
        frame.render_widget(Paragraph::new(footer_line), footer_area);
    }
}

/// Width of the file finder overlay as a percentage of screen width.
const FILE_FINDER_WIDTH_PCT: u16 = 60;
/// Height of the file finder overlay as a percentage of screen height.
const FILE_FINDER_HEIGHT_PCT: u16 = 50;
/// Minimum width of the file finder overlay.
const FILE_FINDER_MIN_WIDTH: u16 = 30;
/// Minimum height of the file finder overlay.
const FILE_FINDER_MIN_HEIGHT: u16 = 8;

/// Renders the fuzzy file finder overlay centered on the screen.
fn render_file_finder(finder: &FileFinder, frame: &mut Frame, theme: &Theme) {
    let area = frame.area();

    let overlay_width = (area.width * FILE_FINDER_WIDTH_PCT / 100)
        .max(FILE_FINDER_MIN_WIDTH)
        .min(area.width.saturating_sub(4));
    let overlay_height = (area.height * FILE_FINDER_HEIGHT_PCT / 100)
        .max(FILE_FINDER_MIN_HEIGHT)
        .min(area.height.saturating_sub(2));

    let horizontal = Layout::horizontal([Constraint::Length(overlay_width)])
        .flex(Flex::Center)
        .split(area);
    let vertical = Layout::vertical([Constraint::Length(overlay_height)])
        .flex(Flex::Center)
        .split(horizontal[0]);
    let overlay_area = vertical[0];

    frame.render_widget(Clear, overlay_area);

    let block = Block::default()
        .title(" Open File ")
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

    if inner.width == 0 || inner.height == 0 {
        return;
    }

    // Input line: "> query|"
    let input_line = Line::from(vec![
        Span::styled(
            " > ",
            Style::default()
                .fg(theme.panel_border_active)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(&finder.query, Style::default().fg(theme.foreground)),
        Span::styled("|", Style::default().fg(theme.panel_border_active)),
    ]);
    let input_area = Rect { height: 1, ..inner };
    frame.render_widget(Paragraph::new(input_line), input_area);

    // Separator
    if inner.height > 1 {
        let sep = Line::from(Span::styled(
            "\u{2500}".repeat(inner.width as usize),
            Style::default().fg(theme.panel_border),
        ));
        let sep_area = Rect {
            y: inner.y + 1,
            height: 1,
            ..inner
        };
        frame.render_widget(Paragraph::new(sep), sep_area);
    }

    // Results list
    let results_start_y = inner.y + 2;
    let results_height = inner.height.saturating_sub(3); // input + sep + footer
    let max_visible = results_height as usize;

    // Adjust scroll offset to keep selection visible.
    let scroll_offset = if finder.selected < finder.scroll_offset {
        finder.selected
    } else if finder.selected >= finder.scroll_offset + max_visible {
        finder
            .selected
            .saturating_sub(max_visible.saturating_sub(1))
    } else {
        finder.scroll_offset
    };

    for (i, filtered_item) in finder
        .filtered
        .iter()
        .skip(scroll_offset)
        .take(max_visible)
        .enumerate()
    {
        let item = &finder.items[filtered_item.index];
        let is_selected = scroll_offset + i == finder.selected;
        let row_y = results_start_y + i as u16;

        let row_area = Rect {
            y: row_y,
            height: 1,
            ..inner
        };

        // Build styled spans with match highlighting.
        let prefix = if is_selected { " > " } else { "   " };
        let mut spans = vec![Span::styled(
            prefix,
            Style::default()
                .fg(if is_selected {
                    theme.panel_border_active
                } else {
                    theme.foreground
                })
                .add_modifier(if is_selected {
                    Modifier::BOLD
                } else {
                    Modifier::empty()
                }),
        )];

        let bg = if is_selected {
            theme.tree_selection_bg
        } else {
            theme.overlay_bg
        };

        // Render path with matched character highlighting.
        for (char_idx, ch) in item.relative_path.chars().enumerate() {
            let is_match = filtered_item.match_indices.contains(&(char_idx as u32));
            let style = if is_match {
                Style::default()
                    .fg(theme.panel_border_active)
                    .bg(bg)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(theme.foreground).bg(bg)
            };
            spans.push(Span::styled(ch.to_string(), style));
        }

        // Fill remaining width with background color for selected row.
        if is_selected {
            let used_width = prefix.len() + item.relative_path.chars().count();
            let remaining = (inner.width as usize).saturating_sub(used_width);
            if remaining > 0 {
                spans.push(Span::styled(" ".repeat(remaining), Style::default().bg(bg)));
            }
        }

        frame.render_widget(Paragraph::new(Line::from(spans)), row_area);
    }

    // Footer: file count
    if inner.height > 2 {
        let footer_y = inner.y + inner.height - 1;
        let count_text = if finder.query.is_empty() {
            format!(" {} files", finder.total_files())
        } else {
            format!(
                " {} / {} files",
                finder.filtered.len(),
                finder.total_files()
            )
        };
        let footer_line = Line::from(Span::styled(
            count_text,
            Style::default().fg(theme.panel_border),
        ));
        let footer_area = Rect {
            y: footer_y,
            height: 1,
            ..inner
        };
        frame.render_widget(Paragraph::new(footer_line), footer_area);
    }
}

/// Width of the command palette overlay as a percentage of screen width.
const CMD_PALETTE_WIDTH_PCT: u16 = 60;
/// Height of the command palette overlay as a percentage of screen height.
const CMD_PALETTE_HEIGHT_PCT: u16 = 50;
/// Minimum width of the command palette overlay.
const CMD_PALETTE_MIN_WIDTH: u16 = 40;
/// Minimum height of the command palette overlay.
const CMD_PALETTE_MIN_HEIGHT: u16 = 8;

/// Renders the command palette overlay centered on the screen.
fn render_command_palette(palette: &CommandPalette, frame: &mut Frame, theme: &Theme) {
    let area = frame.area();

    let overlay_width = (area.width * CMD_PALETTE_WIDTH_PCT / 100)
        .max(CMD_PALETTE_MIN_WIDTH)
        .min(area.width.saturating_sub(4));
    let overlay_height = (area.height * CMD_PALETTE_HEIGHT_PCT / 100)
        .max(CMD_PALETTE_MIN_HEIGHT)
        .min(area.height.saturating_sub(2));

    let horizontal = Layout::horizontal([Constraint::Length(overlay_width)])
        .flex(Flex::Center)
        .split(area);
    let vertical = Layout::vertical([Constraint::Length(overlay_height)])
        .flex(Flex::Center)
        .split(horizontal[0]);
    let overlay_area = vertical[0];

    frame.render_widget(Clear, overlay_area);

    let block = Block::default()
        .title(" Command Palette ")
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

    if inner.width == 0 || inner.height == 0 {
        return;
    }

    // Input line: "> query|"
    let input_line = Line::from(vec![
        Span::styled(
            " > ",
            Style::default()
                .fg(theme.panel_border_active)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(&palette.query, Style::default().fg(theme.foreground)),
        Span::styled("|", Style::default().fg(theme.panel_border_active)),
    ]);
    let input_area = Rect { height: 1, ..inner };
    frame.render_widget(Paragraph::new(input_line), input_area);

    // Separator
    if inner.height > 1 {
        let sep = Line::from(Span::styled(
            "\u{2500}".repeat(inner.width as usize),
            Style::default().fg(theme.panel_border),
        ));
        let sep_area = Rect {
            y: inner.y + 1,
            height: 1,
            ..inner
        };
        frame.render_widget(Paragraph::new(sep), sep_area);
    }

    // Results list
    let results_start_y = inner.y + 2;
    let results_height = inner.height.saturating_sub(3); // input + sep + footer
    let max_visible = results_height as usize;

    // Adjust scroll offset to keep selection visible.
    let scroll_offset = if palette.selected < palette.scroll_offset {
        palette.selected
    } else if palette.selected >= palette.scroll_offset + max_visible {
        palette
            .selected
            .saturating_sub(max_visible.saturating_sub(1))
    } else {
        palette.scroll_offset
    };

    for (i, filtered_item) in palette
        .filtered
        .iter()
        .skip(scroll_offset)
        .take(max_visible)
        .enumerate()
    {
        let item = &palette.items[filtered_item.index];
        let is_selected = scroll_offset + i == palette.selected;
        let row_y = results_start_y + i as u16;

        let row_area = Rect {
            y: row_y,
            height: 1,
            ..inner
        };

        let bg = if is_selected {
            theme.tree_selection_bg
        } else {
            theme.overlay_bg
        };

        // Build styled spans: prefix + display_name (with match highlighting) + keybinding (right-aligned)
        let prefix = if is_selected { " > " } else { "   " };
        let mut spans = vec![Span::styled(
            prefix,
            Style::default()
                .fg(if is_selected {
                    theme.panel_border_active
                } else {
                    theme.foreground
                })
                .bg(bg)
                .add_modifier(if is_selected {
                    Modifier::BOLD
                } else {
                    Modifier::empty()
                }),
        )];

        // Render display name with matched character highlighting.
        for (char_idx, ch) in item.display_name.chars().enumerate() {
            let is_match = filtered_item.match_indices.contains(&(char_idx as u32));
            let style = if is_match {
                Style::default()
                    .fg(theme.panel_border_active)
                    .bg(bg)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(theme.foreground).bg(bg)
            };
            spans.push(Span::styled(ch.to_string(), style));
        }

        // Right-align keybinding if present.
        if !item.keybinding.is_empty() {
            let name_width = prefix.len() + item.display_name.chars().count();
            let kb_width = item.keybinding.len();
            let available = inner.width as usize;
            let gap = available.saturating_sub(name_width + kb_width + 1);
            if gap > 0 {
                spans.push(Span::styled(" ".repeat(gap), Style::default().bg(bg)));
                spans.push(Span::styled(
                    &item.keybinding,
                    Style::default().fg(theme.panel_border).bg(bg),
                ));
            }
        }

        // Fill remaining width with background color for selected row.
        if is_selected {
            let used: usize = spans.iter().map(|s| s.content.chars().count()).sum();
            let remaining = (inner.width as usize).saturating_sub(used);
            if remaining > 0 {
                spans.push(Span::styled(" ".repeat(remaining), Style::default().bg(bg)));
            }
        }

        frame.render_widget(Paragraph::new(Line::from(spans)), row_area);
    }

    // Footer: command count
    if inner.height > 2 {
        let footer_y = inner.y + inner.height - 1;
        let count_text = if palette.query.is_empty() {
            format!(" {} commands", palette.total_commands())
        } else {
            format!(
                " {} / {} commands",
                palette.filtered.len(),
                palette.total_commands()
            )
        };
        let footer_line = Line::from(Span::styled(
            count_text,
            Style::default().fg(theme.panel_border),
        ));
        let footer_area = Rect {
            y: footer_y,
            height: 1,
            ..inner
        };
        frame.render_widget(Paragraph::new(footer_line), footer_area);
    }
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
// Children: build_icon_line(), build_plain_line(), render_inline_input_line().
// Siblings: Tree actions (create/rename) inject inline input lines;
//           delete confirmation is handled by the unified confirm dialog overlay.
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
            if let TreeAction::Creating { input, .. } = action {
                lines.push(render_inline_input_line(&indent, input, area_width, theme));
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

// IMPACT ANALYSIS — render_terminal_tab_bar
// Parents: render_terminal_panel() calls this when tabs exist.
// Children: None — purely visual.
// Siblings: render_terminal_grid() renders the content below the tab bar.

/// Renders the terminal tab bar showing all open tabs.
fn render_terminal_tab_bar(mgr: &TerminalManager, area: Rect, frame: &mut Frame, theme: &Theme) {
    let titles = mgr.tab_titles();
    let active = mgr.active_index();
    let display_offset = mgr.active_display_offset();

    let mut spans: Vec<Span> = Vec::new();
    for (i, title) in titles.iter().enumerate() {
        let label = format!("[{}:{}]", i + 1, title);
        let style = if i == active {
            Style::default()
                .fg(theme.panel_border_active)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default()
                .fg(theme.foreground)
                .add_modifier(Modifier::DIM)
        };
        spans.push(Span::styled(label, style));
        if i + 1 < titles.len() {
            spans.push(Span::raw(" "));
        }
    }

    // Show scroll indicator when not at the bottom.
    if display_offset > 0 {
        let indicator = format!(" [{display_offset} lines up]");
        spans.push(Span::styled(
            indicator,
            Style::default()
                .fg(theme.panel_border_active)
                .add_modifier(Modifier::DIM),
        ));
    }

    let line = Line::from(spans);
    let paragraph = Paragraph::new(line);
    frame.render_widget(paragraph, area);
}

/// Renders the "No files open" message when no buffers exist.
fn render_no_files_message(area: Rect, frame: &mut Frame, theme: &Theme) {
    let text = Line::from(vec![
        Span::styled(
            "No files open",
            Style::default()
                .fg(theme.foreground)
                .add_modifier(Modifier::DIM),
        ),
        Span::styled(
            " -- Select a file from the tree",
            Style::default()
                .fg(theme.foreground)
                .add_modifier(Modifier::DIM),
        ),
    ]);
    let paragraph = Paragraph::new(text).alignment(Alignment::Center);
    let centered_area = Rect {
        y: area.y + area.height / 2,
        height: 1,
        ..area
    };
    frame.render_widget(paragraph, centered_area);
}

/// Renders the "No terminals" message when all tabs are closed.
fn render_no_terminals_message(area: Rect, frame: &mut Frame, theme: &Theme) {
    let text = Line::from(vec![
        Span::styled(
            "No terminals",
            Style::default()
                .fg(theme.foreground)
                .add_modifier(Modifier::DIM),
        ),
        Span::styled(
            " -- Alt+T to create",
            Style::default()
                .fg(theme.foreground)
                .add_modifier(Modifier::DIM),
        ),
    ]);
    let paragraph = Paragraph::new(text).alignment(Alignment::Center);
    let centered_area = Rect {
        y: area.y + area.height / 2,
        height: 1,
        ..area
    };
    frame.render_widget(paragraph, centered_area);
}

// IMPACT ANALYSIS — render_terminal_content
// Parents: render_right_panels() and zoomed view call this.
// Children: convert_ansi_color(), cell_flags_to_modifier(), render_terminal_tab_bar(),
//           render_no_terminals_message().
// Siblings: Terminal panel block is rendered separately — this only fills the inner area.

/// Renders terminal content (tab bar + active tab grid) into the given area.
fn render_terminal_content(mgr: &TerminalManager, area: Rect, frame: &mut Frame, theme: &Theme) {
    if !mgr.has_tabs() {
        render_no_terminals_message(area, frame, theme);
        return;
    }

    // Reserve 1 column on the right for the scrollbar track (like real terminals).
    let scrollbar_width: u16 = 1;
    let content_width = area.width.saturating_sub(scrollbar_width);

    // Split area: 1 row for tab bar, rest for terminal grid.
    let (tab_bar_area, grid_area, scrollbar_area) = if mgr.tab_count() > 0 && area.height > 1 {
        let tab_bar = Rect { height: 1, ..area };
        let grid = Rect {
            y: area.y + 1,
            height: area.height.saturating_sub(1),
            width: content_width,
            ..area
        };
        let scrollbar = Rect {
            x: area.x + content_width,
            y: area.y + 1,
            width: scrollbar_width,
            height: area.height.saturating_sub(1),
        };
        (Some(tab_bar), grid, scrollbar)
    } else {
        let grid = Rect {
            width: content_width,
            ..area
        };
        let scrollbar = Rect {
            x: area.x + content_width,
            width: scrollbar_width,
            ..area
        };
        (None, grid, scrollbar)
    };

    if let Some(tab_bar) = tab_bar_area {
        render_terminal_tab_bar(mgr, tab_bar, frame, theme);
    }

    let tab = match mgr.active_tab() {
        Some(tab) => tab,
        None => return,
    };

    let term = tab.term();

    // Clear the grid and scrollbar areas before direct buffer manipulation.
    // This prevents stale content from persisting when scroll position changes
    // or when the terminal grid has fewer cells than the visible area.
    frame.render_widget(Clear, grid_area);
    frame.render_widget(Clear, scrollbar_area);

    // Render grid content and cursor, then release the borrow on term for scrollbar rendering.
    let display_offset = {
        let content = term.renderable_content();
        let buf = frame.buffer_mut();

        // display_iter returns absolute grid coordinates where line 0 is the
        // top of the current screen. When scrolled, lines start at
        // -display_offset. Convert to viewport-relative row by adding
        // display_offset.
        let offset = content.display_offset as i32;

        for indexed in content.display_iter {
            let point = indexed.point;
            let cell = &indexed.cell;

            // Convert absolute grid line to viewport-relative row.
            let viewport_row = point.line.0 + offset;
            if viewport_row < 0 {
                continue;
            }
            let x = grid_area.x.saturating_add(point.column.0 as u16);
            let y = grid_area.y.saturating_add(viewport_row as u16);

            // Skip cells outside the visible area.
            if x >= grid_area.x + grid_area.width || y >= grid_area.y + grid_area.height {
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

            let mut style = Style::default().fg(fg).bg(bg).add_modifier(modifier);

            // Apply selection highlight (inverted colors).
            if let Some(ref sel_range) = content.selection {
                if sel_range.contains(indexed.point) {
                    style = style.add_modifier(Modifier::REVERSED);
                }
            }

            if let Some(buf_cell) = buf.cell_mut((x, y)) {
                buf_cell.set_char(cell.c);
                buf_cell.set_style(style);
            }
        }

        // Render cursor (only when not scrolled up — cursor is on the live screen).
        if content.cursor.shape != CursorShape::Hidden && offset == 0 {
            let cursor_point = content.cursor.point;
            let cursor_row = cursor_point.line.0;
            if cursor_row >= 0 {
                let cx = grid_area.x.saturating_add(cursor_point.column.0 as u16);
                let cy = grid_area.y.saturating_add(cursor_row as u16);

                if cx < grid_area.x + grid_area.width && cy < grid_area.y + grid_area.height {
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

        content.display_offset
    };

    // Render scrollbar in the reserved right column.
    if scrollbar_area.height > 0 && scrollbar_area.width > 0 {
        let buf = frame.buffer_mut();
        render_terminal_scrollbar(term, display_offset, scrollbar_area, buf, theme);
    }
}

/// Renders a scrollbar in the given scrollbar area.
///
/// The thumb position is proportional to the scroll position within the history.
/// When there is no scrollback history, renders nothing (empty column).
fn render_terminal_scrollbar(
    term: &alacritty_terminal::Term<axe_terminal::event_listener::PtyEventListener>,
    display_offset: usize,
    scrollbar_area: Rect,
    buf: &mut ratatui::buffer::Buffer,
    theme: &Theme,
) {
    use alacritty_terminal::grid::Dimensions;

    let total_lines = term.grid().total_lines();
    let screen_lines = term.grid().screen_lines();
    let max_offset = total_lines.saturating_sub(screen_lines);

    // No scrollback history — nothing to show.
    if max_offset == 0 {
        return;
    }

    let track_height = scrollbar_area.height as usize;
    if track_height == 0 {
        return;
    }

    let scroll_x = scrollbar_area.x;

    // Thumb size: proportional to visible content vs total content, minimum 1 row.
    let thumb_size = ((screen_lines * track_height) / total_lines).max(1);

    // Thumb position: display_offset 0 = at bottom, max_offset = at top.
    // Invert so top of scrollbar = top of history.
    let scroll_fraction = display_offset as f64 / max_offset as f64;
    let thumb_top =
        ((1.0 - scroll_fraction) * (track_height.saturating_sub(thumb_size)) as f64) as usize;

    let track_style = Style::default()
        .fg(theme.foreground)
        .add_modifier(Modifier::DIM);
    let thumb_style = Style::default().fg(theme.panel_border_active);

    for row in 0..track_height {
        let y = scrollbar_area.y + row as u16;
        if let Some(cell) = buf.cell_mut((scroll_x, y)) {
            if row >= thumb_top && row < thumb_top + thumb_size {
                cell.set_char('\u{2588}'); // Full block for thumb
                cell.set_style(thumb_style);
            } else {
                cell.set_char('\u{2502}'); // Thin vertical line for track
                cell.set_style(track_style);
            }
        }
    }
}

/// Returns the inner area of the file tree panel, if visible.
///
/// Used by the main loop to sync `AppState::tree_inner_area` each frame
/// for mouse click detection on tree nodes.
pub fn tree_inner_rect(app: &AppState, area: Rect) -> Option<Rect> {
    if !app.show_tree {
        return None;
    }

    let layout_mgr = LayoutManager {
        show_tree: app.show_tree,
        show_terminal: app.show_terminal,
        tree_width_pct: app.tree_width_pct,
        editor_height_pct: app.editor_height_pct,
    };

    if let Some(ref zoomed) = app.zoomed_panel {
        if matches!(zoomed, FocusTarget::Tree) {
            let vertical =
                Layout::vertical([Constraint::Min(0), Constraint::Length(1)]).split(area);
            let block = panel_block(
                " Files (zoomed) ",
                &app.focus,
                &FocusTarget::Tree,
                &Theme::default(),
                false,
            );
            return Some(block.inner(vertical[0]));
        }
        return None;
    }

    let vertical = Layout::vertical([Constraint::Min(0), Constraint::Length(1)]).split(area);
    let main_area = vertical[0];

    let horizontal = Layout::horizontal([
        Constraint::Percentage(layout_mgr.tree_width_pct),
        Constraint::Percentage(100 - layout_mgr.tree_width_pct),
    ])
    .split(main_area);

    let tree_area = horizontal[0];
    let block = panel_block(
        " Files ",
        &app.focus,
        &FocusTarget::Tree,
        &Theme::default(),
        false,
    );
    Some(block.inner(tree_area))
}

/// Returns the inner area of the terminal panel (excluding tab bar), if terminal is visible.
///
/// Used by the main loop to sync PTY size with panel dimensions.
/// Subtracts 1 row for the tab bar when the terminal manager has tabs.
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

    let has_tabs = app
        .terminal_manager
        .as_ref()
        .is_some_and(|mgr| mgr.has_tabs());

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
            let inner = block.inner(vertical[0]);
            return Some(adjust_terminal_rect(inner, has_tabs));
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
    let inner = term_block.inner(right_split[1]);
    Some(adjust_terminal_rect(inner, has_tabs))
}

/// Computes the editor content area rect (after borders and gutter).
///
/// Used by main.rs to sync `AppState::editor_inner_area` each frame.
/// Returns the screen rectangle for the editor tab bar, or `None` if the tab
/// bar is not visible (single buffer or editor not shown).
///
/// The tab bar occupies the first row of the editor panel inner area when
/// multiple buffers are open.
pub fn editor_tab_bar_rect(app: &AppState, area: Rect) -> Option<Rect> {
    if app.buffer_manager.buffer_count() <= 1 {
        return None;
    }

    let layout_mgr = LayoutManager {
        show_tree: app.show_tree,
        show_terminal: app.show_terminal,
        tree_width_pct: app.tree_width_pct,
        editor_height_pct: app.editor_height_pct,
    };

    // If zoomed to a non-editor panel, editor is not visible.
    if let Some(ref zoomed) = app.zoomed_panel {
        if !matches!(zoomed, FocusTarget::Editor) {
            return None;
        }
        let vertical = Layout::vertical([Constraint::Min(0), Constraint::Length(1)]).split(area);
        let block = panel_block(
            editor_title(app, true),
            &app.focus,
            &FocusTarget::Editor,
            &Theme::default(),
            false,
        );
        let inner = block.inner(vertical[0]);
        if inner.height <= 2 {
            return None;
        }
        return Some(Rect {
            x: inner.x,
            y: inner.y,
            width: inner.width,
            height: 1,
        });
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

    let editor_outer = if layout_mgr.show_terminal {
        let right_split = Layout::vertical([
            Constraint::Percentage(layout_mgr.editor_height_pct),
            Constraint::Percentage(100 - layout_mgr.editor_height_pct),
        ])
        .split(right_area);
        right_split[0]
    } else {
        right_area
    };

    let block = panel_block(
        editor_title(app, false),
        &app.focus,
        &FocusTarget::Editor,
        &Theme::default(),
        false,
    );
    let inner = block.inner(editor_outer);
    if inner.height <= 2 {
        return None;
    }
    Some(Rect {
        x: inner.x,
        y: inner.y,
        width: inner.width,
        height: 1,
    })
}

/// Computes the terminal tab bar rect in screen coordinates.
///
/// Returns the first row inside the terminal panel border where tab labels are
/// rendered. Returns `None` if the terminal is hidden, has no tabs, or is not
/// visible (e.g., another panel is zoomed).
pub fn terminal_tab_bar_rect(app: &AppState, area: Rect) -> Option<Rect> {
    let has_tabs = app
        .terminal_manager
        .as_ref()
        .is_some_and(|mgr| mgr.has_tabs());
    if !has_tabs {
        return None;
    }

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
        if !matches!(zoomed, FocusTarget::Terminal(_)) {
            return None;
        }
        let vertical = Layout::vertical([Constraint::Min(0), Constraint::Length(1)]).split(area);
        let block = panel_block(
            " Terminal (zoomed) ",
            &app.focus,
            &FocusTarget::Terminal(0),
            &Theme::default(),
            false,
        );
        let inner = block.inner(vertical[0]);
        if inner.height < 2 {
            return None;
        }
        return Some(Rect {
            x: inner.x,
            y: inner.y,
            width: inner.width,
            height: 1,
        });
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
    let inner = term_block.inner(right_split[1]);
    if inner.height < 2 {
        return None;
    }
    Some(Rect {
        x: inner.x,
        y: inner.y,
        width: inner.width,
        height: 1,
    })
}

pub fn editor_inner_rect(app: &AppState, area: Rect) -> Option<Rect> {
    let layout_mgr = LayoutManager {
        show_tree: app.show_tree,
        show_terminal: app.show_terminal,
        tree_width_pct: app.tree_width_pct,
        editor_height_pct: app.editor_height_pct,
    };

    // If zoomed to a non-editor panel, editor is not visible.
    if let Some(ref zoomed) = app.zoomed_panel {
        if !matches!(zoomed, FocusTarget::Editor) {
            return None;
        }
        // Editor is zoomed — it fills the main area minus status bar.
        let vertical = Layout::vertical([Constraint::Min(0), Constraint::Length(1)]).split(area);
        let block = panel_block(
            editor_title(app, true),
            &app.focus,
            &FocusTarget::Editor,
            &Theme::default(),
            false,
        );
        let mut inner = block.inner(vertical[0]);
        // Account for tab bar row when multiple buffers are open.
        if app.buffer_manager.buffer_count() > 1 && inner.height > 2 {
            inner.y += 1;
            inner.height = inner.height.saturating_sub(1);
        }
        let gutter_w = app
            .buffer_manager
            .active_buffer()
            .map(|b| gutter_width(b.line_count()))
            .unwrap_or(3);
        return Some(Rect {
            x: inner.x + gutter_w,
            y: inner.y,
            width: inner.width.saturating_sub(gutter_w),
            height: inner.height,
        });
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

    let editor_outer = if layout_mgr.show_terminal {
        let right_split = Layout::vertical([
            Constraint::Percentage(layout_mgr.editor_height_pct),
            Constraint::Percentage(100 - layout_mgr.editor_height_pct),
        ])
        .split(right_area);
        right_split[0]
    } else {
        right_area
    };

    let block = panel_block(
        editor_title(app, false),
        &app.focus,
        &FocusTarget::Editor,
        &Theme::default(),
        false,
    );
    let mut inner = block.inner(editor_outer);
    // Account for tab bar row when multiple buffers are open.
    if app.buffer_manager.buffer_count() > 1 && inner.height > 2 {
        inner.y += 1;
        inner.height = inner.height.saturating_sub(1);
    }
    let gutter_w = app
        .buffer_manager
        .active_buffer()
        .map(|b| gutter_width(b.line_count()))
        .unwrap_or(3);
    Some(Rect {
        x: inner.x + gutter_w,
        y: inner.y,
        width: inner.width.saturating_sub(gutter_w),
        height: inner.height,
    })
}

/// Width reserved for the terminal scrollbar column.
const TERMINAL_SCROLLBAR_WIDTH: u16 = 1;

/// Adjusts a rect for the terminal grid: subtracts 1 row for the tab bar (if needed)
/// and 1 column for the scrollbar.
fn adjust_terminal_rect(rect: Rect, has_tabs: bool) -> Rect {
    let mut r = rect;
    if has_tabs && r.height > 1 {
        r.y += 1;
        r.height = r.height.saturating_sub(1);
    }
    r.width = r.width.saturating_sub(TERMINAL_SCROLLBAR_WIDTH);
    r
}

/// Renders the full IDE interface with conditional panel visibility and a status bar.
pub fn render(app: &AppState, frame: &mut Frame, theme: &Theme) {
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
            FocusTarget::Editor => (editor_title(app, true), FocusTarget::Editor),
            FocusTarget::Terminal(id) => (" Terminal (zoomed) ", FocusTarget::Terminal(*id)),
        };
        let block = panel_block(title, &app.focus, &panel_target, theme, resize_active);
        let inner = block.inner(main_area);
        frame.render_widget(block, main_area);
        match zoomed {
            FocusTarget::Tree => {
                if let Some(ref tree) = app.file_tree {
                    render_tree_content(tree, inner, frame, theme);
                }
            }
            FocusTarget::Terminal(_) => {
                if let Some(ref mgr) = app.terminal_manager {
                    render_terminal_content(mgr, inner, frame, theme);
                }
            }
            FocusTarget::Editor => {
                if let Some(buffer) = app.buffer_manager.active_buffer() {
                    let focused = app.focus == FocusTarget::Editor;
                    let tab_bar = if app.buffer_manager.buffer_count() > 1 {
                        Some((
                            app.buffer_manager.buffers(),
                            app.buffer_manager.active_index(),
                        ))
                    } else {
                        None
                    };
                    render_editor_content(
                        buffer,
                        inner,
                        frame,
                        theme,
                        focused,
                        app.search.as_ref(),
                        tab_bar,
                    );
                } else {
                    render_no_files_message(inner, frame, theme);
                }
            }
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
            theme,
            resize_active,
        );
        let tree_inner = tree_block.inner(tree_area);
        frame.render_widget(tree_block, tree_area);
        if let Some(ref tree) = app.file_tree {
            render_tree_content(tree, tree_inner, frame, theme);
        }

        render_right_panels(app, frame, right_area, &layout_mgr, theme, resize_active);
    } else {
        render_right_panels(app, frame, main_area, &layout_mgr, theme, resize_active);
    }

    // Status bar with hotkey hints
    let status_line = build_status_bar(app, theme);
    let status_bar = Paragraph::new(status_line).style(
        Style::default()
            .bg(theme.status_bar_bg)
            .fg(theme.status_bar_fg),
    );
    frame.render_widget(status_bar, status_area);

    // Overlays (on top of everything)
    if let Some(ref dialog) = app.confirm_dialog {
        render_confirm_dialog(dialog, frame, theme);
    } else if let Some(ref palette) = app.command_palette {
        render_command_palette(palette, frame, theme);
    } else if let Some(ref search) = app.project_search {
        render_project_search(search, frame, theme);
    } else if let Some(ref finder) = app.file_finder {
        render_file_finder(finder, frame, theme);
    } else if app.show_help {
        render_help_overlay(frame, theme);
    }
}

/// Minimum gutter padding (1 space each side of the line number).
const GUTTER_PADDING: u16 = 2;

/// Calculates gutter width: digits needed for the largest line number + padding.
fn gutter_width(line_count: usize) -> u16 {
    let digits = line_count.max(1).ilog10() as u16 + 1;
    digits + GUTTER_PADDING
}

/// Renders the search bar in a 1-row area at the top of the editor content.
///
/// Layout: `Find: [query|] [3 of 17] [Aa] [.*]`
fn render_search_bar(search: &SearchState, area: Rect, frame: &mut Frame, theme: &Theme) {
    if area.width == 0 || area.height == 0 {
        return;
    }

    let bar_bg = theme.status_bar_bg;
    let bar_fg = theme.foreground;
    let dim_fg = theme.status_bar_key;
    let active_fg = theme.search_active_match_bg;

    let label_style = Style::default().fg(dim_fg).bg(bar_bg);
    let query_style = Style::default().fg(bar_fg).bg(bar_bg);
    let count_style = Style::default().fg(dim_fg).bg(bar_bg);

    let case_style = if search.case_sensitive {
        Style::default().fg(active_fg).bg(bar_bg)
    } else {
        Style::default().fg(dim_fg).bg(bar_bg)
    };
    let regex_style = if search.regex_mode {
        Style::default().fg(active_fg).bg(bar_bg)
    } else {
        Style::default().fg(dim_fg).bg(bar_bg)
    };

    let count_display = search.match_count_display();

    let mut spans = vec![
        Span::styled(" Find: ", label_style),
        Span::styled(&search.query, query_style),
        Span::styled("\u{2502}", query_style), // cursor pipe
    ];

    if !count_display.is_empty() {
        spans.push(Span::styled(format!(" {count_display}"), count_style));
    }

    spans.push(Span::styled(" [Aa]", case_style));
    spans.push(Span::styled(" [.*]", regex_style));

    // Pad the rest of the bar with background.
    let used: usize = spans.iter().map(|s| s.content.len()).sum();
    let remaining = (area.width as usize).saturating_sub(used);
    if remaining > 0 {
        spans.push(Span::styled(
            " ".repeat(remaining),
            Style::default().bg(bar_bg),
        ));
    }

    let line = Line::from(spans);
    let paragraph = Paragraph::new(vec![line]).style(Style::default().bg(bar_bg));
    frame.render_widget(paragraph, area);
}

// IMPACT ANALYSIS — render_tab_bar
// Parents: render_editor_content() calls this when multiple buffers are open.
// Children: reads EditorBuffer::file_name() and modified flag for each buffer.
// Siblings: render_search_bar (similar 1-row bar pattern, independent).
/// Renders the buffer tab bar in a 1-row area above the editor content.
///
/// Each tab shows: ` filename.ext ` or ` filename.ext [+] `.
/// Active tab uses the active tab style; inactive tabs use dim style.
fn render_tab_bar(
    buffers: &[EditorBuffer],
    active_index: usize,
    area: Rect,
    frame: &mut Frame,
    theme: &Theme,
) {
    if area.width == 0 || area.height == 0 {
        return;
    }

    let active_style = Style::default()
        .fg(theme.panel_border_active)
        .add_modifier(Modifier::BOLD);
    let inactive_style = Style::default()
        .fg(theme.foreground)
        .add_modifier(Modifier::DIM);

    let mut spans: Vec<Span<'_>> = Vec::new();
    let mut total_width: usize = 0;
    let max_width = area.width as usize;

    for (i, buf) in buffers.iter().enumerate() {
        let name = buf.file_name().unwrap_or("untitled");
        let label = if buf.modified {
            format!("[{}:{}+]", i + 1, name)
        } else {
            format!("[{}:{}]", i + 1, name)
        };

        let tab_width = label.len();
        if total_width + tab_width > max_width {
            break;
        }

        let base_style = if i == active_index {
            active_style
        } else {
            inactive_style
        };
        // Preview buffers use italic to indicate temporary state.
        let style = if buf.is_preview {
            base_style.add_modifier(Modifier::ITALIC)
        } else {
            base_style
        };

        spans.push(Span::styled(label, style));
        total_width += tab_width;

        // Space between tabs.
        if i + 1 < buffers.len() && total_width < max_width {
            spans.push(Span::raw(" "));
            total_width += 1;
        }
    }

    // Fill remaining space with background.
    let remaining = max_width.saturating_sub(total_width);
    if remaining > 0 {
        spans.push(Span::styled(
            " ".repeat(remaining),
            Style::default().bg(theme.tab_bar_bg),
        ));
    }

    let line = Line::from(spans);
    let paragraph = Paragraph::new(vec![line]).style(Style::default().bg(theme.tab_bar_bg));
    frame.render_widget(paragraph, area);
}

// IMPACT ANALYSIS — render_editor_content
// Parents: render_right_panels() calls this with the inner area of the editor block.
// Children: reads EditorBuffer via active_buffer() — cursor, scroll_row, scroll_col.
// Siblings: render_terminal_content (similar pattern, independent).
/// Renders the file content with line numbers, scroll offset, cursor, and
/// current-line highlighting inside the editor panel.
fn render_editor_content(
    buffer: &EditorBuffer,
    area: Rect,
    frame: &mut Frame,
    theme: &Theme,
    editor_focused: bool,
    search: Option<&SearchState>,
    tab_bar: Option<(&[EditorBuffer], usize)>,
) {
    if area.width == 0 || area.height == 0 {
        return;
    }

    // Tab bar: steal 1 row when multiple buffers are open.
    let area = if let Some((buffers, active_idx)) = tab_bar {
        if area.height > 2 {
            let tab_rect = Rect {
                x: area.x,
                y: area.y,
                width: area.width,
                height: 1,
            };
            let rest = Rect {
                x: area.x,
                y: area.y + 1,
                width: area.width,
                height: area.height - 1,
            };
            render_tab_bar(buffers, active_idx, tab_rect, frame, theme);
            rest
        } else {
            area
        }
    } else {
        area
    };

    // If search is active, split off 1 row at the top for the search bar.
    let (search_area, content_area_full) = if search.is_some() && area.height > 1 {
        let search_rect = Rect {
            x: area.x,
            y: area.y,
            width: area.width,
            height: 1,
        };
        let content_rect = Rect {
            x: area.x,
            y: area.y + 1,
            width: area.width,
            height: area.height - 1,
        };
        (Some(search_rect), content_rect)
    } else {
        (None, area)
    };

    if let Some((search_rect, search_state)) = search_area.zip(search) {
        render_search_bar(search_state, search_rect, frame, theme);
    }

    let area = content_area_full;

    let gutter_w = gutter_width(buffer.line_count());
    let content_w = area.width.saturating_sub(gutter_w);

    let gutter_area = Rect {
        x: area.x,
        y: area.y,
        width: gutter_w,
        height: area.height,
    };
    let content_area = Rect {
        x: area.x + gutter_w,
        y: area.y,
        width: content_w,
        height: area.height,
    };

    let gutter_style = Style::default().fg(theme.line_number).bg(theme.gutter_bg);
    let gutter_active_style = Style::default()
        .fg(theme.line_number_active)
        .bg(theme.gutter_bg);

    let visible_lines = area.height as usize;
    let scroll_row = buffer.scroll_row;
    let scroll_col = buffer.scroll_col;
    let cursor_row = buffer.cursor.row;
    let cursor_col = buffer.cursor.col;

    // Render gutter (line numbers) with scroll offset and active line highlight.
    let gutter_lines: Vec<Line<'_>> = (0..visible_lines)
        .map(|i| {
            let file_line = scroll_row + i;
            let line_num = file_line + 1;
            if file_line < buffer.line_count() {
                let style = if file_line == cursor_row && editor_focused {
                    gutter_active_style
                } else {
                    gutter_style
                };
                Line::from(Span::styled(
                    format!("{:>width$} ", line_num, width = (gutter_w - 1) as usize),
                    style,
                ))
            } else {
                Line::from(Span::styled(
                    format!("{:>width$}", "~", width = gutter_w as usize),
                    gutter_style,
                ))
            }
        })
        .collect();

    let gutter_paragraph = Paragraph::new(gutter_lines).style(gutter_style);
    frame.render_widget(gutter_paragraph, gutter_area);

    // Compute normalized selection range (if any).
    let sel_range = buffer.selection.as_ref().and_then(|sel| {
        if sel.is_empty(cursor_row, cursor_col) {
            None
        } else {
            Some(sel.normalized(cursor_row, cursor_col))
        }
    });

    // Render file content with scroll offset, current-line background,
    // syntax highlighting, selection highlight, and search match highlighting.
    let content_style = Style::default().fg(theme.foreground).bg(theme.background);
    let cursor_line_style = Style::default()
        .fg(theme.foreground)
        .bg(theme.cursor_line_bg);
    let selection_style = Style::default().fg(theme.foreground).bg(theme.selection_bg);
    let search_match_style = Style::default()
        .fg(theme.foreground)
        .bg(theme.search_match_bg);
    let search_active_style = Style::default()
        .fg(theme.search_active_match_fg)
        .bg(theme.search_active_match_bg);

    // Fetch syntax highlight data for visible lines.
    let syntax_data = buffer.highlight_range(scroll_row, scroll_row + visible_lines);

    let content_lines: Vec<Line<'_>> = (0..visible_lines)
        .map(|i| {
            let file_line = scroll_row + i;
            let is_cursor_line = file_line == cursor_row && editor_focused;
            let base_bg = if is_cursor_line {
                theme.cursor_line_bg
            } else {
                theme.background
            };
            let base_style = if is_cursor_line {
                cursor_line_style
            } else {
                content_style
            };
            if let Some(rope_line) = buffer.line_at(file_line) {
                let text: String = rope_line.chars().collect();
                // Trim trailing newline for display.
                let trimmed = text.trim_end_matches('\n').trim_end_matches('\r');
                // Apply horizontal scroll and clip to available width.
                let display: String = trimmed
                    .chars()
                    .skip(scroll_col)
                    .take(content_w as usize)
                    .collect();

                // Collect highlight ranges: (col_start, col_end, style) in display coords.
                // Priority: syntax (base fg) < search (bg override) < selection (bg override).
                // Later entries in this vec override earlier ones in the per-char style map.
                let mut highlights: Vec<(usize, usize, Style)> = Vec::new();

                // Syntax highlights (lowest priority — set fg, preserve base bg).
                if let Some(line_hl) = syntax_data.get(i) {
                    for span in line_hl {
                        let hs = span.col_start.saturating_sub(scroll_col);
                        let he = span
                            .col_end
                            .saturating_sub(scroll_col)
                            .min(content_w as usize);
                        if he > hs {
                            let color = theme.syntax_color(span.kind);
                            highlights.push((hs, he, Style::default().fg(color).bg(base_bg)));
                        }
                    }
                }

                // Search match highlights (override syntax bg).
                if let Some(s) = search {
                    for (idx, m) in s.matches.iter().enumerate() {
                        if m.row == file_line {
                            let hs = m.col_start.saturating_sub(scroll_col);
                            let he = m.col_end.saturating_sub(scroll_col).min(content_w as usize);
                            if he > hs {
                                let style = if idx == s.current {
                                    search_active_style
                                } else {
                                    search_match_style
                                };
                                highlights.push((hs, he, style));
                            }
                        }
                    }
                }

                // Selection highlight (highest priority — override bg).
                if let Some((sr, sc, er, ec)) = sel_range {
                    if file_line >= sr && file_line <= er {
                        let line_sel_start = if file_line == sr {
                            sc.saturating_sub(scroll_col)
                        } else {
                            0
                        };
                        let line_sel_end = if file_line == er {
                            ec.saturating_sub(scroll_col)
                        } else {
                            content_w as usize
                        };
                        highlights.push((line_sel_start, line_sel_end, selection_style));
                    }
                }

                if highlights.is_empty() {
                    // No highlights — render normally.
                    let padded = if is_cursor_line {
                        format!("{:<width$}", display, width = content_w as usize)
                    } else {
                        display
                    };
                    return Line::from(Span::styled(padded, base_style));
                }

                // Build spans from highlights. Later highlights override earlier ones.
                let padded = format!("{:<width$}", display, width = content_w as usize);
                let chars: Vec<char> = padded.chars().collect();
                let len = chars.len();

                // Create a per-character style map.
                let mut char_styles: Vec<Style> = vec![base_style; len];
                for (hs, he, style) in &highlights {
                    let start = (*hs).min(len);
                    let end = (*he).min(len);
                    for cs in &mut char_styles[start..end] {
                        *cs = *style;
                    }
                }

                // Compress consecutive same-style chars into spans.
                let mut spans = Vec::new();
                let mut run_start = 0;
                while run_start < len {
                    let run_style = char_styles[run_start];
                    let mut run_end = run_start + 1;
                    while run_end < len && char_styles[run_end] == run_style {
                        run_end += 1;
                    }
                    let s: String = chars[run_start..run_end].iter().collect();
                    spans.push(Span::styled(s, run_style));
                    run_start = run_end;
                }
                Line::from(spans)
            } else {
                Line::from("")
            }
        })
        .collect();

    let content_paragraph = Paragraph::new(content_lines).style(content_style);
    frame.render_widget(content_paragraph, content_area);

    // Render cursor by directly modifying the frame buffer cell at the cursor position.
    if editor_focused {
        let screen_row = cursor_row.saturating_sub(scroll_row);
        let screen_col = cursor_col.saturating_sub(scroll_col);
        if screen_row < visible_lines && screen_col < content_w as usize {
            let cx = content_area.x + screen_col as u16;
            let cy = content_area.y + screen_row as u16;
            if let Some(cell) = frame.buffer_mut().cell_mut((cx, cy)) {
                let fg = cell.fg;
                let bg = cell.bg;
                cell.set_fg(bg);
                cell.set_bg(fg);
            }
        }
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

        let editor_block = panel_block(
            editor_title(app, false),
            &app.focus,
            &FocusTarget::Editor,
            theme,
            resize_active,
        );
        let editor_inner = editor_block.inner(right_split[0]);
        frame.render_widget(editor_block, right_split[0]);
        if let Some(buffer) = app.buffer_manager.active_buffer() {
            let focused = app.focus == FocusTarget::Editor;
            let tab_bar = if app.buffer_manager.buffer_count() > 1 {
                Some((
                    app.buffer_manager.buffers(),
                    app.buffer_manager.active_index(),
                ))
            } else {
                None
            };
            render_editor_content(
                buffer,
                editor_inner,
                frame,
                theme,
                focused,
                app.search.as_ref(),
                tab_bar,
            );
        } else {
            render_no_files_message(editor_inner, frame, theme);
        }

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
        let editor_block = panel_block(
            editor_title(app, false),
            &app.focus,
            &FocusTarget::Editor,
            theme,
            resize_active,
        );
        let editor_inner = editor_block.inner(area);
        frame.render_widget(editor_block, area);
        if let Some(buffer) = app.buffer_manager.active_buffer() {
            let focused = app.focus == FocusTarget::Editor;
            let tab_bar = if app.buffer_manager.buffer_count() > 1 {
                Some((
                    app.buffer_manager.buffers(),
                    app.buffer_manager.active_index(),
                ))
            } else {
                None
            };
            render_editor_content(
                buffer,
                editor_inner,
                frame,
                theme,
                focused,
                app.search.as_ref(),
                tab_bar,
            );
        } else {
            render_no_files_message(editor_inner, frame, theme);
        }
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

        let theme = Theme::default();
        terminal.draw(|frame| render(app, frame, &theme)).unwrap();

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
        let content = render_app_to_string(&app, 80, 60);
        assert!(content.contains("Ctrl+Q"), "expected 'Ctrl+Q' in help");
        assert!(content.contains("Ctrl+B"), "expected 'Ctrl+B' in help");
        assert!(content.contains("Ctrl+T"), "expected 'Ctrl+T' in help");
        assert!(content.contains("Ctrl+H"), "expected 'Ctrl+H' in help");
        assert!(content.contains("Ctrl+R"), "expected 'Ctrl+R' in help");
        assert!(content.contains("Esc"), "expected 'Esc' in help");
    }

    #[test]
    fn render_confirm_dialog_when_quit() {
        let mut app = AppState::new();
        app.confirm_dialog = Some(axe_core::ConfirmDialog::quit());
        let content = render_app_to_string(&app, 80, 24);
        assert!(
            content.contains("Yes"),
            "expected 'Yes' button in quit confirmation overlay"
        );
        assert!(
            content.contains("No"),
            "expected 'No' button in quit confirmation overlay"
        );
        assert!(
            content.contains("Quit"),
            "expected 'Quit' title in quit confirmation overlay"
        );
    }

    #[test]
    fn render_no_confirm_dialog_by_default() {
        let app = AppState::new();
        let content = render_app_to_string(&app, 80, 24);
        assert!(
            !content.contains("[ Yes ]"),
            "confirm dialog should not appear by default"
        );
    }

    #[test]
    fn render_confirm_dialog_close_buffer() {
        let mut app = AppState::new();
        let mut tmp = tempfile::NamedTempFile::new().unwrap();
        std::io::Write::write_all(&mut tmp, b"hello\n").unwrap();
        std::io::Write::flush(&mut tmp).unwrap();
        app.execute(axe_core::Command::OpenFile(tmp.path().to_path_buf()));
        app.buffer_manager.active_buffer_mut().unwrap().modified = true;
        app.confirm_dialog = Some(axe_core::ConfirmDialog::close_buffer("test.txt"));
        let content = render_app_to_string(&app, 80, 24);
        assert!(
            content.contains("Unsaved"),
            "expected 'Unsaved' in close-buffer overlay"
        );
        assert!(
            content.contains("Yes"),
            "expected 'Yes' button in close-buffer overlay"
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

    // --- Editor content rendering tests ---

    #[test]
    fn gutter_width_calculation() {
        assert_eq!(gutter_width(1), 3); // "1 " = 1 digit + 2 padding
        assert_eq!(gutter_width(9), 3); // "9 " = 1 digit + 2 padding
        assert_eq!(gutter_width(10), 4); // "10 " = 2 digits + 2 padding
        assert_eq!(gutter_width(99), 4);
        assert_eq!(gutter_width(100), 5); // "100 " = 3 digits + 2 padding
        assert_eq!(gutter_width(999), 5);
    }

    fn app_with_open_file() -> (AppState, tempfile::TempDir) {
        let tmp = tempfile::TempDir::new().unwrap();
        let file_path = tmp.path().join("test.rs");
        std::fs::write(&file_path, "fn main() {\n    println!(\"hello\");\n}\n").unwrap();

        let mut app = AppState::new_with_root(tmp.path().to_path_buf());
        app.execute(axe_core::Command::OpenFile(file_path));
        (app, tmp)
    }

    #[test]
    fn render_editor_shows_line_numbers() {
        let (app, _tmp) = app_with_open_file();
        let content = render_app_to_string(&app, 100, 24);
        assert!(content.contains('1'), "expected line number 1 in editor");
        assert!(content.contains('2'), "expected line number 2 in editor");
    }

    #[test]
    fn render_editor_shows_file_content() {
        let (app, _tmp) = app_with_open_file();
        let content = render_app_to_string(&app, 100, 24);
        assert!(
            content.contains("fn main()"),
            "expected file content in editor, got: {content}"
        );
    }

    #[test]
    fn status_bar_shows_filename() {
        let (app, _tmp) = app_with_open_file();
        let content = render_app_to_string(&app, 120, 24);
        assert!(
            content.contains("test.rs"),
            "expected filename in status bar"
        );
    }

    #[test]
    fn status_bar_shows_line_count() {
        let (app, _tmp) = app_with_open_file();
        let content = render_app_to_string(&app, 120, 24);
        assert!(content.contains("lines"), "expected 'lines' in status bar");
    }

    #[test]
    fn status_bar_shows_file_type() {
        let (app, _tmp) = app_with_open_file();
        let content = render_app_to_string(&app, 120, 24);
        assert!(
            content.contains("Rust"),
            "expected 'Rust' file type in status bar"
        );
    }

    // --- Tab bar rendering tests ---

    fn app_with_two_files() -> (AppState, tempfile::TempDir) {
        let tmp = tempfile::TempDir::new().unwrap();
        let file1 = tmp.path().join("main.rs");
        let file2 = tmp.path().join("lib.rs");
        std::fs::write(&file1, "fn main() {}\n").unwrap();
        std::fs::write(&file2, "pub fn lib() {}\n").unwrap();

        let mut app = AppState::new_with_root(tmp.path().to_path_buf());
        app.execute(axe_core::Command::OpenFile(file1));
        app.execute(axe_core::Command::OpenFile(file2));
        (app, tmp)
    }

    #[test]
    fn tab_bar_not_shown_with_single_buffer() {
        let (app, _tmp) = app_with_open_file();
        assert_eq!(app.buffer_manager.buffer_count(), 1);
        let content = render_app_to_string(&app, 100, 24);
        // With a single buffer, there should be no tab bar.
        // The file content should still be visible.
        assert!(
            content.contains("fn main()"),
            "expected file content with single buffer"
        );
    }

    #[test]
    fn tab_bar_shown_with_multiple_buffers() {
        let (app, _tmp) = app_with_two_files();
        assert_eq!(app.buffer_manager.buffer_count(), 2);
        let content = render_app_to_string(&app, 100, 24);
        // Both filenames should appear in the tab bar.
        assert!(content.contains("main.rs"), "expected 'main.rs' in tab bar");
        assert!(content.contains("lib.rs"), "expected 'lib.rs' in tab bar");
    }

    #[test]
    fn tab_bar_shows_modified_indicator() {
        let (mut app, _tmp) = app_with_two_files();
        // Modify the active buffer.
        app.execute(axe_core::Command::EditorInsertChar('x'));
        let content = render_app_to_string(&app, 100, 24);
        // Format: "[2:lib.rs+]" — "+" before closing bracket indicates modified.
        assert!(
            content.contains("+]"),
            "expected '+]' modified indicator in tab bar"
        );
    }

    #[test]
    fn render_tab_bar_uses_theme_colors() {
        let buffers = vec![EditorBuffer::new()];
        let theme = Theme::default();
        let backend = TestBackend::new(40, 1);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|frame| {
                let area = Rect {
                    x: 0,
                    y: 0,
                    width: 40,
                    height: 1,
                };
                render_tab_bar(&buffers, 0, area, frame, &theme);
            })
            .unwrap();
        // Verify it renders without panic and the active tab color is applied.
        let buf = terminal.backend().buffer();
        let cell = &buf[(0, 0)];
        // The first (and only) tab is active, so it uses panel_border_active fg.
        assert_eq!(
            cell.fg, theme.panel_border_active,
            "expected panel_border_active on active tab"
        );
    }

    #[test]
    fn editor_inner_rect_accounts_for_tab_bar() {
        let (app, _tmp) = app_with_two_files();
        let area = Rect {
            x: 0,
            y: 0,
            width: 100,
            height: 24,
        };
        let rect_multi = editor_inner_rect(&app, area);

        // Compare with single buffer.
        let (app_single, _tmp2) = app_with_open_file();
        let rect_single = editor_inner_rect(&app_single, area);

        // With multiple buffers, the editor content rect should start 1 row lower.
        assert!(
            rect_multi.is_some() && rect_single.is_some(),
            "expected both rects to be Some"
        );
        let multi = rect_multi.unwrap();
        let single = rect_single.unwrap();
        assert_eq!(
            multi.y,
            single.y + 1,
            "expected tab bar to shift editor content down by 1 row"
        );
        assert_eq!(
            multi.height,
            single.height - 1,
            "expected tab bar to reduce editor content height by 1"
        );
    }

    // --- No files open message tests ---

    #[test]
    fn render_zoomed_editor_shows_message_when_no_buffer() {
        let mut app = AppState::new();
        app.focus = FocusTarget::Editor;
        app.zoomed_panel = Some(FocusTarget::Editor);
        let content = render_app_to_string(&app, 100, 24);
        assert!(
            content.contains("No files open"),
            "expected 'No files open' in zoomed editor with no buffers"
        );
    }

    #[test]
    fn render_unzoomed_editor_shows_message_when_no_buffer() {
        let app = AppState::new();
        let content = render_app_to_string(&app, 100, 24);
        assert!(
            content.contains("No files open"),
            "expected 'No files open' in editor panel with no buffers"
        );
    }

    #[test]
    fn render_zoomed_editor_no_message_when_buffer_exists() {
        let mut app = AppState::new();
        let mut tmp = tempfile::NamedTempFile::new().unwrap();
        std::io::Write::write_all(&mut tmp, b"hello\n").unwrap();
        std::io::Write::flush(&mut tmp).unwrap();
        app.execute(axe_core::Command::OpenFile(tmp.path().to_path_buf()));
        app.focus = FocusTarget::Editor;
        app.zoomed_panel = Some(FocusTarget::Editor);
        let content = render_app_to_string(&app, 100, 24);
        assert!(
            !content.contains("No files open"),
            "expected no 'No files open' when a buffer is open"
        );
    }

    #[test]
    fn help_lines_contain_tab_keybindings() {
        let has_next_tab = HELP_LINES.iter().any(|(k, _)| *k == "Alt+]/[");
        let has_close_tab = HELP_LINES.iter().any(|(k, _)| *k == "Alt+W / Ctrl+W");
        assert!(has_next_tab, "expected 'Alt+]/[' in HELP_LINES");
        assert!(has_close_tab, "expected 'Alt+W / Ctrl+W' in HELP_LINES");
    }

    #[test]
    fn terminal_tab_bar_rect_returns_some_when_tabs_exist() {
        let mut app = AppState::new();
        let cwd = std::env::current_dir().unwrap();
        let mut mgr = axe_terminal::TerminalManager::new();
        mgr.spawn_tab(80, 24, &cwd).unwrap();
        mgr.spawn_tab(80, 24, &cwd).unwrap();
        app.terminal_manager = Some(mgr);

        let area = Rect::new(0, 0, 120, 40);
        let result = terminal_tab_bar_rect(&app, area);
        assert!(result.is_some(), "expected Some when terminal has tabs");
        let rect = result.unwrap();
        assert_eq!(rect.height, 1, "tab bar should be 1 row tall");
    }

    #[test]
    fn terminal_tab_bar_rect_returns_none_when_no_tabs() {
        let app = AppState::new();
        // No terminal manager at all.
        let area = Rect::new(0, 0, 120, 40);
        let result = terminal_tab_bar_rect(&app, area);
        assert!(result.is_none(), "expected None when no terminal manager");
    }

    /// Verifies that the terminal grid area is explicitly cleared before
    /// rendering content, so stale characters from a previous frame do not
    /// persist when cells have no terminal output.
    #[test]
    fn terminal_grid_area_cleared_before_content_render() {
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        let theme = Theme::default();

        let mut app = AppState::new();
        let cwd = std::env::current_dir().unwrap();
        let mut mgr = axe_terminal::TerminalManager::new();
        mgr.spawn_tab(80, 24, &cwd).unwrap();
        app.terminal_manager = Some(mgr);

        // First pass: pre-fill the terminal grid region with stale "X" chars
        // to simulate leftover content from a previous frame.
        terminal
            .draw(|frame| {
                let buf = frame.buffer_mut();
                // Fill entire buffer with 'X' to simulate stale content.
                for y in 0..24u16 {
                    for x in 0..80u16 {
                        if let Some(cell) = buf.cell_mut((x, y)) {
                            cell.set_char('X');
                        }
                    }
                }
            })
            .unwrap();

        // Second pass: render the actual app. The terminal grid area should be
        // cleared — no 'X' chars should remain in the terminal grid region.
        terminal.draw(|frame| render(&app, frame, &theme)).unwrap();

        // Find the terminal grid area by checking where 'X' persists.
        // After a proper clear + render, the terminal grid cells should NOT
        // contain 'X' (they should be space/empty from the clear).
        let buf = terminal.backend().buffer();

        // The terminal panel occupies the bottom-right. Check the interior
        // cells (skip borders). With default layout, terminal starts roughly
        // at the bottom half of the right panel.
        let mut stale_x_count = 0;
        for y in 0..buf.area().height {
            for x in 0..buf.area().width {
                if let Some(cell) = buf.cell((x, y)) {
                    // Only count 'X' in the lower portion (terminal area).
                    // The terminal grid is roughly in the bottom-right.
                    if cell.symbol() == "X" && y > buf.area().height / 2 {
                        stale_x_count += 1;
                    }
                }
            }
        }

        assert_eq!(
            stale_x_count, 0,
            "Stale 'X' characters found in terminal grid area — grid not cleared before render"
        );
    }

    /// Verifies that the terminal scrollbar area is cleared before rendering,
    /// preventing stale scrollbar artifacts when scroll position changes.
    #[test]
    fn terminal_scrollbar_area_cleared_before_render() {
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        let theme = Theme::default();

        let mut app = AppState::new();
        let cwd = std::env::current_dir().unwrap();
        let mut mgr = axe_terminal::TerminalManager::new();
        mgr.spawn_tab(80, 24, &cwd).unwrap();
        app.terminal_manager = Some(mgr);

        // Pre-fill buffer with stale content.
        terminal
            .draw(|frame| {
                let buf = frame.buffer_mut();
                for y in 0..24u16 {
                    for x in 0..80u16 {
                        if let Some(cell) = buf.cell_mut((x, y)) {
                            cell.set_char('X');
                        }
                    }
                }
            })
            .unwrap();

        // Render the app. The scrollbar column should not retain 'X'.
        terminal.draw(|frame| render(&app, frame, &theme)).unwrap();

        let buf = terminal.backend().buffer();
        // The scrollbar is the rightmost column of the terminal panel.
        // Check all cells in the lower-right region for stale 'X'.
        let mut stale_in_scrollbar = 0;
        for y in (buf.area().height / 2)..buf.area().height {
            // The scrollbar column is 1 column before the right border of the terminal panel.
            // With default layout (tree 20%, editor 50%), the right border is at col 79.
            // The scrollbar is at col 78.
            let x = buf.area().width - 2; // Just inside the right border.
            if let Some(cell) = buf.cell((x, y)) {
                if cell.symbol() == "X" {
                    stale_in_scrollbar += 1;
                }
            }
        }

        assert_eq!(
            stale_in_scrollbar, 0,
            "Stale 'X' characters found in scrollbar area — scrollbar not cleared before render"
        );
    }

    #[test]
    fn terminal_tab_bar_rect_returns_none_when_terminal_hidden() {
        let mut app = AppState::new();
        let cwd = std::env::current_dir().unwrap();
        let mut mgr = axe_terminal::TerminalManager::new();
        mgr.spawn_tab(80, 24, &cwd).unwrap();
        mgr.spawn_tab(80, 24, &cwd).unwrap();
        app.terminal_manager = Some(mgr);
        app.show_terminal = false;

        let area = Rect::new(0, 0, 120, 40);
        let result = terminal_tab_bar_rect(&app, area);
        assert!(result.is_none(), "expected None when terminal is hidden");
    }
}
