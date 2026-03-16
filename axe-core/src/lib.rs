pub mod app;
pub use app::{AppState, FocusTarget};

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
