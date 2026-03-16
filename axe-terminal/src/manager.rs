// IMPACT ANALYSIS — manager module
// Parents: AppState holds Option<TerminalManager>; main.rs creates and polls it.
// Children: TerminalTab for individual tab management; background reader threads.
// Siblings: axe-ui reads active_tab() for rendering; main loop calls resize_active()
//           when panel dimensions change. Tab commands in app.rs call lifecycle methods.

use std::io::Read;
use std::path::Path;
use std::sync::mpsc::{self, Receiver, Sender};

use anyhow::{Context, Result};
use log;

use crate::tab::TerminalTab;

/// Size of the read buffer for the background PTY reader thread.
const PTY_READ_BUF_SIZE: usize = 4096;

/// Maximum number of terminal tabs allowed.
const MAX_TERMINAL_TABS: usize = 10;

/// Events sent from background reader threads to the main thread.
///
/// Each event is tagged with a tab ID so output is routed to the correct tab,
/// even when multiple PTY reader threads share the same channel.
pub enum TermEvent {
    /// Raw output bytes from the PTY, tagged with the tab ID.
    Output(usize, Vec<u8>),
    /// The child process for the given tab has exited.
    ChildExited(usize),
}

/// Manages terminal tabs and their background I/O threads.
pub struct TerminalManager {
    tabs: Vec<TerminalTab>,
    active: usize,
    event_rx: Receiver<TermEvent>,
    event_tx: Sender<TermEvent>,
    /// Monotonically increasing counter for assigning unique tab IDs.
    next_tab_id: usize,
    /// Maps from tab ID to index in the `tabs` vec. Rebuilt on mutation.
    tab_ids: Vec<usize>,
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
            next_tab_id: 0,
            tab_ids: Vec::new(),
        }
    }

    /// Spawns a new terminal tab with the user's default shell.
    ///
    /// Returns the index of the new tab. Fails if the maximum number of tabs is reached.
    pub fn spawn_tab(&mut self, cols: u16, rows: u16, cwd: &Path) -> Result<usize> {
        if self.tabs.len() >= MAX_TERMINAL_TABS {
            anyhow::bail!("Maximum of {MAX_TERMINAL_TABS} terminal tabs reached");
        }

        let (tab, reader) =
            TerminalTab::new(cols, rows, cwd).context("Failed to create terminal tab")?;

        let tab_id = self.next_tab_id;
        self.next_tab_id += 1;

        self.tabs.push(tab);
        self.tab_ids.push(tab_id);

        let tx = self.event_tx.clone();
        spawn_reader_thread(reader, tx, tab_id);

        let idx = self.tabs.len() - 1;
        Ok(idx)
    }

    /// Spawns a new terminal tab with the user's default shell (convenience wrapper).
    ///
    /// Sets the new tab as active.
    pub fn spawn_default_tab(&mut self, cols: u16, rows: u16, cwd: &Path) -> Result<()> {
        let idx = self.spawn_tab(cols, rows, cwd)?;
        self.active = idx;
        Ok(())
    }

    /// Closes the tab at the given index, killing its child process.
    ///
    /// Adjusts the active index if needed (clamps to len-1, or 0 if empty).
    pub fn close_tab(&mut self, idx: usize) -> Result<()> {
        if idx >= self.tabs.len() {
            anyhow::bail!("Tab index {idx} out of range");
        }

        let mut tab = self.tabs.remove(idx);
        self.tab_ids.remove(idx);

        // Best-effort kill — the process may have already exited.
        if let Err(e) = tab.kill() {
            log::warn!("Failed to kill terminal tab {idx}: {e}");
        }

        if self.tabs.is_empty() {
            self.active = 0;
        } else if self.active >= self.tabs.len() {
            self.active = self.tabs.len() - 1;
        }

        Ok(())
    }

    /// Sets the active tab index (with bounds check).
    pub fn activate_tab(&mut self, idx: usize) {
        if idx < self.tabs.len() {
            self.active = idx;
        }
    }

    /// Switches to the next tab (wraps around). No-op if empty.
    pub fn next_tab(&mut self) {
        if !self.tabs.is_empty() {
            self.active = (self.active + 1) % self.tabs.len();
        }
    }

    /// Switches to the previous tab (wraps around). No-op if empty.
    pub fn prev_tab(&mut self) {
        if !self.tabs.is_empty() {
            self.active = (self.active + self.tabs.len() - 1) % self.tabs.len();
        }
    }

    /// Returns the active tab index.
    pub fn active_index(&self) -> usize {
        self.active
    }

    /// Returns whether there are any tabs.
    pub fn has_tabs(&self) -> bool {
        !self.tabs.is_empty()
    }

    /// Returns tab titles for rendering the tab bar.
    pub fn tab_titles(&self) -> Vec<&str> {
        self.tabs.iter().map(|t| t.title()).collect()
    }

    /// Returns the tab index at the given x offset within the tab bar, if any.
    ///
    /// Tab labels are formatted as `[N:title]` separated by spaces.
    pub fn tab_at_x_offset(&self, x: usize) -> Option<usize> {
        let mut pos = 0;
        for (i, tab) in self.tabs.iter().enumerate() {
            let label_width = format!("[{}:{}]", i + 1, tab.title()).len();
            if x >= pos && x < pos + label_width {
                return Some(i);
            }
            pos += label_width + 1; // +1 for space separator
        }
        None
    }

    /// Drains pending PTY output from the channel and routes it to the correct tab by ID.
    ///
    /// Returns the indices of tabs whose child processes have exited (sorted descending
    /// so the caller can safely close them back-to-front without index invalidation).
    pub fn poll_output(&mut self) -> Vec<usize> {
        let mut exited_indices = Vec::new();

        while let Ok(event) = self.event_rx.try_recv() {
            match event {
                TermEvent::Output(tab_id, data) => {
                    if let Some(idx) = self.tab_ids.iter().position(|&id| id == tab_id) {
                        if let Some(tab) = self.tabs.get_mut(idx) {
                            tab.process_output(&data);
                        }
                    }
                }
                TermEvent::ChildExited(tab_id) => {
                    log::info!("Terminal child process exited (tab_id={tab_id})");
                    if let Some(idx) = self.tab_ids.iter().position(|&id| id == tab_id) {
                        if !exited_indices.contains(&idx) {
                            exited_indices.push(idx);
                        }
                    }
                }
            }
        }

        // Sort descending so caller can close back-to-front safely.
        exited_indices.sort_unstable_by(|a, b| b.cmp(a));
        exited_indices
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

    /// Provides access to the event sender for testing.
    #[cfg(test)]
    fn send_event(&self, event: TermEvent) {
        let _ = self.event_tx.send(event);
    }
}

/// Spawns a background thread that reads from the PTY and sends tagged output events.
fn spawn_reader_thread(mut reader: Box<dyn Read + Send>, tx: Sender<TermEvent>, tab_id: usize) {
    std::thread::spawn(move || {
        let mut buf = [0u8; PTY_READ_BUF_SIZE];
        loop {
            match reader.read(&mut buf) {
                Ok(0) | Err(_) => {
                    let _ = tx.send(TermEvent::ChildExited(tab_id));
                    break;
                }
                Ok(n) => {
                    if tx
                        .send(TermEvent::Output(tab_id, buf[..n].to_vec()))
                        .is_err()
                    {
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
        assert!(!mgr.has_tabs());
    }

    #[test]
    fn spawn_tab_returns_index() {
        let mut mgr = TerminalManager::new();
        let cwd = std::env::current_dir().unwrap();
        let idx0 = mgr.spawn_tab(80, 24, &cwd).unwrap();
        assert_eq!(idx0, 0);
        let idx1 = mgr.spawn_tab(80, 24, &cwd).unwrap();
        assert_eq!(idx1, 1);
        assert_eq!(mgr.tab_count(), 2);
        assert!(mgr.has_tabs());
    }

    #[test]
    fn spawn_tab_max_limit() {
        let mut mgr = TerminalManager::new();
        let cwd = std::env::current_dir().unwrap();
        for _ in 0..MAX_TERMINAL_TABS {
            mgr.spawn_tab(80, 24, &cwd).unwrap();
        }
        let result = mgr.spawn_tab(80, 24, &cwd);
        assert!(result.is_err(), "11th tab should fail");
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
    fn close_tab_removes_from_vec() {
        let mut mgr = TerminalManager::new();
        let cwd = std::env::current_dir().unwrap();
        mgr.spawn_tab(80, 24, &cwd).unwrap();
        mgr.spawn_tab(80, 24, &cwd).unwrap();
        assert_eq!(mgr.tab_count(), 2);

        mgr.close_tab(0).unwrap();
        assert_eq!(mgr.tab_count(), 1);
    }

    #[test]
    fn close_tab_adjusts_active() {
        let mut mgr = TerminalManager::new();
        let cwd = std::env::current_dir().unwrap();
        mgr.spawn_tab(80, 24, &cwd).unwrap();
        mgr.spawn_tab(80, 24, &cwd).unwrap();
        mgr.activate_tab(1);
        assert_eq!(mgr.active_index(), 1);

        mgr.close_tab(1).unwrap();
        assert_eq!(
            mgr.active_index(),
            0,
            "active should clamp down after closing last tab"
        );
    }

    #[test]
    fn close_last_tab_leaves_empty() {
        let mut mgr = TerminalManager::new();
        let cwd = std::env::current_dir().unwrap();
        mgr.spawn_tab(80, 24, &cwd).unwrap();
        mgr.close_tab(0).unwrap();
        assert_eq!(mgr.tab_count(), 0);
        assert!(!mgr.has_tabs());
    }

    #[test]
    fn next_tab_wraps() {
        let mut mgr = TerminalManager::new();
        let cwd = std::env::current_dir().unwrap();
        mgr.spawn_tab(80, 24, &cwd).unwrap();
        mgr.spawn_tab(80, 24, &cwd).unwrap();
        mgr.spawn_tab(80, 24, &cwd).unwrap();

        mgr.activate_tab(2);
        mgr.next_tab();
        assert_eq!(mgr.active_index(), 0, "should wrap from last to first");
    }

    #[test]
    fn prev_tab_wraps() {
        let mut mgr = TerminalManager::new();
        let cwd = std::env::current_dir().unwrap();
        mgr.spawn_tab(80, 24, &cwd).unwrap();
        mgr.spawn_tab(80, 24, &cwd).unwrap();
        mgr.spawn_tab(80, 24, &cwd).unwrap();

        mgr.activate_tab(0);
        mgr.prev_tab();
        assert_eq!(mgr.active_index(), 2, "should wrap from first to last");
    }

    #[test]
    fn activate_tab_sets_active() {
        let mut mgr = TerminalManager::new();
        let cwd = std::env::current_dir().unwrap();
        mgr.spawn_tab(80, 24, &cwd).unwrap();
        mgr.spawn_tab(80, 24, &cwd).unwrap();

        mgr.activate_tab(1);
        assert_eq!(mgr.active_index(), 1);

        // Out of bounds — should not change.
        mgr.activate_tab(99);
        assert_eq!(mgr.active_index(), 1);
    }

    #[test]
    fn tab_titles_returns_all() {
        let mut mgr = TerminalManager::new();
        let cwd = std::env::current_dir().unwrap();
        mgr.spawn_tab(80, 24, &cwd).unwrap();
        mgr.spawn_tab(80, 24, &cwd).unwrap();

        let titles = mgr.tab_titles();
        assert_eq!(titles.len(), 2);
        assert!(!titles[0].is_empty());
        assert!(!titles[1].is_empty());
    }

    #[test]
    fn poll_output_routes_to_correct_tab() {
        let mut mgr = TerminalManager::new();
        let cwd = std::env::current_dir().unwrap();
        mgr.spawn_tab(80, 24, &cwd).unwrap();
        mgr.spawn_tab(80, 24, &cwd).unwrap();

        // tab_ids[0] is the ID of the first tab, tab_ids[1] is the second.
        let tab0_id = mgr.tab_ids[0];
        let tab1_id = mgr.tab_ids[1];

        // Send data tagged for tab 1 (second tab).
        mgr.send_event(TermEvent::Output(tab1_id, b"world".to_vec()));
        // Send data tagged for tab 0 (first tab).
        mgr.send_event(TermEvent::Output(tab0_id, b"hello".to_vec()));
        mgr.poll_output();

        // Verify tab 0 got "hello".
        let tab0 = &mgr.tabs[0];
        let grid0 = tab0.term().grid();
        let mut text0 = String::new();
        for col in 0..5 {
            let cell =
                &grid0[alacritty_terminal::index::Line(0)][alacritty_terminal::index::Column(col)];
            text0.push(cell.c);
        }
        assert_eq!(text0, "hello");

        // Verify tab 1 got "world".
        let tab1 = &mgr.tabs[1];
        let grid1 = tab1.term().grid();
        let mut text1 = String::new();
        for col in 0..5 {
            let cell =
                &grid1[alacritty_terminal::index::Line(0)][alacritty_terminal::index::Column(col)];
            text1.push(cell.c);
        }
        assert_eq!(text1, "world");
    }

    #[test]
    fn poll_output_processes_channel_data() {
        let mut mgr = TerminalManager::new();
        mgr.spawn_default_tab(80, 24, &std::env::current_dir().unwrap())
            .unwrap();

        // Manually send test data through the channel tagged with tab 0's ID.
        let tab_id = mgr.tab_ids[0];
        mgr.send_event(TermEvent::Output(tab_id, b"test".to_vec()));
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

    #[test]
    fn next_tab_noop_when_empty() {
        let mut mgr = TerminalManager::new();
        mgr.next_tab(); // Should not panic.
        assert_eq!(mgr.active_index(), 0);
    }

    #[test]
    fn prev_tab_noop_when_empty() {
        let mut mgr = TerminalManager::new();
        mgr.prev_tab(); // Should not panic.
        assert_eq!(mgr.active_index(), 0);
    }

    #[test]
    fn poll_output_returns_exited_tab_indices() {
        let mut mgr = TerminalManager::new();
        let cwd = std::env::current_dir().unwrap();
        mgr.spawn_tab(80, 24, &cwd).unwrap();
        mgr.spawn_tab(80, 24, &cwd).unwrap();
        mgr.spawn_tab(80, 24, &cwd).unwrap();

        let tab1_id = mgr.tab_ids[1];
        mgr.send_event(TermEvent::ChildExited(tab1_id));

        let exited = mgr.poll_output();
        assert_eq!(exited, vec![1], "should report index 1 as exited");
    }

    #[test]
    fn poll_output_returns_exited_sorted_descending() {
        let mut mgr = TerminalManager::new();
        let cwd = std::env::current_dir().unwrap();
        mgr.spawn_tab(80, 24, &cwd).unwrap();
        mgr.spawn_tab(80, 24, &cwd).unwrap();
        mgr.spawn_tab(80, 24, &cwd).unwrap();

        let tab0_id = mgr.tab_ids[0];
        let tab2_id = mgr.tab_ids[2];
        mgr.send_event(TermEvent::ChildExited(tab0_id));
        mgr.send_event(TermEvent::ChildExited(tab2_id));

        let exited = mgr.poll_output();
        assert_eq!(exited, vec![2, 0], "should be sorted descending");
    }

    #[test]
    fn poll_output_no_exits_returns_empty() {
        let mut mgr = TerminalManager::new();
        let cwd = std::env::current_dir().unwrap();
        mgr.spawn_tab(80, 24, &cwd).unwrap();

        let tab_id = mgr.tab_ids[0];
        mgr.send_event(TermEvent::Output(tab_id, b"data".to_vec()));

        let exited = mgr.poll_output();
        assert!(exited.is_empty());
    }

    #[test]
    fn tab_at_x_offset_returns_correct_tab() {
        let mut mgr = TerminalManager::new();
        let cwd = std::env::current_dir().unwrap();
        mgr.spawn_tab(80, 24, &cwd).unwrap();
        mgr.spawn_tab(80, 24, &cwd).unwrap();

        // First tab label starts at offset 0: "[1:title]"
        assert_eq!(mgr.tab_at_x_offset(0), Some(0));
        assert_eq!(mgr.tab_at_x_offset(1), Some(0));

        // After first label + space, the second tab label starts.
        let first_label_len = format!("[1:{}]", mgr.tabs[0].title()).len();
        let second_start = first_label_len + 1;
        assert_eq!(mgr.tab_at_x_offset(second_start), Some(1));
    }

    #[test]
    fn tab_at_x_offset_returns_none_past_end() {
        let mut mgr = TerminalManager::new();
        let cwd = std::env::current_dir().unwrap();
        mgr.spawn_tab(80, 24, &cwd).unwrap();

        let label_len = format!("[1:{}]", mgr.tabs[0].title()).len();
        assert_eq!(mgr.tab_at_x_offset(label_len + 10), None);
    }

    #[test]
    fn tab_at_x_offset_empty_tabs() {
        let mgr = TerminalManager::new();
        assert_eq!(mgr.tab_at_x_offset(0), None);
    }
}
