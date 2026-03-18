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
    /// Create a new tab in the focused panel (terminal only for now).
    NewTab,
    /// Close the active tab in the focused panel (editor buffer or terminal tab).
    CloseTab,
    /// Switch to the next tab in the focused panel (editor buffer or terminal tab).
    NextTab,
    /// Switch to the previous tab in the focused panel (editor buffer or terminal tab).
    PrevTab,
    /// Create a new terminal tab.
    NewTerminalTab,
    /// Close the active terminal tab (checks if process is alive first).
    CloseTerminalTab,
    /// Force-close the active terminal tab without further prompts.
    ForceCloseTerminalTab,
    /// Cancel the close-terminal-tab confirmation dialog.
    CancelCloseTerminalTab,
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
    /// Open a file as a preview buffer (replaced by next preview, promoted on edit or double-click).
    PreviewFile(PathBuf),
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
    /// Extend selection upward by one line.
    EditorSelectUp,
    /// Extend selection downward by one line.
    EditorSelectDown,
    /// Extend selection left by one character.
    EditorSelectLeft,
    /// Extend selection right by one character.
    EditorSelectRight,
    /// Extend selection to the beginning of the current line.
    EditorSelectHome,
    /// Extend selection to the end of the current line.
    EditorSelectEnd,
    /// Extend selection to the beginning of the file.
    EditorSelectFileStart,
    /// Extend selection to the end of the file.
    EditorSelectFileEnd,
    /// Extend selection to the previous word boundary.
    EditorSelectWordLeft,
    /// Extend selection to the next word boundary.
    EditorSelectWordRight,
    /// Select all text in the active buffer.
    EditorSelectAll,
    /// Copy the current selection to the system clipboard.
    EditorCopy,
    /// Cut the current selection to the system clipboard.
    EditorCut,
    /// Paste from the system clipboard at the cursor position.
    EditorPaste,
    /// Switch to the next open buffer tab.
    NextBuffer,
    /// Switch to the previous open buffer tab.
    PrevBuffer,
    /// Close the active buffer (prompts if modified).
    CloseBuffer,
    /// Confirm closing a modified buffer without saving.
    ConfirmCloseBuffer,
    /// Cancel the close-buffer confirmation dialog.
    CancelCloseBuffer,
    /// Switch to a specific buffer tab by index (0-based).
    ActivateBuffer(usize),
    /// Open or focus the in-file search bar.
    EditorFind,
    /// Close the search bar.
    SearchClose,
    /// Jump to the next search match.
    SearchNextMatch,
    /// Jump to the previous search match.
    SearchPrevMatch,
    /// Toggle case sensitivity in search.
    SearchToggleCase,
    /// Toggle regex mode in search.
    SearchToggleRegex,
    /// Confirm tree node deletion from the confirmation dialog.
    ConfirmTreeDelete,
    /// Cancel tree node deletion from the confirmation dialog.
    CancelTreeDelete,
    /// Open the fuzzy file finder overlay (Ctrl+P).
    OpenFileFinder,
    /// Open the command palette overlay (Ctrl+Shift+P).
    OpenCommandPalette,
    /// Open the project-wide search overlay (Ctrl+Shift+F).
    OpenProjectSearch,
    /// Jump to the next diagnostic in the active buffer.
    GoToNextDiagnostic,
    /// Jump to the previous diagnostic in the active buffer.
    GoToPrevDiagnostic,
    /// Trigger code completion at the current cursor position (Ctrl+Space).
    TriggerCompletion,
    /// Accept the currently selected completion item.
    AcceptCompletion,
    /// Dismiss the completion popup.
    DismissCompletion,
    /// Go to the definition of the symbol under the cursor (F12).
    GoToDefinition,
    /// Find all references to the symbol under the cursor (Shift+F12).
    FindReferences,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unified_tab_commands_exist() {
        // These unified commands dispatch based on focus context.
        let cmds = vec![
            Command::NewTab,
            Command::CloseTab,
            Command::NextTab,
            Command::PrevTab,
        ];
        for cmd in &cmds {
            assert_ne!(
                cmd,
                &Command::Quit,
                "Unified tab commands should be distinct from Quit"
            );
        }
    }

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
            Command::NewTab,
            Command::CloseTab,
            Command::NextTab,
            Command::PrevTab,
            Command::NewTerminalTab,
            Command::CloseTerminalTab,
            Command::ForceCloseTerminalTab,
            Command::CancelCloseTerminalTab,
            Command::ActivateTerminalTab(0),
            Command::ActivateTerminalTab(1),
            Command::TerminalScrollPageUp,
            Command::TerminalScrollPageDown,
            Command::TerminalScrollTop,
            Command::TerminalScrollBottom,
            Command::OpenFile(PathBuf::from("/tmp/test")),
            Command::PreviewFile(PathBuf::from("/tmp/preview")),
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
            Command::EditorSelectUp,
            Command::EditorSelectDown,
            Command::EditorSelectLeft,
            Command::EditorSelectRight,
            Command::EditorSelectHome,
            Command::EditorSelectEnd,
            Command::EditorSelectFileStart,
            Command::EditorSelectFileEnd,
            Command::EditorSelectWordLeft,
            Command::EditorSelectWordRight,
            Command::EditorSelectAll,
            Command::EditorCopy,
            Command::EditorCut,
            Command::EditorPaste,
            Command::NextBuffer,
            Command::PrevBuffer,
            Command::CloseBuffer,
            Command::ConfirmCloseBuffer,
            Command::CancelCloseBuffer,
            Command::ActivateBuffer(0),
            Command::ActivateBuffer(1),
            Command::EditorFind,
            Command::SearchClose,
            Command::SearchNextMatch,
            Command::SearchPrevMatch,
            Command::SearchToggleCase,
            Command::SearchToggleRegex,
            Command::ConfirmTreeDelete,
            Command::CancelTreeDelete,
            Command::OpenFileFinder,
            Command::OpenCommandPalette,
            Command::OpenProjectSearch,
            Command::GoToNextDiagnostic,
            Command::GoToPrevDiagnostic,
            Command::TriggerCompletion,
            Command::AcceptCompletion,
            Command::DismissCompletion,
            Command::GoToDefinition,
            Command::FindReferences,
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
