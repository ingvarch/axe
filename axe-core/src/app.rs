use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

/// Identifies which panel currently has keyboard focus.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum FocusTarget {
    Tree,
    #[default]
    Editor,
    Terminal(usize),
}

impl FocusTarget {
    /// Returns the next focus target in cycle: Tree -> Editor -> Terminal(0) -> Tree.
    pub fn next(&self) -> Self {
        match self {
            Self::Tree => Self::Editor,
            Self::Editor => Self::Terminal(0),
            Self::Terminal(_) => Self::Tree,
        }
    }

    /// Returns the previous focus target in cycle: Tree -> Terminal(0) -> Editor -> Tree.
    pub fn prev(&self) -> Self {
        match self {
            Self::Tree => Self::Terminal(0),
            Self::Editor => Self::Tree,
            Self::Terminal(_) => Self::Editor,
        }
    }

    /// Returns a human-readable label for display in the status bar.
    pub fn label(&self) -> &'static str {
        match self {
            Self::Tree => "Files",
            Self::Editor => "Editor",
            Self::Terminal(_) => "Terminal",
        }
    }
}

/// Central application state shared across all subsystems.
pub struct AppState {
    pub should_quit: bool,
    pub focus: FocusTarget,
}

impl AppState {
    /// Creates a new `AppState` with default values.
    pub fn new() -> Self {
        Self {
            should_quit: false,
            focus: FocusTarget::default(),
        }
    }

    /// Signals the application to exit the event loop.
    pub fn quit(&mut self) {
        self.should_quit = true;
    }

    /// Processes a key event and updates application state accordingly.
    pub fn handle_key_event(&mut self, key: KeyEvent) {
        match (key.modifiers, key.code) {
            (KeyModifiers::NONE, KeyCode::Char('q')) => self.quit(),
            (KeyModifiers::CONTROL, KeyCode::Char('c')) => self.quit(),
            (KeyModifiers::NONE, KeyCode::Tab) => self.focus = self.focus.next(),
            (KeyModifiers::SHIFT, KeyCode::BackTab) => self.focus = self.focus.prev(),
            (KeyModifiers::CONTROL, KeyCode::Char('1')) => self.focus = FocusTarget::Tree,
            (KeyModifiers::CONTROL, KeyCode::Char('2')) => self.focus = FocusTarget::Editor,
            (KeyModifiers::CONTROL, KeyCode::Char('3')) => self.focus = FocusTarget::Terminal(0),
            _ => {}
        }
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    #[test]
    fn app_state_starts_not_quit() {
        let app = AppState::new();
        assert!(!app.should_quit);
    }

    #[test]
    fn app_state_quit_sets_flag() {
        let mut app = AppState::new();
        app.quit();
        assert!(app.should_quit);
    }

    #[test]
    fn handle_q_key_quits() {
        let mut app = AppState::new();
        app.handle_key_event(KeyEvent::new(KeyCode::Char('q'), KeyModifiers::NONE));
        assert!(app.should_quit);
    }

    #[test]
    fn handle_ctrl_c_quits() {
        let mut app = AppState::new();
        app.handle_key_event(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL));
        assert!(app.should_quit);
    }

    #[test]
    fn handle_other_key_does_not_quit() {
        let mut app = AppState::new();
        app.handle_key_event(KeyEvent::new(KeyCode::Char('a'), KeyModifiers::NONE));
        assert!(!app.should_quit);
    }

    // --- FocusTarget tests ---

    #[test]
    fn focus_target_default_is_editor() {
        assert_eq!(FocusTarget::default(), FocusTarget::Editor);
    }

    #[test]
    fn focus_target_next_cycles() {
        assert_eq!(FocusTarget::Tree.next(), FocusTarget::Editor);
        assert_eq!(FocusTarget::Editor.next(), FocusTarget::Terminal(0));
        assert_eq!(FocusTarget::Terminal(0).next(), FocusTarget::Tree);
    }

    #[test]
    fn focus_target_prev_cycles() {
        assert_eq!(FocusTarget::Tree.prev(), FocusTarget::Terminal(0));
        assert_eq!(FocusTarget::Editor.prev(), FocusTarget::Tree);
        assert_eq!(FocusTarget::Terminal(0).prev(), FocusTarget::Editor);
    }

    #[test]
    fn focus_target_label() {
        assert_eq!(FocusTarget::Tree.label(), "Files");
        assert_eq!(FocusTarget::Editor.label(), "Editor");
        assert_eq!(FocusTarget::Terminal(0).label(), "Terminal");
    }

    #[test]
    fn app_state_default_focus_is_editor() {
        let app = AppState::new();
        assert_eq!(app.focus, FocusTarget::Editor);
    }

    #[test]
    fn handle_tab_cycles_focus_forward() {
        let mut app = AppState::new();
        assert_eq!(app.focus, FocusTarget::Editor);
        app.handle_key_event(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));
        assert_eq!(app.focus, FocusTarget::Terminal(0));
    }

    #[test]
    fn handle_shift_tab_cycles_focus_backward() {
        let mut app = AppState::new();
        assert_eq!(app.focus, FocusTarget::Editor);
        app.handle_key_event(KeyEvent::new(KeyCode::BackTab, KeyModifiers::SHIFT));
        assert_eq!(app.focus, FocusTarget::Tree);
    }

    #[test]
    fn handle_ctrl_1_focuses_tree() {
        let mut app = AppState::new();
        app.handle_key_event(KeyEvent::new(KeyCode::Char('1'), KeyModifiers::CONTROL));
        assert_eq!(app.focus, FocusTarget::Tree);
    }

    #[test]
    fn handle_ctrl_2_focuses_editor() {
        let mut app = AppState::new();
        app.focus = FocusTarget::Tree;
        app.handle_key_event(KeyEvent::new(KeyCode::Char('2'), KeyModifiers::CONTROL));
        assert_eq!(app.focus, FocusTarget::Editor);
    }

    #[test]
    fn handle_ctrl_3_focuses_terminal() {
        let mut app = AppState::new();
        app.handle_key_event(KeyEvent::new(KeyCode::Char('3'), KeyModifiers::CONTROL));
        assert_eq!(app.focus, FocusTarget::Terminal(0));
    }

    #[test]
    fn handle_tab_from_terminal_wraps_to_tree() {
        let mut app = AppState::new();
        app.focus = FocusTarget::Terminal(0);
        app.handle_key_event(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));
        assert_eq!(app.focus, FocusTarget::Tree);
    }

    #[test]
    fn existing_quit_keys_still_work_with_focus() {
        let mut app = AppState::new();
        app.focus = FocusTarget::Tree;
        app.handle_key_event(KeyEvent::new(KeyCode::Char('q'), KeyModifiers::NONE));
        assert!(app.should_quit);

        let mut app2 = AppState::new();
        app2.focus = FocusTarget::Terminal(0);
        app2.handle_key_event(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL));
        assert!(app2.should_quit);
    }
}
