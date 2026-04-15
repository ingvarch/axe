use axe_editor::diff::{diff_kind_for_line, DiffHunkKind};

use super::types::DiffPopup;
use super::AppState;

/// Width of the diff indicator column in the gutter.
const DIFF_GUTTER_WIDTH: u16 = 1;

impl AppState {
    /// Checks if a screen position is on the diff gutter column and returns the buffer line.
    ///
    /// The diff gutter is the column immediately to the left of `editor_inner_area`.
    /// Returns `Some(buffer_line)` if the click is on a line with a diff indicator.
    pub(super) fn screen_to_diff_gutter_line(
        &self,
        screen_col: u16,
        screen_row: u16,
    ) -> Option<usize> {
        let (ex, ey, _ew, eh) = self.editor_inner_area?;
        // The diff gutter column is immediately before the editor content area.
        let diff_col = ex.checked_sub(DIFF_GUTTER_WIDTH)?;
        if screen_col != diff_col {
            return None;
        }
        if screen_row < ey || screen_row >= ey + eh {
            return None;
        }
        let buf = self.buffer_manager.active_buffer()?;
        let rel_row = (screen_row - ey) as usize;
        let buffer_line = buf.scroll_row + rel_row;
        // Only return if this line has a diff indicator.
        if diff_kind_for_line(buf.diff_hunks(), buffer_line).is_some() {
            Some(buffer_line)
        } else {
            None
        }
    }

    /// Opens the diff popup for the hunk at the current cursor position.
    ///
    /// Does nothing if the cursor is not on a changed line or no buffer is active.
    pub fn show_diff_hunk(&mut self) {
        let cursor_line = match self.buffer_manager.active_buffer() {
            Some(b) => b.cursor().row,
            None => return,
        };
        self.show_diff_hunk_at_line(cursor_line);
    }

    /// Opens the diff popup for the hunk covering the given buffer line.
    ///
    /// Does nothing if the line has no diff hunk or no buffer is active.
    pub fn show_diff_hunk_at_line(&mut self, target_line: usize) {
        let buf = match self.buffer_manager.active_buffer() {
            Some(b) => b,
            None => return,
        };

        let cursor_line = target_line;
        let hunks = buf.diff_hunks();

        // Find the hunk that covers the cursor line.
        if diff_kind_for_line(hunks, cursor_line).is_none() {
            return;
        }

        let (hunk_index, hunk) = match hunks.iter().enumerate().find(|(_, h)| match h.kind {
            DiffHunkKind::Deleted => h.line_count == 0 && cursor_line == h.start_line,
            _ => cursor_line >= h.start_line && cursor_line < h.start_line + h.line_count,
        }) {
            Some(pair) => pair,
            None => return,
        };

        // Extract current lines from the buffer for this hunk range.
        let new_lines: Vec<String> = (0..hunk.line_count)
            .filter_map(|i| {
                buf.line_at(hunk.start_line + i)
                    .map(|s| s.to_string().trim_end_matches('\n').to_string())
            })
            .collect();

        self.diff_popup = Some(DiffPopup {
            hunk_index,
            start_line: hunk.start_line,
            line_count: hunk.line_count,
            kind: hunk.kind,
            old_lines: hunk.old_lines.clone(),
            new_lines,
            selected: Default::default(),
        });
    }

    /// Reverts the hunk shown in the diff popup to its original (HEAD) content.
    ///
    /// Uses `apply_text_edit` so the change is recorded in undo history.
    pub fn revert_diff_hunk(&mut self) {
        let popup = match self.diff_popup.take() {
            Some(p) => p,
            None => return,
        };

        let buf = match self.buffer_manager.active_buffer_mut() {
            Some(b) => b,
            None => return,
        };

        let total_lines = buf.line_count();

        match popup.kind {
            DiffHunkKind::Modified => {
                // Replace the modified lines with the original content.
                let replacement = popup.old_lines.join("\n") + "\n";
                let end_line = (popup.start_line + popup.line_count).min(total_lines);
                let end_col = if end_line < total_lines {
                    0
                } else {
                    buf.line_at(end_line.saturating_sub(1))
                        .map(|s| s.len_chars())
                        .unwrap_or(0)
                };

                // Replace from start of first changed line to start of line after last changed line.
                if end_line < total_lines {
                    buf.apply_text_edit(popup.start_line, 0, end_line, 0, &replacement);
                } else {
                    buf.apply_text_edit(
                        popup.start_line,
                        0,
                        end_line.saturating_sub(1),
                        end_col,
                        // Don't add trailing newline if replacing up to end of file.
                        &popup.old_lines.join("\n"),
                    );
                }
            }
            DiffHunkKind::Added => {
                // Delete the added lines.
                let end_line = (popup.start_line + popup.line_count).min(total_lines);
                if end_line < total_lines {
                    buf.apply_text_edit(popup.start_line, 0, end_line, 0, "");
                } else {
                    // Deleting at end of file — also remove the preceding newline.
                    let start_line = popup.start_line;
                    let end_col = buf
                        .line_at(end_line.saturating_sub(1))
                        .map(|s| s.len_chars())
                        .unwrap_or(0);
                    if start_line > 0 {
                        let prev_len = buf
                            .line_at(start_line - 1)
                            .map(|s| s.len_chars().saturating_sub(1))
                            .unwrap_or(0);
                        buf.apply_text_edit(
                            start_line - 1,
                            prev_len,
                            end_line.saturating_sub(1),
                            end_col,
                            "",
                        );
                    } else {
                        buf.apply_text_edit(0, 0, end_line.saturating_sub(1), end_col, "");
                    }
                }
            }
            DiffHunkKind::Deleted => {
                // Re-insert the deleted lines at the deletion point.
                let insertion = popup.old_lines.join("\n") + "\n";
                buf.apply_text_edit(popup.start_line, 0, popup.start_line, 0, &insertion);
            }
        }

        // Refresh diff hunks after the revert.
        self.refresh_active_buffer_diff_hunks();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axe_editor::diff::DiffHunk;
    use std::io::Write;

    /// Helper: create a minimal AppState with one buffer containing given text and diff hunks.
    fn app_with_buffer(text: &str, hunks: Vec<DiffHunk>) -> (AppState, tempfile::NamedTempFile) {
        let mut tmp = tempfile::NamedTempFile::new().expect("create temp file");
        tmp.write_all(text.as_bytes())
            .expect("write temp file content");
        tmp.flush().expect("flush temp file");

        let mut app = AppState::new();
        app.buffer_manager
            .open_file(tmp.path())
            .expect("open temp file");
        if let Some(buf) = app.buffer_manager.active_buffer_mut() {
            buf.set_diff_hunks(hunks);
        }
        (app, tmp)
    }

    #[test]
    fn show_diff_hunk_on_unchanged_line_does_nothing() {
        let (mut app, _tmp) = app_with_buffer("line 1\nline 2\nline 3\n", Vec::new());
        app.show_diff_hunk();
        assert!(
            app.diff_popup.is_none(),
            "should not show popup on unchanged line"
        );
    }

    #[test]
    fn show_diff_hunk_on_modified_line_creates_popup() {
        let hunks = vec![DiffHunk {
            start_line: 1,
            line_count: 1,
            kind: DiffHunkKind::Modified,
            old_lines: vec!["original line 2".to_string()],
        }];
        let (mut app, _tmp) = app_with_buffer("line 1\nmodified\nline 3\n", hunks);
        // Move cursor to line 1.
        if let Some(buf) = app.buffer_manager.active_buffer_mut() {
            buf.cursor_mut().row = 1;
        }
        app.show_diff_hunk();
        let popup = app.diff_popup.as_ref().expect("popup should be shown");
        assert_eq!(popup.kind, DiffHunkKind::Modified);
        assert_eq!(popup.old_lines, vec!["original line 2"]);
        assert_eq!(popup.new_lines, vec!["modified"]);
    }

    #[test]
    fn show_diff_hunk_on_added_line_creates_popup() {
        let hunks = vec![DiffHunk {
            start_line: 2,
            line_count: 1,
            kind: DiffHunkKind::Added,
            old_lines: Vec::new(),
        }];
        let (mut app, _tmp) = app_with_buffer("line 1\nline 2\nnew line\n", hunks);
        if let Some(buf) = app.buffer_manager.active_buffer_mut() {
            buf.cursor_mut().row = 2;
        }
        app.show_diff_hunk();
        let popup = app.diff_popup.as_ref().expect("popup should be shown");
        assert_eq!(popup.kind, DiffHunkKind::Added);
        assert!(popup.old_lines.is_empty());
        assert_eq!(popup.new_lines, vec!["new line"]);
    }

    #[test]
    fn revert_modified_hunk_restores_old_content() {
        let hunks = vec![DiffHunk {
            start_line: 1,
            line_count: 1,
            kind: DiffHunkKind::Modified,
            old_lines: vec!["original".to_string()],
        }];
        let (mut app, _tmp) = app_with_buffer("line 1\nmodified\nline 3\n", hunks);
        if let Some(buf) = app.buffer_manager.active_buffer_mut() {
            buf.cursor_mut().row = 1;
        }
        app.show_diff_hunk();
        assert!(app.diff_popup.is_some());
        app.revert_diff_hunk();
        assert!(app.diff_popup.is_none(), "popup should be closed");
        let content = app
            .buffer_manager
            .active_buffer()
            .expect("buffer")
            .content_string();
        assert_eq!(content, "line 1\noriginal\nline 3\n");
    }

    #[test]
    fn revert_added_hunk_removes_lines() {
        let hunks = vec![DiffHunk {
            start_line: 1,
            line_count: 2,
            kind: DiffHunkKind::Added,
            old_lines: Vec::new(),
        }];
        let (mut app, _tmp) = app_with_buffer("line 1\nnew 1\nnew 2\nline 2\n", hunks);
        if let Some(buf) = app.buffer_manager.active_buffer_mut() {
            buf.cursor_mut().row = 1;
        }
        app.show_diff_hunk();
        app.revert_diff_hunk();
        let content = app
            .buffer_manager
            .active_buffer()
            .expect("buffer")
            .content_string();
        assert_eq!(content, "line 1\nline 2\n");
    }

    #[test]
    fn revert_deleted_hunk_reinserts_lines() {
        let hunks = vec![DiffHunk {
            start_line: 1,
            line_count: 0,
            kind: DiffHunkKind::Deleted,
            old_lines: vec!["deleted line".to_string()],
        }];
        let (mut app, _tmp) = app_with_buffer("line 1\nline 3\n", hunks);
        if let Some(buf) = app.buffer_manager.active_buffer_mut() {
            buf.cursor_mut().row = 1;
        }
        app.show_diff_hunk();
        app.revert_diff_hunk();
        let content = app
            .buffer_manager
            .active_buffer()
            .expect("buffer")
            .content_string();
        assert_eq!(content, "line 1\ndeleted line\nline 3\n");
    }
}
