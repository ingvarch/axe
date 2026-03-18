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
