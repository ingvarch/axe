mod filter;
pub mod icons;
pub mod watcher;

pub use filter::TreeFilter;
pub use watcher::FileWatcher;

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};

/// Active file operation state in the tree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TreeAction {
    /// No active action.
    Idle,
    /// Creating a new file or directory. Input is the name being typed.
    Creating { is_dir: bool, input: String },
    /// Renaming the selected node. `node_idx` is the node being renamed.
    Renaming { node_idx: usize, input: String },
    /// Confirming deletion of the selected node.
    ConfirmDelete { node_idx: usize },
}

/// Identifies what kind of filesystem entry a node represents.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NodeKind {
    File { size: u64, language: Option<String> },
    Directory { child_count: usize },
    Symlink { target: PathBuf },
}

/// A single node in the file tree (file, directory, or symlink).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TreeNode {
    pub path: PathBuf,
    pub name: String,
    pub kind: NodeKind,
    pub depth: usize,
    pub expanded: bool,
    pub children_loaded: bool,
    pub git_status: Option<String>,
    pub parent: Option<usize>,
}

/// A flat-vec file tree representing a project directory.
///
/// Nodes are stored in display order: the root at index 0, followed by its
/// visible children. Directories that are expanded have their children
/// inserted immediately after them.
pub struct FileTree {
    root: PathBuf,
    nodes: Vec<TreeNode>,
    selected: usize,
    scroll: usize,
    viewport_height: usize,
    viewport_width: usize,
    filter: TreeFilter,
    action: TreeAction,
    show_icons: bool,
    scroll_col: usize,
}

impl FileTree {
    /// Builds a `FileTree` by reading the given root directory.
    ///
    /// The root directory itself becomes the first node (expanded).
    /// Its immediate children are loaded, sorted (directories first,
    /// then alphabetically), with hidden files (names starting with `.`) excluded.
    pub fn new(root: PathBuf) -> Result<Self> {
        let root = root
            .canonicalize()
            .with_context(|| format!("Failed to canonicalize path: {}", root.display()))?;

        let filter = TreeFilter::new(&root);

        let root_name = root
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| root.to_string_lossy().into_owned());

        let mut nodes = Vec::new();

        // Root node — always expanded with children loaded.
        let child_count = Self::count_visible_children(&root, &filter);
        nodes.push(TreeNode {
            path: root.clone(),
            name: root_name,
            kind: NodeKind::Directory { child_count },
            depth: 0,
            expanded: true,
            children_loaded: true,
            git_status: None,
            parent: None,
        });

        // Read and sort children.
        let children = Self::read_children(&root, 1, 0, &filter)?;
        nodes.extend(children);

        Ok(Self {
            root,
            nodes,
            selected: 0,
            scroll: 0,
            viewport_height: usize::MAX,
            viewport_width: usize::MAX,
            filter,
            action: TreeAction::Idle,
            show_icons: true,
            scroll_col: 0,
        })
    }

    /// Returns the root directory path.
    pub fn root_path(&self) -> &Path {
        &self.root
    }

    /// Returns all nodes in display order.
    pub fn visible_nodes(&self) -> &[TreeNode] {
        &self.nodes
    }

    /// Returns the root directory name.
    pub fn root_name(&self) -> &str {
        &self.nodes[0].name
    }

    /// Returns the currently selected node index.
    pub fn selected(&self) -> usize {
        self.selected
    }

    /// Returns the current scroll offset.
    pub fn scroll(&self) -> usize {
        self.scroll
    }

    /// Returns a reference to all nodes.
    pub fn nodes(&self) -> &[TreeNode] {
        &self.nodes
    }

    /// Sets the viewport height for scroll calculations.
    pub fn set_viewport_height(&mut self, h: usize) {
        self.viewport_height = h;
        self.clamp_scroll();
    }

    /// Sets the viewport width for horizontal scroll clamping.
    pub fn set_viewport_width(&mut self, w: usize) {
        self.viewport_width = w;
        self.clamp_scroll_col();
    }

    /// Returns a reference to the currently selected node, if any.
    pub fn selected_node(&self) -> Option<&TreeNode> {
        self.nodes.get(self.selected)
    }

    /// Moves selection up by one. Wraps from first to last.
    pub fn move_up(&mut self) {
        if self.nodes.is_empty() {
            return;
        }
        if self.selected == 0 {
            self.selected = self.nodes.len() - 1;
        } else {
            self.selected -= 1;
        }
        self.adjust_scroll();
    }

    /// Moves selection down by one. Wraps from last to first.
    pub fn move_down(&mut self) {
        if self.nodes.is_empty() {
            return;
        }
        if self.selected >= self.nodes.len() - 1 {
            self.selected = 0;
        } else {
            self.selected += 1;
        }
        self.adjust_scroll();
    }

    /// Selects the node at the given index directly.
    ///
    /// Returns `true` if the selection changed, `false` if out of bounds.
    pub fn select(&mut self, index: usize) -> bool {
        if index < self.nodes.len() {
            self.selected = index;
            self.adjust_scroll();
            true
        } else {
            false
        }
    }

    /// Jumps selection to the first item and resets scroll.
    pub fn move_home(&mut self) {
        self.selected = 0;
        self.scroll = 0;
    }

    /// Jumps selection to the last item and adjusts scroll.
    pub fn move_end(&mut self) {
        if !self.nodes.is_empty() {
            self.selected = self.nodes.len() - 1;
        }
        self.adjust_scroll();
    }

    /// Expands the selected directory, loading its children into the flat vec.
    ///
    /// Noop if the selected node is a file or already expanded.
    pub fn expand(&mut self) -> Result<()> {
        self.expand_at(self.selected)?;
        Ok(())
    }

    /// Expands a directory at the given index, loading its children.
    ///
    /// Unlike `expand()`, this does not use `self.selected`. Returns the number
    /// of children inserted. Noop (returns 0) if the index is out of bounds,
    /// points to a non-directory, or the directory is already expanded.
    pub fn expand_at(&mut self, index: usize) -> Result<usize> {
        if index >= self.nodes.len() {
            return Ok(0);
        }
        let node = &self.nodes[index];
        if !matches!(node.kind, NodeKind::Directory { .. }) || node.expanded {
            return Ok(0);
        }

        let dir_path = node.path.clone();
        let depth = node.depth + 1;

        self.filter.add_nested_gitignore(&dir_path);
        let children = Self::read_children(&dir_path, depth, index, &self.filter)?;
        let count = children.len();

        self.nodes[index].expanded = true;
        self.nodes[index].children_loaded = true;

        let insert_pos = index + 1;
        for (i, child) in children.into_iter().enumerate() {
            self.nodes.insert(insert_pos + i, child);
        }

        Ok(count)
    }

    /// Collapses the selected directory, removing all its descendants.
    ///
    /// Noop if the selected node is a file or already collapsed.
    pub fn collapse(&mut self) {
        if self.selected >= self.nodes.len() {
            return;
        }
        let node = &self.nodes[self.selected];
        if !matches!(node.kind, NodeKind::Directory { .. }) || !node.expanded {
            return;
        }

        let selected_depth = node.depth;
        // Find the range of descendants: all nodes after selected with depth > selected_depth.
        let mut end = self.selected + 1;
        while end < self.nodes.len() && self.nodes[end].depth > selected_depth {
            end += 1;
        }

        self.nodes.drain((self.selected + 1)..end);
        self.nodes[self.selected].expanded = false;
        self.adjust_scroll();
    }

    /// Toggles expand/collapse on the selected directory.
    ///
    /// If collapsed directory: expand. If expanded directory: collapse. File: noop.
    pub fn toggle(&mut self) -> Result<()> {
        if self.selected >= self.nodes.len() {
            return Ok(());
        }
        if !matches!(self.nodes[self.selected].kind, NodeKind::Directory { .. }) {
            return Ok(());
        }
        if self.nodes[self.selected].expanded {
            self.collapse();
        } else {
            self.expand()?;
        }
        Ok(())
    }

    /// Collapses the selected directory if expanded, otherwise navigates to parent.
    pub fn collapse_or_parent(&mut self) {
        if self.selected >= self.nodes.len() {
            return;
        }
        let node = &self.nodes[self.selected];

        if matches!(node.kind, NodeKind::Directory { .. }) && node.expanded {
            self.collapse();
        } else if let Some(parent_idx) = self.find_parent_index(self.selected) {
            self.selected = parent_idx;
            self.adjust_scroll();
        }
    }

    /// Toggles visibility of gitignored files and refreshes the tree.
    ///
    /// Preserves expanded directories and selection while reflecting the new filter state.
    pub fn toggle_show_ignored(&mut self) {
        self.filter.toggle_show_ignored();
        self.refresh_tree();
    }

    /// Returns whether ignored files are currently shown.
    pub fn show_ignored(&self) -> bool {
        self.filter.show_ignored()
    }

    /// Returns whether file icons are currently shown.
    pub fn show_icons(&self) -> bool {
        self.show_icons
    }

    // IMPACT ANALYSIS — toggle_show_icons
    // Parents: Command::ToggleIcons dispatched from Ctrl+I in app.rs.
    // Children: None — only flips a bool. Rendering reads show_icons() next frame.
    // Siblings: show_ignored toggle — similar pattern, independent flag.

    /// Toggles display of file type icons in the tree.
    pub fn toggle_show_icons(&mut self) {
        self.show_icons = !self.show_icons;
    }

    /// Sets whether file type icons are shown in the tree.
    pub fn set_show_icons(&mut self, show: bool) {
        self.show_icons = show;
    }

    /// Sets whether ignored/hidden files are shown in the tree.
    pub fn set_show_ignored(&mut self, show: bool) {
        self.filter.set_show_ignored(show);
    }

    /// Returns the current horizontal scroll offset.
    pub fn scroll_col(&self) -> usize {
        self.scroll_col
    }

    // IMPACT ANALYSIS — scroll_by
    // Parents: Mouse wheel events and keyboard shortcuts in app.rs.
    // Children: None — only modifies scroll offset.
    // Siblings: adjust_scroll (selection-driven), move_up/move_down (selection-driven).
    // Risk: None — scroll is purely visual, clamped to valid range.

    /// Scrolls the tree vertically by `delta` lines (positive = down, negative = up).
    ///
    /// Clamped to `0..=max_scroll` where `max_scroll = nodes.len() - viewport_height`.
    pub fn scroll_by(&mut self, delta: i32) {
        let max_scroll = self.nodes.len().saturating_sub(self.viewport_height);
        if delta >= 0 {
            self.scroll = self.scroll.saturating_add(delta as usize).min(max_scroll);
        } else {
            self.scroll = self.scroll.saturating_sub(delta.unsigned_abs() as usize);
        }
    }

    // IMPACT ANALYSIS — scroll_horizontally_by
    // Parents: Mouse wheel + Shift, Shift+Arrow keys in app.rs.
    // Children: render_tree_content reads scroll_col to offset line text.
    // Siblings: scroll_by (vertical) — independent axis.
    // Risk: None — clamped at 0, rendering handles overflow.

    /// Computes the maximum content width across all nodes.
    ///
    /// `indent_per_level` is the number of chars per depth level (e.g. 2).
    /// `icon_overhead` is the extra chars for icon/prefix per node (e.g. 2).
    pub fn max_content_width(&self, indent_per_level: usize, icon_overhead: usize) -> usize {
        self.nodes
            .iter()
            .map(|n| n.depth * indent_per_level + icon_overhead + n.name.chars().count())
            .max()
            .unwrap_or(0)
    }

    /// Scrolls the tree horizontally by `delta` columns (positive = right, negative = left).
    ///
    /// Clamped so the view never scrolls past the longest line:
    /// `max_scroll = max_content_width - viewport_width`.
    pub fn scroll_horizontally_by(
        &mut self,
        delta: i32,
        indent_per_level: usize,
        icon_overhead: usize,
    ) {
        if delta >= 0 {
            self.scroll_col = self.scroll_col.saturating_add(delta as usize);
        } else {
            self.scroll_col = self
                .scroll_col
                .saturating_sub(delta.unsigned_abs() as usize);
        }
        self.clamp_scroll_col_with(indent_per_level, icon_overhead);
    }

    /// Clamps horizontal scroll so content remains visible.
    fn clamp_scroll_col_with(&mut self, indent_per_level: usize, icon_overhead: usize) {
        let max_width = self.max_content_width(indent_per_level, icon_overhead);
        let max_scroll = max_width.saturating_sub(self.viewport_width);
        self.scroll_col = self.scroll_col.min(max_scroll);
    }

    /// Clamps horizontal scroll using default indent/icon values.
    fn clamp_scroll_col(&mut self) {
        // Default values matching TREE_INDENT (2) and icon overhead (2).
        self.clamp_scroll_col_with(2, 2);
    }

    /// Returns the current tree action state.
    pub fn action(&self) -> &TreeAction {
        &self.action
    }

    /// Returns `true` if a tree action (create, rename, delete confirm) is active.
    pub fn is_action_active(&self) -> bool {
        !matches!(self.action, TreeAction::Idle)
    }

    /// Begins creating a new file at the current selection position.
    pub fn start_create_file(&mut self) {
        self.action = TreeAction::Creating {
            is_dir: false,
            input: String::new(),
        };
    }

    /// Begins creating a new directory at the current selection position.
    pub fn start_create_dir(&mut self) {
        self.action = TreeAction::Creating {
            is_dir: true,
            input: String::new(),
        };
    }

    /// Begins renaming the selected node. Noop on root (index 0).
    pub fn start_rename(&mut self) {
        if self.selected == 0 || self.selected >= self.nodes.len() {
            return;
        }
        let name = self.nodes[self.selected].name.clone();
        self.action = TreeAction::Renaming {
            node_idx: self.selected,
            input: name,
        };
    }

    /// Begins delete confirmation for the selected node. Noop on root (index 0).
    pub fn start_delete(&mut self) {
        if self.selected == 0 || self.selected >= self.nodes.len() {
            return;
        }
        self.action = TreeAction::ConfirmDelete {
            node_idx: self.selected,
        };
    }

    /// Cancels the current action, resetting to Idle.
    pub fn cancel_action(&mut self) {
        self.action = TreeAction::Idle;
    }

    /// Appends a character to the current input (Creating or Renaming only).
    pub fn input_char(&mut self, c: char) {
        match &mut self.action {
            TreeAction::Creating { ref mut input, .. }
            | TreeAction::Renaming { ref mut input, .. } => {
                input.push(c);
            }
            _ => {}
        }
    }

    /// Removes the last character from the current input.
    pub fn input_backspace(&mut self) {
        match &mut self.action {
            TreeAction::Creating { ref mut input, .. }
            | TreeAction::Renaming { ref mut input, .. } => {
                input.pop();
            }
            _ => {}
        }
    }

    /// Validates a file/directory name.
    fn validate_name(name: &str) -> Result<()> {
        if name.is_empty() {
            bail!("Name cannot be empty");
        }
        if name == "." || name == ".." {
            bail!("Invalid name: '{name}'");
        }
        if name.contains('/') || name.contains('\0') {
            bail!("Name contains invalid character");
        }
        Ok(())
    }

    /// Returns the parent directory path for a new file/directory operation.
    ///
    /// If the selected node is a directory, creates inside it.
    /// Otherwise, creates alongside the selected file (in its parent directory).
    fn parent_dir_for_create(&self) -> PathBuf {
        let node = &self.nodes[self.selected];
        if matches!(node.kind, NodeKind::Directory { .. }) {
            node.path.clone()
        } else {
            node.path
                .parent()
                .map(|p| p.to_path_buf())
                .unwrap_or_else(|| self.root.clone())
        }
    }

    /// Confirms and executes the current action (create or rename).
    pub fn confirm_action(&mut self) -> Result<()> {
        match self.action.clone() {
            TreeAction::Creating { is_dir, input } => {
                Self::validate_name(&input)?;
                let parent = self.parent_dir_for_create();
                let new_path = parent.join(&input);
                if is_dir {
                    std::fs::create_dir(&new_path).with_context(|| {
                        format!("Failed to create directory: {}", new_path.display())
                    })?;
                } else {
                    std::fs::write(&new_path, "").with_context(|| {
                        format!("Failed to create file: {}", new_path.display())
                    })?;
                }
                self.action = TreeAction::Idle;
                self.refresh_tree();
                Ok(())
            }
            TreeAction::Renaming { node_idx, input } => {
                Self::validate_name(&input)?;
                if node_idx >= self.nodes.len() {
                    bail!("Invalid node index");
                }
                let old_path = &self.nodes[node_idx].path;
                let new_path = old_path
                    .parent()
                    .map(|p| p.join(&input))
                    .unwrap_or_else(|| PathBuf::from(&input));
                std::fs::rename(old_path, &new_path)
                    .with_context(|| format!("Failed to rename: {}", old_path.display()))?;
                self.action = TreeAction::Idle;
                self.refresh_tree();
                Ok(())
            }
            _ => Ok(()),
        }
    }

    /// Confirms and executes a delete operation.
    pub fn confirm_delete(&mut self) -> Result<()> {
        if let TreeAction::ConfirmDelete { node_idx } = self.action {
            if node_idx >= self.nodes.len() {
                bail!("Invalid node index");
            }
            let path = &self.nodes[node_idx].path;
            if path.is_dir() {
                std::fs::remove_dir_all(path)
                    .with_context(|| format!("Failed to delete directory: {}", path.display()))?;
            } else {
                std::fs::remove_file(path)
                    .with_context(|| format!("Failed to delete file: {}", path.display()))?;
            }
            self.action = TreeAction::Idle;
            self.refresh_tree();
        }
        Ok(())
    }

    /// Re-expands directories from a set of previously expanded paths.
    ///
    /// Walks the tree in forward order (parent before child) and expands
    /// any collapsed directory whose path is in the given set. This ensures
    /// correct ordering because `expand_at` inserts children after the node.
    pub fn restore_expanded(&mut self, paths: &HashSet<PathBuf>) {
        let mut i = 0;
        while i < self.nodes.len() {
            if matches!(self.nodes[i].kind, NodeKind::Directory { .. })
                && !self.nodes[i].expanded
                && paths.contains(&self.nodes[i].path)
            {
                let _ = self.expand_at(i);
            }
            i += 1;
        }
    }

    /// Selects the node whose path matches the given path.
    ///
    /// If no node matches, tries parent directories. If nothing matches,
    /// selection is unchanged.
    pub fn set_selected_by_path(&mut self, path: &Path) {
        if let Some(idx) = self.nodes.iter().position(|n| n.path == path) {
            self.selected = idx;
            self.adjust_scroll();
            return;
        }
        // Try parent directories.
        let mut candidate = path.parent();
        while let Some(parent) = candidate {
            if let Some(idx) = self.nodes.iter().position(|n| n.path == parent) {
                self.selected = idx;
                self.adjust_scroll();
                return;
            }
            candidate = parent.parent();
        }
    }

    /// Sets the vertical scroll offset directly.
    ///
    /// Clamped so the scroll cannot exceed `nodes.len() - viewport_height`.
    pub fn set_scroll(&mut self, scroll: usize) {
        let max_scroll = self.nodes.len().saturating_sub(self.viewport_height);
        self.scroll = scroll.min(max_scroll);
    }

    /// Rebuilds the tree from scratch: collapses all, reloads root children.
    fn rebuild_tree(&mut self) {
        let child_count = Self::count_visible_children(&self.root, &self.filter);
        self.nodes.truncate(1);
        self.nodes[0].kind = NodeKind::Directory { child_count };
        self.nodes[0].expanded = true;
        self.nodes[0].children_loaded = true;

        if let Ok(children) = Self::read_children(&self.root, 1, 0, &self.filter) {
            self.nodes.extend(children);
        }

        self.selected = 0;
        self.scroll = 0;
    }

    // IMPACT ANALYSIS — refresh_tree
    // Parents: confirm_action(), confirm_delete(), toggle_show_ignored() — called after FS ops.
    // Children: rebuild_tree() (resets tree), expand_at() (re-expands dirs).
    // Siblings: selection, scroll, action state — selection/scroll restored, action untouched.
    // Risk: Re-expansion order must be top-down (parent before child) because expand_at
    //        inserts children and shifts subsequent indices.

    /// Refreshes the tree while preserving expanded directories and selection.
    ///
    /// Snapshots expanded paths and selected path, rebuilds from disk, then
    /// re-expands directories in top-to-bottom order and restores selection.
    pub fn refresh_tree(&mut self) {
        // 1. Snapshot expanded directory paths (excluding root, which is always expanded).
        let expanded_paths: HashSet<PathBuf> = self
            .nodes
            .iter()
            .filter(|n| n.expanded && n.depth > 0)
            .map(|n| n.path.clone())
            .collect();

        // 2. Snapshot selected path.
        let selected_path = self.nodes.get(self.selected).map(|n| n.path.clone());

        // 3. Rebuild from scratch.
        self.rebuild_tree();

        // 4. Re-expand directories in forward order (parent before child).
        //    After each expand_at(), new children are inserted, shifting later indices.
        //    We scan forward to find the next directory to expand.
        let mut i = 0;
        while i < self.nodes.len() {
            if matches!(self.nodes[i].kind, NodeKind::Directory { .. })
                && !self.nodes[i].expanded
                && expanded_paths.contains(&self.nodes[i].path)
            {
                let _ = self.expand_at(i);
            }
            i += 1;
        }

        // 5. Restore selection to same path, or clamp to bounds.
        if let Some(ref path) = selected_path {
            if let Some(idx) = self.nodes.iter().position(|n| &n.path == path) {
                self.selected = idx;
            } else {
                // Path gone — try to find nearest parent.
                let mut candidate = path.parent();
                while let Some(parent) = candidate {
                    if let Some(idx) = self.nodes.iter().position(|n| n.path == parent) {
                        self.selected = idx;
                        break;
                    }
                    candidate = parent.parent();
                }
                // If no parent found, clamp to bounds.
                if self.selected >= self.nodes.len() {
                    self.selected = self.nodes.len().saturating_sub(1);
                }
            }
        }

        self.adjust_scroll();
    }

    /// Keeps the selected index within the visible scroll window.
    fn adjust_scroll(&mut self) {
        if self.selected < self.scroll {
            self.scroll = self.selected;
        } else if self.selected >= self.scroll + self.viewport_height {
            self.scroll = self.selected + 1 - self.viewport_height;
        }
    }

    /// Clamps scroll to the valid range without snapping to the selected item.
    ///
    /// Used by `set_viewport_height` to avoid fighting mouse-wheel scrolling.
    fn clamp_scroll(&mut self) {
        let max_scroll = self.nodes.len().saturating_sub(self.viewport_height);
        if self.scroll > max_scroll {
            self.scroll = max_scroll;
        }
    }

    /// Finds the parent node index by scanning backward for the first node
    /// with a lower depth.
    fn find_parent_index(&self, index: usize) -> Option<usize> {
        if index == 0 {
            return None;
        }
        let target_depth = self.nodes[index].depth;
        if target_depth == 0 {
            return None;
        }
        (0..index)
            .rev()
            .find(|&i| self.nodes[i].depth < target_depth)
    }

    /// Reads the children of a directory, filtering gitignored files,
    /// and sorting directories before files, alphabetically within each group.
    fn read_children(
        dir: &Path,
        depth: usize,
        parent_index: usize,
        filter: &TreeFilter,
    ) -> Result<Vec<TreeNode>> {
        let entries = std::fs::read_dir(dir)
            .with_context(|| format!("Failed to read directory: {}", dir.display()))?;

        let mut dirs = Vec::new();
        let mut files = Vec::new();

        for entry in entries {
            let entry = entry?;
            let name = entry.file_name().to_string_lossy().into_owned();
            let path = entry.path();
            let metadata = entry.metadata()?;
            let is_dir = metadata.is_dir();

            // Skip gitignored entries.
            if !filter.is_visible(&path, is_dir) {
                continue;
            }

            let node = if metadata.is_symlink() {
                let target = std::fs::read_link(&path).unwrap_or_default();
                TreeNode {
                    path,
                    name: name.clone(),
                    kind: NodeKind::Symlink { target },
                    depth,
                    expanded: false,
                    children_loaded: false,
                    git_status: None,
                    parent: Some(parent_index),
                }
            } else if is_dir {
                let child_count = Self::count_visible_children(&path, filter);
                TreeNode {
                    path,
                    name: name.clone(),
                    kind: NodeKind::Directory { child_count },
                    depth,
                    expanded: false,
                    children_loaded: false,
                    git_status: None,
                    parent: Some(parent_index),
                }
            } else {
                TreeNode {
                    path,
                    name: name.clone(),
                    kind: NodeKind::File {
                        size: metadata.len(),
                        language: None,
                    },
                    depth,
                    expanded: false,
                    children_loaded: false,
                    git_status: None,
                    parent: Some(parent_index),
                }
            };

            if is_dir {
                dirs.push(node);
            } else {
                files.push(node);
            }
        }

        // Sort alphabetically (case-insensitive) within each group.
        dirs.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
        files.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));

        // Directories first, then files.
        dirs.extend(files);
        Ok(dirs)
    }

    /// Counts visible entries in a directory (for `child_count`).
    fn count_visible_children(dir: &Path, filter: &TreeFilter) -> usize {
        std::fs::read_dir(dir)
            .map(|entries| {
                entries
                    .filter_map(|e| e.ok())
                    .filter(|e| {
                        let is_dir = e.file_type().map(|ft| ft.is_dir()).unwrap_or(false);
                        filter.is_visible(&e.path(), is_dir)
                    })
                    .count()
            })
            .unwrap_or(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    /// Creates a temp directory with a known structure for testing:
    /// root/
    ///   .hidden
    ///   alpha.txt
    ///   beta/
    ///   gamma.rs
    ///   delta/
    fn create_test_dir() -> TempDir {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();

        fs::create_dir(root.join("beta")).unwrap();
        fs::create_dir(root.join("delta")).unwrap();
        fs::write(root.join("alpha.txt"), "hello").unwrap();
        fs::write(root.join("gamma.rs"), "fn main() {}").unwrap();
        fs::write(root.join(".hidden"), "secret").unwrap();

        tmp
    }

    #[test]
    fn new_with_valid_directory() {
        let tmp = create_test_dir();
        let tree = FileTree::new(tmp.path().to_path_buf()).unwrap();
        // root + 2 dirs + 3 files (.hidden, alpha.txt, gamma.rs) = 6
        assert_eq!(tree.nodes().len(), 6);
    }

    #[test]
    fn root_node_is_directory() {
        let tmp = create_test_dir();
        let tree = FileTree::new(tmp.path().to_path_buf()).unwrap();
        assert!(
            matches!(tree.nodes()[0].kind, NodeKind::Directory { .. }),
            "root node should be a Directory"
        );
    }

    #[test]
    fn directories_before_files() {
        let tmp = create_test_dir();
        let tree = FileTree::new(tmp.path().to_path_buf()).unwrap();

        // Skip root (index 0). Children start at index 1.
        let children = &tree.nodes()[1..];
        let first_file_pos = children
            .iter()
            .position(|n| matches!(n.kind, NodeKind::File { .. }))
            .unwrap();
        let last_dir_pos = children
            .iter()
            .rposition(|n| matches!(n.kind, NodeKind::Directory { .. }))
            .unwrap();

        assert!(
            last_dir_pos < first_file_pos,
            "all directories should come before files"
        );
    }

    #[test]
    fn dot_files_included() {
        let tmp = create_test_dir();
        let tree = FileTree::new(tmp.path().to_path_buf()).unwrap();
        let names: Vec<&str> = tree.nodes()[1..].iter().map(|n| n.name.as_str()).collect();
        assert!(
            names.contains(&".hidden"),
            "dot-files should be included, found: {names:?}"
        );
    }

    #[test]
    fn children_depth_is_one() {
        let tmp = create_test_dir();
        let tree = FileTree::new(tmp.path().to_path_buf()).unwrap();
        for node in &tree.nodes()[1..] {
            assert_eq!(node.depth, 1, "child depth should be 1, got {}", node.depth);
        }
    }

    #[test]
    fn root_is_expanded() {
        let tmp = create_test_dir();
        let tree = FileTree::new(tmp.path().to_path_buf()).unwrap();
        assert!(tree.nodes()[0].expanded, "root should be expanded");
    }

    #[test]
    fn children_not_expanded() {
        let tmp = create_test_dir();
        let tree = FileTree::new(tmp.path().to_path_buf()).unwrap();
        for node in &tree.nodes()[1..] {
            if matches!(node.kind, NodeKind::Directory { .. }) {
                assert!(!node.expanded, "child dirs should not be expanded");
            }
        }
    }

    #[test]
    fn alphabetical_sort_within_kind() {
        let tmp = create_test_dir();
        let tree = FileTree::new(tmp.path().to_path_buf()).unwrap();
        let children = &tree.nodes()[1..];

        // Directories: beta, delta (alphabetical)
        let dir_names: Vec<&str> = children
            .iter()
            .filter(|n| matches!(n.kind, NodeKind::Directory { .. }))
            .map(|n| n.name.as_str())
            .collect();
        assert_eq!(dir_names, vec!["beta", "delta"]);

        // Files: .hidden, alpha.txt, gamma.rs (alphabetical)
        let file_names: Vec<&str> = children
            .iter()
            .filter(|n| matches!(n.kind, NodeKind::File { .. }))
            .map(|n| n.name.as_str())
            .collect();
        assert_eq!(file_names, vec![".hidden", "alpha.txt", "gamma.rs"]);
    }

    #[test]
    fn root_name_matches_directory() {
        let tmp = create_test_dir();
        let dir_name = tmp
            .path()
            .canonicalize()
            .unwrap()
            .file_name()
            .unwrap()
            .to_string_lossy()
            .into_owned();
        let tree = FileTree::new(tmp.path().to_path_buf()).unwrap();
        assert_eq!(tree.root_name(), dir_name);
    }

    #[test]
    fn selected_defaults_to_zero() {
        let tmp = create_test_dir();
        let tree = FileTree::new(tmp.path().to_path_buf()).unwrap();
        assert_eq!(tree.selected(), 0);
    }

    #[test]
    fn scroll_defaults_to_zero() {
        let tmp = create_test_dir();
        let tree = FileTree::new(tmp.path().to_path_buf()).unwrap();
        assert_eq!(tree.scroll(), 0);
    }

    #[test]
    fn empty_directory_has_only_root() {
        let tmp = TempDir::new().unwrap();
        let tree = FileTree::new(tmp.path().to_path_buf()).unwrap();
        assert_eq!(tree.nodes().len(), 1);
        assert!(matches!(
            tree.nodes()[0].kind,
            NodeKind::Directory { child_count: 0 }
        ));
    }

    #[test]
    fn child_parent_index_is_zero() {
        let tmp = create_test_dir();
        let tree = FileTree::new(tmp.path().to_path_buf()).unwrap();
        for node in &tree.nodes()[1..] {
            assert_eq!(node.parent, Some(0), "children should reference root");
        }
    }

    // --- Navigation tests ---

    #[test]
    fn move_down_increments_selected() {
        let tmp = create_test_dir();
        let mut tree = FileTree::new(tmp.path().to_path_buf()).unwrap();
        assert_eq!(tree.selected(), 0);
        tree.move_down();
        assert_eq!(tree.selected(), 1);
    }

    #[test]
    fn move_down_wraps_at_end() {
        let tmp = create_test_dir();
        let mut tree = FileTree::new(tmp.path().to_path_buf()).unwrap();
        // 6 nodes (root + 2 dirs + 3 files), go to last then wrap
        for _ in 0..5 {
            tree.move_down();
        }
        assert_eq!(tree.selected(), 5);
        tree.move_down();
        assert_eq!(tree.selected(), 0);
    }

    #[test]
    fn move_up_decrements_selected() {
        let tmp = create_test_dir();
        let mut tree = FileTree::new(tmp.path().to_path_buf()).unwrap();
        tree.move_down();
        tree.move_down();
        assert_eq!(tree.selected(), 2);
        tree.move_up();
        assert_eq!(tree.selected(), 1);
    }

    #[test]
    fn move_up_wraps_at_start() {
        let tmp = create_test_dir();
        let mut tree = FileTree::new(tmp.path().to_path_buf()).unwrap();
        assert_eq!(tree.selected(), 0);
        tree.move_up();
        assert_eq!(tree.selected(), 5); // last node
    }

    #[test]
    fn move_home_goes_to_zero() {
        let tmp = create_test_dir();
        let mut tree = FileTree::new(tmp.path().to_path_buf()).unwrap();
        tree.move_down();
        tree.move_down();
        tree.move_home();
        assert_eq!(tree.selected(), 0);
    }

    #[test]
    fn move_end_goes_to_last() {
        let tmp = create_test_dir();
        let mut tree = FileTree::new(tmp.path().to_path_buf()).unwrap();
        tree.move_end();
        assert_eq!(tree.selected(), 5);
    }

    #[test]
    fn selected_node_returns_correct() {
        let tmp = create_test_dir();
        let mut tree = FileTree::new(tmp.path().to_path_buf()).unwrap();
        // selected=0 is root
        assert!(matches!(
            tree.selected_node().unwrap().kind,
            NodeKind::Directory { .. }
        ));
        tree.move_down(); // first child: "beta" dir
        assert_eq!(tree.selected_node().unwrap().name, "beta");
    }

    // --- Direct selection tests ---

    #[test]
    fn select_valid_index_changes_selected() {
        let tmp = create_test_dir();
        let mut tree = FileTree::new(tmp.path().to_path_buf()).unwrap();
        assert_eq!(tree.selected(), 0);
        assert!(tree.select(2));
        assert_eq!(tree.selected(), 2);
    }

    #[test]
    fn select_out_of_bounds_returns_false() {
        let tmp = create_test_dir();
        let mut tree = FileTree::new(tmp.path().to_path_buf()).unwrap();
        tree.select(1);
        assert!(!tree.select(999));
        assert_eq!(tree.selected(), 1); // unchanged
    }

    #[test]
    fn select_adjusts_scroll() {
        let tmp = create_test_dir();
        let mut tree = FileTree::new(tmp.path().to_path_buf()).unwrap();
        tree.set_viewport_height(2); // only 2 visible rows
        assert!(tree.select(4)); // select node beyond viewport
        assert_eq!(tree.selected(), 4);
        // scroll should have adjusted so selected is visible
        assert!(tree.scroll() + 2 > 4);
    }

    // --- Expand/collapse tests ---

    /// Creates a nested directory structure for expand/collapse tests:
    /// root/
    ///   sub/           (dir with children)
    ///     nested/      (dir)
    ///     file.txt     (file)
    ///   other.txt      (file)
    fn create_nested_test_dir() -> TempDir {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();

        fs::create_dir_all(root.join("sub/nested")).unwrap();
        fs::write(root.join("sub/file.txt"), "content").unwrap();
        fs::write(root.join("other.txt"), "content").unwrap();

        tmp
    }

    #[test]
    fn expand_directory_inserts_children() {
        let tmp = create_nested_test_dir();
        let mut tree = FileTree::new(tmp.path().to_path_buf()).unwrap();
        // Initial: root, sub (collapsed), other.txt = 3 nodes
        assert_eq!(tree.nodes().len(), 3);

        // Select "sub" (index 1) and expand
        tree.move_down();
        tree.expand().unwrap();

        // Now: root, sub (expanded), nested, file.txt, other.txt = 5 nodes
        assert_eq!(tree.nodes().len(), 5);
    }

    #[test]
    fn expand_sets_expanded_flag() {
        let tmp = create_nested_test_dir();
        let mut tree = FileTree::new(tmp.path().to_path_buf()).unwrap();
        tree.move_down(); // select "sub"
        assert!(!tree.nodes()[1].expanded);
        tree.expand().unwrap();
        assert!(tree.nodes()[1].expanded);
    }

    #[test]
    fn expand_on_file_is_noop() {
        let tmp = create_nested_test_dir();
        let mut tree = FileTree::new(tmp.path().to_path_buf()).unwrap();
        let initial_count = tree.nodes().len();
        // Select "other.txt" (last node, index 2)
        tree.move_down();
        tree.move_down();
        tree.expand().unwrap();
        assert_eq!(tree.nodes().len(), initial_count);
    }

    #[test]
    fn expand_already_expanded_is_noop() {
        let tmp = create_nested_test_dir();
        let mut tree = FileTree::new(tmp.path().to_path_buf()).unwrap();
        tree.move_down(); // select "sub"
        tree.expand().unwrap();
        let count_after = tree.nodes().len();
        tree.expand().unwrap(); // expand again
        assert_eq!(tree.nodes().len(), count_after);
    }

    #[test]
    fn collapse_removes_descendants() {
        let tmp = create_nested_test_dir();
        let mut tree = FileTree::new(tmp.path().to_path_buf()).unwrap();
        tree.move_down(); // select "sub"
        tree.expand().unwrap();
        assert_eq!(tree.nodes().len(), 5);

        tree.collapse();
        assert_eq!(tree.nodes().len(), 3); // back to initial
    }

    #[test]
    fn collapse_resets_expanded_flag() {
        let tmp = create_nested_test_dir();
        let mut tree = FileTree::new(tmp.path().to_path_buf()).unwrap();
        tree.move_down();
        tree.expand().unwrap();
        assert!(tree.nodes()[1].expanded);
        tree.collapse();
        assert!(!tree.nodes()[1].expanded);
    }

    #[test]
    fn collapse_on_file_is_noop() {
        let tmp = create_nested_test_dir();
        let mut tree = FileTree::new(tmp.path().to_path_buf()).unwrap();
        let initial_count = tree.nodes().len();
        tree.move_down();
        tree.move_down(); // select "other.txt"
        tree.collapse();
        assert_eq!(tree.nodes().len(), initial_count);
    }

    #[test]
    fn collapse_removes_deeply_nested() {
        let tmp = create_nested_test_dir();
        let mut tree = FileTree::new(tmp.path().to_path_buf()).unwrap();
        // Expand "sub"
        tree.move_down();
        tree.expand().unwrap();
        // Now expand "nested" (index 2 after expansion)
        tree.move_down(); // select "nested" at index 2
        tree.expand().unwrap();
        // Collapse "sub" at index 1 — should remove nested and its children
        tree.move_up(); // back to "sub"
        let sub_idx = tree.selected();
        assert_eq!(tree.nodes()[sub_idx].name, "sub");
        tree.collapse();
        // Should be back to: root, sub (collapsed), other.txt
        assert_eq!(tree.nodes().len(), 3);
    }

    #[test]
    fn toggle_expands_collapsed() {
        let tmp = create_nested_test_dir();
        let mut tree = FileTree::new(tmp.path().to_path_buf()).unwrap();
        tree.move_down(); // select "sub"
        tree.toggle().unwrap();
        assert!(tree.nodes()[1].expanded);
    }

    #[test]
    fn toggle_collapses_expanded() {
        let tmp = create_nested_test_dir();
        let mut tree = FileTree::new(tmp.path().to_path_buf()).unwrap();
        tree.move_down();
        tree.expand().unwrap();
        tree.toggle().unwrap();
        assert!(!tree.nodes()[1].expanded);
    }

    #[test]
    fn collapse_or_parent_collapses_expanded_dir() {
        let tmp = create_nested_test_dir();
        let mut tree = FileTree::new(tmp.path().to_path_buf()).unwrap();
        tree.move_down(); // select "sub"
        tree.expand().unwrap();
        tree.collapse_or_parent();
        assert!(!tree.nodes()[1].expanded);
        assert_eq!(tree.nodes().len(), 3);
    }

    #[test]
    fn collapse_or_parent_moves_to_parent_on_file() {
        let tmp = create_nested_test_dir();
        let mut tree = FileTree::new(tmp.path().to_path_buf()).unwrap();
        // Select "other.txt" (last node, depth 1, parent is root at index 0)
        tree.move_end();
        assert_eq!(tree.selected_node().unwrap().name, "other.txt");
        tree.collapse_or_parent();
        assert_eq!(tree.selected(), 0); // moved to root
    }

    #[test]
    fn collapse_or_parent_noop_at_root() {
        let tmp = create_nested_test_dir();
        let mut tree = FileTree::new(tmp.path().to_path_buf()).unwrap();
        assert_eq!(tree.selected(), 0); // root, depth 0
        tree.collapse_or_parent();
        // Root is expanded — collapse_or_parent should collapse it
        // After collapse root has no visible children
        // Actually, root IS expanded but at depth 0 — let's check behavior
        // The root is a directory and expanded, so it should collapse
        assert!(!tree.nodes()[0].expanded);
    }

    // --- Scroll tests ---

    #[test]
    fn scroll_adjusts_when_below_viewport() {
        let tmp = create_test_dir();
        let mut tree = FileTree::new(tmp.path().to_path_buf()).unwrap();
        tree.set_viewport_height(3); // can see 3 items at a time
                                     // Move to item 3 (0-indexed), which is beyond viewport [0,1,2]
        tree.move_down();
        tree.move_down();
        tree.move_down();
        assert_eq!(tree.selected(), 3);
        assert!(
            tree.scroll() > 0,
            "scroll should adjust to keep selected visible"
        );
        assert!(tree.selected() < tree.scroll() + 3);
    }

    #[test]
    fn move_home_resets_scroll() {
        let tmp = create_test_dir();
        let mut tree = FileTree::new(tmp.path().to_path_buf()).unwrap();
        tree.set_viewport_height(3);
        tree.move_end();
        assert!(tree.scroll() > 0);
        tree.move_home();
        assert_eq!(tree.scroll(), 0);
        assert_eq!(tree.selected(), 0);
    }

    // --- Gitignore filter integration tests ---

    /// Creates a temp directory with .git and .gitignore for filter tests:
    /// root/
    ///   .git/
    ///   .gitignore  (contains "*.log")
    ///   src/
    ///     main.rs
    ///   debug.log
    ///   README.md
    fn create_gitignore_test_dir() -> TempDir {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();

        fs::create_dir(root.join(".git")).unwrap();
        fs::write(root.join(".gitignore"), "*.log\n").unwrap();
        fs::create_dir(root.join("src")).unwrap();
        fs::write(root.join("src/main.rs"), "fn main() {}").unwrap();
        fs::write(root.join("debug.log"), "log content").unwrap();
        fs::write(root.join("README.md"), "# Readme").unwrap();

        tmp
    }

    #[test]
    fn new_shows_gitignored_files_by_default() {
        let tmp = create_gitignore_test_dir();
        let tree = FileTree::new(tmp.path().to_path_buf()).unwrap();
        let names: Vec<&str> = tree.nodes()[1..].iter().map(|n| n.name.as_str()).collect();
        assert!(
            names.contains(&"debug.log"),
            "gitignored file should be visible by default"
        );
        assert!(
            names.contains(&"README.md"),
            "non-ignored file should be visible"
        );
    }

    #[test]
    fn new_shows_dot_files() {
        let tmp = create_gitignore_test_dir();
        let tree = FileTree::new(tmp.path().to_path_buf()).unwrap();
        let names: Vec<&str> = tree.nodes()[1..].iter().map(|n| n.name.as_str()).collect();
        assert!(
            names.contains(&".git"),
            ".git should be visible, found: {names:?}"
        );
        assert!(
            names.contains(&".gitignore"),
            ".gitignore should be visible, found: {names:?}"
        );
    }

    #[test]
    fn toggle_hides_gitignored_files() {
        let tmp = create_gitignore_test_dir();
        let mut tree = FileTree::new(tmp.path().to_path_buf()).unwrap();

        // Initially visible
        let names_before: Vec<&str> = tree.nodes()[1..].iter().map(|n| n.name.as_str()).collect();
        assert!(names_before.contains(&"debug.log"));

        // Toggle to hide ignored
        tree.toggle_show_ignored();
        assert!(!tree.show_ignored());

        let names_after: Vec<&str> = tree.nodes()[1..].iter().map(|n| n.name.as_str()).collect();
        assert!(
            !names_after.contains(&"debug.log"),
            "ignored file should be hidden after toggle"
        );
    }

    #[test]
    fn toggle_hides_node_modules() {
        let tmp = TempDir::new().unwrap();
        fs::create_dir(tmp.path().join(".git")).unwrap();
        fs::create_dir(tmp.path().join("node_modules")).unwrap();
        fs::write(tmp.path().join("index.js"), "").unwrap();

        let mut tree = FileTree::new(tmp.path().to_path_buf()).unwrap();
        tree.toggle_show_ignored();

        let names: Vec<&str> = tree.nodes()[1..].iter().map(|n| n.name.as_str()).collect();
        assert!(
            !names.contains(&"node_modules"),
            "node_modules should be hidden after toggle"
        );
    }

    #[test]
    fn toggle_hides_target() {
        let tmp = TempDir::new().unwrap();
        fs::create_dir(tmp.path().join(".git")).unwrap();
        fs::create_dir(tmp.path().join("target")).unwrap();
        fs::write(tmp.path().join("Cargo.toml"), "").unwrap();

        let mut tree = FileTree::new(tmp.path().to_path_buf()).unwrap();
        tree.toggle_show_ignored();

        let names: Vec<&str> = tree.nodes()[1..].iter().map(|n| n.name.as_str()).collect();
        assert!(
            !names.contains(&"target"),
            "target should be hidden after toggle"
        );
    }

    #[test]
    fn expand_respects_gitignore_when_toggled() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        fs::create_dir(root.join(".git")).unwrap();
        fs::write(root.join(".gitignore"), "*.bak\n").unwrap();
        let sub = root.join("sub");
        fs::create_dir(&sub).unwrap();
        fs::write(sub.join("keep.txt"), "").unwrap();
        fs::write(sub.join("remove.bak"), "").unwrap();

        let mut tree = FileTree::new(root.to_path_buf()).unwrap();
        // Toggle to hide ignored
        tree.toggle_show_ignored();
        // Navigate to "sub" directory
        let sub_idx = tree
            .nodes()
            .iter()
            .position(|n| n.name == "sub")
            .expect("sub should exist");
        for _ in 0..sub_idx {
            tree.move_down();
        }
        tree.expand().unwrap();

        let names: Vec<&str> = tree.nodes().iter().map(|n| n.name.as_str()).collect();
        assert!(
            names.contains(&"keep.txt"),
            "non-ignored child should be visible"
        );
        assert!(
            !names.contains(&"remove.bak"),
            "ignored child should be hidden"
        );
    }

    #[test]
    fn nested_gitignore_in_expand_when_toggled() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        fs::create_dir(root.join(".git")).unwrap();
        let sub = root.join("sub");
        fs::create_dir(&sub).unwrap();
        fs::write(sub.join(".gitignore"), "secret.txt\n").unwrap();
        fs::write(sub.join("public.txt"), "").unwrap();
        fs::write(sub.join("secret.txt"), "").unwrap();

        let mut tree = FileTree::new(root.to_path_buf()).unwrap();
        // Toggle to hide ignored
        tree.toggle_show_ignored();
        // Navigate to "sub" directory
        let sub_idx = tree
            .nodes()
            .iter()
            .position(|n| n.name == "sub")
            .expect("sub should exist");
        for _ in 0..sub_idx {
            tree.move_down();
        }
        tree.expand().unwrap();

        let names: Vec<&str> = tree.nodes().iter().map(|n| n.name.as_str()).collect();
        assert!(
            names.contains(&"public.txt"),
            "non-ignored child should be visible"
        );
        assert!(
            !names.contains(&"secret.txt"),
            "file ignored by nested .gitignore should be hidden"
        );
    }

    // --- Tree action tests ---

    #[test]
    fn start_create_file_sets_action() {
        let tmp = create_test_dir();
        let mut tree = FileTree::new(tmp.path().to_path_buf()).unwrap();
        tree.start_create_file();
        assert_eq!(
            tree.action(),
            &TreeAction::Creating {
                is_dir: false,
                input: String::new()
            }
        );
        assert!(tree.is_action_active());
    }

    #[test]
    fn start_create_dir_sets_action() {
        let tmp = create_test_dir();
        let mut tree = FileTree::new(tmp.path().to_path_buf()).unwrap();
        tree.start_create_dir();
        assert_eq!(
            tree.action(),
            &TreeAction::Creating {
                is_dir: true,
                input: String::new()
            }
        );
    }

    #[test]
    fn start_rename_sets_action_with_name() {
        let tmp = create_test_dir();
        let mut tree = FileTree::new(tmp.path().to_path_buf()).unwrap();
        tree.move_down(); // select "beta"
        let name = tree.selected_node().unwrap().name.clone();
        tree.start_rename();
        assert_eq!(
            tree.action(),
            &TreeAction::Renaming {
                node_idx: 1,
                input: name
            }
        );
    }

    #[test]
    fn start_rename_noop_on_root() {
        let tmp = create_test_dir();
        let mut tree = FileTree::new(tmp.path().to_path_buf()).unwrap();
        assert_eq!(tree.selected(), 0);
        tree.start_rename();
        assert_eq!(tree.action(), &TreeAction::Idle);
    }

    #[test]
    fn start_delete_sets_action() {
        let tmp = create_test_dir();
        let mut tree = FileTree::new(tmp.path().to_path_buf()).unwrap();
        tree.move_down(); // select "beta"
        tree.start_delete();
        assert_eq!(tree.action(), &TreeAction::ConfirmDelete { node_idx: 1 });
    }

    #[test]
    fn start_delete_noop_on_root() {
        let tmp = create_test_dir();
        let mut tree = FileTree::new(tmp.path().to_path_buf()).unwrap();
        assert_eq!(tree.selected(), 0);
        tree.start_delete();
        assert_eq!(tree.action(), &TreeAction::Idle);
    }

    #[test]
    fn cancel_action_resets_to_idle() {
        let tmp = create_test_dir();
        let mut tree = FileTree::new(tmp.path().to_path_buf()).unwrap();
        tree.start_create_file();
        assert!(tree.is_action_active());
        tree.cancel_action();
        assert_eq!(tree.action(), &TreeAction::Idle);
        assert!(!tree.is_action_active());
    }

    #[test]
    fn input_char_appends_to_creating() {
        let tmp = create_test_dir();
        let mut tree = FileTree::new(tmp.path().to_path_buf()).unwrap();
        tree.start_create_file();
        tree.input_char('h');
        tree.input_char('i');
        assert_eq!(
            tree.action(),
            &TreeAction::Creating {
                is_dir: false,
                input: "hi".to_string()
            }
        );
    }

    #[test]
    fn input_char_appends_to_renaming() {
        let tmp = create_test_dir();
        let mut tree = FileTree::new(tmp.path().to_path_buf()).unwrap();
        tree.move_down();
        tree.start_rename();
        tree.input_char('2');
        if let TreeAction::Renaming { input, .. } = tree.action() {
            assert!(input.ends_with('2'));
        } else {
            panic!("expected Renaming action");
        }
    }

    #[test]
    fn input_backspace_removes_last_char() {
        let tmp = create_test_dir();
        let mut tree = FileTree::new(tmp.path().to_path_buf()).unwrap();
        tree.start_create_file();
        tree.input_char('a');
        tree.input_char('b');
        tree.input_backspace();
        assert_eq!(
            tree.action(),
            &TreeAction::Creating {
                is_dir: false,
                input: "a".to_string()
            }
        );
    }

    #[test]
    fn confirm_create_file_creates_on_filesystem() {
        let tmp = create_test_dir();
        let mut tree = FileTree::new(tmp.path().to_path_buf()).unwrap();
        // Select root (directory) — will create inside root
        tree.start_create_file();
        tree.input_char('n');
        tree.input_char('e');
        tree.input_char('w');
        tree.confirm_action().unwrap();
        assert!(tmp.path().join("new").exists());
        assert_eq!(tree.action(), &TreeAction::Idle);
        // Tree should contain the new file
        let names: Vec<&str> = tree.nodes().iter().map(|n| n.name.as_str()).collect();
        assert!(names.contains(&"new"));
    }

    #[test]
    fn confirm_create_dir_creates_on_filesystem() {
        let tmp = create_test_dir();
        let mut tree = FileTree::new(tmp.path().to_path_buf()).unwrap();
        tree.start_create_dir();
        tree.input_char('d');
        tree.input_char('i');
        tree.input_char('r');
        tree.confirm_action().unwrap();
        assert!(tmp.path().join("dir").is_dir());
        assert_eq!(tree.action(), &TreeAction::Idle);
    }

    #[test]
    fn confirm_rename_renames_on_filesystem() {
        let tmp = create_test_dir();
        let mut tree = FileTree::new(tmp.path().to_path_buf()).unwrap();
        // Select "alpha.txt" — it's a file at depth 1
        // Nodes: root(0), beta(1), delta(2), .hidden(3), alpha.txt(4), gamma.rs(5)
        let alpha_idx = tree
            .nodes()
            .iter()
            .position(|n| n.name == "alpha.txt")
            .unwrap();
        for _ in 0..alpha_idx {
            tree.move_down();
        }
        tree.start_rename();
        // Clear existing name and type new one
        // input is pre-filled with "alpha.txt", we need to clear and retype
        for _ in 0.."alpha.txt".len() {
            tree.input_backspace();
        }
        for c in "renamed.txt".chars() {
            tree.input_char(c);
        }
        tree.confirm_action().unwrap();
        assert!(!tmp.path().join("alpha.txt").exists());
        assert!(tmp.path().join("renamed.txt").exists());
    }

    #[test]
    fn confirm_delete_removes_file() {
        let tmp = create_test_dir();
        let mut tree = FileTree::new(tmp.path().to_path_buf()).unwrap();
        let alpha_idx = tree
            .nodes()
            .iter()
            .position(|n| n.name == "alpha.txt")
            .unwrap();
        for _ in 0..alpha_idx {
            tree.move_down();
        }
        tree.start_delete();
        tree.confirm_delete().unwrap();
        assert!(!tmp.path().join("alpha.txt").exists());
        assert_eq!(tree.action(), &TreeAction::Idle);
    }

    #[test]
    fn confirm_delete_removes_directory() {
        let tmp = create_test_dir();
        let mut tree = FileTree::new(tmp.path().to_path_buf()).unwrap();
        tree.move_down(); // select "beta" directory
        assert_eq!(tree.selected_node().unwrap().name, "beta");
        tree.start_delete();
        tree.confirm_delete().unwrap();
        assert!(!tmp.path().join("beta").exists());
    }

    #[test]
    fn confirm_create_with_empty_name_fails() {
        let tmp = create_test_dir();
        let mut tree = FileTree::new(tmp.path().to_path_buf()).unwrap();
        tree.start_create_file();
        // Don't type anything — empty input
        let result = tree.confirm_action();
        assert!(result.is_err());
    }

    #[test]
    fn confirm_create_with_invalid_name_fails() {
        let tmp = create_test_dir();
        let mut tree = FileTree::new(tmp.path().to_path_buf()).unwrap();
        tree.start_create_file();
        tree.input_char('.');
        // Name is "." which is invalid
        let result = tree.confirm_action();
        assert!(result.is_err());

        // Also test ".."
        tree.start_create_file();
        tree.input_char('.');
        tree.input_char('.');
        let result = tree.confirm_action();
        assert!(result.is_err());

        // Also test name with "/"
        tree.start_create_file();
        tree.input_char('a');
        tree.input_char('/');
        tree.input_char('b');
        let result = tree.confirm_action();
        assert!(result.is_err());
    }

    #[test]
    fn show_icons_defaults_to_true() {
        let tmp = tempfile::TempDir::new().unwrap();
        let tree = FileTree::new(tmp.path().to_path_buf()).unwrap();
        assert!(tree.show_icons());
    }

    #[test]
    fn toggle_show_icons_flips() {
        let tmp = tempfile::TempDir::new().unwrap();
        let mut tree = FileTree::new(tmp.path().to_path_buf()).unwrap();
        assert!(tree.show_icons());
        tree.toggle_show_icons();
        assert!(!tree.show_icons());
        tree.toggle_show_icons();
        assert!(tree.show_icons());
    }

    #[test]
    fn set_show_icons_sets_value() {
        let tmp = tempfile::TempDir::new().unwrap();
        let mut tree = FileTree::new(tmp.path().to_path_buf()).unwrap();
        tree.set_show_icons(false);
        assert!(!tree.show_icons());
        tree.set_show_icons(true);
        assert!(tree.show_icons());
    }

    #[test]
    fn set_show_ignored_sets_value() {
        let tmp = tempfile::TempDir::new().unwrap();
        let mut tree = FileTree::new(tmp.path().to_path_buf()).unwrap();
        // Default is true (show all).
        assert!(tree.show_ignored());
        tree.set_show_ignored(false);
        assert!(!tree.show_ignored());
        tree.set_show_ignored(true);
        assert!(tree.show_ignored());
    }

    #[test]
    fn scroll_col_defaults_to_zero() {
        let tmp = create_test_dir();
        let tree = FileTree::new(tmp.path().to_path_buf()).unwrap();
        assert_eq!(tree.scroll_col(), 0);
    }

    #[test]
    fn scroll_by_positive_scrolls_down() {
        let tmp = create_test_dir();
        let mut tree = FileTree::new(tmp.path().to_path_buf()).unwrap();
        tree.set_viewport_height(3);
        tree.scroll_by(2);
        assert_eq!(tree.scroll(), 2);
    }

    #[test]
    fn scroll_by_negative_scrolls_up() {
        let tmp = create_test_dir();
        let mut tree = FileTree::new(tmp.path().to_path_buf()).unwrap();
        tree.set_viewport_height(3);
        tree.scroll_by(4);
        let scrolled = tree.scroll();
        tree.scroll_by(-2);
        assert_eq!(tree.scroll(), scrolled - 2);
    }

    #[test]
    fn scroll_by_clamps_at_bounds() {
        let tmp = create_test_dir();
        let mut tree = FileTree::new(tmp.path().to_path_buf()).unwrap();
        let node_count = tree.nodes().len();
        tree.set_viewport_height(3);
        // Scroll way past the end.
        tree.scroll_by(1000);
        assert_eq!(tree.scroll(), node_count.saturating_sub(3));
        // Scroll way past the start.
        tree.scroll_by(-1000);
        assert_eq!(tree.scroll(), 0);
    }

    #[test]
    fn set_viewport_height_does_not_undo_scroll_by() {
        let tmp = create_test_dir();
        let mut tree = FileTree::new(tmp.path().to_path_buf()).unwrap();
        tree.set_viewport_height(3);
        tree.scroll_by(2);
        assert_eq!(tree.scroll(), 2);
        // Simulate next frame: viewport height set again.
        tree.set_viewport_height(3);
        // scroll_by must NOT have been undone by set_viewport_height.
        assert_eq!(tree.scroll(), 2);
    }

    #[test]
    fn scroll_horizontally_by_positive() {
        let tmp = create_test_dir();
        let mut tree = FileTree::new(tmp.path().to_path_buf()).unwrap();
        // Narrow viewport so there is room to scroll.
        tree.set_viewport_width(5);
        tree.scroll_horizontally_by(3, 2, 2);
        assert_eq!(tree.scroll_col(), 3);
    }

    #[test]
    fn scroll_horizontally_by_negative() {
        let tmp = create_test_dir();
        let mut tree = FileTree::new(tmp.path().to_path_buf()).unwrap();
        tree.set_viewport_width(3);
        let max_w = tree.max_content_width(2, 2);
        // Scroll to max, then back by 2.
        tree.scroll_horizontally_by(max_w as i32, 2, 2);
        let at_max = tree.scroll_col();
        tree.scroll_horizontally_by(-2, 2, 2);
        assert_eq!(tree.scroll_col(), at_max - 2);
    }

    #[test]
    fn scroll_horizontally_clamps_at_zero() {
        let tmp = create_test_dir();
        let mut tree = FileTree::new(tmp.path().to_path_buf()).unwrap();
        tree.set_viewport_width(5);
        tree.scroll_horizontally_by(2, 2, 2);
        tree.scroll_horizontally_by(-100, 2, 2);
        assert_eq!(tree.scroll_col(), 0);
    }

    #[test]
    fn max_content_width_accounts_for_depth_and_name() {
        let tmp = create_test_dir();
        let tree = FileTree::new(tmp.path().to_path_buf()).unwrap();
        let max_w = tree.max_content_width(2, 2);
        // Depth-1 nodes: 2 + 2 + name.len() — longest depth-1 name is "alpha.txt" (9)
        // So at least 2 + 2 + 9 = 13
        assert!(
            max_w >= 13,
            "max_content_width should be >= 13, got {max_w}"
        );
    }

    #[test]
    fn scroll_horizontally_clamps_at_panel_width() {
        let tmp = create_test_dir();
        let mut tree = FileTree::new(tmp.path().to_path_buf()).unwrap();
        tree.set_viewport_height(10);
        // Panel width 10 — max_scroll = max_content_width - 10.
        tree.set_viewport_width(10);
        tree.scroll_horizontally_by(1000, 2, 2);
        let max_w = tree.max_content_width(2, 2);
        let expected_max = max_w.saturating_sub(10);
        assert_eq!(
            tree.scroll_col(),
            expected_max,
            "scroll_col should be clamped to max_content_width - viewport_width"
        );
    }

    #[test]
    fn scroll_horizontally_no_scroll_when_content_fits() {
        let tmp = create_test_dir();
        let mut tree = FileTree::new(tmp.path().to_path_buf()).unwrap();
        // Panel wider than content — no scroll possible.
        tree.set_viewport_width(200);
        tree.scroll_horizontally_by(100, 2, 2);
        assert_eq!(tree.scroll_col(), 0, "should not scroll when content fits");
    }

    // --- expand_at tests ---

    #[test]
    fn expand_at_non_selected_directory() {
        let tmp = create_nested_test_dir();
        let mut tree = FileTree::new(tmp.path().to_path_buf()).unwrap();
        // Initial: root(0), sub(1), other.txt(2)
        assert_eq!(tree.selected(), 0);
        // Expand "sub" at index 1 without changing selection.
        let inserted = tree.expand_at(1).unwrap();
        assert_eq!(inserted, 2); // nested/ and file.txt
        assert_eq!(tree.selected(), 0); // selection unchanged
        assert!(tree.nodes()[1].expanded);
        assert_eq!(tree.nodes().len(), 5);
    }

    // --- refresh_tree tests ---

    #[test]
    fn refresh_tree_preserves_expanded_dirs() {
        let tmp = create_nested_test_dir();
        let mut tree = FileTree::new(tmp.path().to_path_buf()).unwrap();
        // Expand "sub" (index 1).
        tree.move_down();
        tree.expand().unwrap();
        // Nodes: root(0), sub(1 expanded), nested(2), file.txt(3), other.txt(4)
        assert!(tree.nodes()[1].expanded);
        assert_eq!(tree.nodes().len(), 5);

        tree.refresh_tree();

        // After refresh, "sub" should still be expanded with its children.
        assert_eq!(tree.nodes().len(), 5);
        let sub = tree.nodes().iter().find(|n| n.name == "sub").unwrap();
        assert!(sub.expanded);
    }

    #[test]
    fn refresh_tree_preserves_selection() {
        let tmp = create_nested_test_dir();
        let mut tree = FileTree::new(tmp.path().to_path_buf()).unwrap();
        // Select "other.txt" (index 2).
        tree.move_down();
        tree.move_down();
        let selected_path = tree.selected_node().unwrap().path.clone();
        assert_eq!(tree.selected_node().unwrap().name, "other.txt");

        tree.refresh_tree();

        assert_eq!(tree.selected_node().unwrap().path, selected_path);
    }

    #[test]
    fn refresh_tree_adjusts_selection_when_file_deleted() {
        let tmp = create_nested_test_dir();
        let mut tree = FileTree::new(tmp.path().to_path_buf()).unwrap();
        // Select "other.txt" (last node, index 2).
        tree.move_end();
        assert_eq!(tree.selected_node().unwrap().name, "other.txt");

        // Delete other.txt externally.
        fs::remove_file(tmp.path().join("other.txt")).unwrap();

        tree.refresh_tree();

        // Selection should clamp to valid range (path gone, falls to nearest/parent).
        assert!(tree.selected() < tree.nodes().len());
    }

    #[test]
    fn refresh_tree_shows_new_files() {
        let tmp = create_nested_test_dir();
        let mut tree = FileTree::new(tmp.path().to_path_buf()).unwrap();
        let initial_len = tree.nodes().len();

        // Create a new file externally.
        fs::write(tmp.path().join("new_file.txt"), "hello").unwrap();

        tree.refresh_tree();

        let names: Vec<&str> = tree.nodes().iter().map(|n| n.name.as_str()).collect();
        assert!(
            names.contains(&"new_file.txt"),
            "new file should appear: {names:?}"
        );
        assert_eq!(tree.nodes().len(), initial_len + 1);
    }

    #[test]
    fn refresh_tree_removes_deleted_files() {
        let tmp = create_nested_test_dir();
        let mut tree = FileTree::new(tmp.path().to_path_buf()).unwrap();

        // Delete "other.txt" externally.
        fs::remove_file(tmp.path().join("other.txt")).unwrap();

        tree.refresh_tree();

        let names: Vec<&str> = tree.nodes().iter().map(|n| n.name.as_str()).collect();
        assert!(
            !names.contains(&"other.txt"),
            "deleted file should be gone: {names:?}"
        );
    }

    #[test]
    fn refresh_tree_preserves_deeply_nested_expansion() {
        let tmp = create_nested_test_dir();
        let mut tree = FileTree::new(tmp.path().to_path_buf()).unwrap();
        // Expand "sub" then "nested".
        tree.move_down(); // select sub
        tree.expand().unwrap();
        // Nodes: root(0), sub(1), nested(2), file.txt(3), other.txt(4)
        tree.move_down(); // select nested
        tree.expand().unwrap();
        // nested is now expanded (empty dir, but expanded flag set).
        assert!(
            tree.nodes()
                .iter()
                .find(|n| n.name == "sub")
                .unwrap()
                .expanded
        );
        assert!(
            tree.nodes()
                .iter()
                .find(|n| n.name == "nested")
                .unwrap()
                .expanded
        );

        tree.refresh_tree();

        assert!(
            tree.nodes()
                .iter()
                .find(|n| n.name == "sub")
                .unwrap()
                .expanded
        );
        assert!(
            tree.nodes()
                .iter()
                .find(|n| n.name == "nested")
                .unwrap()
                .expanded
        );
    }

    // --- State-preserving operation tests ---

    #[test]
    fn confirm_create_preserves_expanded_state() {
        let tmp = create_nested_test_dir();
        let mut tree = FileTree::new(tmp.path().to_path_buf()).unwrap();
        // Expand "sub".
        tree.move_down();
        tree.expand().unwrap();
        assert!(
            tree.nodes()
                .iter()
                .find(|n| n.name == "sub")
                .unwrap()
                .expanded
        );

        // Create a new file at root.
        tree.select(0);
        tree.start_create_file();
        for c in "newfile.txt".chars() {
            tree.input_char(c);
        }
        tree.confirm_action().unwrap();

        // "sub" should still be expanded after create.
        assert!(
            tree.nodes()
                .iter()
                .find(|n| n.name == "sub")
                .unwrap()
                .expanded,
            "expanded state should be preserved after create"
        );
        let names: Vec<&str> = tree.nodes().iter().map(|n| n.name.as_str()).collect();
        assert!(names.contains(&"newfile.txt"));
    }

    #[test]
    fn confirm_rename_preserves_expanded_state() {
        let tmp = create_nested_test_dir();
        let mut tree = FileTree::new(tmp.path().to_path_buf()).unwrap();
        // Expand "sub".
        tree.move_down();
        tree.expand().unwrap();

        // Rename "other.txt" (find its index after expansion).
        let other_idx = tree
            .nodes()
            .iter()
            .position(|n| n.name == "other.txt")
            .unwrap();
        tree.select(other_idx);
        tree.start_rename();
        for _ in 0.."other.txt".len() {
            tree.input_backspace();
        }
        for c in "renamed.txt".chars() {
            tree.input_char(c);
        }
        tree.confirm_action().unwrap();

        assert!(
            tree.nodes()
                .iter()
                .find(|n| n.name == "sub")
                .unwrap()
                .expanded,
            "expanded state should be preserved after rename"
        );
    }

    #[test]
    fn confirm_delete_preserves_expanded_state() {
        let tmp = create_nested_test_dir();
        let mut tree = FileTree::new(tmp.path().to_path_buf()).unwrap();
        // Expand "sub".
        tree.move_down();
        tree.expand().unwrap();

        // Delete "other.txt".
        let other_idx = tree
            .nodes()
            .iter()
            .position(|n| n.name == "other.txt")
            .unwrap();
        tree.select(other_idx);
        tree.start_delete();
        tree.confirm_delete().unwrap();

        assert!(
            tree.nodes()
                .iter()
                .find(|n| n.name == "sub")
                .unwrap()
                .expanded,
            "expanded state should be preserved after delete"
        );
    }

    #[test]
    fn toggle_show_ignored_preserves_expanded_state() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        fs::create_dir(root.join(".git")).unwrap();
        fs::write(root.join(".gitignore"), "*.log\n").unwrap();
        fs::create_dir(root.join("src")).unwrap();
        fs::write(root.join("src/main.rs"), "").unwrap();
        fs::write(root.join("debug.log"), "").unwrap();

        let mut tree = FileTree::new(root.to_path_buf()).unwrap();
        // Expand "src".
        let src_idx = tree.nodes().iter().position(|n| n.name == "src").unwrap();
        tree.select(src_idx);
        tree.expand().unwrap();
        assert!(
            tree.nodes()
                .iter()
                .find(|n| n.name == "src")
                .unwrap()
                .expanded
        );

        tree.toggle_show_ignored();

        assert!(
            tree.nodes()
                .iter()
                .find(|n| n.name == "src")
                .unwrap()
                .expanded,
            "expanded state should be preserved after toggle_show_ignored"
        );
    }

    // --- restore_expanded tests ---

    #[test]
    fn restore_expanded_expands_matching_dirs() {
        let tmp = create_nested_test_dir();
        let mut tree = FileTree::new(tmp.path().to_path_buf()).unwrap();
        // Get the canonical path for "sub".
        let sub_path = tree
            .nodes()
            .iter()
            .find(|n| n.name == "sub")
            .unwrap()
            .path
            .clone();

        // Initially "sub" is collapsed.
        assert!(
            !tree
                .nodes()
                .iter()
                .find(|n| n.name == "sub")
                .unwrap()
                .expanded
        );

        let paths: HashSet<PathBuf> = [sub_path].into_iter().collect();
        tree.restore_expanded(&paths);

        assert!(
            tree.nodes()
                .iter()
                .find(|n| n.name == "sub")
                .unwrap()
                .expanded
        );
        // Children should be visible.
        let names: Vec<&str> = tree.nodes().iter().map(|n| n.name.as_str()).collect();
        assert!(names.contains(&"nested"));
        assert!(names.contains(&"file.txt"));
    }

    #[test]
    fn restore_expanded_ignores_nonexistent_paths() {
        let tmp = create_nested_test_dir();
        let mut tree = FileTree::new(tmp.path().to_path_buf()).unwrap();
        let initial_len = tree.nodes().len();

        let paths: HashSet<PathBuf> = [PathBuf::from("/nonexistent/path")].into_iter().collect();
        tree.restore_expanded(&paths);

        assert_eq!(tree.nodes().len(), initial_len);
    }

    // --- set_selected_by_path tests ---

    #[test]
    fn set_selected_by_path_selects_matching_node() {
        let tmp = create_nested_test_dir();
        let mut tree = FileTree::new(tmp.path().to_path_buf()).unwrap();
        let other_path = tree
            .nodes()
            .iter()
            .find(|n| n.name == "other.txt")
            .unwrap()
            .path
            .clone();

        assert_eq!(tree.selected(), 0); // initially root
        tree.set_selected_by_path(&other_path);
        assert_eq!(tree.selected_node().unwrap().name, "other.txt");
    }

    #[test]
    fn set_selected_by_path_falls_back_to_parent() {
        let tmp = create_nested_test_dir();
        let mut tree = FileTree::new(tmp.path().to_path_buf()).unwrap();
        // Expand sub so its children are visible.
        tree.move_down();
        tree.expand().unwrap();
        let nested_path = tree
            .nodes()
            .iter()
            .find(|n| n.name == "nested")
            .unwrap()
            .path
            .clone();
        // Create a fake path inside nested that doesn't exist as a node.
        let fake_child = nested_path.join("nonexistent.rs");
        tree.set_selected_by_path(&fake_child);
        // Should fall back to "nested" directory.
        assert_eq!(tree.selected_node().unwrap().name, "nested");
    }

    #[test]
    fn set_selected_by_path_noop_for_unknown_path() {
        let tmp = create_nested_test_dir();
        let mut tree = FileTree::new(tmp.path().to_path_buf()).unwrap();
        tree.select(1);
        tree.set_selected_by_path(Path::new("/completely/unknown/path"));
        // Selection unchanged.
        assert_eq!(tree.selected(), 1);
    }

    // --- set_scroll tests ---

    #[test]
    fn set_scroll_sets_value() {
        let tmp = create_test_dir();
        let mut tree = FileTree::new(tmp.path().to_path_buf()).unwrap();
        tree.set_viewport_height(3);
        tree.set_scroll(2);
        assert_eq!(tree.scroll(), 2);
    }

    #[test]
    fn set_scroll_clamps_to_max() {
        let tmp = create_test_dir();
        let mut tree = FileTree::new(tmp.path().to_path_buf()).unwrap();
        tree.set_viewport_height(3);
        tree.set_scroll(1000);
        let max_scroll = tree.nodes().len().saturating_sub(3);
        assert_eq!(tree.scroll(), max_scroll);
    }
}
