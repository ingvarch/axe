pub mod app;
pub mod command;
pub mod file_finder;
pub mod keymap;
pub mod search;

pub use app::{
    AppState, ConfirmButton, ConfirmDialog, DragBorder, FocusTarget, MouseDragState,
    ResizeModeState,
};
pub use axe_tree::FileTree;
pub use command::Command;
pub use file_finder::FileFinder;
pub use keymap::KeymapResolver;
pub use search::SearchState;

/// Returns the crate version string from `Cargo.toml`.
pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_returns_crate_version() {
        assert_eq!(version(), "0.1.0");
    }
}
