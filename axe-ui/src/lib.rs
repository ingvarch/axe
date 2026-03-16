use axe_core::AppState;
use ratatui::layout::{Constraint, Flex, Layout};
use ratatui::text::Text;
use ratatui::widgets::Paragraph;
use ratatui::Frame;

pub fn render(_app: &AppState, frame: &mut Frame) {
    let area = frame.area();

    let vertical =
        Layout::vertical([Constraint::Length(1)]).flex(Flex::Center).split(area);

    let horizontal =
        Layout::horizontal([Constraint::Length(7)]).flex(Flex::Center).split(vertical[0]);

    let title = Paragraph::new(Text::raw("Axe IDE"));
    frame.render_widget(title, horizontal[0]);
}

#[cfg(test)]
mod tests {
    use super::*;
    use axe_core::AppState;
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    #[test]
    fn render_shows_axe_ide_text() {
        let app = AppState::new();
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|frame| render(&app, frame))
            .unwrap();

        let buffer = terminal.backend().buffer().clone();
        let content: String = buffer
            .content()
            .iter()
            .map(|cell| cell.symbol().chars().next().unwrap_or(' '))
            .collect();

        assert!(
            content.contains("Axe IDE"),
            "expected 'Axe IDE' in rendered output, got: {content}"
        );
    }
}
