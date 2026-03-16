use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::command::Command;
use crate::keymap::KeymapResolver;

/// Default width of the file tree panel as a percentage of total width.
const DEFAULT_TREE_WIDTH_PCT: u16 = 20;
/// Default height of the editor panel as a percentage of the right-side area.
const DEFAULT_EDITOR_HEIGHT_PCT: u16 = 70;
/// Percentage change per resize step.
const RESIZE_STEP: u16 = 2;
/// Minimum allowed panel size percentage.
const MIN_PANEL_PCT: u16 = 10;
/// Maximum allowed panel size percentage.
const MAX_PANEL_PCT: u16 = 90;

/// State for the panel resize mode.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ResizeModeState {
    /// Whether resize mode is currently active.
    pub active: bool,
}

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
    pub show_tree: bool,
    pub show_terminal: bool,
    pub show_help: bool,
    pub resize_mode: ResizeModeState,
    pub tree_width_pct: u16,
    pub editor_height_pct: u16,
    keymap: KeymapResolver,
}

impl AppState {
    /// Creates a new `AppState` with default values.
    pub fn new() -> Self {
        Self {
            should_quit: false,
            focus: FocusTarget::default(),
            show_tree: true,
            show_terminal: true,
            show_help: false,
            resize_mode: ResizeModeState::default(),
            tree_width_pct: DEFAULT_TREE_WIDTH_PCT,
            editor_height_pct: DEFAULT_EDITOR_HEIGHT_PCT,
            keymap: KeymapResolver::with_defaults(),
        }
    }

    /// Signals the application to exit the event loop.
    pub fn quit(&mut self) {
        self.should_quit = true;
    }

    /// Processes a key event by resolving it through the keymap and executing
    /// the resulting command, if any.
    ///
    /// When resize mode is active, arrow keys and special keys are intercepted
    /// before normal dispatch. When a help overlay is open, only Quit, ShowHelp,
    /// and CloseOverlay commands are processed; all other keys are consumed silently.
    pub fn handle_key_event(&mut self, key: KeyEvent) {
        // Resize mode intercepts keys before normal keymap resolution.
        if self.resize_mode.active {
            let cmd = match (key.modifiers, key.code) {
                (KeyModifiers::NONE, KeyCode::Left) => Some(Command::ResizeLeft),
                (KeyModifiers::NONE, KeyCode::Right) => Some(Command::ResizeRight),
                (KeyModifiers::NONE, KeyCode::Up) => Some(Command::ResizeUp),
                (KeyModifiers::NONE, KeyCode::Down) => Some(Command::ResizeDown),
                (KeyModifiers::NONE, KeyCode::Char('=')) => Some(Command::EqualizeLayout),
                (KeyModifiers::NONE, KeyCode::Esc) | (KeyModifiers::NONE, KeyCode::Enter) => {
                    Some(Command::ExitResizeMode)
                }
                (KeyModifiers::CONTROL, KeyCode::Char('q'))
                | (KeyModifiers::CONTROL, KeyCode::Char('c')) => Some(Command::Quit),
                _ => None, // All other keys consumed silently
            };
            if let Some(cmd) = cmd {
                self.execute(cmd);
            }
            return;
        }

        if let Some(cmd) = self.keymap.resolve(&key) {
            if self.show_help {
                match cmd {
                    Command::Quit | Command::ShowHelp | Command::CloseOverlay => {
                        self.execute(cmd);
                    }
                    _ => {}
                }
            } else {
                self.execute(cmd);
            }
        }
    }

    /// Dispatches a command to update application state.
    pub fn execute(&mut self, cmd: Command) {
        match cmd {
            Command::Quit => self.quit(),
            Command::FocusNext => self.cycle_focus_next(),
            Command::FocusPrev => self.cycle_focus_prev(),
            Command::FocusTree => self.focus = FocusTarget::Tree,
            Command::FocusEditor => self.focus = FocusTarget::Editor,
            Command::FocusTerminal => self.focus = FocusTarget::Terminal(0),
            Command::ToggleTree => self.toggle_tree(),
            Command::ToggleTerminal => self.toggle_terminal(),
            Command::ShowHelp => self.show_help = !self.show_help,
            Command::CloseOverlay => self.show_help = false,
            Command::EnterResizeMode => self.resize_mode.active = true,
            Command::ExitResizeMode => self.resize_mode.active = false,
            Command::ResizeLeft => self.resize_horizontal(-1),
            Command::ResizeRight => self.resize_horizontal(1),
            Command::ResizeUp => self.resize_vertical(-1),
            Command::ResizeDown => self.resize_vertical(1),
            Command::EqualizeLayout => self.equalize_layout(),
        }
    }

    /// Adjusts tree width by `direction` steps (+1 = grow, -1 = shrink).
    /// Only applies when the Tree panel is focused.
    fn resize_horizontal(&mut self, direction: i16) {
        if self.focus != FocusTarget::Tree {
            return;
        }
        let new_pct = (self.tree_width_pct as i16 + direction * RESIZE_STEP as i16)
            .clamp(MIN_PANEL_PCT as i16, MAX_PANEL_PCT as i16);
        self.tree_width_pct = new_pct as u16;
    }

    /// Adjusts the editor/terminal split by moving the border in the arrow direction.
    /// Up = border moves up (editor shrinks, terminal grows).
    /// Down = border moves down (editor grows, terminal shrinks).
    /// Only applies when the Editor or Terminal panel is focused.
    fn resize_vertical(&mut self, direction: i16) {
        if self.focus == FocusTarget::Tree {
            return;
        }
        let new_pct = (self.editor_height_pct as i16 + direction * RESIZE_STEP as i16)
            .clamp(MIN_PANEL_PCT as i16, MAX_PANEL_PCT as i16);
        self.editor_height_pct = new_pct as u16;
    }

    /// Resets all panel sizes to their defaults.
    fn equalize_layout(&mut self) {
        self.tree_width_pct = DEFAULT_TREE_WIDTH_PCT;
        self.editor_height_pct = DEFAULT_EDITOR_HEIGHT_PCT;
    }

    /// Cycles focus forward, skipping hidden panels.
    fn cycle_focus_next(&mut self) {
        let next = self.focus.next();
        self.focus = self.skip_hidden_forward(next);
    }

    /// Cycles focus backward, skipping hidden panels.
    fn cycle_focus_prev(&mut self) {
        let prev = self.focus.prev();
        self.focus = self.skip_hidden_backward(prev);
    }

    /// Skips hidden panels when cycling forward.
    fn skip_hidden_forward(&self, target: FocusTarget) -> FocusTarget {
        match &target {
            FocusTarget::Tree if !self.show_tree => self.skip_hidden_forward(target.next()),
            FocusTarget::Terminal(_) if !self.show_terminal => {
                self.skip_hidden_forward(target.next())
            }
            _ => target,
        }
    }

    /// Skips hidden panels when cycling backward.
    fn skip_hidden_backward(&self, target: FocusTarget) -> FocusTarget {
        match &target {
            FocusTarget::Tree if !self.show_tree => self.skip_hidden_backward(target.prev()),
            FocusTarget::Terminal(_) if !self.show_terminal => {
                self.skip_hidden_backward(target.prev())
            }
            _ => target,
        }
    }

    /// Toggles the file tree panel visibility.
    /// If the tree is currently focused, moves focus to the editor.
    fn toggle_tree(&mut self) {
        self.show_tree = !self.show_tree;
        if !self.show_tree && self.focus == FocusTarget::Tree {
            self.focus = FocusTarget::Editor;
        }
    }

    /// Toggles the terminal panel visibility.
    /// If the terminal is currently focused, moves focus to the editor.
    fn toggle_terminal(&mut self) {
        self.show_terminal = !self.show_terminal;
        if !self.show_terminal && matches!(self.focus, FocusTarget::Terminal(_)) {
            self.focus = FocusTarget::Editor;
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
    use crossterm::event::KeyEvent;

    // --- AppState basic tests ---

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
    fn app_state_defaults_show_tree_true() {
        let app = AppState::new();
        assert!(app.show_tree);
    }

    #[test]
    fn app_state_defaults_show_terminal_true() {
        let app = AppState::new();
        assert!(app.show_terminal);
    }

    #[test]
    fn app_state_defaults_show_help_false() {
        let app = AppState::new();
        assert!(!app.show_help);
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

    // --- Execute command tests ---

    #[test]
    fn execute_quit_sets_should_quit() {
        let mut app = AppState::new();
        app.execute(Command::Quit);
        assert!(app.should_quit);
    }

    #[test]
    fn execute_focus_next_cycles_focus() {
        let mut app = AppState::new();
        assert_eq!(app.focus, FocusTarget::Editor);
        app.execute(Command::FocusNext);
        assert_eq!(app.focus, FocusTarget::Terminal(0));
        app.execute(Command::FocusNext);
        assert_eq!(app.focus, FocusTarget::Tree);
        app.execute(Command::FocusNext);
        assert_eq!(app.focus, FocusTarget::Editor);
    }

    #[test]
    fn execute_focus_prev_cycles_focus() {
        let mut app = AppState::new();
        assert_eq!(app.focus, FocusTarget::Editor);
        app.execute(Command::FocusPrev);
        assert_eq!(app.focus, FocusTarget::Tree);
        app.execute(Command::FocusPrev);
        assert_eq!(app.focus, FocusTarget::Terminal(0));
        app.execute(Command::FocusPrev);
        assert_eq!(app.focus, FocusTarget::Editor);
    }

    #[test]
    fn execute_focus_tree_sets_focus() {
        let mut app = AppState::new();
        app.execute(Command::FocusTree);
        assert_eq!(app.focus, FocusTarget::Tree);
    }

    #[test]
    fn execute_focus_editor_sets_focus() {
        let mut app = AppState::new();
        app.focus = FocusTarget::Tree;
        app.execute(Command::FocusEditor);
        assert_eq!(app.focus, FocusTarget::Editor);
    }

    #[test]
    fn execute_focus_terminal_sets_focus() {
        let mut app = AppState::new();
        app.execute(Command::FocusTerminal);
        assert_eq!(app.focus, FocusTarget::Terminal(0));
    }

    #[test]
    fn execute_toggle_tree_hides_and_shows() {
        let mut app = AppState::new();
        assert!(app.show_tree);
        app.execute(Command::ToggleTree);
        assert!(!app.show_tree);
        app.execute(Command::ToggleTree);
        assert!(app.show_tree);
    }

    #[test]
    fn execute_toggle_terminal_hides_and_shows() {
        let mut app = AppState::new();
        assert!(app.show_terminal);
        app.execute(Command::ToggleTerminal);
        assert!(!app.show_terminal);
        app.execute(Command::ToggleTerminal);
        assert!(app.show_terminal);
    }

    #[test]
    fn toggle_tree_when_tree_focused_moves_focus_to_editor() {
        let mut app = AppState::new();
        app.focus = FocusTarget::Tree;
        app.execute(Command::ToggleTree);
        assert!(!app.show_tree);
        assert_eq!(app.focus, FocusTarget::Editor);
    }

    #[test]
    fn toggle_terminal_when_terminal_focused_moves_focus_to_editor() {
        let mut app = AppState::new();
        app.focus = FocusTarget::Terminal(0);
        app.execute(Command::ToggleTerminal);
        assert!(!app.show_terminal);
        assert_eq!(app.focus, FocusTarget::Editor);
    }

    #[test]
    fn execute_show_help_toggles() {
        let mut app = AppState::new();
        assert!(!app.show_help);
        app.execute(Command::ShowHelp);
        assert!(app.show_help);
        app.execute(Command::ShowHelp);
        assert!(!app.show_help);
    }

    #[test]
    fn execute_close_overlay_closes_help() {
        let mut app = AppState::new();
        app.show_help = true;
        app.execute(Command::CloseOverlay);
        assert!(!app.show_help);
    }

    #[test]
    fn close_overlay_noop_when_no_overlay() {
        let mut app = AppState::new();
        app.execute(Command::CloseOverlay);
        assert!(!app.show_help);
    }

    // --- Key event integration tests ---

    #[test]
    fn handle_key_ctrl_q_quits() {
        let mut app = AppState::new();
        app.handle_key_event(KeyEvent::new(KeyCode::Char('q'), KeyModifiers::CONTROL));
        assert!(app.should_quit);
    }

    #[test]
    fn handle_key_q_does_not_quit() {
        let mut app = AppState::new();
        app.handle_key_event(KeyEvent::new(KeyCode::Char('q'), KeyModifiers::NONE));
        assert!(!app.should_quit);
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
    fn handle_key_ctrl_b_toggles_tree() {
        let mut app = AppState::new();
        assert!(app.show_tree);
        app.handle_key_event(KeyEvent::new(KeyCode::Char('b'), KeyModifiers::CONTROL));
        assert!(!app.show_tree);
        app.handle_key_event(KeyEvent::new(KeyCode::Char('b'), KeyModifiers::CONTROL));
        assert!(app.show_tree);
    }

    #[test]
    fn handle_key_ctrl_t_toggles_terminal() {
        let mut app = AppState::new();
        assert!(app.show_terminal);
        app.handle_key_event(KeyEvent::new(KeyCode::Char('t'), KeyModifiers::CONTROL));
        assert!(!app.show_terminal);
        app.handle_key_event(KeyEvent::new(KeyCode::Char('t'), KeyModifiers::CONTROL));
        assert!(app.show_terminal);
    }

    #[test]
    fn handle_key_ctrl_h_toggles_help() {
        let mut app = AppState::new();
        assert!(!app.show_help);
        app.handle_key_event(KeyEvent::new(KeyCode::Char('h'), KeyModifiers::CONTROL));
        assert!(app.show_help);
        app.handle_key_event(KeyEvent::new(KeyCode::Char('h'), KeyModifiers::CONTROL));
        assert!(!app.show_help);
    }

    #[test]
    fn handle_esc_closes_help() {
        let mut app = AppState::new();
        app.show_help = true;
        app.handle_key_event(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
        assert!(!app.show_help);
    }

    #[test]
    fn help_overlay_blocks_other_commands() {
        let mut app = AppState::new();
        app.show_help = true;
        // Tab should not cycle focus while help is open
        app.handle_key_event(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));
        assert_eq!(app.focus, FocusTarget::Editor);
    }

    #[test]
    fn help_overlay_allows_quit() {
        let mut app = AppState::new();
        app.show_help = true;
        app.handle_key_event(KeyEvent::new(KeyCode::Char('q'), KeyModifiers::CONTROL));
        assert!(app.should_quit);
    }

    // --- Focus cycling with hidden panels ---

    #[test]
    fn focus_next_skips_hidden_tree() {
        let mut app = AppState::new();
        app.show_tree = false;
        app.focus = FocusTarget::Terminal(0);
        app.execute(Command::FocusNext);
        assert_eq!(app.focus, FocusTarget::Editor);
    }

    #[test]
    fn focus_next_skips_hidden_terminal() {
        let mut app = AppState::new();
        app.show_terminal = false;
        app.focus = FocusTarget::Editor;
        app.execute(Command::FocusNext);
        assert_eq!(app.focus, FocusTarget::Tree);
    }

    #[test]
    fn focus_prev_skips_hidden_tree() {
        let mut app = AppState::new();
        app.show_tree = false;
        app.focus = FocusTarget::Editor;
        app.execute(Command::FocusPrev);
        assert_eq!(app.focus, FocusTarget::Terminal(0));
    }

    #[test]
    fn focus_prev_skips_hidden_terminal() {
        let mut app = AppState::new();
        app.show_terminal = false;
        app.focus = FocusTarget::Tree;
        app.execute(Command::FocusPrev);
        assert_eq!(app.focus, FocusTarget::Editor);
    }

    // --- Resize mode defaults ---

    #[test]
    fn resize_mode_inactive_by_default() {
        let app = AppState::new();
        assert!(!app.resize_mode.active);
    }

    #[test]
    fn default_tree_width_pct_is_20() {
        let app = AppState::new();
        assert_eq!(app.tree_width_pct, 20);
    }

    #[test]
    fn default_editor_height_pct_is_70() {
        let app = AppState::new();
        assert_eq!(app.editor_height_pct, 70);
    }

    // --- Resize command execution ---

    #[test]
    fn enter_resize_mode_activates() {
        let mut app = AppState::new();
        app.execute(Command::EnterResizeMode);
        assert!(app.resize_mode.active);
    }

    #[test]
    fn exit_resize_mode_deactivates() {
        let mut app = AppState::new();
        app.resize_mode.active = true;
        app.execute(Command::ExitResizeMode);
        assert!(!app.resize_mode.active);
    }

    #[test]
    fn resize_left_decreases_tree_width() {
        let mut app = AppState::new();
        app.focus = FocusTarget::Tree;
        let original = app.tree_width_pct;
        app.execute(Command::ResizeLeft);
        assert_eq!(app.tree_width_pct, original - 2);
    }

    #[test]
    fn resize_right_increases_tree_width() {
        let mut app = AppState::new();
        app.focus = FocusTarget::Tree;
        let original = app.tree_width_pct;
        app.execute(Command::ResizeRight);
        assert_eq!(app.tree_width_pct, original + 2);
    }

    #[test]
    fn resize_up_decreases_editor_height() {
        let mut app = AppState::new();
        app.focus = FocusTarget::Editor;
        let original = app.editor_height_pct;
        app.execute(Command::ResizeUp);
        // Up = border moves up = editor shrinks
        assert_eq!(app.editor_height_pct, original - 2);
    }

    #[test]
    fn resize_down_increases_editor_height() {
        let mut app = AppState::new();
        app.focus = FocusTarget::Editor;
        let original = app.editor_height_pct;
        app.execute(Command::ResizeDown);
        // Down = border moves down = editor grows
        assert_eq!(app.editor_height_pct, original + 2);
    }

    #[test]
    fn resize_clamps_at_minimum() {
        let mut app = AppState::new();
        app.focus = FocusTarget::Tree;
        app.tree_width_pct = 10;
        app.execute(Command::ResizeLeft);
        assert_eq!(app.tree_width_pct, 10);
    }

    #[test]
    fn resize_clamps_at_maximum() {
        let mut app = AppState::new();
        app.focus = FocusTarget::Tree;
        app.tree_width_pct = 90;
        app.execute(Command::ResizeRight);
        assert_eq!(app.tree_width_pct, 90);
    }

    #[test]
    fn equalize_layout_resets_defaults() {
        let mut app = AppState::new();
        app.tree_width_pct = 50;
        app.editor_height_pct = 50;
        app.execute(Command::EqualizeLayout);
        assert_eq!(app.tree_width_pct, 20);
        assert_eq!(app.editor_height_pct, 70);
    }

    #[test]
    fn resize_left_noop_when_editor_focused() {
        let mut app = AppState::new();
        app.focus = FocusTarget::Editor;
        let original = app.tree_width_pct;
        app.execute(Command::ResizeLeft);
        assert_eq!(app.tree_width_pct, original);
    }

    #[test]
    fn resize_up_noop_when_tree_focused() {
        let mut app = AppState::new();
        app.focus = FocusTarget::Tree;
        let original = app.editor_height_pct;
        app.execute(Command::ResizeUp);
        assert_eq!(app.editor_height_pct, original);
    }

    #[test]
    fn resize_up_moves_border_up_when_terminal_focused() {
        let mut app = AppState::new();
        app.focus = FocusTarget::Terminal(0);
        let original = app.editor_height_pct;
        app.execute(Command::ResizeUp);
        // Up = border moves up = editor shrinks, terminal grows
        assert_eq!(app.editor_height_pct, original - 2);
    }

    #[test]
    fn resize_down_moves_border_down_when_terminal_focused() {
        let mut app = AppState::new();
        app.focus = FocusTarget::Terminal(0);
        let original = app.editor_height_pct;
        app.execute(Command::ResizeDown);
        // Down = border moves down = editor grows, terminal shrinks
        assert_eq!(app.editor_height_pct, original + 2);
    }

    // --- Resize mode key routing ---

    #[test]
    fn resize_mode_arrow_left_resizes() {
        let mut app = AppState::new();
        app.resize_mode.active = true;
        app.focus = FocusTarget::Tree;
        let original = app.tree_width_pct;
        app.handle_key_event(KeyEvent::new(KeyCode::Left, KeyModifiers::NONE));
        assert_eq!(app.tree_width_pct, original - 2);
    }

    #[test]
    fn resize_mode_arrow_right_resizes() {
        let mut app = AppState::new();
        app.resize_mode.active = true;
        app.focus = FocusTarget::Tree;
        let original = app.tree_width_pct;
        app.handle_key_event(KeyEvent::new(KeyCode::Right, KeyModifiers::NONE));
        assert_eq!(app.tree_width_pct, original + 2);
    }

    #[test]
    fn resize_mode_arrow_up_resizes() {
        let mut app = AppState::new();
        app.resize_mode.active = true;
        app.focus = FocusTarget::Editor;
        let original = app.editor_height_pct;
        app.handle_key_event(KeyEvent::new(KeyCode::Up, KeyModifiers::NONE));
        // Up = border moves up = editor shrinks
        assert_eq!(app.editor_height_pct, original - 2);
    }

    #[test]
    fn resize_mode_arrow_down_resizes() {
        let mut app = AppState::new();
        app.resize_mode.active = true;
        app.focus = FocusTarget::Editor;
        let original = app.editor_height_pct;
        app.handle_key_event(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
        // Down = border moves down = editor grows
        assert_eq!(app.editor_height_pct, original + 2);
    }

    #[test]
    fn resize_mode_esc_exits() {
        let mut app = AppState::new();
        app.resize_mode.active = true;
        app.handle_key_event(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
        assert!(!app.resize_mode.active);
    }

    #[test]
    fn resize_mode_enter_exits() {
        let mut app = AppState::new();
        app.resize_mode.active = true;
        app.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        assert!(!app.resize_mode.active);
    }

    #[test]
    fn resize_mode_equals_equalizes() {
        let mut app = AppState::new();
        app.resize_mode.active = true;
        app.tree_width_pct = 50;
        app.editor_height_pct = 50;
        app.handle_key_event(KeyEvent::new(KeyCode::Char('='), KeyModifiers::NONE));
        assert_eq!(app.tree_width_pct, 20);
        assert_eq!(app.editor_height_pct, 70);
    }

    #[test]
    fn resize_mode_blocks_focus_commands() {
        let mut app = AppState::new();
        app.resize_mode.active = true;
        app.focus = FocusTarget::Editor;
        // Tab should not cycle focus while resize mode is active
        app.handle_key_event(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));
        assert_eq!(app.focus, FocusTarget::Editor);
    }

    #[test]
    fn resize_mode_allows_quit() {
        let mut app = AppState::new();
        app.resize_mode.active = true;
        app.handle_key_event(KeyEvent::new(KeyCode::Char('q'), KeyModifiers::CONTROL));
        assert!(app.should_quit);
    }

    #[test]
    fn handle_ctrl_r_enters_resize_mode() {
        let mut app = AppState::new();
        app.handle_key_event(KeyEvent::new(KeyCode::Char('r'), KeyModifiers::CONTROL));
        assert!(app.resize_mode.active);
    }
}
