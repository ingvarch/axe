// IMPACT ANALYSIS — axe-ui::ai_overlay render
// Parents: lib.rs::render() calls render_ai_overlay() near the end of the
//          modal-overlay stack, after confirm_dialog (so confirms render on
//          top) and before help.
// Children: terminal_panel::{convert_ansi_color,cell_flags_to_modifier} for
//          cell drawing.
// Siblings: Other modal overlays (command_palette, file_finder) live in
//          overlays.rs; this one gets its own file because the PTY grid render
//          path is non-trivial.

use alacritty_terminal::term::cell::Flags as CellFlags;
use alacritty_terminal::vte::ansi::CursorShape;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Clear, List, ListItem, ListState, Paragraph};
use ratatui::Frame;

use axe_core::ai_overlay::{AgentPicker, AiOverlay};
use axe_core::AppState;
use axe_terminal::event_listener::PtyEventListener;
use axe_terminal::tab::TerminalTab;

use crate::terminal_panel::{cell_flags_to_modifier, convert_ansi_color};
use crate::theme::Theme;

/// Renders the AI overlay on top of `area` if it is currently visible.
///
/// Takes `&mut AppState` so the inner grid rectangle can be written back to
/// `app.ai_overlay_grid_area` for the input layer to consume on the next
/// tick — this is how mouse clicks are hit-tested against the overlay.
///
/// No-op when `app.ai_overlay.visible == false` so the caller can always
/// invoke this unconditionally at the end of the render cascade.
pub(crate) fn render_ai_overlay(frame: &mut Frame, area: Rect, app: &mut AppState, theme: &Theme) {
    if !app.ai_overlay.visible {
        // Clear any stale grid area so mouse clicks don't hit an invisible
        // overlay after it's been closed.
        app.ai_overlay_grid_area = None;
        return;
    }

    let rect = centered_rect(area, 80, 80);
    frame.render_widget(Clear, rect);

    let title = ai_overlay_title(&app.ai_overlay);
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Double)
        .border_style(Style::default().fg(theme.overlay_border))
        .title(title)
        .title_style(
            Style::default()
                .fg(theme.foreground)
                .add_modifier(Modifier::BOLD),
        );

    let inner = block.inner(rect);
    frame.render_widget(block, rect);

    // Expose the inner PTY rect so the mouse handler can map screen
    // coordinates to grid points.
    app.ai_overlay_grid_area = Some((inner.x, inner.y, inner.width, inner.height));

    if let Some(picker) = app.ai_overlay.picker.as_ref() {
        render_picker(frame, inner, picker, theme);
        return;
    }

    if let Some(session) = app.ai_overlay.session.as_ref() {
        render_term_grid(frame, inner, &session.tab, theme);
        return;
    }

    render_empty_state(frame, inner, theme);
}

/// Produces the block title string, including the agent's display name when
/// a session is running.
fn ai_overlay_title(overlay: &AiOverlay) -> String {
    if let Some(session) = overlay.session.as_ref() {
        format!(" AI: {} ", session.display_name)
    } else if overlay.picker.is_some() {
        " AI: pick an agent ".to_string()
    } else {
        " AI ".to_string()
    }
}

/// Computes a centered rect `width_pct` wide and `height_pct` tall within `area`.
fn centered_rect(area: Rect, width_pct: u16, height_pct: u16) -> Rect {
    let w = (area.width as u32 * width_pct as u32 / 100) as u16;
    let h = (area.height as u32 * height_pct as u32 / 100) as u16;
    let x = area.x + area.width.saturating_sub(w) / 2;
    let y = area.y + area.height.saturating_sub(h) / 2;
    Rect {
        x,
        y,
        width: w,
        height: h,
    }
}

/// Renders the agent picker list inside the overlay's inner area.
fn render_picker(frame: &mut Frame, area: Rect, picker: &AgentPicker, theme: &Theme) {
    let items: Vec<ListItem> = picker
        .items
        .iter()
        .map(|a| {
            ListItem::new(Line::from(vec![
                Span::styled(
                    format!("  {:<14}", a.display),
                    Style::default().fg(theme.foreground),
                ),
                Span::styled(
                    format!("  {}", a.command),
                    Style::default().fg(theme.line_number),
                ),
            ]))
        })
        .collect();

    let mut state = ListState::default();
    state.select(Some(picker.selected));

    let list = List::new(items)
        .highlight_style(
            Style::default()
                .bg(theme.overlay_border)
                .fg(theme.background)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("▶ ");

    frame.render_stateful_widget(list, area, &mut state);
}

/// Renders the empty "no session yet" placeholder.
fn render_empty_state(frame: &mut Frame, area: Rect, theme: &Theme) {
    let text = Paragraph::new(vec![
        Line::from(""),
        Line::from(Span::styled(
            "  No AI session running.",
            Style::default().fg(theme.foreground),
        )),
        Line::from(""),
        Line::from(Span::styled(
            "  Press Ctrl+Shift+A to start.",
            Style::default().fg(theme.line_number),
        )),
    ]);
    frame.render_widget(text, area);
}

/// Renders a `TerminalTab`'s grid into `area`.
///
/// Thin copy of the cell-drawing loop from `terminal_panel::render_terminal_content`,
/// stripped of tab bar / scrollbar logic — the AI overlay has only one session
/// and its own border, so neither is needed.
fn render_term_grid(frame: &mut Frame, area: Rect, tab: &TerminalTab, theme: &Theme) {
    let term: &alacritty_terminal::Term<PtyEventListener> = tab.term();
    let content = term.renderable_content();
    let offset = content.display_offset as i32;
    let selection_range = content.selection;

    frame.render_widget(Clear, area);
    let buf = frame.buffer_mut();

    for indexed in content.display_iter {
        let point = indexed.point;
        let cell = &indexed.cell;

        let viewport_row = point.line.0 + offset;
        if viewport_row < 0 {
            continue;
        }
        let x = area.x.saturating_add(point.column.0 as u16);
        let y = area.y.saturating_add(viewport_row as u16);

        if x >= area.x + area.width || y >= area.y + area.height {
            continue;
        }

        if cell.flags.contains(CellFlags::WIDE_CHAR_SPACER)
            || cell.flags.contains(CellFlags::LEADING_WIDE_CHAR_SPACER)
        {
            continue;
        }

        let fg = convert_ansi_color(&cell.fg);
        let bg = convert_ansi_color(&cell.bg);
        let mut modifier = cell_flags_to_modifier(cell.flags);
        // Selection highlight: invert fg/bg via REVERSED. Matches the
        // terminal panel's selection rendering exactly.
        if let Some(ref sel) = selection_range {
            if sel.contains(point) {
                modifier.insert(Modifier::REVERSED);
            }
        }
        let style = Style::default().fg(fg).bg(bg).add_modifier(modifier);

        if let Some(buf_cell) = buf.cell_mut((x, y)) {
            buf_cell.set_char(cell.c);
            buf_cell.set_style(style);
        }
    }

    // Cursor — only when we are scrolled to the live screen.
    if content.cursor.shape != CursorShape::Hidden && offset == 0 {
        let cursor_point = content.cursor.point;
        let cursor_row = cursor_point.line.0;
        if cursor_row >= 0 {
            let cx = area.x.saturating_add(cursor_point.column.0 as u16);
            let cy = area.y.saturating_add(cursor_row as u16);
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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn centered_rect_80_percent_of_100_is_80x80_centered() {
        let area = Rect {
            x: 0,
            y: 0,
            width: 100,
            height: 100,
        };
        let r = centered_rect(area, 80, 80);
        assert_eq!(r.width, 80);
        assert_eq!(r.height, 80);
        assert_eq!(r.x, 10);
        assert_eq!(r.y, 10);
    }

    #[test]
    fn centered_rect_handles_odd_size() {
        let area = Rect {
            x: 0,
            y: 0,
            width: 99,
            height: 51,
        };
        let r = centered_rect(area, 80, 80);
        assert_eq!(r.width, 79);
        assert_eq!(r.height, 40);
        // Centered: (99-79)/2 = 10, (51-40)/2 = 5.
        assert_eq!(r.x, 10);
        assert_eq!(r.y, 5);
    }

    #[test]
    fn centered_rect_respects_area_offset() {
        let area = Rect {
            x: 5,
            y: 3,
            width: 100,
            height: 100,
        };
        let r = centered_rect(area, 80, 80);
        assert_eq!(r.x, 5 + 10);
        assert_eq!(r.y, 3 + 10);
    }

    #[test]
    fn title_uses_agent_display_name_when_session_present() {
        // Build a fake overlay with a real session so title() reflects the display name.
        let mut overlay = AiOverlay::new();
        let cwd = std::env::current_dir().unwrap();
        let agent = axe_core::ai_overlay::registry::ResolvedAgent {
            id: "cat".to_string(),
            command: "/bin/cat".to_string(),
            args: Vec::new(),
            display: "Cat Agent".to_string(),
        };
        overlay.start_session(&agent, &cwd).expect("spawn");

        let title = ai_overlay_title(&overlay);
        assert!(title.contains("Cat Agent"), "got title: {title:?}");
    }

    #[test]
    fn title_when_picker_open() {
        let mut overlay = AiOverlay::new();
        overlay.picker = Some(AgentPicker::new(Vec::new()));
        let title = ai_overlay_title(&overlay);
        assert!(title.contains("pick"));
    }

    #[test]
    fn title_when_empty() {
        let overlay = AiOverlay::new();
        assert_eq!(ai_overlay_title(&overlay).trim(), "AI");
    }
}
