use alacritty_terminal::term::cell::Flags as CellFlags;
use alacritty_terminal::vte::ansi::{Color as AnsiColor, CursorShape, NamedColor};
use ratatui::layout::{Alignment, Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Clear, Paragraph};
use ratatui::Frame;

use axe_core::{AppState, FocusTarget};
use axe_terminal::TerminalManager;

use crate::editor_panel::{editor_title, render_scrollbar};
use crate::layout::LayoutManager;
use crate::theme::Theme;

/// Width reserved for the terminal scrollbar column.
pub(crate) const TERMINAL_SCROLLBAR_WIDTH: u16 = 1;

// IMPACT ANALYSIS — convert_ansi_color
// Parents: render_terminal_content() uses this to convert cell colors.
// Children: None.
// Siblings: Theme colors — terminal colors are independent from theme.

/// Converts an alacritty_terminal ANSI color to a ratatui color.
pub(crate) fn convert_ansi_color(color: &AnsiColor) -> ratatui::style::Color {
    use ratatui::style::Color;
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
pub(crate) fn cell_flags_to_modifier(flags: CellFlags) -> Modifier {
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

    // [+] button for creating a new tab.
    spans.push(Span::raw(" "));
    let plus_style = if mgr.is_at_tab_limit() {
        Style::default()
            .fg(theme.foreground)
            .add_modifier(Modifier::DIM)
    } else {
        Style::default().fg(theme.panel_border_active)
    };
    spans.push(Span::styled("[+]", plus_style));

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
pub(crate) fn render_terminal_content(
    mgr: &TerminalManager,
    area: Rect,
    frame: &mut Frame,
    theme: &Theme,
) {
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

/// Renders a scrollbar for the terminal panel.
///
/// Delegates to `render_scrollbar` with inverted offset (terminal uses
/// display_offset 0 = bottom, but scrollbar expects 0 = top).
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

    // Terminal display_offset: 0 = bottom (most recent), max = top (oldest).
    // Invert so scrollbar thumb at top = oldest content visible.
    let inverted_offset = max_offset.saturating_sub(display_offset);

    render_scrollbar(
        total_lines,
        screen_lines,
        inverted_offset,
        scrollbar_area,
        buf,
        theme,
    );
}

/// Adjusts a rect for the terminal grid: subtracts 1 row for the tab bar (if needed)
/// and 1 column for the scrollbar.
pub(crate) fn adjust_terminal_rect(rect: Rect, has_tabs: bool) -> Rect {
    let mut r = rect;
    if has_tabs && r.height > 1 {
        r.y += 1;
        r.height = r.height.saturating_sub(1);
    }
    r.width = r.width.saturating_sub(TERMINAL_SCROLLBAR_WIDTH);
    r
}

/// Renders the right-side panels (editor and optionally terminal) in the given area.
pub(crate) fn render_right_panels(
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

        let editor_block = crate::panel_block(
            editor_title(app, false),
            &app.focus,
            &FocusTarget::Editor,
            theme,
            resize_active,
        );
        let editor_inner = editor_block.inner(right_split[0]);
        frame.render_widget(editor_block, right_split[0]);
        crate::render_editor_splits(app, editor_inner, frame, theme);

        let term_block = crate::panel_block(
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
        let editor_block = crate::panel_block(
            editor_title(app, false),
            &app.focus,
            &FocusTarget::Editor,
            theme,
            resize_active,
        );
        let editor_inner = editor_block.inner(area);
        frame.render_widget(editor_block, area);
        crate::render_editor_splits(app, editor_inner, frame, theme);
    }
}
