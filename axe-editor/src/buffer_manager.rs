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

    /// Returns a mutable reference to the active buffer, if any.
    pub fn active_buffer_mut(&mut self) -> Option<&mut EditorBuffer> {
        self.buffers.get_mut(self.active)
    }

    /// Returns the number of open buffers.
    pub fn buffer_count(&self) -> usize {
        self.buffers.len()
    }

    /// Returns the index of the currently active buffer.
    pub fn active_index(&self) -> usize {
        self.active
    }

    /// Returns a slice of all open buffers.
    pub fn buffers(&self) -> &[EditorBuffer] {
        &self.buffers
    }

    /// Cycles to the next buffer, wrapping around to the first.
    ///
    /// Does nothing if there are zero or one buffers.
    pub fn next_buffer(&mut self) {
        if self.buffers.len() <= 1 {
            return;
        }
        self.active = (self.active + 1) % self.buffers.len();
    }

    /// Cycles to the previous buffer, wrapping around to the last.
    ///
    /// Does nothing if there are zero or one buffers.
    pub fn prev_buffer(&mut self) {
        if self.buffers.len() <= 1 {
            return;
        }
        if self.active == 0 {
            self.active = self.buffers.len() - 1;
        } else {
            self.active -= 1;
        }
    }

    /// Sets the active buffer to the given index, clamping to the valid range.
    ///
    /// Does nothing if there are no buffers.
    pub fn set_active(&mut self, index: usize) {
        if self.buffers.is_empty() {
            return;
        }
        self.active = index.min(self.buffers.len() - 1);
    }

    /// Closes the buffer at the given index and adjusts the active index.
    ///
    /// Does nothing if the index is out of bounds. After removal, if the
    /// active index was beyond the removed position it is decremented.
    /// If the active index would exceed the last valid index, it is clamped.
    pub fn close_buffer(&mut self, index: usize) {
        if index >= self.buffers.len() {
            return;
        }
        self.buffers.remove(index);
        if self.buffers.is_empty() {
            self.active = 0;
        } else if self.active > index {
            self.active -= 1;
        } else if self.active >= self.buffers.len() {
            self.active = self.buffers.len() - 1;
        }
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
    fn active_buffer_mut_returns_mutable_ref() {
        let mut tmp = tempfile::NamedTempFile::new().unwrap();
        writeln!(tmp, "hello").unwrap();
        tmp.flush().unwrap();

        let mut mgr = BufferManager::new();
        mgr.open_file(tmp.path()).unwrap();

        let buf = mgr.active_buffer_mut().unwrap();
        buf.cursor.row = 42;

        assert_eq!(mgr.active_buffer().unwrap().cursor.row, 42);
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

    fn two_buffer_mgr() -> BufferManager {
        let mut tmp1 = tempfile::NamedTempFile::new().unwrap();
        writeln!(tmp1, "file1").unwrap();
        tmp1.flush().unwrap();

        let mut tmp2 = tempfile::NamedTempFile::new().unwrap();
        writeln!(tmp2, "file2").unwrap();
        tmp2.flush().unwrap();

        let mut mgr = BufferManager::new();
        mgr.open_file(tmp1.path()).unwrap();
        mgr.open_file(tmp2.path()).unwrap();

        // Keep temp files alive by leaking them (tests only)
        std::mem::forget(tmp1);
        std::mem::forget(tmp2);

        mgr
    }

    #[test]
    fn active_index_returns_current() {
        let mgr = two_buffer_mgr();
        // After opening two files, active should be 1 (the last opened)
        assert_eq!(mgr.active_index(), 1);
    }

    #[test]
    fn buffers_returns_slice() {
        let mgr = two_buffer_mgr();
        assert_eq!(mgr.buffers().len(), 2);
    }

    #[test]
    fn next_buffer_cycles() {
        let mut mgr = two_buffer_mgr();
        assert_eq!(mgr.active_index(), 1);
        mgr.next_buffer();
        assert_eq!(mgr.active_index(), 0);
        mgr.next_buffer();
        assert_eq!(mgr.active_index(), 1);
    }

    #[test]
    fn prev_buffer_cycles() {
        let mut mgr = two_buffer_mgr();
        assert_eq!(mgr.active_index(), 1);
        mgr.prev_buffer();
        assert_eq!(mgr.active_index(), 0);
        mgr.prev_buffer();
        assert_eq!(mgr.active_index(), 1);
    }

    #[test]
    fn set_active_clamps() {
        let mut mgr = two_buffer_mgr();
        mgr.set_active(100);
        assert_eq!(mgr.active_index(), 1); // clamped to last valid index

        mgr.set_active(0);
        assert_eq!(mgr.active_index(), 0);
    }

    #[test]
    fn close_buffer_removes_and_adjusts_active() {
        let mut mgr = two_buffer_mgr();
        // active is 1, close buffer 0 => active should become 0
        mgr.close_buffer(0);
        assert_eq!(mgr.buffer_count(), 1);
        assert_eq!(mgr.active_index(), 0);
    }

    #[test]
    fn close_buffer_last_index_adjusts() {
        let mut mgr = two_buffer_mgr();
        mgr.set_active(1);
        // Close the last buffer (index 1), active should clamp to 0
        mgr.close_buffer(1);
        assert_eq!(mgr.buffer_count(), 1);
        assert_eq!(mgr.active_index(), 0);
    }

    #[test]
    fn close_buffer_out_of_bounds_noop() {
        let mut mgr = two_buffer_mgr();
        mgr.close_buffer(10);
        assert_eq!(mgr.buffer_count(), 2);
    }

    #[test]
    fn next_buffer_noop_when_empty() {
        let mut mgr = BufferManager::new();
        mgr.next_buffer();
        assert_eq!(mgr.active_index(), 0);
    }

    #[test]
    fn next_buffer_noop_when_single() {
        let mut tmp = tempfile::NamedTempFile::new().unwrap();
        writeln!(tmp, "solo").unwrap();
        tmp.flush().unwrap();

        let mut mgr = BufferManager::new();
        mgr.open_file(tmp.path()).unwrap();
        mgr.next_buffer();
        assert_eq!(mgr.active_index(), 0);
    }
}
