// IMPACT ANALYSIS — manager module
// Parents: AppState holds Option<TerminalManager>; main.rs creates and polls it.
// Children: TerminalTab for individual tab management; background reader threads.
// Siblings: axe-ui reads active_tab() for rendering; main loop calls resize_active()
//           when panel dimensions change.

use std::io::Read;
use std::path::Path;
use std::sync::mpsc::{self, Receiver, Sender};

use anyhow::{Context, Result};
use log;

use crate::tab::TerminalTab;

/// Size of the read buffer for the background PTY reader thread.
const PTY_READ_BUF_SIZE: usize = 4096;

/// Events sent from background reader threads to the main thread.
pub enum TermEvent {
    /// Raw output bytes from the PTY.
    Output(Vec<u8>),
    /// The child process has exited.
    ChildExited,
}

/// Manages terminal tabs and their background I/O threads.
pub struct TerminalManager {
    tabs: Vec<TerminalTab>,
    active: usize,
    event_rx: Receiver<TermEvent>,
    event_tx: Sender<TermEvent>,
}

impl Default for TerminalManager {
    fn default() -> Self {
        Self::new()
    }
}

impl TerminalManager {
    /// Creates a new manager with no tabs.
    pub fn new() -> Self {
        let (event_tx, event_rx) = mpsc::channel();
        Self {
            tabs: Vec::new(),
            active: 0,
            event_rx,
            event_tx,
        }
    }

    /// Spawns a new terminal tab with the user's default shell.
    ///
    /// Starts a background thread that reads PTY output and sends it through
    /// the internal channel.
    pub fn spawn_default_tab(&mut self, cols: u16, rows: u16, cwd: &Path) -> Result<()> {
        let (tab, reader) =
            TerminalTab::new(cols, rows, cwd).context("Failed to create terminal tab")?;
        self.tabs.push(tab);

        let tx = self.event_tx.clone();
        spawn_reader_thread(reader, tx);

        Ok(())
    }

    /// Drains pending PTY output from the channel and feeds it to the active tab.
    pub fn poll_output(&mut self) {
        while let Ok(event) = self.event_rx.try_recv() {
            match event {
                TermEvent::Output(data) => {
                    if let Some(tab) = self.tabs.get_mut(self.active) {
                        tab.process_output(&data);
                    }
                }
                TermEvent::ChildExited => {
                    log::info!("Terminal child process exited");
                }
            }
        }
    }

    /// Writes raw bytes to the active terminal tab's PTY.
    ///
    /// No-op if there are no tabs.
    pub fn write_to_active(&mut self, data: &[u8]) -> Result<()> {
        if let Some(tab) = self.tabs.get_mut(self.active) {
            tab.write(data)?;
        }
        Ok(())
    }

    /// Returns a reference to the currently active tab, if any.
    pub fn active_tab(&self) -> Option<&TerminalTab> {
        self.tabs.get(self.active)
    }

    /// Resizes the active terminal tab's PTY and grid.
    pub fn resize_active(&mut self, cols: u16, rows: u16) -> Result<()> {
        if let Some(tab) = self.tabs.get_mut(self.active) {
            tab.resize(cols, rows)?;
        }
        Ok(())
    }

    /// Returns the number of tabs.
    pub fn tab_count(&self) -> usize {
        self.tabs.len()
    }
}

/// Spawns a background thread that reads from the PTY and sends output events.
fn spawn_reader_thread(mut reader: Box<dyn Read + Send>, tx: Sender<TermEvent>) {
    std::thread::spawn(move || {
        let mut buf = [0u8; PTY_READ_BUF_SIZE];
        loop {
            match reader.read(&mut buf) {
                Ok(0) | Err(_) => {
                    let _ = tx.send(TermEvent::ChildExited);
                    break;
                }
                Ok(n) => {
                    if tx.send(TermEvent::Output(buf[..n].to_vec())).is_err() {
                        break;
                    }
                }
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_has_no_tabs() {
        let mgr = TerminalManager::new();
        assert_eq!(mgr.tab_count(), 0);
        assert!(mgr.active_tab().is_none());
    }

    #[test]
    fn spawn_default_tab_adds_tab() {
        let mut mgr = TerminalManager::new();
        let result = mgr.spawn_default_tab(80, 24, &std::env::current_dir().unwrap());
        assert!(
            result.is_ok(),
            "spawn_default_tab should succeed: {:?}",
            result.err()
        );
        assert_eq!(mgr.tab_count(), 1);
        assert!(mgr.active_tab().is_some());
    }

    #[test]
    fn poll_output_processes_channel_data() {
        let mut mgr = TerminalManager::new();
        mgr.spawn_default_tab(80, 24, &std::env::current_dir().unwrap())
            .unwrap();

        // Manually send test data through the channel.
        mgr.event_tx
            .send(TermEvent::Output(b"test".to_vec()))
            .unwrap();
        mgr.poll_output();

        // Verify the data was processed by reading the grid.
        let tab = mgr.active_tab().unwrap();
        let grid = tab.term().grid();
        let mut text = String::new();
        for col in 0..4 {
            let cell =
                &grid[alacritty_terminal::index::Line(0)][alacritty_terminal::index::Column(col)];
            text.push(cell.c);
        }
        assert_eq!(text, "test");
    }

    #[test]
    fn resize_active_works() {
        let mut mgr = TerminalManager::new();
        mgr.spawn_default_tab(80, 24, &std::env::current_dir().unwrap())
            .unwrap();

        let result = mgr.resize_active(120, 40);
        assert!(
            result.is_ok(),
            "resize_active should succeed: {:?}",
            result.err()
        );
    }

    #[test]
    fn resize_active_noop_without_tabs() {
        let mut mgr = TerminalManager::new();
        let result = mgr.resize_active(120, 40);
        assert!(
            result.is_ok(),
            "resize_active with no tabs should be a no-op"
        );
    }

    #[test]
    fn poll_output_noop_without_tabs() {
        let mut mgr = TerminalManager::new();
        mgr.poll_output(); // Should not panic.
    }

    #[test]
    fn write_to_active_works() {
        let mut mgr = TerminalManager::new();
        mgr.spawn_default_tab(80, 24, &std::env::current_dir().unwrap())
            .unwrap();
        let result = mgr.write_to_active(b"echo test\n");
        assert!(
            result.is_ok(),
            "write_to_active should succeed: {:?}",
            result.err()
        );
    }

    #[test]
    fn write_to_active_noop_without_tabs() {
        let mut mgr = TerminalManager::new();
        let result = mgr.write_to_active(b"hello");
        assert!(
            result.is_ok(),
            "write_to_active with no tabs should be a no-op"
        );
    }
}
