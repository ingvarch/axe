use super::AppState;

impl AppState {
    // IMPACT ANALYSIS — poll_fs_events
    // Parents: main loop calls this each iteration after poll_terminal().
    // Children: FileWatcher::has_changes() (drains events), FileTree::refresh_tree()
    //           (preserves expanded/selection), refresh_git_modified_files().
    // Siblings: poll_terminal, poll_lsp — independent polling loops.
    // Risk: None — debounced, non-blocking, read-only on watcher state.

    /// Polls the filesystem watcher for external changes and refreshes the tree
    /// if any relevant events (create, remove, rename) were detected.
    pub fn poll_fs_events(&mut self) {
        let changed = self.file_watcher.as_mut().is_some_and(|w| w.has_changes());
        if changed {
            if let Some(ref mut tree) = self.file_tree {
                tree.refresh_tree();
            }
            self.refresh_git_modified_files();
            // Reload open buffers whose files changed on disk (e.g. after
            // `git checkout .` in the terminal). Only reloads unmodified buffers.
            self.buffer_manager.reload_unmodified_buffers();
            // Refresh editor diff hunks — git operations in the terminal
            // change HEAD, so the active buffer's diff markers must be recalculated.
            self.refresh_active_buffer_diff_hunks();
        }
    }

    /// Scrolls the file tree vertically by the given delta lines.
    pub(super) fn tree_scroll(&mut self, delta: i32) {
        if let Some(ref mut tree) = self.file_tree {
            tree.scroll_by(delta);
        }
    }

    /// Scrolls the file tree horizontally by the given delta columns.
    ///
    /// Clamped to max content width so the view can't scroll into empty space.
    pub(super) fn tree_scroll_horizontal(&mut self, delta: i32) {
        /// Indent chars per depth level (must match `TREE_INDENT` in axe-ui).
        const TREE_INDENT: usize = 2;
        /// Extra chars for icon/prefix per node (icon + space).
        const ICON_OVERHEAD: usize = 2;

        if let Some(ref mut tree) = self.file_tree {
            tree.scroll_horizontally_by(delta, TREE_INDENT, ICON_OVERHEAD);
        }
    }

    // IMPACT ANALYSIS — screen_to_tree_node_index
    // Parents: handle_mouse_event() calls this for Down events to detect tree item clicks.
    // Children: None — pure conversion function returning Option<usize>.
    // Siblings: tree_inner_area (must be set by main.rs each frame),
    //           file_tree scroll offset (used to convert screen row to node index).
    // Risk: None — stateless helper, cannot corrupt state.

    /// Converts screen coordinates to a tree node index.
    ///
    /// Returns `None` if the coordinates are outside the tree inner area,
    /// no tree is loaded, or the click is below the last visible node.
    pub(super) fn screen_to_tree_node_index(&self, col: u16, row: u16) -> Option<usize> {
        let (tx, ty, tw, th) = self.tree_inner_area?;
        let tree = self.file_tree.as_ref()?;
        if col < tx || col >= tx + tw || row < ty || row >= ty + th {
            return None;
        }
        let relative_row = (row - ty) as usize;
        let node_index = tree.scroll() + relative_row;
        if node_index < tree.visible_nodes().len() {
            Some(node_index)
        } else {
            None
        }
    }
}
