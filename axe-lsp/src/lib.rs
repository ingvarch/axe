pub mod client;
pub mod language;
pub mod manager;
pub mod transport;

pub use axe_config::LspServerConfig;
pub use client::{LspEvent, PendingRequestKind};
pub use manager::LspManager;

#[cfg(test)]
mod tests {
    #[test]
    fn crate_compiles() {
        // Smoke test: if this runs, the crate compiled successfully
    }
}
