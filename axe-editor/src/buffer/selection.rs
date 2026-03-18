use crate::highlight;
use crate::history::Edit;
use crate::selection::Selection;

use super::EditorBuffer;

// --- Selection methods ---

// IMPACT ANALYSIS — Selection methods
// Parents: KeyEvent → EditorSelect* commands → app.execute() → these methods
// Children: UI renders selection highlight, clipboard ops read selected_text()
// Siblings: Cursor movement (move_* methods stay pure, clear_selection called from app),
//           Edit methods (insert_char, etc.) must delete selection first,
//           Undo/redo (delete_selection records Edit)

impl EditorBuffer {
    /// Sets the selection anchor to the current cursor position if no selection exists.
    pub fn start_or_extend_selection(&mut self) {
        if self.selection.is_none() {
            self.selection = Some(Selection {
                anchor_row: self.cursor.row,
                anchor_col: self.cursor.col,
            });
        }
    }

    /// Clears the current selection.
    pub fn clear_selection(&mut self) {
        self.selection = None;
    }

    /// Extends selection rightward by one character.
    pub fn select_right(&mut self) {
        self.start_or_extend_selection();
        self.move_right();
    }

    /// Extends selection leftward by one character.
    pub fn select_left(&mut self) {
        self.start_or_extend_selection();
        self.move_left();
    }

    /// Extends selection upward by one line.
    pub fn select_up(&mut self) {
        self.start_or_extend_selection();
        self.move_up();
    }

    /// Extends selection downward by one line.
    pub fn select_down(&mut self) {
        self.start_or_extend_selection();
        self.move_down();
    }

    /// Extends selection to the beginning of the current line.
    pub fn select_home(&mut self) {
        self.start_or_extend_selection();
        self.move_home();
    }

    /// Extends selection to the end of the current line.
    pub fn select_end(&mut self) {
        self.start_or_extend_selection();
        self.move_end();
    }

    /// Extends selection to the beginning of the file.
    pub fn select_file_start(&mut self) {
        self.start_or_extend_selection();
        self.move_file_start();
    }

    /// Extends selection to the end of the file.
    pub fn select_file_end(&mut self) {
        self.start_or_extend_selection();
        self.move_file_end();
    }

    /// Extends selection to the next word boundary.
    pub fn select_word_right(&mut self) {
        self.start_or_extend_selection();
        self.move_word_right();
    }

    /// Extends selection to the previous word boundary.
    pub fn select_word_left(&mut self) {
        self.start_or_extend_selection();
        self.move_word_left();
    }

    /// Selects all text in the buffer.
    pub fn select_all(&mut self) {
        self.selection = Some(Selection {
            anchor_row: 0,
            anchor_col: 0,
        });
        self.move_file_end();
    }

    /// Returns the selected text, or `None` if there is no selection.
    pub fn selected_text(&self) -> Option<String> {
        let sel = self.selection.as_ref()?;
        if sel.is_empty(self.cursor.row, self.cursor.col) {
            return None;
        }
        let (start_row, start_col, end_row, end_col) =
            sel.normalized(self.cursor.row, self.cursor.col);
        let start_idx = self.content.line_to_char(start_row) + start_col;
        let end_idx = self.content.line_to_char(end_row) + end_col;
        Some(self.content.slice(start_idx..end_idx).to_string())
    }

    /// Deletes the selected text, records an undo edit, and returns the deleted text.
    ///
    /// Returns `None` if there is no selection. Moves cursor to the start of the
    /// deleted range and clears the selection.
    pub fn delete_selection(&mut self) -> Option<String> {
        let sel = self.selection.as_ref()?;
        if sel.is_empty(self.cursor.row, self.cursor.col) {
            self.selection = None;
            return None;
        }
        let (start_row, start_col, end_row, end_col) =
            sel.normalized(self.cursor.row, self.cursor.col);
        let start_idx = self.content.line_to_char(start_row) + start_col;
        let end_idx = self.content.line_to_char(end_row) + end_col;
        let deleted: String = self.content.slice(start_idx..end_idx).to_string();
        let old_byte_len = deleted.len();
        let chars_deleted = end_idx - start_idx;
        let old_end_pos =
            highlight::byte_to_point(&self.content, self.content.char_to_byte(end_idx));

        let cursor_before = self.cursor.clone();
        self.content.remove(start_idx..end_idx);
        self.cursor.row = start_row;
        self.cursor.col = start_col;
        self.cursor.desired_col = start_col;
        self.selection = None;
        self.modified = true;
        self.history.record(
            Edit {
                char_idx: start_idx,
                old_text: deleted.clone(),
                new_text: String::new(),
            },
            cursor_before,
            self.cursor.clone(),
        );
        self.notify_highlight_delete(start_idx, chars_deleted, old_byte_len, old_end_pos);
        Some(deleted)
    }

    /// Selects the word at the current cursor position.
    ///
    /// Uses `is_word_char()` to find word boundaries around the cursor.
    /// Does nothing if the cursor is on whitespace, past line end, or on an empty line.
    pub fn select_word_at_cursor(&mut self) {
        let line_len = self.line_length(self.cursor.row);
        if line_len == 0 || self.cursor.col >= line_len {
            return;
        }

        let line = self.content.line(self.cursor.row);
        let chars: Vec<char> = line.chars().take(line_len).collect();

        if !Self::is_word_char(chars[self.cursor.col]) {
            return;
        }

        // Find word start.
        let mut start = self.cursor.col;
        while start > 0 && Self::is_word_char(chars[start - 1]) {
            start -= 1;
        }

        // Find word end.
        let mut end = self.cursor.col;
        while end < chars.len() && Self::is_word_char(chars[end]) {
            end += 1;
        }

        self.selection = Some(Selection {
            anchor_row: self.cursor.row,
            anchor_col: start,
        });
        self.cursor.col = end;
        self.cursor.desired_col = end;
    }

    /// Selects the entire line at the current cursor position.
    ///
    /// Sets anchor to column 0 and moves cursor to end of line.
    pub fn select_line_at_cursor(&mut self) {
        let line_len = self.line_length(self.cursor.row);
        self.selection = Some(Selection {
            anchor_row: self.cursor.row,
            anchor_col: 0,
        });
        self.cursor.col = line_len;
        self.cursor.desired_col = line_len;
    }
}
