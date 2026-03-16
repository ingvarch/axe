use std::path::PathBuf;
use std::time::{Duration, Instant};

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};

use alacritty_terminal::index::{Column, Direction, Line, Point};
use alacritty_terminal::selection::SelectionType;

use axe_tree::NodeKind;

use crate::command::Command;
use crate::keymap::KeymapResolver;
use crate::search::SearchState;

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
/// Number of lines to scroll per mouse wheel tick.
const MOUSE_SCROLL_LINES: i32 = 3;
/// Delay after the last edit before triggering autosave.
const AUTOSAVE_DELAY: Duration = Duration::from_secs(2);
/// How long a status message remains visible.
const STATUS_MESSAGE_DURATION: Duration = Duration::from_secs(3);

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

/// Default terminal size used when the actual panel size is not yet known.
const DEFAULT_TERMINAL_COLS: u16 = 80;
/// Default terminal rows used when the actual panel size is not yet known.
const DEFAULT_TERMINAL_ROWS: u16 = 24;

/// Central application state shared across all subsystems.
pub struct AppState {
    pub should_quit: bool,
    pub focus: FocusTarget,
    pub show_tree: bool,
    pub show_terminal: bool,
    pub show_help: bool,
    /// Whether the quit confirmation dialog is visible.
    pub confirm_quit: bool,
    /// Whether the "close modified buffer?" confirmation dialog is visible.
    pub confirm_close_buffer: bool,
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
    /// Project root directory for spawning new terminals.
    pub project_root: Option<PathBuf>,
    /// Last known terminal panel width in columns.
    pub last_terminal_cols: u16,
    /// Last known terminal panel height in rows.
    pub last_terminal_rows: u16,
    /// Editor buffer manager holding all open file buffers.
    pub buffer_manager: axe_editor::BufferManager,
    /// Terminal grid area in screen coordinates (x, y, width, height).
    ///
    /// Updated each frame from `terminal_inner_rect()` in main.rs.
    /// Used for converting mouse screen coordinates to grid coordinates.
    pub terminal_grid_area: Option<(u16, u16, u16, u16)>,
    /// Temporary status message shown in the status bar.
    ///
    /// Cleared automatically after `STATUS_MESSAGE_DURATION` elapses.
    pub status_message: Option<(String, Instant)>,
    /// Editor content area in screen coordinates (x, y, width, height).
    ///
    /// Updated each frame from `editor_inner_rect()` in main.rs.
    /// Used for viewport calculations and mouse coordinate conversion.
    pub editor_inner_area: Option<(u16, u16, u16, u16)>,
    /// Editor tab bar area in screen coordinates (x, y, width, height).
    ///
    /// Set each frame by the UI when tab bar is visible.
    /// Used for detecting mouse clicks on editor buffer tabs.
    pub editor_tab_bar_area: Option<(u16, u16, u16, u16)>,
    /// Timestamp of the last edit operation, used for autosave debouncing.
    pub last_edit_time: Option<Instant>,
    /// System clipboard for copy/paste operations.
    ///
    /// Lazily initialized on first use. `None` if clipboard access fails
    /// (e.g., headless/SSH environment).
    clipboard: Option<arboard::Clipboard>,
    /// Whether an editor text selection drag is currently in progress.
    editor_selecting: bool,
    /// Active search state, if the search bar is open.
    pub search: Option<SearchState>,
    /// Whether a terminal text selection drag is currently in progress.
    terminal_selecting: bool,
    /// Screen position where the last mouse-down occurred in the terminal grid.
    /// Used to distinguish clicks (no movement) from drags.
    terminal_select_start: Option<(u16, u16)>,
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
            confirm_close_buffer: false,
            resize_mode: ResizeModeState::default(),
            mouse_drag: MouseDragState::default(),
            zoomed_panel: None,
            tree_width_pct: DEFAULT_TREE_WIDTH_PCT,
            editor_height_pct: DEFAULT_EDITOR_HEIGHT_PCT,
            file_tree: None,
            buffer_manager: axe_editor::BufferManager::new(),
            terminal_manager: None,
            project_root: None,
            last_terminal_cols: DEFAULT_TERMINAL_COLS,
            last_terminal_rows: DEFAULT_TERMINAL_ROWS,
            terminal_grid_area: None,
            status_message: None,
            editor_inner_area: None,
            editor_tab_bar_area: None,
            last_edit_time: None,
            clipboard: None,
            search: None,
            editor_selecting: false,
            terminal_selecting: false,
            terminal_select_start: None,
            keymap: KeymapResolver::with_defaults(),
        }
    }

    /// Creates a new `AppState` with a file tree loaded from the given root directory.
    ///
    /// If the directory cannot be read, logs a warning and falls back to no file tree.
    pub fn new_with_root(root: PathBuf) -> Self {
        let file_tree = match axe_tree::FileTree::new(root.clone()) {
            Ok(tree) => Some(tree),
            Err(e) => {
                log::warn!("Failed to load file tree: {e}");
                None
            }
        };
        Self {
            file_tree,
            project_root: Some(root),
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
    /// Automatically closes tabs whose child processes have exited (e.g. user typed `exit`
    /// or pressed Ctrl+D in the shell). Updates focus accordingly.
    pub fn poll_terminal(&mut self) {
        if let Some(ref mut mgr) = self.terminal_manager {
            let exited = mgr.poll_output();
            if !exited.is_empty() {
                // Close exited tabs back-to-front (indices are sorted descending).
                for idx in exited {
                    if let Err(e) = mgr.close_tab(idx) {
                        log::warn!("Failed to close exited terminal tab {idx}: {e}");
                    }
                }

                if mgr.has_tabs() {
                    // Still have tabs — sync focus to the new active tab.
                    if matches!(self.focus, FocusTarget::Terminal(_)) {
                        self.focus = FocusTarget::Terminal(mgr.active_index());
                    }
                } else {
                    // Last tab closed — hide terminal panel, move focus to editor.
                    self.show_terminal = false;
                    if matches!(self.focus, FocusTarget::Terminal(_)) {
                        self.focus = FocusTarget::Editor;
                    }
                }
            }
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

        // Close-buffer confirmation dialog intercepts all keys.
        if self.confirm_close_buffer {
            match key.code {
                KeyCode::Char('y') | KeyCode::Char('Y') => {
                    self.execute(Command::ConfirmCloseBuffer);
                }
                _ => {
                    self.execute(Command::CancelCloseBuffer);
                }
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

        // Search bar active: intercept keys for search input before editor keys.
        if self.search.is_some() && self.focus == FocusTarget::Editor && !self.show_help {
            match (key.modifiers, key.code) {
                (KeyModifiers::NONE, KeyCode::Esc) => {
                    self.execute(Command::SearchClose);
                    return;
                }
                (KeyModifiers::NONE, KeyCode::Enter) => {
                    self.execute(Command::SearchNextMatch);
                    return;
                }
                (KeyModifiers::SHIFT, KeyCode::Enter) => {
                    self.execute(Command::SearchPrevMatch);
                    return;
                }
                (KeyModifiers::ALT, KeyCode::Char('c')) => {
                    self.execute(Command::SearchToggleCase);
                    return;
                }
                (KeyModifiers::ALT, KeyCode::Char('r')) => {
                    self.execute(Command::SearchToggleRegex);
                    return;
                }
                (KeyModifiers::NONE, KeyCode::Backspace) => {
                    if let Some(ref mut search) = self.search {
                        if let Some(buf) = self.buffer_manager.active_buffer() {
                            search.input_backspace(buf);
                        }
                    }
                    return;
                }
                (KeyModifiers::NONE, KeyCode::Char(c))
                | (KeyModifiers::SHIFT, KeyCode::Char(c)) => {
                    if let Some(ref mut search) = self.search {
                        if let Some(buf) = self.buffer_manager.active_buffer() {
                            search.input_char(c, buf);
                        }
                    }
                    return;
                }
                _ => {
                    // Let Ctrl+F, Ctrl+Q, etc. fall through to global keymap.
                }
            }
        }

        // Editor-focus key interception: cursor movement and navigation.
        if self.focus == FocusTarget::Editor && !self.show_help {
            let editor_cmd = match (key.modifiers, key.code) {
                (KeyModifiers::NONE, KeyCode::Up) => Some(Command::EditorUp),
                (KeyModifiers::NONE, KeyCode::Down) => Some(Command::EditorDown),
                (KeyModifiers::NONE, KeyCode::Left) => Some(Command::EditorLeft),
                (KeyModifiers::NONE, KeyCode::Right) => Some(Command::EditorRight),
                (KeyModifiers::NONE, KeyCode::Home) => Some(Command::EditorHome),
                (KeyModifiers::NONE, KeyCode::End) => Some(Command::EditorEnd),
                (KeyModifiers::CONTROL, KeyCode::Home) => Some(Command::EditorFileStart),
                (KeyModifiers::CONTROL, KeyCode::End) => Some(Command::EditorFileEnd),
                (KeyModifiers::NONE, KeyCode::PageUp) => Some(Command::EditorPageUp),
                (KeyModifiers::NONE, KeyCode::PageDown) => Some(Command::EditorPageDown),
                (KeyModifiers::CONTROL, KeyCode::Left) => Some(Command::EditorWordLeft),
                (KeyModifiers::CONTROL, KeyCode::Right) => Some(Command::EditorWordRight),
                // Selection movement: Shift+Arrow extends selection.
                (KeyModifiers::SHIFT, KeyCode::Up) => Some(Command::EditorSelectUp),
                (KeyModifiers::SHIFT, KeyCode::Down) => Some(Command::EditorSelectDown),
                (KeyModifiers::SHIFT, KeyCode::Left) => Some(Command::EditorSelectLeft),
                (KeyModifiers::SHIFT, KeyCode::Right) => Some(Command::EditorSelectRight),
                (KeyModifiers::SHIFT, KeyCode::Home) => Some(Command::EditorSelectHome),
                (KeyModifiers::SHIFT, KeyCode::End) => Some(Command::EditorSelectEnd),
                (m, KeyCode::Home) if m == KeyModifiers::CONTROL | KeyModifiers::SHIFT => {
                    Some(Command::EditorSelectFileStart)
                }
                (m, KeyCode::End) if m == KeyModifiers::CONTROL | KeyModifiers::SHIFT => {
                    Some(Command::EditorSelectFileEnd)
                }
                (m, KeyCode::Left) if m == KeyModifiers::CONTROL | KeyModifiers::SHIFT => {
                    Some(Command::EditorSelectWordLeft)
                }
                (m, KeyCode::Right) if m == KeyModifiers::CONTROL | KeyModifiers::SHIFT => {
                    Some(Command::EditorSelectWordRight)
                }
                (KeyModifiers::NONE, KeyCode::Backspace) => Some(Command::EditorBackspace),
                (KeyModifiers::NONE, KeyCode::Delete) => Some(Command::EditorDelete),
                (KeyModifiers::NONE, KeyCode::Enter) => Some(Command::EditorNewline),
                (KeyModifiers::NONE, KeyCode::Tab) => Some(Command::EditorTab),
                (KeyModifiers::NONE, KeyCode::Char(c)) => Some(Command::EditorInsertChar(c)),
                (KeyModifiers::SHIFT, KeyCode::Char(c)) => Some(Command::EditorInsertChar(c)),
                _ => None,
            };
            if let Some(cmd) = editor_cmd {
                self.execute(cmd);
                return;
            }
            // Fall through to global keymap for Ctrl+Q, Tab, etc.
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

        // Terminal-focus key interception: only specific global bindings are intercepted,
        // everything else is forwarded to the PTY as raw bytes.
        // CloseOverlay (Esc) is NOT intercepted here — shell needs Esc for vi mode,
        // completion cancel, etc. Also prevents SGR mouse sequence splitting: if crossterm
        // splits a mouse escape, the leading Esc would be consumed while `[<65;...M` would
        // leak into the PTY as visible text.
        if matches!(self.focus, FocusTarget::Terminal(_)) && !self.show_help {
            if let Some(cmd) = self.keymap.resolve(&key) {
                if cmd == Command::CloseOverlay {
                    // Esc with no overlay open — forward to PTY.
                    self.write_terminal_input(&key);
                } else {
                    self.execute(cmd);
                }
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
            // IMPACT ANALYSIS — TreeToggle with file open
            // Parents: KeyEvent(Enter) when tree focused -> Command::TreeToggle
            // Children: BufferManager::open_file() -> UI render, FocusTarget::Editor
            // Siblings: Tree selection (unchanged), terminal (unaffected)
            Command::TreeToggle => {
                if let Some(ref tree) = self.file_tree {
                    if let Some(node) = tree.selected_node() {
                        if matches!(node.kind, NodeKind::File { .. }) {
                            let path = node.path.clone();
                            self.execute(Command::OpenFile(path));
                            return;
                        }
                    }
                }
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
            Command::NewTerminalTab => self.new_terminal_tab(),
            Command::CloseTerminalTab => self.close_terminal_tab(),
            Command::ActivateTerminalTab(idx) => self.activate_terminal_tab(idx),
            Command::TerminalScrollPageUp => {
                self.terminal_scroll(alacritty_terminal::grid::Scroll::PageUp);
            }
            Command::TerminalScrollPageDown => {
                self.terminal_scroll(alacritty_terminal::grid::Scroll::PageDown);
            }
            Command::TerminalScrollTop => {
                self.terminal_scroll(alacritty_terminal::grid::Scroll::Top);
            }
            Command::TerminalScrollBottom => {
                self.terminal_scroll(alacritty_terminal::grid::Scroll::Bottom);
            }
            // IMPACT ANALYSIS — OpenFile
            // Parents: TreeToggle on file node, future: command palette, fuzzy finder
            // Children: BufferManager adds buffer, focus switches to Editor
            // Siblings: Tree state (unchanged), terminal (unchanged)
            Command::OpenFile(path) => match self.buffer_manager.open_file(&path) {
                Ok(()) => self.focus = FocusTarget::Editor,
                Err(e) => log::warn!("Failed to open file: {e}"),
            },
            // IMPACT ANALYSIS — Editor cursor movement commands
            // Parents: KeyEvent with editor focus -> editor-focus interception -> these commands
            // Children: EditorBuffer cursor/scroll state
            // Siblings: Tree/terminal unaffected; UI reads cursor/scroll to render
            Command::EditorUp
            | Command::EditorDown
            | Command::EditorLeft
            | Command::EditorRight
            | Command::EditorHome
            | Command::EditorEnd
            | Command::EditorFileStart
            | Command::EditorFileEnd
            | Command::EditorPageUp
            | Command::EditorPageDown
            | Command::EditorWordRight
            | Command::EditorWordLeft => {
                let (h, w) = self.editor_viewport();
                if let Some(buf) = self.buffer_manager.active_buffer_mut() {
                    buf.clear_selection();
                    match cmd {
                        Command::EditorUp => buf.move_up(),
                        Command::EditorDown => buf.move_down(),
                        Command::EditorLeft => buf.move_left(),
                        Command::EditorRight => buf.move_right(),
                        Command::EditorHome => buf.move_home(),
                        Command::EditorEnd => buf.move_end(),
                        Command::EditorFileStart => buf.move_file_start(),
                        Command::EditorFileEnd => buf.move_file_end(),
                        Command::EditorPageUp => buf.move_page_up(h),
                        Command::EditorPageDown => buf.move_page_down(h),
                        Command::EditorWordRight => buf.move_word_right(),
                        Command::EditorWordLeft => buf.move_word_left(),
                        _ => unreachable!(),
                    }
                    buf.ensure_cursor_visible(h, w);
                }
            }
            // IMPACT ANALYSIS — Editor edit commands
            // Parents: KeyEvent with editor focus -> editor-focus interception -> these commands
            // Children: EditorBuffer content/cursor/modified state, last_edit_time for autosave
            // Siblings: UI reads modified flag for [+] indicator, status bar line count
            Command::EditorInsertChar(ch) => {
                let (h, w) = self.editor_viewport();
                if let Some(buf) = self.buffer_manager.active_buffer_mut() {
                    buf.insert_char(ch);
                    buf.ensure_cursor_visible(h, w);
                }
                self.last_edit_time = Some(Instant::now());
            }
            Command::EditorBackspace => {
                let (h, w) = self.editor_viewport();
                if let Some(buf) = self.buffer_manager.active_buffer_mut() {
                    buf.delete_char_backward();
                    buf.ensure_cursor_visible(h, w);
                }
                self.last_edit_time = Some(Instant::now());
            }
            Command::EditorDelete => {
                let (h, w) = self.editor_viewport();
                if let Some(buf) = self.buffer_manager.active_buffer_mut() {
                    buf.delete_char_forward();
                    buf.ensure_cursor_visible(h, w);
                }
                self.last_edit_time = Some(Instant::now());
            }
            Command::EditorNewline => {
                let (h, w) = self.editor_viewport();
                if let Some(buf) = self.buffer_manager.active_buffer_mut() {
                    buf.insert_newline();
                    buf.ensure_cursor_visible(h, w);
                }
                self.last_edit_time = Some(Instant::now());
            }
            Command::EditorTab => {
                let (h, w) = self.editor_viewport();
                if let Some(buf) = self.buffer_manager.active_buffer_mut() {
                    buf.insert_tab();
                    buf.ensure_cursor_visible(h, w);
                }
                self.last_edit_time = Some(Instant::now());
            }
            Command::EditorSave => {
                if let Some(buf) = self.buffer_manager.active_buffer_mut() {
                    if let Err(e) = buf.save_to_file() {
                        log::warn!("Save failed: {e}");
                    }
                }
                self.last_edit_time = None;
            }
            // IMPACT ANALYSIS — EditorUndo / EditorRedo
            // Parents: KeyEvent → Ctrl+Z / Ctrl+Y / Ctrl+Shift+Z → keymap → these commands
            // Children: EditorBuffer::undo()/redo() reverses/replays edits on rope, restores cursor
            // Siblings: Does NOT set last_edit_time (no autosave trigger for undo/redo)
            Command::EditorUndo => {
                let (h, w) = self.editor_viewport();
                if let Some(buf) = self.buffer_manager.active_buffer_mut() {
                    buf.undo();
                    buf.ensure_cursor_visible(h, w);
                }
            }
            Command::EditorRedo => {
                let (h, w) = self.editor_viewport();
                if let Some(buf) = self.buffer_manager.active_buffer_mut() {
                    buf.redo();
                    buf.ensure_cursor_visible(h, w);
                }
            }
            // IMPACT ANALYSIS — Selection commands
            // Parents: KeyEvent → Shift+Arrow/Ctrl+Shift+Arrow → these commands
            // Children: EditorBuffer selection state, cursor position
            // Siblings: Plain movement commands (clear selection), UI renders selection highlight
            Command::EditorSelectUp
            | Command::EditorSelectDown
            | Command::EditorSelectLeft
            | Command::EditorSelectRight
            | Command::EditorSelectHome
            | Command::EditorSelectEnd
            | Command::EditorSelectFileStart
            | Command::EditorSelectFileEnd
            | Command::EditorSelectWordLeft
            | Command::EditorSelectWordRight => {
                let (h, w) = self.editor_viewport();
                if let Some(buf) = self.buffer_manager.active_buffer_mut() {
                    match cmd {
                        Command::EditorSelectUp => buf.select_up(),
                        Command::EditorSelectDown => buf.select_down(),
                        Command::EditorSelectLeft => buf.select_left(),
                        Command::EditorSelectRight => buf.select_right(),
                        Command::EditorSelectHome => buf.select_home(),
                        Command::EditorSelectEnd => buf.select_end(),
                        Command::EditorSelectFileStart => buf.select_file_start(),
                        Command::EditorSelectFileEnd => buf.select_file_end(),
                        Command::EditorSelectWordLeft => buf.select_word_left(),
                        Command::EditorSelectWordRight => buf.select_word_right(),
                        _ => unreachable!(),
                    }
                    buf.ensure_cursor_visible(h, w);
                }
            }
            Command::EditorSelectAll => {
                if let Some(buf) = self.buffer_manager.active_buffer_mut() {
                    buf.select_all();
                }
            }
            // IMPACT ANALYSIS — Clipboard commands
            // Parents: KeyEvent → Ctrl+C/X/V → keymap → these commands
            // Children: System clipboard (read/write), buffer content (cut/paste)
            // Siblings: Selection (copy/cut read it, paste may replace it),
            //           last_edit_time (cut/paste trigger autosave timer)
            Command::EditorCopy => {
                let text = self
                    .buffer_manager
                    .active_buffer()
                    .and_then(|buf| buf.selected_text());
                if let Some(ref text) = text {
                    if !text.is_empty() {
                        self.ensure_clipboard();
                        if let Some(ref mut cb) = self.clipboard {
                            if let Err(e) = cb.set_text(text) {
                                log::warn!("Failed to copy to clipboard: {e}");
                            }
                        }
                        let lines = text.lines().count();
                        let chars = text.len();
                        self.set_status_message(format!("Copied {lines} line(s), {chars} char(s)"));
                    }
                }
            }
            Command::EditorCut => {
                let (h, w) = self.editor_viewport();
                let cut_text = self.buffer_manager.active_buffer_mut().and_then(|buf| {
                    let text = buf.delete_selection();
                    buf.ensure_cursor_visible(h, w);
                    text
                });
                if let Some(ref text) = cut_text {
                    if !text.is_empty() {
                        self.ensure_clipboard();
                        if let Some(ref mut cb) = self.clipboard {
                            if let Err(e) = cb.set_text(text) {
                                log::warn!("Failed to copy to clipboard: {e}");
                            }
                        }
                        let lines = text.lines().count();
                        let chars = text.len();
                        self.set_status_message(format!("Cut {lines} line(s), {chars} char(s)"));
                    }
                }
                self.last_edit_time = Some(Instant::now());
            }
            Command::EditorPaste => {
                let (h, w) = self.editor_viewport();
                self.ensure_clipboard();
                let text = self
                    .clipboard
                    .as_mut()
                    .and_then(|cb| cb.get_text().ok())
                    .unwrap_or_default();
                if !text.is_empty() {
                    if let Some(buf) = self.buffer_manager.active_buffer_mut() {
                        buf.insert_text(&text);
                        buf.ensure_cursor_visible(h, w);
                    }
                    self.last_edit_time = Some(Instant::now());
                }
            }
            // IMPACT ANALYSIS — Search commands
            // Parents: KeyEvent → Ctrl+F / search interception layer → these commands
            // Children: SearchState (created/modified), cursor position (match navigation)
            // Siblings: Selection (cleared on search open), editor key interception
            //           (search layer runs before editor layer when active)
            Command::EditorFind => {
                if self.search.is_none() {
                    let mut search = SearchState::new();
                    // Pre-fill from selection if available.
                    if let Some(buf) = self.buffer_manager.active_buffer() {
                        if let Some(text) = buf.selected_text() {
                            // Use only the first line for pre-fill.
                            let first_line = text.lines().next().unwrap_or("").to_string();
                            if !first_line.is_empty() {
                                search.query = first_line;
                                search.update_matches(buf);
                                let row = buf.cursor.row;
                                let col = buf.cursor.col;
                                search.nearest_match_from(row, col);
                            }
                        }
                    }
                    self.search = Some(search);
                }
                // If already open, no-op (focus stays on search bar).
            }
            Command::SearchClose => {
                self.search = None;
            }
            Command::SearchNextMatch => {
                let (h, w) = self.editor_viewport();
                if let Some(ref mut search) = self.search {
                    search.next_match();
                    if let Some(m) = search.current_match().cloned() {
                        if let Some(buf) = self.buffer_manager.active_buffer_mut() {
                            buf.cursor.row = m.row;
                            buf.cursor.col = m.col_start;
                            buf.clear_selection();
                            buf.ensure_cursor_visible(h, w);
                        }
                    }
                }
            }
            Command::SearchPrevMatch => {
                let (h, w) = self.editor_viewport();
                if let Some(ref mut search) = self.search {
                    search.prev_match();
                    if let Some(m) = search.current_match().cloned() {
                        if let Some(buf) = self.buffer_manager.active_buffer_mut() {
                            buf.cursor.row = m.row;
                            buf.cursor.col = m.col_start;
                            buf.clear_selection();
                            buf.ensure_cursor_visible(h, w);
                        }
                    }
                }
            }
            Command::SearchToggleCase => {
                if let Some(ref mut search) = self.search {
                    if let Some(buf) = self.buffer_manager.active_buffer() {
                        search.toggle_case(buf);
                    }
                }
            }
            Command::SearchToggleRegex => {
                if let Some(ref mut search) = self.search {
                    if let Some(buf) = self.buffer_manager.active_buffer() {
                        search.toggle_regex(buf);
                    }
                }
            }
            Command::NextBuffer => {
                self.search = None;
                self.buffer_manager.next_buffer();
            }
            Command::PrevBuffer => {
                self.search = None;
                self.buffer_manager.prev_buffer();
            }
            Command::ActivateBuffer(idx) => {
                self.search = None;
                self.buffer_manager.set_active(idx);
            }
            Command::CloseBuffer => {
                if let Some(buf) = self.buffer_manager.active_buffer() {
                    if buf.modified {
                        self.confirm_close_buffer = true;
                    } else {
                        let idx = self.buffer_manager.active_index();
                        self.buffer_manager.close_buffer(idx);
                        self.search = None;
                    }
                }
            }
            Command::ConfirmCloseBuffer => {
                self.confirm_close_buffer = false;
                let idx = self.buffer_manager.active_index();
                self.buffer_manager.close_buffer(idx);
                self.search = None;
            }
            Command::CancelCloseBuffer => {
                self.confirm_close_buffer = false;
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

                // Tab bar click takes priority — its row overlaps with border tolerance.
                if self.show_terminal {
                    if let Some(tab_idx) = self.tab_bar_hit(col, row, screen_width, main_height) {
                        self.activate_terminal_tab(tab_idx);
                        return;
                    }
                }

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

                // Editor tab bar click: switch to the clicked buffer tab.
                if let Some((tx, ty, tw, _th)) = self.editor_tab_bar_area {
                    if row == ty && col >= tx && col < tx + tw {
                        if let Some(idx) = self.editor_tab_index_at_col(col - tx) {
                            self.execute(Command::ActivateBuffer(idx));
                            self.focus = FocusTarget::Editor;
                            return;
                        }
                    }
                }

                // IMPACT ANALYSIS — Editor mouse text selection (Down/Drag/Up)
                // Parents: MouseEvent from crossterm, routed through handle_mouse_event.
                // Children: EditorBuffer cursor/selection state.
                // Siblings: mouse_drag.border (mutually exclusive, checked first),
                //           editor_inner_area must be kept in sync by main.rs each frame.
                // Risk: editor_selecting flag must be cleared on Up to avoid stale drag state.

                // Check if click is in editor content area — position cursor and start selection.
                if let Some((erow, ecol)) = self.screen_to_editor_pos(col, row) {
                    if let Some(buf) = self.buffer_manager.active_buffer_mut() {
                        buf.clear_selection();
                        buf.cursor.row = erow;
                        buf.cursor.col = ecol;
                        buf.cursor.desired_col = ecol;
                    }
                    self.editor_selecting = true;
                    self.focus = FocusTarget::Editor;
                    return;
                }

                // IMPACT ANALYSIS — Terminal mouse text selection (Down/Drag/Up)
                // Parents: MouseEvent from crossterm, routed through handle_mouse_event.
                // Children: terminal_manager selection state, system clipboard (on drag release).
                // Siblings: mouse_drag.border (panel border resize — mutually exclusive, border check
                //           runs first and returns early), tab_bar_hit (also checked before selection).
                //           terminal_grid_area must be kept in sync by main.rs each frame.
                // Risk: terminal_selecting flag must be cleared on Up to avoid stale drag state.

                // Check if click is in terminal grid area — start selection.
                if let Some(point) = self.screen_to_terminal_point(col, row) {
                    if let Some(ref mut mgr) = self.terminal_manager {
                        mgr.clear_selection_active();
                        mgr.start_selection_active(SelectionType::Simple, point, Direction::Left);
                    }
                    self.terminal_selecting = true;
                    self.terminal_select_start = Some((col, row));
                    self.focus = FocusTarget::Terminal(0);
                    return;
                }

                // No border, tab bar, or terminal grid hit — focus the clicked panel.
                if row < main_height {
                    self.focus = self.panel_at(col, row, screen_width, main_height);
                }
            }
            MouseEventKind::Drag(MouseButton::Left) => {
                // Editor text selection drag.
                if self.editor_selecting {
                    // Clamp mouse to editor content area.
                    let pos = if let Some((ex, ey, ew, eh)) = self.editor_inner_area {
                        let clamped_col = mouse.column.clamp(ex, ex + ew.saturating_sub(1));
                        let clamped_row = mouse.row.clamp(ey, ey + eh.saturating_sub(1));
                        self.screen_to_editor_pos(clamped_col, clamped_row)
                    } else {
                        None
                    };
                    if let Some((erow, ecol)) = pos {
                        if let Some(buf) = self.buffer_manager.active_buffer_mut() {
                            buf.start_or_extend_selection();
                            buf.cursor.row = erow;
                            buf.cursor.col = ecol;
                            buf.cursor.desired_col = ecol;
                        }
                    }
                    return;
                }

                // Terminal text selection drag.
                if self.terminal_selecting {
                    // Clamp coordinates to the terminal grid area.
                    let point = if let Some((gx, gy, gw, gh)) = self.terminal_grid_area {
                        let clamped_col = mouse.column.clamp(gx, gx + gw.saturating_sub(1));
                        let clamped_row = mouse.row.clamp(gy, gy + gh.saturating_sub(1));
                        self.screen_to_terminal_point(clamped_col, clamped_row)
                    } else {
                        None
                    };
                    if let Some(point) = point {
                        if let Some(ref mut mgr) = self.terminal_manager {
                            mgr.update_selection_active(point, Direction::Right);
                        }
                    }
                    return;
                }

                // Panel border drag.
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
                if self.editor_selecting {
                    self.editor_selecting = false;
                    // Clean up empty selection (click without drag).
                    if let Some(buf) = self.buffer_manager.active_buffer_mut() {
                        if buf
                            .selection
                            .as_ref()
                            .is_some_and(|s| s.is_empty(buf.cursor.row, buf.cursor.col))
                        {
                            buf.clear_selection();
                        }
                    }
                }
                if self.terminal_selecting {
                    self.terminal_selecting = false;

                    // Check if this was a click without drag (no movement).
                    let was_click = self
                        .terminal_select_start
                        .is_none_or(|(sx, sy)| sx == mouse.column && sy == mouse.row);
                    self.terminal_select_start = None;

                    if was_click {
                        // Click without drag — clear selection.
                        if let Some(ref mut mgr) = self.terminal_manager {
                            mgr.clear_selection_active();
                        }
                    } else {
                        // Drag completed — copy selection to clipboard.
                        let text = self
                            .terminal_manager
                            .as_ref()
                            .and_then(|mgr| mgr.copy_selection_active());
                        if let Some(ref text) = text {
                            if !text.is_empty() {
                                Self::copy_to_clipboard(text);
                            }
                        }
                    }
                }
                self.mouse_drag.border = None;
            }
            MouseEventKind::ScrollUp => {
                if self.show_terminal
                    && matches!(
                        self.panel_at(mouse.column, mouse.row, screen_width, main_height),
                        FocusTarget::Terminal(_)
                    )
                {
                    self.terminal_scroll(alacritty_terminal::grid::Scroll::Delta(
                        MOUSE_SCROLL_LINES,
                    ));
                }
            }
            MouseEventKind::ScrollDown => {
                if self.show_terminal
                    && matches!(
                        self.panel_at(mouse.column, mouse.row, screen_width, main_height),
                        FocusTarget::Terminal(_)
                    )
                {
                    self.terminal_scroll(alacritty_terminal::grid::Scroll::Delta(
                        -MOUSE_SCROLL_LINES,
                    ));
                }
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

    /// Checks if a click landed on the terminal tab bar and returns the tab index.
    ///
    /// The tab bar is the first row inside the terminal panel border.
    /// Returns `None` if the click is outside the tab bar or if there's no terminal manager.
    fn tab_bar_hit(
        &self,
        col: u16,
        row: u16,
        screen_width: u16,
        main_height: u16,
    ) -> Option<usize> {
        let mgr = self.terminal_manager.as_ref()?;
        if !mgr.has_tabs() {
            return None;
        }

        // Compute terminal panel position.
        let term_x_start = if self.show_tree {
            (u32::from(screen_width) * u32::from(self.tree_width_pct) / 100) as u16
        } else {
            0
        };
        let term_y_start =
            (u32::from(main_height) * u32::from(self.editor_height_pct) / 100) as u16;

        // The tab bar row is at term_y_start + 1 (after the top border).
        let tab_bar_row = term_y_start + 1;
        if row != tab_bar_row {
            return None;
        }

        // The tab bar content starts at term_x_start + 1 (after the left border).
        let tab_bar_x_start = term_x_start + 1;
        if col < tab_bar_x_start {
            return None;
        }

        let x_offset = (col - tab_bar_x_start) as usize;
        mgr.tab_at_x_offset(x_offset)
    }

    /// Lazily initializes the system clipboard.
    ///
    /// On headless/SSH environments where clipboard access fails,
    /// `self.clipboard` remains `None` and clipboard ops silently no-op.
    fn ensure_clipboard(&mut self) {
        if self.clipboard.is_none() {
            match arboard::Clipboard::new() {
                Ok(cb) => self.clipboard = Some(cb),
                Err(e) => log::warn!("Failed to access clipboard: {e}"),
            }
        }
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

    /// Creates a new terminal tab and focuses it.
    ///
    /// No-op if the terminal panel is hidden — the user should toggle the panel first.
    fn new_terminal_tab(&mut self) {
        if !self.show_terminal {
            return;
        }

        let cwd = self
            .project_root
            .clone()
            .unwrap_or_else(|| PathBuf::from("."));

        if let Some(ref mut mgr) = self.terminal_manager {
            match mgr.spawn_tab(self.last_terminal_cols, self.last_terminal_rows, &cwd) {
                Ok(idx) => {
                    mgr.activate_tab(idx);
                    self.focus = FocusTarget::Terminal(idx);
                }
                Err(e) => {
                    log::warn!("Failed to create terminal tab: {e}");
                }
            }
        } else {
            // No manager yet — create one with a first tab.
            let mut mgr = axe_terminal::TerminalManager::new();
            match mgr.spawn_tab(self.last_terminal_cols, self.last_terminal_rows, &cwd) {
                Ok(idx) => {
                    mgr.activate_tab(idx);
                    self.focus = FocusTarget::Terminal(idx);
                    self.terminal_manager = Some(mgr);
                }
                Err(e) => {
                    log::warn!("Failed to create terminal tab: {e}");
                }
            }
        }
    }

    /// Closes the active terminal tab.
    fn close_terminal_tab(&mut self) {
        if let Some(ref mut mgr) = self.terminal_manager {
            let active = mgr.active_index();
            if let Err(e) = mgr.close_tab(active) {
                log::warn!("Failed to close terminal tab: {e}");
                return;
            }
            self.focus = FocusTarget::Terminal(mgr.active_index());
        }
    }

    /// Scrolls the active terminal tab by the given amount.
    fn terminal_scroll(&mut self, scroll: alacritty_terminal::grid::Scroll) {
        if let Some(ref mut mgr) = self.terminal_manager {
            mgr.scroll_active(scroll);
        }
    }

    /// Returns `(height, width)` of the editor content area for viewport calculations.
    fn editor_viewport(&self) -> (usize, usize) {
        self.editor_inner_area
            .map(|(_x, _y, w, h)| (h as usize, w as usize))
            .unwrap_or((20, 80))
    }

    /// Checks if autosave should trigger based on elapsed time since last edit.
    ///
    /// Saves the active buffer if it has been modified and has a file path,
    /// and at least `AUTOSAVE_DELAY` has passed since the last edit.
    pub fn check_autosave(&mut self) {
        if let Some(last_edit) = self.last_edit_time {
            if last_edit.elapsed() >= AUTOSAVE_DELAY {
                if let Some(buf) = self.buffer_manager.active_buffer_mut() {
                    if buf.modified && buf.path().is_some() {
                        if let Err(e) = buf.save_to_file() {
                            log::warn!("Autosave failed: {e}");
                        }
                    }
                }
                self.last_edit_time = None;
            }
        }
    }

    /// Sets a temporary status message that appears in the status bar.
    pub fn set_status_message(&mut self, msg: String) {
        self.status_message = Some((msg, Instant::now()));
    }

    /// Clears the status message if it has expired.
    pub fn expire_status_message(&mut self) {
        if let Some((_, created)) = &self.status_message {
            if created.elapsed() >= STATUS_MESSAGE_DURATION {
                self.status_message = None;
            }
        }
    }

    /// Activates a specific terminal tab by index. No-op if the index is out of range.
    fn activate_terminal_tab(&mut self, idx: usize) {
        if let Some(ref mut mgr) = self.terminal_manager {
            if idx < mgr.tab_count() {
                mgr.activate_tab(idx);
                self.focus = FocusTarget::Terminal(idx);
            }
        }
    }

    // IMPACT ANALYSIS — editor_tab_index_at_col
    // Parents: handle_mouse_event() calls this when a click lands on the editor tab bar row.
    // Children: reads buffer_manager.buffers() for tab names and modified flags.
    // Siblings: render_tab_bar in axe-ui (must use identical tab width calculation).
    // Risk: None — stateless helper, cannot corrupt state.

    /// Determines which editor tab is at the given column offset within the tab bar.
    ///
    /// Walks buffer names to find which tab occupies the column position.
    /// Returns `None` if the column is past all tabs.
    fn editor_tab_index_at_col(&self, col: u16) -> Option<usize> {
        let mut x: u16 = 0;
        let buf_count = self.buffer_manager.buffers().len();
        for (i, buf) in self.buffer_manager.buffers().iter().enumerate() {
            let name = buf.file_name().unwrap_or("untitled");
            // Format: "[N:name]" or "[N:name+]"
            let num_width = (i + 1).ilog10() as u16 + 1;
            let tab_width = if buf.modified {
                // "[" + num + ":" + name + "+" + "]"
                1 + num_width + 1 + name.len() as u16 + 1 + 1
            } else {
                // "[" + num + ":" + name + "]"
                1 + num_width + 1 + name.len() as u16 + 1
            };
            if col >= x && col < x + tab_width {
                return Some(i);
            }
            x += tab_width;
            // Space between tabs.
            if i + 1 < buf_count {
                x += 1;
            }
        }
        None
    }

    // IMPACT ANALYSIS — screen_to_editor_pos
    // Parents: handle_mouse_event() calls this for Down and Drag events in the editor area.
    // Children: None — pure conversion function returning Option<(row, col)> in buffer coordinates.
    // Siblings: editor_inner_area (must be set by main.rs each frame),
    //           buffer scroll_row/scroll_col (used to convert screen to file position).
    // Risk: None — stateless helper, cannot corrupt state.

    /// Converts screen coordinates to editor buffer (row, col) position.
    ///
    /// Returns `None` if the coordinates are outside the editor content area
    /// or if no editor area has been set.
    fn screen_to_editor_pos(&self, screen_col: u16, screen_row: u16) -> Option<(usize, usize)> {
        let (ex, ey, ew, eh) = self.editor_inner_area?;
        if screen_col < ex || screen_col >= ex + ew || screen_row < ey || screen_row >= ey + eh {
            return None;
        }
        let buf = self.buffer_manager.active_buffer()?;
        let rel_row = (screen_row - ey) as usize;
        let rel_col = (screen_col - ex) as usize;
        let file_row = buf.scroll_row + rel_row;
        let file_col = buf.scroll_col + rel_col;
        // Clamp to actual content bounds.
        let max_row = buf.line_count().saturating_sub(1);
        let row = file_row.min(max_row);
        let col = file_col.min(buf.line_length(row));
        Some((row, col))
    }

    // IMPACT ANALYSIS — screen_to_terminal_point
    // Parents: handle_mouse_event() calls this for Down and Drag events in the terminal grid.
    // Children: None — pure conversion function returning Option<Point>.
    // Siblings: terminal_grid_area (must be set by main.rs each frame),
    //           terminal_manager.active_display_offset() (must reflect current scroll state).
    // Risk: None — stateless helper, cannot corrupt state.

    /// Converts screen coordinates to a terminal grid Point.
    ///
    /// Returns `None` if the coordinates are outside the terminal grid area
    /// or if no grid area has been set.
    fn screen_to_terminal_point(&self, col: u16, row: u16) -> Option<Point> {
        let (gx, gy, gw, gh) = self.terminal_grid_area?;
        if col < gx || col >= gx + gw || row < gy || row >= gy + gh {
            return None;
        }
        let grid_col = (col - gx) as usize;
        let grid_row = (row - gy) as i32;
        let display_offset = self
            .terminal_manager
            .as_ref()
            .map(|mgr| mgr.active_display_offset())
            .unwrap_or(0) as i32;
        Some(Point::new(
            Line(grid_row - display_offset),
            Column(grid_col),
        ))
    }

    // IMPACT ANALYSIS — copy_to_clipboard
    // Parents: handle_mouse_event() Up handler calls this after drag selection completes.
    // Children: System clipboard (external side effect).
    // Siblings: None — standalone utility, no shared state.
    // Risk: Clipboard access may fail on headless systems or Wayland without focus. Errors are logged.

    /// Copies the given text to the system clipboard.
    ///
    /// Logs a warning if clipboard access fails.
    fn copy_to_clipboard(text: &str) {
        match arboard::Clipboard::new() {
            Ok(mut clipboard) => {
                if let Err(e) = clipboard.set_text(text) {
                    log::warn!("Failed to copy to clipboard: {e}");
                }
            }
            Err(e) => {
                log::warn!("Failed to access clipboard: {e}");
            }
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
    ///
    /// When showing the panel and there are no tabs, automatically spawns one.
    /// When hiding, moves focus to Editor if terminal was focused.
    fn toggle_terminal(&mut self) {
        self.show_terminal = !self.show_terminal;
        if self.show_terminal {
            let has_tabs = self
                .terminal_manager
                .as_ref()
                .is_some_and(|mgr| mgr.has_tabs());
            if !has_tabs {
                self.new_terminal_tab();
            }
        } else if matches!(self.focus, FocusTarget::Terminal(_)) {
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
    fn handle_key_alt_z_toggles_zoom() {
        let mut app = AppState::new();
        app.focus = FocusTarget::Editor;
        app.handle_key_event(KeyEvent::new(KeyCode::Char('z'), KeyModifiers::ALT));
        assert_eq!(app.zoomed_panel, Some(FocusTarget::Editor));
        app.handle_key_event(KeyEvent::new(KeyCode::Char('z'), KeyModifiers::ALT));
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

    #[test]
    fn terminal_focused_esc_forwarded_to_pty_not_close_overlay() {
        let mut app = AppState::new();
        app.focus = FocusTarget::Terminal(0);
        // Esc should NOT trigger CloseOverlay when terminal is focused without overlay.
        // It should be forwarded to PTY (for shell vi-mode, cancel completion, etc.).
        app.handle_key_event(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
        assert_eq!(app.focus, FocusTarget::Terminal(0));
        assert!(!app.should_quit);
        // show_help was already false, stays false — verifying no side effect.
        assert!(!app.show_help);
    }

    // --- Mouse scroll tests ---

    #[test]
    fn mouse_scroll_up_over_terminal_does_not_panic() {
        let mut app = AppState::new();
        app.show_terminal = true;
        // Scroll over the terminal area (bottom-right area with default layout).
        let evt = mouse_event(MouseEventKind::ScrollUp, 60, 25);
        app.handle_mouse_event(evt, 100, 30);
        // No terminal_manager — should be a no-op, no panic.
    }

    #[test]
    fn mouse_scroll_down_over_terminal_does_not_panic() {
        let mut app = AppState::new();
        app.show_terminal = true;
        let evt = mouse_event(MouseEventKind::ScrollDown, 60, 25);
        app.handle_mouse_event(evt, 100, 30);
    }

    #[test]
    fn mouse_scroll_over_editor_does_not_scroll_terminal() {
        let mut app = AppState::new();
        app.show_terminal = true;
        // Scroll over the editor area (top-right with default layout).
        let evt = mouse_event(MouseEventKind::ScrollUp, 60, 5);
        app.handle_mouse_event(evt, 100, 30);
        // Should not panic, terminal not scrolled.
    }

    #[test]
    fn mouse_scroll_ignored_when_terminal_hidden() {
        let mut app = AppState::new();
        app.show_terminal = false;
        let evt = mouse_event(MouseEventKind::ScrollUp, 60, 25);
        app.handle_mouse_event(evt, 100, 30);
        // No-op, no panic.
    }

    // --- Terminal selection tests ---

    #[test]
    fn terminal_grid_area_initially_none() {
        let app = AppState::new();
        assert_eq!(app.terminal_grid_area, None);
    }

    #[test]
    fn screen_to_terminal_point_none_without_grid_area() {
        let app = AppState::new();
        assert!(app.screen_to_terminal_point(10, 10).is_none());
    }

    #[test]
    fn screen_to_terminal_point_converts_correctly() {
        let mut app = AppState::new();
        app.terminal_grid_area = Some((20, 15, 60, 10)); // grid starts at (20,15), 60x10

        // Point inside the grid.
        let point = app.screen_to_terminal_point(25, 17);
        assert!(point.is_some());
        let p = point.unwrap();
        assert_eq!(p.column, Column(5)); // 25 - 20
        assert_eq!(p.line, Line(2)); // 17 - 15, no display_offset
    }

    #[test]
    fn screen_to_terminal_point_none_outside_grid() {
        let mut app = AppState::new();
        app.terminal_grid_area = Some((20, 15, 60, 10));

        // Left of grid.
        assert!(app.screen_to_terminal_point(19, 17).is_none());
        // Above grid.
        assert!(app.screen_to_terminal_point(25, 14).is_none());
        // Right of grid.
        assert!(app.screen_to_terminal_point(80, 17).is_none());
        // Below grid.
        assert!(app.screen_to_terminal_point(25, 25).is_none());
    }

    #[test]
    fn terminal_selecting_default_false() {
        let app = AppState::new();
        assert!(!app.terminal_selecting);
        assert!(app.terminal_select_start.is_none());
    }

    #[test]
    fn mouse_down_in_terminal_grid_starts_selection() {
        let mut app = AppState::new();
        app.show_terminal = true;
        app.terminal_grid_area = Some((20, 15, 60, 10));

        // Set up terminal manager with a tab.
        let mut mgr = axe_terminal::TerminalManager::new();
        mgr.spawn_default_tab(60, 10, &std::env::current_dir().unwrap())
            .unwrap();
        app.terminal_manager = Some(mgr);

        // Click inside terminal grid.
        let evt = mouse_event(MouseEventKind::Down(MouseButton::Left), 25, 17);
        app.handle_mouse_event(evt, 100, 30);

        assert!(app.terminal_selecting, "Selection drag should be active");
        assert_eq!(app.terminal_select_start, Some((25, 17)));
        assert!(
            app.terminal_manager
                .as_ref()
                .unwrap()
                .active_tab()
                .unwrap()
                .has_selection(),
            "Terminal should have an active selection"
        );
    }

    #[test]
    fn mouse_click_without_drag_clears_selection() {
        let mut app = AppState::new();
        app.show_terminal = true;
        app.terminal_grid_area = Some((20, 15, 60, 10));

        let mut mgr = axe_terminal::TerminalManager::new();
        mgr.spawn_default_tab(60, 10, &std::env::current_dir().unwrap())
            .unwrap();
        app.terminal_manager = Some(mgr);

        // Mouse down.
        let down = mouse_event(MouseEventKind::Down(MouseButton::Left), 25, 17);
        app.handle_mouse_event(down, 100, 30);
        assert!(app.terminal_selecting);

        // Mouse up at same position (click, no drag).
        let up = mouse_event(MouseEventKind::Up(MouseButton::Left), 25, 17);
        app.handle_mouse_event(up, 100, 30);

        assert!(!app.terminal_selecting, "Selection drag should end");
        assert!(
            !app.terminal_manager
                .as_ref()
                .unwrap()
                .active_tab()
                .unwrap()
                .has_selection(),
            "Selection should be cleared on click without drag"
        );
    }

    // --- BufferManager integration tests ---

    #[test]
    fn new_app_has_empty_buffer_manager() {
        let app = AppState::new();
        assert_eq!(app.buffer_manager.buffer_count(), 0);
    }

    #[test]
    fn execute_open_file_adds_buffer() {
        let mut tmp = tempfile::NamedTempFile::new().unwrap();
        std::io::Write::write_all(&mut tmp, b"hello\n").unwrap();
        std::io::Write::flush(&mut tmp).unwrap();

        let mut app = AppState::new();
        app.execute(Command::OpenFile(tmp.path().to_path_buf()));

        assert!(app.buffer_manager.active_buffer().is_some());
        assert_eq!(app.buffer_manager.buffer_count(), 1);
    }

    #[test]
    fn execute_open_file_switches_focus() {
        let mut tmp = tempfile::NamedTempFile::new().unwrap();
        std::io::Write::write_all(&mut tmp, b"hello\n").unwrap();
        std::io::Write::flush(&mut tmp).unwrap();

        let mut app = AppState::new();
        assert_eq!(app.focus, FocusTarget::Tree);
        app.execute(Command::OpenFile(tmp.path().to_path_buf()));
        assert_eq!(app.focus, FocusTarget::Editor);
    }

    #[test]
    fn tree_toggle_on_file_opens_it() {
        let tmp = tempfile::TempDir::new().unwrap();
        std::fs::write(tmp.path().join("test.rs"), "fn main() {}").unwrap();

        let mut app = AppState::new_with_root(tmp.path().to_path_buf());
        assert!(app.file_tree.is_some());

        // Move down to the file (first child after root dir).
        app.execute(Command::TreeDown);

        // Verify we selected the file.
        let node = app.file_tree.as_ref().unwrap().selected_node().unwrap();
        assert!(
            matches!(node.kind, NodeKind::File { .. }),
            "expected file node, got {:?}",
            node.kind
        );

        // TreeToggle on a file should open it.
        app.execute(Command::TreeToggle);
        assert_eq!(app.focus, FocusTarget::Editor);
        assert_eq!(app.buffer_manager.buffer_count(), 1);
    }

    // --- Editor cursor movement tests ---

    fn app_with_editor_buffer(content: &str) -> AppState {
        let mut tmp = tempfile::NamedTempFile::new().unwrap();
        std::io::Write::write_all(&mut tmp, content.as_bytes()).unwrap();
        std::io::Write::flush(&mut tmp).unwrap();

        let mut app = AppState::new();
        app.buffer_manager.open_file(tmp.path()).expect("open file");
        app.focus = FocusTarget::Editor;
        // Leak the tempfile so the path remains valid for the test.
        let _ = tmp.into_temp_path();
        app
    }

    #[test]
    fn editor_up_moves_cursor() {
        let mut app = app_with_editor_buffer("line1\nline2\nline3");
        app.execute(Command::EditorDown);
        assert_eq!(app.buffer_manager.active_buffer().unwrap().cursor.row, 1);
        app.execute(Command::EditorUp);
        assert_eq!(app.buffer_manager.active_buffer().unwrap().cursor.row, 0);
    }

    #[test]
    fn editor_arrow_keys_intercepted_when_editor_focused() {
        let mut app = app_with_editor_buffer("hello\nworld");
        app.handle_key_event(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
        assert_eq!(app.buffer_manager.active_buffer().unwrap().cursor.row, 1);
        app.handle_key_event(KeyEvent::new(KeyCode::Right, KeyModifiers::NONE));
        assert_eq!(app.buffer_manager.active_buffer().unwrap().cursor.col, 1);
    }

    #[test]
    fn editor_home_end_work() {
        let mut app = app_with_editor_buffer("hello world");
        app.execute(Command::EditorEnd);
        assert_eq!(app.buffer_manager.active_buffer().unwrap().cursor.col, 11);
        app.execute(Command::EditorHome);
        assert_eq!(app.buffer_manager.active_buffer().unwrap().cursor.col, 0);
    }

    #[test]
    fn editor_page_down_uses_viewport() {
        let content = (0..50)
            .map(|i| format!("line{i}"))
            .collect::<Vec<_>>()
            .join("\n");
        let mut app = app_with_editor_buffer(&content);
        app.editor_inner_area = Some((0, 0, 80, 10));
        app.execute(Command::EditorPageDown);
        assert_eq!(app.buffer_manager.active_buffer().unwrap().cursor.row, 10);
    }

    #[test]
    fn editor_word_movement_works() {
        let mut app = app_with_editor_buffer("hello world foo");
        app.handle_key_event(KeyEvent::new(KeyCode::Right, KeyModifiers::CONTROL));
        assert_eq!(app.buffer_manager.active_buffer().unwrap().cursor.col, 6);
        app.handle_key_event(KeyEvent::new(KeyCode::Left, KeyModifiers::CONTROL));
        assert_eq!(app.buffer_manager.active_buffer().unwrap().cursor.col, 0);
    }

    // --- Editor edit command tests ---

    #[test]
    fn editor_insert_char_modifies_buffer() {
        let mut app = app_with_editor_buffer("hello");
        app.execute(Command::EditorInsertChar('X'));
        let buf = app.buffer_manager.active_buffer().unwrap();
        assert_eq!(buf.line_at(0).unwrap().to_string(), "Xhello");
        assert!(buf.modified);
        assert!(app.last_edit_time.is_some());
    }

    #[test]
    fn editor_backspace_deletes_char() {
        let mut app = app_with_editor_buffer("hello");
        // Move cursor to col 3
        app.buffer_manager.active_buffer_mut().unwrap().cursor.col = 3;
        app.execute(Command::EditorBackspace);
        let buf = app.buffer_manager.active_buffer().unwrap();
        assert_eq!(buf.line_at(0).unwrap().to_string(), "helo");
        assert_eq!(buf.cursor.col, 2);
    }

    #[test]
    fn editor_enter_splits_line() {
        let mut app = app_with_editor_buffer("hello");
        app.buffer_manager.active_buffer_mut().unwrap().cursor.col = 3;
        app.execute(Command::EditorNewline);
        let buf = app.buffer_manager.active_buffer().unwrap();
        assert_eq!(buf.cursor.row, 1);
        assert_eq!(buf.line_at(0).unwrap().to_string(), "hel\n");
    }

    #[test]
    fn editor_save_clears_modified() {
        let mut tmp = tempfile::NamedTempFile::new().unwrap();
        std::io::Write::write_all(&mut tmp, b"data").unwrap();
        std::io::Write::flush(&mut tmp).unwrap();

        let mut app = AppState::new();
        app.buffer_manager.open_file(tmp.path()).expect("open file");
        app.focus = FocusTarget::Editor;

        app.execute(Command::EditorInsertChar('x'));
        assert!(app.buffer_manager.active_buffer().unwrap().modified);
        assert!(app.last_edit_time.is_some());

        app.execute(Command::EditorSave);
        assert!(!app.buffer_manager.active_buffer().unwrap().modified);
        assert!(app.last_edit_time.is_none());
    }

    #[test]
    fn autosave_triggers_after_delay() {
        let mut tmp = tempfile::NamedTempFile::new().unwrap();
        std::io::Write::write_all(&mut tmp, b"data").unwrap();
        std::io::Write::flush(&mut tmp).unwrap();

        let mut app = AppState::new();
        app.buffer_manager.open_file(tmp.path()).expect("open file");
        app.focus = FocusTarget::Editor;

        app.execute(Command::EditorInsertChar('z'));
        assert!(app.buffer_manager.active_buffer().unwrap().modified);

        // Simulate time passing by backdating last_edit_time.
        app.last_edit_time = Some(Instant::now() - Duration::from_secs(3));
        app.check_autosave();

        assert!(!app.buffer_manager.active_buffer().unwrap().modified);
        assert!(app.last_edit_time.is_none());
    }

    #[test]
    fn printable_chars_intercepted_when_editor_focused() {
        let mut app = app_with_editor_buffer("hello");
        // Type 'a' — should be intercepted as EditorInsertChar, not fall through.
        app.handle_key_event(KeyEvent::new(KeyCode::Char('a'), KeyModifiers::NONE));
        let buf = app.buffer_manager.active_buffer().unwrap();
        assert_eq!(buf.line_at(0).unwrap().to_string(), "ahello");
    }

    #[test]
    fn editor_undo_reverses_insert() {
        let mut app = app_with_editor_buffer("hello");
        app.execute(Command::EditorInsertChar('X'));
        assert_eq!(
            app.buffer_manager
                .active_buffer()
                .unwrap()
                .line_at(0)
                .unwrap()
                .to_string(),
            "Xhello"
        );
        app.execute(Command::EditorUndo);
        assert_eq!(
            app.buffer_manager
                .active_buffer()
                .unwrap()
                .line_at(0)
                .unwrap()
                .to_string(),
            "hello"
        );
    }

    #[test]
    fn editor_redo_restores_insert() {
        let mut app = app_with_editor_buffer("hello");
        app.execute(Command::EditorInsertChar('X'));
        app.execute(Command::EditorUndo);
        app.execute(Command::EditorRedo);
        assert_eq!(
            app.buffer_manager
                .active_buffer()
                .unwrap()
                .line_at(0)
                .unwrap()
                .to_string(),
            "Xhello"
        );
    }

    #[test]
    fn editor_undo_does_not_set_last_edit_time() {
        let mut app = app_with_editor_buffer("hello");
        app.execute(Command::EditorInsertChar('X'));
        app.last_edit_time = None;
        app.execute(Command::EditorUndo);
        assert!(app.last_edit_time.is_none());
    }

    // --- Editor mouse selection tests ---

    #[test]
    fn editor_mouse_click_positions_cursor() {
        let mut app = app_with_editor_buffer("hello\nworld\nfoo");
        // Set editor area at screen position (5, 2) with 40x10
        app.editor_inner_area = Some((5, 2, 40, 10));

        // Click at screen (8, 3) => relative (3, 1) => buffer row=1, col=3
        let mouse = MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: 8,
            row: 3,
            modifiers: KeyModifiers::NONE,
        };
        app.handle_mouse_event(mouse, 80, 24);

        let buf = app.buffer_manager.active_buffer().unwrap();
        assert_eq!(buf.cursor.row, 1);
        assert_eq!(buf.cursor.col, 3);
        assert!(buf.selection.is_none());
        assert_eq!(app.focus, FocusTarget::Editor);
    }

    #[test]
    fn editor_mouse_drag_creates_selection() {
        let mut app = app_with_editor_buffer("hello\nworld\nfoo");
        app.editor_inner_area = Some((5, 2, 40, 10));

        // Mouse down at (5, 2) => buffer (0, 0)
        app.handle_mouse_event(
            MouseEvent {
                kind: MouseEventKind::Down(MouseButton::Left),
                column: 5,
                row: 2,
                modifiers: KeyModifiers::NONE,
            },
            80,
            24,
        );

        // Drag to (10, 2) => buffer (0, 5)
        app.handle_mouse_event(
            MouseEvent {
                kind: MouseEventKind::Drag(MouseButton::Left),
                column: 10,
                row: 2,
                modifiers: KeyModifiers::NONE,
            },
            80,
            24,
        );

        let buf = app.buffer_manager.active_buffer().unwrap();
        assert!(buf.selection.is_some());
        let sel = buf.selection.as_ref().unwrap();
        assert_eq!(sel.anchor_row, 0);
        assert_eq!(sel.anchor_col, 0);
        assert_eq!(buf.cursor.row, 0);
        assert_eq!(buf.cursor.col, 5);
    }

    #[test]
    fn editor_mouse_click_without_drag_clears_selection_on_up() {
        let mut app = app_with_editor_buffer("hello");
        app.editor_inner_area = Some((5, 2, 40, 10));

        // Click down
        app.handle_mouse_event(
            MouseEvent {
                kind: MouseEventKind::Down(MouseButton::Left),
                column: 7,
                row: 2,
                modifiers: KeyModifiers::NONE,
            },
            80,
            24,
        );

        // Release without drag — selection should be cleared
        app.handle_mouse_event(
            MouseEvent {
                kind: MouseEventKind::Up(MouseButton::Left),
                column: 7,
                row: 2,
                modifiers: KeyModifiers::NONE,
            },
            80,
            24,
        );

        let buf = app.buffer_manager.active_buffer().unwrap();
        assert!(buf.selection.is_none());
    }

    #[test]
    fn editor_mouse_click_clamps_col_to_line_length() {
        let mut app = app_with_editor_buffer("hi");
        app.editor_inner_area = Some((5, 2, 40, 10));

        // Click far past end of "hi" (col 2) => should clamp to col 2
        app.handle_mouse_event(
            MouseEvent {
                kind: MouseEventKind::Down(MouseButton::Left),
                column: 30,
                row: 2,
                modifiers: KeyModifiers::NONE,
            },
            80,
            24,
        );

        let buf = app.buffer_manager.active_buffer().unwrap();
        assert_eq!(buf.cursor.col, 2);
    }

    // --- Status message tests ---

    #[test]
    fn copy_sets_status_message() {
        let mut app = app_with_editor_buffer("hello world");
        app.execute(Command::EditorSelectAll);
        app.execute(Command::EditorCopy);
        assert!(app.status_message.is_some());
        let (msg, _) = app.status_message.as_ref().unwrap();
        assert!(
            msg.contains("Copied"),
            "expected 'Copied' in message, got: {msg}"
        );
        assert!(
            msg.contains("1 line(s)"),
            "expected '1 line(s)' in message, got: {msg}"
        );
    }

    #[test]
    fn cut_sets_status_message() {
        let mut app = app_with_editor_buffer("hello\nworld");
        app.execute(Command::EditorSelectAll);
        app.execute(Command::EditorCut);
        assert!(app.status_message.is_some());
        let (msg, _) = app.status_message.as_ref().unwrap();
        assert!(msg.contains("Cut"), "expected 'Cut' in message, got: {msg}");
        assert!(
            msg.contains("2 line(s)"),
            "expected '2 line(s)' in message, got: {msg}"
        );
    }

    #[test]
    fn copy_without_selection_no_status_message() {
        let mut app = app_with_editor_buffer("hello");
        app.execute(Command::EditorCopy);
        assert!(app.status_message.is_none());
    }

    #[test]
    fn status_message_expires() {
        let mut app = AppState::new();
        app.set_status_message("test".to_string());
        assert!(app.status_message.is_some());
        // Simulate time passing by replacing the instant.
        app.status_message = Some(("test".to_string(), Instant::now() - Duration::from_secs(5)));
        app.expire_status_message();
        assert!(app.status_message.is_none());
    }

    #[test]
    fn editor_find_opens_search() {
        let mut app = AppState::new();
        assert!(app.search.is_none());
        app.execute(Command::EditorFind);
        assert!(app.search.is_some());
    }

    #[test]
    fn search_close_clears_search() {
        let mut app = AppState::new();
        app.execute(Command::EditorFind);
        assert!(app.search.is_some());
        app.execute(Command::SearchClose);
        assert!(app.search.is_none());
    }

    #[test]
    fn search_input_updates_query() {
        let mut tmp = tempfile::NamedTempFile::new().unwrap();
        std::io::Write::write_all(&mut tmp, b"hello world\n").unwrap();
        std::io::Write::flush(&mut tmp).unwrap();

        let mut app = AppState::new();
        app.execute(Command::OpenFile(tmp.path().to_path_buf()));
        app.focus = FocusTarget::Editor;
        app.execute(Command::EditorFind);

        // Simulate typing "he" via key events.
        app.handle_key_event(KeyEvent::new(KeyCode::Char('h'), KeyModifiers::NONE));
        app.handle_key_event(KeyEvent::new(KeyCode::Char('e'), KeyModifiers::NONE));

        let search = app.search.as_ref().unwrap();
        assert_eq!(search.query, "he");
        assert_eq!(search.matches.len(), 1);
    }

    #[test]
    fn search_next_match_moves_cursor() {
        let mut tmp = tempfile::NamedTempFile::new().unwrap();
        std::io::Write::write_all(&mut tmp, b"aaa\naaa\n").unwrap();
        std::io::Write::flush(&mut tmp).unwrap();

        let mut app = AppState::new();
        app.execute(Command::OpenFile(tmp.path().to_path_buf()));
        app.focus = FocusTarget::Editor;
        app.execute(Command::EditorFind);

        // Type "aaa" to find matches.
        if let Some(ref mut search) = app.search {
            if let Some(buf) = app.buffer_manager.active_buffer() {
                search.input_char('a', buf);
                search.input_char('a', buf);
                search.input_char('a', buf);
            }
        }

        // Navigate to next match.
        app.execute(Command::SearchNextMatch);
        let buf = app.buffer_manager.active_buffer().unwrap();
        let search = app.search.as_ref().unwrap();
        assert_eq!(search.current, 1);
        assert_eq!(buf.cursor.row, 1);
    }

    #[test]
    fn search_prev_match_moves_cursor_back() {
        let mut tmp = tempfile::NamedTempFile::new().unwrap();
        std::io::Write::write_all(&mut tmp, b"aaa\naaa\n").unwrap();
        std::io::Write::flush(&mut tmp).unwrap();

        let mut app = AppState::new();
        app.execute(Command::OpenFile(tmp.path().to_path_buf()));
        app.focus = FocusTarget::Editor;
        app.execute(Command::EditorFind);

        if let Some(ref mut search) = app.search {
            if let Some(buf) = app.buffer_manager.active_buffer() {
                search.input_char('a', buf);
                search.input_char('a', buf);
                search.input_char('a', buf);
            }
        }

        // prev from 0 wraps to last.
        app.execute(Command::SearchPrevMatch);
        let search = app.search.as_ref().unwrap();
        assert_eq!(search.current, 1);
    }

    #[test]
    fn search_wraps_around() {
        let mut tmp = tempfile::NamedTempFile::new().unwrap();
        std::io::Write::write_all(&mut tmp, b"aa\n").unwrap();
        std::io::Write::flush(&mut tmp).unwrap();

        let mut app = AppState::new();
        app.execute(Command::OpenFile(tmp.path().to_path_buf()));
        app.focus = FocusTarget::Editor;
        app.execute(Command::EditorFind);

        if let Some(ref mut search) = app.search {
            if let Some(buf) = app.buffer_manager.active_buffer() {
                search.input_char('a', buf);
            }
        }

        // 2 matches: a at col 0 and a at col 1
        let count = app.search.as_ref().unwrap().matches.len();
        assert_eq!(count, 2);

        // next twice wraps back to 0.
        app.execute(Command::SearchNextMatch);
        app.execute(Command::SearchNextMatch);
        assert_eq!(app.search.as_ref().unwrap().current, 0);
    }

    #[test]
    fn editor_find_prefills_selection() {
        let mut tmp = tempfile::NamedTempFile::new().unwrap();
        std::io::Write::write_all(&mut tmp, b"hello world hello\n").unwrap();
        std::io::Write::flush(&mut tmp).unwrap();

        let mut app = AppState::new();
        app.execute(Command::OpenFile(tmp.path().to_path_buf()));
        app.focus = FocusTarget::Editor;

        // Select "hello" (first 5 chars) via selection commands.
        for _ in 0..5 {
            app.execute(Command::EditorSelectRight);
        }

        app.execute(Command::EditorFind);
        let search = app.search.as_ref().unwrap();
        assert_eq!(search.query, "hello");
        assert_eq!(search.matches.len(), 2);
    }

    #[test]
    fn search_esc_closes_via_key_event() {
        let mut app = AppState::new();
        app.focus = FocusTarget::Editor;
        app.execute(Command::EditorFind);
        assert!(app.search.is_some());

        app.handle_key_event(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
        assert!(app.search.is_none());
    }

    // --- Buffer tab management tests ---

    fn open_two_temp_files(app: &mut AppState) {
        let mut tmp1 = tempfile::NamedTempFile::new().unwrap();
        std::io::Write::write_all(&mut tmp1, b"file1\n").unwrap();
        std::io::Write::flush(&mut tmp1).unwrap();
        let mut tmp2 = tempfile::NamedTempFile::new().unwrap();
        std::io::Write::write_all(&mut tmp2, b"file2\n").unwrap();
        std::io::Write::flush(&mut tmp2).unwrap();

        app.buffer_manager
            .open_file(tmp1.path())
            .expect("open file1");
        app.buffer_manager
            .open_file(tmp2.path())
            .expect("open file2");

        // Leak so paths remain valid.
        let _ = tmp1.into_temp_path();
        let _ = tmp2.into_temp_path();
    }

    #[test]
    fn next_buffer_switches_active() {
        let mut app = AppState::new();
        open_two_temp_files(&mut app);
        // After opening two files, active is 1 (last opened).
        assert_eq!(app.buffer_manager.active_index(), 1);
        app.execute(Command::NextBuffer);
        assert_eq!(app.buffer_manager.active_index(), 0);
    }

    #[test]
    fn prev_buffer_switches_active() {
        let mut app = AppState::new();
        open_two_temp_files(&mut app);
        app.buffer_manager.set_active(0);
        assert_eq!(app.buffer_manager.active_index(), 0);
        app.execute(Command::PrevBuffer);
        assert_eq!(app.buffer_manager.active_index(), 1);
    }

    #[test]
    fn close_buffer_unmodified_removes() {
        let mut tmp = tempfile::NamedTempFile::new().unwrap();
        std::io::Write::write_all(&mut tmp, b"data\n").unwrap();
        std::io::Write::flush(&mut tmp).unwrap();

        let mut app = AppState::new();
        app.buffer_manager.open_file(tmp.path()).expect("open file");
        assert_eq!(app.buffer_manager.buffer_count(), 1);

        app.execute(Command::CloseBuffer);
        assert_eq!(app.buffer_manager.buffer_count(), 0);
    }

    #[test]
    fn close_buffer_modified_shows_confirmation() {
        let mut tmp = tempfile::NamedTempFile::new().unwrap();
        std::io::Write::write_all(&mut tmp, b"data\n").unwrap();
        std::io::Write::flush(&mut tmp).unwrap();

        let mut app = AppState::new();
        app.buffer_manager.open_file(tmp.path()).expect("open file");
        app.buffer_manager.active_buffer_mut().unwrap().modified = true;

        app.execute(Command::CloseBuffer);
        assert!(app.confirm_close_buffer);
        assert_eq!(app.buffer_manager.buffer_count(), 1);
    }

    #[test]
    fn confirm_close_buffer_removes() {
        let mut tmp = tempfile::NamedTempFile::new().unwrap();
        std::io::Write::write_all(&mut tmp, b"data\n").unwrap();
        std::io::Write::flush(&mut tmp).unwrap();

        let mut app = AppState::new();
        app.buffer_manager.open_file(tmp.path()).expect("open file");
        app.buffer_manager.active_buffer_mut().unwrap().modified = true;

        app.execute(Command::CloseBuffer);
        assert!(app.confirm_close_buffer);

        app.execute(Command::ConfirmCloseBuffer);
        assert!(!app.confirm_close_buffer);
        assert_eq!(app.buffer_manager.buffer_count(), 0);
    }

    #[test]
    fn cancel_close_buffer_keeps_buffer() {
        let mut tmp = tempfile::NamedTempFile::new().unwrap();
        std::io::Write::write_all(&mut tmp, b"data\n").unwrap();
        std::io::Write::flush(&mut tmp).unwrap();

        let mut app = AppState::new();
        app.buffer_manager.open_file(tmp.path()).expect("open file");
        app.buffer_manager.active_buffer_mut().unwrap().modified = true;

        app.execute(Command::CloseBuffer);
        assert!(app.confirm_close_buffer);

        app.execute(Command::CancelCloseBuffer);
        assert!(!app.confirm_close_buffer);
        assert_eq!(app.buffer_manager.buffer_count(), 1);
    }

    #[test]
    fn activate_buffer_switches() {
        let mut app = AppState::new();
        open_two_temp_files(&mut app);
        assert_eq!(app.buffer_manager.active_index(), 1);

        app.execute(Command::ActivateBuffer(0));
        assert_eq!(app.buffer_manager.active_index(), 0);
    }

    #[test]
    fn switching_buffer_clears_search() {
        let mut app = AppState::new();
        open_two_temp_files(&mut app);
        app.focus = FocusTarget::Editor;

        app.execute(Command::EditorFind);
        assert!(app.search.is_some());

        app.execute(Command::NextBuffer);
        assert!(app.search.is_none());
    }

    #[test]
    fn close_buffer_confirmation_intercepts_keys() {
        let mut tmp = tempfile::NamedTempFile::new().unwrap();
        std::io::Write::write_all(&mut tmp, b"data\n").unwrap();
        std::io::Write::flush(&mut tmp).unwrap();

        let mut app = AppState::new();
        app.buffer_manager.open_file(tmp.path()).expect("open file");
        app.buffer_manager.active_buffer_mut().unwrap().modified = true;

        app.execute(Command::CloseBuffer);
        assert!(app.confirm_close_buffer);

        // Pressing 'n' should cancel the confirmation and keep the buffer.
        app.handle_key_event(KeyEvent::new(KeyCode::Char('n'), KeyModifiers::NONE));
        assert!(!app.confirm_close_buffer);
        assert_eq!(app.buffer_manager.buffer_count(), 1);
    }

    // --- Editor tab bar mouse click tests ---

    #[test]
    fn editor_tab_index_at_col_finds_correct_tab() {
        let mut app = AppState::new();
        open_two_temp_files(&mut app);
        // Two buffers are open. Format: "[1:name]" = 1 + 1 + 1 + name.len() + 1
        // First tab starts at col 0.
        let idx0 = app.editor_tab_index_at_col(0);
        assert_eq!(idx0, Some(0), "col 0 should be inside first tab");

        // First tab width = "[1:" + name + "]" = 3 + name.len() + 1, then 1 space separator.
        let name0 = app.buffer_manager.buffers()[0].file_name().unwrap().len();
        let first_tab_width = 3 + name0 as u16 + 1; // "[1:name]"
        let second_tab_start = first_tab_width + 1; // +1 for space between tabs
        let idx1 = app.editor_tab_index_at_col(second_tab_start);
        assert_eq!(
            idx1,
            Some(1),
            "col at second tab start should be second tab"
        );
    }

    #[test]
    fn editor_tab_index_at_col_returns_none_past_tabs() {
        let mut app = AppState::new();
        open_two_temp_files(&mut app);
        // Very large column past all tabs.
        let idx = app.editor_tab_index_at_col(500);
        assert_eq!(idx, None, "col far past all tabs should return None");
    }

    #[test]
    fn mouse_click_on_editor_tab_switches_buffer() {
        let mut app = AppState::new();
        open_two_temp_files(&mut app);
        // After opening two files, active index is 1 (last opened).
        assert_eq!(app.buffer_manager.active_index(), 1);

        // Set up editor tab bar area at screen row 5, starting at column 2.
        app.editor_tab_bar_area = Some((2, 5, 80, 1));

        // Click on first tab (col 2, row 5) => relative col 0 => first tab.
        let mouse = MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: 2,
            row: 5,
            modifiers: KeyModifiers::NONE,
        };
        app.handle_mouse_event(mouse, 100, 30);

        assert_eq!(
            app.buffer_manager.active_index(),
            0,
            "clicking first tab should activate buffer 0"
        );
        assert_eq!(app.focus, FocusTarget::Editor);
    }
}
