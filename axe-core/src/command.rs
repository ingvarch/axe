use std::path::PathBuf;

/// Commands that can be dispatched through the keybinding system.
///
/// Each variant represents a discrete action. New features add new variants
/// rather than adding raw key checks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    /// Exit the application immediately (used from confirmation dialog).
    Quit,
    /// Request quit — shows a confirmation dialog.
    RequestQuit,
    /// Cycle focus to the next panel.
    FocusNext,
    /// Cycle focus to the previous panel.
    FocusPrev,
    /// Focus the file tree panel directly.
    FocusTree,
    /// Focus the editor panel directly.
    FocusEditor,
    /// Focus the terminal panel directly.
    FocusTerminal,
    /// Toggle the file tree panel visibility.
    ToggleTree,
    /// Toggle the terminal panel visibility.
    ToggleTerminal,
    /// Show the help overlay.
    ShowHelp,
    /// Close the current overlay.
    CloseOverlay,
    /// Enter panel resize mode.
    EnterResizeMode,
    /// Exit panel resize mode.
    ExitResizeMode,
    /// Resize the active panel leftward.
    ResizeLeft,
    /// Resize the active panel rightward.
    ResizeRight,
    /// Resize the active panel upward.
    ResizeUp,
    /// Resize the active panel downward.
    ResizeDown,
    /// Reset all panels to default sizes.
    EqualizeLayout,
    /// Toggle zoom on the focused panel.
    ZoomPanel,
    /// Move tree selection up (wraps from first to last).
    TreeUp,
    /// Move tree selection down (wraps from last to first).
    TreeDown,
    /// Toggle expand/collapse on directory, noop on file.
    TreeToggle,
    /// Expand directory (right arrow).
    TreeExpand,
    /// Collapse expanded directory, or navigate to parent.
    TreeCollapseOrParent,
    /// Jump to first item in tree.
    TreeHome,
    /// Jump to last item in tree.
    TreeEnd,
    /// Toggle visibility of gitignored files in the file tree.
    ToggleIgnored,
    /// Start creating a new file in the tree.
    TreeCreateFile,
    /// Start creating a new directory in the tree.
    TreeCreateDir,
    /// Start renaming the selected tree node.
    TreeRename,
    /// Start delete confirmation for the selected tree node.
    TreeDelete,
    /// Toggle file type icons in the tree panel.
    ToggleIcons,
    /// Create a new terminal tab.
    NewTerminalTab,
    /// Close the active terminal tab.
    CloseTerminalTab,
    /// Activate a specific terminal tab by index (0-based).
    ActivateTerminalTab(usize),
    /// Scroll terminal up by one page.
    TerminalScrollPageUp,
    /// Scroll terminal down by one page.
    TerminalScrollPageDown,
    /// Scroll terminal to the top of history.
    TerminalScrollTop,
    /// Scroll terminal to the bottom (current output).
    TerminalScrollBottom,
    /// Open a file in the editor from the given path.
    OpenFile(PathBuf),
    /// Move cursor up one line.
    EditorUp,
    /// Move cursor down one line.
    EditorDown,
    /// Move cursor left one character.
    EditorLeft,
    /// Move cursor right one character.
    EditorRight,
    /// Move cursor to beginning of line.
    EditorHome,
    /// Move cursor to end of line.
    EditorEnd,
    /// Move cursor to beginning of file.
    EditorFileStart,
    /// Move cursor to end of file.
    EditorFileEnd,
    /// Scroll/move cursor up by one page.
    EditorPageUp,
    /// Scroll/move cursor down by one page.
    EditorPageDown,
    /// Move cursor to next word boundary.
    EditorWordRight,
    /// Move cursor to previous word boundary.
    EditorWordLeft,
    /// Insert a character at the cursor position.
    EditorInsertChar(char),
    /// Delete the character before the cursor (backspace).
    EditorBackspace,
    /// Delete the character at the cursor position (forward delete).
    EditorDelete,
    /// Insert a newline at the cursor position with auto-indent.
    EditorNewline,
    /// Insert a tab (spaces) at the cursor position.
    EditorTab,
    /// Save the current buffer to disk.
    EditorSave,
    /// Undo the last edit in the active buffer.
    EditorUndo,
    /// Redo the last undone edit in the active buffer.
    EditorRedo,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_variants_are_distinct() {
        let variants: Vec<Command> = vec![
            Command::Quit,
            Command::RequestQuit,
            Command::FocusNext,
            Command::FocusPrev,
            Command::FocusTree,
            Command::FocusEditor,
            Command::FocusTerminal,
            Command::ToggleTree,
            Command::ToggleTerminal,
            Command::ShowHelp,
            Command::CloseOverlay,
            Command::EnterResizeMode,
            Command::ExitResizeMode,
            Command::ResizeLeft,
            Command::ResizeRight,
            Command::ResizeUp,
            Command::ResizeDown,
            Command::EqualizeLayout,
            Command::ZoomPanel,
            Command::TreeUp,
            Command::TreeDown,
            Command::TreeToggle,
            Command::TreeExpand,
            Command::TreeCollapseOrParent,
            Command::TreeHome,
            Command::TreeEnd,
            Command::ToggleIgnored,
            Command::TreeCreateFile,
            Command::TreeCreateDir,
            Command::TreeRename,
            Command::TreeDelete,
            Command::ToggleIcons,
            Command::NewTerminalTab,
            Command::CloseTerminalTab,
            Command::ActivateTerminalTab(0),
            Command::ActivateTerminalTab(1),
            Command::TerminalScrollPageUp,
            Command::TerminalScrollPageDown,
            Command::TerminalScrollTop,
            Command::TerminalScrollBottom,
            Command::OpenFile(PathBuf::from("/tmp/test")),
            Command::EditorUp,
            Command::EditorDown,
            Command::EditorLeft,
            Command::EditorRight,
            Command::EditorHome,
            Command::EditorEnd,
            Command::EditorFileStart,
            Command::EditorFileEnd,
            Command::EditorPageUp,
            Command::EditorPageDown,
            Command::EditorWordRight,
            Command::EditorWordLeft,
            Command::EditorInsertChar('a'),
            Command::EditorInsertChar('b'),
            Command::EditorBackspace,
            Command::EditorDelete,
            Command::EditorNewline,
            Command::EditorTab,
            Command::EditorSave,
            Command::EditorUndo,
            Command::EditorRedo,
        ];

        for (i, a) in variants.iter().enumerate() {
            for (j, b) in variants.iter().enumerate() {
                if i != j {
                    assert_ne!(a, b, "Variants at index {i} and {j} should differ");
                }
            }
        }
    }

    #[test]
    fn command_implements_debug() {
        let output = format!("{:?}", Command::Quit);
        assert!(
            output.contains("Quit"),
            "Debug output should contain 'Quit'"
        );
    }
}
