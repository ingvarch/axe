use ratatui::layout::{Alignment, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

use axe_core::search::SearchField;
use axe_core::{AppState, InlayHint, SearchState};
use axe_editor::diagnostic::{diagnostics_for_line, most_severe_for_line, DiagnosticSeverity};
use axe_editor::EditorBuffer;

use crate::inlay::{format_inlay_label, hint_shift_before, paint_line_with_hints, VisualCell};
use crate::theme::Theme;

/// Minimum gutter padding (1 space each side of the line number).
pub(crate) const GUTTER_PADDING: u16 = 2;
/// Width of the diagnostic indicator column in the gutter.
pub(crate) const DIAGNOSTIC_GUTTER_WIDTH: u16 = 2;
/// Width of the git diff indicator column in the gutter.
pub(crate) const DIFF_GUTTER_WIDTH: u16 = 1;
/// Width reserved for the editor scrollbar column.
pub(crate) const EDITOR_SCROLLBAR_WIDTH: u16 = 1;

/// Returns the editor panel title, including a modified indicator if needed.
pub(crate) fn editor_title(app: &AppState, zoomed: bool) -> &'static str {
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

/// Returns the theme color for a diagnostic severity level.
pub(crate) fn diagnostic_color(severity: DiagnosticSeverity, theme: &Theme) -> Color {
    match severity {
        DiagnosticSeverity::Error => theme.diagnostic_error,
        DiagnosticSeverity::Warning => theme.diagnostic_warning,
        DiagnosticSeverity::Info => theme.diagnostic_info,
        DiagnosticSeverity::Hint => theme.diagnostic_hint,
    }
}

/// Calculates gutter width: diagnostic column + digits + padding + diff indicator.
pub(crate) fn gutter_width(line_count: usize) -> u16 {
    let digits = line_count.max(1).ilog10() as u16 + 1;
    DIAGNOSTIC_GUTTER_WIDTH + digits + GUTTER_PADDING + DIFF_GUTTER_WIDTH
}

/// Returns the number of rows the search bar needs (1 for find-only, 2 with replace).
pub(crate) fn search_bar_rows(search: &SearchState) -> u16 {
    if search.replace_visible {
        2
    } else {
        1
    }
}

/// Renders the search bar in 1 or 2 rows at the top of the editor content.
///
/// Row 1: `Find:    [query|] [3 of 17] [Aa] [.*]`
/// Row 2 (when replace visible): `Replace: [text|]  [Replace] [All]`
fn render_search_bar(search: &SearchState, area: Rect, frame: &mut Frame, theme: &Theme) {
    if area.width == 0 || area.height == 0 {
        return;
    }

    let bar_bg = theme.status_bar_bg;
    let bar_fg = theme.foreground;
    let dim_fg = theme.status_bar_key;
    let active_fg = theme.search_active_match_bg;

    let find_active = search.active_field == SearchField::Find;

    let find_label_style = if find_active || !search.replace_visible {
        Style::default().fg(bar_fg).bg(bar_bg)
    } else {
        Style::default().fg(dim_fg).bg(bar_bg)
    };
    let query_style = if find_active || !search.replace_visible {
        Style::default().fg(bar_fg).bg(bar_bg)
    } else {
        Style::default().fg(dim_fg).bg(bar_bg)
    };
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

    // Use consistent label width so fields align when replace is visible.
    let label = if search.replace_visible {
        " Find:    "
    } else {
        " Find: "
    };
    let cursor_char = if find_active || !search.replace_visible {
        "\u{2502}"
    } else {
        ""
    };

    let mut spans = vec![
        Span::styled(label, find_label_style),
        Span::styled(&search.query, query_style),
        Span::styled(cursor_char, query_style),
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

    let find_row = Rect { height: 1, ..area };
    let line = Line::from(spans);
    let paragraph = Paragraph::new(vec![line]).style(Style::default().bg(bar_bg));
    frame.render_widget(paragraph, find_row);

    // Row 2: Replace field (only when visible and area has room).
    if search.replace_visible && area.height > 1 {
        let replace_active = search.active_field == SearchField::Replace;
        let replace_label_style = if replace_active {
            Style::default().fg(bar_fg).bg(bar_bg)
        } else {
            Style::default().fg(dim_fg).bg(bar_bg)
        };
        let replace_query_style = if replace_active {
            Style::default().fg(bar_fg).bg(bar_bg)
        } else {
            Style::default().fg(dim_fg).bg(bar_bg)
        };
        let replace_cursor = if replace_active { "\u{2502}" } else { "" };

        let mut rspans = vec![
            Span::styled(" Replace: ", replace_label_style),
            Span::styled(&search.replace_query, replace_query_style),
            Span::styled(replace_cursor, replace_query_style),
            Span::styled("  [Replace] [All]", Style::default().fg(dim_fg).bg(bar_bg)),
        ];

        let rused: usize = rspans.iter().map(|s| s.content.len()).sum();
        let rremaining = (area.width as usize).saturating_sub(rused);
        if rremaining > 0 {
            rspans.push(Span::styled(
                " ".repeat(rremaining),
                Style::default().bg(bar_bg),
            ));
        }

        let replace_row = Rect {
            y: area.y + 1,
            height: 1,
            ..area
        };
        let rline = Line::from(rspans);
        let rparagraph = Paragraph::new(vec![rline]).style(Style::default().bg(bar_bg));
        frame.render_widget(rparagraph, replace_row);
    }
}

// IMPACT ANALYSIS — render_tab_bar
// Parents: render_editor_content() calls this when multiple buffers are open.
// Children: reads EditorBuffer::file_name() and modified flag for each buffer.
// Siblings: render_search_bar (similar 1-row bar pattern, independent).
/// Renders the buffer tab bar in a 1-row area above the editor content.
///
/// Each tab shows: ` filename.ext ` or ` filename.ext [+] `.
/// Active tab uses the active tab style; inactive tabs use dim style.
pub(crate) fn render_tab_bar(
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

/// Result of expanding tab characters in a line.
///
/// Contains the expanded string and a mapping from original char indices
/// to display column positions, used to remap syntax highlight spans,
/// cursor, and selection ranges.
pub(crate) struct ExpandedLine {
    /// The line with tabs replaced by spaces (aligned to tab stops).
    pub(crate) text: String,
    /// Maps original char index to the display column where that char starts.
    /// Length is `original_char_count + 1` — the last entry is the total display width.
    pub(crate) char_to_col: Vec<usize>,
}

/// Expands tab characters to spaces aligned to tab stops.
///
/// Each `\t` is replaced by 1..tab_size spaces so the next character lands on
/// the next tab-stop boundary (column divisible by `tab_size`).
pub(crate) fn expand_tabs(line: &str, tab_size: usize) -> ExpandedLine {
    let tab_size = tab_size.max(1);
    let mut text = String::with_capacity(line.len());
    let mut char_to_col = Vec::with_capacity(line.len() + 1);
    let mut col = 0;
    for ch in line.chars() {
        char_to_col.push(col);
        if ch == '\t' {
            let spaces = tab_size - (col % tab_size);
            for _ in 0..spaces {
                text.push(' ');
            }
            col += spaces;
        } else {
            text.push(ch);
            col += 1;
        }
    }
    char_to_col.push(col);
    ExpandedLine { text, char_to_col }
}

/// Converts a rope char offset to a display column using the char-to-col mapping.
///
/// If `char_idx` is beyond the mapping, returns the total display width (last entry).
pub(crate) fn char_to_display_col(char_to_col: &[usize], char_idx: usize) -> usize {
    if char_idx >= char_to_col.len() {
        *char_to_col.last().unwrap_or(&0)
    } else {
        char_to_col[char_idx]
    }
}

/// ASCII logo for the startup screen.
const AXE_LOGO: &[&str] = &[
    r" @@@@@@   @@@  @@@  @@@@@@@@",
    r"@@@@@@@@  @@@  @@@  @@@@@@@@",
    r"@@!  @@@  @@!  !@@  @@!",
    r"!@!  @!@  !@!  @!!  !@!",
    r"@!@!@!@!   !@@!@!   @!!!:!",
    r"!!!@!!!!    @!!!    !!!!!:",
    r"!!:  !!!   !: :!!   !!:",
    r":!:  !:!  :!:  !:!  :!:",
    r"::   :::   ::  :::   :: ::::",
    r" :   : :   :   ::   : :: ::",
];

/// Keyboard shortcuts shown on the startup screen.
const STARTUP_SHORTCUTS: &[(&str, &str)] = &[
    ("Ctrl+P", "Open file finder"),
    ("Ctrl+Shift+P", "Command palette"),
    ("F2 / Ctrl+Shift+F", "Find in project"),
    ("Ctrl+B", "Toggle file tree"),
    ("Ctrl+T", "Toggle terminal"),
    ("F1", "Help"),
    ("Ctrl+Q", "Quit"),
];

// IMPACT ANALYSIS — render_startup_screen
// Parents: render() and render_right_panels() call this when no buffers are open.
// Children: None — leaf rendering function.
// Siblings: render_no_terminals_message() in terminal_panel.rs — independent.
/// Renders the startup/welcome screen when no buffers are open.
///
/// Shows an ASCII logo, version string, and keyboard shortcuts.
/// Gracefully degrades for small terminal sizes.
pub(crate) fn render_startup_screen(area: Rect, frame: &mut Frame, theme: &Theme, version: &str) {
    if area.width == 0 || area.height == 0 {
        return;
    }

    let height = area.height as usize;

    // Graceful degradation for small terminals.
    if height < 3 {
        // Fallback: single dim message.
        let text = Line::from(Span::styled(
            "No files open",
            Style::default()
                .fg(theme.foreground)
                .add_modifier(Modifier::DIM),
        ));
        let paragraph = Paragraph::new(text).alignment(Alignment::Center);
        let centered = Rect {
            y: area.y + area.height / 2,
            height: 1,
            ..area
        };
        frame.render_widget(paragraph, centered);
        return;
    }

    let logo_height = AXE_LOGO.len();
    let version_line = if version.is_empty() { 0 } else { 1 };
    let shortcuts_height = STARTUP_SHORTCUTS.len();

    // Determine what to show based on available height.
    let show_version = height >= logo_height + 2 + version_line;
    let show_shortcuts = height >= logo_height + 2 + version_line + 1 + shortcuts_height;

    let total_height = logo_height
        + if show_version { 2 } else { 0 }
        + if show_shortcuts {
            1 + shortcuts_height
        } else {
            0
        };

    let start_y = area.y + (area.height.saturating_sub(total_height as u16)) / 2;
    let mut y = start_y;

    let logo_style = Style::default().fg(theme.panel_border_active);

    // Render logo as a left-aligned block centered as a whole.
    let logo_max_width = AXE_LOGO.iter().map(|l| l.len()).max().unwrap_or(0) as u16;
    let logo_x = area.x + area.width.saturating_sub(logo_max_width) / 2;
    let logo_w = logo_max_width.min(area.width);

    for line in AXE_LOGO {
        if y >= area.y + area.height {
            break;
        }
        let paragraph =
            Paragraph::new(Line::from(Span::styled(*line, logo_style))).alignment(Alignment::Left);
        frame.render_widget(
            paragraph,
            Rect {
                x: logo_x,
                y,
                width: logo_w,
                height: 1,
            },
        );
        y += 1;
    }

    // Render version.
    if show_version {
        y += 1; // blank line
        let version_text = if version.starts_with('v') {
            version.to_string()
        } else {
            format!("v{version}")
        };
        let version_style = Style::default()
            .fg(theme.foreground)
            .add_modifier(Modifier::DIM);
        let paragraph = Paragraph::new(Line::from(Span::styled(version_text, version_style)))
            .alignment(Alignment::Center);
        frame.render_widget(
            paragraph,
            Rect {
                y,
                height: 1,
                ..area
            },
        );
        y += 1;
    }

    // Render shortcuts as an aligned table (same style as help overlay).
    if show_shortcuts {
        y += 1; // blank line

        let key_style = Style::default()
            .fg(theme.panel_border_active)
            .add_modifier(Modifier::BOLD);
        let desc_style = Style::default()
            .fg(theme.foreground)
            .add_modifier(Modifier::DIM);

        // Fixed column width for key names, matching help overlay style.
        const KEY_COL_WIDTH: usize = 20;

        // Build all shortcut lines as left-aligned paragraphs, then center
        // the block as a whole by computing the x offset once.
        let max_line_len = STARTUP_SHORTCUTS
            .iter()
            .map(|(_, d)| KEY_COL_WIDTH + d.len())
            .max()
            .unwrap_or(0);
        let block_x = area.x + area.width.saturating_sub(max_line_len as u16) / 2;
        let block_w = (max_line_len as u16).min(area.width);

        for (key, desc) in STARTUP_SHORTCUTS {
            if y >= area.y + area.height {
                break;
            }
            let line = Line::from(vec![
                Span::styled(format!("{key:<KEY_COL_WIDTH$}"), key_style),
                Span::styled(*desc, desc_style),
            ]);
            let paragraph = Paragraph::new(line).alignment(Alignment::Left);
            frame.render_widget(
                paragraph,
                Rect {
                    x: block_x,
                    y,
                    width: block_w,
                    height: 1,
                },
            );
            y += 1;
        }
    }
}

/// Renders a vertical scrollbar in the given 1-column-wide area.
///
/// - `total_lines`: total number of lines in the content
/// - `visible_lines`: how many lines fit in the viewport
/// - `scroll_offset`: current scroll position (0 = top of content visible)
/// - `area`: 1-column-wide `Rect` for the scrollbar
/// - `buf`: frame buffer
/// - `theme`: for styling
///
/// When the content fits within the viewport (`total_lines <= visible_lines`),
/// renders nothing.
pub(crate) fn render_scrollbar(
    total_lines: usize,
    visible_lines: usize,
    scroll_offset: usize,
    area: Rect,
    buf: &mut ratatui::buffer::Buffer,
    theme: &Theme,
) {
    if total_lines <= visible_lines {
        return;
    }

    let track_height = area.height as usize;
    if track_height == 0 {
        return;
    }

    let scroll_x = area.x;

    // Thumb size: proportional to visible content vs total content, minimum 1 row.
    let thumb_size = ((visible_lines * track_height) / total_lines).max(1);

    // Thumb position: scroll_offset 0 = thumb at top, max = thumb at bottom.
    let max_offset = total_lines.saturating_sub(visible_lines);
    let scroll_fraction = if max_offset == 0 {
        0.0
    } else {
        scroll_offset as f64 / max_offset as f64
    };
    let thumb_top = (scroll_fraction * (track_height.saturating_sub(thumb_size)) as f64) as usize;

    let track_style = Style::default()
        .fg(theme.foreground)
        .add_modifier(Modifier::DIM);
    let thumb_style = Style::default().fg(theme.panel_border_active);

    for row in 0..track_height {
        let y = area.y + row as u16;
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

// IMPACT ANALYSIS — render_editor_content
// Parents: render_right_panels() calls this with the inner area of the editor block.
// Children: reads EditorBuffer via active_buffer() — cursor, scroll_row, scroll_col.
// Siblings: render_terminal_content (similar pattern, independent).
/// Renders the file content with line numbers, scroll offset, cursor, and
/// current-line highlighting inside the editor panel.
#[allow(clippy::too_many_arguments)]
pub(crate) fn render_editor_content(
    buffer: &EditorBuffer,
    area: Rect,
    frame: &mut Frame,
    theme: &Theme,
    editor_focused: bool,
    search: Option<&SearchState>,
    tab_bar: Option<(&[EditorBuffer], usize)>,
    inlay_hints: &[InlayHint],
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

    // If search is active, split off rows at the top for the search bar.
    let search_rows = search.map(search_bar_rows).unwrap_or(0);
    let (search_area, content_area_full) = if search_rows > 0 && area.height > search_rows {
        let search_rect = Rect {
            x: area.x,
            y: area.y,
            width: area.width,
            height: search_rows,
        };
        let content_rect = Rect {
            x: area.x,
            y: area.y + search_rows,
            width: area.width,
            height: area.height - search_rows,
        };
        (Some(search_rect), content_rect)
    } else {
        (None, area)
    };

    if let Some((search_rect, search_state)) = search_area.zip(search) {
        render_search_bar(search_state, search_rect, frame, theme);
    }

    let area = content_area_full;

    // Reserve 1 column on the right for the scrollbar (rendered only when needed).
    let scrollbar_area = Rect {
        x: area.x + area.width.saturating_sub(EDITOR_SCROLLBAR_WIDTH),
        y: area.y,
        width: EDITOR_SCROLLBAR_WIDTH,
        height: area.height,
    };
    let area_without_scrollbar = Rect {
        x: area.x,
        y: area.y,
        width: area.width.saturating_sub(EDITOR_SCROLLBAR_WIDTH),
        height: area.height,
    };

    let gutter_w = gutter_width(buffer.line_count());
    let content_w = area_without_scrollbar.width.saturating_sub(gutter_w);

    let gutter_area = Rect {
        x: area_without_scrollbar.x,
        y: area_without_scrollbar.y,
        width: gutter_w,
        height: area_without_scrollbar.height,
    };
    let content_area = Rect {
        x: area_without_scrollbar.x + gutter_w,
        y: area_without_scrollbar.y,
        width: content_w,
        height: area_without_scrollbar.height,
    };

    let gutter_style = Style::default().fg(theme.line_number).bg(theme.gutter_bg);
    let gutter_active_style = Style::default()
        .fg(theme.line_number_active)
        .bg(theme.gutter_bg);

    let visible_lines = area.height as usize;
    let scroll_row = buffer.scroll_row;
    let scroll_col_char = buffer.scroll_col;
    let cursor_row = buffer.cursor().row;
    let cursor_col_char = buffer.cursor().col;
    let tab_size = buffer.tab_size();

    // Width available for line numbers (total gutter - diagnostic column - diff column - trailing space).
    let line_num_width = (gutter_w - DIAGNOSTIC_GUTTER_WIDTH - DIFF_GUTTER_WIDTH - 1) as usize;

    // Render gutter (diagnostic icon + line numbers + diff indicator) with scroll offset.
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

                // Diagnostic icon for this line.
                let diag_span =
                    if let Some(sev) = most_severe_for_line(buffer.diagnostics(), file_line) {
                        let color = diagnostic_color(sev, theme);
                        Span::styled("\u{25CF} ", Style::default().fg(color).bg(theme.gutter_bg))
                    } else {
                        Span::styled("  ", style)
                    };

                // Git diff indicator for this line.
                let diff_span = if let Some(kind) =
                    axe_editor::diff::diff_kind_for_line(buffer.diff_hunks(), file_line)
                {
                    let color = match kind {
                        axe_editor::DiffHunkKind::Added => theme.diff_added,
                        axe_editor::DiffHunkKind::Modified => theme.diff_modified,
                        axe_editor::DiffHunkKind::Deleted => theme.diff_deleted,
                    };
                    let ch = match kind {
                        axe_editor::DiffHunkKind::Added | axe_editor::DiffHunkKind::Modified => {
                            "\u{258E}"
                        }
                        axe_editor::DiffHunkKind::Deleted => "\u{2581}",
                    };
                    Span::styled(ch, Style::default().fg(color).bg(theme.gutter_bg))
                } else {
                    Span::styled(" ", gutter_style)
                };

                Line::from(vec![
                    diag_span,
                    Span::styled(
                        format!("{:>width$} ", line_num, width = line_num_width),
                        style,
                    ),
                    diff_span,
                ])
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
    let sel_range = buffer.selection().and_then(|sel| {
        if sel.is_empty(cursor_row, cursor_col_char) {
            None
        } else {
            Some(sel.normalized(cursor_row, cursor_col_char))
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
                // Expand tabs to spaces aligned to tab stops and build
                // a char→display-column mapping for position conversions.
                let expanded = expand_tabs(trimmed, tab_size);
                let c2d = &expanded.char_to_col;
                let scroll_col = char_to_display_col(c2d, scroll_col_char);
                // Apply horizontal scroll and clip to available width.
                let display: String = expanded
                    .text
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
                        let hs =
                            char_to_display_col(c2d, span.col_start).saturating_sub(scroll_col);
                        let he = char_to_display_col(c2d, span.col_end)
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
                            let hs =
                                char_to_display_col(c2d, m.col_start).saturating_sub(scroll_col);
                            let he = char_to_display_col(c2d, m.col_end)
                                .saturating_sub(scroll_col)
                                .min(content_w as usize);
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
                            char_to_display_col(c2d, sc).saturating_sub(scroll_col)
                        } else {
                            0
                        };
                        let line_sel_end = if file_line == er {
                            char_to_display_col(c2d, ec).saturating_sub(scroll_col)
                        } else {
                            content_w as usize
                        };
                        highlights.push((line_sel_start, line_sel_end, selection_style));
                    }
                }

                // Collect diagnostic underline ranges for this line.
                let diag_underlines: Vec<(usize, usize, Color)> =
                    diagnostics_for_line(buffer.diagnostics(), file_line)
                        .map(|d| {
                            let hs =
                                char_to_display_col(c2d, d.col_start).saturating_sub(scroll_col);
                            let he = char_to_display_col(c2d, d.col_end)
                                .saturating_sub(scroll_col)
                                .min(content_w as usize);
                            let color = diagnostic_color(d.severity, theme);
                            (hs, he, color)
                        })
                        .filter(|(hs, he, _)| he > hs)
                        .collect();

                // Collect inlay hints for this line, converting their logical
                // column to the scrolled display column used by the renderer.
                // Hints past the viewport are dropped.
                let line_hint_positions: Vec<(usize, String)> = inlay_hints
                    .iter()
                    .filter(|h| h.row == file_line)
                    .filter_map(|h| {
                        let display_col =
                            char_to_display_col(c2d, h.col).saturating_sub(scroll_col);
                        if display_col > content_w as usize {
                            return None;
                        }
                        Some((display_col, format_inlay_label(h)))
                    })
                    .collect();

                if highlights.is_empty()
                    && diag_underlines.is_empty()
                    && line_hint_positions.is_empty()
                {
                    // No highlights or diagnostics — render normally.
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

                // Diagnostic underlines — additive (preserve existing fg/bg, add underline).
                for (hs, he, color) in &diag_underlines {
                    let start = (*hs).min(len);
                    let end = (*he).min(len);
                    for cs in &mut char_styles[start..end] {
                        *cs = cs
                            .add_modifier(Modifier::UNDERLINED)
                            .underline_color(*color);
                    }
                }

                // Inlay hints: merge virtual cells into the rendered line via
                // the pure `paint_line_with_hints` helper. Each hint cell
                // inherits a dim/italic style from the theme. Char cells keep
                // the per-character styles we just computed.
                let hint_style = Style::default()
                    .fg(theme.inlay_hint)
                    .bg(base_bg)
                    .add_modifier(Modifier::ITALIC);

                let (final_chars, final_styles): (Vec<char>, Vec<Style>) =
                    if line_hint_positions.is_empty() {
                        (chars, char_styles)
                    } else {
                        let padded_str: String = chars.iter().collect();
                        let visuals = paint_line_with_hints(&padded_str, &line_hint_positions);
                        let mut merged_chars = Vec::with_capacity(visuals.len());
                        let mut merged_styles = Vec::with_capacity(visuals.len());
                        let mut char_idx = 0usize;
                        for cell in &visuals {
                            match cell {
                                VisualCell::Char(c) => {
                                    merged_chars.push(*c);
                                    merged_styles.push(char_styles[char_idx]);
                                    char_idx += 1;
                                }
                                VisualCell::Hint(c) => {
                                    merged_chars.push(*c);
                                    merged_styles.push(hint_style);
                                }
                            }
                        }
                        // Clip to content_w in case hints extended the line.
                        if merged_chars.len() > content_w as usize {
                            merged_chars.truncate(content_w as usize);
                            merged_styles.truncate(content_w as usize);
                        }
                        (merged_chars, merged_styles)
                    };

                let final_len = final_chars.len();

                // Compress consecutive same-style chars into spans.
                let mut spans = Vec::new();
                let mut run_start = 0;
                while run_start < final_len {
                    let run_style = final_styles[run_start];
                    let mut run_end = run_start + 1;
                    while run_end < final_len && final_styles[run_end] == run_style {
                        run_end += 1;
                    }
                    let s: String = final_chars[run_start..run_end].iter().collect();
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

    // Render editor scrollbar when content exceeds viewport.
    if buffer.line_count() > visible_lines {
        render_scrollbar(
            buffer.line_count(),
            visible_lines,
            scroll_row,
            scrollbar_area,
            frame.buffer_mut(),
            theme,
        );
    }

    // Render cursor by directly modifying the frame buffer cell at the cursor position.
    // Convert char-based cursor and scroll positions to display columns via tab expansion,
    // then shift right by any inlay hint cells that appear before the cursor on that line.
    if editor_focused {
        let screen_row = cursor_row.saturating_sub(scroll_row);
        let cursor_display_col = if let Some(cursor_line) = buffer.line_at(cursor_row) {
            let line_text: String = cursor_line.chars().collect();
            let trimmed = line_text.trim_end_matches('\n').trim_end_matches('\r');
            let expanded = expand_tabs(trimmed, tab_size);
            let scroll_col = char_to_display_col(&expanded.char_to_col, scroll_col_char);
            let base = char_to_display_col(&expanded.char_to_col, cursor_col_char)
                .saturating_sub(scroll_col);

            // Build the same hint positions the renderer used for this row and
            // shift the cursor past any hint cells preceding it.
            let cursor_line_hints: Vec<(usize, String)> = inlay_hints
                .iter()
                .filter(|h| h.row == cursor_row)
                .map(|h| {
                    let display_col = char_to_display_col(&expanded.char_to_col, h.col)
                        .saturating_sub(scroll_col);
                    (display_col, format_inlay_label(h))
                })
                .collect();
            // A hint anchored exactly at the cursor column appears BEFORE the
            // cursor's character, so it shifts the cursor; use `< base + 1`
            // semantics via the helper which accepts `<= display_col`.
            let shift = hint_shift_before(&cursor_line_hints, base.saturating_sub(1));
            base + shift
        } else {
            cursor_col_char.saturating_sub(scroll_col_char)
        };
        let screen_col = cursor_display_col;
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
