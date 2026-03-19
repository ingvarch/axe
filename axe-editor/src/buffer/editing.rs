use anyhow::{Context, Result};

use crate::highlight;
use crate::history::Edit;

use super::EditorBuffer;

impl EditorBuffer {
    /// Begins an undo group — all subsequent edits merge into a single undo step.
    ///
    /// Call `end_undo_group()` when done. Used by Replace All so that
    /// a single Ctrl+Z undoes all replacements at once.
    pub fn begin_undo_group(&mut self) {
        self.history.set_force_merge(true);
    }

    /// Ends the undo group started by `begin_undo_group()`.
    pub fn end_undo_group(&mut self) {
        self.history.set_force_merge(false);
    }

    // IMPACT ANALYSIS — insert_char
    // Parents: KeyEvent → Command::EditorInsertChar(ch) → this function
    // Children: UI renders updated content, cursor advances, modified flag set
    // Siblings: Selection (none yet), SyntaxHighlighter (future), LspClient (future)

    /// Inserts a character at the current cursor position.
    ///
    /// If a selection is active, deletes it first.
    pub fn insert_char(&mut self, ch: char) {
        if self.selection.is_some() {
            self.delete_selection();
        }
        let char_idx = self.content.line_to_char(self.cursor.row) + self.cursor.col;
        let cursor_before = self.cursor.clone();
        self.content.insert_char(char_idx, ch);
        self.cursor.col += 1;
        self.cursor.desired_col = self.cursor.col;
        self.modified = true;
        self.history.record(
            Edit {
                char_idx,
                old_text: String::new(),
                new_text: ch.to_string(),
            },
            cursor_before,
            self.cursor.clone(),
        );
        self.notify_highlight_insert(char_idx, 1);
    }

    // IMPACT ANALYSIS — insert_newline
    // Parents: KeyEvent → Command::EditorNewline → this function
    // Children: Splits line, auto-indents from current line, cursor moves to new line
    // Siblings: Line count changes (affects gutter width, status bar line count)

    /// Inserts a newline at the current cursor position with auto-indent.
    ///
    /// If a selection is active, deletes it first.
    pub fn insert_newline(&mut self) {
        if self.selection.is_some() {
            self.delete_selection();
        }
        let char_idx = self.content.line_to_char(self.cursor.row) + self.cursor.col;
        let cursor_before = self.cursor.clone();
        let indent = self.leading_whitespace(self.cursor.row);
        let insert_str = format!("\n{indent}");
        let insert_chars = insert_str.chars().count();
        self.content.insert(char_idx, &insert_str);
        self.cursor.row += 1;
        self.cursor.col = indent.len();
        self.cursor.desired_col = self.cursor.col;
        self.modified = true;
        self.history.record(
            Edit {
                char_idx,
                old_text: String::new(),
                new_text: insert_str,
            },
            cursor_before,
            self.cursor.clone(),
        );
        self.notify_highlight_insert(char_idx, insert_chars);
    }

    // IMPACT ANALYSIS — insert_tab
    // Parents: KeyEvent → Command::EditorTab → this function
    // Children: Inserts tab_size spaces (or a literal \t), cursor advances
    // Siblings: Same as insert_char

    /// Inserts a tab at the current cursor position.
    ///
    /// When `insert_spaces` is `true`, inserts `tab_size` space characters.
    /// When `insert_spaces` is `false`, inserts a single literal tab character.
    /// If a selection is active, deletes it first.
    pub fn insert_tab(&mut self) {
        if self.selection.is_some() {
            self.delete_selection();
        }
        let char_idx = self.content.line_to_char(self.cursor.row) + self.cursor.col;
        let cursor_before = self.cursor.clone();

        let (insert_text, advance) = if self.insert_spaces {
            (" ".repeat(self.tab_size), self.tab_size)
        } else {
            ("\t".to_owned(), 1)
        };

        self.content.insert(char_idx, &insert_text);
        self.cursor.col += advance;
        self.cursor.desired_col = self.cursor.col;
        self.modified = true;
        self.history.record(
            Edit {
                char_idx,
                old_text: String::new(),
                new_text: insert_text,
            },
            cursor_before,
            self.cursor.clone(),
        );
        self.notify_highlight_insert(char_idx, advance);
    }

    // IMPACT ANALYSIS — delete_char_backward
    // Parents: KeyEvent → Command::EditorBackspace → this function
    // Children: Removes char before cursor or joins lines, cursor moves back
    // Siblings: Line count may change (if joining lines), gutter width may change

    /// Deletes the character before the cursor (backspace).
    ///
    /// If a selection is active, deletes the selection instead.
    /// At the beginning of a line, joins with the previous line.
    /// At the beginning of the file, does nothing.
    pub fn delete_char_backward(&mut self) {
        if self.selection.is_some() {
            self.delete_selection();
            return;
        }
        if self.cursor.col > 0 {
            let char_idx = self.content.line_to_char(self.cursor.row) + self.cursor.col;
            let cursor_before = self.cursor.clone();
            let deleted: String = self.content.slice(char_idx - 1..char_idx).into();
            let old_byte_len = deleted.len();
            let old_end_pos =
                highlight::byte_to_point(&self.content, self.content.char_to_byte(char_idx));
            self.content.remove(char_idx - 1..char_idx);
            self.cursor.col -= 1;
            self.cursor.desired_col = self.cursor.col;
            self.modified = true;
            self.history.record(
                Edit {
                    char_idx: char_idx - 1,
                    old_text: deleted,
                    new_text: String::new(),
                },
                cursor_before,
                self.cursor.clone(),
            );
            self.notify_highlight_delete(char_idx - 1, 1, old_byte_len, old_end_pos);
        } else if self.cursor.row > 0 {
            let cursor_before = self.cursor.clone();
            let prev_line_len = self.line_length(self.cursor.row - 1);
            let char_idx = self.content.line_to_char(self.cursor.row);
            // Remove \r\n or \n at end of previous line.
            let remove_start = if char_idx >= 2 && self.content.char(char_idx - 2) == '\r' {
                char_idx - 2
            } else {
                char_idx - 1
            };
            let deleted: String = self.content.slice(remove_start..char_idx).into();
            let old_byte_len = deleted.len();
            let chars_deleted = char_idx - remove_start;
            let old_end_pos =
                highlight::byte_to_point(&self.content, self.content.char_to_byte(char_idx));
            self.content.remove(remove_start..char_idx);
            self.cursor.row -= 1;
            self.cursor.col = prev_line_len;
            self.cursor.desired_col = self.cursor.col;
            self.modified = true;
            self.history.record(
                Edit {
                    char_idx: remove_start,
                    old_text: deleted,
                    new_text: String::new(),
                },
                cursor_before,
                self.cursor.clone(),
            );
            self.notify_highlight_delete(remove_start, chars_deleted, old_byte_len, old_end_pos);
        }
    }

    // IMPACT ANALYSIS — delete_char_forward
    // Parents: KeyEvent → Command::EditorDelete → this function
    // Children: Removes char at cursor or joins with next line, cursor stays
    // Siblings: Line count may change, gutter width may change

    /// Deletes the character at the cursor position (forward delete).
    ///
    /// If a selection is active, deletes the selection instead.
    /// At the end of a line, joins with the next line.
    /// At the end of the file, does nothing.
    pub fn delete_char_forward(&mut self) {
        if self.selection.is_some() {
            self.delete_selection();
            return;
        }
        let line_len = self.line_length(self.cursor.row);
        let char_idx = self.content.line_to_char(self.cursor.row) + self.cursor.col;
        if self.cursor.col < line_len {
            let cursor_before = self.cursor.clone();
            let deleted: String = self.content.slice(char_idx..char_idx + 1).into();
            let old_byte_len = deleted.len();
            let old_end_pos =
                highlight::byte_to_point(&self.content, self.content.char_to_byte(char_idx + 1));
            self.content.remove(char_idx..char_idx + 1);
            self.modified = true;
            self.history.record(
                Edit {
                    char_idx,
                    old_text: deleted,
                    new_text: String::new(),
                },
                cursor_before,
                self.cursor.clone(),
            );
            self.notify_highlight_delete(char_idx, 1, old_byte_len, old_end_pos);
        } else if self.cursor.row + 1 < self.content_line_count() {
            let cursor_before = self.cursor.clone();
            // At end of line — join with next line by removing the newline.
            let remove_end =
                if char_idx < self.content.len_chars() && self.content.char(char_idx) == '\r' {
                    char_idx + 2
                } else {
                    char_idx + 1
                };
            let deleted: String = self.content.slice(char_idx..remove_end).into();
            let old_byte_len = deleted.len();
            let chars_deleted = remove_end - char_idx;
            let old_end_pos =
                highlight::byte_to_point(&self.content, self.content.char_to_byte(remove_end));
            self.content.remove(char_idx..remove_end);
            self.modified = true;
            self.history.record(
                Edit {
                    char_idx,
                    old_text: deleted,
                    new_text: String::new(),
                },
                cursor_before,
                self.cursor.clone(),
            );
            self.notify_highlight_delete(char_idx, chars_deleted, old_byte_len, old_end_pos);
        }
    }

    // IMPACT ANALYSIS — save_to_file
    // Parents: Command::EditorSave → this function, autosave timer
    // Children: Writes file to disk atomically (temp file + rename), clears modified flag
    // Siblings: File watcher (future) may trigger on save, tree may need refresh

    /// Saves the buffer content to its associated file path atomically.
    ///
    /// Uses a temporary file in the same directory followed by a rename to
    /// prevent data loss on crash. Returns an error if no path is set.
    pub fn save_to_file(&mut self) -> Result<()> {
        let path = self.path.as_ref().context("Buffer has no file path")?;
        let dir = path.parent().context("File path has no parent directory")?;
        let tmp = tempfile::NamedTempFile::new_in(dir)
            .with_context(|| format!("Failed to create temp file in {}", dir.display()))?;
        self.content
            .write_to(std::io::BufWriter::new(tmp.as_file()))
            .context("Failed to write buffer content")?;
        tmp.persist(path)
            .with_context(|| format!("Failed to save to {}", path.display()))?;
        self.modified = false;
        Ok(())
    }

    // IMPACT ANALYSIS — undo / redo
    // Parents: Command::EditorUndo/EditorRedo → app.execute() → these methods
    // Children: Reverses/replays edits on rope, restores cursor position
    // Siblings: modified flag set (always true after undo/redo since content changed)

    /// Undoes the last edit group, restoring content and cursor.
    pub fn undo(&mut self) {
        if let Some(group) = self.history.undo() {
            for edit in group.edits.iter().rev() {
                // Remove what was inserted.
                if !edit.new_text.is_empty() {
                    let end = edit.char_idx + edit.new_text.chars().count();
                    self.content.remove(edit.char_idx..end);
                }
                // Re-insert what was deleted.
                if !edit.old_text.is_empty() {
                    self.content.insert(edit.char_idx, &edit.old_text);
                }
            }
            self.cursor = group.cursor_before;
            self.cursor.desired_col = self.cursor.col;
            self.modified = true;
            self.reparse_highlight_full();
        }
    }

    /// Redoes the last undone edit group, re-applying content changes and cursor.
    pub fn redo(&mut self) {
        if let Some(group) = self.history.redo() {
            for edit in group.edits.iter() {
                // Remove what was originally there.
                if !edit.old_text.is_empty() {
                    let end = edit.char_idx + edit.old_text.chars().count();
                    self.content.remove(edit.char_idx..end);
                }
                // Insert what was added.
                if !edit.new_text.is_empty() {
                    self.content.insert(edit.char_idx, &edit.new_text);
                }
            }
            self.cursor = group.cursor_after;
            self.cursor.desired_col = self.cursor.col;
            self.modified = true;
            self.reparse_highlight_full();
        }
    }

    /// Inserts text at the current cursor position.
    ///
    /// If a selection is active, deletes it first. Handles multi-line text
    /// by advancing cursor row/col appropriately.
    pub fn insert_text(&mut self, text: &str) {
        if text.is_empty() {
            return;
        }
        // Delete selection first if active.
        if self.selection.is_some() {
            self.delete_selection();
        }
        let char_idx = self.content.line_to_char(self.cursor.row) + self.cursor.col;
        let cursor_before = self.cursor.clone();
        let chars_inserted = text.chars().count();
        self.content.insert(char_idx, text);

        // Advance cursor past the inserted text.
        for ch in text.chars() {
            if ch == '\n' {
                self.cursor.row += 1;
                self.cursor.col = 0;
            } else {
                self.cursor.col += 1;
            }
        }
        self.cursor.desired_col = self.cursor.col;
        self.modified = true;
        self.history.record(
            Edit {
                char_idx,
                old_text: String::new(),
                new_text: text.to_string(),
            },
            cursor_before,
            self.cursor.clone(),
        );
        self.notify_highlight_insert(char_idx, chars_inserted);
    }

    // IMPACT ANALYSIS — apply_text_edit
    // Parents: apply_completion in AppState
    // Children: Rope content changes, cursor moves, history records, highlight notified
    // Siblings: Selection (cleared), diagnostics (shifted by LSP after didChange)

    /// Replaces text in a range and positions the cursor at the end of the insertion.
    ///
    /// Converts (line, col) positions to rope char offsets, deletes the range,
    /// inserts `new_text`, and records the edit for undo.
    pub fn apply_text_edit(
        &mut self,
        start_line: usize,
        start_col: usize,
        end_line: usize,
        end_col: usize,
        new_text: &str,
    ) {
        // Clear selection before editing.
        self.selection = None;

        let total_lines = self.content.len_lines();
        let start_line = start_line.min(total_lines.saturating_sub(1));
        let end_line = end_line.min(total_lines.saturating_sub(1));

        let start_idx = self.content.line_to_char(start_line) + start_col;
        let end_idx = self.content.line_to_char(end_line) + end_col;

        // Clamp indices to content length.
        let total_chars = self.content.len_chars();
        let start_idx = start_idx.min(total_chars);
        let end_idx = end_idx.min(total_chars).max(start_idx);

        let cursor_before = self.cursor.clone();
        let old_text: String = self.content.slice(start_idx..end_idx).into();
        let old_len = old_text.len();
        let old_end_pos = if end_idx > start_idx {
            highlight::byte_to_point(&self.content, self.content.char_to_byte(end_idx))
        } else {
            highlight::byte_to_point(&self.content, self.content.char_to_byte(start_idx))
        };

        // Delete old range.
        if end_idx > start_idx {
            self.content.remove(start_idx..end_idx);
        }

        // Insert new text.
        let new_chars = new_text.chars().count();
        if !new_text.is_empty() {
            self.content.insert(start_idx, new_text);
        }

        // Position cursor at end of inserted text.
        let mut row = start_line;
        let mut col = start_col;
        for ch in new_text.chars() {
            if ch == '\n' {
                row += 1;
                col = 0;
            } else {
                col += 1;
            }
        }
        self.cursor.row = row;
        self.cursor.col = col;
        self.cursor.desired_col = col;
        self.modified = true;

        self.history.record(
            Edit {
                char_idx: start_idx,
                old_text,
                new_text: new_text.to_string(),
            },
            cursor_before,
            self.cursor.clone(),
        );

        // Notify syntax highlighter about the edit.
        if end_idx > start_idx {
            self.notify_highlight_delete(start_idx, end_idx - start_idx, old_len, old_end_pos);
        }
        if new_chars > 0 {
            self.notify_highlight_insert(start_idx, new_chars);
        }
    }
}
