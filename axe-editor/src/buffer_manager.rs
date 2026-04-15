use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use crate::buffer::EditorBuffer;

/// A single contiguous text edit within one file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextEdit {
    pub start_line: usize,
    pub start_col: usize,
    pub end_line: usize,
    pub end_col: usize,
    pub new_text: String,
}

/// All edits targeted at one file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileEdit {
    pub path: PathBuf,
    pub edits: Vec<TextEdit>,
}

/// A workspace edit spanning one or more files (the subset of LSP
/// `WorkspaceEdit` that Axe currently honours).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct WorkspaceEdit {
    pub files: Vec<FileEdit>,
}

/// Summary returned by [`BufferManager::apply_workspace_edit`].
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AppliedWorkspaceEdit {
    /// Number of files touched (including files newly opened for the edit).
    pub files_affected: usize,
    /// Total number of individual text edits applied.
    pub edits_applied: usize,
}

/// Manages multiple open editor buffers and tracks the active one.
///
/// Prevents duplicate opens of the same file path by switching to
/// the existing buffer instead.
pub struct BufferManager {
    buffers: Vec<EditorBuffer>,
    active: usize,
    /// Tab size to apply to newly created buffers.
    tab_size: usize,
    /// Whether new buffers should insert spaces for tabs.
    insert_spaces: bool,
}

impl Default for BufferManager {
    fn default() -> Self {
        Self::new()
    }
}

impl BufferManager {
    /// Creates a new empty buffer manager with no open buffers.
    ///
    /// Uses default tab configuration (4 spaces, insert spaces mode).
    pub fn new() -> Self {
        Self {
            buffers: Vec::new(),
            active: 0,
            tab_size: 4,
            insert_spaces: true,
        }
    }

    /// Creates a new buffer manager with custom tab configuration.
    ///
    /// All subsequently opened or created buffers will inherit these settings.
    pub fn with_editor_config(tab_size: usize, insert_spaces: bool) -> Self {
        Self {
            buffers: Vec::new(),
            active: 0,
            tab_size,
            insert_spaces,
        }
    }

    /// Returns the configured tab size for new buffers.
    pub fn tab_size(&self) -> usize {
        self.tab_size
    }

    /// Returns whether new buffers insert spaces for tabs.
    pub fn insert_spaces(&self) -> bool {
        self.insert_spaces
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

        let mut buffer = EditorBuffer::from_file(path)?;
        buffer.set_tab_config(self.tab_size, self.insert_spaces);
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

    /// Opens a file as a preview buffer, replacing any existing preview.
    ///
    /// If the file is already open (as preview or permanent), switches to it
    /// without changing its preview status. Otherwise, closes the existing
    /// preview buffer (if any) and opens a new one marked as preview.
    pub fn open_file_as_preview(&mut self, path: &Path) -> Result<()> {
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

        // Close existing preview buffer (if any).
        if let Some(preview_idx) = self.preview_index() {
            self.buffers.remove(preview_idx);
            if self.active > preview_idx && self.active > 0 {
                self.active -= 1;
            } else if self.active >= self.buffers.len() && !self.buffers.is_empty() {
                self.active = self.buffers.len() - 1;
            }
        }

        let mut buffer = EditorBuffer::from_file(path)?;
        buffer.set_tab_config(self.tab_size, self.insert_spaces);
        buffer.is_preview = true;
        self.buffers.push(buffer);
        self.active = self.buffers.len() - 1;
        Ok(())
    }

    /// Promotes the active buffer from preview to permanent.
    ///
    /// Does nothing if the active buffer is not a preview.
    pub fn promote_preview(&mut self) {
        if let Some(buf) = self.buffers.get_mut(self.active) {
            buf.is_preview = false;
        }
    }

    /// Promotes the active buffer if it has been modified.
    ///
    /// Called after edit operations to automatically convert a preview
    /// into a permanent buffer when the user starts editing.
    pub fn auto_promote_if_modified(&mut self) {
        if let Some(buf) = self.buffers.get_mut(self.active) {
            if buf.is_preview && buf.modified {
                buf.is_preview = false;
            }
        }
    }

    /// Returns the index of the current preview buffer, if any.
    fn preview_index(&self) -> Option<usize> {
        self.buffers.iter().position(|b| b.is_preview)
    }

    /// Reloads all unmodified buffers whose files changed on disk.
    ///
    /// Returns `true` if any buffer was reloaded.
    pub fn reload_unmodified_buffers(&mut self) -> bool {
        let mut any_reloaded = false;
        for buf in &mut self.buffers {
            if buf.reload_from_disk() {
                any_reloaded = true;
            }
        }
        any_reloaded
    }

    /// Returns a mutable reference to the buffer with the given file path, if any.
    ///
    /// Uses `std::fs::canonicalize` for path comparison.
    pub fn buffer_mut_by_path(&mut self, path: &Path) -> Option<&mut EditorBuffer> {
        let canonical = std::fs::canonicalize(path).ok()?;
        self.buffers.iter_mut().find(|buf| {
            buf.path()
                .and_then(|p| std::fs::canonicalize(p).ok())
                .is_some_and(|c| c == canonical)
        })
    }

    /// Applies an LSP-style workspace edit across one or more files.
    ///
    /// For every `FileEdit`:
    /// - Finds the open buffer for that path, or opens it from disk.
    /// - Sorts the edits by `(start_line desc, start_col desc)` so each edit
    ///   is applied right-to-left and earlier positions stay valid.
    /// - Starts a fresh labeled undo group (`label`) on that buffer and
    ///   applies every edit into it so a single undo reverts the whole
    ///   per-file group.
    ///
    /// Leaves the currently active buffer unchanged. Returns a summary of
    /// how many files and edits were applied. Errors bubble up from
    /// `EditorBuffer::from_file` when a file referenced by the edit does
    /// not exist or cannot be read.
    pub fn apply_workspace_edit(
        &mut self,
        workspace_edit: &WorkspaceEdit,
        label: &str,
    ) -> Result<AppliedWorkspaceEdit> {
        let mut summary = AppliedWorkspaceEdit::default();
        let previous_active = self.active;

        for file in &workspace_edit.files {
            if file.edits.is_empty() {
                continue;
            }

            // Make sure the buffer exists; open it if needed.
            let idx = match self.index_of_path(&file.path) {
                Some(i) => i,
                None => {
                    self.open_file(&file.path).with_context(|| {
                        format!("Failed to open {} for workspace edit", file.path.display())
                    })?;
                    self.buffers.len() - 1
                }
            };

            // Sort edits right-to-left so earlier positions remain valid as
            // we apply them one by one.
            let mut sorted = file.edits.clone();
            sorted.sort_by(|a, b| {
                b.start_line
                    .cmp(&a.start_line)
                    .then_with(|| b.start_col.cmp(&a.start_col))
            });

            let edit_count = sorted.len();
            let buffer = &mut self.buffers[idx];
            buffer.begin_labeled_undo_group(label);
            for edit in &sorted {
                buffer.apply_text_edit(
                    edit.start_line,
                    edit.start_col,
                    edit.end_line,
                    edit.end_col,
                    &edit.new_text,
                );
            }
            buffer.end_undo_group();

            summary.files_affected += 1;
            summary.edits_applied += edit_count;
        }

        // Restore original active index — applying edits across files should
        // not silently switch which buffer the user is looking at.
        if previous_active < self.buffers.len() {
            self.active = previous_active;
        }

        Ok(summary)
    }

    /// Returns the index of a buffer with the given canonical path, if any.
    fn index_of_path(&self, path: &Path) -> Option<usize> {
        let canonical = std::fs::canonicalize(path).ok()?;
        self.buffers.iter().position(|buf| {
            buf.path()
                .and_then(|p| std::fs::canonicalize(p).ok())
                .is_some_and(|c| c == canonical)
        })
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
        buf.cursor_mut().row = 42;

        assert_eq!(mgr.active_buffer().unwrap().cursor().row, 42);
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

    // --- preview buffer tests ---

    #[test]
    fn open_file_as_preview_marks_buffer_preview() {
        let mut tmp = tempfile::NamedTempFile::new().unwrap();
        writeln!(tmp, "hello").unwrap();
        tmp.flush().unwrap();

        let mut mgr = BufferManager::new();
        mgr.open_file_as_preview(tmp.path()).unwrap();
        assert_eq!(mgr.buffer_count(), 1);
        assert!(mgr.active_buffer().unwrap().is_preview);
    }

    #[test]
    fn open_preview_replaces_existing_preview() {
        let mut tmp1 = tempfile::NamedTempFile::new().unwrap();
        writeln!(tmp1, "file1").unwrap();
        tmp1.flush().unwrap();
        let mut tmp2 = tempfile::NamedTempFile::new().unwrap();
        writeln!(tmp2, "file2").unwrap();
        tmp2.flush().unwrap();

        let mut mgr = BufferManager::new();
        mgr.open_file_as_preview(tmp1.path()).unwrap();
        mgr.open_file_as_preview(tmp2.path()).unwrap();
        // Should replace, not add
        assert_eq!(mgr.buffer_count(), 1);
        assert!(mgr.active_buffer().unwrap().is_preview);
        let name = mgr
            .active_buffer()
            .unwrap()
            .file_name()
            .unwrap()
            .to_string();
        let expected = tmp2.path().file_name().unwrap().to_str().unwrap();
        assert_eq!(name, expected);
    }

    #[test]
    fn open_preview_does_not_replace_permanent_buffer() {
        let mut tmp1 = tempfile::NamedTempFile::new().unwrap();
        writeln!(tmp1, "permanent").unwrap();
        tmp1.flush().unwrap();
        let mut tmp2 = tempfile::NamedTempFile::new().unwrap();
        writeln!(tmp2, "preview").unwrap();
        tmp2.flush().unwrap();

        let mut mgr = BufferManager::new();
        mgr.open_file(tmp1.path()).unwrap(); // permanent
        mgr.open_file_as_preview(tmp2.path()).unwrap();
        assert_eq!(mgr.buffer_count(), 2);
        assert!(!mgr.buffers()[0].is_preview);
        assert!(mgr.active_buffer().unwrap().is_preview);
    }

    #[test]
    fn promote_preview_makes_buffer_permanent() {
        let mut tmp = tempfile::NamedTempFile::new().unwrap();
        writeln!(tmp, "hello").unwrap();
        tmp.flush().unwrap();

        let mut mgr = BufferManager::new();
        mgr.open_file_as_preview(tmp.path()).unwrap();
        assert!(mgr.active_buffer().unwrap().is_preview);
        mgr.promote_preview();
        assert!(!mgr.active_buffer().unwrap().is_preview);
    }

    #[test]
    fn open_preview_on_already_open_file_switches_to_it() {
        let mut tmp = tempfile::NamedTempFile::new().unwrap();
        writeln!(tmp, "content").unwrap();
        tmp.flush().unwrap();

        let mut mgr = BufferManager::new();
        mgr.open_file(tmp.path()).unwrap(); // permanent
        mgr.open_file_as_preview(tmp.path()).unwrap(); // should just switch
        assert_eq!(mgr.buffer_count(), 1);
        assert!(!mgr.active_buffer().unwrap().is_preview); // stays permanent
    }

    #[test]
    fn buffer_mut_by_path_no_match() {
        let mut mgr = BufferManager::new();
        assert!(mgr.buffer_mut_by_path(Path::new("/nonexistent")).is_none());
    }

    #[test]
    fn buffer_mut_by_path_finds_buffer() {
        let mut tmp = tempfile::NamedTempFile::new().unwrap();
        writeln!(tmp, "hello").unwrap();
        tmp.flush().unwrap();

        let mut mgr = BufferManager::new();
        mgr.open_file(tmp.path()).unwrap();

        let buf = mgr.buffer_mut_by_path(tmp.path());
        assert!(buf.is_some());
    }

    #[test]
    fn editing_preview_promotes_it() {
        let mut tmp = tempfile::NamedTempFile::new().unwrap();
        writeln!(tmp, "hello").unwrap();
        tmp.flush().unwrap();

        let mut mgr = BufferManager::new();
        mgr.open_file_as_preview(tmp.path()).unwrap();
        assert!(mgr.active_buffer().unwrap().is_preview);
        // Simulate an edit by marking modified
        mgr.active_buffer_mut().unwrap().modified = true;
        mgr.auto_promote_if_modified();
        assert!(!mgr.active_buffer().unwrap().is_preview);
    }

    // --- tab config propagation tests ---

    #[test]
    fn with_editor_config_stores_settings() {
        let mgr = BufferManager::with_editor_config(2, false);
        assert_eq!(mgr.buffer_count(), 0);
        // Settings are stored and will be applied to newly opened buffers.
        assert_eq!(mgr.tab_size(), 2);
        assert!(!mgr.insert_spaces());
    }

    #[test]
    fn with_editor_config_passes_to_opened_buffers() {
        let mut tmp = tempfile::NamedTempFile::new().unwrap();
        writeln!(tmp, "hello").unwrap();
        tmp.flush().unwrap();

        let mut mgr = BufferManager::with_editor_config(2, false);
        mgr.open_file(tmp.path()).unwrap();

        let buf = mgr.active_buffer().unwrap();
        assert_eq!(buf.tab_size(), 2);
        assert!(!buf.insert_spaces());
    }

    #[test]
    fn default_manager_uses_default_tab_config() {
        let mut tmp = tempfile::NamedTempFile::new().unwrap();
        writeln!(tmp, "hello").unwrap();
        tmp.flush().unwrap();

        let mut mgr = BufferManager::new();
        mgr.open_file(tmp.path()).unwrap();

        let buf = mgr.active_buffer().unwrap();
        assert_eq!(buf.tab_size(), 4);
        assert!(buf.insert_spaces());
    }

    // --- Workspace edit applier tests ---

    fn text_edit(sl: usize, sc: usize, el: usize, ec: usize, new: &str) -> TextEdit {
        TextEdit {
            start_line: sl,
            start_col: sc,
            end_line: el,
            end_col: ec,
            new_text: new.to_string(),
        }
    }

    #[test]
    fn apply_workspace_edit_single_buffer_multi_edit_reverts_as_one_undo() {
        let mut tmp = tempfile::NamedTempFile::new().unwrap();
        write!(tmp, "foo foo foo").unwrap();
        tmp.flush().unwrap();

        let mut mgr = BufferManager::new();
        mgr.open_file(tmp.path()).unwrap();

        let edit = WorkspaceEdit {
            files: vec![FileEdit {
                path: tmp.path().to_path_buf(),
                edits: vec![
                    text_edit(0, 0, 0, 3, "bar"),
                    text_edit(0, 4, 0, 7, "bar"),
                    text_edit(0, 8, 0, 11, "bar"),
                ],
            }],
        };
        let summary = mgr.apply_workspace_edit(&edit, "Rename").unwrap();
        assert_eq!(summary.files_affected, 1);
        assert_eq!(summary.edits_applied, 3);
        assert_eq!(mgr.active_buffer().unwrap().content_string(), "bar bar bar");

        // A single undo must revert all three edits atomically.
        mgr.active_buffer_mut().unwrap().undo();
        assert_eq!(mgr.active_buffer().unwrap().content_string(), "foo foo foo");
    }

    #[test]
    fn apply_workspace_edit_labels_group() {
        let mut tmp = tempfile::NamedTempFile::new().unwrap();
        write!(tmp, "abc").unwrap();
        tmp.flush().unwrap();

        let mut mgr = BufferManager::new();
        mgr.open_file(tmp.path()).unwrap();
        let edit = WorkspaceEdit {
            files: vec![FileEdit {
                path: tmp.path().to_path_buf(),
                edits: vec![text_edit(0, 0, 0, 3, "xyz")],
            }],
        };
        mgr.apply_workspace_edit(&edit, "Rename").unwrap();

        // Undo to inspect the group's label via the returned EditGroup — we
        // can observe it indirectly: the content reverts in a single step.
        let buf = mgr.active_buffer_mut().unwrap();
        buf.undo();
        assert_eq!(buf.content_string(), "abc");
    }

    #[test]
    fn apply_workspace_edit_opens_unopened_file() {
        let mut tmp1 = tempfile::NamedTempFile::new().unwrap();
        write!(tmp1, "one").unwrap();
        tmp1.flush().unwrap();
        let mut tmp2 = tempfile::NamedTempFile::new().unwrap();
        write!(tmp2, "two").unwrap();
        tmp2.flush().unwrap();

        let mut mgr = BufferManager::new();
        mgr.open_file(tmp1.path()).unwrap();
        assert_eq!(mgr.buffer_count(), 1);

        let edit = WorkspaceEdit {
            files: vec![FileEdit {
                path: tmp2.path().to_path_buf(),
                edits: vec![text_edit(0, 0, 0, 3, "TWO")],
            }],
        };
        let summary = mgr.apply_workspace_edit(&edit, "Rename").unwrap();
        assert_eq!(summary.files_affected, 1);
        assert_eq!(mgr.buffer_count(), 2, "second file must be opened");
        // Active stays on the originally-active buffer.
        assert_eq!(mgr.active_index(), 0);
        // Find the second buffer and verify its content.
        let second = mgr
            .buffers()
            .iter()
            .find(|b| b.path() == Some(tmp2.path()))
            .unwrap();
        assert_eq!(second.content_string(), "TWO");
    }

    #[test]
    fn apply_workspace_edit_multiple_files_single_call() {
        let mut tmp1 = tempfile::NamedTempFile::new().unwrap();
        write!(tmp1, "foo").unwrap();
        tmp1.flush().unwrap();
        let mut tmp2 = tempfile::NamedTempFile::new().unwrap();
        write!(tmp2, "foo").unwrap();
        tmp2.flush().unwrap();

        let mut mgr = BufferManager::new();
        mgr.open_file(tmp1.path()).unwrap();
        mgr.open_file(tmp2.path()).unwrap();

        let edit = WorkspaceEdit {
            files: vec![
                FileEdit {
                    path: tmp1.path().to_path_buf(),
                    edits: vec![text_edit(0, 0, 0, 3, "bar")],
                },
                FileEdit {
                    path: tmp2.path().to_path_buf(),
                    edits: vec![text_edit(0, 0, 0, 3, "baz")],
                },
            ],
        };
        let summary = mgr.apply_workspace_edit(&edit, "Rename").unwrap();
        assert_eq!(summary.files_affected, 2);
        assert_eq!(summary.edits_applied, 2);

        let b1 = mgr
            .buffers()
            .iter()
            .find(|b| b.path() == Some(tmp1.path()))
            .unwrap();
        assert_eq!(b1.content_string(), "bar");
        let b2 = mgr
            .buffers()
            .iter()
            .find(|b| b.path() == Some(tmp2.path()))
            .unwrap();
        assert_eq!(b2.content_string(), "baz");
    }

    #[test]
    fn apply_workspace_edit_applies_right_to_left() {
        // If edits were applied in their original order, an early insertion
        // would shift the coordinates of later edits and break them. The
        // applier sorts descending, so both succeed cleanly.
        let mut tmp = tempfile::NamedTempFile::new().unwrap();
        write!(tmp, "aXXXb").unwrap();
        tmp.flush().unwrap();

        let mut mgr = BufferManager::new();
        mgr.open_file(tmp.path()).unwrap();

        // Insert "(" before col 1 and ")" before col 4 — result "a(XXX)b".
        let edit = WorkspaceEdit {
            files: vec![FileEdit {
                path: tmp.path().to_path_buf(),
                edits: vec![text_edit(0, 1, 0, 1, "("), text_edit(0, 4, 0, 4, ")")],
            }],
        };
        mgr.apply_workspace_edit(&edit, "Wrap").unwrap();
        assert_eq!(mgr.active_buffer().unwrap().content_string(), "a(XXX)b");
    }

    #[test]
    fn apply_workspace_edit_empty_files_is_noop() {
        let mut mgr = BufferManager::new();
        let summary = mgr
            .apply_workspace_edit(&WorkspaceEdit::default(), "Rename")
            .unwrap();
        assert_eq!(summary.files_affected, 0);
        assert_eq!(summary.edits_applied, 0);
    }
}
