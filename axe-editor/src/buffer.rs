use std::fs::File;
use std::io::BufReader;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use ropey::{Rope, RopeSlice};

use crate::cursor::CursorState;
use crate::diagnostic::BufferDiagnostic;
use crate::highlight::{self, HighlightSpan, HighlightState};
use crate::history::{Edit, EditHistory};
use crate::selection::Selection;

/// Default number of spaces inserted for a tab.
const DEFAULT_TAB_SIZE: usize = 4;

/// A single editor buffer holding file content as a rope.
///
/// Each buffer optionally tracks the file path it was loaded from,
/// whether it has been modified, and the cursor position.
pub struct EditorBuffer {
    content: Rope,
    path: Option<PathBuf>,
    /// Whether the buffer has unsaved modifications.
    pub modified: bool,
    /// Whether this buffer is a preview (single-click open, replaced by next preview).
    pub is_preview: bool,
    /// Current cursor position within this buffer.
    pub cursor: CursorState,
    /// First visible line (vertical scroll offset).
    pub scroll_row: usize,
    /// First visible column (horizontal scroll offset).
    pub scroll_col: usize,
    /// Edit history for undo/redo.
    history: EditHistory,
    /// Current text selection, if any.
    pub selection: Option<Selection>,
    /// Syntax highlighting state, if the file type is supported.
    highlight: Option<HighlightState>,
    /// LSP diagnostics for this buffer (errors, warnings, etc.).
    diagnostics: Vec<BufferDiagnostic>,
    /// Number of spaces (or columns) per tab stop.
    tab_size: usize,
    /// Whether Tab key inserts spaces (`true`) or a literal tab character (`false`).
    insert_spaces: bool,
}

impl Default for EditorBuffer {
    fn default() -> Self {
        Self::new()
    }
}

impl EditorBuffer {
    /// Creates a new empty buffer with no associated file path.
    ///
    /// Uses default tab configuration (4 spaces, insert spaces mode).
    pub fn new() -> Self {
        Self {
            content: Rope::new(),
            path: None,
            modified: false,
            is_preview: false,
            cursor: CursorState::default(),
            scroll_row: 0,
            scroll_col: 0,
            history: EditHistory::new(),
            selection: None,
            highlight: None,
            diagnostics: Vec::new(),
            tab_size: DEFAULT_TAB_SIZE,
            insert_spaces: true,
        }
    }

    /// Creates a new empty buffer with custom tab configuration.
    pub fn with_tab_config(tab_size: usize, insert_spaces: bool) -> Self {
        Self {
            tab_size,
            insert_spaces,
            ..Self::new()
        }
    }

    /// Returns the configured tab size.
    pub fn tab_size(&self) -> usize {
        self.tab_size
    }

    /// Returns whether the buffer inserts spaces for tabs.
    pub fn insert_spaces(&self) -> bool {
        self.insert_spaces
    }

    /// Sets the tab configuration for this buffer.
    pub fn set_tab_config(&mut self, tab_size: usize, insert_spaces: bool) {
        self.tab_size = tab_size;
        self.insert_spaces = insert_spaces;
    }

    /// Returns the current diagnostics for this buffer.
    pub fn diagnostics(&self) -> &[BufferDiagnostic] {
        &self.diagnostics
    }

    /// Replaces all diagnostics for this buffer.
    pub fn set_diagnostics(&mut self, diags: Vec<BufferDiagnostic>) {
        self.diagnostics = diags;
    }

    /// Removes all diagnostics from this buffer.
    pub fn clear_diagnostics(&mut self) {
        self.diagnostics.clear();
    }

    /// Loads a buffer from a file on disk.
    ///
    /// Returns an error if the file cannot be read.
    pub fn from_file(path: &Path) -> Result<Self> {
        let file =
            File::open(path).with_context(|| format!("Failed to open file: {}", path.display()))?;
        let content = Rope::from_reader(BufReader::new(file))
            .with_context(|| format!("Failed to read file: {}", path.display()))?;

        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
        let mut highlight = HighlightState::new(ext);
        if let Some(hl) = highlight.as_mut() {
            hl.parse_full(&content);
        }

        Ok(Self {
            content,
            path: Some(path.to_path_buf()),
            modified: false,
            is_preview: false,
            cursor: CursorState::default(),
            scroll_row: 0,
            scroll_col: 0,
            history: EditHistory::new(),
            selection: None,
            highlight,
            diagnostics: Vec::new(),
            tab_size: DEFAULT_TAB_SIZE,
            insert_spaces: true,
        })
    }

    /// Returns the number of lines in the buffer.
    pub fn line_count(&self) -> usize {
        self.content.len_lines()
    }

    /// Scrolls the viewport by the given number of lines without moving the cursor.
    ///
    /// Positive delta scrolls down (later lines become visible),
    /// negative scrolls up (earlier lines become visible).
    /// Clamped to valid bounds.
    pub fn scroll_by(&mut self, delta: i32, viewport_height: usize) {
        let max_scroll = self.line_count().saturating_sub(viewport_height);
        if delta > 0 {
            self.scroll_row = (self.scroll_row + delta as usize).min(max_scroll);
        } else {
            self.scroll_row = self.scroll_row.saturating_sub((-delta) as usize);
        }
    }

    /// Scrolls the viewport horizontally by the given number of columns
    /// without moving the cursor.
    ///
    /// Positive delta scrolls right, negative scrolls left.
    /// Clamped to zero on the left; no upper bound (long lines may extend arbitrarily).
    pub fn scroll_horizontally_by(&mut self, delta: i32) {
        if delta > 0 {
            self.scroll_col += delta as usize;
        } else {
            self.scroll_col = self.scroll_col.saturating_sub((-delta) as usize);
        }
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

    /// Returns the full buffer content as a `String`.
    ///
    /// Used by the LSP subsystem for full-document synchronization.
    pub fn content_string(&self) -> String {
        self.content.to_string()
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

    /// Returns highlight spans for the given line range `[start, end)`.
    ///
    /// Each element in the returned `Vec` corresponds to one line and
    /// contains the spans for that line. Returns an empty set of spans
    /// per line if no highlighting is available.
    pub fn highlight_range(&self, start: usize, end: usize) -> Vec<Vec<HighlightSpan>> {
        match self.highlight.as_ref() {
            Some(hl) => hl.highlights_for_range(start, end, &self.content),
            None => vec![Vec::new(); end.saturating_sub(start)],
        }
    }

    /// Notifies the tree-sitter highlighter about a text edit and re-parses.
    ///
    /// Must be called AFTER the rope has been mutated. The `start_char`,
    /// `old_end_char` refer to positions in the OLD rope (before the edit),
    /// and `new_end_char` refers to the position in the NEW rope.
    fn notify_highlight_insert(&mut self, start_char: usize, chars_inserted: usize) {
        if let Some(hl) = self.highlight.as_mut() {
            // Build a snapshot of the old rope state for InputEdit.
            // Since the rope has already been mutated, we reconstruct old positions.
            let new_end_char = start_char + chars_inserted;
            let start_byte = self
                .content
                .char_to_byte(start_char.min(self.content.len_chars()));
            let new_end_byte = self
                .content
                .char_to_byte(new_end_char.min(self.content.len_chars()));

            let start_position = highlight::byte_to_point(&self.content, start_byte);
            // Old end = start (it was an insertion, no old text removed).
            let old_end_position = start_position;
            let new_end_position = highlight::byte_to_point(&self.content, new_end_byte);

            let edit = tree_sitter::InputEdit {
                start_byte,
                old_end_byte: start_byte,
                new_end_byte,
                start_position,
                old_end_position,
                new_end_position,
            };
            hl.edit_and_reparse(&edit, &self.content);
        }
    }

    /// Notifies the tree-sitter highlighter about a deletion and re-parses.
    ///
    /// `start_char` is the char index where deletion starts (in both old and new),
    /// `chars_deleted` is how many chars were removed.
    fn notify_highlight_delete(
        &mut self,
        start_char: usize,
        _chars_deleted: usize,
        old_bytes: usize,
        old_end_position: tree_sitter::Point,
    ) {
        if let Some(hl) = self.highlight.as_mut() {
            let start_byte = self
                .content
                .char_to_byte(start_char.min(self.content.len_chars()));
            let start_position = highlight::byte_to_point(&self.content, start_byte);

            let edit = tree_sitter::InputEdit {
                start_byte,
                old_end_byte: start_byte + old_bytes,
                new_end_byte: start_byte,
                start_position,
                old_end_position,
                new_end_position: start_position,
            };
            hl.edit_and_reparse(&edit, &self.content);
        }
    }

    /// Re-parses the full content for highlight after undo/redo.
    fn reparse_highlight_full(&mut self) {
        if let Some(hl) = self.highlight.as_mut() {
            hl.parse_full(&self.content);
        }
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

    // --- Selection methods ---

    // IMPACT ANALYSIS — Selection methods
    // Parents: KeyEvent → EditorSelect* commands → app.execute() → these methods
    // Children: UI renders selection highlight, clipboard ops read selected_text()
    // Siblings: Cursor movement (move_* methods stay pure, clear_selection called from app),
    //           Edit methods (insert_char, etc.) must delete selection first,
    //           Undo/redo (delete_selection records Edit)

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

    /// Returns the text content of a line as a `String`, excluding trailing newline.
    ///
    /// Returns an empty string if the line index is out of bounds.
    pub fn line_text(&self, row: usize) -> String {
        match self.line_at(row) {
            Some(slice) => {
                let s = slice.to_string();
                s.trim_end_matches('\n').trim_end_matches('\r').to_string()
            }
            None => String::new(),
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn new_empty_buffer() {
        let buf = EditorBuffer::new();
        assert!(buf.path().is_none());
        assert!(!buf.modified);
        assert!(!buf.is_preview);
        // An empty rope has 1 line (the empty line).
        assert_eq!(buf.line_count(), 1);
    }

    #[test]
    fn preview_flag_defaults_to_false() {
        let buf = EditorBuffer::new();
        assert!(!buf.is_preview);
    }

    #[test]
    fn content_string_returns_buffer_text() {
        let mut tmp = tempfile::NamedTempFile::new().unwrap();
        write!(tmp, "hello world").unwrap();
        tmp.flush().unwrap();
        let buf = EditorBuffer::from_file(tmp.path()).unwrap();
        assert_eq!(buf.content_string(), "hello world");
    }

    #[test]
    fn content_string_empty_buffer() {
        let buf = EditorBuffer::new();
        assert_eq!(buf.content_string(), "");
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
                is_preview: false,
                cursor: CursorState::default(),
                scroll_row: 0,
                scroll_col: 0,
                history: EditHistory::new(),
                selection: None,
                highlight: None,
                diagnostics: Vec::new(),
                tab_size: DEFAULT_TAB_SIZE,
                insert_spaces: true,
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
            is_preview: false,
            cursor: CursorState::default(),
            scroll_row: 0,
            scroll_col: 0,
            history: EditHistory::new(),
            selection: None,
            highlight: None,
            diagnostics: Vec::new(),
            tab_size: DEFAULT_TAB_SIZE,
            insert_spaces: true,
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
        writeln!(tmp).unwrap();
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

    // --- undo/redo tests ---

    #[test]
    fn undo_insert_char() {
        let mut buf = buffer_from_str("hello");
        buf.cursor.col = 5;
        buf.insert_char('x');
        assert_eq!(buf.line_at(0).unwrap().to_string(), "hellox");
        buf.undo();
        assert_eq!(buf.line_at(0).unwrap().to_string(), "hello");
        assert_eq!(buf.cursor.col, 5);
    }

    #[test]
    fn redo_insert_char() {
        let mut buf = buffer_from_str("hello");
        buf.cursor.col = 5;
        buf.insert_char('x');
        buf.undo();
        buf.redo();
        assert_eq!(buf.line_at(0).unwrap().to_string(), "hellox");
        assert_eq!(buf.cursor.col, 6);
    }

    #[test]
    fn undo_backspace() {
        let mut buf = buffer_from_str("hello");
        buf.cursor.col = 3;
        buf.delete_char_backward();
        assert_eq!(buf.line_at(0).unwrap().to_string(), "helo");
        buf.undo();
        assert_eq!(buf.line_at(0).unwrap().to_string(), "hello");
        assert_eq!(buf.cursor.col, 3);
    }

    #[test]
    fn undo_newline() {
        let mut buf = buffer_from_str("hello");
        buf.cursor.col = 3;
        buf.insert_newline();
        assert_eq!(buf.line_count(), 2);
        buf.undo();
        assert_eq!(buf.line_at(0).unwrap().to_string(), "hello");
        assert_eq!(buf.cursor.row, 0);
        assert_eq!(buf.cursor.col, 3);
    }

    #[test]
    fn undo_delete_forward() {
        let mut buf = buffer_from_str("hello");
        buf.cursor.col = 2;
        buf.delete_char_forward();
        assert_eq!(buf.line_at(0).unwrap().to_string(), "helo");
        buf.undo();
        assert_eq!(buf.line_at(0).unwrap().to_string(), "hello");
        assert_eq!(buf.cursor.col, 2);
    }

    #[test]
    fn undo_tab() {
        let mut buf = buffer_from_str("hello");
        buf.insert_tab();
        assert_eq!(buf.line_at(0).unwrap().to_string(), "    hello");
        buf.undo();
        assert_eq!(buf.line_at(0).unwrap().to_string(), "hello");
        assert_eq!(buf.cursor.col, 0);
    }

    #[test]
    fn undo_preserves_across_save() {
        let mut tmp = tempfile::NamedTempFile::new().unwrap();
        write!(tmp, "data").unwrap();
        tmp.flush().unwrap();

        let mut buf = EditorBuffer::from_file(tmp.path()).unwrap();
        buf.insert_char('x');
        buf.save_to_file().unwrap();
        buf.undo();
        assert_eq!(buf.line_at(0).unwrap().to_string(), "data");
    }

    #[test]
    fn undo_redo_multiple_steps() {
        let mut buf = buffer_from_str("");
        // Use sleep to force separate undo groups.
        buf.insert_char('a');
        std::thread::sleep(std::time::Duration::from_millis(600));
        buf.insert_char('b');
        std::thread::sleep(std::time::Duration::from_millis(600));
        buf.insert_char('c');

        assert_eq!(buf.line_at(0).unwrap().to_string(), "abc");

        buf.undo(); // remove 'c'
        assert_eq!(buf.line_at(0).unwrap().to_string(), "ab");
        buf.undo(); // remove 'b'
        assert_eq!(buf.line_at(0).unwrap().to_string(), "a");
        buf.redo(); // restore 'b'
        assert_eq!(buf.line_at(0).unwrap().to_string(), "ab");
        buf.redo(); // restore 'c'
        assert_eq!(buf.line_at(0).unwrap().to_string(), "abc");
    }

    #[test]
    fn undo_backspace_at_line_join() {
        let mut buf = buffer_from_str("hello\nworld");
        buf.cursor.row = 1;
        buf.cursor.col = 0;
        buf.delete_char_backward();
        assert_eq!(buf.line_at(0).unwrap().to_string(), "helloworld");
        buf.undo();
        assert_eq!(buf.line_at(0).unwrap().to_string(), "hello\n");
        assert_eq!(buf.line_at(1).unwrap().to_string(), "world");
        assert_eq!(buf.cursor.row, 1);
        assert_eq!(buf.cursor.col, 0);
    }

    #[test]
    fn undo_on_empty_history_is_noop() {
        let mut buf = buffer_from_str("hello");
        buf.undo(); // Should not panic or change anything.
        assert_eq!(buf.line_at(0).unwrap().to_string(), "hello");
        assert!(!buf.modified);
    }

    #[test]
    fn redo_on_empty_history_is_noop() {
        let mut buf = buffer_from_str("hello");
        buf.redo();
        assert_eq!(buf.line_at(0).unwrap().to_string(), "hello");
        assert!(!buf.modified);
    }

    // --- Selection tests ---

    #[test]
    fn new_buffer_has_no_selection() {
        let buf = EditorBuffer::new();
        assert!(buf.selection.is_none());
    }

    #[test]
    fn from_file_has_no_selection() {
        let mut tmp = tempfile::NamedTempFile::new().unwrap();
        write!(tmp, "hello").unwrap();
        tmp.flush().unwrap();
        let buf = EditorBuffer::from_file(tmp.path()).unwrap();
        assert!(buf.selection.is_none());
    }

    #[test]
    fn select_right_starts_selection() {
        let mut buf = buffer_from_str("hello");
        buf.select_right();
        assert!(buf.selection.is_some());
        let sel = buf.selection.as_ref().unwrap();
        assert_eq!(sel.anchor_row, 0);
        assert_eq!(sel.anchor_col, 0);
        assert_eq!(buf.cursor.col, 1);
    }

    #[test]
    fn select_right_extends_selection() {
        let mut buf = buffer_from_str("hello");
        buf.select_right();
        buf.select_right();
        let sel = buf.selection.as_ref().unwrap();
        assert_eq!(sel.anchor_col, 0);
        assert_eq!(buf.cursor.col, 2);
    }

    #[test]
    fn select_left_from_mid() {
        let mut buf = buffer_from_str("hello");
        buf.cursor.col = 3;
        buf.select_left();
        let sel = buf.selection.as_ref().unwrap();
        assert_eq!(sel.anchor_col, 3);
        assert_eq!(buf.cursor.col, 2);
    }

    #[test]
    fn select_down() {
        let mut buf = buffer_from_str("hello\nworld");
        buf.select_down();
        let sel = buf.selection.as_ref().unwrap();
        assert_eq!(sel.anchor_row, 0);
        assert_eq!(buf.cursor.row, 1);
    }

    #[test]
    fn select_up() {
        let mut buf = buffer_from_str("hello\nworld");
        buf.cursor.row = 1;
        buf.select_up();
        let sel = buf.selection.as_ref().unwrap();
        assert_eq!(sel.anchor_row, 1);
        assert_eq!(buf.cursor.row, 0);
    }

    #[test]
    fn select_home() {
        let mut buf = buffer_from_str("hello");
        buf.cursor.col = 3;
        buf.select_home();
        let sel = buf.selection.as_ref().unwrap();
        assert_eq!(sel.anchor_col, 3);
        assert_eq!(buf.cursor.col, 0);
    }

    #[test]
    fn select_end() {
        let mut buf = buffer_from_str("hello");
        buf.select_end();
        let sel = buf.selection.as_ref().unwrap();
        assert_eq!(sel.anchor_col, 0);
        assert_eq!(buf.cursor.col, 5);
    }

    #[test]
    fn select_file_start() {
        let mut buf = buffer_from_str("hello\nworld");
        buf.cursor.row = 1;
        buf.cursor.col = 3;
        buf.select_file_start();
        let sel = buf.selection.as_ref().unwrap();
        assert_eq!(sel.anchor_row, 1);
        assert_eq!(sel.anchor_col, 3);
        assert_eq!(buf.cursor.row, 0);
        assert_eq!(buf.cursor.col, 0);
    }

    #[test]
    fn select_file_end() {
        let mut buf = buffer_from_str("hello\nworld");
        buf.select_file_end();
        let sel = buf.selection.as_ref().unwrap();
        assert_eq!(sel.anchor_row, 0);
        assert_eq!(sel.anchor_col, 0);
        assert_eq!(buf.cursor.row, 1);
        assert_eq!(buf.cursor.col, 5);
    }

    #[test]
    fn select_word_right() {
        let mut buf = buffer_from_str("hello world");
        buf.select_word_right();
        let sel = buf.selection.as_ref().unwrap();
        assert_eq!(sel.anchor_col, 0);
        assert_eq!(buf.cursor.col, 6);
    }

    #[test]
    fn select_word_left() {
        let mut buf = buffer_from_str("hello world");
        buf.cursor.col = 11;
        buf.select_word_left();
        let sel = buf.selection.as_ref().unwrap();
        assert_eq!(sel.anchor_col, 11);
        assert_eq!(buf.cursor.col, 6);
    }

    #[test]
    fn select_all() {
        let mut buf = buffer_from_str("hello\nworld");
        buf.select_all();
        let sel = buf.selection.as_ref().unwrap();
        assert_eq!(sel.anchor_row, 0);
        assert_eq!(sel.anchor_col, 0);
        assert_eq!(buf.cursor.row, 1);
        assert_eq!(buf.cursor.col, 5);
    }

    #[test]
    fn clear_selection_clears() {
        let mut buf = buffer_from_str("hello");
        buf.select_right();
        assert!(buf.selection.is_some());
        buf.clear_selection();
        assert!(buf.selection.is_none());
    }

    // --- selected_text tests ---

    #[test]
    fn selected_text_single_line() {
        let mut buf = buffer_from_str("hello world");
        buf.selection = Some(Selection {
            anchor_row: 0,
            anchor_col: 0,
        });
        buf.cursor.col = 5;
        assert_eq!(buf.selected_text(), Some("hello".to_string()));
    }

    #[test]
    fn selected_text_multi_line() {
        let mut buf = buffer_from_str("hello\nworld");
        buf.selection = Some(Selection {
            anchor_row: 0,
            anchor_col: 3,
        });
        buf.cursor.row = 1;
        buf.cursor.col = 2;
        assert_eq!(buf.selected_text(), Some("lo\nwo".to_string()));
    }

    #[test]
    fn selected_text_backward() {
        let mut buf = buffer_from_str("hello world");
        buf.selection = Some(Selection {
            anchor_row: 0,
            anchor_col: 8,
        });
        buf.cursor.col = 3;
        assert_eq!(buf.selected_text(), Some("lo wo".to_string()));
    }

    #[test]
    fn selected_text_none() {
        let buf = buffer_from_str("hello");
        assert_eq!(buf.selected_text(), None);
    }

    // --- delete_selection tests ---

    #[test]
    fn delete_selection_single_line() {
        let mut buf = buffer_from_str("hello world");
        buf.selection = Some(Selection {
            anchor_row: 0,
            anchor_col: 0,
        });
        buf.cursor.col = 5;
        buf.delete_selection();
        assert_eq!(buf.line_at(0).unwrap().to_string(), " world");
        assert_eq!(buf.cursor.col, 0);
        assert!(buf.selection.is_none());
    }

    #[test]
    fn delete_selection_multi_line() {
        let mut buf = buffer_from_str("hello\nworld");
        buf.selection = Some(Selection {
            anchor_row: 0,
            anchor_col: 3,
        });
        buf.cursor.row = 1;
        buf.cursor.col = 2;
        buf.delete_selection();
        assert_eq!(buf.line_at(0).unwrap().to_string(), "helrld");
        assert_eq!(buf.cursor.row, 0);
        assert_eq!(buf.cursor.col, 3);
    }

    #[test]
    fn delete_selection_records_undo() {
        let mut buf = buffer_from_str("hello");
        buf.selection = Some(Selection {
            anchor_row: 0,
            anchor_col: 1,
        });
        buf.cursor.col = 4;
        buf.delete_selection();
        assert_eq!(buf.line_at(0).unwrap().to_string(), "ho");
        buf.undo();
        assert_eq!(buf.line_at(0).unwrap().to_string(), "hello");
    }

    #[test]
    fn delete_selection_returns_text() {
        let mut buf = buffer_from_str("hello");
        buf.selection = Some(Selection {
            anchor_row: 0,
            anchor_col: 0,
        });
        buf.cursor.col = 3;
        let deleted = buf.delete_selection();
        assert_eq!(deleted, Some("hel".to_string()));
    }

    #[test]
    fn delete_selection_no_selection_noop() {
        let mut buf = buffer_from_str("hello");
        let result = buf.delete_selection();
        assert_eq!(result, None);
        assert_eq!(buf.line_at(0).unwrap().to_string(), "hello");
    }

    // --- insert_text tests ---

    #[test]
    fn insert_text_single_char() {
        let mut buf = buffer_from_str("hello");
        buf.cursor.col = 2;
        buf.insert_text("x");
        assert_eq!(buf.line_at(0).unwrap().to_string(), "hexllo");
        assert_eq!(buf.cursor.col, 3);
    }

    #[test]
    fn insert_text_multiline() {
        let mut buf = buffer_from_str("hello");
        buf.cursor.col = 2;
        buf.insert_text("a\nb");
        assert_eq!(buf.line_at(0).unwrap().to_string(), "hea\n");
        assert_eq!(buf.line_at(1).unwrap().to_string(), "bllo");
        assert_eq!(buf.cursor.row, 1);
        assert_eq!(buf.cursor.col, 1);
    }

    #[test]
    fn insert_text_replaces_selection() {
        let mut buf = buffer_from_str("hello world");
        buf.selection = Some(Selection {
            anchor_row: 0,
            anchor_col: 0,
        });
        buf.cursor.col = 5;
        buf.insert_text("hi");
        assert_eq!(buf.line_at(0).unwrap().to_string(), "hi world");
        assert!(buf.selection.is_none());
    }

    #[test]
    fn insert_text_empty_noop() {
        let mut buf = buffer_from_str("hello");
        buf.insert_text("");
        assert_eq!(buf.line_at(0).unwrap().to_string(), "hello");
        assert!(!buf.modified);
    }

    // --- Edit methods with selection tests ---

    #[test]
    fn insert_char_with_selection_replaces() {
        let mut buf = buffer_from_str("hello");
        buf.selection = Some(Selection {
            anchor_row: 0,
            anchor_col: 1,
        });
        buf.cursor.col = 4;
        buf.insert_char('x');
        assert_eq!(buf.line_at(0).unwrap().to_string(), "hxo");
    }

    #[test]
    fn insert_newline_with_selection_replaces() {
        let mut buf = buffer_from_str("hello");
        buf.selection = Some(Selection {
            anchor_row: 0,
            anchor_col: 1,
        });
        buf.cursor.col = 4;
        buf.insert_newline();
        assert_eq!(buf.line_at(0).unwrap().to_string(), "h\n");
        assert!(buf.line_at(1).unwrap().to_string().starts_with("o"));
    }

    #[test]
    fn insert_tab_with_selection_replaces() {
        let mut buf = buffer_from_str("hello");
        buf.selection = Some(Selection {
            anchor_row: 0,
            anchor_col: 1,
        });
        buf.cursor.col = 4;
        buf.insert_tab();
        assert_eq!(buf.line_at(0).unwrap().to_string(), "h    o");
    }

    #[test]
    fn backspace_with_selection_deletes_selection() {
        let mut buf = buffer_from_str("hello");
        buf.selection = Some(Selection {
            anchor_row: 0,
            anchor_col: 1,
        });
        buf.cursor.col = 4;
        buf.delete_char_backward();
        assert_eq!(buf.line_at(0).unwrap().to_string(), "ho");
        assert!(buf.selection.is_none());
    }

    #[test]
    fn delete_with_selection_deletes_selection() {
        let mut buf = buffer_from_str("hello");
        buf.selection = Some(Selection {
            anchor_row: 0,
            anchor_col: 1,
        });
        buf.cursor.col = 4;
        buf.delete_char_forward();
        assert_eq!(buf.line_at(0).unwrap().to_string(), "ho");
        assert!(buf.selection.is_none());
    }

    // --- Syntax highlighting integration tests ---

    #[test]
    fn from_file_initializes_highlight_for_rust() {
        use std::io::Write;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.rs");
        let mut file = File::create(&path).unwrap();
        writeln!(file, "fn main() {{}}").unwrap();

        let buf = EditorBuffer::from_file(&path).unwrap();
        // highlight should be Some for .rs files.
        assert!(buf.highlight.is_some());
    }

    #[test]
    fn from_file_no_highlight_for_unknown_ext() {
        use std::io::Write;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.xyz");
        let mut file = File::create(&path).unwrap();
        writeln!(file, "hello world").unwrap();

        let buf = EditorBuffer::from_file(&path).unwrap();
        assert!(buf.highlight.is_none());
    }

    #[test]
    fn highlight_range_returns_spans_for_rust_file() {
        use std::io::Write;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.rs");
        let mut file = File::create(&path).unwrap();
        writeln!(file, "fn main() {{}}").unwrap();

        let buf = EditorBuffer::from_file(&path).unwrap();
        let spans = buf.highlight_range(0, 1);
        assert_eq!(spans.len(), 1);
        // "fn" should produce a Keyword span.
        let has_keyword = spans[0]
            .iter()
            .any(|s| s.kind == crate::highlight::HighlightKind::Keyword);
        assert!(
            has_keyword,
            "expected keyword highlight, got: {:?}",
            spans[0]
        );
    }

    #[test]
    fn highlight_range_returns_empty_for_plain_text() {
        let buf = buffer_from_str("hello world\n");
        let spans = buf.highlight_range(0, 1);
        assert_eq!(spans.len(), 1);
        assert!(spans[0].is_empty());
    }

    #[test]
    fn highlight_updates_after_insert_char() {
        use std::io::Write;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.rs");
        let mut file = File::create(&path).unwrap();
        writeln!(file, "// hello").unwrap();

        let mut buf = EditorBuffer::from_file(&path).unwrap();
        // Insert at start of file: "let x = 1;\n"
        buf.cursor.row = 0;
        buf.cursor.col = 0;
        for ch in "let x = 1;\n".chars() {
            if ch == '\n' {
                buf.insert_newline();
            } else {
                buf.insert_char(ch);
            }
        }
        let spans = buf.highlight_range(0, 1);
        // First line should now have "let" keyword.
        let has_let = spans[0]
            .iter()
            .any(|s| s.kind == crate::highlight::HighlightKind::Keyword);
        assert!(
            has_let,
            "expected 'let' keyword after insert, got: {:?}",
            spans[0]
        );
    }

    #[test]
    fn highlight_updates_after_undo() {
        use std::io::Write;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.rs");
        let mut file = File::create(&path).unwrap();
        writeln!(file, "fn main() {{}}").unwrap();

        let mut buf = EditorBuffer::from_file(&path).unwrap();
        // Type a character.
        buf.cursor.row = 0;
        buf.cursor.col = 0;
        buf.insert_char('x');
        // Undo — should restore the original parse.
        buf.undo();
        let spans = buf.highlight_range(0, 1);
        let has_fn = spans[0].iter().any(|s| {
            s.kind == crate::highlight::HighlightKind::Keyword && s.col_start == 0 && s.col_end == 2
        });
        assert!(
            has_fn,
            "expected 'fn' keyword after undo, got: {:?}",
            spans[0]
        );
    }

    // --- tab_size / insert_spaces config tests ---

    #[test]
    fn buffer_default_tab_size_is_4() {
        let buf = EditorBuffer::new();
        assert_eq!(buf.tab_size(), 4);
    }

    #[test]
    fn buffer_default_insert_spaces_is_true() {
        let buf = EditorBuffer::new();
        assert!(buf.insert_spaces());
    }

    #[test]
    fn buffer_with_custom_tab_size() {
        let buf = EditorBuffer::with_tab_config(2, true);
        assert_eq!(buf.tab_size(), 2);
    }

    #[test]
    fn buffer_with_insert_spaces_false() {
        let buf = EditorBuffer::with_tab_config(4, false);
        assert!(!buf.insert_spaces());
    }

    #[test]
    fn buffer_insert_tab_uses_configured_size() {
        let mut buf = EditorBuffer::with_tab_config(2, true);
        buf.insert_tab();
        assert_eq!(buf.line_at(0).unwrap().to_string(), "  ");
        assert_eq!(buf.cursor.col, 2);
    }

    #[test]
    fn buffer_insert_tab_with_spaces_false_inserts_tab_char() {
        let mut buf = EditorBuffer::with_tab_config(4, false);
        buf.insert_tab();
        assert_eq!(buf.line_at(0).unwrap().to_string(), "\t");
        assert_eq!(buf.cursor.col, 1);
    }

    #[test]
    fn buffer_insert_tab_with_spaces_false_and_custom_size() {
        // insert_spaces=false always inserts a single \t regardless of tab_size
        let mut buf = EditorBuffer::with_tab_config(8, false);
        buf.insert_tab();
        assert_eq!(buf.line_at(0).unwrap().to_string(), "\t");
        assert_eq!(buf.cursor.col, 1);
    }

    // --- select_word_at_cursor tests ---

    #[test]
    fn select_word_at_cursor_middle_of_word() {
        let mut buf = buffer_from_str("hello world");
        buf.cursor.col = 2;
        buf.select_word_at_cursor();
        assert_eq!(buf.selected_text(), Some("hello".to_string()));
        let sel = buf.selection.as_ref().unwrap();
        assert_eq!(sel.anchor_col, 0);
        assert_eq!(buf.cursor.col, 5);
    }

    #[test]
    fn select_word_at_cursor_second_word() {
        let mut buf = buffer_from_str("hello world");
        buf.cursor.col = 8;
        buf.select_word_at_cursor();
        assert_eq!(buf.selected_text(), Some("world".to_string()));
    }

    #[test]
    fn select_word_at_cursor_on_whitespace_does_nothing() {
        let mut buf = buffer_from_str("hello world");
        buf.cursor.col = 5;
        buf.select_word_at_cursor();
        assert!(buf.selection.is_none());
    }

    #[test]
    fn select_word_at_cursor_snake_case() {
        let mut buf = buffer_from_str("snake_case_var = 42");
        buf.cursor.col = 6;
        buf.select_word_at_cursor();
        assert_eq!(buf.selected_text(), Some("snake_case_var".to_string()));
    }

    #[test]
    fn select_word_at_cursor_start_of_word() {
        let mut buf = buffer_from_str("hello world");
        buf.cursor.col = 0;
        buf.select_word_at_cursor();
        assert_eq!(buf.selected_text(), Some("hello".to_string()));
    }

    #[test]
    fn select_word_at_cursor_end_of_word() {
        let mut buf = buffer_from_str("hello world");
        buf.cursor.col = 4;
        buf.select_word_at_cursor();
        assert_eq!(buf.selected_text(), Some("hello".to_string()));
    }

    #[test]
    fn select_word_at_cursor_empty_line() {
        let mut buf = buffer_from_str("");
        buf.select_word_at_cursor();
        assert!(buf.selection.is_none());
    }

    #[test]
    fn select_word_at_cursor_past_line_end() {
        let mut buf = buffer_from_str("hi");
        buf.cursor.col = 5;
        buf.select_word_at_cursor();
        assert!(buf.selection.is_none());
    }

    // --- select_line_at_cursor tests ---

    #[test]
    fn select_line_at_cursor_first_line() {
        let mut buf = buffer_from_str("hello world\nsecond line");
        buf.cursor.row = 0;
        buf.cursor.col = 3;
        buf.select_line_at_cursor();
        let sel = buf.selection.as_ref().unwrap();
        assert_eq!(sel.anchor_row, 0);
        assert_eq!(sel.anchor_col, 0);
        assert_eq!(buf.cursor.col, 11);
        assert_eq!(buf.selected_text(), Some("hello world".to_string()));
    }

    #[test]
    fn select_line_at_cursor_second_line() {
        let mut buf = buffer_from_str("first\nsecond\nthird");
        buf.cursor.row = 1;
        buf.select_line_at_cursor();
        assert_eq!(buf.selected_text(), Some("second".to_string()));
    }

    #[test]
    fn select_line_at_cursor_empty_line() {
        let mut buf = buffer_from_str("hello\n\nworld");
        buf.cursor.row = 1;
        buf.select_line_at_cursor();
        assert!(buf.selection.is_some());
        assert_eq!(buf.cursor.col, 0);
    }

    // --- Diagnostics ---

    #[test]
    fn new_buffer_has_no_diagnostics() {
        let buf = EditorBuffer::new();
        assert!(buf.diagnostics().is_empty());
    }

    #[test]
    fn set_and_get_diagnostics() {
        let mut buf = EditorBuffer::new();
        let diags = vec![BufferDiagnostic {
            line: 0,
            col_start: 0,
            col_end: 5,
            severity: crate::diagnostic::DiagnosticSeverity::Error,
            message: "test error".to_string(),
            source: None,
            code: None,
        }];
        buf.set_diagnostics(diags.clone());
        assert_eq!(buf.diagnostics().len(), 1);
        assert_eq!(buf.diagnostics()[0].message, "test error");
    }

    #[test]
    fn clear_diagnostics() {
        let mut buf = EditorBuffer::new();
        buf.set_diagnostics(vec![BufferDiagnostic {
            line: 0,
            col_start: 0,
            col_end: 5,
            severity: crate::diagnostic::DiagnosticSeverity::Warning,
            message: "warning".to_string(),
            source: None,
            code: None,
        }]);
        assert!(!buf.diagnostics().is_empty());
        buf.clear_diagnostics();
        assert!(buf.diagnostics().is_empty());
    }

    // --- line_text ---

    #[test]
    fn line_text_returns_content() {
        let mut buf = EditorBuffer::new();
        buf.insert_text("hello\nworld");
        assert_eq!(buf.line_text(0), "hello");
        assert_eq!(buf.line_text(1), "world");
    }

    #[test]
    fn line_text_out_of_bounds_returns_empty() {
        let buf = EditorBuffer::new();
        assert_eq!(buf.line_text(999), "");
    }

    // --- apply_text_edit ---

    #[test]
    fn apply_text_edit_single_line() {
        let mut buf = EditorBuffer::new();
        buf.insert_text("hello world");
        // Replace "world" (col 6..11) with "rust"
        buf.apply_text_edit(0, 6, 0, 11, "rust");
        assert_eq!(buf.content_string(), "hello rust");
        assert_eq!(buf.cursor.row, 0);
        assert_eq!(buf.cursor.col, 10);
    }

    #[test]
    fn apply_text_edit_replaces_range() {
        let mut buf = EditorBuffer::new();
        buf.insert_text("fn foo()");
        // Replace "foo" (col 3..6) with "bar"
        buf.apply_text_edit(0, 3, 0, 6, "bar");
        assert_eq!(buf.content_string(), "fn bar()");
        assert_eq!(buf.cursor.col, 6);
    }

    #[test]
    fn apply_text_edit_insert_without_delete() {
        let mut buf = EditorBuffer::new();
        buf.insert_text("ab");
        // Insert at col 1 with zero-width range
        buf.apply_text_edit(0, 1, 0, 1, "XY");
        assert_eq!(buf.content_string(), "aXYb");
        assert_eq!(buf.cursor.col, 3);
    }

    #[test]
    fn apply_text_edit_records_in_history() {
        let mut buf = EditorBuffer::new();
        buf.insert_text("hello");
        buf.apply_text_edit(0, 0, 0, 5, "bye");
        assert_eq!(buf.content_string(), "bye");
        buf.undo();
        assert_eq!(buf.content_string(), "hello");
    }

    // --- scroll_by tests ---

    /// Helper: creates a buffer with `n` lines of content.
    fn buffer_with_lines(n: usize) -> EditorBuffer {
        let mut buf = EditorBuffer::new();
        let text: String = (0..n).map(|i| format!("line {i}\n")).collect();
        buf.insert_text(&text);
        buf
    }

    #[test]
    fn scroll_by_positive_scrolls_down() {
        let mut buf = buffer_with_lines(100);
        buf.scroll_by(5, 20);
        assert_eq!(buf.scroll_row, 5);
    }

    #[test]
    fn scroll_by_negative_scrolls_up() {
        let mut buf = buffer_with_lines(100);
        buf.scroll_row = 10;
        buf.scroll_by(-3, 20);
        assert_eq!(buf.scroll_row, 7);
    }

    #[test]
    fn scroll_by_clamps_to_zero() {
        let mut buf = buffer_with_lines(100);
        buf.scroll_row = 2;
        buf.scroll_by(-10, 20);
        assert_eq!(buf.scroll_row, 0);
    }

    #[test]
    fn scroll_by_clamps_to_max() {
        let mut buf = buffer_with_lines(50);
        let viewport_height = 20;
        buf.scroll_by(100, viewport_height);
        // Max scroll = line_count - viewport_height
        let max = buf.line_count().saturating_sub(viewport_height);
        assert_eq!(buf.scroll_row, max);
    }

    #[test]
    fn scroll_by_no_op_for_small_file() {
        let mut buf = buffer_with_lines(5);
        let viewport_height = 20;
        buf.scroll_by(10, viewport_height);
        assert_eq!(buf.scroll_row, 0);
    }

    #[test]
    fn scroll_by_does_not_move_cursor() {
        let mut buf = buffer_with_lines(100);
        buf.cursor.row = 5;
        buf.cursor.col = 3;
        buf.scroll_by(10, 20);
        assert_eq!(buf.cursor.row, 5);
        assert_eq!(buf.cursor.col, 3);
    }

    // --- scroll_horizontally_by tests ---

    #[test]
    fn scroll_horizontally_positive_scrolls_right() {
        let mut buf = EditorBuffer::new();
        buf.insert_text("a]".repeat(100).as_str());
        buf.scroll_horizontally_by(5);
        assert_eq!(buf.scroll_col, 5);
    }

    #[test]
    fn scroll_horizontally_negative_scrolls_left() {
        let mut buf = EditorBuffer::new();
        buf.scroll_col = 10;
        buf.scroll_horizontally_by(-3);
        assert_eq!(buf.scroll_col, 7);
    }

    #[test]
    fn scroll_horizontally_clamps_to_zero() {
        let mut buf = EditorBuffer::new();
        buf.scroll_col = 2;
        buf.scroll_horizontally_by(-10);
        assert_eq!(buf.scroll_col, 0);
    }

    #[test]
    fn scroll_horizontally_does_not_move_cursor() {
        let mut buf = EditorBuffer::new();
        buf.insert_text(&"x".repeat(200));
        buf.cursor.row = 0;
        buf.cursor.col = 5;
        buf.scroll_horizontally_by(10);
        assert_eq!(buf.cursor.row, 0);
        assert_eq!(buf.cursor.col, 5);
    }
}
