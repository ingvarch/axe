mod cursor;
mod editing;
mod highlighting;
mod scrolling;
mod selection;

#[cfg(test)]
mod tests;

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use ropey::{Rope, RopeSlice};

use crate::cursor::CursorState;
use crate::diagnostic::BufferDiagnostic;
use crate::diff::DiffHunk;
use crate::highlight::HighlightState;
use crate::history::EditHistory;
use crate::selection::Selection;

/// Default number of spaces inserted for a tab.
const DEFAULT_TAB_SIZE: usize = 4;

/// Maximum number of bytes to scan when detecting line endings.
const LINE_ENDING_SCAN_LIMIT: usize = 8192;

/// Represents the line ending style of a buffer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineEnding {
    /// Unix-style line feed (`\n`).
    Lf,
    /// Windows-style carriage return + line feed (`\r\n`).
    CrLf,
}

impl LineEnding {
    /// Returns the display string for this line ending.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Lf => "LF",
            Self::CrLf => "CRLF",
        }
    }
}

/// Detects the line ending style from raw bytes.
///
/// Scans up to `LINE_ENDING_SCAN_LIMIT` bytes for `\r\n`. Defaults to `Lf`.
fn detect_line_ending(bytes: &[u8]) -> LineEnding {
    let limit = bytes.len().min(LINE_ENDING_SCAN_LIMIT);
    if bytes[..limit].windows(2).any(|w| w == b"\r\n") {
        LineEnding::CrLf
    } else {
        LineEnding::Lf
    }
}

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
    /// Viewport width in columns for horizontal scroll clamping.
    viewport_width: usize,
    /// Current text selection, if any.
    pub selection: Option<Selection>,
    /// Syntax highlighting state, if the file type is supported.
    highlight: Option<HighlightState>,
    /// LSP diagnostics for this buffer (errors, warnings, etc.).
    diagnostics: Vec<BufferDiagnostic>,
    /// Git diff hunks for this buffer (added/modified/deleted lines vs HEAD).
    diff_hunks: Vec<DiffHunk>,
    /// Number of spaces (or columns) per tab stop.
    tab_size: usize,
    /// Whether Tab key inserts spaces (`true`) or a literal tab character (`false`).
    insert_spaces: bool,
    /// Detected line ending style for this buffer.
    line_ending: LineEnding,
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
            viewport_width: usize::MAX,
            history: EditHistory::new(),
            selection: None,
            highlight: None,
            diagnostics: Vec::new(),
            diff_hunks: Vec::new(),
            tab_size: DEFAULT_TAB_SIZE,
            insert_spaces: true,
            line_ending: LineEnding::Lf,
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

    /// Returns the detected line ending style for this buffer.
    pub fn line_ending(&self) -> LineEnding {
        self.line_ending
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

    /// Returns the git diff hunks for this buffer.
    pub fn diff_hunks(&self) -> &[DiffHunk] {
        &self.diff_hunks
    }

    /// Replaces all diff hunks for this buffer.
    pub fn set_diff_hunks(&mut self, hunks: Vec<DiffHunk>) {
        self.diff_hunks = hunks;
    }

    /// Removes all diagnostics from this buffer.
    pub fn clear_diagnostics(&mut self) {
        self.diagnostics.clear();
    }

    /// Loads a buffer from a file on disk.
    ///
    /// Returns an error if the file cannot be read.
    pub fn from_file(path: &Path) -> Result<Self> {
        let raw_bytes = std::fs::read(path)
            .with_context(|| format!("Failed to read file: {}", path.display()))?;
        let line_ending = detect_line_ending(&raw_bytes);
        let content = Rope::from_reader(raw_bytes.as_slice())
            .with_context(|| format!("Failed to parse file: {}", path.display()))?;

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
            viewport_width: usize::MAX,
            history: EditHistory::new(),
            selection: None,
            highlight,
            diagnostics: Vec::new(),
            diff_hunks: Vec::new(),
            tab_size: DEFAULT_TAB_SIZE,
            insert_spaces: true,
            line_ending,
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
            "tf" | "tfvars" => "Terraform",
            "hcl" => "HCL",
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

    /// Reloads buffer content from disk if the file changed externally.
    ///
    /// Skips reload if:
    /// - The buffer has no associated file path
    /// - The buffer has unsaved user modifications (`modified == true`)
    /// - The disk content matches the current buffer content
    ///
    /// Returns `true` if the buffer was actually reloaded.
    pub fn reload_from_disk(&mut self) -> bool {
        // Don't overwrite user's unsaved edits.
        if self.modified {
            return false;
        }

        let path = match self.path {
            Some(ref p) => p.clone(),
            None => return false,
        };

        let raw_bytes = match std::fs::read(&path) {
            Ok(b) => b,
            Err(_) => return false,
        };

        let new_rope = match Rope::from_reader(raw_bytes.as_slice()) {
            Ok(r) => r,
            Err(_) => return false,
        };

        // Skip if content is identical.
        if self.content == new_rope {
            return false;
        }

        self.line_ending = detect_line_ending(&raw_bytes);
        self.content = new_rope;
        self.modified = false;
        self.history = EditHistory::new();
        self.selection = None;
        self.diff_hunks.clear();

        // Re-parse syntax highlighting for the new content.
        if let Some(ref mut hl) = self.highlight {
            hl.parse_full(&self.content);
        }

        // Clamp cursor to valid range.
        let max_row = self.content_line_count().saturating_sub(1);
        if self.cursor.row > max_row {
            self.cursor.row = max_row;
        }
        let max_col = self.line_length(self.cursor.row);
        if self.cursor.col > max_col {
            self.cursor.col = max_col;
        }

        // Clamp scroll position.
        if self.scroll_row > max_row {
            self.scroll_row = max_row;
        }

        true
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

    /// Returns true if `c` is a word character (alphanumeric or underscore).
    fn is_word_char(c: char) -> bool {
        c.is_alphanumeric() || c == '_'
    }
}
