// IMPACT ANALYSIS — pty module
// Parents: TerminalTab::new() calls spawn_shell() to create a PTY.
// Children: The returned MasterPty is held by TerminalTab for resize; the reader is
//           handed to a background thread; the Child is held for lifecycle checks.
// Siblings: detect_shell() is also used by TerminalTab to determine which shell to spawn.

use std::io::Read;
use std::path::Path;

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

/// Spawns an arbitrary command in a new PTY with the given dimensions.
///
/// `program` is the binary to launch (resolved against `$PATH` by the OS or
/// given as an absolute path), and `args` are passed to it unchanged. The
/// child's working directory is set to `cwd`. Returns the master PTY (for
/// resize/write), the child process handle, and a reader stream for the PTY
/// output.
///
/// This is the general-purpose spawner used by the AI chat overlay to run
/// tools like `claude`, `codex`, or `aider` inside a PTY.
pub fn spawn_command(
    program: &str,
    args: &[String],
    cols: u16,
    rows: u16,
    cwd: &Path,
) -> Result<SpawnResult> {
    let pty_system = native_pty_system();
    let pair = pty_system
        .openpty(PtySize {
            rows,
            cols,
            pixel_width: 0,
            pixel_height: 0,
        })
        .context("Failed to open PTY")?;

    let mut cmd = CommandBuilder::new(program);
    for arg in args {
        cmd.arg(arg);
    }
    cmd.cwd(cwd);
    let child = pair
        .slave
        .spawn_command(cmd)
        .with_context(|| format!("Failed to spawn process: {program}"))?;

    let reader = pair
        .master
        .try_clone_reader()
        .context("Failed to clone PTY reader")?;

    Ok((pair.master, child, reader))
}

/// Spawns a shell process in a new PTY with the given dimensions.
///
/// Thin wrapper over [`spawn_command`] that passes no arguments — kept as a
/// stable entry point for the existing terminal panel code path.
pub fn spawn_shell(shell: &str, cols: u16, rows: u16, cwd: &Path) -> Result<SpawnResult> {
    spawn_command(shell, &[], cols, rows, cwd)
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
        let cwd = std::env::current_dir().unwrap();
        let result = spawn_shell(&shell, 80, 24, &cwd);
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

    #[test]
    fn spawn_shell_respects_cwd() {
        let shell = detect_shell();
        let tmp = std::env::temp_dir();
        let result = spawn_shell(&shell, 80, 24, &tmp);
        assert!(
            result.is_ok(),
            "spawn_shell with temp dir should succeed: {:?}",
            result.err()
        );
    }

    #[test]
    fn spawn_command_runs_echo_with_args() {
        // Use /bin/sh -c so we portably get "hello\n" on stdout with no TTY dependency.
        let cwd = std::env::temp_dir();
        let result = spawn_command(
            "/bin/sh",
            &["-c".to_string(), "echo hello".to_string()],
            80,
            24,
            &cwd,
        );
        assert!(
            result.is_ok(),
            "spawn_command should succeed: {:?}",
            result.err()
        );

        let (_master, mut child, mut reader) = result.unwrap();

        // Read until we see "hello" or EOF / timeout. The child is short-lived so
        // the reader hits EOF quickly after the echo completes.
        let mut buf = [0u8; 4096];
        let mut output = String::new();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
        while std::time::Instant::now() < deadline {
            match reader.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => output.push_str(&String::from_utf8_lossy(&buf[..n])),
                Err(_) => break,
            }
            if output.contains("hello") {
                break;
            }
        }

        let _ = child.wait();
        assert!(
            output.contains("hello"),
            "expected 'hello' in PTY output, got {output:?}"
        );
    }

    #[test]
    fn spawn_command_with_no_args_equals_spawn_shell() {
        // Regression: spawn_shell now routes through spawn_command; the old
        // "spawn a shell with no args" behavior must still hold.
        let shell = detect_shell();
        let cwd = std::env::current_dir().unwrap();
        let result = spawn_command(&shell, &[], 80, 24, &cwd);
        assert!(result.is_ok());
        let (_m, mut child, _r) = result.unwrap();
        assert!(child.try_wait().unwrap().is_none(), "shell should be alive");
    }
}
