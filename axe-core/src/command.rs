/// Commands that can be dispatched through the keybinding system.
///
/// Each variant represents a discrete action. New features add new variants
/// rather than adding raw key checks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    Quit,
    FocusNext,
    FocusPrev,
    FocusTree,
    FocusEditor,
    FocusTerminal,
    ToggleTree,
    ToggleTerminal,
    ShowHelp,
    CloseOverlay,
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
