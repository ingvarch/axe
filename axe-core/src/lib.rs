pub mod ai_overlay;
pub mod app;
pub mod command;
pub mod command_palette;
pub mod completion;
pub mod file_finder;
pub mod fuzzy;
pub mod git;
pub mod hover;
pub mod inlay;
pub mod keymap;
pub mod location_list;
pub mod project_search;
pub mod rename;
pub mod search;
pub mod session;
pub mod signature_help;
pub mod ssh_host;
pub mod ssh_host_finder;

pub use app::{
    AppState, ConfirmButton, ConfirmDialog, DiffPopup, DiffPopupButton, DragBorder, FocusTarget,
    GoToLineDialog, MouseDragState, PasswordDialog, ResizeModeState,
};
pub use axe_tree::FileTree;
pub use command::Command;
pub use command_palette::CommandPalette;
pub use completion::{CompletionItem, CompletionKind, CompletionState};
pub use file_finder::FileFinder;
pub use fuzzy::FilteredItem;
pub use hover::HoverInfo;
pub use inlay::{InlayHint, InlayHintEntry, InlayHintKind, InlayHintStore};
pub use keymap::KeymapResolver;
pub use location_list::LocationList;
pub use project_search::ProjectSearch;
pub use rename::RenameState;
pub use search::{SearchField, SearchState};
pub use signature_help::{ParameterInfo, Signature, SignatureHelpState};

/// Returns the crate version string from `Cargo.toml`.
pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_returns_non_empty_string() {
        assert!(!version().is_empty());
    }
}
