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

use alacritty_terminal::grid::Scroll;
use alacritty_terminal::index::{Direction, Point};
use alacritty_terminal::selection::SelectionType;
use alacritty_terminal::Term;

use crate::PtyEventListener;

use crate::ssh_tab::{SshConnectionState, SshTerminalTab};
use crate::tab::TerminalTab;

/// Result of a hit test on the terminal tab bar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TabBarHit {
    /// A click on a tab at the given index.
    Tab(usize),
    /// A click on the [+] button.
    PlusButton,
}

/// A terminal tab that can be either a local PTY or a remote SSH session.
pub enum ManagedTab {
    /// A local terminal tab backed by a PTY.
    Local(TerminalTab),
    /// A remote terminal tab backed by an SSH connection.
    Ssh(SshTerminalTab),
}

impl ManagedTab {
    pub fn title(&self) -> &str {
        match self {
            Self::Local(tab) => tab.title(),
            Self::Ssh(tab) => tab.title(),
        }
    }

    pub fn write(&mut self, data: &[u8]) -> Result<()> {
        match self {
            Self::Local(tab) => tab.write(data),
            Self::Ssh(tab) => tab.write(data),
        }
    }

    pub fn process_output(&mut self, data: &[u8]) {
        match self {
            Self::Local(tab) => tab.process_output(data),
            Self::Ssh(tab) => tab.process_output(data),
        }
    }

    pub fn resize(&mut self, cols: u16, rows: u16) -> Result<()> {
        match self {
            Self::Local(tab) => tab.resize(cols, rows),
            Self::Ssh(tab) => tab.resize(cols, rows),
        }
    }

    pub fn term(&self) -> &Term<PtyEventListener> {
        match self {
            Self::Local(tab) => tab.term(),
            Self::Ssh(tab) => tab.term(),
        }
    }

    pub fn term_mut(&mut self) -> &mut Term<PtyEventListener> {
        match self {
            Self::Local(tab) => tab.term_mut(),
            Self::Ssh(tab) => tab.term_mut(),
        }
    }

    pub fn kill(&mut self) -> Result<()> {
        match self {
            Self::Local(tab) => tab.kill(),
            Self::Ssh(tab) => tab.kill(),
        }
    }

    pub fn scroll(&mut self, scroll: Scroll) {
        match self {
            Self::Local(tab) => tab.scroll(scroll),
            Self::Ssh(tab) => tab.scroll(scroll),
        }
    }

    pub fn display_offset(&self) -> usize {
        match self {
            Self::Local(tab) => tab.display_offset(),
            Self::Ssh(tab) => tab.display_offset(),
        }
    }

    pub fn is_alive(&mut self) -> bool {
        match self {
            Self::Local(tab) => tab.is_alive(),
            Self::Ssh(tab) => tab.is_alive(),
        }
    }

    pub fn start_selection(&mut self, ty: SelectionType, point: Point, side: Direction) {
        match self {
            Self::Local(tab) => tab.start_selection(ty, point, side),
            Self::Ssh(tab) => tab.start_selection(ty, point, side),
        }
    }

    pub fn update_selection(&mut self, point: Point, side: Direction) {
        match self {
            Self::Local(tab) => tab.update_selection(point, side),
            Self::Ssh(tab) => tab.update_selection(point, side),
        }
    }

    pub fn selection_to_string(&self) -> Option<String> {
        match self {
            Self::Local(tab) => tab.selection_to_string(),
            Self::Ssh(tab) => tab.selection_to_string(),
        }
    }

    pub fn clear_selection(&mut self) {
        match self {
            Self::Local(tab) => tab.clear_selection(),
            Self::Ssh(tab) => tab.clear_selection(),
        }
    }

    pub fn has_selection(&self) -> bool {
        match self {
            Self::Local(tab) => tab.has_selection(),
            Self::Ssh(tab) => tab.has_selection(),
        }
    }
}

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
    /// SSH connection established successfully.
    SshConnected(usize),
    /// SSH authentication requires a password (agent + key auth failed).
    SshNeedsPassword(usize),
    /// SSH connection or auth error.
    SshError(usize, String),
}

/// Manages terminal tabs and their background I/O threads.
pub struct TerminalManager {
    tabs: Vec<ManagedTab>,
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
        self.spawn_tab_with_shell(cols, rows, cwd, None)
    }

    /// Spawns a new terminal tab, optionally overriding the detected shell.
    ///
    /// If `shell` is `None` or an empty string, falls back to the detected shell.
    /// Returns the index of the new tab. Fails if the maximum number of tabs is reached.
    pub fn spawn_tab_with_shell(
        &mut self,
        cols: u16,
        rows: u16,
        cwd: &Path,
        shell: Option<&str>,
    ) -> Result<usize> {
        if self.tabs.len() >= MAX_TERMINAL_TABS {
            anyhow::bail!("Maximum of {MAX_TERMINAL_TABS} terminal tabs reached");
        }

        let (tab, reader) = TerminalTab::new_with_shell(cols, rows, cwd, shell)
            .context("Failed to create terminal tab")?;

        let tab_id = self.next_tab_id;
        self.next_tab_id += 1;

        self.tabs.push(ManagedTab::Local(tab));
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
        self.spawn_default_tab_with_shell(cols, rows, cwd, None)
    }

    /// Spawns a new terminal tab with an optional shell override (convenience wrapper).
    ///
    /// Sets the new tab as active.
    pub fn spawn_default_tab_with_shell(
        &mut self,
        cols: u16,
        rows: u16,
        cwd: &Path,
        shell: Option<&str>,
    ) -> Result<()> {
        let idx = self.spawn_tab_with_shell(cols, rows, cwd, shell)?;
        self.active = idx;
        Ok(())
    }

    /// Spawns an SSH terminal tab and starts the async connection task.
    ///
    /// Returns the index of the new tab. The tab starts in `Connecting` state.
    pub fn spawn_ssh_tab(
        &mut self,
        cols: u16,
        rows: u16,
        params: crate::ssh_connect::SshConnectParams,
    ) -> Result<usize> {
        if self.tabs.len() >= MAX_TERMINAL_TABS {
            anyhow::bail!("Maximum of {MAX_TERMINAL_TABS} terminal tabs reached");
        }

        let tab_id = self.next_tab_id;
        self.next_tab_id += 1;

        let connect_params = crate::ssh_connect::SshConnectParams { tab_id, ..params };

        let (tab, input_rx) =
            SshTerminalTab::new(cols, rows, &connect_params.user, &connect_params.hostname);

        self.tabs.push(ManagedTab::Ssh(tab));
        self.tab_ids.push(tab_id);

        let tx = self.event_tx.clone();
        crate::ssh_connect::spawn_ssh_task(connect_params, tx, input_rx);

        let idx = self.tabs.len() - 1;
        Ok(idx)
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

    /// Returns whether the tab limit has been reached.
    pub fn is_at_tab_limit(&self) -> bool {
        self.tabs.len() >= MAX_TERMINAL_TABS
    }

    /// Returns the tab index at the given x offset within the tab bar, if any.
    ///
    /// Tab labels are formatted as `[N:title]` separated by spaces.
    pub fn tab_at_x_offset(&self, x: usize) -> Option<usize> {
        match self.tab_bar_hit_at_x(x) {
            Some(TabBarHit::Tab(i)) => Some(i),
            _ => None,
        }
    }

    /// Hit-tests the tab bar at the given x offset.
    ///
    /// Returns `Tab(i)` for a tab click, `PlusButton` for the `[+]` button,
    /// or `None` if the click is outside all elements.
    pub fn tab_bar_hit_at_x(&self, x: usize) -> Option<TabBarHit> {
        let mut pos = 0;
        for (i, tab) in self.tabs.iter().enumerate() {
            let label_width = format!("[{}:{}]", i + 1, tab.title()).len();
            if x >= pos && x < pos + label_width {
                return Some(TabBarHit::Tab(i));
            }
            pos += label_width + 1; // +1 for space separator
        }
        // [+] button follows after tabs: " [+]"
        let plus_start = pos;
        let plus_end = plus_start + "[+]".len();
        if x >= plus_start && x < plus_end {
            return Some(TabBarHit::PlusButton);
        }
        None
    }

    /// Drains pending PTY output from the channel and routes it to the correct tab by ID.
    ///
    /// Returns `(had_output, exited_indices)`:
    /// - `had_output`: true if any PTY output data was processed this call.
    /// - `exited_indices`: indices of tabs whose child processes have exited,
    ///   sorted descending so the caller can safely close them back-to-front
    ///   without index invalidation.
    pub fn poll_output(&mut self) -> (bool, Vec<usize>) {
        let mut exited_indices = Vec::new();
        let mut had_output = false;

        while let Ok(event) = self.event_rx.try_recv() {
            match event {
                TermEvent::Output(tab_id, data) => {
                    had_output = true;
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
                TermEvent::SshConnected(tab_id) => {
                    if let Some(idx) = self.tab_ids.iter().position(|&id| id == tab_id) {
                        if let Some(ManagedTab::Ssh(ref mut tab)) = self.tabs.get_mut(idx) {
                            tab.state = SshConnectionState::Connected;
                            log::info!("SSH connected (tab_id={tab_id})");
                        }
                    }
                }
                TermEvent::SshNeedsPassword(tab_id) => {
                    if let Some(idx) = self.tab_ids.iter().position(|&id| id == tab_id) {
                        if let Some(ManagedTab::Ssh(ref mut tab)) = self.tabs.get_mut(idx) {
                            tab.state = SshConnectionState::NeedsPassword;
                            log::info!("SSH needs password (tab_id={tab_id})");
                        }
                    }
                }
                TermEvent::SshError(tab_id, ref msg) => {
                    if let Some(idx) = self.tab_ids.iter().position(|&id| id == tab_id) {
                        if let Some(ManagedTab::Ssh(ref mut tab)) = self.tabs.get_mut(idx) {
                            tab.state = SshConnectionState::Disconnected(msg.clone());
                            log::warn!("SSH error (tab_id={tab_id}): {msg}");
                        }
                    }
                }
            }
        }

        // Sort descending so caller can close back-to-front safely.
        exited_indices.sort_unstable_by(|a, b| b.cmp(a));
        (had_output, exited_indices)
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
    pub fn active_tab(&self) -> Option<&ManagedTab> {
        self.tabs.get(self.active)
    }

    /// Resizes the active terminal tab's PTY and grid.
    pub fn resize_active(&mut self, cols: u16, rows: u16) -> Result<()> {
        if let Some(tab) = self.tabs.get_mut(self.active) {
            tab.resize(cols, rows)?;
        }
        Ok(())
    }

    /// Resizes all terminal tabs' PTY and grid to the given dimensions.
    ///
    /// Unlike `resize_active`, this ensures inactive tabs are also updated,
    /// so switching tabs after a resize shows correctly wrapped content.
    pub fn resize_all(&mut self, cols: u16, rows: u16) -> Result<()> {
        for tab in &mut self.tabs {
            tab.resize(cols, rows)?;
        }
        Ok(())
    }

    /// Scrolls the active terminal tab by the given amount.
    pub fn scroll_active(&mut self, scroll: Scroll) {
        if let Some(tab) = self.tabs.get_mut(self.active) {
            tab.scroll(scroll);
        }
    }

    /// Returns the display offset of the active terminal tab (0 = at bottom).
    pub fn active_display_offset(&self) -> usize {
        self.active_tab().map(|t| t.display_offset()).unwrap_or(0)
    }

    /// Returns the number of tabs.
    pub fn tab_count(&self) -> usize {
        self.tabs.len()
    }

    /// Starts a text selection on the active tab.
    ///
    /// No-op if there are no tabs.
    pub fn start_selection_active(&mut self, ty: SelectionType, point: Point, side: Direction) {
        if let Some(tab) = self.tabs.get_mut(self.active) {
            tab.start_selection(ty, point, side);
        }
    }

    /// Extends the selection on the active tab to the given point.
    ///
    /// No-op if there are no tabs or no active selection.
    pub fn update_selection_active(&mut self, point: Point, side: Direction) {
        if let Some(tab) = self.tabs.get_mut(self.active) {
            tab.update_selection(point, side);
        }
    }

    /// Returns the selected text from the active tab, if any.
    pub fn copy_selection_active(&self) -> Option<String> {
        self.active_tab()?.selection_to_string()
    }

    /// Clears the selection on the active tab.
    ///
    /// No-op if there are no tabs.
    pub fn clear_selection_active(&mut self) {
        if let Some(tab) = self.tabs.get_mut(self.active) {
            tab.clear_selection();
        }
    }

    /// Returns whether the active terminal tab's child process is still running.
    pub fn active_tab_is_alive(&mut self) -> bool {
        self.tabs
            .get_mut(self.active)
            .map(|tab| tab.is_alive())
            .unwrap_or(false)
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
        let _ = mgr.poll_output();

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
        let _ = mgr.poll_output();

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

        let (_, exited) = mgr.poll_output();
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

        let (_, exited) = mgr.poll_output();
        assert_eq!(exited, vec![2, 0], "should be sorted descending");
    }

    #[test]
    fn poll_output_no_exits_returns_empty() {
        let mut mgr = TerminalManager::new();
        let cwd = std::env::current_dir().unwrap();
        mgr.spawn_tab(80, 24, &cwd).unwrap();

        let tab_id = mgr.tab_ids[0];
        mgr.send_event(TermEvent::Output(tab_id, b"data".to_vec()));

        let (had_output, exited) = mgr.poll_output();
        assert!(had_output, "should report output was received");
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
    fn active_display_offset_zero_by_default() {
        let mut mgr = TerminalManager::new();
        assert_eq!(mgr.active_display_offset(), 0);

        let cwd = std::env::current_dir().unwrap();
        mgr.spawn_tab(80, 24, &cwd).unwrap();
        assert_eq!(mgr.active_display_offset(), 0);
    }

    #[test]
    fn scroll_active_no_tabs_is_noop() {
        let mut mgr = TerminalManager::new();
        mgr.scroll_active(Scroll::PageUp); // should not panic
    }

    #[test]
    fn scroll_active_changes_display_offset() {
        let mut mgr = TerminalManager::new();
        let cwd = std::env::current_dir().unwrap();
        mgr.spawn_tab(80, 24, &cwd).unwrap();

        // Feed output to create scrollback.
        let tab_id = mgr.tab_ids[0];
        for i in 0..100 {
            let line = format!("line {i}\r\n");
            mgr.send_event(TermEvent::Output(tab_id, line.into_bytes()));
        }
        let _ = mgr.poll_output();

        mgr.scroll_active(Scroll::PageUp);
        assert!(mgr.active_display_offset() > 0);

        mgr.scroll_active(Scroll::Bottom);
        assert_eq!(mgr.active_display_offset(), 0);
    }

    #[test]
    fn tab_at_x_offset_empty_tabs() {
        let mgr = TerminalManager::new();
        assert_eq!(mgr.tab_at_x_offset(0), None);
    }

    // --- Selection delegation tests ---

    #[test]
    fn start_selection_active_sets_selection() {
        use alacritty_terminal::index::{Column, Direction, Line, Point};
        use alacritty_terminal::selection::SelectionType;

        let mut mgr = TerminalManager::new();
        let cwd = std::env::current_dir().unwrap();
        mgr.spawn_tab(80, 24, &cwd).unwrap();

        mgr.start_selection_active(
            SelectionType::Simple,
            Point::new(Line(0), Column(0)),
            Direction::Left,
        );
        assert!(
            mgr.active_tab().unwrap().has_selection(),
            "Active tab should have selection"
        );
    }

    #[test]
    fn copy_selection_active_returns_text() {
        use alacritty_terminal::index::{Column, Direction, Line, Point};
        use alacritty_terminal::selection::SelectionType;

        let mut mgr = TerminalManager::new();
        let cwd = std::env::current_dir().unwrap();
        mgr.spawn_tab(80, 24, &cwd).unwrap();

        // Feed content and select it.
        let tab_id = mgr.tab_ids[0];
        mgr.send_event(TermEvent::Output(tab_id, b"hello world".to_vec()));
        let _ = mgr.poll_output();

        mgr.start_selection_active(
            SelectionType::Simple,
            Point::new(Line(0), Column(0)),
            Direction::Left,
        );
        mgr.update_selection_active(Point::new(Line(0), Column(4)), Direction::Right);

        let text = mgr.copy_selection_active();
        assert_eq!(text, Some("hello".to_string()));
    }

    #[test]
    fn clear_selection_active_removes_selection() {
        use alacritty_terminal::index::{Column, Direction, Line, Point};
        use alacritty_terminal::selection::SelectionType;

        let mut mgr = TerminalManager::new();
        let cwd = std::env::current_dir().unwrap();
        mgr.spawn_tab(80, 24, &cwd).unwrap();

        mgr.start_selection_active(
            SelectionType::Simple,
            Point::new(Line(0), Column(0)),
            Direction::Left,
        );
        mgr.clear_selection_active();
        assert!(
            !mgr.active_tab().unwrap().has_selection(),
            "Selection should be cleared"
        );
    }

    // --- Shell override tests ---

    #[test]
    fn spawn_tab_with_shell_none_uses_default() {
        let mut mgr = TerminalManager::new();
        let cwd = std::env::current_dir().unwrap();
        let result = mgr.spawn_tab_with_shell(80, 24, &cwd, None);
        assert!(
            result.is_ok(),
            "spawn_tab_with_shell(None) should succeed: {:?}",
            result.err()
        );
        assert_eq!(mgr.tab_count(), 1);
    }

    #[test]
    fn spawn_tab_with_shell_explicit_uses_that_shell() {
        let mut mgr = TerminalManager::new();
        let cwd = std::env::current_dir().unwrap();
        let result = mgr.spawn_tab_with_shell(80, 24, &cwd, Some("/bin/sh"));
        assert!(
            result.is_ok(),
            "spawn_tab_with_shell(Some(\"/bin/sh\")) should succeed: {:?}",
            result.err()
        );
        assert_eq!(mgr.tab_count(), 1);
    }

    #[test]
    fn spawn_default_tab_with_shell_sets_active() {
        let mut mgr = TerminalManager::new();
        let cwd = std::env::current_dir().unwrap();
        let result = mgr.spawn_default_tab_with_shell(80, 24, &cwd, None);
        assert!(result.is_ok());
        assert_eq!(mgr.tab_count(), 1);
        assert!(mgr.active_tab().is_some());
    }

    #[test]
    fn active_tab_is_alive_returns_true_for_running() {
        let mut mgr = TerminalManager::new();
        let cwd = std::env::current_dir().unwrap();
        mgr.spawn_tab(80, 24, &cwd).unwrap();
        assert!(
            mgr.active_tab_is_alive(),
            "Freshly spawned tab should be alive"
        );
    }

    #[test]
    fn active_tab_is_alive_returns_false_no_tabs() {
        let mut mgr = TerminalManager::new();
        assert!(!mgr.active_tab_is_alive(), "No tabs means not alive");
    }

    #[test]
    fn resize_all_resizes_all_tabs() {
        let mut mgr = TerminalManager::new();
        let cwd = std::env::current_dir().unwrap();
        mgr.spawn_tab(80, 24, &cwd).unwrap();
        mgr.spawn_tab(80, 24, &cwd).unwrap();
        mgr.spawn_tab(80, 24, &cwd).unwrap();

        let result = mgr.resize_all(120, 40);
        assert!(
            result.is_ok(),
            "resize_all should succeed: {:?}",
            result.err()
        );
    }

    #[test]
    fn resize_all_noop_without_tabs() {
        let mut mgr = TerminalManager::new();
        let result = mgr.resize_all(120, 40);
        assert!(result.is_ok(), "resize_all with no tabs should be a no-op");
    }

    #[test]
    fn resize_all_skips_same_size() {
        let mut mgr = TerminalManager::new();
        let cwd = std::env::current_dir().unwrap();
        mgr.spawn_tab(80, 24, &cwd).unwrap();

        // Resize to same dimensions should succeed (early return in tab.resize).
        let result = mgr.resize_all(80, 24);
        assert!(result.is_ok(), "resize_all with same size should succeed");
    }

    #[test]
    fn selection_methods_noop_without_tabs() {
        use alacritty_terminal::index::{Column, Direction, Line, Point};
        use alacritty_terminal::selection::SelectionType;

        let mut mgr = TerminalManager::new();
        // These should not panic with no tabs.
        mgr.start_selection_active(
            SelectionType::Simple,
            Point::new(Line(0), Column(0)),
            Direction::Left,
        );
        mgr.update_selection_active(Point::new(Line(0), Column(5)), Direction::Right);
        mgr.clear_selection_active();
        assert_eq!(mgr.copy_selection_active(), None);
    }

    #[test]
    fn is_at_tab_limit_false_when_under() {
        let mut mgr = TerminalManager::new();
        let cwd = std::env::current_dir().unwrap();
        mgr.spawn_tab(80, 24, &cwd).unwrap();
        assert!(!mgr.is_at_tab_limit());
    }

    #[test]
    fn is_at_tab_limit_true_when_at_max() {
        let mut mgr = TerminalManager::new();
        let cwd = std::env::current_dir().unwrap();
        for _ in 0..MAX_TERMINAL_TABS {
            mgr.spawn_tab(80, 24, &cwd).unwrap();
        }
        assert!(mgr.is_at_tab_limit());
    }

    #[test]
    fn tab_bar_hit_at_x_returns_tab() {
        let mut mgr = TerminalManager::new();
        let cwd = std::env::current_dir().unwrap();
        mgr.spawn_tab(80, 24, &cwd).unwrap();

        assert_eq!(mgr.tab_bar_hit_at_x(0), Some(TabBarHit::Tab(0)));
        assert_eq!(mgr.tab_bar_hit_at_x(1), Some(TabBarHit::Tab(0)));
    }

    #[test]
    fn tab_bar_hit_at_x_returns_plus_button() {
        let mut mgr = TerminalManager::new();
        let cwd = std::env::current_dir().unwrap();
        mgr.spawn_tab(80, 24, &cwd).unwrap();

        // [+] starts after the tab label + space
        let label_len = format!("[1:{}]", mgr.tabs[0].title()).len();
        let plus_start = label_len + 1; // +1 for space separator
        assert_eq!(
            mgr.tab_bar_hit_at_x(plus_start),
            Some(TabBarHit::PlusButton)
        );
    }

    #[test]
    fn tab_bar_hit_at_x_returns_none_outside() {
        let mut mgr = TerminalManager::new();
        let cwd = std::env::current_dir().unwrap();
        mgr.spawn_tab(80, 24, &cwd).unwrap();

        let label_len = format!("[1:{}]", mgr.tabs[0].title()).len();
        // Past [+] button: label + space + "[+]" = label_len + 1 + 3
        let past_end = label_len + 1 + 3;
        assert_eq!(mgr.tab_bar_hit_at_x(past_end), None);
    }
}
