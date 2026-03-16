use std::fs::File;
use std::io::BufReader;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use ropey::{Rope, RopeSlice};

use crate::cursor::CursorState;

/// Number of spaces inserted for a tab.
const TAB_WIDTH: usize = 4;

/// A single editor buffer holding file content as a rope.
///
/// Each buffer optionally tracks the file path it was loaded from,
/// whether it has been modified, and the cursor position.
pub struct EditorBuffer {
    content: Rope,
    path: Option<PathBuf>,
    /// Whether the buffer has unsaved modifications.
    pub modified: bool,
    /// Current cursor position within this buffer.
    pub cursor: CursorState,
    /// First visible line (vertical scroll offset).
    pub scroll_row: usize,
    /// First visible column (horizontal scroll offset).
    pub scroll_col: usize,
}

impl Default for EditorBuffer {
    fn default() -> Self {
        Self::new()
    }
}

impl EditorBuffer {
    /// Creates a new empty buffer with no associated file path.
    pub fn new() -> Self {
        Self {
            content: Rope::new(),
            path: None,
            modified: false,
            cursor: CursorState::default(),
            scroll_row: 0,
            scroll_col: 0,
        }
    }

    /// Loads a buffer from a file on disk.
    ///
    /// Returns an error if the file cannot be read.
    pub fn from_file(path: &Path) -> Result<Self> {
        let file =
            File::open(path).with_context(|| format!("Failed to open file: {}", path.display()))?;
        let content = Rope::from_reader(BufReader::new(file))
            .with_context(|| format!("Failed to read file: {}", path.display()))?;
        Ok(Self {
            content,
            path: Some(path.to_path_buf()),
            modified: false,
            cursor: CursorState::default(),
            scroll_row: 0,
            scroll_col: 0,
        })
    }

    /// Returns the number of lines in the buffer.
    pub fn line_count(&self) -> usize {
        self.content.len_lines()
    }

    /// Returns the file name (without directory) if a path is set.
    pub fn file_name(&self) -> Option<&str> {
        self.path
            .as_ref()
            .and_then(|p| p.file_name())
            .and_then(|n| n.to_str())
    }

    /// Returns a human-readable file type string based on the file extension.
    pub fn file_type(&self) -> &str {
        let ext = self
            .path
            .as_ref()
            .and_then(|p| p.extension())
            .and_then(|e| e.to_str())
            .unwrap_or("");

        match ext {
            "rs" => "Rust",
            "py" => "Python",
            "js" => "JavaScript",
            "ts" => "TypeScript",
            "jsx" => "JSX",
            "tsx" => "TSX",
            "go" => "Go",
            "c" => "C",
            "cpp" | "cc" | "cxx" => "C++",
            "h" | "hpp" => "C Header",
            "java" => "Java",
            "rb" => "Ruby",
            "swift" => "Swift",
            "kt" | "kts" => "Kotlin",
            "toml" => "TOML",
            "yaml" | "yml" => "YAML",
            "json" => "JSON",
            "md" => "Markdown",
            "html" => "HTML",
            "css" => "CSS",
            "sh" | "bash" | "zsh" | "fish" => "Shell",
            "sql" => "SQL",
            _ => "Plain Text",
        }
    }

    /// Returns the content of a specific line by zero-based index.
    ///
    /// Returns `None` if the index is out of bounds.
    pub fn line_at(&self, idx: usize) -> Option<RopeSlice<'_>> {
        if idx < self.content.len_lines() {
            Some(self.content.line(idx))
        } else {
            None
        }
    }

    /// Returns the file path if one is associated with this buffer.
    pub fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }

    /// Returns the number of content lines (excludes phantom trailing empty line from ropey).
    fn content_line_count(&self) -> usize {
        let total = self.content.len_lines();
        // Ropey adds a trailing empty line when the file ends with '\n'.
        if total > 1
            && self.content.len_chars() > 0
            && self.content.char(self.content.len_chars() - 1) == '\n'
        {
            total - 1
        } else {
            total
        }
    }

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

    // IMPACT ANALYSIS — insert_char
    // Parents: KeyEvent → Command::EditorInsertChar(ch) → this function
    // Children: UI renders updated content, cursor advances, modified flag set
    // Siblings: Selection (none yet), SyntaxHighlighter (future), LspClient (future)

    /// Inserts a character at the current cursor position.
    pub fn insert_char(&mut self, ch: char) {
        let char_idx = self.content.line_to_char(self.cursor.row) + self.cursor.col;
        self.content.insert_char(char_idx, ch);
        self.cursor.col += 1;
        self.cursor.desired_col = self.cursor.col;
        self.modified = true;
    }

    // IMPACT ANALYSIS — insert_newline
    // Parents: KeyEvent → Command::EditorNewline → this function
    // Children: Splits line, auto-indents from current line, cursor moves to new line
    // Siblings: Line count changes (affects gutter width, status bar line count)

    /// Inserts a newline at the current cursor position with auto-indent.
    pub fn insert_newline(&mut self) {
        let char_idx = self.content.line_to_char(self.cursor.row) + self.cursor.col;
        let indent = self.leading_whitespace(self.cursor.row);
        let insert_str = format!("\n{indent}");
        self.content.insert(char_idx, &insert_str);
        self.cursor.row += 1;
        self.cursor.col = indent.len();
        self.cursor.desired_col = self.cursor.col;
        self.modified = true;
    }

    // IMPACT ANALYSIS — insert_tab
    // Parents: KeyEvent → Command::EditorTab → this function
    // Children: Inserts TAB_WIDTH spaces, cursor advances
    // Siblings: Same as insert_char

    /// Inserts a tab (TAB_WIDTH spaces) at the current cursor position.
    pub fn insert_tab(&mut self) {
        let char_idx = self.content.line_to_char(self.cursor.row) + self.cursor.col;
        let spaces = " ".repeat(TAB_WIDTH);
        self.content.insert(char_idx, &spaces);
        self.cursor.col += TAB_WIDTH;
        self.cursor.desired_col = self.cursor.col;
        self.modified = true;
    }

    // IMPACT ANALYSIS — delete_char_backward
    // Parents: KeyEvent → Command::EditorBackspace → this function
    // Children: Removes char before cursor or joins lines, cursor moves back
    // Siblings: Line count may change (if joining lines), gutter width may change

    /// Deletes the character before the cursor (backspace).
    ///
    /// At the beginning of a line, joins with the previous line.
    /// At the beginning of the file, does nothing.
    pub fn delete_char_backward(&mut self) {
        if self.cursor.col > 0 {
            let char_idx = self.content.line_to_char(self.cursor.row) + self.cursor.col;
            self.content.remove(char_idx - 1..char_idx);
            self.cursor.col -= 1;
            self.cursor.desired_col = self.cursor.col;
            self.modified = true;
        } else if self.cursor.row > 0 {
            let prev_line_len = self.line_length(self.cursor.row - 1);
            let char_idx = self.content.line_to_char(self.cursor.row);
            // Remove \r\n or \n at end of previous line.
            let remove_start = if char_idx >= 2 && self.content.char(char_idx - 2) == '\r' {
                char_idx - 2
            } else {
                char_idx - 1
            };
            self.content.remove(remove_start..char_idx);
            self.cursor.row -= 1;
            self.cursor.col = prev_line_len;
            self.cursor.desired_col = self.cursor.col;
            self.modified = true;
        }
    }

    // IMPACT ANALYSIS — delete_char_forward
    // Parents: KeyEvent → Command::EditorDelete → this function
    // Children: Removes char at cursor or joins with next line, cursor stays
    // Siblings: Line count may change, gutter width may change

    /// Deletes the character at the cursor position (forward delete).
    ///
    /// At the end of a line, joins with the next line.
    /// At the end of the file, does nothing.
    pub fn delete_char_forward(&mut self) {
        let line_len = self.line_length(self.cursor.row);
        let char_idx = self.content.line_to_char(self.cursor.row) + self.cursor.col;
        if self.cursor.col < line_len {
            self.content.remove(char_idx..char_idx + 1);
            self.modified = true;
        } else if self.cursor.row + 1 < self.content_line_count() {
            // At end of line — join with next line by removing the newline.
            let remove_end =
                if char_idx < self.content.len_chars() && self.content.char(char_idx) == '\r' {
                    char_idx + 2
                } else {
                    char_idx + 1
                };
            self.content.remove(char_idx..remove_end);
            self.modified = true;
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

    /// Returns the leading whitespace of the given line as a string.
    fn leading_whitespace(&self, row: usize) -> String {
        self.line_at(row)
            .map(|line| {
                line.chars()
                    .take_while(|c| *c == ' ' || *c == '\t')
                    .collect()
            })
            .unwrap_or_default()
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

    /// Returns true if `c` is a word character (alphanumeric or underscore).
    fn is_word_char(c: char) -> bool {
        c.is_alphanumeric() || c == '_'
    }

    /// Adjusts scroll offsets to keep the cursor visible within the viewport.
    ///
    /// Maintains a margin of `SCROLL_MARGIN` lines from the viewport edges
    /// when possible.
    pub fn ensure_cursor_visible(&mut self, viewport_height: usize, viewport_width: usize) {
        const SCROLL_MARGIN: usize = 5;

        if viewport_height > 0 {
            // Vertical scrolling.
            if self.cursor.row < self.scroll_row + SCROLL_MARGIN {
                self.scroll_row = self.cursor.row.saturating_sub(SCROLL_MARGIN);
            }
            if viewport_height > SCROLL_MARGIN
                && self.cursor.row >= self.scroll_row + viewport_height - SCROLL_MARGIN
            {
                self.scroll_row = self.cursor.row + SCROLL_MARGIN + 1 - viewport_height;
            }
        }

        if viewport_width > 0 {
            // Horizontal scrolling.
            if self.cursor.col < self.scroll_col {
                self.scroll_col = self.cursor.col;
            }
            if self.cursor.col >= self.scroll_col + viewport_width {
                self.scroll_col = self.cursor.col + 1 - viewport_width;
            }
        }
    }

    /// Character count of the given line, excluding trailing newline/carriage return.
    ///
    /// Returns 0 if the line index is out of bounds.
    pub fn line_length(&self, row: usize) -> usize {
        if row >= self.content.len_lines() {
            return 0;
        }
        let line = self.content.line(row);
        let mut len = line.len_chars();
        // Strip trailing newline characters.
        if len > 0 && line.char(len - 1) == '\n' {
            len -= 1;
        }
        if len > 0 && line.char(len - 1) == '\r' {
            len -= 1;
        }
        len
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn new_empty_buffer() {
        let buf = EditorBuffer::new();
        assert!(buf.path().is_none());
        assert!(!buf.modified);
        // An empty rope has 1 line (the empty line).
        assert_eq!(buf.line_count(), 1);
    }

    #[test]
    fn from_file_loads_content() {
        let mut tmp = tempfile::NamedTempFile::new().unwrap();
        writeln!(tmp, "line1").unwrap();
        writeln!(tmp, "line2").unwrap();
        writeln!(tmp, "line3").unwrap();
        writeln!(tmp, "line4").unwrap();
        write!(tmp, "line5").unwrap();
        tmp.flush().unwrap();

        let buf = EditorBuffer::from_file(tmp.path()).unwrap();
        assert_eq!(buf.line_count(), 5);
        assert!(buf.path().is_some());
        assert!(!buf.modified);
    }

    #[test]
    fn from_file_nonexistent_returns_error() {
        let result = EditorBuffer::from_file(Path::new("/nonexistent/file/12345.txt"));
        assert!(result.is_err());
    }

    #[test]
    fn line_count_correct() {
        let mut tmp = tempfile::NamedTempFile::new().unwrap();
        writeln!(tmp, "a").unwrap();
        writeln!(tmp, "b").unwrap();
        write!(tmp, "c").unwrap();
        tmp.flush().unwrap();

        let buf = EditorBuffer::from_file(tmp.path()).unwrap();
        assert_eq!(buf.line_count(), 3);
    }

    #[test]
    fn file_name_from_path() {
        let mut tmp = tempfile::Builder::new().suffix(".rs").tempfile().unwrap();
        write!(tmp, "fn main() {{}}").unwrap();
        tmp.flush().unwrap();

        let buf = EditorBuffer::from_file(tmp.path()).unwrap();
        let name = buf.file_name().unwrap();
        assert!(name.ends_with(".rs"), "expected .rs extension, got {name}");
    }

    #[test]
    fn file_name_none_for_untitled() {
        let buf = EditorBuffer::new();
        assert!(buf.file_name().is_none());
    }

    #[test]
    fn file_type_known_extensions() {
        let cases = vec![
            ("test.rs", "Rust"),
            ("test.py", "Python"),
            ("test.js", "JavaScript"),
            ("test.ts", "TypeScript"),
            ("test.go", "Go"),
            ("test.toml", "TOML"),
            ("test.json", "JSON"),
            ("test.md", "Markdown"),
            ("test.html", "HTML"),
            ("test.css", "CSS"),
        ];

        for (filename, expected_type) in cases {
            let buf = EditorBuffer {
                content: Rope::new(),
                path: Some(PathBuf::from(filename)),
                modified: false,
                cursor: CursorState::default(),
                scroll_row: 0,
                scroll_col: 0,
            };
            assert_eq!(buf.file_type(), expected_type, "wrong type for {filename}");
        }
    }

    #[test]
    fn file_type_unknown_extension() {
        let buf = EditorBuffer {
            content: Rope::new(),
            path: Some(PathBuf::from("test.xyz")),
            modified: false,
            cursor: CursorState::default(),
            scroll_row: 0,
            scroll_col: 0,
        };
        assert_eq!(buf.file_type(), "Plain Text");
    }

    #[test]
    fn line_at_returns_correct() {
        let mut tmp = tempfile::NamedTempFile::new().unwrap();
        writeln!(tmp, "first").unwrap();
        writeln!(tmp, "second").unwrap();
        write!(tmp, "third").unwrap();
        tmp.flush().unwrap();

        let buf = EditorBuffer::from_file(tmp.path()).unwrap();

        let first = buf.line_at(0).unwrap().to_string();
        assert!(first.starts_with("first"), "got: {first}");

        let last = buf.line_at(2).unwrap().to_string();
        assert!(last.starts_with("third"), "got: {last}");
    }

    #[test]
    fn line_at_out_of_bounds() {
        let buf = EditorBuffer::new();
        assert!(buf.line_at(999).is_none());
    }

    #[test]
    fn line_length_returns_char_count() {
        let mut tmp = tempfile::NamedTempFile::new().unwrap();
        writeln!(tmp, "hello").unwrap();
        tmp.flush().unwrap();
        let buf = EditorBuffer::from_file(tmp.path()).unwrap();
        assert_eq!(buf.line_length(0), 5);
    }

    #[test]
    fn line_length_empty_line() {
        let mut tmp = tempfile::NamedTempFile::new().unwrap();
        writeln!(tmp, "").unwrap();
        write!(tmp, "x").unwrap();
        tmp.flush().unwrap();
        let buf = EditorBuffer::from_file(tmp.path()).unwrap();
        assert_eq!(buf.line_length(0), 0);
    }

    #[test]
    fn line_length_last_line_no_newline() {
        let mut tmp = tempfile::NamedTempFile::new().unwrap();
        write!(tmp, "abc").unwrap();
        tmp.flush().unwrap();
        let buf = EditorBuffer::from_file(tmp.path()).unwrap();
        assert_eq!(buf.line_length(0), 3);
    }

    #[test]
    fn line_length_out_of_bounds() {
        let buf = EditorBuffer::new();
        assert_eq!(buf.line_length(999), 0);
    }

    #[test]
    fn scroll_defaults_to_zero() {
        let buf = EditorBuffer::new();
        assert_eq!(buf.scroll_row, 0);
        assert_eq!(buf.scroll_col, 0);
    }

    // --- Cursor movement tests ---

    fn buffer_from_str(s: &str) -> EditorBuffer {
        let mut tmp = tempfile::NamedTempFile::new().unwrap();
        write!(tmp, "{s}").unwrap();
        tmp.flush().unwrap();
        EditorBuffer::from_file(tmp.path()).unwrap()
    }

    #[test]
    fn move_right_advances_col() {
        let mut buf = buffer_from_str("hello");
        buf.move_right();
        assert_eq!(buf.cursor.col, 1);
    }

    #[test]
    fn move_right_at_eol_wraps_to_next_line() {
        let mut buf = buffer_from_str("ab\ncd");
        buf.cursor.col = 2;
        buf.move_right();
        assert_eq!(buf.cursor.row, 1);
        assert_eq!(buf.cursor.col, 0);
    }

    #[test]
    fn move_right_at_eof_does_nothing() {
        let mut buf = buffer_from_str("ab\ncd");
        buf.cursor.row = 1;
        buf.cursor.col = 2;
        buf.move_right();
        assert_eq!(buf.cursor.row, 1);
        assert_eq!(buf.cursor.col, 2);
    }

    #[test]
    fn move_left_decreases_col() {
        let mut buf = buffer_from_str("hello");
        buf.cursor.col = 5;
        buf.move_left();
        assert_eq!(buf.cursor.col, 4);
    }

    #[test]
    fn move_left_at_bol_wraps_to_prev_line() {
        let mut buf = buffer_from_str("ab\ncd");
        buf.cursor.row = 1;
        buf.cursor.col = 0;
        buf.move_left();
        assert_eq!(buf.cursor.row, 0);
        assert_eq!(buf.cursor.col, 2);
    }

    #[test]
    fn move_left_at_bof_does_nothing() {
        let mut buf = buffer_from_str("hello");
        buf.move_left();
        assert_eq!(buf.cursor.row, 0);
        assert_eq!(buf.cursor.col, 0);
    }

    #[test]
    fn move_down_advances_row() {
        let mut buf = buffer_from_str("a\nb");
        buf.move_down();
        assert_eq!(buf.cursor.row, 1);
    }

    #[test]
    fn move_down_clamps_col_to_line_length() {
        let mut buf = buffer_from_str("hello\nab");
        buf.cursor.col = 5;
        buf.cursor.desired_col = 5;
        buf.move_down();
        assert_eq!(buf.cursor.row, 1);
        assert_eq!(buf.cursor.col, 2);
    }

    #[test]
    fn move_down_restores_desired_col() {
        let mut buf = buffer_from_str("hello\nab\nworld");
        buf.cursor.col = 4;
        buf.cursor.desired_col = 4;
        buf.move_down(); // row 1 "ab" -> col clamped to 2
        assert_eq!(buf.cursor.col, 2);
        buf.move_down(); // row 2 "world" -> col restored to 4
        assert_eq!(buf.cursor.row, 2);
        assert_eq!(buf.cursor.col, 4);
    }

    #[test]
    fn move_down_at_last_line_does_nothing() {
        let mut buf = buffer_from_str("only");
        buf.move_down();
        assert_eq!(buf.cursor.row, 0);
    }

    #[test]
    fn move_up_decreases_row() {
        let mut buf = buffer_from_str("a\nb");
        buf.cursor.row = 1;
        buf.move_up();
        assert_eq!(buf.cursor.row, 0);
    }

    #[test]
    fn move_up_restores_desired_col() {
        let mut buf = buffer_from_str("hello\nab\nworld");
        buf.cursor.row = 2;
        buf.cursor.col = 4;
        buf.cursor.desired_col = 4;
        buf.move_up(); // row 1 "ab" -> col clamped to 2
        assert_eq!(buf.cursor.col, 2);
        buf.move_up(); // row 0 "hello" -> col restored to 4
        assert_eq!(buf.cursor.row, 0);
        assert_eq!(buf.cursor.col, 4);
    }

    #[test]
    fn move_up_at_first_line_does_nothing() {
        let mut buf = buffer_from_str("only");
        buf.move_up();
        assert_eq!(buf.cursor.row, 0);
    }

    #[test]
    fn move_home_goes_to_col_zero() {
        let mut buf = buffer_from_str("hello");
        buf.cursor.col = 3;
        buf.move_home();
        assert_eq!(buf.cursor.col, 0);
    }

    #[test]
    fn move_end_goes_to_end_of_line() {
        let mut buf = buffer_from_str("hello\nworld");
        buf.move_end();
        assert_eq!(buf.cursor.col, 5);
    }

    #[test]
    fn move_file_start_goes_to_0_0() {
        let mut buf = buffer_from_str("hello\nworld");
        buf.cursor.row = 1;
        buf.cursor.col = 3;
        buf.move_file_start();
        assert_eq!(buf.cursor.row, 0);
        assert_eq!(buf.cursor.col, 0);
    }

    #[test]
    fn move_file_end_goes_to_last_line_end() {
        let mut buf = buffer_from_str("hello\nworld");
        buf.move_file_end();
        assert_eq!(buf.cursor.row, 1);
        assert_eq!(buf.cursor.col, 5);
    }

    #[test]
    fn move_page_down_moves_by_viewport() {
        let mut buf = buffer_from_str(&"line\n".repeat(50));
        buf.move_page_down(10);
        assert_eq!(buf.cursor.row, 10);
    }

    #[test]
    fn move_page_down_clamps_to_last_line() {
        let mut buf = buffer_from_str("a\nb\nc");
        buf.move_page_down(100);
        assert_eq!(buf.cursor.row, 2);
    }

    #[test]
    fn move_page_up_moves_by_viewport() {
        let mut buf = buffer_from_str(&"line\n".repeat(50));
        buf.cursor.row = 30;
        buf.move_page_up(10);
        assert_eq!(buf.cursor.row, 20);
    }

    #[test]
    fn move_page_up_clamps_to_zero() {
        let mut buf = buffer_from_str("a\nb\nc");
        buf.cursor.row = 1;
        buf.move_page_up(100);
        assert_eq!(buf.cursor.row, 0);
    }

    #[test]
    fn move_word_right_skips_word() {
        let mut buf = buffer_from_str("hello world");
        buf.move_word_right();
        assert_eq!(buf.cursor.col, 6);
    }

    #[test]
    fn move_word_right_at_eol_wraps() {
        let mut buf = buffer_from_str("hello\nworld");
        buf.cursor.col = 5;
        buf.move_word_right();
        assert_eq!(buf.cursor.row, 1);
        assert_eq!(buf.cursor.col, 0);
    }

    #[test]
    fn move_word_left_skips_word() {
        let mut buf = buffer_from_str("hello world");
        buf.cursor.col = 11;
        buf.move_word_left();
        assert_eq!(buf.cursor.col, 6);
    }

    #[test]
    fn move_word_left_at_bol_wraps() {
        let mut buf = buffer_from_str("hello\nworld");
        buf.cursor.row = 1;
        buf.cursor.col = 0;
        buf.move_word_left();
        assert_eq!(buf.cursor.row, 0);
        assert_eq!(buf.cursor.col, 5);
    }

    // --- ensure_cursor_visible tests ---

    #[test]
    fn ensure_visible_scrolls_down_when_cursor_below() {
        let mut buf = buffer_from_str(&"line\n".repeat(50));
        buf.cursor.row = 30;
        buf.ensure_cursor_visible(20, 80);
        // cursor.row (30) >= scroll_row + viewport_height(20) - margin(5)
        // scroll_row = 30 + 5 + 1 - 20 = 16
        assert_eq!(buf.scroll_row, 16);
    }

    #[test]
    fn ensure_visible_scrolls_up_when_cursor_above() {
        let mut buf = buffer_from_str(&"line\n".repeat(50));
        buf.scroll_row = 10;
        buf.cursor.row = 2;
        buf.ensure_cursor_visible(20, 80);
        // cursor.row (2) < scroll_row(10) + margin(5) => scroll_row = 2 - 5 = 0
        assert_eq!(buf.scroll_row, 0);
    }

    #[test]
    fn ensure_visible_no_change_when_in_view() {
        let mut buf = buffer_from_str(&"line\n".repeat(50));
        buf.scroll_row = 5;
        buf.cursor.row = 12;
        buf.ensure_cursor_visible(20, 80);
        assert_eq!(buf.scroll_row, 5);
    }

    #[test]
    fn ensure_visible_horizontal_scroll() {
        let mut buf = buffer_from_str(&"x".repeat(200));
        buf.cursor.col = 100;
        buf.ensure_cursor_visible(20, 80);
        // cursor.col (100) >= scroll_col(0) + viewport_width(80)
        // scroll_col = 100 + 1 - 80 = 21
        assert_eq!(buf.scroll_col, 21);
    }

    // --- insert_char tests ---

    #[test]
    fn insert_char_at_start() {
        let mut buf = buffer_from_str("hello");
        buf.insert_char('x');
        assert_eq!(buf.line_at(0).unwrap().to_string(), "xhello");
        assert_eq!(buf.cursor.col, 1);
    }

    #[test]
    fn insert_char_in_middle() {
        let mut buf = buffer_from_str("hello");
        buf.cursor.col = 3;
        buf.insert_char('x');
        assert_eq!(buf.line_at(0).unwrap().to_string(), "helxlo");
        assert_eq!(buf.cursor.col, 4);
    }

    #[test]
    fn insert_char_at_end() {
        let mut buf = buffer_from_str("hello");
        buf.cursor.col = 5;
        buf.insert_char('x');
        assert_eq!(buf.line_at(0).unwrap().to_string(), "hellox");
        assert_eq!(buf.cursor.col, 6);
    }

    #[test]
    fn insert_char_sets_modified() {
        let mut buf = buffer_from_str("hello");
        assert!(!buf.modified);
        buf.insert_char('x');
        assert!(buf.modified);
    }

    #[test]
    fn insert_char_in_empty_buffer() {
        let mut buf = EditorBuffer::new();
        buf.insert_char('a');
        assert_eq!(buf.line_at(0).unwrap().to_string(), "a");
        assert_eq!(buf.cursor.col, 1);
    }

    // --- insert_newline tests ---

    #[test]
    fn newline_splits_line() {
        let mut buf = buffer_from_str("hello");
        buf.cursor.col = 3;
        buf.insert_newline();
        assert_eq!(buf.line_at(0).unwrap().to_string(), "hel\n");
        assert_eq!(buf.line_at(1).unwrap().to_string(), "lo");
        assert_eq!(buf.cursor.row, 1);
        assert_eq!(buf.cursor.col, 0);
    }

    #[test]
    fn newline_at_start() {
        let mut buf = buffer_from_str("hello");
        buf.insert_newline();
        assert_eq!(buf.line_at(0).unwrap().to_string(), "\n");
        assert!(buf.line_at(1).unwrap().to_string().starts_with("hello"));
        assert_eq!(buf.cursor.row, 1);
        assert_eq!(buf.cursor.col, 0);
    }

    #[test]
    fn newline_at_end() {
        let mut buf = buffer_from_str("hello");
        buf.cursor.col = 5;
        buf.insert_newline();
        assert_eq!(buf.line_at(0).unwrap().to_string(), "hello\n");
        assert_eq!(buf.cursor.row, 1);
        assert_eq!(buf.cursor.col, 0);
    }

    #[test]
    fn newline_auto_indents() {
        let mut buf = buffer_from_str("  hello");
        buf.cursor.col = 6;
        buf.insert_newline();
        assert_eq!(buf.line_at(0).unwrap().to_string(), "  hell\n");
        assert_eq!(buf.line_at(1).unwrap().to_string(), "  o");
        assert_eq!(buf.cursor.row, 1);
        assert_eq!(buf.cursor.col, 2);
    }

    #[test]
    fn newline_sets_modified() {
        let mut buf = buffer_from_str("hello");
        assert!(!buf.modified);
        buf.insert_newline();
        assert!(buf.modified);
    }

    // --- insert_tab tests ---

    #[test]
    fn tab_inserts_spaces() {
        let mut buf = buffer_from_str("hello");
        buf.insert_tab();
        assert_eq!(buf.line_at(0).unwrap().to_string(), "    hello");
        assert_eq!(buf.cursor.col, 4);
    }

    #[test]
    fn tab_sets_modified() {
        let mut buf = buffer_from_str("hello");
        assert!(!buf.modified);
        buf.insert_tab();
        assert!(buf.modified);
    }

    // --- delete_char_backward (backspace) tests ---

    #[test]
    fn backspace_deletes_char() {
        let mut buf = buffer_from_str("hello");
        buf.cursor.col = 3;
        buf.delete_char_backward();
        assert_eq!(buf.line_at(0).unwrap().to_string(), "helo");
        assert_eq!(buf.cursor.col, 2);
    }

    #[test]
    fn backspace_at_bof_noop() {
        let mut buf = buffer_from_str("hello");
        buf.delete_char_backward();
        assert_eq!(buf.line_at(0).unwrap().to_string(), "hello");
        assert_eq!(buf.cursor.col, 0);
        assert!(!buf.modified);
    }

    #[test]
    fn backspace_joins_lines() {
        let mut buf = buffer_from_str("hello\nworld");
        buf.cursor.row = 1;
        buf.cursor.col = 0;
        buf.delete_char_backward();
        assert_eq!(buf.line_at(0).unwrap().to_string(), "helloworld");
        assert_eq!(buf.cursor.row, 0);
        assert_eq!(buf.cursor.col, 5);
    }

    #[test]
    fn backspace_sets_modified() {
        let mut buf = buffer_from_str("hello");
        buf.cursor.col = 1;
        assert!(!buf.modified);
        buf.delete_char_backward();
        assert!(buf.modified);
    }

    // --- delete_char_forward (delete) tests ---

    #[test]
    fn delete_forward_deletes_char() {
        let mut buf = buffer_from_str("hello");
        buf.cursor.col = 2;
        buf.delete_char_forward();
        assert_eq!(buf.line_at(0).unwrap().to_string(), "helo");
        assert_eq!(buf.cursor.col, 2);
    }

    #[test]
    fn delete_forward_at_eof_noop() {
        let mut buf = buffer_from_str("hello");
        buf.cursor.col = 5;
        buf.delete_char_forward();
        assert_eq!(buf.line_at(0).unwrap().to_string(), "hello");
        assert!(!buf.modified);
    }

    #[test]
    fn delete_forward_joins_lines() {
        let mut buf = buffer_from_str("hello\nworld");
        buf.cursor.col = 5;
        buf.delete_char_forward();
        assert_eq!(buf.line_at(0).unwrap().to_string(), "helloworld");
        assert_eq!(buf.cursor.row, 0);
        assert_eq!(buf.cursor.col, 5);
    }

    #[test]
    fn delete_forward_sets_modified() {
        let mut buf = buffer_from_str("hello");
        assert!(!buf.modified);
        buf.delete_char_forward();
        assert!(buf.modified);
    }

    // --- save_to_file tests ---

    #[test]
    fn save_writes_content() {
        let mut tmp = tempfile::NamedTempFile::new().unwrap();
        write!(tmp, "original").unwrap();
        tmp.flush().unwrap();

        let mut buf = EditorBuffer::from_file(tmp.path()).unwrap();
        buf.insert_char('X');
        buf.save_to_file().unwrap();

        let content = std::fs::read_to_string(tmp.path()).unwrap();
        assert_eq!(content, "Xoriginal");
    }

    #[test]
    fn save_clears_modified() {
        let mut tmp = tempfile::NamedTempFile::new().unwrap();
        write!(tmp, "data").unwrap();
        tmp.flush().unwrap();

        let mut buf = EditorBuffer::from_file(tmp.path()).unwrap();
        buf.insert_char('x');
        assert!(buf.modified);
        buf.save_to_file().unwrap();
        assert!(!buf.modified);
    }

    #[test]
    fn save_no_path_returns_error() {
        let mut buf = EditorBuffer::new();
        buf.insert_char('x');
        assert!(buf.save_to_file().is_err());
    }

    #[test]
    fn save_is_atomic() {
        let mut tmp = tempfile::NamedTempFile::new().unwrap();
        write!(tmp, "safe").unwrap();
        tmp.flush().unwrap();

        let mut buf = EditorBuffer::from_file(tmp.path()).unwrap();
        buf.cursor.col = 4;
        buf.insert_char('!');
        buf.save_to_file().unwrap();

        let content = std::fs::read_to_string(tmp.path()).unwrap();
        assert_eq!(content, "safe!");
    }
}
