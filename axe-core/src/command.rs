/// Commands that can be dispatched through the keybinding system.
///
/// Each variant represents a discrete action. New features add new variants
/// rather than adding raw key checks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    /// Exit the application.
    Quit,
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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_variants_are_distinct() {
        let variants: Vec<Command> = vec![
            Command::Quit,
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
