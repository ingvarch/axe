use super::EditorBuffer;

impl EditorBuffer {
    /// Move cursor right one character. Wraps to next line at end-of-line.
    pub fn move_right(&mut self) {
        let line_len = self.line_length(self.cursor.row);
        if self.cursor.col < line_len {
            self.cursor.col += 1;
        } else if self.cursor.row + 1 < self.content_line_count() {
            self.cursor.row += 1;
            self.cursor.col = 0;
        }
        self.cursor.desired_col = self.cursor.col;
    }

    /// Move cursor left one character. Wraps to previous line at beginning-of-line.
    pub fn move_left(&mut self) {
        if self.cursor.col > 0 {
            self.cursor.col -= 1;
        } else if self.cursor.row > 0 {
            self.cursor.row -= 1;
            self.cursor.col = self.line_length(self.cursor.row);
        }
        self.cursor.desired_col = self.cursor.col;
    }

    /// Move cursor down one line, using `desired_col` for stickiness.
    pub fn move_down(&mut self) {
        if self.cursor.row + 1 < self.content_line_count() {
            // Update desired_col only on first vertical move (when col == desired_col).
            if self.cursor.col == self.cursor.desired_col {
                self.cursor.desired_col = self.cursor.col;
            }
            self.cursor.row += 1;
            let line_len = self.line_length(self.cursor.row);
            self.cursor.col = self.cursor.desired_col.min(line_len);
        }
    }

    /// Move cursor up one line, using `desired_col` for stickiness.
    pub fn move_up(&mut self) {
        if self.cursor.row > 0 {
            if self.cursor.col == self.cursor.desired_col {
                self.cursor.desired_col = self.cursor.col;
            }
            self.cursor.row -= 1;
            let line_len = self.line_length(self.cursor.row);
            self.cursor.col = self.cursor.desired_col.min(line_len);
        }
    }

    /// Move cursor to beginning of current line.
    pub fn move_home(&mut self) {
        self.cursor.col = 0;
        self.cursor.desired_col = 0;
    }

    /// Move cursor to end of current line.
    pub fn move_end(&mut self) {
        self.cursor.col = self.line_length(self.cursor.row);
        self.cursor.desired_col = self.cursor.col;
    }

    /// Move cursor to beginning of file (row 0, col 0).
    pub fn move_file_start(&mut self) {
        self.cursor.row = 0;
        self.cursor.col = 0;
        self.cursor.desired_col = 0;
    }

    /// Move cursor to end of file (last line, end of line).
    pub fn move_file_end(&mut self) {
        let last = self.content_line_count().saturating_sub(1);
        self.cursor.row = last;
        self.cursor.col = self.line_length(last);
        self.cursor.desired_col = self.cursor.col;
    }

    /// Move cursor down by one page (viewport_height lines).
    pub fn move_page_down(&mut self, viewport_height: usize) {
        let max_row = self.content_line_count().saturating_sub(1);
        self.cursor.row = (self.cursor.row + viewport_height).min(max_row);
        let line_len = self.line_length(self.cursor.row);
        self.cursor.col = self.cursor.desired_col.min(line_len);
    }

    /// Move cursor up by one page (viewport_height lines).
    pub fn move_page_up(&mut self, viewport_height: usize) {
        self.cursor.row = self.cursor.row.saturating_sub(viewport_height);
        let line_len = self.line_length(self.cursor.row);
        self.cursor.col = self.cursor.desired_col.min(line_len);
    }

    /// Move cursor to the next word boundary.
    pub fn move_word_right(&mut self) {
        let total_lines = self.content_line_count();
        let mut row = self.cursor.row;
        let mut col = self.cursor.col;

        loop {
            if row >= total_lines {
                break;
            }
            let line_len = self.line_length(row);
            let line = self.content.line(row);
            let chars: Vec<char> = line.chars().take(line_len).collect();

            if col < chars.len() {
                // Skip current category.
                let at_word = Self::is_word_char(chars[col]);
                while col < chars.len() && Self::is_word_char(chars[col]) == at_word {
                    col += 1;
                }

                // Skip whitespace/non-word to find next word start.
                while col < chars.len() && !Self::is_word_char(chars[col]) {
                    col += 1;
                }

                if col <= line_len {
                    self.cursor.row = row;
                    self.cursor.col = col;
                    self.cursor.desired_col = col;
                    return;
                }
            }

            // At or past end of line — wrap to next line.
            row += 1;
            col = 0;
            if row < total_lines {
                self.cursor.row = row;
                self.cursor.col = 0;
                self.cursor.desired_col = 0;
                return;
            }
        }

        // At end of file — go to end of last line.
        let last = total_lines.saturating_sub(1);
        self.cursor.row = last;
        self.cursor.col = self.line_length(last);
        self.cursor.desired_col = self.cursor.col;
    }

    /// Move cursor to the previous word boundary.
    pub fn move_word_left(&mut self) {
        let row = self.cursor.row;
        let col = self.cursor.col;

        if col > 0 {
            let line_len = self.line_length(row);
            let line = self.content.line(row);
            let chars: Vec<char> = line.chars().take(line_len).collect();
            let mut c = col - 1;
            // Skip whitespace/non-word backwards.
            while c > 0 && !Self::is_word_char(chars[c]) {
                c -= 1;
            }
            // Skip word chars backwards to find word start.
            if Self::is_word_char(chars[c]) {
                while c > 0 && Self::is_word_char(chars[c - 1]) {
                    c -= 1;
                }
            }
            self.cursor.col = c;
            self.cursor.desired_col = c;
        } else if row > 0 {
            // At beginning of line — wrap to end of previous line.
            let new_row = row - 1;
            let new_col = self.line_length(new_row);
            self.cursor.row = new_row;
            self.cursor.col = new_col;
            self.cursor.desired_col = new_col;
        }
    }
}
