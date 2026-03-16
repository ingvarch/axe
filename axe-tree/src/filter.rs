use std::path::{Path, PathBuf};

use ignore::gitignore::{Gitignore, GitignoreBuilder};

/// Filters file tree entries based on `.gitignore` patterns and default ignore rules.
///
/// Handles root and nested `.gitignore` files, plus hardcoded defaults
/// (`node_modules/`, `target/`). Supports toggling visibility of ignored files.
pub struct TreeFilter {
    /// Compiled gitignore matcher.
    gitignore: Option<Gitignore>,
    /// Whether to show ignored files (toggle state).
    show_ignored: bool,
    /// Git repository root (for resolving gitignore paths).
    git_root: Option<PathBuf>,
    /// Accumulated gitignore file paths (for rebuilding when nested `.gitignore` found).
    gitignore_paths: Vec<PathBuf>,
}

impl TreeFilter {
    /// Creates a new filter by finding the git root and loading `.gitignore` patterns.
    ///
    /// Walks up from `root` looking for a `.git/` directory. If found, loads the root
    /// `.gitignore` and adds default patterns (`node_modules/`, `target/`).
    pub fn new(root: &Path) -> Self {
        let git_root = Self::find_git_root(root);
        let mut gitignore_paths = Vec::new();

        if let Some(ref gr) = git_root {
            let gitignore_file = gr.join(".gitignore");
            if gitignore_file.exists() {
                gitignore_paths.push(gitignore_file);
            }
        }

        let gitignore = Self::build_gitignore(git_root.as_deref(), &gitignore_paths);

        Self {
            gitignore,
            show_ignored: true,
            git_root,
            gitignore_paths,
        }
    }

    /// Adds a nested `.gitignore` from the given directory and rebuilds the matcher.
    ///
    /// Call this when expanding a directory to pick up its `.gitignore` rules.
    pub fn add_nested_gitignore(&mut self, dir: &Path) {
        let gitignore_file = dir.join(".gitignore");
        if gitignore_file.exists() && !self.gitignore_paths.contains(&gitignore_file) {
            self.gitignore_paths.push(gitignore_file);
            self.gitignore = Self::build_gitignore(self.git_root.as_deref(), &self.gitignore_paths);
        }
    }

    /// Returns `true` if the entry should be shown in the tree.
    ///
    /// When `show_ignored` is `true`, all entries pass. Otherwise, entries matching
    /// `.gitignore` patterns or default ignore rules are hidden.
    pub fn is_visible(&self, path: &Path, is_dir: bool) -> bool {
        if self.show_ignored {
            return true;
        }

        if let Some(ref gi) = self.gitignore {
            let matched = gi.matched(path, is_dir);
            if matched.is_ignore() {
                return false;
            }
        }

        true
    }

    /// Toggles whether ignored files are shown.
    pub fn toggle_show_ignored(&mut self) {
        self.show_ignored = !self.show_ignored;
    }

    /// Returns whether ignored files are currently shown.
    pub fn show_ignored(&self) -> bool {
        self.show_ignored
    }

    /// Walks up from `start` looking for a `.git/` directory.
    fn find_git_root(start: &Path) -> Option<PathBuf> {
        let mut current = start.to_path_buf();
        loop {
            if current.join(".git").is_dir() {
                return Some(current);
            }
            if !current.pop() {
                return None;
            }
        }
    }

    /// Builds a `Gitignore` matcher from collected `.gitignore` files plus default patterns.
    fn build_gitignore(git_root: Option<&Path>, paths: &[PathBuf]) -> Option<Gitignore> {
        let root = git_root?;
        let mut builder = GitignoreBuilder::new(root);

        // Default patterns — always applied.
        builder
            .add_line(None, "node_modules/")
            .expect("default pattern 'node_modules/' is valid");
        builder
            .add_line(None, "target/")
            .expect("default pattern 'target/' is valid");

        for path in paths {
            builder.add(path);
        }

        builder.build().ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn find_git_root_returns_none_without_git() {
        let tmp = TempDir::new().unwrap();
        let result = TreeFilter::find_git_root(tmp.path());
        assert!(result.is_none());
    }

    #[test]
    fn find_git_root_finds_root() {
        let tmp = TempDir::new().unwrap();
        fs::create_dir(tmp.path().join(".git")).unwrap();
        let result = TreeFilter::find_git_root(tmp.path());
        assert_eq!(result, Some(tmp.path().to_path_buf()));
    }

    #[test]
    fn find_git_root_finds_parent() {
        let tmp = TempDir::new().unwrap();
        fs::create_dir(tmp.path().join(".git")).unwrap();
        let nested = tmp.path().join("a/b/c");
        fs::create_dir_all(&nested).unwrap();
        let result = TreeFilter::find_git_root(&nested);
        assert_eq!(result, Some(tmp.path().to_path_buf()));
    }

    #[test]
    fn is_visible_allows_normal_file() {
        let tmp = TempDir::new().unwrap();
        fs::create_dir(tmp.path().join(".git")).unwrap();
        fs::write(tmp.path().join(".gitignore"), "*.log\n").unwrap();
        fs::write(tmp.path().join("main.rs"), "").unwrap();

        let filter = TreeFilter::new(tmp.path());
        assert!(filter.is_visible(&tmp.path().join("main.rs"), false));
    }

    #[test]
    fn is_visible_shows_gitignored_by_default() {
        let tmp = TempDir::new().unwrap();
        fs::create_dir(tmp.path().join(".git")).unwrap();
        fs::write(tmp.path().join(".gitignore"), "*.log\n").unwrap();

        let filter = TreeFilter::new(tmp.path());
        // show_ignored defaults to true — ignored files are visible
        assert!(filter.show_ignored());
        assert!(filter.is_visible(&tmp.path().join("debug.log"), false));
    }

    #[test]
    fn is_visible_hides_gitignored_file_when_toggled() {
        let tmp = TempDir::new().unwrap();
        fs::create_dir(tmp.path().join(".git")).unwrap();
        fs::write(tmp.path().join(".gitignore"), "*.log\n").unwrap();

        let mut filter = TreeFilter::new(tmp.path());
        filter.toggle_show_ignored();
        assert!(!filter.is_visible(&tmp.path().join("debug.log"), false));
    }

    #[test]
    fn is_visible_hides_node_modules_when_toggled() {
        let tmp = TempDir::new().unwrap();
        fs::create_dir(tmp.path().join(".git")).unwrap();
        fs::create_dir(tmp.path().join("node_modules")).unwrap();

        let mut filter = TreeFilter::new(tmp.path());
        filter.toggle_show_ignored();
        assert!(!filter.is_visible(&tmp.path().join("node_modules"), true));
    }

    #[test]
    fn is_visible_hides_target_when_toggled() {
        let tmp = TempDir::new().unwrap();
        fs::create_dir(tmp.path().join(".git")).unwrap();
        fs::create_dir(tmp.path().join("target")).unwrap();

        let mut filter = TreeFilter::new(tmp.path());
        filter.toggle_show_ignored();
        assert!(!filter.is_visible(&tmp.path().join("target"), true));
    }

    #[test]
    fn toggle_show_ignored_hides_ignored() {
        let tmp = TempDir::new().unwrap();
        fs::create_dir(tmp.path().join(".git")).unwrap();
        fs::write(tmp.path().join(".gitignore"), "*.log\n").unwrap();

        let mut filter = TreeFilter::new(tmp.path());
        assert!(filter.is_visible(&tmp.path().join("debug.log"), false));

        filter.toggle_show_ignored();
        assert!(!filter.show_ignored());
        assert!(!filter.is_visible(&tmp.path().join("debug.log"), false));
    }

    #[test]
    fn toggle_show_ignored_twice_shows_again() {
        let tmp = TempDir::new().unwrap();
        fs::create_dir(tmp.path().join(".git")).unwrap();
        fs::write(tmp.path().join(".gitignore"), "*.log\n").unwrap();

        let mut filter = TreeFilter::new(tmp.path());
        filter.toggle_show_ignored();
        filter.toggle_show_ignored();
        assert!(filter.show_ignored());
        assert!(filter.is_visible(&tmp.path().join("debug.log"), false));
    }

    #[test]
    fn nested_gitignore_respected() {
        let tmp = TempDir::new().unwrap();
        fs::create_dir(tmp.path().join(".git")).unwrap();
        let sub = tmp.path().join("sub");
        fs::create_dir(&sub).unwrap();
        fs::write(sub.join(".gitignore"), "*.tmp\n").unwrap();
        fs::write(sub.join("data.tmp"), "").unwrap();

        let mut filter = TreeFilter::new(tmp.path());
        // Toggle to hide ignored files so we can test filter behavior
        filter.toggle_show_ignored();

        // Before adding nested gitignore, *.tmp is not hidden
        assert!(filter.is_visible(&sub.join("data.tmp"), false));

        filter.add_nested_gitignore(&sub);
        assert!(!filter.is_visible(&sub.join("data.tmp"), false));
    }

    #[test]
    fn filter_without_git_root_allows_all() {
        let tmp = TempDir::new().unwrap();
        // No .git directory
        let filter = TreeFilter::new(tmp.path());
        assert!(filter.is_visible(&tmp.path().join("anything.txt"), false));
    }

    #[test]
    fn add_nested_gitignore_deduplicates() {
        let tmp = TempDir::new().unwrap();
        fs::create_dir(tmp.path().join(".git")).unwrap();
        let sub = tmp.path().join("sub");
        fs::create_dir(&sub).unwrap();
        fs::write(sub.join(".gitignore"), "*.tmp\n").unwrap();

        let mut filter = TreeFilter::new(tmp.path());
        filter.add_nested_gitignore(&sub);
        let count_before = filter.gitignore_paths.len();
        filter.add_nested_gitignore(&sub);
        assert_eq!(filter.gitignore_paths.len(), count_before);
    }
}
