// IMPACT ANALYSIS — ai_overlay module
// Parents: AppState holds an AiOverlay value (always present, never Option).
// Children: registry provides the list of available agents. detect filters by PATH.
//           The actual PTY session lives in AiSession which wraps
//           axe_terminal::TerminalTab.
// Siblings: command dispatch (execute.rs) drives lifecycle; input routing (input.rs)
//           forwards keystrokes into the PTY; rendering (axe-ui/src/ai_overlay.rs)
//           draws the modal over the rest of the UI.
// Risk: the background reader thread is owned by AiSession and torn down via a
//       poisoned output channel when AiSession is dropped. Do NOT replace the
//       channel without also joining the thread — it would leak.

pub mod detect;
pub mod registry;

use std::path::Path;
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread::{self, JoinHandle};

use anyhow::{Context, Result};
use axe_terminal::tab::TerminalTab;

use crate::ai_overlay::registry::ResolvedAgent;

/// Initial dimensions used when spawning a PTY for the AI overlay before it is
/// ever rendered. The first resize call from the UI layer corrects these to
/// match the centered modal's rect.
const DEFAULT_COLS: u16 = 100;
const DEFAULT_ROWS: u16 = 30;

/// State container for the AI chat overlay.
///
/// Unlike other overlays in Axe, this one is **not** modeled as `Option<T>`: the
/// struct is always present on `AppState`, and `visible` controls whether the
/// UI draws it. This is the only way to let a user hide the overlay without
/// killing the underlying PTY session.
#[derive(Default)]
pub struct AiOverlay {
    pub visible: bool,
    pub session: Option<AiSession>,
    pub picker: Option<AgentPicker>,
}

impl AiOverlay {
    pub fn new() -> Self {
        Self::default()
    }

    /// Toggles the visibility flag without touching the session.
    ///
    /// This is the "minimize/restore" operation: hiding an AI overlay must keep
    /// the PTY alive so the chat history and any in-flight request survive.
    pub fn toggle_visible(&mut self) {
        self.visible = !self.visible;
    }

    /// Starts a new PTY session running the given agent.
    ///
    /// Returns an error if the child process fails to spawn. Any existing
    /// session is dropped (and therefore killed) before the new one starts —
    /// callers should show a confirmation dialog before invoking this when
    /// the old session is still alive.
    pub fn start_session(&mut self, agent: &ResolvedAgent, cwd: &Path) -> Result<()> {
        // Dropping the old session runs its Drop impl, which kills the child.
        self.session = None;
        let session = AiSession::spawn(agent, cwd)?;
        self.session = Some(session);
        Ok(())
    }

    /// Returns true if a session exists and its child process is still running.
    ///
    /// Used by the toggle flow to detect `/exit`-in-agent cases and respawn
    /// on the next show.
    pub fn session_is_alive(&mut self) -> bool {
        match self.session.as_mut() {
            Some(s) => s.is_alive(),
            None => false,
        }
    }

    /// Drops the current session (killing the child if it is still alive).
    pub fn kill_session(&mut self) {
        self.session = None;
    }

    /// Feeds any pending PTY output into the session's terminal buffer.
    ///
    /// Must be called from the main loop once per tick so the overlay's
    /// visible contents stay in sync with the live PTY.
    pub fn drain_output(&mut self) {
        let Some(session) = self.session.as_mut() else {
            return;
        };
        while let Ok(chunk) = session.output_rx.try_recv() {
            session.tab.process_output(&chunk);
        }
    }

    /// Resizes the AI session's PTY so its grid matches the inner area of the
    /// centered modal the UI will draw.
    ///
    /// The overlay is rendered as an 80%×80% centered rect with a 1-char
    /// border on every side, so the usable grid is `(full*0.8) - 2` in each
    /// dimension. Without this call the PTY stays at its spawn size (100×30)
    /// and the rendered grid visibly fails to fill the overlay on terminals
    /// whose 80% is larger than that.
    ///
    /// Safe to call on every tick: `TerminalTab::resize` is a no-op when the
    /// dimensions are unchanged.
    pub fn sync_pty_size(&mut self, full_cols: u16, full_rows: u16) {
        let Some(session) = self.session.as_mut() else {
            return;
        };
        let inner_cols = ((full_cols as u32) * 80 / 100).saturating_sub(2) as u16;
        let inner_rows = ((full_rows as u32) * 80 / 100).saturating_sub(2) as u16;
        if inner_cols == 0 || inner_rows == 0 {
            return;
        }
        if let Err(e) = session.tab.resize(inner_cols, inner_rows) {
            log::warn!("Failed to resize AI overlay PTY: {e}");
        }
    }
}

/// A live AI CLI session wrapped around a `TerminalTab`.
///
/// Owns the PTY, the child process, and the background reader thread. All
/// of these are torn down together in `Drop`.
pub struct AiSession {
    pub agent_id: String,
    pub display_name: String,
    pub tab: TerminalTab,
    output_rx: Receiver<Vec<u8>>,
    reader_thread: Option<JoinHandle<()>>,
}

impl AiSession {
    /// Spawns `agent` in a new PTY and starts the background reader thread.
    pub fn spawn(agent: &ResolvedAgent, cwd: &Path) -> Result<Self> {
        let (tab, reader) = TerminalTab::new_with_command(
            DEFAULT_COLS,
            DEFAULT_ROWS,
            cwd,
            &agent.command,
            &agent.args,
        )
        .with_context(|| format!("Failed to spawn AI agent '{}'", agent.command))?;

        let (tx, rx) = mpsc::channel::<Vec<u8>>();
        let handle = spawn_reader_thread(reader, tx);

        Ok(Self {
            agent_id: agent.id.clone(),
            display_name: agent.display.clone(),
            tab,
            output_rx: rx,
            reader_thread: Some(handle),
        })
    }

    pub fn is_alive(&mut self) -> bool {
        self.tab.is_alive()
    }
}

impl Drop for AiSession {
    fn drop(&mut self) {
        let _ = self.tab.kill();
        // The reader thread exits on its own once the PTY reader hits EOF/Err
        // after the child is killed. We don't join it here because joining on
        // Drop can deadlock if the thread is mid-read on a slow syscall.
        let _ = self.reader_thread.take();
    }
}

/// State for the first-run / switch-agent picker overlay.
///
/// Holds the list of detected agents and the currently highlighted index.
/// Rendered inside the same modal rect as the AI chat itself.
pub struct AgentPicker {
    pub items: Vec<ResolvedAgent>,
    pub selected: usize,
}

impl AgentPicker {
    pub fn new(items: Vec<ResolvedAgent>) -> Self {
        Self { items, selected: 0 }
    }

    pub fn move_up(&mut self) {
        if self.selected > 0 {
            self.selected -= 1;
        }
    }

    pub fn move_down(&mut self) {
        if !self.items.is_empty() && self.selected + 1 < self.items.len() {
            self.selected += 1;
        }
    }

    pub fn selected_agent(&self) -> Option<&ResolvedAgent> {
        self.items.get(self.selected)
    }
}

const PTY_READ_BUF_SIZE: usize = 4096;

/// Spawns a background thread that drains the PTY reader and forwards chunks
/// over the given sender.
///
/// Terminates cleanly on read error or EOF, or when the receiver is dropped.
fn spawn_reader_thread(
    mut reader: Box<dyn std::io::Read + Send>,
    tx: Sender<Vec<u8>>,
) -> JoinHandle<()> {
    thread::spawn(move || {
        let mut buf = [0u8; PTY_READ_BUF_SIZE];
        loop {
            match reader.read(&mut buf) {
                Ok(0) | Err(_) => break,
                Ok(n) => {
                    if tx.send(buf[..n].to_vec()).is_err() {
                        break;
                    }
                }
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fake_agent(id: &str, command: &str, args: &[&str]) -> ResolvedAgent {
        ResolvedAgent {
            id: id.to_string(),
            command: command.to_string(),
            args: args.iter().map(|s| s.to_string()).collect(),
            display: id.to_string(),
        }
    }

    fn cat_agent() -> ResolvedAgent {
        // /bin/cat with no args blocks on stdin forever — ideal "long-running
        // fake AI agent" for lifecycle tests on Unix.
        fake_agent("cat", "/bin/cat", &[])
    }

    fn sleep_agent(secs: u32) -> ResolvedAgent {
        fake_agent("sleep", "/bin/sleep", &[&secs.to_string()])
    }

    #[test]
    fn new_is_empty_and_hidden() {
        let overlay = AiOverlay::new();
        assert!(!overlay.visible);
        assert!(overlay.session.is_none());
        assert!(overlay.picker.is_none());
    }

    #[test]
    fn toggle_visible_flips_flag_without_session() {
        let mut overlay = AiOverlay::new();
        overlay.toggle_visible();
        assert!(overlay.visible);
        overlay.toggle_visible();
        assert!(!overlay.visible);
        assert!(overlay.session.is_none());
    }

    #[test]
    fn start_session_creates_live_session() {
        let mut overlay = AiOverlay::new();
        let cwd = std::env::current_dir().unwrap();
        overlay
            .start_session(&cat_agent(), &cwd)
            .expect("spawn cat");

        assert!(overlay.session.is_some());
        assert!(overlay.session_is_alive(), "cat should be alive");
        // Sanity: metadata is stored.
        assert_eq!(overlay.session.as_ref().unwrap().agent_id, "cat");
    }

    #[test]
    fn toggle_visible_with_live_session_does_not_kill_it() {
        // THE key test: hiding the overlay must preserve the PTY session.
        let mut overlay = AiOverlay::new();
        let cwd = std::env::current_dir().unwrap();
        overlay.start_session(&cat_agent(), &cwd).expect("spawn");
        overlay.visible = true;

        overlay.toggle_visible(); // hide
        assert!(!overlay.visible);
        assert!(
            overlay.session_is_alive(),
            "session must survive the hide toggle"
        );

        overlay.toggle_visible(); // show again
        assert!(overlay.visible);
        assert!(
            overlay.session_is_alive(),
            "session must still be alive after show"
        );
    }

    #[test]
    fn kill_session_drops_session() {
        let mut overlay = AiOverlay::new();
        let cwd = std::env::current_dir().unwrap();
        overlay.start_session(&cat_agent(), &cwd).expect("spawn");
        assert!(overlay.session.is_some());

        overlay.kill_session();
        assert!(overlay.session.is_none());
    }

    #[test]
    fn start_session_replaces_previous_session() {
        let mut overlay = AiOverlay::new();
        let cwd = std::env::current_dir().unwrap();
        overlay.start_session(&cat_agent(), &cwd).expect("spawn 1");
        overlay
            .start_session(&sleep_agent(30), &cwd)
            .expect("spawn 2");

        assert_eq!(
            overlay.session.as_ref().unwrap().agent_id,
            "sleep",
            "new session replaces previous"
        );
        assert!(overlay.session_is_alive());
        overlay.kill_session();
    }

    #[test]
    fn dead_child_reports_not_alive() {
        // `sh -c "exit 0"` exits immediately. After a tiny delay, is_alive
        // must be false. Portable across macOS/Linux; avoids hardcoding a path
        // like /bin/true which lives in /usr/bin on macOS.
        let mut overlay = AiOverlay::new();
        let cwd = std::env::current_dir().unwrap();
        overlay
            .start_session(
                &fake_agent("exit-agent", "/bin/sh", &["-c", "exit 0"]),
                &cwd,
            )
            .expect("spawn exit-agent");

        // Wait briefly for the process to actually exit.
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
        while std::time::Instant::now() < deadline && overlay.session_is_alive() {
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        assert!(
            !overlay.session_is_alive(),
            "/bin/true should have exited by now"
        );
    }

    #[test]
    fn session_is_alive_returns_false_without_session() {
        let mut overlay = AiOverlay::new();
        assert!(!overlay.session_is_alive());
    }

    #[test]
    fn drain_output_is_noop_without_session() {
        let mut overlay = AiOverlay::new();
        overlay.drain_output(); // must not panic
    }

    #[test]
    fn agent_picker_navigation() {
        let items = vec![
            fake_agent("a", "a", &[]),
            fake_agent("b", "b", &[]),
            fake_agent("c", "c", &[]),
        ];
        let mut picker = AgentPicker::new(items);
        assert_eq!(picker.selected, 0);
        picker.move_up(); // already at top
        assert_eq!(picker.selected, 0);
        picker.move_down();
        assert_eq!(picker.selected, 1);
        picker.move_down();
        assert_eq!(picker.selected, 2);
        picker.move_down(); // already at bottom
        assert_eq!(picker.selected, 2);
        assert_eq!(picker.selected_agent().unwrap().id, "c");
        picker.move_up();
        assert_eq!(picker.selected_agent().unwrap().id, "b");
    }

    #[test]
    fn agent_picker_empty_list_has_no_selected_agent() {
        let picker = AgentPicker::new(Vec::new());
        assert!(picker.selected_agent().is_none());
    }

    #[test]
    fn sync_pty_size_resizes_live_session() {
        let mut overlay = AiOverlay::new();
        let cwd = std::env::current_dir().unwrap();
        overlay.start_session(&cat_agent(), &cwd).expect("spawn");

        // Full terminal 200x60. Inner = (200*0.8)-2 = 158 cols, (60*0.8)-2 = 46 rows.
        overlay.sync_pty_size(200, 60);

        let (cols, rows) = {
            let term = overlay.session.as_ref().unwrap().tab.term();
            use alacritty_terminal::grid::Dimensions;
            (term.grid().columns(), term.grid().screen_lines())
        };
        assert_eq!(cols, 158);
        assert_eq!(rows, 46);
    }

    #[test]
    fn sync_pty_size_noop_without_session() {
        let mut overlay = AiOverlay::new();
        overlay.sync_pty_size(200, 60); // must not panic
        assert!(overlay.session.is_none());
    }

    #[test]
    fn sync_pty_size_ignores_tiny_terminals() {
        // 4 cols -> (4*0.8)=3, -2 = 1; 3 rows -> 0 - clamped. Should early-return
        // rather than trying to resize to 0 rows (which some PTY impls reject).
        let mut overlay = AiOverlay::new();
        let cwd = std::env::current_dir().unwrap();
        overlay.start_session(&cat_agent(), &cwd).expect("spawn");
        overlay.sync_pty_size(4, 3); // inner rows = 0 → early return
                                     // Session stays alive.
        assert!(overlay.session_is_alive());
    }
}
