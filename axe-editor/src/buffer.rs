use std::fs::File;
use std::io::BufReader;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use ropey::{Rope, RopeSlice};

use crate::cursor::CursorState;

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
}
