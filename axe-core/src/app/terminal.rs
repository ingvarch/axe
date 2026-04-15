use std::path::PathBuf;

use alacritty_terminal::index::{Column, Line, Point};
use crossterm::event::KeyEvent;

use super::{AppState, FocusTarget, PasswordDialog};

impl AppState {
    /// Polls terminal output from the PTY background thread and feeds it to the terminal.
    ///
    /// Automatically closes tabs whose child processes have exited (e.g. user typed `exit`
    /// or pressed Ctrl+D in the shell). Updates focus accordingly.
    pub fn poll_terminal(&mut self) {
        if let Some(ref mut mgr) = self.terminal_manager {
            let (had_output, exited) = mgr.poll_output();

            // Flag that PTY output was received this frame. The main loop
            // uses this to "poison" ratatui's front buffer after draw(),
            // forcing the NEXT frame's diff to resend all cells. This catches
            // any cells the real terminal missed during rapid output (e.g.
            // alternate screen exit, fast scrolling) without sending ESC[2J
            // (which would cause visible flicker).
            if had_output {
                self.terminal_output_this_frame = true;
            }

            if !exited.is_empty() {
                // Close exited tabs back-to-front (indices are sorted descending).
                for idx in exited {
                    if let Err(e) = mgr.close_tab(idx) {
                        log::warn!("Failed to close exited terminal tab {idx}: {e}");
                    }
                }

                if mgr.has_tabs() {
                    // Still have tabs — sync focus to the new active tab.
                    if matches!(self.focus, FocusTarget::Terminal(_)) {
                        self.focus = FocusTarget::Terminal(mgr.active_index());
                    }
                } else {
                    // Last tab closed — hide terminal panel, move focus to editor.
                    self.show_terminal = false;
                    if matches!(self.focus, FocusTarget::Terminal(_)) {
                        self.focus = FocusTarget::Editor;
                    }
                }
            }

            // Check if any SSH tab needs a password and show the dialog.
            if self.password_dialog.is_none() {
                for (idx, tab) in mgr.tabs_ref().iter().enumerate() {
                    if let axe_terminal::ManagedTab::Ssh(ref ssh_tab) = tab {
                        if ssh_tab.state == axe_terminal::ssh_tab::SshConnectionState::NeedsPassword
                        {
                            self.password_dialog =
                                Some(PasswordDialog::new(ssh_tab.title().to_string(), idx));
                            break;
                        }
                    }
                }
            }
        }
    }

    /// Drains results from the background project search thread.
    ///
    /// Call this each frame from the main loop to progressively populate results.
    pub fn drain_project_search_results(&mut self) {
        if let Some(ref mut ps) = self.project_search {
            ps.drain_results();
        }
    }

    /// Converts a key event to bytes and writes them to the active terminal PTY.
    ///
    /// Reads the application cursor mode from the terminal state to produce the
    /// correct escape sequences for arrow keys.
    pub(super) fn write_terminal_input(&mut self, key: &KeyEvent) {
        let app_cursor = self
            .terminal_manager
            .as_ref()
            .and_then(|mgr| mgr.active_tab())
            .map(|tab| {
                tab.term()
                    .mode()
                    .contains(alacritty_terminal::term::TermMode::APP_CURSOR)
            })
            .unwrap_or(false);

        if let Some(bytes) = axe_terminal::input::key_to_bytes(key, app_cursor) {
            if let Some(ref mut mgr) = self.terminal_manager {
                if let Err(e) = mgr.write_to_active(&bytes) {
                    log::warn!("Failed to write to terminal: {e}");
                }
            }
        }
    }

    /// Spawns an SSH terminal tab for the given host and focuses it.
    pub(super) fn spawn_ssh_tab(&mut self, host: crate::ssh_host::SshHost) {
        if !self.show_terminal {
            self.show_terminal = true;
        }

        let params = axe_terminal::ssh_connect::SshConnectParams {
            hostname: host.hostname.clone(),
            port: host.port,
            user: host.user.clone(),
            identity_file: host.identity_file.clone(),
            cols: self.last_terminal_cols,
            rows: self.last_terminal_rows,
            tab_id: 0, // Will be overwritten by manager.
            connect_timeout_secs: self.config.ssh.connect_timeout,
        };

        if let Some(ref mut mgr) = self.terminal_manager {
            match mgr.spawn_ssh_tab(self.last_terminal_cols, self.last_terminal_rows, params) {
                Ok(idx) => {
                    mgr.activate_tab(idx);
                    self.focus = FocusTarget::Terminal(idx);
                }
                Err(e) => log::warn!("Failed to create SSH tab: {e}"),
            }
        } else {
            let mut mgr = axe_terminal::TerminalManager::new();
            match mgr.spawn_ssh_tab(self.last_terminal_cols, self.last_terminal_rows, params) {
                Ok(idx) => {
                    mgr.activate_tab(idx);
                    self.focus = FocusTarget::Terminal(idx);
                    self.terminal_manager = Some(mgr);
                }
                Err(e) => log::warn!("Failed to create SSH tab: {e}"),
            }
        }
    }

    /// Returns whether the active terminal tab is a disconnected SSH tab.
    pub(super) fn is_active_ssh_tab_disconnected(&self) -> bool {
        let Some(ref mgr) = self.terminal_manager else {
            return false;
        };
        matches!(
            mgr.active_tab(),
            Some(axe_terminal::ManagedTab::Ssh(ref tab))
                if matches!(tab.state, axe_terminal::ssh_tab::SshConnectionState::Disconnected(_))
        )
    }

    /// Sends a password to an SSH tab for authentication.
    pub(super) fn send_ssh_password(&mut self, tab_idx: usize, password: String) {
        if let Some(ref mut mgr) = self.terminal_manager {
            if let Some(axe_terminal::ManagedTab::Ssh(ref tab)) = mgr.tabs_ref().get(tab_idx) {
                tab.send_password(password);
            }
        }
    }

    /// Opens the SSH Host Finder overlay by parsing SSH configs and creating the finder.
    pub(super) fn open_ssh_host_finder(&mut self) {
        let ssh_config_path = crate::ssh_host::default_ssh_config_path();
        let ssh_hosts = ssh_config_path
            .as_deref()
            .map(crate::ssh_host::parse_ssh_config)
            .unwrap_or_default();
        let axe_hosts = crate::ssh_host::hosts_from_axe_config(&self.config);
        let merged = crate::ssh_host::merge_hosts(ssh_hosts, axe_hosts);
        self.ssh_host_finder = Some(crate::ssh_host_finder::SshHostFinder::new(merged));
    }

    /// Creates a new terminal tab and focuses it.
    ///
    /// No-op if the terminal panel is hidden — the user should toggle the panel first.
    pub(super) fn new_terminal_tab(&mut self) {
        if !self.show_terminal {
            return;
        }

        let cwd = self
            .project_root
            .clone()
            .unwrap_or_else(|| PathBuf::from("."));

        let shell = if self.config.terminal.shell.is_empty() {
            None
        } else {
            Some(self.config.terminal.shell.as_str())
        };

        if let Some(ref mut mgr) = self.terminal_manager {
            match mgr.spawn_tab_with_shell(
                self.last_terminal_cols,
                self.last_terminal_rows,
                &cwd,
                shell,
            ) {
                Ok(idx) => {
                    mgr.activate_tab(idx);
                    self.focus = FocusTarget::Terminal(idx);
                }
                Err(e) => {
                    log::warn!("Failed to create terminal tab: {e}");
                }
            }
        } else {
            // No manager yet — create one with a first tab.
            let mut mgr = axe_terminal::TerminalManager::new();
            match mgr.spawn_tab_with_shell(
                self.last_terminal_cols,
                self.last_terminal_rows,
                &cwd,
                shell,
            ) {
                Ok(idx) => {
                    mgr.activate_tab(idx);
                    self.focus = FocusTarget::Terminal(idx);
                    self.terminal_manager = Some(mgr);
                }
                Err(e) => {
                    log::warn!("Failed to create terminal tab: {e}");
                }
            }
        }
    }

    /// Closes the active terminal tab.
    pub(super) fn close_terminal_tab(&mut self) {
        if let Some(ref mut mgr) = self.terminal_manager {
            let active = mgr.active_index();
            if let Err(e) = mgr.close_tab(active) {
                log::warn!("Failed to close terminal tab: {e}");
                return;
            }
            if mgr.has_tabs() {
                self.focus = FocusTarget::Terminal(mgr.active_index());
            } else {
                self.show_terminal = false;
                if matches!(self.focus, FocusTarget::Terminal(_)) {
                    self.focus = FocusTarget::Editor;
                }
            }
        }
    }

    /// Scrolls the active terminal tab by the given amount.
    pub(super) fn terminal_scroll(&mut self, scroll: alacritty_terminal::grid::Scroll) {
        if let Some(ref mut mgr) = self.terminal_manager {
            mgr.scroll_active(scroll);
        }
    }

    /// Switches to the next terminal tab, wrapping from last to first.
    pub(super) fn next_terminal_tab(&mut self) {
        if let Some(ref mut mgr) = self.terminal_manager {
            let count = mgr.tab_count();
            if count > 0 {
                let next = (mgr.active_index() + 1) % count;
                mgr.activate_tab(next);
                self.focus = FocusTarget::Terminal(next);
            }
        }
    }

    /// Switches to the previous terminal tab, wrapping from first to last.
    pub(super) fn prev_terminal_tab(&mut self) {
        if let Some(ref mut mgr) = self.terminal_manager {
            let count = mgr.tab_count();
            if count > 0 {
                let prev = if mgr.active_index() == 0 {
                    count - 1
                } else {
                    mgr.active_index() - 1
                };
                mgr.activate_tab(prev);
                self.focus = FocusTarget::Terminal(prev);
            }
        }
    }

    /// Activates a specific terminal tab by index. No-op if the index is out of range.
    pub(super) fn activate_terminal_tab(&mut self, idx: usize) {
        if let Some(ref mut mgr) = self.terminal_manager {
            if idx < mgr.tab_count() {
                mgr.activate_tab(idx);
                self.focus = FocusTarget::Terminal(idx);
            }
        }
    }

    /// Checks if a click landed on the terminal tab bar and returns the hit target.
    ///
    /// The tab bar is the first row inside the terminal panel border.
    /// Returns `None` if the click is outside the tab bar or if there's no terminal manager.
    pub(super) fn tab_bar_hit(&self, col: u16, row: u16) -> Option<axe_terminal::TabBarHit> {
        let mgr = self.terminal_manager.as_ref()?;
        if !mgr.has_tabs() {
            return None;
        }
        let (tx, ty, tw, _th) = self.terminal_tab_bar_area?;
        if row != ty || col < tx || col >= tx + tw {
            return None;
        }
        mgr.tab_bar_hit_at_x((col - tx) as usize)
    }

    // IMPACT ANALYSIS — screen_to_terminal_point
    // Parents: handle_mouse_event() calls this for Down and Drag events in the terminal grid.
    // Children: None — pure conversion function returning Option<Point>.
    // Siblings: terminal_grid_area (must be set by main.rs each frame),
    //           terminal_manager.active_display_offset() (must reflect current scroll state).
    // Risk: None — stateless helper, cannot corrupt state.

    /// Converts screen coordinates to a terminal grid Point.
    ///
    /// Returns `None` if the coordinates are outside the terminal grid area
    /// or if no grid area has been set.
    pub(super) fn screen_to_terminal_point(&self, col: u16, row: u16) -> Option<Point> {
        let (gx, gy, gw, gh) = self.terminal_grid_area?;
        if col < gx || col >= gx + gw || row < gy || row >= gy + gh {
            return None;
        }
        let grid_col = (col - gx) as usize;
        let grid_row = (row - gy) as i32;
        let display_offset = self
            .terminal_manager
            .as_ref()
            .map(|mgr| mgr.active_display_offset())
            .unwrap_or(0) as i32;
        Some(Point::new(
            Line(grid_row - display_offset),
            Column(grid_col),
        ))
    }

    // IMPACT ANALYSIS — screen_to_ai_overlay_point
    // Parents: handle_mouse_event() calls this for down/drag events when the
    //          AI overlay is visible and a session exists.
    // Children: None — pure conversion function returning Option<Point>.
    // Siblings: ai_overlay_grid_area (written each frame by render_ai_overlay),
    //           ai_overlay.session.tab.display_offset() (current scroll).
    // Risk: None — stateless helper. Returns None cleanly if the overlay is
    //       hidden, the grid area has not been set yet (first frame after
    //       show), or the click falls outside the overlay rect.

    /// Converts screen coordinates to an AI overlay grid `Point`.
    ///
    /// Returns `None` if the overlay is not visible, no session exists, the
    /// grid area has not been set yet this frame, or the coordinates fall
    /// outside the overlay's inner rectangle.
    pub(super) fn screen_to_ai_overlay_point(&self, col: u16, row: u16) -> Option<Point> {
        if !self.ai_overlay.visible {
            return None;
        }
        let session = self.ai_overlay.session.as_ref()?;
        let (gx, gy, gw, gh) = self.ai_overlay_grid_area?;
        if col < gx || col >= gx + gw || row < gy || row >= gy + gh {
            return None;
        }
        let grid_col = (col - gx) as usize;
        let grid_row = (row - gy) as i32;
        let display_offset = session.tab.display_offset() as i32;
        Some(Point::new(
            Line(grid_row - display_offset),
            Column(grid_col),
        ))
    }

    /// Scrolls the AI overlay's active session by the given amount.
    pub(super) fn ai_overlay_scroll(&mut self, scroll: alacritty_terminal::grid::Scroll) {
        if let Some(session) = self.ai_overlay.session.as_mut() {
            session.tab.scroll(scroll);
        }
    }
}
