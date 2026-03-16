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
}
