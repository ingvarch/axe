use std::path::Path;

use anyhow::{Context, Result};

use crate::buffer::EditorBuffer;

/// Manages multiple open editor buffers and tracks the active one.
///
/// Prevents duplicate opens of the same file path by switching to
/// the existing buffer instead.
pub struct BufferManager {
    buffers: Vec<EditorBuffer>,
    active: usize,
}

impl Default for BufferManager {
    fn default() -> Self {
        Self::new()
    }
}

impl BufferManager {
    /// Creates a new empty buffer manager with no open buffers.
    pub fn new() -> Self {
        Self {
            buffers: Vec::new(),
            active: 0,
        }
    }

    /// Opens a file as a new buffer, or switches to it if already open.
    ///
    /// Returns an error if the file cannot be read.
    pub fn open_file(&mut self, path: &Path) -> Result<()> {
        // Dedup: if already open, just switch to it.
        let canonical = std::fs::canonicalize(path)
            .with_context(|| format!("Failed to resolve path: {}", path.display()))?;

        for (i, buf) in self.buffers.iter().enumerate() {
            if let Some(existing) = buf.path() {
                if let Ok(existing_canonical) = std::fs::canonicalize(existing) {
                    if existing_canonical == canonical {
                        self.active = i;
                        return Ok(());
                    }
                }
            }
        }

        let buffer = EditorBuffer::from_file(path)?;
        self.buffers.push(buffer);
        self.active = self.buffers.len() - 1;
        Ok(())
    }

    /// Returns a reference to the active buffer, if any.
    pub fn active_buffer(&self) -> Option<&EditorBuffer> {
        self.buffers.get(self.active)
    }

    /// Returns the number of open buffers.
    pub fn buffer_count(&self) -> usize {
        self.buffers.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn new_is_empty() {
        let mgr = BufferManager::new();
        assert_eq!(mgr.buffer_count(), 0);
        assert!(mgr.active_buffer().is_none());
    }

    #[test]
    fn open_file_adds_buffer() {
        let mut tmp = tempfile::NamedTempFile::new().unwrap();
        writeln!(tmp, "hello").unwrap();
        tmp.flush().unwrap();

        let mut mgr = BufferManager::new();
        mgr.open_file(tmp.path()).unwrap();

        assert_eq!(mgr.buffer_count(), 1);
        assert!(mgr.active_buffer().is_some());
    }

    #[test]
    fn open_file_sets_active_to_latest() {
        let mut tmp1 = tempfile::NamedTempFile::new().unwrap();
        writeln!(tmp1, "file1").unwrap();
        tmp1.flush().unwrap();

        let mut tmp2 = tempfile::NamedTempFile::new().unwrap();
        writeln!(tmp2, "file2").unwrap();
        tmp2.flush().unwrap();

        let mut mgr = BufferManager::new();
        mgr.open_file(tmp1.path()).unwrap();
        mgr.open_file(tmp2.path()).unwrap();

        assert_eq!(mgr.buffer_count(), 2);
        let active = mgr.active_buffer().unwrap();
        let name = active.file_name().unwrap();
        let expected_name = tmp2.path().file_name().unwrap().to_str().unwrap();
        assert_eq!(name, expected_name);
    }

    #[test]
    fn open_same_file_no_duplicate() {
        let mut tmp = tempfile::NamedTempFile::new().unwrap();
        writeln!(tmp, "content").unwrap();
        tmp.flush().unwrap();

        let mut mgr = BufferManager::new();
        mgr.open_file(tmp.path()).unwrap();
        mgr.open_file(tmp.path()).unwrap();

        assert_eq!(mgr.buffer_count(), 1);
    }

    #[test]
    fn active_buffer_returns_content() {
        let mut tmp = tempfile::NamedTempFile::new().unwrap();
        writeln!(tmp, "line1").unwrap();
        writeln!(tmp, "line2").unwrap();
        write!(tmp, "line3").unwrap();
        tmp.flush().unwrap();

        let mut mgr = BufferManager::new();
        mgr.open_file(tmp.path()).unwrap();

        let buf = mgr.active_buffer().unwrap();
        assert_eq!(buf.line_count(), 3);
    }
}
