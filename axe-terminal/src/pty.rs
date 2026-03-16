// IMPACT ANALYSIS — pty module
// Parents: TerminalTab::new() calls spawn_shell() to create a PTY.
// Children: The returned MasterPty is held by TerminalTab for resize; the reader is
//           handed to a background thread; the Child is held for lifecycle checks.
// Siblings: detect_shell() is also used by TerminalTab to determine which shell to spawn.

use std::io::Read;

use anyhow::{Context, Result};
use portable_pty::{native_pty_system, Child, CommandBuilder, MasterPty, PtySize};

/// Return type for `spawn_shell`: master PTY, child process, and PTY reader.
pub type SpawnResult = (
    Box<dyn MasterPty + Send>,
    Box<dyn Child + Send + Sync>,
    Box<dyn Read + Send>,
);

/// Detects the user's preferred shell from the `$SHELL` environment variable.
///
/// Falls back to `/bin/bash` if the variable is unset or empty.
pub fn detect_shell() -> String {
    std::env::var("SHELL")
        .ok()
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "/bin/bash".to_owned())
}

/// Spawns a shell process in a new PTY with the given dimensions.
///
/// Returns the master PTY (for resize/write), the child process handle, and a
/// reader stream for the PTY output.
pub fn spawn_shell(shell: &str, cols: u16, rows: u16) -> Result<SpawnResult> {
    let pty_system = native_pty_system();
    let pair = pty_system
        .openpty(PtySize {
            rows,
            cols,
            pixel_width: 0,
            pixel_height: 0,
        })
        .context("Failed to open PTY")?;

    let cmd = CommandBuilder::new(shell);
    let child = pair
        .slave
        .spawn_command(cmd)
        .context("Failed to spawn shell process")?;

    let reader = pair
        .master
        .try_clone_reader()
        .context("Failed to clone PTY reader")?;

    Ok((pair.master, child, reader))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detect_shell_returns_nonempty() {
        let shell = detect_shell();
        assert!(
            !shell.is_empty(),
            "detect_shell should return a non-empty string"
        );
    }

    #[test]
    fn detect_shell_default_exists() {
        // On macOS/Linux, /bin/bash or /bin/sh should exist.
        let fallback = "/bin/bash";
        assert!(
            std::path::Path::new(fallback).exists() || std::path::Path::new("/bin/sh").exists(),
            "At least one of /bin/bash or /bin/sh should exist"
        );
    }

    #[test]
    fn spawn_shell_creates_valid_pty() {
        let shell = detect_shell();
        let result = spawn_shell(&shell, 80, 24);
        assert!(
            result.is_ok(),
            "spawn_shell should succeed: {:?}",
            result.err()
        );

        let (_master, mut child, _reader) = result.unwrap();
        // Child should be alive immediately after spawn.
        assert!(
            child.try_wait().unwrap().is_none(),
            "Child should still be running"
        );
    }
}
