use anyhow::{Context, Result};

use crate::cursor::CursorState;
use crate::highlight;
use crate::history::Edit;
use crate::selection::Selection;

use super::EditorBuffer;

/// One cursor's planned edit inside a multi-cursor operation.
struct PerCursorEdit {
    orig_idx: usize,
    row: usize,
    start_col: usize,
    end_col: usize,
    new_text: String,
    new_text_chars: usize,
}

impl EditorBuffer {
    /// Begins an undo group — all subsequent edits merge into a single undo step.
    ///
    /// Call `end_undo_group()` when done. Used by Replace All so that
    /// a single Ctrl+Z undoes all replacements at once.
    pub fn begin_undo_group(&mut self) {
        self.history.set_force_merge(true);
    }

    /// Begins a labeled undo group that does not merge with any pre-existing
    /// group even when `force_merge` is set. Used by multi-edit operations
    /// like Rename and Code Actions to make their undo step distinct and
    /// discoverable.
    pub fn begin_labeled_undo_group(&mut self, label: &str) {
        self.history.begin_isolated_group(label.to_string());
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
    /// If a selection is active, deletes it first. With multiple cursors,
    /// each cursor (and its selection) receives the insertion, all edits
    /// grouped into a single undo step.
    pub fn insert_char(&mut self, ch: char) {
        if self.has_multiple_cursors() {
            self.insert_string_multi(&ch.to_string());
            return;
        }
        if self.has_selection() {
            self.delete_selection();
        }
        let char_idx = self.content.line_to_char(self.cursor().row) + self.cursor().col;
        let cursor_before = self.cursor().clone();
        self.content.insert_char(char_idx, ch);
        self.cursor_mut().col += 1;
        self.cursor_mut().desired_col = self.cursor().col;
        self.modified = true;
        self.history.record(
            Edit {
                char_idx,
                old_text: String::new(),
                new_text: ch.to_string(),
            },
            cursor_before,
            self.cursor().clone(),
        );
        self.notify_highlight_insert(char_idx, 1);
    }

    /// Multi-cursor insert path used by [`insert_char`] when there are
    /// secondary cursors.
    ///
    /// Each cursor that has a selection gets the selection deleted and
    /// `text` inserted in its place; cursors without a selection just get
    /// `text` inserted at their position. Restrictions:
    ///
    /// - `text` must not contain newlines (phase-D v1 is single-line only).
    /// - Every selection must be single-line — a multi-line selection
    ///   triggers a fallback that clears secondary cursors and runs the
    ///   single-cursor fast path on the primary.
    ///
    /// All per-cursor edits are grouped into a single undo step and
    /// applied right-to-left in byte order so earlier-in-document positions
    /// stay valid while the rope mutates.
    fn insert_string_multi(&mut self, text: &str) {
        if text.contains('\n') {
            // Multi-line inserts are out of scope for v1 — drop secondaries
            // so the primary-only fast path below has clean state.
            self.clear_secondary_cursors();
            self.insert_text(text);
            return;
        }

        // Snapshot the current cursor set; we rebuild it from scratch below.
        let cursors_snapshot: Vec<CursorState> = self.cursors.all().to_vec();
        let selections_snapshot: Vec<Option<Selection>> = self.cursors.all_selections().to_vec();
        let primary_idx = self.cursors.primary_index();

        // Convert each cursor into an edit descriptor.
        let mut edits: Vec<PerCursorEdit> = Vec::with_capacity(cursors_snapshot.len());
        for (orig_idx, (cursor, selection)) in cursors_snapshot
            .iter()
            .zip(selections_snapshot.iter())
            .enumerate()
        {
            let (sr, sc, er, ec) = if let Some(sel) = selection {
                sel.normalized(cursor.row, cursor.col)
            } else {
                (cursor.row, cursor.col, cursor.row, cursor.col)
            };
            if er != sr {
                // Multi-line selection — fall back to single-cursor behaviour.
                self.clear_secondary_cursors();
                if self.has_selection() {
                    self.delete_selection();
                }
                for ch in text.chars() {
                    self.insert_char(ch);
                }
                return;
            }
            edits.push(PerCursorEdit {
                orig_idx,
                row: sr,
                start_col: sc,
                end_col: ec,
                new_text: text.to_string(),
                new_text_chars: text.chars().count(),
            });
        }

        // Compute each cursor's final column BEFORE mutating the rope.
        // Edits on the same row affect later cols by `new_len - old_len`.
        let mut final_positions: Vec<(usize, usize)> = vec![(0, 0); edits.len()];
        // Group edit indices by row, sorted by ascending start_col, to
        // walk them left-to-right and accumulate the delta.
        let mut by_row: std::collections::HashMap<usize, Vec<usize>> =
            std::collections::HashMap::new();
        for (i, e) in edits.iter().enumerate() {
            by_row.entry(e.row).or_default().push(i);
        }
        for group in by_row.values_mut() {
            group.sort_by_key(|i| edits[*i].start_col);
            let mut cumulative_delta: isize = 0;
            for &i in group.iter() {
                let e = &edits[i];
                let adjusted_start = (e.start_col as isize + cumulative_delta) as usize;
                let new_col = adjusted_start + e.new_text_chars;
                final_positions[i] = (e.row, new_col);
                let delta = e.new_text_chars as isize - (e.end_col as isize - e.start_col as isize);
                cumulative_delta += delta;
            }
        }

        // Apply the rope mutations right-to-left so earlier positions keep
        // working while we iterate.
        let mut order: Vec<usize> = (0..edits.len()).collect();
        order.sort_by(|a, b| {
            let ea = &edits[*a];
            let eb = &edits[*b];
            (eb.row, eb.start_col).cmp(&(ea.row, ea.start_col))
        });

        self.begin_undo_group();
        for i in &order {
            let e = &edits[*i];
            let start_idx = self.content.line_to_char(e.row) + e.start_col;
            let end_idx = self.content.line_to_char(e.row) + e.end_col;

            let old_text: String = if end_idx > start_idx {
                self.content.slice(start_idx..end_idx).to_string()
            } else {
                String::new()
            };
            let old_byte_len = old_text.len();
            let old_end_pos = highlight::byte_to_point(
                &self.content,
                self.content.char_to_byte(end_idx.max(start_idx)),
            );

            if end_idx > start_idx {
                self.content.remove(start_idx..end_idx);
            }
            if !e.new_text.is_empty() {
                self.content.insert(start_idx, &e.new_text);
            }

            let cursor_before = cursors_snapshot[e.orig_idx].clone();
            let cursor_after = CursorState {
                row: final_positions[*i].0,
                col: final_positions[*i].1,
                desired_col: final_positions[*i].1,
            };
            self.history.record(
                Edit {
                    char_idx: start_idx,
                    old_text,
                    new_text: e.new_text.clone(),
                },
                cursor_before,
                cursor_after,
            );

            if end_idx > start_idx {
                self.notify_highlight_delete(
                    start_idx,
                    end_idx - start_idx,
                    old_byte_len,
                    old_end_pos,
                );
            }
            if e.new_text_chars > 0 {
                self.notify_highlight_insert(start_idx, e.new_text_chars);
            }
        }
        self.end_undo_group();
        self.modified = true;

        // Rebuild the cursor set with the computed final positions.
        let new_cursors: Vec<CursorState> = final_positions
            .iter()
            .map(|(row, col)| CursorState {
                row: *row,
                col: *col,
                desired_col: *col,
            })
            .collect();
        let new_selections: Vec<Option<Selection>> = vec![None; new_cursors.len()];
        self.cursors
            .replace_with(new_cursors, new_selections, primary_idx);
    }

    // IMPACT ANALYSIS — insert_newline
    // Parents: KeyEvent → Command::EditorNewline → this function
    // Children: Splits line, auto-indents from current line, cursor moves to new line
    // Siblings: Line count changes (affects gutter width, status bar line count)

    /// Inserts a newline at the current cursor position with auto-indent.
    ///
    /// If a selection is active, deletes it first. Clears any secondary
    /// cursors first — multi-cursor newline insertion is out of scope in
    /// phase-D v1.
    pub fn insert_newline(&mut self) {
        if self.has_multiple_cursors() {
            self.clear_secondary_cursors();
        }
        if self.has_selection() {
            self.delete_selection();
        }
        let char_idx = self.content.line_to_char(self.cursor().row) + self.cursor().col;
        let cursor_before = self.cursor().clone();
        let indent = self.leading_whitespace(self.cursor().row);
        let insert_str = format!("\n{indent}");
        let insert_chars = insert_str.chars().count();
        self.content.insert(char_idx, &insert_str);
        self.cursor_mut().row += 1;
        self.cursor_mut().col = indent.len();
        self.cursor_mut().desired_col = self.cursor().col;
        self.modified = true;
        self.history.record(
            Edit {
                char_idx,
                old_text: String::new(),
                new_text: insert_str,
            },
            cursor_before,
            self.cursor().clone(),
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
    /// If a selection is active, deletes it first. Multi-cursor state is
    /// dropped first — phase-D v1 does not handle multi-cursor tab inserts.
    pub fn insert_tab(&mut self) {
        if self.has_multiple_cursors() {
            self.clear_secondary_cursors();
        }
        if self.has_selection() {
            self.delete_selection();
        }
        let char_idx = self.content.line_to_char(self.cursor().row) + self.cursor().col;
        let cursor_before = self.cursor().clone();

        let (insert_text, advance) = if self.insert_spaces {
            (" ".repeat(self.tab_size), self.tab_size)
        } else {
            ("\t".to_owned(), 1)
        };

        self.content.insert(char_idx, &insert_text);
        self.cursor_mut().col += advance;
        self.cursor_mut().desired_col = self.cursor().col;
        self.modified = true;
        self.history.record(
            Edit {
                char_idx,
                old_text: String::new(),
                new_text: insert_text,
            },
            cursor_before,
            self.cursor().clone(),
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
    /// At the beginning of the file, does nothing. Drops multi-cursor
    /// state first — phase-D v1 delegates backspace to the primary only.
    pub fn delete_char_backward(&mut self) {
        if self.has_multiple_cursors() {
            self.clear_secondary_cursors();
        }
        if self.has_selection() {
            self.delete_selection();
            return;
        }
        if self.cursor().col > 0 {
            let char_idx = self.content.line_to_char(self.cursor().row) + self.cursor().col;
            let cursor_before = self.cursor().clone();
            let deleted: String = self.content.slice(char_idx - 1..char_idx).into();
            let old_byte_len = deleted.len();
            let old_end_pos =
                highlight::byte_to_point(&self.content, self.content.char_to_byte(char_idx));
            self.content.remove(char_idx - 1..char_idx);
            self.cursor_mut().col -= 1;
            self.cursor_mut().desired_col = self.cursor().col;
            self.modified = true;
            self.history.record(
                Edit {
                    char_idx: char_idx - 1,
                    old_text: deleted,
                    new_text: String::new(),
                },
                cursor_before,
                self.cursor().clone(),
            );
            self.notify_highlight_delete(char_idx - 1, 1, old_byte_len, old_end_pos);
        } else if self.cursor().row > 0 {
            let cursor_before = self.cursor().clone();
            let prev_line_len = self.line_length(self.cursor().row - 1);
            let char_idx = self.content.line_to_char(self.cursor().row);
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
            self.cursor_mut().row -= 1;
            self.cursor_mut().col = prev_line_len;
            self.cursor_mut().desired_col = self.cursor().col;
            self.modified = true;
            self.history.record(
                Edit {
                    char_idx: remove_start,
                    old_text: deleted,
                    new_text: String::new(),
                },
                cursor_before,
                self.cursor().clone(),
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
    /// At the end of the file, does nothing. Drops multi-cursor state first.
    pub fn delete_char_forward(&mut self) {
        if self.has_multiple_cursors() {
            self.clear_secondary_cursors();
        }
        if self.has_selection() {
            self.delete_selection();
            return;
        }
        let line_len = self.line_length(self.cursor().row);
        let char_idx = self.content.line_to_char(self.cursor().row) + self.cursor().col;
        if self.cursor().col < line_len {
            let cursor_before = self.cursor().clone();
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
                self.cursor().clone(),
            );
            self.notify_highlight_delete(char_idx, 1, old_byte_len, old_end_pos);
        } else if self.cursor().row + 1 < self.content_line_count() {
            let cursor_before = self.cursor().clone();
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
                self.cursor().clone(),
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
            self.set_cursor(group.cursor_before);
            self.cursor_mut().desired_col = self.cursor().col;
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
            self.set_cursor(group.cursor_after);
            self.cursor_mut().desired_col = self.cursor().col;
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
        if self.has_selection() {
            self.delete_selection();
        }
        let char_idx = self.content.line_to_char(self.cursor().row) + self.cursor().col;
        let cursor_before = self.cursor().clone();
        let chars_inserted = text.chars().count();
        self.content.insert(char_idx, text);

        // Advance cursor past the inserted text.
        for ch in text.chars() {
            if ch == '\n' {
                self.cursor_mut().row += 1;
                self.cursor_mut().col = 0;
            } else {
                self.cursor_mut().col += 1;
            }
        }
        self.cursor_mut().desired_col = self.cursor().col;
        self.modified = true;
        self.history.record(
            Edit {
                char_idx,
                old_text: String::new(),
                new_text: text.to_string(),
            },
            cursor_before,
            self.cursor().clone(),
        );
        self.notify_highlight_insert(char_idx, chars_inserted);
    }

    // IMPACT ANALYSIS — toggle_line_comment
    // Parents: KeyEvent → Command::ToggleLineComment → app.execute() → this function
    // Children: Buffer content changes (multi-line), history records one group, highlight notified
    // Siblings: Selection (preserved row-wise; col shifted by per-row delta),
    //           LSP didChange (single notification via execute caller),
    //           Tree-sitter reparse (incremental via apply_text_edit path)

    /// Toggles line comments over the current selection or cursor line.
    ///
    /// If every non-blank line in the affected range already begins with `token`,
    /// the token is removed; otherwise `token + " "` is inserted at the column of
    /// the minimum common indent across non-blank lines. Blank lines are left
    /// untouched when commenting. All edits are grouped into a single undo step.
    ///
    /// When a selection ends exactly at column 0 of a row, that row is excluded
    /// from the range (matches VS Code / JetBrains behaviour).
    pub fn toggle_line_comment(&mut self, token: &str) {
        if token.is_empty() {
            return;
        }

        let (start_row, end_row) = self.toggle_line_range();
        let max_row = self.content_line_count().saturating_sub(1);
        if start_row > max_row {
            return;
        }
        let end_row = end_row.min(max_row);

        // Gather text and classify lines.
        let line_texts: Vec<String> = (start_row..=end_row).map(|r| self.line_text(r)).collect();

        let non_blank: Vec<(usize, &String)> = line_texts
            .iter()
            .enumerate()
            .filter(|(_, line)| !line.trim().is_empty())
            .collect();

        if non_blank.is_empty() {
            return;
        }

        // All non-blank lines commented? Uncomment. Otherwise, comment.
        let all_commented = non_blank.iter().all(|(_, line)| {
            let trimmed = line.trim_start();
            trimmed.starts_with(token)
        });

        // Save cursor/selection anchor so we can restore row positions after edits.
        let cursor_before = self.cursor().clone();
        let selection_before = self.selection().cloned();

        self.begin_undo_group();

        if all_commented {
            // Uncomment: remove `token` (and one trailing space if present) from each
            // non-blank line, iterating in reverse so earlier char indices stay valid.
            for (offset, line) in line_texts.iter().enumerate().rev() {
                if line.trim().is_empty() {
                    continue;
                }
                let row = start_row + offset;
                let indent_len = line.len() - line.trim_start().len();
                let after_indent = &line[indent_len..];
                if !after_indent.starts_with(token) {
                    continue;
                }
                let token_len = token.chars().count();
                // Swallow a single space after the token if present.
                let rest = &after_indent[token.len()..];
                let extra = if rest.starts_with(' ') { 1 } else { 0 };
                let start_col = indent_len;
                let end_col = indent_len + token_len + extra;
                self.apply_text_edit(row, start_col, row, end_col, "");
            }
        } else {
            // Comment: insert `token + " "` at the common minimum indent of non-blank lines.
            let min_indent = non_blank
                .iter()
                .map(|(_, line)| line.len() - line.trim_start().len())
                .min()
                .unwrap_or(0);
            let insertion = format!("{token} ");
            for (offset, line) in line_texts.iter().enumerate().rev() {
                if line.trim().is_empty() {
                    continue;
                }
                let row = start_row + offset;
                self.apply_text_edit(row, min_indent, row, min_indent, &insertion);
            }
        }

        self.end_undo_group();

        // Restore cursor row/selection row; `apply_text_edit` leaves cursor at the
        // position of the last edit, which is not what the user expects.
        self.cursor_mut().row = cursor_before
            .row
            .min(self.content_line_count().saturating_sub(1));
        let max_col = self.line_length(self.cursor().row);
        self.cursor_mut().col = cursor_before.col.min(max_col);
        self.cursor_mut().desired_col = self.cursor().col;
        if let Some(sel) = selection_before {
            self.set_selection(Some(sel));
        }
    }

    // IMPACT ANALYSIS — toggle_block_comment
    // Parents: KeyEvent → Command::ToggleBlockComment → app.execute() → this function
    // Children: Single apply_text_edit replacing the selection; history records one edit
    // Siblings: Selection (cleared by apply_text_edit), LSP didChange, tree-sitter reparse

    /// Toggles a block comment around the current selection.
    ///
    /// If the selection is empty, does nothing. If the selection already begins
    /// with `open` and ends with `close`, the wrapper is removed; otherwise the
    /// selection is wrapped with `open ... close`.
    pub fn toggle_block_comment(&mut self, open: &str, close: &str) {
        if open.is_empty() || close.is_empty() {
            return;
        }
        let Some(sel) = self.selection() else {
            return;
        };
        if sel.is_empty(self.cursor().row, self.cursor().col) {
            return;
        }
        let (sr, sc, er, ec) = sel.normalized(self.cursor().row, self.cursor().col);
        let start_idx = self.content.line_to_char(sr) + sc;
        let end_idx = self.content.line_to_char(er) + ec;
        if end_idx <= start_idx {
            return;
        }
        let selected: String = self.content.slice(start_idx..end_idx).into();
        let new_text = if selected.starts_with(open) && selected.ends_with(close) {
            // Unwrap.
            let inner = &selected[open.len()..selected.len() - close.len()];
            inner.to_string()
        } else {
            // Wrap.
            format!("{open}{selected}{close}")
        };
        self.apply_text_edit(sr, sc, er, ec, &new_text);
    }

    // IMPACT ANALYSIS — toggle_line_range
    // Parents: toggle_line_comment
    // Children: Returns (start_row, end_row) inclusive range based on selection/cursor
    // Siblings: None

    /// Returns the inclusive row range affected by a line-comment toggle.
    ///
    /// Uses the normalized selection if present; if the selection ends at column 0
    /// of a row, that row is excluded (matching VS Code / JetBrains behaviour).
    /// Falls back to the cursor row if there is no selection.
    fn toggle_line_range(&self) -> (usize, usize) {
        match self.selection() {
            Some(sel) if !sel.is_empty(self.cursor().row, self.cursor().col) => {
                let (sr, _sc, er, ec) = sel.normalized(self.cursor().row, self.cursor().col);
                let end = if ec == 0 && er > sr { er - 1 } else { er };
                (sr, end)
            }
            _ => (self.cursor().row, self.cursor().row),
        }
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
        self.set_selection(None);

        let total_lines = self.content.len_lines();
        let start_line = start_line.min(total_lines.saturating_sub(1));
        let end_line = end_line.min(total_lines.saturating_sub(1));

        let start_idx = self.content.line_to_char(start_line) + start_col;
        let end_idx = self.content.line_to_char(end_line) + end_col;

        // Clamp indices to content length.
        let total_chars = self.content.len_chars();
        let start_idx = start_idx.min(total_chars);
        let end_idx = end_idx.min(total_chars).max(start_idx);

        let cursor_before = self.cursor().clone();
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
        self.cursor_mut().row = row;
        self.cursor_mut().col = col;
        self.cursor_mut().desired_col = col;
        self.modified = true;

        self.history.record(
            Edit {
                char_idx: start_idx,
                old_text,
                new_text: new_text.to_string(),
            },
            cursor_before,
            self.cursor().clone(),
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
