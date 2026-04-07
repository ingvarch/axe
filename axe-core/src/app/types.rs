use std::time::{Duration, Instant};

use crate::command::Command;

/// Maximum time between two clicks to register as a double-click.
pub(super) const DOUBLE_CLICK_THRESHOLD: Duration = Duration::from_millis(400);

/// Maximum distance (in cells) between clicks to still count as "same position".
const CLICK_POSITION_TOLERANCE: usize = 1;

/// Default terminal size used when the actual panel size is not yet known.
pub(super) const DEFAULT_TERMINAL_COLS: u16 = 80;
/// Default terminal rows used when the actual panel size is not yet known.
pub(super) const DEFAULT_TERMINAL_ROWS: u16 = 24;

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

/// Tracks consecutive mouse clicks at approximately the same position
/// for multi-click detection (double-click, triple-click).
#[derive(Debug, Clone, Default)]
pub struct ClickState {
    /// Timestamp of the last mouse-down event.
    last_time: Option<Instant>,
    /// Buffer/grid position of the last click (row, col).
    last_pos: Option<(usize, usize)>,
    /// Number of consecutive clicks (1 = single, 2 = double, 3 = triple).
    pub click_count: u8,
}

impl ClickState {
    /// Registers a click and returns the updated click count.
    ///
    /// Increments if the click is at the same position (within tolerance)
    /// and within the time threshold. Otherwise resets to 1.
    /// Caps at 3 (triple-click).
    pub fn register(&mut self, now: Instant, row: usize, col: usize, threshold: Duration) -> u8 {
        let same_pos = self.last_pos.is_some_and(|(r, c)| {
            r.abs_diff(row) <= CLICK_POSITION_TOLERANCE
                && c.abs_diff(col) <= CLICK_POSITION_TOLERANCE
        });
        let within_threshold = self
            .last_time
            .is_some_and(|t| now.duration_since(t) < threshold);

        if same_pos && within_threshold {
            self.click_count = (self.click_count + 1).min(3);
        } else {
            self.click_count = 1;
        }

        self.last_time = Some(now);
        self.last_pos = Some((row, col));
        self.click_count
    }
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

/// Which button is focused in the confirmation dialog.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ConfirmButton {
    Yes,
    #[default]
    No,
}

/// A reusable confirmation dialog with navigable [Yes] / [No] buttons.
///
/// Default focus is on [No] (safe default). Left/Right arrows move focus,
/// Enter activates, Esc cancels.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfirmDialog {
    /// Title shown in the dialog border.
    pub title: String,
    /// Message lines displayed in the dialog body.
    pub message: Vec<String>,
    /// Currently focused button.
    pub selected: ConfirmButton,
    /// Command dispatched when the user confirms (Yes).
    pub on_confirm: Command,
    /// Command dispatched when the user cancels (No / Esc). None = just dismiss.
    pub on_cancel: Option<Command>,
}

impl ConfirmDialog {
    /// Creates a quit confirmation dialog.
    pub fn quit() -> Self {
        Self {
            title: "Quit".to_string(),
            message: vec!["Are you sure?".to_string()],
            selected: ConfirmButton::default(),
            on_confirm: Command::Quit,
            on_cancel: None,
        }
    }

    /// Creates a close-buffer confirmation dialog showing the file name.
    pub fn close_buffer(file_name: &str) -> Self {
        Self {
            title: "Close Buffer".to_string(),
            message: vec![
                file_name.to_string(),
                String::new(),
                "Unsaved changes will be lost.".to_string(),
            ],
            selected: ConfirmButton::default(),
            on_confirm: Command::ConfirmCloseBuffer,
            on_cancel: Some(Command::CancelCloseBuffer),
        }
    }

    /// Creates a close-terminal confirmation dialog showing the tab title.
    pub fn close_terminal(tab_title: &str) -> Self {
        Self {
            title: "Close Terminal".to_string(),
            message: vec![
                tab_title.to_string(),
                String::new(),
                "Process is still running.".to_string(),
            ],
            selected: ConfirmButton::default(),
            on_confirm: Command::ForceCloseTerminalTab,
            on_cancel: Some(Command::CancelCloseTerminalTab),
        }
    }

    /// Creates a delete-tree-node confirmation dialog showing the node name.
    pub fn delete_tree_node(node_name: &str) -> Self {
        Self {
            title: "Delete".to_string(),
            message: vec![
                node_name.to_string(),
                String::new(),
                "This cannot be undone.".to_string(),
            ],
            selected: ConfirmButton::default(),
            on_confirm: Command::ConfirmTreeDelete,
            on_cancel: Some(Command::CancelTreeDelete),
        }
    }
}

/// Dialog for jumping to a specific line number.
///
/// Accepts digit input only. Displays line count for reference.
/// Input is 1-indexed (user types "42" to go to line 42); `parse_line()`
/// converts to 0-indexed and clamps to file bounds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GoToLineDialog {
    /// The current digit input string.
    pub input: String,
    /// Total number of lines in the active buffer.
    pub max_lines: usize,
}

impl GoToLineDialog {
    /// Creates a new dialog for a buffer with the given line count.
    pub fn new(max_lines: usize) -> Self {
        Self {
            input: String::new(),
            max_lines,
        }
    }

    /// Appends a digit character. Non-digit characters are silently rejected.
    pub fn input_char(&mut self, c: char) {
        if c.is_ascii_digit() {
            self.input.push(c);
        }
    }

    /// Removes the last character from the input.
    pub fn input_backspace(&mut self) {
        self.input.pop();
    }

    /// Parses the input as a 1-indexed line number and returns 0-indexed.
    ///
    /// Returns `None` for empty input or "0".
    /// Clamps to `max_lines - 1` if the input exceeds the file length.
    pub fn parse_line(&self) -> Option<usize> {
        if self.input.is_empty() {
            return None;
        }
        let n: usize = self.input.parse().ok()?;
        if n == 0 {
            return None;
        }
        // Convert 1-indexed to 0-indexed, clamped to file bounds.
        let zero_indexed = n.saturating_sub(1);
        Some(zero_indexed.min(self.max_lines.saturating_sub(1)))
    }
}

#[cfg(test)]
mod go_to_line_tests {
    use super::*;

    #[test]
    fn new_initializes_with_empty_input() {
        let dialog = GoToLineDialog::new(100);
        assert_eq!(dialog.input, "");
        assert_eq!(dialog.max_lines, 100);
    }

    #[test]
    fn input_char_appends_digit() {
        let mut dialog = GoToLineDialog::new(100);
        dialog.input_char('5');
        assert_eq!(dialog.input, "5");
        dialog.input_char('3');
        assert_eq!(dialog.input, "53");
    }

    #[test]
    fn input_char_rejects_non_digit() {
        let mut dialog = GoToLineDialog::new(100);
        dialog.input_char('a');
        assert_eq!(dialog.input, "");
        dialog.input_char('!');
        assert_eq!(dialog.input, "");
        dialog.input_char(' ');
        assert_eq!(dialog.input, "");
    }

    #[test]
    fn input_backspace_removes_last_char() {
        let mut dialog = GoToLineDialog::new(100);
        dialog.input_char('4');
        dialog.input_char('2');
        dialog.input_backspace();
        assert_eq!(dialog.input, "4");
    }

    #[test]
    fn input_backspace_on_empty_is_noop() {
        let mut dialog = GoToLineDialog::new(100);
        dialog.input_backspace();
        assert_eq!(dialog.input, "");
    }

    #[test]
    fn parse_line_returns_zero_indexed() {
        let mut dialog = GoToLineDialog::new(100);
        dialog.input_char('5');
        assert_eq!(dialog.parse_line(), Some(4)); // line 5 -> index 4
    }

    #[test]
    fn parse_line_returns_none_for_empty_input() {
        let dialog = GoToLineDialog::new(100);
        assert_eq!(dialog.parse_line(), None);
    }

    #[test]
    fn parse_line_returns_none_for_zero() {
        let mut dialog = GoToLineDialog::new(100);
        dialog.input_char('0');
        assert_eq!(dialog.parse_line(), None);
    }

    #[test]
    fn parse_line_clamps_to_max_lines() {
        let mut dialog = GoToLineDialog::new(50);
        dialog.input = "999".to_string();
        assert_eq!(dialog.parse_line(), Some(49)); // clamped to last line index
    }

    #[test]
    fn parse_line_line_one_returns_zero() {
        let mut dialog = GoToLineDialog::new(100);
        dialog.input_char('1');
        assert_eq!(dialog.parse_line(), Some(0));
    }

    #[test]
    fn parse_line_max_line_returns_last_index() {
        let mut dialog = GoToLineDialog::new(100);
        dialog.input = "100".to_string();
        assert_eq!(dialog.parse_line(), Some(99));
    }
}

/// Dialog for entering an SSH password.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PasswordDialog {
    /// The host being connected to (for display).
    pub host_display: String,
    /// The current password input.
    pub input: String,
    /// Index of the SSH tab awaiting the password.
    pub tab_index: usize,
}

impl PasswordDialog {
    /// Creates a new password dialog for the given host.
    pub fn new(host_display: String, tab_index: usize) -> Self {
        Self {
            host_display,
            input: String::new(),
            tab_index,
        }
    }

    /// Appends a character to the password input.
    pub fn input_char(&mut self, c: char) {
        self.input.push(c);
    }

    /// Removes the last character from the password input.
    pub fn input_backspace(&mut self) {
        self.input.pop();
    }
}

#[cfg(test)]
mod password_dialog_tests {
    use super::*;

    #[test]
    fn new_initializes_empty_input() {
        let dialog = PasswordDialog::new("user@host".to_string(), 0);
        assert_eq!(dialog.input, "");
        assert_eq!(dialog.host_display, "user@host");
        assert_eq!(dialog.tab_index, 0);
    }

    #[test]
    fn input_char_appends() {
        let mut dialog = PasswordDialog::new("host".to_string(), 0);
        dialog.input_char('a');
        dialog.input_char('b');
        assert_eq!(dialog.input, "ab");
    }

    #[test]
    fn input_backspace_removes_last() {
        let mut dialog = PasswordDialog::new("host".to_string(), 0);
        dialog.input_char('a');
        dialog.input_char('b');
        dialog.input_backspace();
        assert_eq!(dialog.input, "a");
    }

    #[test]
    fn input_backspace_on_empty_is_noop() {
        let mut dialog = PasswordDialog::new("host".to_string(), 0);
        dialog.input_backspace();
        assert_eq!(dialog.input, "");
    }
}
