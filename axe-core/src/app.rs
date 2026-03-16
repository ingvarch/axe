use std::path::PathBuf;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};

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

/// Which panel border is being dragged.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DragBorder {
    /// Vertical border between tree and editor/terminal.
    Vertical,
    /// Horizontal border between editor and terminal.
    Horizontal,
}

/// Tracks mouse drag state for panel border resizing.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MouseDragState {
    /// Which border is currently being dragged, if any.
    pub border: Option<DragBorder>,
}

/// State for the panel resize mode.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ResizeModeState {
    /// Whether resize mode is currently active.
    pub active: bool,
}

/// Identifies which panel currently has keyboard focus.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum FocusTarget {
    #[default]
    Tree,
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
    /// Whether the quit confirmation dialog is visible.
    pub confirm_quit: bool,
    pub resize_mode: ResizeModeState,
    pub mouse_drag: MouseDragState,
    /// Which panel is currently zoomed to full screen, if any.
    pub zoomed_panel: Option<FocusTarget>,
    pub tree_width_pct: u16,
    pub editor_height_pct: u16,
    /// File tree for the project directory, if loaded.
    pub file_tree: Option<axe_tree::FileTree>,
    /// Terminal emulator manager, if initialized.
    pub terminal_manager: Option<axe_terminal::TerminalManager>,
    keymap: KeymapResolver,
}

impl AppState {
    /// Creates a new `AppState` with default values and no file tree.
    pub fn new() -> Self {
        Self {
            should_quit: false,
            focus: FocusTarget::default(),
            show_tree: true,
            show_terminal: true,
            show_help: false,
            confirm_quit: false,
            resize_mode: ResizeModeState::default(),
            mouse_drag: MouseDragState::default(),
            zoomed_panel: None,
            tree_width_pct: DEFAULT_TREE_WIDTH_PCT,
            editor_height_pct: DEFAULT_EDITOR_HEIGHT_PCT,
            file_tree: None,
            terminal_manager: None,
            keymap: KeymapResolver::with_defaults(),
        }
    }

    /// Creates a new `AppState` with a file tree loaded from the given root directory.
    ///
    /// If the directory cannot be read, logs a warning and falls back to no file tree.
    pub fn new_with_root(root: PathBuf) -> Self {
        let file_tree = match axe_tree::FileTree::new(root) {
            Ok(tree) => Some(tree),
            Err(e) => {
                log::warn!("Failed to load file tree: {e}");
                None
            }
        };
        Self {
            file_tree,
            ..Self::new()
        }
    }

    /// Signals the application to exit the event loop.
    pub fn quit(&mut self) {
        self.confirm_quit = false;
        self.should_quit = true;
    }

    /// Polls terminal output from the PTY background thread and feeds it to the terminal.
    ///
    /// No-op if no terminal manager is initialized.
    pub fn poll_terminal(&mut self) {
        if let Some(ref mut mgr) = self.terminal_manager {
            mgr.poll_output();
        }
    }

    /// Converts a key event to bytes and writes them to the active terminal PTY.
    ///
    /// Reads the application cursor mode from the terminal state to produce the
    /// correct escape sequences for arrow keys.
    fn write_terminal_input(&mut self, key: &KeyEvent) {
        let app_cursor = self
            .terminal_manager
            .as_ref()
            .and_then(|mgr| mgr.active_tab())
            .map(|tab| {
                tab.term()
                    .mode()
                    .contains(alacritty_terminal::term::TermMode::APP_CURSOR)
            })
            .unwrap_or(false);

        if let Some(bytes) = axe_terminal::input::key_to_bytes(key, app_cursor) {
            if let Some(ref mut mgr) = self.terminal_manager {
                if let Err(e) = mgr.write_to_active(&bytes) {
                    log::warn!("Failed to write to terminal: {e}");
                }
            }
        }
    }

    /// Processes a key event by resolving it through the keymap and executing
    /// the resulting command, if any.
    ///
    /// When resize mode is active, arrow keys and special keys are intercepted
    /// before normal dispatch. When a help overlay is open, only Quit, ShowHelp,
    /// and CloseOverlay commands are processed; all other keys are consumed silently.
    pub fn handle_key_event(&mut self, key: KeyEvent) {
        // Quit confirmation dialog intercepts all keys.
        if self.confirm_quit {
            match key.code {
                KeyCode::Char('y') | KeyCode::Char('Y') => self.execute(Command::Quit),
                _ => self.confirm_quit = false,
            }
            return;
        }

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
                (KeyModifiers::CONTROL, KeyCode::Char('q')) => Some(Command::RequestQuit),
                _ => None, // All other keys consumed silently
            };
            if let Some(cmd) = cmd {
                self.execute(cmd);
            }
            return;
        }

        // Tree-focus key interception: handle active actions, navigation, and file operations.
        if self.focus == FocusTarget::Tree && !self.show_help {
            // Layer 1: Active action input handling — consumes ALL keys while active.
            if let Some(ref mut tree) = self.file_tree {
                if tree.is_action_active() {
                    match tree.action().clone() {
                        axe_tree::TreeAction::ConfirmDelete { .. } => match key.code {
                            KeyCode::Char('y') | KeyCode::Char('Y') => {
                                let _ = tree.confirm_delete();
                            }
                            _ => {
                                tree.cancel_action();
                            }
                        },
                        axe_tree::TreeAction::Creating { .. }
                        | axe_tree::TreeAction::Renaming { .. } => match key.code {
                            KeyCode::Enter => {
                                let _ = tree.confirm_action();
                            }
                            KeyCode::Esc => {
                                tree.cancel_action();
                            }
                            KeyCode::Backspace => {
                                tree.input_backspace();
                            }
                            KeyCode::Char(c) => {
                                tree.input_char(c);
                            }
                            _ => {}
                        },
                        axe_tree::TreeAction::Idle => {}
                    }
                    return;
                }
            }

            // Layer 2: Navigation and file operation keys.
            let tree_cmd = match (key.modifiers, key.code) {
                (KeyModifiers::NONE, KeyCode::Up) => Some(Command::TreeUp),
                (KeyModifiers::NONE, KeyCode::Down) => Some(Command::TreeDown),
                (KeyModifiers::NONE, KeyCode::Enter) => Some(Command::TreeToggle),
                (KeyModifiers::NONE, KeyCode::Right) => Some(Command::TreeExpand),
                (KeyModifiers::NONE, KeyCode::Left) => Some(Command::TreeCollapseOrParent),
                (KeyModifiers::NONE, KeyCode::Home) => Some(Command::TreeHome),
                (KeyModifiers::NONE, KeyCode::End) => Some(Command::TreeEnd),
                (KeyModifiers::NONE, KeyCode::Char('n')) => Some(Command::TreeCreateFile),
                (KeyModifiers::SHIFT, KeyCode::Char('N')) => Some(Command::TreeCreateDir),
                (KeyModifiers::NONE, KeyCode::Char('r')) => Some(Command::TreeRename),
                (KeyModifiers::NONE, KeyCode::Char('d')) => Some(Command::TreeDelete),
                _ => None,
            };
            if let Some(cmd) = tree_cmd {
                self.execute(cmd);
                return;
            }
            // Fall through to global keymap for Ctrl+Q, Tab, etc.
        }

        // Terminal-focus key interception: global bindings take priority, everything else
        // is forwarded to the PTY as raw bytes.
        if matches!(self.focus, FocusTarget::Terminal(_)) && !self.show_help {
            if let Some(cmd) = self.keymap.resolve(&key) {
                self.execute(cmd);
            } else {
                self.write_terminal_input(&key);
            }
            return;
        }

        if let Some(cmd) = self.keymap.resolve(&key) {
            if self.show_help {
                match cmd {
                    Command::Quit
                    | Command::RequestQuit
                    | Command::ShowHelp
                    | Command::CloseOverlay => {
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
            Command::RequestQuit => self.confirm_quit = true,
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
            Command::ZoomPanel => self.toggle_zoom(),
            Command::TreeUp => {
                if let Some(ref mut tree) = self.file_tree {
                    tree.move_up();
                }
            }
            Command::TreeDown => {
                if let Some(ref mut tree) = self.file_tree {
                    tree.move_down();
                }
            }
            Command::TreeToggle => {
                if let Some(ref mut tree) = self.file_tree {
                    let _ = tree.toggle();
                }
            }
            Command::TreeExpand => {
                if let Some(ref mut tree) = self.file_tree {
                    let _ = tree.expand();
                }
            }
            Command::TreeCollapseOrParent => {
                if let Some(ref mut tree) = self.file_tree {
                    tree.collapse_or_parent();
                }
            }
            Command::TreeHome => {
                if let Some(ref mut tree) = self.file_tree {
                    tree.move_home();
                }
            }
            Command::TreeEnd => {
                if let Some(ref mut tree) = self.file_tree {
                    tree.move_end();
                }
            }
            Command::ToggleIgnored => {
                if let Some(ref mut tree) = self.file_tree {
                    tree.toggle_show_ignored();
                }
            }
            Command::TreeCreateFile => {
                if let Some(ref mut tree) = self.file_tree {
                    tree.start_create_file();
                }
            }
            Command::TreeCreateDir => {
                if let Some(ref mut tree) = self.file_tree {
                    tree.start_create_dir();
                }
            }
            Command::TreeRename => {
                if let Some(ref mut tree) = self.file_tree {
                    tree.start_rename();
                }
            }
            Command::TreeDelete => {
                if let Some(ref mut tree) = self.file_tree {
                    tree.start_delete();
                }
            }
            Command::ToggleIcons => {
                if let Some(ref mut tree) = self.file_tree {
                    tree.toggle_show_icons();
                }
            }
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

    /// Processes a mouse event for panel border drag-resizing.
    ///
    /// Mouse drag works without entering resize mode -- it's always available.
    /// The caller must provide the current screen dimensions so border positions
    /// can be computed from stored percentages.
    pub fn handle_mouse_event(&mut self, mouse: MouseEvent, screen_width: u16, screen_height: u16) {
        /// Border detection tolerance in cells.
        const BORDER_TOLERANCE: u16 = 1;
        /// Status bar height in rows.
        const STATUS_BAR_HEIGHT: u16 = 1;

        let main_height = screen_height.saturating_sub(STATUS_BAR_HEIGHT);

        match mouse.kind {
            MouseEventKind::Down(MouseButton::Left) => {
                let col = mouse.column;
                let row = mouse.row;

                // Check vertical border (tree/editor boundary)
                if self.show_tree {
                    let border_x =
                        (u32::from(screen_width) * u32::from(self.tree_width_pct) / 100) as u16;
                    if col.abs_diff(border_x) <= BORDER_TOLERANCE && row < main_height {
                        self.mouse_drag.border = Some(DragBorder::Vertical);
                        return;
                    }
                }

                // Check horizontal border (editor/terminal boundary)
                if self.show_terminal && self.show_tree {
                    let right_x =
                        (u32::from(screen_width) * u32::from(self.tree_width_pct) / 100) as u16;
                    let right_height = main_height;
                    let border_y =
                        (u32::from(right_height) * u32::from(self.editor_height_pct) / 100) as u16;
                    if row.abs_diff(border_y) <= BORDER_TOLERANCE && col >= right_x {
                        self.mouse_drag.border = Some(DragBorder::Horizontal);
                        return;
                    }
                } else if self.show_terminal {
                    // Tree hidden: horizontal border spans entire width
                    let border_y =
                        (u32::from(main_height) * u32::from(self.editor_height_pct) / 100) as u16;
                    if row.abs_diff(border_y) <= BORDER_TOLERANCE {
                        self.mouse_drag.border = Some(DragBorder::Horizontal);
                        return;
                    }
                }

                // No border hit — focus the clicked panel
                if row < main_height {
                    self.focus = self.panel_at(col, row, screen_width, main_height);
                }
            }
            MouseEventKind::Drag(MouseButton::Left) => {
                match self.mouse_drag.border {
                    Some(DragBorder::Vertical) => {
                        if !self.show_tree || screen_width == 0 {
                            return;
                        }
                        let new_pct =
                            (u32::from(mouse.column) * 100 / u32::from(screen_width)) as u16;
                        self.tree_width_pct = new_pct.clamp(MIN_PANEL_PCT, MAX_PANEL_PCT);
                    }
                    Some(DragBorder::Horizontal) => {
                        if !self.show_terminal || main_height == 0 {
                            return;
                        }
                        // Compute right area start (0 if tree hidden)
                        let right_area_start_y: u16 = 0;
                        let right_area_height = main_height;
                        let relative_row = mouse.row.saturating_sub(right_area_start_y);
                        let new_pct =
                            (u32::from(relative_row) * 100 / u32::from(right_area_height)) as u16;
                        self.editor_height_pct = new_pct.clamp(MIN_PANEL_PCT, MAX_PANEL_PCT);
                    }
                    None => {}
                }
            }
            MouseEventKind::Up(MouseButton::Left) => {
                self.mouse_drag.border = None;
            }
            _ => {}
        }
    }

    /// Determines which panel occupies the given screen position.
    fn panel_at(&self, col: u16, row: u16, screen_width: u16, main_height: u16) -> FocusTarget {
        let tree_border_x = (u32::from(screen_width) * u32::from(self.tree_width_pct) / 100) as u16;

        if self.show_tree && col < tree_border_x {
            return FocusTarget::Tree;
        }

        if self.show_terminal {
            let border_y =
                (u32::from(main_height) * u32::from(self.editor_height_pct) / 100) as u16;
            if row >= border_y {
                return FocusTarget::Terminal(0);
            }
        }

        FocusTarget::Editor
    }

    /// Toggles zoom on the focused panel.
    ///
    /// - `None` -> zoom current focus
    /// - `Some(x)` where `x == focus` -> un-zoom
    /// - `Some(_)` -> switch zoom to current focus
    ///
    // IMPACT ANALYSIS — toggle_zoom
    // Parents: KeyEvent → Command::ZoomPanel → this function
    // Children: render() checks zoomed_panel to decide layout
    // Siblings: resize_mode (must be deactivated), focus cycling (unaffected),
    //           mouse drag (unaffected — drag still updates percentages even while zoomed)
    // Risk: None — zoomed_panel is purely additive, no existing state is modified
    fn toggle_zoom(&mut self) {
        self.resize_mode.active = false;
        if self.zoomed_panel.as_ref() == Some(&self.focus) {
            self.zoomed_panel = None;
        } else {
            self.zoomed_panel = Some(self.focus.clone());
        }
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
    fn focus_target_default_is_tree() {
        assert_eq!(FocusTarget::default(), FocusTarget::Tree);
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
    fn app_state_default_focus_is_tree() {
        let app = AppState::new();
        assert_eq!(app.focus, FocusTarget::Tree);
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
        assert_eq!(app.focus, FocusTarget::Tree);
        app.execute(Command::FocusNext);
        assert_eq!(app.focus, FocusTarget::Editor);
        app.execute(Command::FocusNext);
        assert_eq!(app.focus, FocusTarget::Terminal(0));
        app.execute(Command::FocusNext);
        assert_eq!(app.focus, FocusTarget::Tree);
    }

    #[test]
    fn execute_focus_prev_cycles_focus() {
        let mut app = AppState::new();
        assert_eq!(app.focus, FocusTarget::Tree);
        app.execute(Command::FocusPrev);
        assert_eq!(app.focus, FocusTarget::Terminal(0));
        app.execute(Command::FocusPrev);
        assert_eq!(app.focus, FocusTarget::Editor);
        app.execute(Command::FocusPrev);
        assert_eq!(app.focus, FocusTarget::Tree);
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
    fn handle_key_ctrl_q_shows_confirm_quit() {
        let mut app = AppState::new();
        app.handle_key_event(KeyEvent::new(KeyCode::Char('q'), KeyModifiers::CONTROL));
        assert!(!app.should_quit, "Ctrl+Q should not quit immediately");
        assert!(app.confirm_quit, "Ctrl+Q should show quit confirmation");
    }

    #[test]
    fn confirm_quit_y_quits() {
        let mut app = AppState::new();
        app.confirm_quit = true;
        app.handle_key_event(KeyEvent::new(KeyCode::Char('y'), KeyModifiers::NONE));
        assert!(app.should_quit);
        assert!(!app.confirm_quit);
    }

    #[test]
    fn confirm_quit_other_key_cancels() {
        let mut app = AppState::new();
        app.confirm_quit = true;
        app.handle_key_event(KeyEvent::new(KeyCode::Char('n'), KeyModifiers::NONE));
        assert!(!app.should_quit);
        assert!(!app.confirm_quit);
    }

    #[test]
    fn confirm_quit_esc_cancels() {
        let mut app = AppState::new();
        app.confirm_quit = true;
        app.handle_key_event(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
        assert!(!app.should_quit);
        assert!(!app.confirm_quit);
    }

    #[test]
    fn handle_key_q_does_not_quit() {
        let mut app = AppState::new();
        app.handle_key_event(KeyEvent::new(KeyCode::Char('q'), KeyModifiers::NONE));
        assert!(!app.should_quit);
    }

    #[test]
    fn ctrl_c_not_bound_globally() {
        let mut app = AppState::new();
        app.handle_key_event(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL));
        assert!(!app.should_quit);
        assert!(!app.confirm_quit);
    }

    #[test]
    fn handle_other_key_does_not_quit() {
        let mut app = AppState::new();
        app.handle_key_event(KeyEvent::new(KeyCode::Char('a'), KeyModifiers::NONE));
        assert!(!app.should_quit);
    }

    #[test]
    fn tab_not_bound_globally() {
        let mut app = AppState::new();
        assert_eq!(app.focus, FocusTarget::Tree);
        app.handle_key_event(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));
        // Tab is no longer a global binding — focus should not change.
        assert_eq!(app.focus, FocusTarget::Tree);
    }

    #[test]
    fn handle_shift_tab_cycles_focus_backward() {
        let mut app = AppState::new();
        assert_eq!(app.focus, FocusTarget::Tree);
        app.handle_key_event(KeyEvent::new(KeyCode::BackTab, KeyModifiers::SHIFT));
        assert_eq!(app.focus, FocusTarget::Terminal(0));
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
    fn tab_from_terminal_forwarded_to_pty() {
        let mut app = AppState::new();
        app.focus = FocusTarget::Terminal(0);
        app.handle_key_event(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));
        // Tab is forwarded to PTY, not used for focus cycling.
        assert_eq!(app.focus, FocusTarget::Terminal(0));
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
        assert_eq!(app.focus, FocusTarget::Tree);
    }

    #[test]
    fn help_overlay_allows_request_quit() {
        let mut app = AppState::new();
        app.show_help = true;
        app.handle_key_event(KeyEvent::new(KeyCode::Char('q'), KeyModifiers::CONTROL));
        assert!(
            app.confirm_quit,
            "Ctrl+Q should show quit dialog even with help open"
        );
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
    fn resize_mode_allows_request_quit() {
        let mut app = AppState::new();
        app.resize_mode.active = true;
        app.handle_key_event(KeyEvent::new(KeyCode::Char('q'), KeyModifiers::CONTROL));
        assert!(
            app.confirm_quit,
            "Ctrl+Q should show quit dialog in resize mode"
        );
    }

    #[test]
    fn handle_ctrl_r_enters_resize_mode() {
        let mut app = AppState::new();
        app.handle_key_event(KeyEvent::new(KeyCode::Char('r'), KeyModifiers::CONTROL));
        assert!(app.resize_mode.active);
    }

    // --- Mouse drag resize tests ---

    fn mouse_event(kind: MouseEventKind, col: u16, row: u16) -> MouseEvent {
        MouseEvent {
            kind,
            column: col,
            row,
            modifiers: KeyModifiers::NONE,
        }
    }

    #[test]
    fn mouse_drag_inactive_by_default() {
        let app = AppState::new();
        assert_eq!(app.mouse_drag.border, None);
    }

    #[test]
    fn mouse_down_near_vertical_border_starts_drag() {
        let mut app = AppState::new();
        // tree_width_pct = 20, screen_width = 100 → border at col 20
        let evt = mouse_event(MouseEventKind::Down(MouseButton::Left), 20, 5);
        app.handle_mouse_event(evt, 100, 30);
        assert_eq!(app.mouse_drag.border, Some(DragBorder::Vertical));
    }

    #[test]
    fn mouse_down_near_horizontal_border_starts_drag() {
        let mut app = AppState::new();
        // editor_height_pct = 70, main_height = 29 (30-1 status), border_y = 29*70/100 = 20
        // col must be >= tree border (col 20 for 20% of 100)
        let evt = mouse_event(MouseEventKind::Down(MouseButton::Left), 50, 20);
        app.handle_mouse_event(evt, 100, 30);
        assert_eq!(app.mouse_drag.border, Some(DragBorder::Horizontal));
    }

    #[test]
    fn mouse_down_away_from_border_no_drag() {
        let mut app = AppState::new();
        // Click in the middle of editor area, far from any border
        let evt = mouse_event(MouseEventKind::Down(MouseButton::Left), 60, 5);
        app.handle_mouse_event(evt, 100, 30);
        assert_eq!(app.mouse_drag.border, None);
    }

    #[test]
    fn mouse_drag_vertical_updates_tree_width() {
        let mut app = AppState::new();
        // Start drag on vertical border
        app.mouse_drag.border = Some(DragBorder::Vertical);
        // Drag to col 30 of 100 → 30%
        let evt = mouse_event(MouseEventKind::Drag(MouseButton::Left), 30, 5);
        app.handle_mouse_event(evt, 100, 30);
        assert_eq!(app.tree_width_pct, 30);
    }

    #[test]
    fn mouse_drag_horizontal_updates_editor_height() {
        let mut app = AppState::new();
        app.mouse_drag.border = Some(DragBorder::Horizontal);
        // main_height = 29, drag to row 14 → 14*100/29 = 48%
        let evt = mouse_event(MouseEventKind::Drag(MouseButton::Left), 50, 14);
        app.handle_mouse_event(evt, 100, 30);
        assert_eq!(app.editor_height_pct, 48);
    }

    #[test]
    fn mouse_up_ends_drag() {
        let mut app = AppState::new();
        app.mouse_drag.border = Some(DragBorder::Vertical);
        let evt = mouse_event(MouseEventKind::Up(MouseButton::Left), 30, 5);
        app.handle_mouse_event(evt, 100, 30);
        assert_eq!(app.mouse_drag.border, None);
    }

    #[test]
    fn mouse_drag_clamps_at_minimum() {
        let mut app = AppState::new();
        app.mouse_drag.border = Some(DragBorder::Vertical);
        // Drag to col 2 of 100 → 2%, should clamp to 10%
        let evt = mouse_event(MouseEventKind::Drag(MouseButton::Left), 2, 5);
        app.handle_mouse_event(evt, 100, 30);
        assert_eq!(app.tree_width_pct, MIN_PANEL_PCT);
    }

    #[test]
    fn mouse_drag_clamps_at_maximum() {
        let mut app = AppState::new();
        app.mouse_drag.border = Some(DragBorder::Vertical);
        // Drag to col 98 of 100 → 98%, should clamp to 90%
        let evt = mouse_event(MouseEventKind::Drag(MouseButton::Left), 98, 5);
        app.handle_mouse_event(evt, 100, 30);
        assert_eq!(app.tree_width_pct, MAX_PANEL_PCT);
    }

    #[test]
    fn mouse_drag_vertical_noop_when_tree_hidden() {
        let mut app = AppState::new();
        app.show_tree = false;
        app.mouse_drag.border = Some(DragBorder::Vertical);
        let original = app.tree_width_pct;
        let evt = mouse_event(MouseEventKind::Drag(MouseButton::Left), 30, 5);
        app.handle_mouse_event(evt, 100, 30);
        assert_eq!(app.tree_width_pct, original);
    }

    #[test]
    fn mouse_drag_horizontal_noop_when_terminal_hidden() {
        let mut app = AppState::new();
        app.show_terminal = false;
        app.mouse_drag.border = Some(DragBorder::Horizontal);
        let original = app.editor_height_pct;
        let evt = mouse_event(MouseEventKind::Drag(MouseButton::Left), 50, 14);
        app.handle_mouse_event(evt, 100, 30);
        assert_eq!(app.editor_height_pct, original);
    }

    #[test]
    fn mouse_drag_ignores_right_button() {
        let mut app = AppState::new();
        // Right-click near vertical border should not start drag
        let evt = mouse_event(MouseEventKind::Down(MouseButton::Right), 20, 5);
        app.handle_mouse_event(evt, 100, 30);
        assert_eq!(app.mouse_drag.border, None);
    }

    // --- Mouse click focus tests ---

    #[test]
    fn mouse_click_in_tree_focuses_tree() {
        let mut app = AppState::new();
        app.focus = FocusTarget::Editor;
        // tree_width_pct = 20, screen_width = 100 → tree occupies cols 0..20
        // Click at col 5 (well inside tree area)
        let evt = mouse_event(MouseEventKind::Down(MouseButton::Left), 5, 5);
        app.handle_mouse_event(evt, 100, 30);
        assert_eq!(app.focus, FocusTarget::Tree);
    }

    #[test]
    fn mouse_click_in_editor_focuses_editor() {
        let mut app = AppState::new();
        app.focus = FocusTarget::Tree;
        // Click at col 60, row 5 (well inside editor area, above horizontal border)
        let evt = mouse_event(MouseEventKind::Down(MouseButton::Left), 60, 5);
        app.handle_mouse_event(evt, 100, 30);
        assert_eq!(app.focus, FocusTarget::Editor);
    }

    #[test]
    fn mouse_click_in_terminal_focuses_terminal() {
        let mut app = AppState::new();
        app.focus = FocusTarget::Editor;
        // editor_height_pct = 70, main_height = 29, border_y = 20
        // Click at col 60, row 25 (below the horizontal border)
        let evt = mouse_event(MouseEventKind::Down(MouseButton::Left), 60, 25);
        app.handle_mouse_event(evt, 100, 30);
        assert_eq!(app.focus, FocusTarget::Terminal(0));
    }

    #[test]
    fn mouse_click_in_editor_when_tree_hidden() {
        let mut app = AppState::new();
        app.show_tree = false;
        app.focus = FocusTarget::Terminal(0);
        // Tree hidden, click at col 5 row 5 → editor (no tree to click)
        let evt = mouse_event(MouseEventKind::Down(MouseButton::Left), 5, 5);
        app.handle_mouse_event(evt, 100, 30);
        assert_eq!(app.focus, FocusTarget::Editor);
    }

    #[test]
    fn mouse_click_in_editor_when_terminal_hidden() {
        let mut app = AppState::new();
        app.show_terminal = false;
        app.focus = FocusTarget::Tree;
        // Terminal hidden, click at col 60 row 25 → editor (no terminal)
        let evt = mouse_event(MouseEventKind::Down(MouseButton::Left), 60, 25);
        app.handle_mouse_event(evt, 100, 30);
        assert_eq!(app.focus, FocusTarget::Editor);
    }

    #[test]
    fn mouse_click_on_border_does_not_change_focus() {
        let mut app = AppState::new();
        assert_eq!(app.focus, FocusTarget::Tree);
        // Click right on the vertical border → starts drag, does NOT change focus
        let evt = mouse_event(MouseEventKind::Down(MouseButton::Left), 20, 5);
        app.handle_mouse_event(evt, 100, 30);
        assert_eq!(app.focus, FocusTarget::Tree);
        assert_eq!(app.mouse_drag.border, Some(DragBorder::Vertical));
    }

    #[test]
    fn mouse_click_in_status_bar_does_not_change_focus() {
        let mut app = AppState::new();
        app.focus = FocusTarget::Tree;
        // Status bar is the last row (row 29 for screen_height=30)
        let evt = mouse_event(MouseEventKind::Down(MouseButton::Left), 50, 29);
        app.handle_mouse_event(evt, 100, 30);
        assert_eq!(app.focus, FocusTarget::Tree);
    }

    // --- Zoom panel tests ---

    #[test]
    fn zoomed_panel_none_by_default() {
        let app = AppState::new();
        assert_eq!(app.zoomed_panel, None);
    }

    #[test]
    fn zoom_panel_sets_zoomed_to_current_focus() {
        let mut app = AppState::new();
        app.focus = FocusTarget::Editor;
        app.execute(Command::ZoomPanel);
        assert_eq!(app.zoomed_panel, Some(FocusTarget::Editor));
    }

    #[test]
    fn zoom_panel_again_unzooms() {
        let mut app = AppState::new();
        app.focus = FocusTarget::Editor;
        app.execute(Command::ZoomPanel);
        app.execute(Command::ZoomPanel);
        assert_eq!(app.zoomed_panel, None);
    }

    #[test]
    fn zoom_panel_switches_zoom_to_new_focus() {
        let mut app = AppState::new();
        app.focus = FocusTarget::Editor;
        app.execute(Command::ZoomPanel);
        app.focus = FocusTarget::Tree;
        app.execute(Command::ZoomPanel);
        assert_eq!(app.zoomed_panel, Some(FocusTarget::Tree));
    }

    #[test]
    fn zoom_panel_exits_resize_mode() {
        let mut app = AppState::new();
        app.resize_mode.active = true;
        app.execute(Command::ZoomPanel);
        assert!(!app.resize_mode.active);
    }

    #[test]
    fn handle_key_ctrl_z_toggles_zoom() {
        let mut app = AppState::new();
        app.focus = FocusTarget::Editor;
        app.handle_key_event(KeyEvent::new(KeyCode::Char('z'), KeyModifiers::CONTROL));
        assert_eq!(app.zoomed_panel, Some(FocusTarget::Editor));
        app.handle_key_event(KeyEvent::new(KeyCode::Char('z'), KeyModifiers::CONTROL));
        assert_eq!(app.zoomed_panel, None);
    }

    #[test]
    fn mouse_right_click_does_not_change_focus() {
        let mut app = AppState::new();
        app.focus = FocusTarget::Editor;
        // Right-click in tree area should not change focus
        let evt = mouse_event(MouseEventKind::Down(MouseButton::Right), 5, 5);
        app.handle_mouse_event(evt, 100, 30);
        assert_eq!(app.focus, FocusTarget::Editor);
    }

    // --- FileTree integration tests ---

    #[test]
    fn new_has_no_file_tree() {
        let app = AppState::new();
        assert!(app.file_tree.is_none());
    }

    #[test]
    fn new_with_root_has_file_tree() {
        let tmp = std::env::temp_dir();
        let app = AppState::new_with_root(tmp);
        assert!(app.file_tree.is_some());
    }

    #[test]
    fn new_with_root_invalid_path_has_no_file_tree() {
        let app = AppState::new_with_root(PathBuf::from("/nonexistent/path/12345"));
        assert!(app.file_tree.is_none());
    }

    // --- Tree navigation key routing tests ---

    fn app_with_tree_focused() -> AppState {
        let tmp = std::env::temp_dir();
        let mut app = AppState::new_with_root(tmp);
        app.focus = FocusTarget::Tree;
        app
    }

    #[test]
    fn tree_down_when_focused() {
        let mut app = app_with_tree_focused();
        let initial = app.file_tree.as_ref().unwrap().selected();
        app.handle_key_event(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
        assert_ne!(app.file_tree.as_ref().unwrap().selected(), initial);
    }

    #[test]
    fn tree_up_when_focused() {
        let mut app = app_with_tree_focused();
        // Move down first, then up
        app.handle_key_event(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
        let after_down = app.file_tree.as_ref().unwrap().selected();
        app.handle_key_event(KeyEvent::new(KeyCode::Up, KeyModifiers::NONE));
        assert_ne!(app.file_tree.as_ref().unwrap().selected(), after_down);
    }

    #[test]
    fn arrows_not_intercepted_when_editor_focused() {
        let tmp = std::env::temp_dir();
        let mut app = AppState::new_with_root(tmp);
        app.focus = FocusTarget::Editor;
        let initial = app.file_tree.as_ref().unwrap().selected();
        app.handle_key_event(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
        // Down arrow in editor mode should not affect tree
        assert_eq!(app.file_tree.as_ref().unwrap().selected(), initial);
    }

    #[test]
    fn tree_keys_blocked_when_help_open() {
        let mut app = app_with_tree_focused();
        app.show_help = true;
        let initial = app.file_tree.as_ref().unwrap().selected();
        app.handle_key_event(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
        assert_eq!(app.file_tree.as_ref().unwrap().selected(), initial);
    }

    #[test]
    fn global_keys_work_when_tree_focused() {
        let mut app = app_with_tree_focused();
        // Ctrl+Q should show quit confirmation
        app.handle_key_event(KeyEvent::new(KeyCode::Char('q'), KeyModifiers::CONTROL));
        assert!(app.confirm_quit);
    }

    #[test]
    fn tab_not_intercepted_when_tree_focused() {
        let mut app = app_with_tree_focused();
        app.handle_key_event(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));
        // Tab is not a global binding, and not a tree-specific key.
        // It falls through to global keymap which returns None.
        assert_eq!(app.focus, FocusTarget::Tree);
    }

    // --- Toggle ignored tests ---

    #[test]
    fn toggle_ignored_toggles_filter() {
        let mut app = app_with_tree_focused();
        assert!(app.file_tree.as_ref().unwrap().show_ignored());
        app.execute(Command::ToggleIgnored);
        assert!(!app.file_tree.as_ref().unwrap().show_ignored());
    }

    #[test]
    fn ctrl_g_toggles_ignored() {
        let mut app = app_with_tree_focused();
        assert!(app.file_tree.as_ref().unwrap().show_ignored());
        app.handle_key_event(KeyEvent::new(KeyCode::Char('g'), KeyModifiers::CONTROL));
        assert!(!app.file_tree.as_ref().unwrap().show_ignored());
    }

    #[test]
    fn new_has_no_terminal_manager() {
        let app = AppState::new();
        assert!(app.terminal_manager.is_none());
    }

    #[test]
    fn poll_terminal_noop_without_manager() {
        let mut app = AppState::new();
        app.poll_terminal(); // Should not panic.
    }

    // --- Terminal key interception tests ---

    #[test]
    fn terminal_focused_printable_key_not_handled_as_command() {
        let mut app = AppState::new();
        app.focus = FocusTarget::Terminal(0);
        // Typing 'a' should not trigger quit or any command side effect.
        app.handle_key_event(KeyEvent::new(KeyCode::Char('a'), KeyModifiers::NONE));
        assert!(!app.should_quit);
        // Focus should remain on terminal (not cycled).
        assert_eq!(app.focus, FocusTarget::Terminal(0));
    }

    #[test]
    fn terminal_focused_ctrl_q_shows_confirm() {
        let mut app = AppState::new();
        app.focus = FocusTarget::Terminal(0);
        app.handle_key_event(KeyEvent::new(KeyCode::Char('q'), KeyModifiers::CONTROL));
        assert!(
            app.confirm_quit,
            "Ctrl+Q should show quit dialog from terminal"
        );
        assert!(!app.should_quit);
    }

    #[test]
    fn terminal_focused_tab_forwarded_to_pty() {
        let mut app = AppState::new();
        app.focus = FocusTarget::Terminal(0);
        app.handle_key_event(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));
        // Tab is forwarded to PTY, not used for focus cycling.
        assert_eq!(app.focus, FocusTarget::Terminal(0));
    }

    #[test]
    fn terminal_focused_ctrl_c_forwarded_to_pty() {
        // Ctrl+C is no longer a global binding — it's forwarded to the PTY
        // so shell processes can be interrupted.
        let mut app = AppState::new();
        app.focus = FocusTarget::Terminal(0);
        app.handle_key_event(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL));
        assert!(!app.should_quit);
        assert!(!app.confirm_quit);
        assert_eq!(app.focus, FocusTarget::Terminal(0));
    }

    #[test]
    fn terminal_focused_enter_not_handled_as_command() {
        let mut app = AppState::new();
        app.focus = FocusTarget::Terminal(0);
        app.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        assert!(!app.should_quit);
        assert_eq!(app.focus, FocusTarget::Terminal(0));
    }

    #[test]
    fn terminal_focused_arrow_keys_not_handled_as_command() {
        let mut app = AppState::new();
        app.focus = FocusTarget::Terminal(0);
        app.handle_key_event(KeyEvent::new(KeyCode::Up, KeyModifiers::NONE));
        assert_eq!(app.focus, FocusTarget::Terminal(0));
        assert!(!app.should_quit);
    }
}
