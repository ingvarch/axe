use ratatui::layout::{Alignment, Constraint, Flex, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Clear, Paragraph};
use ratatui::Frame;

use axe_core::completion::{self, CompletionState};
use axe_core::project_search::{DisplayItem, SearchField};
use axe_core::{AppState, CommandPalette, FileFinder, GoToLineDialog, ProjectSearch};
use axe_editor::EditorBuffer;

use crate::editor_panel::{DIAGNOSTIC_GUTTER_WIDTH, DIFF_GUTTER_WIDTH, GUTTER_PADDING};
use crate::theme::Theme;

/// Number of columns in the help overlay layout.
pub(crate) const HELP_COLUMNS: usize = 3;
/// Width of the key column within each help column.
const HELP_KEY_COL_WIDTH: usize = 20;
/// Width of a single help column (key + description + padding).
pub(crate) const HELP_SINGLE_COL_WIDTH: u16 = 38;
/// Gap between help columns.
pub(crate) const HELP_COL_GAP: u16 = 2;
/// Horizontal padding inside overlay border.
pub(crate) const HELP_INNER_PAD: u16 = 1;
/// Extra vertical rows: 1 top padding + 1 blank before footer + 1 footer.
const HELP_VERTICAL_EXTRA: u16 = 3;

/// A single keybinding entry in the help overlay.
pub(crate) struct HelpEntry {
    /// Fallback key (e.g. F1, F2) -- shown first when present.
    pub(crate) fallback_key: Option<&'static str>,
    /// Primary keybinding (e.g. Ctrl+Shift+P).
    pub(crate) primary_key: &'static str,
    /// Description of what the keybinding does.
    pub(crate) description: &'static str,
}

/// A titled section of keybinding entries.
pub(crate) struct HelpSection {
    pub(crate) title: &'static str,
    pub(crate) entries: &'static [HelpEntry],
}

pub(crate) const HELP_GENERAL: HelpSection = HelpSection {
    title: "General",
    entries: &[
        HelpEntry {
            fallback_key: None,
            primary_key: "Ctrl+Q",
            description: "Quit",
        },
        HelpEntry {
            fallback_key: None,
            primary_key: "Alt+1",
            description: "Focus Files",
        },
        HelpEntry {
            fallback_key: None,
            primary_key: "Alt+2",
            description: "Focus Editor",
        },
        HelpEntry {
            fallback_key: None,
            primary_key: "Alt+3",
            description: "Focus Terminal",
        },
        HelpEntry {
            fallback_key: None,
            primary_key: "Ctrl+B",
            description: "Toggle file tree",
        },
        HelpEntry {
            fallback_key: None,
            primary_key: "Ctrl+T",
            description: "Toggle terminal",
        },
        HelpEntry {
            fallback_key: None,
            primary_key: "Ctrl+R",
            description: "Resize mode",
        },
        HelpEntry {
            fallback_key: None,
            primary_key: "Alt+Z",
            description: "Zoom panel",
        },
        HelpEntry {
            fallback_key: None,
            primary_key: "Click panel",
            description: "Focus panel",
        },
        HelpEntry {
            fallback_key: None,
            primary_key: "Drag border",
            description: "Resize panel",
        },
    ],
};

pub(crate) const HELP_TREE: HelpSection = HelpSection {
    title: "Tree",
    entries: &[
        HelpEntry {
            fallback_key: None,
            primary_key: "\u{2191}/\u{2193}",
            description: "Navigate tree",
        },
        HelpEntry {
            fallback_key: None,
            primary_key: "Enter",
            description: "Expand/collapse dir",
        },
        HelpEntry {
            fallback_key: None,
            primary_key: "\u{2190}/\u{2192}",
            description: "Collapse/expand",
        },
        HelpEntry {
            fallback_key: None,
            primary_key: "Home/End",
            description: "First/last item",
        },
        HelpEntry {
            fallback_key: None,
            primary_key: "Ctrl+Shift+G",
            description: "Toggle ignored files",
        },
        HelpEntry {
            fallback_key: None,
            primary_key: "Ctrl+I",
            description: "Toggle file icons",
        },
        HelpEntry {
            fallback_key: None,
            primary_key: "n",
            description: "New file",
        },
        HelpEntry {
            fallback_key: None,
            primary_key: "N",
            description: "New directory",
        },
        HelpEntry {
            fallback_key: None,
            primary_key: "r",
            description: "Rename",
        },
        HelpEntry {
            fallback_key: None,
            primary_key: "d",
            description: "Delete",
        },
        HelpEntry {
            fallback_key: None,
            primary_key: "Shift+\u{2190}/\u{2192}",
            description: "Scroll horizontally",
        },
    ],
};

pub(crate) const HELP_TABS: HelpSection = HelpSection {
    title: "Tabs",
    entries: &[
        HelpEntry {
            fallback_key: None,
            primary_key: "Alt+T",
            description: "New tab (terminal)",
        },
        HelpEntry {
            fallback_key: None,
            primary_key: "Alt+W / Ctrl+W",
            description: "Close tab",
        },
        HelpEntry {
            fallback_key: None,
            primary_key: "Alt+]/[",
            description: "Next/prev tab",
        },
    ],
};

pub(crate) const HELP_EDITOR: HelpSection = HelpSection {
    title: "Editor",
    entries: &[
        HelpEntry {
            fallback_key: None,
            primary_key: "Ctrl+G",
            description: "Go to line",
        },
        HelpEntry {
            fallback_key: None,
            primary_key: "Ctrl+F",
            description: "Find in file",
        },
        HelpEntry {
            fallback_key: None,
            primary_key: "Shift+Arrows",
            description: "Select text",
        },
        HelpEntry {
            fallback_key: None,
            primary_key: "Ctrl+A",
            description: "Select all",
        },
        HelpEntry {
            fallback_key: None,
            primary_key: "Ctrl+C",
            description: "Copy",
        },
        HelpEntry {
            fallback_key: None,
            primary_key: "Ctrl+X",
            description: "Cut",
        },
        HelpEntry {
            fallback_key: None,
            primary_key: "Ctrl+V",
            description: "Paste",
        },
        HelpEntry {
            fallback_key: None,
            primary_key: "Ctrl+Z",
            description: "Undo",
        },
        HelpEntry {
            fallback_key: None,
            primary_key: "Ctrl+Shift+Z",
            description: "Redo",
        },
        HelpEntry {
            fallback_key: None,
            primary_key: "Ctrl+Y",
            description: "Redo",
        },
        HelpEntry {
            fallback_key: None,
            primary_key: "Ctrl+S",
            description: "Save",
        },
        HelpEntry {
            fallback_key: None,
            primary_key: "Ctrl+P",
            description: "Open file finder",
        },
        HelpEntry {
            fallback_key: Some("F1"),
            primary_key: "Ctrl+Shift+P",
            description: "Command palette",
        },
        HelpEntry {
            fallback_key: Some("F2"),
            primary_key: "Ctrl+Shift+F",
            description: "Find in project",
        },
        HelpEntry {
            fallback_key: Some("F3"),
            primary_key: "Alt+/",
            description: "Code completion",
        },
        HelpEntry {
            fallback_key: None,
            primary_key: "Alt+.",
            description: "Next diagnostic",
        },
        HelpEntry {
            fallback_key: None,
            primary_key: "Alt+,",
            description: "Prev diagnostic",
        },
        HelpEntry {
            fallback_key: Some("F12"),
            primary_key: "",
            description: "Go to definition",
        },
        HelpEntry {
            fallback_key: Some("Shift+F12"),
            primary_key: "",
            description: "Find references",
        },
        HelpEntry {
            fallback_key: Some("F4"),
            primary_key: "Ctrl+Shift+K",
            description: "Show hover info",
        },
        HelpEntry {
            fallback_key: None,
            primary_key: "Ctrl+Shift+I",
            description: "Format document",
        },
    ],
};

pub(crate) const HELP_TERMINAL: HelpSection = HelpSection {
    title: "Terminal",
    entries: &[
        HelpEntry {
            fallback_key: None,
            primary_key: "Shift+PgUp",
            description: "Scroll up",
        },
        HelpEntry {
            fallback_key: None,
            primary_key: "Shift+PgDn",
            description: "Scroll down",
        },
        HelpEntry {
            fallback_key: None,
            primary_key: "Shift+Home",
            description: "Scroll to top",
        },
        HelpEntry {
            fallback_key: None,
            primary_key: "Shift+End",
            description: "Scroll to bottom",
        },
        HelpEntry {
            fallback_key: None,
            primary_key: "Mouse drag",
            description: "Select text (copy)",
        },
    ],
};

pub(crate) const HELP_CLOSE: HelpSection = HelpSection {
    title: "",
    entries: &[
        HelpEntry {
            fallback_key: None,
            primary_key: "Ctrl+H",
            description: "Toggle this help",
        },
        HelpEntry {
            fallback_key: None,
            primary_key: "Esc",
            description: "Close overlay",
        },
    ],
};

pub(crate) const HELP_SECTIONS: &[&HelpSection] = &[
    &HELP_GENERAL,
    &HELP_TREE,
    &HELP_TABS,
    &HELP_EDITOR,
    &HELP_TERMINAL,
    &HELP_CLOSE,
];

/// Formats a keybinding entry for display.
/// Fallback key appears first, separated by " / " from the primary key.
pub(crate) fn format_help_key(entry: &HelpEntry) -> String {
    match (entry.fallback_key, entry.primary_key) {
        (Some(fallback), primary) if !primary.is_empty() => {
            format!("{fallback} / {primary}")
        }
        (Some(fallback), _) => fallback.to_string(),
        (None, primary) => primary.to_string(),
    }
}

/// Calculates help overlay dimensions based on content size, clamped to screen.
/// Returns (width, height).
pub(crate) fn help_overlay_dimensions(area: Rect, max_col_lines: u16) -> (u16, u16) {
    // Width: 3 columns + gaps + inner padding + border (2)
    let content_w =
        HELP_SINGLE_COL_WIDTH * HELP_COLUMNS as u16 + HELP_COL_GAP * (HELP_COLUMNS as u16 - 1);
    let width = (content_w + HELP_INNER_PAD * 2 + 2).min(area.width.saturating_sub(2));

    // Height: tallest column + extra rows + border (2)
    let height = (max_col_lines + HELP_VERTICAL_EXTRA + 2).min(area.height.saturating_sub(2));

    (width, height)
}

/// A renderable line in a help column.
enum HelpLine {
    /// Section title.
    Header(String),
    /// Horizontal rule under a section title.
    Separator,
    /// Blank spacer between sections.
    Spacer,
    /// Key-description pair.
    Entry(String, String),
}

/// Converts a section into renderable lines (header + separator + entries).
fn section_to_lines(section: &HelpSection) -> Vec<HelpLine> {
    let mut lines = Vec::new();
    if !section.title.is_empty() {
        lines.push(HelpLine::Header(section.title.to_string()));
        lines.push(HelpLine::Separator);
    }
    for entry in section.entries {
        lines.push(HelpLine::Entry(
            format_help_key(entry),
            entry.description.to_string(),
        ));
    }
    lines
}

/// Calculates line count for a section including header and separator.
fn section_line_count(section: &HelpSection) -> usize {
    // header + separator = 2 lines when title is present
    let header = if section.title.is_empty() { 0 } else { 2 };
    header + section.entries.len()
}

/// Distributes sections into columns, keeping each section intact.
/// Uses largest-first bin-packing: sorts sections by size descending, then
/// assigns each to the shortest column. This produces balanced columns.
/// A blank spacer line is added between sections in the same column.
fn distribute_sections_into_columns() -> Vec<Vec<HelpLine>> {
    let mut columns: Vec<Vec<HelpLine>> = (0..HELP_COLUMNS).map(|_| Vec::new()).collect();
    let mut col_heights: Vec<usize> = vec![0; HELP_COLUMNS];

    // Sort sections by size descending for better packing
    let mut sorted: Vec<&HelpSection> = HELP_SECTIONS.to_vec();
    sorted.sort_by_key(|s| std::cmp::Reverse(section_line_count(s)));

    for section in sorted {
        // Find the shortest column
        let min_col = col_heights
            .iter()
            .enumerate()
            .min_by_key(|(_, h)| **h)
            .map(|(i, _)| i)
            .unwrap_or(0);

        // Add spacer if column already has content
        if !columns[min_col].is_empty() {
            columns[min_col].push(HelpLine::Spacer);
            col_heights[min_col] += 1;
        }

        let lines = section_to_lines(section);
        col_heights[min_col] += lines.len();
        columns[min_col].extend(lines);
    }

    columns
}

/// Renders the help overlay centered on the screen.
pub(crate) fn render_help_overlay(frame: &mut Frame, theme: &Theme) {
    let area = frame.area();

    let columns = distribute_sections_into_columns();
    let max_col_lines = columns.iter().map(|c| c.len() as u16).max().unwrap_or(0);

    let (overlay_width, overlay_height) = help_overlay_dimensions(area, max_col_lines);

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
    frame.render_widget(block, overlay_area);

    let header_style = Style::default()
        .fg(theme.panel_border_active)
        .add_modifier(Modifier::BOLD);
    let separator_style = Style::default()
        .fg(theme.panel_border)
        .add_modifier(Modifier::DIM);
    let key_style = Style::default()
        .fg(theme.panel_border_active)
        .add_modifier(Modifier::BOLD);
    let desc_style = Style::default().fg(theme.foreground);

    // Render each column with proper gaps
    for (col_idx, col_lines) in columns.iter().enumerate() {
        let col_x =
            inner.x + HELP_INNER_PAD + (col_idx as u16) * (HELP_SINGLE_COL_WIDTH + HELP_COL_GAP);
        let col_w = HELP_SINGLE_COL_WIDTH.min(inner.width.saturating_sub(col_x - inner.x));
        let col_rect = Rect {
            x: col_x,
            y: inner.y + 1, // 1 row top padding
            width: col_w,
            height: inner.height.saturating_sub(HELP_VERTICAL_EXTRA),
        };

        // Build separator: "─" repeated to fill the column width (with indent)
        let rule_len = col_w.saturating_sub(2) as usize;
        let rule_str: String = format!(" {}", "\u{2500}".repeat(rule_len));

        let rendered: Vec<Line> = col_lines
            .iter()
            .map(|line| match line {
                HelpLine::Header(title) => {
                    Line::from(Span::styled(format!(" {title}"), header_style))
                }
                HelpLine::Separator => Line::from(Span::styled(rule_str.clone(), separator_style)),
                HelpLine::Spacer => Line::from(""),
                HelpLine::Entry(key, desc) => Line::from(vec![
                    Span::styled(format!("  {key:<HELP_KEY_COL_WIDTH$}"), key_style),
                    Span::styled(desc.clone(), desc_style),
                ]),
            })
            .collect();

        let paragraph = Paragraph::new(rendered).alignment(Alignment::Left);
        frame.render_widget(paragraph, col_rect);
    }

    // Footer: "Esc to close" centered at the bottom
    let footer_area = Rect {
        x: inner.x,
        y: inner.y + inner.height.saturating_sub(1),
        width: inner.width,
        height: 1,
    };
    let footer = Paragraph::new(Line::from(Span::styled(
        "Esc to close",
        Style::default()
            .fg(theme.foreground)
            .add_modifier(Modifier::DIM),
    )))
    .alignment(Alignment::Center);
    frame.render_widget(footer, footer_area);
}

/// Minimum width of the confirmation dialog in columns.
const CONFIRM_DIALOG_MIN_WIDTH: u16 = 30;
/// Padding for button labels (spaces around text).
const CONFIRM_BUTTON_PADDING: usize = 2;

/// Renders a centered confirmation dialog with navigable [Yes] / [No] buttons.
pub(crate) fn render_confirm_dialog(
    dialog: &axe_core::ConfirmDialog,
    frame: &mut Frame,
    theme: &Theme,
) {
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
pub(crate) fn render_project_search(search: &ProjectSearch, frame: &mut Frame, theme: &Theme) {
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

/// Maximum number of visible items in the completion popup.
const COMPLETION_MAX_VISIBLE: usize = 10;
/// Minimum width of the completion popup.
const COMPLETION_MIN_WIDTH: u16 = 20;
/// Maximum width of the completion popup.
const COMPLETION_MAX_WIDTH: u16 = 60;

/// Renders the completion popup at the cursor position in the editor.
///
/// The popup is non-modal and appears below the cursor line (or above
/// if there's not enough space below). Items show [kind] label detail.
pub(crate) fn render_completion_popup(
    comp: &CompletionState,
    buffer: &EditorBuffer,
    app: &AppState,
    frame: &mut Frame,
    theme: &Theme,
) {
    if comp.filtered.is_empty() {
        return;
    }

    let Some((editor_x, editor_y, editor_w, editor_h)) = app.editor_inner_area else {
        return;
    };

    // Calculate line number gutter width for the current buffer.
    let line_count = buffer.line_count();
    let digits = if line_count == 0 {
        1
    } else {
        (line_count as f64).log10().floor() as u16 + 1
    };
    let gutter_width = digits + GUTTER_PADDING + DIAGNOSTIC_GUTTER_WIDTH + DIFF_GUTTER_WIDTH;

    // Screen position relative to editor inner area.
    let cursor_screen_row = comp.trigger_row.saturating_sub(buffer.scroll_row) as u16;
    let cursor_screen_col = (comp.trigger_col.saturating_sub(buffer.scroll_col)) as u16;

    // Absolute screen position.
    let popup_x = (editor_x + gutter_width + cursor_screen_col).min(
        editor_x
            .saturating_add(editor_w)
            .saturating_sub(COMPLETION_MIN_WIDTH),
    );
    let below_y = editor_y + cursor_screen_row + 1; // +1 to be below cursor line

    let visible_count = comp.filtered.len().min(COMPLETION_MAX_VISIBLE);
    let popup_height = visible_count as u16 + 2; // +2 for borders

    // Calculate width from items.
    let max_label_width = comp
        .filtered
        .iter()
        .filter_map(|&idx| comp.items.get(idx))
        .map(|item| {
            let detail_len = item.detail.as_ref().map(|d| d.len() + 2).unwrap_or(0);
            // "kw " + label + "  " + detail
            3 + item.label.len() + detail_len
        })
        .max()
        .unwrap_or(COMPLETION_MIN_WIDTH as usize);
    let popup_width = (max_label_width as u16 + 2) // +2 for borders
        .clamp(COMPLETION_MIN_WIDTH, COMPLETION_MAX_WIDTH)
        .min(editor_w);

    // Place below cursor if enough room, otherwise above.
    let space_below = (editor_y + editor_h).saturating_sub(below_y);
    let above_y = editor_y
        .saturating_add(cursor_screen_row)
        .saturating_sub(popup_height);
    let popup_y = if space_below >= popup_height {
        below_y
    } else {
        above_y
    };

    let popup_area = Rect::new(popup_x, popup_y, popup_width, popup_height);

    // Ensure scroll_offset keeps selected item visible.
    let scroll_offset = if comp.selected < comp.scroll_offset {
        comp.selected
    } else if comp.selected >= comp.scroll_offset + COMPLETION_MAX_VISIBLE {
        comp.selected + 1 - COMPLETION_MAX_VISIBLE
    } else {
        comp.scroll_offset
    };

    frame.render_widget(Clear, popup_area);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .style(Style::default().bg(theme.overlay_bg).fg(theme.foreground))
        .border_style(Style::default().fg(theme.overlay_border));
    let inner = block.inner(popup_area);
    frame.render_widget(block, popup_area);

    // Render visible items.
    let inner_width = inner.width as usize;
    for (i, &item_idx) in comp
        .filtered
        .iter()
        .skip(scroll_offset)
        .take(COMPLETION_MAX_VISIBLE)
        .enumerate()
    {
        if i as u16 >= inner.height {
            break;
        }
        let Some(item) = comp.items.get(item_idx) else {
            continue;
        };

        let is_selected = (scroll_offset + i) == comp.selected;
        let base_style = if is_selected {
            Style::default()
                .bg(theme.tree_selection_bg)
                .fg(theme.foreground)
        } else {
            Style::default().bg(theme.overlay_bg).fg(theme.foreground)
        };

        let icon = completion::kind_icon(item.kind);
        let icon_style = base_style.add_modifier(Modifier::DIM);

        let mut spans = vec![
            Span::styled(icon, icon_style),
            Span::styled(" ", base_style),
            Span::styled(&item.label, base_style),
        ];

        // Add detail if present and there's room.
        if let Some(ref detail) = item.detail {
            let used = 3 + item.label.len();
            let remaining = inner_width.saturating_sub(used + 2);
            if remaining > 0 {
                let truncated: String = detail.chars().take(remaining).collect();
                spans.push(Span::styled("  ", base_style));
                spans.push(Span::styled(
                    truncated,
                    base_style.add_modifier(Modifier::DIM),
                ));
            }
        }

        // Pad to full width.
        let content_len: usize = spans.iter().map(|s| s.content.len()).sum();
        if content_len < inner_width {
            spans.push(Span::styled(
                " ".repeat(inner_width - content_len),
                base_style,
            ));
        }

        let line = Line::from(spans);
        let line_area = Rect::new(inner.x, inner.y + i as u16, inner.width, 1);
        frame.render_widget(Paragraph::new(line), line_area);
    }
}

/// Maximum width for the hover tooltip.
const HOVER_MAX_WIDTH: u16 = 80;
/// Maximum height for the hover tooltip.
const HOVER_MAX_HEIGHT: u16 = 20;
/// Minimum width for the hover tooltip.
const HOVER_MIN_WIDTH: u16 = 20;

/// Renders the hover tooltip positioned near the cursor.
pub(crate) fn render_hover_tooltip(
    hover: &axe_core::hover::HoverInfo,
    buffer: &EditorBuffer,
    app: &AppState,
    frame: &mut Frame,
    theme: &Theme,
) {
    if hover.lines.is_empty() {
        return;
    }

    let Some((editor_x, editor_y, editor_w, editor_h)) = app.editor_inner_area else {
        return;
    };

    // Calculate gutter width (same as completion popup).
    let line_count = buffer.line_count();
    let digits = if line_count == 0 {
        1
    } else {
        (line_count as f64).log10().floor() as u16 + 1
    };
    let gutter_width = digits + GUTTER_PADDING + DIAGNOSTIC_GUTTER_WIDTH + DIFF_GUTTER_WIDTH;

    // Screen position relative to editor inner area.
    let cursor_screen_row = hover.trigger_row.saturating_sub(buffer.scroll_row) as u16;
    let cursor_screen_col = hover.trigger_col.saturating_sub(buffer.scroll_col) as u16;

    // Calculate content dimensions.
    let content_width: u16 = hover
        .lines
        .iter()
        .map(|line| line.spans.iter().map(|s| s.text.len()).sum::<usize>() as u16)
        .max()
        .unwrap_or(0);

    let popup_width = (content_width + 2) // +2 for borders
        .clamp(HOVER_MIN_WIDTH, HOVER_MAX_WIDTH)
        .min(editor_w);

    let content_height = hover.lines.len() as u16;
    let popup_height = (content_height + 2) // +2 for borders
        .min(HOVER_MAX_HEIGHT)
        .min(editor_h);

    // Position: prefer above cursor line; if no space, place below.
    let popup_x = (editor_x + gutter_width + cursor_screen_col).min(
        editor_x
            .saturating_add(editor_w)
            .saturating_sub(popup_width),
    );

    let space_above = cursor_screen_row;
    let above_y = editor_y
        .saturating_add(cursor_screen_row)
        .saturating_sub(popup_height);
    let below_y = editor_y + cursor_screen_row + 1;

    let popup_y = if space_above >= popup_height {
        above_y
    } else {
        below_y
    };

    let popup_area = Rect::new(popup_x, popup_y, popup_width, popup_height);

    frame.render_widget(Clear, popup_area);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .style(Style::default().bg(theme.overlay_bg).fg(theme.foreground))
        .border_style(Style::default().fg(theme.overlay_border));
    let inner = block.inner(popup_area);
    frame.render_widget(block, popup_area);

    // Render hover lines.
    let inner_width = inner.width as usize;
    let visible_lines = (inner.height as usize).min(hover.lines.len());

    for (i, hover_line) in hover.lines.iter().take(visible_lines).enumerate() {
        if i as u16 >= inner.height {
            break;
        }

        let code_block_bg = ratatui::style::Color::Rgb(
            theme.overlay_bg.to_string().len() as u8, // dummy — use slightly modified bg
            0,
            0,
        );
        // Use a slightly lighter background for code blocks.
        let code_bg = match theme.overlay_bg {
            ratatui::style::Color::Rgb(r, g, b) => ratatui::style::Color::Rgb(
                r.saturating_add(15),
                g.saturating_add(15),
                b.saturating_add(15),
            ),
            _ => theme.overlay_bg,
        };
        let _ = code_block_bg; // avoid unused warning

        let base_style = if hover_line.is_code_block {
            Style::default().bg(code_bg).fg(theme.foreground)
        } else {
            Style::default().bg(theme.overlay_bg).fg(theme.foreground)
        };

        let spans: Vec<Span> = if hover_line.spans.is_empty() {
            // Separator line.
            vec![Span::styled(
                "\u{2500}".repeat(inner_width.min(popup_width as usize)),
                Style::default()
                    .bg(theme.overlay_bg)
                    .fg(theme.overlay_border)
                    .add_modifier(Modifier::DIM),
            )]
        } else {
            hover_line
                .spans
                .iter()
                .map(|span| {
                    let mut style = base_style;
                    if span.bold {
                        style = style.add_modifier(Modifier::BOLD);
                    }
                    if span.italic {
                        style = style.add_modifier(Modifier::ITALIC);
                    }
                    if span.code {
                        style = style.bg(code_bg);
                    }
                    Span::styled(&span.text, style)
                })
                .collect()
        };

        // Pad to full width.
        let content_len: usize = spans.iter().map(|s| s.content.len()).sum();
        let mut all_spans = spans;
        if content_len < inner_width {
            all_spans.push(Span::styled(
                " ".repeat(inner_width - content_len),
                base_style,
            ));
        }

        let line = Line::from(all_spans);
        let line_area = Rect::new(inner.x, inner.y + i as u16, inner.width, 1);
        frame.render_widget(Paragraph::new(line), line_area);
    }
}

/// Width percentage for the location list overlay.
const LOCATION_LIST_WIDTH_PCT: u16 = 60;
/// Height percentage for the location list overlay.
const LOCATION_LIST_HEIGHT_PCT: u16 = 50;
/// Minimum width for the location list overlay.
const LOCATION_LIST_MIN_WIDTH: u16 = 40;
/// Minimum height for the location list overlay.
const LOCATION_LIST_MIN_HEIGHT: u16 = 8;

/// Renders the location list overlay (definition/references results).
pub(crate) fn render_location_list(
    loc_list: &axe_core::location_list::LocationList,
    frame: &mut Frame,
    theme: &Theme,
) {
    let area = frame.area();

    let overlay_width = (area.width * LOCATION_LIST_WIDTH_PCT / 100)
        .max(LOCATION_LIST_MIN_WIDTH)
        .min(area.width.saturating_sub(4));
    let overlay_height = (area.height * LOCATION_LIST_HEIGHT_PCT / 100)
        .max(LOCATION_LIST_MIN_HEIGHT)
        .min(area.height.saturating_sub(2));

    let horizontal = Layout::horizontal([Constraint::Length(overlay_width)])
        .flex(Flex::Center)
        .split(area);
    let vertical = Layout::vertical([Constraint::Length(overlay_height)])
        .flex(Flex::Center)
        .split(horizontal[0]);
    let overlay_area = vertical[0];

    frame.render_widget(Clear, overlay_area);

    let title = format!(" {} ({}) ", loc_list.title, loc_list.items.len());
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

    if inner.width == 0 || inner.height == 0 {
        return;
    }

    let visible_count = inner.height as usize;

    // Adjust scroll_offset to keep selected item visible.
    // We use a mutable reference pattern here — but LocationList is passed as &,
    // so we compute the effective scroll offset locally.
    let scroll_offset = if loc_list.selected < loc_list.scroll_offset {
        loc_list.selected
    } else if loc_list.selected >= loc_list.scroll_offset + visible_count {
        loc_list
            .selected
            .saturating_sub(visible_count.saturating_sub(1))
    } else {
        loc_list.scroll_offset
    };

    for (i, item) in loc_list
        .items
        .iter()
        .skip(scroll_offset)
        .take(visible_count)
        .enumerate()
    {
        let idx = scroll_offset + i;
        let is_selected = idx == loc_list.selected;

        let location = format!("{}:{}", item.display_path, item.line + 1);
        let text_preview = if item.line_text.is_empty() {
            String::new()
        } else {
            format!("  {}", item.line_text)
        };

        let style = if is_selected {
            Style::default()
                .fg(theme.overlay_bg)
                .bg(theme.panel_border_active)
        } else {
            Style::default().fg(theme.foreground)
        };

        let path_style = if is_selected {
            style.add_modifier(Modifier::BOLD)
        } else {
            Style::default()
                .fg(theme.panel_border_active)
                .add_modifier(Modifier::BOLD)
        };

        // Truncate to fit width.
        let max_width = inner.width as usize;
        let combined = format!("{location}{text_preview}");
        let display = if combined.len() > max_width {
            combined[..max_width].to_string()
        } else {
            combined.clone()
        };

        let line = if is_selected {
            // Pad to full width for selection highlight.
            let padded = format!("{display:<width$}", width = max_width);
            Line::from(Span::styled(padded, style))
        } else {
            let loc_len = location.len().min(max_width);
            let remaining = max_width.saturating_sub(loc_len);
            let preview = if text_preview.len() > remaining {
                &text_preview[..remaining]
            } else {
                &text_preview
            };
            Line::from(vec![
                Span::styled(&location[..loc_len], path_style),
                Span::styled(preview.to_string(), style),
            ])
        };

        let line_area = Rect::new(inner.x, inner.y + i as u16, inner.width, 1);
        frame.render_widget(Paragraph::new(line), line_area);
    }
}

/// Renders the fuzzy file finder overlay centered on the screen.
pub(crate) fn render_file_finder(finder: &FileFinder, frame: &mut Frame, theme: &Theme) {
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
pub(crate) fn render_command_palette(palette: &CommandPalette, frame: &mut Frame, theme: &Theme) {
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

/// Width of the Go to Line dialog in columns.
const GO_TO_LINE_WIDTH: u16 = 30;
/// Height of the Go to Line dialog in rows (border + input + footer).
const GO_TO_LINE_HEIGHT: u16 = 5;

/// Renders the Go to Line dialog centered on the screen.
pub(crate) fn render_go_to_line(dialog: &GoToLineDialog, frame: &mut Frame, theme: &Theme) {
    let area = frame.area();

    let overlay_width = GO_TO_LINE_WIDTH.min(area.width.saturating_sub(4));
    let overlay_height = GO_TO_LINE_HEIGHT.min(area.height.saturating_sub(2));

    let horizontal = Layout::horizontal([Constraint::Length(overlay_width)])
        .flex(Flex::Center)
        .split(area);
    let vertical = Layout::vertical([Constraint::Length(overlay_height)])
        .flex(Flex::Center)
        .split(horizontal[0]);
    let overlay_area = vertical[0];

    frame.render_widget(Clear, overlay_area);

    let block = Block::default()
        .title(" Go to Line ")
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

    // Input line: "> {input}|"
    let input_line = Line::from(vec![
        Span::styled(
            " > ",
            Style::default()
                .fg(theme.panel_border_active)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(&dialog.input, Style::default().fg(theme.foreground)),
        Span::styled("|", Style::default().fg(theme.panel_border_active)),
    ]);
    let input_area = Rect { height: 1, ..inner };
    frame.render_widget(Paragraph::new(input_line), input_area);

    // Footer: "Line 1..{max_lines}"
    if inner.height > 1 {
        let footer_text = format!(" Line 1..{}", dialog.max_lines);
        let footer_line = Line::from(Span::styled(
            footer_text,
            Style::default().fg(theme.panel_border),
        ));
        let footer_area = Rect {
            y: inner.y + inner.height.saturating_sub(1),
            height: 1,
            ..inner
        };
        frame.render_widget(Paragraph::new(footer_line), footer_area);
    }
}
