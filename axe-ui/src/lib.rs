pub mod layout;
pub mod theme;

use axe_core::{AppState, FocusTarget};
use ratatui::layout::{Alignment, Constraint, Flex, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Clear, Paragraph};
use ratatui::Frame;

use layout::LayoutManager;
use theme::Theme;

/// Returns the border style for a panel based on whether it has focus.
fn border_style_for(focus: &FocusTarget, panel: &FocusTarget, theme: &Theme) -> Style {
    if focus == panel {
        Style::default().fg(theme.panel_border_active)
    } else {
        Style::default().fg(theme.panel_border)
    }
}

/// Returns the title style for a panel — bold when focused.
fn title_style_for(focus: &FocusTarget, panel: &FocusTarget, theme: &Theme) -> Style {
    if focus == panel {
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
) -> Block<'a> {
    let panel_style = Style::default().bg(theme.background).fg(theme.foreground);

    Block::default()
        .title(title)
        .title_style(title_style_for(focus, panel, theme))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(border_style_for(focus, panel, theme))
        .style(panel_style)
}

/// Builds the status bar line with hotkey hints.
fn build_status_bar<'a>(app: &AppState, theme: &Theme) -> Line<'a> {
    let version = axe_core::version();
    let focus_label = app.focus.label();
    let key_style = Style::default().fg(theme.status_bar_key);
    let text_style = Style::default().fg(theme.status_bar_fg);

    Line::from(vec![
        Span::styled(format!(" Axe v{version}"), text_style),
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
    ])
}

/// Help text lines for the help overlay.
const HELP_LINES: &[(&str, &str)] = &[
    ("Ctrl+Q", "Quit"),
    ("Ctrl+C", "Quit"),
    ("Tab", "Next panel"),
    ("Shift+Tab", "Previous panel"),
    ("Ctrl+1", "Focus Files"),
    ("Ctrl+2", "Focus Editor"),
    ("Ctrl+3", "Focus Terminal"),
    ("Ctrl+B", "Toggle file tree"),
    ("Ctrl+T", "Toggle terminal"),
    ("Ctrl+H", "Toggle this help"),
    ("Esc", "Close overlay"),
];

/// Renders the help overlay centered on the screen.
fn render_help_overlay(frame: &mut Frame, theme: &Theme) {
    let area = frame.area();

    let overlay_width = 40_u16.min(area.width.saturating_sub(4));
    let overlay_height = (HELP_LINES.len() as u16 + 4).min(area.height.saturating_sub(2));

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
                    format!("  {key:<14}"),
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
        y: inner.y + 1,
        height: inner.height.saturating_sub(1),
        ..inner
    };
    frame.render_widget(help_text, content_area);
}

/// Renders the full IDE interface with conditional panel visibility and a status bar.
pub fn render(app: &AppState, frame: &mut Frame) {
    let theme = Theme::default();
    let layout_mgr = LayoutManager {
        show_tree: app.show_tree,
        show_terminal: app.show_terminal,
        ..LayoutManager::default()
    };
    let area = frame.area();

    // Split vertically: main area + status bar
    let vertical = Layout::vertical([Constraint::Min(0), Constraint::Length(1)]).split(area);
    let main_area = vertical[0];
    let status_area = vertical[1];

    if layout_mgr.show_tree {
        let horizontal = Layout::horizontal([
            Constraint::Percentage(layout_mgr.tree_width_pct),
            Constraint::Percentage(100 - layout_mgr.tree_width_pct),
        ])
        .split(main_area);

        let tree_area = horizontal[0];
        let right_area = horizontal[1];

        frame.render_widget(
            panel_block(" Files ", &app.focus, &FocusTarget::Tree, &theme),
            tree_area,
        );

        render_right_panels(app, frame, right_area, &layout_mgr, &theme);
    } else {
        render_right_panels(app, frame, main_area, &layout_mgr, &theme);
    }

    // Status bar with hotkey hints
    let status_line = build_status_bar(app, &theme);
    let status_bar = Paragraph::new(status_line).style(
        Style::default()
            .bg(theme.status_bar_bg)
            .fg(theme.status_bar_fg),
    );
    frame.render_widget(status_bar, status_area);

    // Help overlay (on top of everything)
    if app.show_help {
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
) {
    if layout_mgr.show_terminal {
        let right_split = Layout::vertical([
            Constraint::Percentage(layout_mgr.editor_height_pct),
            Constraint::Percentage(100 - layout_mgr.editor_height_pct),
        ])
        .split(area);

        frame.render_widget(
            panel_block(" Editor ", &app.focus, &FocusTarget::Editor, theme),
            right_split[0],
        );
        frame.render_widget(
            panel_block(" Terminal ", &app.focus, &FocusTarget::Terminal(0), theme),
            right_split[1],
        );
    } else {
        frame.render_widget(
            panel_block(" Editor ", &app.focus, &FocusTarget::Editor, theme),
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
    fn render_editor_has_active_border_by_default() {
        let content = render_to_string(100, 24);
        assert!(
            content.contains("Focus: Editor"),
            "expected 'Focus: Editor' in status bar"
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
        let content = render_app_to_string(&app, 80, 30);
        assert!(content.contains("Ctrl+Q"), "expected 'Ctrl+Q' in help");
        assert!(content.contains("Ctrl+B"), "expected 'Ctrl+B' in help");
        assert!(content.contains("Ctrl+T"), "expected 'Ctrl+T' in help");
        assert!(content.contains("Ctrl+H"), "expected 'Ctrl+H' in help");
        assert!(content.contains("Esc"), "expected 'Esc' in help");
    }
}
