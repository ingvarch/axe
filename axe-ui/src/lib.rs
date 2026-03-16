pub mod layout;
pub mod theme;

use axe_core::{AppState, FocusTarget};
use ratatui::layout::{Constraint, Layout};
use ratatui::style::{Modifier, Style};
use ratatui::widgets::{Block, BorderType, Borders, Paragraph};
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

/// Renders the full IDE interface: three panels (Files, Editor, Terminal) and a status bar.
pub fn render(app: &AppState, frame: &mut Frame) {
    let theme = Theme::default();
    let layout_mgr = LayoutManager::default();
    let area = frame.area();

    // Split vertically: main area + status bar
    let vertical = Layout::vertical([Constraint::Min(0), Constraint::Length(1)]).split(area);

    let main_area = vertical[0];
    let status_area = vertical[1];

    // Split main area horizontally: tree | right
    let horizontal = Layout::horizontal([
        Constraint::Percentage(layout_mgr.tree_width_pct),
        Constraint::Percentage(100 - layout_mgr.tree_width_pct),
    ])
    .split(main_area);

    let tree_area = horizontal[0];
    let right_area = horizontal[1];

    // Split right area vertically: editor | terminal
    let right_split = Layout::vertical([
        Constraint::Percentage(layout_mgr.editor_height_pct),
        Constraint::Percentage(100 - layout_mgr.editor_height_pct),
    ])
    .split(right_area);

    let editor_area = right_split[0];
    let terminal_area = right_split[1];

    // Panel identifiers for focus comparison
    let tree_focus = FocusTarget::Tree;
    let editor_focus = FocusTarget::Editor;
    let terminal_focus = FocusTarget::Terminal(0);

    // Render panels with dynamic focus-based styling
    let panel_style = Style::default().bg(theme.background).fg(theme.foreground);

    let files_block = Block::default()
        .title(" Files ")
        .title_style(title_style_for(&app.focus, &tree_focus, &theme))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(border_style_for(&app.focus, &tree_focus, &theme))
        .style(panel_style);
    frame.render_widget(files_block, tree_area);

    let editor_block = Block::default()
        .title(" Editor ")
        .title_style(title_style_for(&app.focus, &editor_focus, &theme))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(border_style_for(&app.focus, &editor_focus, &theme))
        .style(panel_style);
    frame.render_widget(editor_block, editor_area);

    let terminal_block = Block::default()
        .title(" Terminal ")
        .title_style(title_style_for(&app.focus, &terminal_focus, &theme))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(border_style_for(&app.focus, &terminal_focus, &theme))
        .style(panel_style);
    frame.render_widget(terminal_block, terminal_area);

    // Status bar with focus indicator
    let version = axe_core::version();
    let focus_label = app.focus.label();
    let status_text = format!("Axe IDE v{version} | Focus: {focus_label} | Press q to quit");
    let status_bar = Paragraph::new(status_text).style(
        Style::default()
            .bg(theme.status_bar_bg)
            .fg(theme.status_bar_fg),
    );
    frame.render_widget(status_bar, status_area);
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
    fn render_shows_status_bar() {
        let content = render_to_string(80, 24);
        assert!(
            content.contains("Axe IDE v"),
            "expected 'Axe IDE v' in status bar"
        );
        assert!(
            content.contains("Press q to quit"),
            "expected 'Press q to quit' in status bar"
        );
    }

    #[test]
    fn render_works_with_small_terminal() {
        // Should not panic with a very small terminal
        let content = render_to_string(40, 10);
        assert!(!content.is_empty());
    }

    #[test]
    fn render_editor_has_active_border_by_default() {
        // Default focus is Editor — status bar should show "Focus: Editor"
        let content = render_to_string(80, 24);
        assert!(
            content.contains("Focus: Editor"),
            "expected 'Focus: Editor' in status bar"
        );
    }

    #[test]
    fn render_status_bar_shows_focus_files() {
        let mut app = AppState::new();
        app.focus = FocusTarget::Tree;
        let content = render_app_to_string(&app, 80, 24);
        assert!(
            content.contains("Focus: Files"),
            "expected 'Focus: Files' in status bar"
        );
    }

    #[test]
    fn render_status_bar_shows_focus_terminal() {
        let mut app = AppState::new();
        app.focus = FocusTarget::Terminal(0);
        let content = render_app_to_string(&app, 80, 24);
        assert!(
            content.contains("Focus: Terminal"),
            "expected 'Focus: Terminal' in status bar"
        );
    }
}
