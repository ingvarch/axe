use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

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

        let root_name = root
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| root.to_string_lossy().into_owned());

        let mut nodes = Vec::new();

        // Root node — always expanded with children loaded.
        let child_count = Self::count_visible_children(&root);
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
        let children = Self::read_children(&root, 1, 0)?;
        nodes.extend(children);

        Ok(Self {
            root,
            nodes,
            selected: 0,
            scroll: 0,
            viewport_height: usize::MAX,
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
        self.adjust_scroll();
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
        if self.selected >= self.nodes.len() {
            return Ok(());
        }
        let node = &self.nodes[self.selected];
        if !matches!(node.kind, NodeKind::Directory { .. }) || node.expanded {
            return Ok(());
        }

        let dir_path = node.path.clone();
        let depth = node.depth + 1;
        let parent_index = self.selected;

        let children = Self::read_children(&dir_path, depth, parent_index)?;

        self.nodes[self.selected].expanded = true;
        self.nodes[self.selected].children_loaded = true;

        // Insert children right after the selected node.
        let insert_pos = self.selected + 1;
        for (i, child) in children.into_iter().enumerate() {
            self.nodes.insert(insert_pos + i, child);
        }

        Ok(())
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

    /// Keeps the selected index within the visible scroll window.
    fn adjust_scroll(&mut self) {
        if self.selected < self.scroll {
            self.scroll = self.selected;
        } else if self.selected >= self.scroll + self.viewport_height {
            self.scroll = self.selected + 1 - self.viewport_height;
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

    /// Reads the children of a directory, filtering hidden files and sorting
    /// directories before files, alphabetically within each group.
    fn read_children(dir: &Path, depth: usize, parent_index: usize) -> Result<Vec<TreeNode>> {
        let entries = std::fs::read_dir(dir)
            .with_context(|| format!("Failed to read directory: {}", dir.display()))?;

        let mut dirs = Vec::new();
        let mut files = Vec::new();

        for entry in entries {
            let entry = entry?;
            let name = entry.file_name().to_string_lossy().into_owned();

            // Skip hidden files.
            if name.starts_with('.') {
                continue;
            }

            let path = entry.path();
            let metadata = entry.metadata()?;

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
            } else if metadata.is_dir() {
                let child_count = Self::count_visible_children(&path);
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

            if metadata.is_dir() {
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

    /// Counts non-hidden entries in a directory (for `child_count`).
    fn count_visible_children(dir: &Path) -> usize {
        std::fs::read_dir(dir)
            .map(|entries| {
                entries
                    .filter_map(|e| e.ok())
                    .filter(|e| !e.file_name().to_string_lossy().starts_with('.'))
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
        // root + 2 dirs + 2 files = 5 (hidden excluded)
        assert_eq!(tree.nodes().len(), 5);
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
    fn hidden_files_excluded() {
        let tmp = create_test_dir();
        let tree = FileTree::new(tmp.path().to_path_buf()).unwrap();
        // Skip root node (index 0) — root is the project dir, always shown.
        let hidden: Vec<&str> = tree.nodes()[1..]
            .iter()
            .filter(|n| n.name.starts_with('.'))
            .map(|n| n.name.as_str())
            .collect();
        assert!(
            hidden.is_empty(),
            "hidden files should be excluded, found: {hidden:?}"
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

        // Files: alpha.txt, gamma.rs (alphabetical)
        let file_names: Vec<&str> = children
            .iter()
            .filter(|n| matches!(n.kind, NodeKind::File { .. }))
            .map(|n| n.name.as_str())
            .collect();
        assert_eq!(file_names, vec!["alpha.txt", "gamma.rs"]);
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
        // 5 nodes (root + 2 dirs + 2 files), go to last then wrap
        for _ in 0..4 {
            tree.move_down();
        }
        assert_eq!(tree.selected(), 4);
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
        assert_eq!(tree.selected(), 4); // last node
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
        assert_eq!(tree.selected(), 4);
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
}
