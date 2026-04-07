// IMPACT ANALYSIS — SshTerminalTab
// Parents: TerminalManager::spawn_ssh_tab() creates this tab.
//          ManagedTab::Ssh variant wraps this.
// Children: ssh_connect async task reads input_tx and sends TermEvent::Output.
// Siblings: TerminalTab (local) — shares ManagedTab interface.
//           TerminalManager::poll_output handles SSH events.
// Risk: Must handle disconnect gracefully. Resize must propagate to remote.

use std::sync::mpsc;

use alacritty_terminal::grid::Scroll;
use alacritty_terminal::index::{Direction, Point};
use alacritty_terminal::selection::{Selection, SelectionType};
use alacritty_terminal::term::Config as TermConfig;
use alacritty_terminal::vte::ansi;
use alacritty_terminal::Term;

use crate::tab::TermSize;

use anyhow::Result;

use crate::event_listener::{PtyEvent, PtyEventListener};

/// State of the SSH connection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SshConnectionState {
    /// Attempting to connect and authenticate.
    Connecting,
    /// Authenticated and PTY channel open.
    Connected,
    /// Connection closed or failed.
    Disconnected(String),
    /// Agent/key auth failed, password required.
    NeedsPassword,
}

/// Commands sent from the main thread to the SSH async task.
pub enum SshInput {
    /// Raw input bytes to send to the remote shell.
    Data(Vec<u8>),
    /// Terminal was resized — send window-change request.
    Resize(u16, u16),
    /// User-provided password for authentication.
    Password(String),
    /// Close the connection.
    Close,
}

/// A terminal tab backed by a remote SSH connection.
pub struct SshTerminalTab {
    title: String,
    term: Term<PtyEventListener>,
    pty_event_rx: mpsc::Receiver<PtyEvent>,
    processor: ansi::Processor,
    last_cols: u16,
    last_rows: u16,
    /// Channel to send input to the SSH async task.
    input_tx: tokio::sync::mpsc::UnboundedSender<SshInput>,
    /// Current connection state.
    pub state: SshConnectionState,
}

impl SshTerminalTab {
    /// Creates a new SSH terminal tab in `Connecting` state.
    ///
    /// Returns the tab and the receiver end of the input channel
    /// (to be consumed by the SSH async task).
    pub fn new(
        cols: u16,
        rows: u16,
        user: &str,
        hostname: &str,
    ) -> (Self, tokio::sync::mpsc::UnboundedReceiver<SshInput>) {
        let size = TermSize::new(cols, rows);
        let (pty_event_tx, pty_event_rx) = mpsc::channel();
        let term = Term::new(
            TermConfig::default(),
            &size,
            PtyEventListener::new(pty_event_tx),
        );
        let processor = ansi::Processor::new();

        let (input_tx, input_rx) = tokio::sync::mpsc::unbounded_channel();

        let title = format!("{user}@{hostname}");

        let tab = Self {
            title,
            term,
            pty_event_rx,
            processor,
            last_cols: cols,
            last_rows: rows,
            input_tx,
            state: SshConnectionState::Connecting,
        };

        (tab, input_rx)
    }

    /// Sends raw input bytes to the SSH async task.
    pub fn write(&mut self, data: &[u8]) -> Result<()> {
        if self.state != SshConnectionState::Connected {
            return Ok(());
        }
        self.input_tx
            .send(SshInput::Data(data.to_vec()))
            .map_err(|_| anyhow::anyhow!("SSH input channel closed"))?;
        Ok(())
    }

    /// Feeds raw output bytes from the SSH channel into the VT parser.
    pub fn process_output(&mut self, data: &[u8]) {
        self.processor.advance(&mut self.term, data);
        // Drain any events from the terminal emulator (e.g., DSR responses).
        // For SSH, we send these back through the input channel.
        while let Ok(event) = self.pty_event_rx.try_recv() {
            match event {
                PtyEvent::Write(s) => {
                    let _ = self.input_tx.send(SshInput::Data(s.into_bytes()));
                }
                PtyEvent::Title(s) => {
                    // Preserve the user@host prefix, append shell title.
                    let base = self.title.split(" - ").next().unwrap_or(&self.title);
                    self.title = format!("{base} - {s}");
                }
                PtyEvent::Bell => {}
            }
        }
    }

    /// Resizes the local terminal grid and sends a resize request to the SSH task.
    pub fn resize(&mut self, cols: u16, rows: u16) -> Result<()> {
        if cols == self.last_cols && rows == self.last_rows {
            return Ok(());
        }

        let size = TermSize::new(cols, rows);
        self.term.resize(size);
        self.last_cols = cols;
        self.last_rows = rows;

        let _ = self.input_tx.send(SshInput::Resize(cols, rows));
        Ok(())
    }

    /// Returns a reference to the alacritty terminal state for rendering.
    pub fn term(&self) -> &Term<PtyEventListener> {
        &self.term
    }

    /// Returns a mutable reference to the alacritty terminal state.
    pub fn term_mut(&mut self) -> &mut Term<PtyEventListener> {
        &mut self.term
    }

    /// Returns the tab title (e.g., "user@hostname").
    pub fn title(&self) -> &str {
        &self.title
    }

    /// Sends a close request to the SSH async task.
    pub fn kill(&mut self) -> Result<()> {
        let _ = self.input_tx.send(SshInput::Close);
        self.state = SshConnectionState::Disconnected("closed".to_string());
        Ok(())
    }

    /// Scrolls the terminal display.
    pub fn scroll(&mut self, scroll: Scroll) {
        self.term.scroll_display(scroll);
    }

    /// Returns the current display offset.
    pub fn display_offset(&self) -> usize {
        self.term.grid().display_offset()
    }

    /// Returns whether the SSH connection is alive.
    pub fn is_alive(&mut self) -> bool {
        matches!(
            self.state,
            SshConnectionState::Connecting
                | SshConnectionState::Connected
                | SshConnectionState::NeedsPassword
        )
    }

    /// Sends a password to the SSH async task for authentication.
    pub fn send_password(&self, password: String) {
        let _ = self.input_tx.send(SshInput::Password(password));
    }

    pub fn start_selection(&mut self, ty: SelectionType, point: Point, side: Direction) {
        self.term.selection = Some(Selection::new(ty, point, side));
    }

    pub fn update_selection(&mut self, point: Point, side: Direction) {
        if let Some(ref mut sel) = self.term.selection {
            sel.update(point, side);
        }
    }

    pub fn selection_to_string(&self) -> Option<String> {
        self.term.selection_to_string()
    }

    pub fn clear_selection(&mut self) {
        self.term.selection = None;
    }

    pub fn has_selection(&self) -> bool {
        self.term.selection.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_creates_tab_in_connecting_state() {
        let (tab, _rx) = SshTerminalTab::new(80, 24, "user", "example.com");
        assert_eq!(tab.state, SshConnectionState::Connecting);
        assert_eq!(tab.title(), "user@example.com");
    }

    #[test]
    fn write_ignored_when_not_connected() {
        let (mut tab, _rx) = SshTerminalTab::new(80, 24, "user", "host");
        // State is Connecting, write should be a no-op.
        assert!(tab.write(b"hello").is_ok());
    }

    #[test]
    fn write_sends_data_when_connected() {
        let (mut tab, mut rx) = SshTerminalTab::new(80, 24, "user", "host");
        tab.state = SshConnectionState::Connected;
        tab.write(b"hello").unwrap();
        match rx.try_recv() {
            Ok(SshInput::Data(data)) => assert_eq!(data, b"hello"),
            other => panic!("Expected Data, got {:?}", other.is_ok()),
        }
    }

    #[test]
    fn resize_sends_resize_input() {
        let (mut tab, mut rx) = SshTerminalTab::new(80, 24, "user", "host");
        tab.resize(120, 40).unwrap();
        match rx.try_recv() {
            Ok(SshInput::Resize(cols, rows)) => {
                assert_eq!(cols, 120);
                assert_eq!(rows, 40);
            }
            other => panic!("Expected Resize, got {:?}", other.is_ok()),
        }
    }

    #[test]
    fn resize_same_size_is_noop() {
        let (mut tab, mut rx) = SshTerminalTab::new(80, 24, "user", "host");
        tab.resize(80, 24).unwrap();
        assert!(rx.try_recv().is_err(), "No event for same-size resize");
    }

    #[test]
    fn kill_sends_close_and_disconnects() {
        let (mut tab, mut rx) = SshTerminalTab::new(80, 24, "user", "host");
        tab.kill().unwrap();
        assert_eq!(
            tab.state,
            SshConnectionState::Disconnected("closed".to_string())
        );
        assert!(matches!(rx.try_recv(), Ok(SshInput::Close)));
    }

    #[test]
    fn is_alive_in_various_states() {
        let (mut tab, _rx) = SshTerminalTab::new(80, 24, "user", "host");
        assert!(tab.is_alive()); // Connecting

        tab.state = SshConnectionState::Connected;
        assert!(tab.is_alive());

        tab.state = SshConnectionState::NeedsPassword;
        assert!(tab.is_alive());

        tab.state = SshConnectionState::Disconnected("error".to_string());
        assert!(!tab.is_alive());
    }

    #[test]
    fn send_password_sends_input() {
        let (tab, mut rx) = SshTerminalTab::new(80, 24, "user", "host");
        tab.send_password("secret".to_string());
        match rx.try_recv() {
            Ok(SshInput::Password(pw)) => assert_eq!(pw, "secret"),
            other => panic!("Expected Password, got {:?}", other.is_ok()),
        }
    }

    #[test]
    fn process_output_updates_terminal_grid() {
        let (mut tab, _rx) = SshTerminalTab::new(80, 24, "user", "host");
        // Feed some output — should not panic.
        tab.process_output(b"Hello, SSH!\r\n");
    }
}
