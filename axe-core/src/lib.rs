pub mod app;
pub mod command;
pub mod keymap;

pub use app::{AppState, DragBorder, FocusTarget, MouseDragState, ResizeModeState};
pub use command::Command;
pub use keymap::KeymapResolver;

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
