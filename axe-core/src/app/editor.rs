use std::time::{Duration, Instant};

use super::AppState;

impl AppState {
    /// Scrolls the active editor buffer vertically by the given delta lines.
    pub(super) fn editor_scroll(&mut self, delta: i32) {
        let (viewport_height, _) = self.editor_viewport();
        if let Some(buf) = self.buffer_manager.active_buffer_mut() {
            buf.scroll_by(delta, viewport_height);
        }
    }

    /// Scrolls the active editor buffer horizontally by the given delta columns.
    pub(super) fn editor_scroll_horizontal(&mut self, delta: i32) {
        if let Some(buf) = self.buffer_manager.active_buffer_mut() {
            buf.scroll_horizontally_by(delta);
        }
    }

    /// Returns `(height, width)` of the editor content area for viewport calculations.
    pub(super) fn editor_viewport(&self) -> (usize, usize) {
        self.editor_inner_area
            .map(|(_x, _y, w, h)| (h as usize, w as usize))
            .unwrap_or((20, 80))
    }

    /// Saves the active buffer to disk and notifies the LSP.
    pub(super) fn save_active_buffer(&mut self) {
        if let Some(buf) = self.buffer_manager.active_buffer_mut() {
            if let Err(e) = buf.save_to_file() {
                log::warn!("Save failed: {e}");
            }
        }
        // Notify LSP about save.
        if let Some(ref mut lsp) = self.lsp_manager {
            if let Some(buf) = self.buffer_manager.active_buffer() {
                if let Some(path) = buf.path() {
                    let path = path.to_path_buf();
                    if let Err(e) = lsp.file_saved(&path) {
                        log::warn!("LSP didSave failed: {e}");
                    }
                }
            }
        }
        self.last_edit_time = None;
        // Refresh git branch after save (branch may change after file operations).
        self.force_refresh_git_branch();
        // Recalculate git diff hunks for the saved buffer.
        self.refresh_active_buffer_diff_hunks();
        // Refresh the set of modified files for the tree panel.
        self.refresh_git_modified_files();
    }

    /// Applies the currently selected completion item, replacing the typed prefix.
    pub(super) fn apply_completion(&mut self) {
        let Some(ref comp) = self.completion else {
            return;
        };
        let Some(item) = comp.selected_item().cloned() else {
            self.completion = None;
            return;
        };
        let trigger_col = comp.trigger_col;
        let trigger_row = comp.trigger_row;
        self.completion = None;

        let insert = item.insert_text.as_deref().unwrap_or(&item.label);
        let (h, w) = self.editor_viewport();
        if let Some(buf) = self.buffer_manager.active_buffer_mut() {
            // Only apply if cursor is on the trigger row.
            if buf.cursor().row != trigger_row {
                return;
            }
            let current_col = buf.cursor().col;
            // Delete the typed prefix (from trigger_col to current cursor).
            if current_col > trigger_col {
                buf.apply_text_edit(trigger_row, trigger_col, trigger_row, current_col, insert);
            } else {
                buf.apply_text_edit(trigger_row, trigger_col, trigger_row, trigger_col, insert);
            }
            buf.ensure_cursor_visible(h, w);
        }
        self.last_edit_time = Some(Instant::now());
        self.notify_lsp_change();
    }

    /// Updates the completion filter after an edit (insert/backspace).
    ///
    /// Extracts the text between trigger_col and the current cursor as the prefix,
    /// then re-filters. Dismisses if the filter is empty or cursor moved away.
    pub(super) fn update_completion_after_edit(&mut self) {
        let Some(ref mut comp) = self.completion else {
            return;
        };
        let Some(buf) = self.buffer_manager.active_buffer() else {
            self.completion = None;
            return;
        };
        // Dismiss if cursor moved to a different row.
        if buf.cursor().row != comp.trigger_row {
            self.completion = None;
            return;
        }
        // Dismiss if cursor moved before the trigger column.
        if buf.cursor().col < comp.trigger_col {
            self.completion = None;
            return;
        }
        // Extract prefix text from trigger_col to cursor.
        let line_text = buf.line_text(comp.trigger_row);
        let prefix: String = line_text
            .chars()
            .skip(comp.trigger_col)
            .take(buf.cursor().col - comp.trigger_col)
            .collect();
        comp.update_filter(&prefix);
        // Dismiss if nothing matches.
        if comp.filtered.is_empty() {
            self.completion = None;
        }
    }

    /// Auto-triggers completion when typing certain characters (`.`, `:`).
    pub(super) fn maybe_auto_trigger_completion(&mut self, ch: char) {
        if self.completion.is_some() {
            return;
        }
        if ch == '.' || ch == ':' {
            self.request_completion();
        }
    }

    // IMPACT ANALYSIS — editor_tab_index_at_col
    // Parents: handle_mouse_event() calls this when a click lands on the editor tab bar row.
    // Children: reads buffer_manager.buffers() for tab names and modified flags.
    // Siblings: render_tab_bar in axe-ui (must use identical tab width calculation).
    // Risk: None — stateless helper, cannot corrupt state.

    /// Determines which editor tab is at the given column offset within the tab bar.
    ///
    /// Walks buffer names to find which tab occupies the column position.
    /// Returns `None` if the column is past all tabs.
    pub(super) fn editor_tab_index_at_col(&self, col: u16) -> Option<usize> {
        let mut x: u16 = 0;
        let buf_count = self.buffer_manager.buffers().len();
        for (i, buf) in self.buffer_manager.buffers().iter().enumerate() {
            let name = buf.file_name().unwrap_or("untitled");
            // Format: "[N:name]" or "[N:name+]"
            let num_width = (i + 1).ilog10() as u16 + 1;
            let tab_width = if buf.modified {
                // "[" + num + ":" + name + "+" + "]"
                1 + num_width + 1 + name.len() as u16 + 1 + 1
            } else {
                // "[" + num + ":" + name + "]"
                1 + num_width + 1 + name.len() as u16 + 1
            };
            if col >= x && col < x + tab_width {
                return Some(i);
            }
            x += tab_width;
            // Space between tabs.
            if i + 1 < buf_count {
                x += 1;
            }
        }
        None
    }

    // IMPACT ANALYSIS — screen_to_editor_pos
    // Parents: handle_mouse_event() calls this for Down and Drag events in the editor area.
    // Children: None — pure conversion function returning Option<(row, col)> in buffer coordinates.
    // Siblings: editor_inner_area (must be set by main.rs each frame),
    //           buffer scroll_row/scroll_col (used to convert screen to file position).
    // Risk: None — stateless helper, cannot corrupt state.

    /// Returns `true` if the screen coordinates fall within the editor scrollbar area.
    pub(super) fn scrollbar_hit(&self, screen_col: u16, screen_row: u16) -> bool {
        if let Some((sx, sy, sw, sh)) = self.editor_scrollbar_area {
            screen_col >= sx && screen_col < sx + sw && screen_row >= sy && screen_row < sy + sh
        } else {
            false
        }
    }

    /// Sets the editor `scroll_row` proportional to the mouse Y within the scrollbar area.
    pub(super) fn scrollbar_jump_to(&mut self, screen_row: u16) {
        let (_, sy, _, sh) = match self.editor_scrollbar_area {
            Some(area) => area,
            None => return,
        };
        let buf = match self.buffer_manager.active_buffer_mut() {
            Some(b) => b,
            None => return,
        };
        let (viewport_height, _) = self
            .editor_inner_area
            .map(|(_x, _y, w, h)| (h as usize, w as usize))
            .unwrap_or((20, 80));
        let max_scroll = buf.line_count().saturating_sub(viewport_height);
        if max_scroll == 0 || sh == 0 {
            return;
        }
        // Clamp mouse row to scrollbar bounds.
        let clamped_row = screen_row.clamp(sy, sy + sh.saturating_sub(1));
        let relative = (clamped_row - sy) as f64;
        let fraction = relative / (sh.saturating_sub(1)).max(1) as f64;
        buf.scroll_row = (fraction * max_scroll as f64).round() as usize;
    }

    /// Converts screen coordinates to editor buffer (row, col) position.
    ///
    /// Returns `None` if the coordinates are outside the editor content area
    /// or if no editor area has been set.
    pub(super) fn screen_to_editor_pos(
        &self,
        screen_col: u16,
        screen_row: u16,
    ) -> Option<(usize, usize)> {
        let (ex, ey, ew, eh) = self.editor_inner_area?;
        if screen_col < ex || screen_col >= ex + ew || screen_row < ey || screen_row >= ey + eh {
            return None;
        }
        let buf = self.buffer_manager.active_buffer()?;
        let rel_row = (screen_row - ey) as usize;
        let rel_col = (screen_col - ex) as usize;
        let file_row = buf.scroll_row + rel_row;
        let file_col = buf.scroll_col + rel_col;
        // Clamp to actual content bounds.
        let max_row = buf.line_count().saturating_sub(1);
        let row = file_row.min(max_row);
        let col = file_col.min(buf.line_length(row));
        Some((row, col))
    }

    /// Checks if autosave should trigger based on elapsed time since last edit.
    ///
    /// Saves the active buffer if it has been modified and has a file path,
    /// and at least `AUTOSAVE_DELAY` has passed since the last edit.
    pub fn check_autosave(&mut self) {
        if !self.config.editor.auto_save {
            return;
        }
        let delay = Duration::from_millis(self.config.editor.auto_save_delay_ms);
        if let Some(last_edit) = self.last_edit_time {
            if last_edit.elapsed() >= delay {
                if let Some(buf) = self.buffer_manager.active_buffer_mut() {
                    if buf.modified && buf.path().is_some() {
                        if let Err(e) = buf.save_to_file() {
                            log::warn!("Autosave failed: {e}");
                        }
                    }
                }
                self.last_edit_time = None;
            }
        }
    }

    /// Checks if the mouse hover delay has elapsed and triggers a hover request.
    ///
    /// Called each frame from the main loop. If the mouse has been stationary
    /// over a buffer position for 500ms, sends a hover request to the LSP.
    pub fn check_hover_timer(&mut self) {
        const HOVER_DELAY: Duration = Duration::from_millis(500);

        if let Some((time, row, col)) = self.hover_mouse_state {
            if time.elapsed() >= HOVER_DELAY {
                self.hover_mouse_state = None;
                // Send hover request for the mouse position.
                self.ensure_lsp_open_for_active_buffer();
                if let Some(ref mut lsp) = self.lsp_manager {
                    if let Some(buf) = self.buffer_manager.active_buffer() {
                        if let Some(path) = buf.path() {
                            let path = path.to_path_buf();
                            if let Err(e) = lsp.request_hover(&path, row as u32, col as u32) {
                                log::warn!("LSP hover request (mouse) failed: {e}");
                            }
                        }
                    }
                }
            }
        }
    }

    /// Handles mouse movement for hover delay tracking.
    ///
    /// Records mouse position in editor area for 500ms delay-triggered hover.
    pub fn handle_mouse_moved(&mut self, column: u16, row: u16) {
        let Some((editor_x, editor_y, editor_w, editor_h)) = self.editor_inner_area else {
            self.hover_mouse_state = None;
            return;
        };

        // Check if mouse is within editor content area.
        if column < editor_x
            || column >= editor_x + editor_w
            || row < editor_y
            || row >= editor_y + editor_h
        {
            self.hover_mouse_state = None;
            return;
        }

        // Convert screen coordinates to buffer coordinates.
        if let Some(buf) = self.buffer_manager.active_buffer() {
            let line_count = buf.line_count();
            let digits = if line_count == 0 {
                1
            } else {
                (line_count as f64).log10().floor() as u16 + 1
            };
            // Gutter: digits + 2 padding + 2 diagnostic indicator
            let gutter_width = digits + 4;

            let rel_col = column.saturating_sub(editor_x);
            if rel_col < gutter_width {
                self.hover_mouse_state = None;
                return;
            }

            let buf_col = (rel_col - gutter_width) as usize + buf.scroll_col;
            let buf_row = (row - editor_y) as usize + buf.scroll_row;

            // Only update if position changed.
            let new_pos = (buf_row, buf_col);
            let same = self
                .hover_mouse_state
                .as_ref()
                .is_some_and(|(_, r, c)| *r == new_pos.0 && *c == new_pos.1);
            if !same {
                // Clear current hover when mouse moves to a different position.
                self.hover_info = None;
                self.hover_mouse_state = Some((Instant::now(), buf_row, buf_col));
            }
        }
    }
}
