use std::collections::HashSet;
use std::path::{Path, PathBuf};

use axe_editor::diff::{DiffHunk, DiffHunkKind};
use git2::{DiffOptions, Repository};

/// Returns the current branch name, or short commit hash for detached HEAD.
/// Returns `None` if the path is not inside a git repository.
pub fn current_branch(project_root: &Path) -> Option<String> {
    let repo = Repository::discover(project_root).ok()?;
    let head = repo.head().ok()?;

    if head.is_branch() {
        head.shorthand().map(String::from)
    } else {
        // Detached HEAD — return short commit hash.
        let oid = head.target()?;
        let mut hex = oid.to_string();
        hex.truncate(7);
        Some(hex)
    }
}

/// Computes diff hunks between the HEAD version and current disk content of a file.
///
/// Returns an empty `Vec` if the file is not tracked by git, the repo cannot be found,
/// or the file is unchanged.
pub fn compute_diff_hunks(project_root: &Path, file_path: &Path) -> Vec<DiffHunk> {
    let repo = match Repository::discover(project_root) {
        Ok(r) => r,
        Err(_) => return Vec::new(),
    };

    let workdir = match repo.workdir() {
        Some(w) => w,
        None => return Vec::new(),
    };

    // Get relative path within the repo.
    let rel_path = resolve_relative_path(file_path, workdir);
    let rel_path = match rel_path {
        Some(p) => p,
        None => return Vec::new(),
    };

    // Get the HEAD tree.
    let head_tree = match repo.head().ok().and_then(|h| h.peel_to_tree().ok()) {
        Some(t) => t,
        None => return Vec::new(),
    };

    // Check if the file is tracked (exists in HEAD tree).
    let rel_str = match rel_path.to_str() {
        Some(s) => s,
        None => return Vec::new(),
    };
    if head_tree.get_path(&rel_path).is_err() {
        // File not tracked in HEAD — return empty per acceptance criteria.
        return Vec::new();
    }

    // Diff HEAD tree to workdir, filtered to just this file.
    // Use zero context lines to get precise hunk boundaries.
    let mut opts = DiffOptions::new();
    opts.pathspec(rel_str);
    opts.context_lines(0);

    let diff = match repo.diff_tree_to_workdir(Some(&head_tree), Some(&mut opts)) {
        Ok(d) => d,
        Err(_) => return Vec::new(),
    };

    // Extract hunks from the diff.
    let mut hunks = Vec::new();
    let result = diff.foreach(
        &mut |_delta, _progress| true,
        None,
        Some(&mut |_delta, hunk| {
            let new_start = hunk.new_start().saturating_sub(1) as usize; // 1-based to 0-based
            let new_lines = hunk.new_lines() as usize;
            let old_lines = hunk.old_lines() as usize;

            let kind = if old_lines == 0 {
                DiffHunkKind::Added
            } else if new_lines == 0 {
                DiffHunkKind::Deleted
            } else {
                DiffHunkKind::Modified
            };

            hunks.push(DiffHunk {
                start_line: new_start,
                line_count: new_lines,
                kind,
            });
            true
        }),
        None,
    );

    if result.is_err() {
        return Vec::new();
    }

    hunks
}

/// Resolves a file path to a relative path within the workdir.
fn resolve_relative_path(file_path: &Path, workdir: &Path) -> Option<std::path::PathBuf> {
    if let Ok(rel) = file_path.strip_prefix(workdir) {
        return Some(rel.to_path_buf());
    }
    // Try canonicalizing both paths for symlink resolution.
    let canon_file = file_path.canonicalize().ok()?;
    let canon_workdir = workdir.canonicalize().ok()?;
    canon_file
        .strip_prefix(&canon_workdir)
        .ok()
        .map(|p| p.to_path_buf())
}

/// Returns the set of absolute file paths that have uncommitted changes.
///
/// Includes modified, new (untracked), deleted, and renamed files.
/// Returns an empty set if the path is not inside a git repository.
pub fn modified_files(project_root: &Path) -> HashSet<PathBuf> {
    let repo = match Repository::discover(project_root) {
        Ok(r) => r,
        Err(_) => return HashSet::new(),
    };

    let workdir = match repo.workdir() {
        Some(w) => w.to_path_buf(),
        None => return HashSet::new(),
    };

    let mut result = HashSet::new();

    // Diff HEAD tree to workdir (tracked changes).
    if let Ok(head_tree) = repo.head().and_then(|h| h.peel_to_tree()) {
        if let Ok(diff) = repo.diff_tree_to_workdir_with_index(Some(&head_tree), None) {
            for i in 0..diff.deltas().len() {
                if let Some(delta) = diff.get_delta(i) {
                    // Use new_file path for renames/additions, old_file for deletions.
                    let rel = delta.new_file().path().or_else(|| delta.old_file().path());
                    if let Some(p) = rel {
                        result.insert(workdir.join(p));
                    }
                }
            }
        }
    }

    // Also pick up untracked files.
    let mut status_opts = git2::StatusOptions::new();
    status_opts.include_untracked(true);
    status_opts.recurse_untracked_dirs(true);
    if let Ok(statuses) = repo.statuses(Some(&mut status_opts)) {
        for entry in statuses.iter() {
            if let Some(p) = entry.path() {
                let status = entry.status();
                if status.intersects(
                    git2::Status::WT_NEW
                        | git2::Status::WT_MODIFIED
                        | git2::Status::WT_DELETED
                        | git2::Status::WT_RENAMED
                        | git2::Status::INDEX_NEW
                        | git2::Status::INDEX_MODIFIED
                        | git2::Status::INDEX_DELETED
                        | git2::Status::INDEX_RENAMED,
                ) {
                    result.insert(workdir.join(p));
                }
            }
        }
    }

    result
}

/// Returns the set of ancestor directories that transitively contain at least
/// one modified file. The project root itself is excluded.
pub fn dirty_parent_dirs(
    modified_files: &HashSet<PathBuf>,
    project_root: &Path,
) -> HashSet<PathBuf> {
    let mut dirs = HashSet::new();
    for file_path in modified_files {
        let mut ancestor = file_path.parent();
        while let Some(dir) = ancestor {
            if dir == project_root || !dir.starts_with(project_root) {
                break;
            }
            if !dirs.insert(dir.to_path_buf()) {
                break; // already visited — ancestors already added
            }
            ancestor = dir.parent();
        }
    }
    dirs
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command as ProcessCommand;

    /// Helper to initialize a git repo with a committed file.
    fn init_repo_with_file(dir: &Path, filename: &str, content: &str) {
        ProcessCommand::new("git")
            .args(["init", "-b", "main"])
            .current_dir(dir)
            .output()
            .expect("git init failed");
        ProcessCommand::new("git")
            .args(["config", "user.email", "test@test.com"])
            .current_dir(dir)
            .output()
            .expect("git config email failed");
        ProcessCommand::new("git")
            .args(["config", "user.name", "Test"])
            .current_dir(dir)
            .output()
            .expect("git config name failed");

        let file_path = dir.join(filename);
        std::fs::write(&file_path, content).expect("write file failed");

        ProcessCommand::new("git")
            .args(["add", filename])
            .current_dir(dir)
            .output()
            .expect("git add failed");
        ProcessCommand::new("git")
            .args(["commit", "-m", "initial"])
            .current_dir(dir)
            .output()
            .expect("git commit failed");
    }

    #[test]
    fn non_git_dir_returns_none() {
        let tmp = tempfile::tempdir().expect("failed to create tempdir");
        assert_eq!(current_branch(tmp.path()), None);
    }

    #[test]
    fn git_repo_returns_branch_name() {
        let tmp = tempfile::tempdir().expect("failed to create tempdir");
        let dir = tmp.path();

        ProcessCommand::new("git")
            .args(["init", "-b", "main"])
            .current_dir(dir)
            .output()
            .expect("git init failed");
        ProcessCommand::new("git")
            .args(["config", "user.email", "test@test.com"])
            .current_dir(dir)
            .output()
            .expect("git config email failed");
        ProcessCommand::new("git")
            .args(["config", "user.name", "Test"])
            .current_dir(dir)
            .output()
            .expect("git config name failed");
        ProcessCommand::new("git")
            .args(["commit", "--allow-empty", "-m", "init"])
            .current_dir(dir)
            .output()
            .expect("git commit failed");

        assert_eq!(current_branch(dir), Some("main".to_string()));
    }

    #[test]
    fn detached_head_returns_short_hash() {
        let tmp = tempfile::tempdir().expect("failed to create tempdir");
        let dir = tmp.path();

        ProcessCommand::new("git")
            .args(["init", "-b", "main"])
            .current_dir(dir)
            .output()
            .expect("git init failed");
        ProcessCommand::new("git")
            .args(["config", "user.email", "test@test.com"])
            .current_dir(dir)
            .output()
            .expect("git config email failed");
        ProcessCommand::new("git")
            .args(["config", "user.name", "Test"])
            .current_dir(dir)
            .output()
            .expect("git config name failed");
        ProcessCommand::new("git")
            .args(["commit", "--allow-empty", "-m", "init"])
            .current_dir(dir)
            .output()
            .expect("git commit failed");

        let output = ProcessCommand::new("git")
            .args(["rev-parse", "--short", "HEAD"])
            .current_dir(dir)
            .output()
            .expect("git rev-parse failed");
        let short_hash = String::from_utf8_lossy(&output.stdout).trim().to_string();

        ProcessCommand::new("git")
            .args(["checkout", "--detach"])
            .current_dir(dir)
            .output()
            .expect("git checkout --detach failed");

        assert_eq!(current_branch(dir), Some(short_hash));
    }

    // --- compute_diff_hunks tests ---

    #[test]
    fn diff_untracked_file_returns_empty() {
        let tmp = tempfile::tempdir().expect("failed to create tempdir");
        let dir = tmp.path();

        init_repo_with_file(dir, "tracked.txt", "hello\n");
        let untracked = dir.join("untracked.txt");
        std::fs::write(&untracked, "new content\n").expect("write failed");

        let hunks = compute_diff_hunks(dir, &untracked);
        assert!(hunks.is_empty(), "untracked file should have no hunks");
    }

    #[test]
    fn diff_unchanged_file_returns_empty() {
        let tmp = tempfile::tempdir().expect("failed to create tempdir");
        let dir = tmp.path();

        init_repo_with_file(dir, "test.txt", "line 1\nline 2\nline 3\n");

        let hunks = compute_diff_hunks(dir, &dir.join("test.txt"));
        assert!(hunks.is_empty(), "unchanged file should have no hunks");
    }

    #[test]
    fn diff_added_lines_detected() {
        let tmp = tempfile::tempdir().expect("failed to create tempdir");
        let dir = tmp.path();

        init_repo_with_file(dir, "test.txt", "line 1\nline 2\n");

        std::fs::write(dir.join("test.txt"), "line 1\nline 2\nline 3\nline 4\n")
            .expect("write failed");

        let hunks = compute_diff_hunks(dir, &dir.join("test.txt"));
        assert!(!hunks.is_empty(), "should detect added lines");
        assert!(
            hunks.iter().any(|h| h.kind == DiffHunkKind::Added),
            "should have Added hunk"
        );
    }

    #[test]
    fn diff_modified_lines_detected() {
        let tmp = tempfile::tempdir().expect("failed to create tempdir");
        let dir = tmp.path();

        init_repo_with_file(dir, "test.txt", "line 1\nline 2\nline 3\n");

        std::fs::write(dir.join("test.txt"), "line 1\nmodified\nline 3\n").expect("write failed");

        let hunks = compute_diff_hunks(dir, &dir.join("test.txt"));
        assert!(!hunks.is_empty(), "should detect modified lines");
        assert!(
            hunks.iter().any(|h| h.kind == DiffHunkKind::Modified),
            "should have Modified hunk"
        );
    }

    #[test]
    fn diff_deleted_lines_detected() {
        let tmp = tempfile::tempdir().expect("failed to create tempdir");
        let dir = tmp.path();

        init_repo_with_file(dir, "test.txt", "line 1\nline 2\nline 3\n");

        std::fs::write(dir.join("test.txt"), "line 1\nline 3\n").expect("write failed");

        let hunks = compute_diff_hunks(dir, &dir.join("test.txt"));
        assert!(!hunks.is_empty(), "should detect deleted lines");
        assert!(
            hunks.iter().any(|h| h.kind == DiffHunkKind::Deleted),
            "should have Deleted hunk"
        );
    }

    #[test]
    fn diff_non_git_dir_returns_empty() {
        let tmp = tempfile::tempdir().expect("failed to create tempdir");
        let file = tmp.path().join("test.txt");
        std::fs::write(&file, "content\n").expect("write failed");

        let hunks = compute_diff_hunks(tmp.path(), &file);
        assert!(hunks.is_empty(), "non-git dir should return empty hunks");
    }

    // --- modified_files tests ---

    #[test]
    fn modified_files_non_git_dir_returns_empty() {
        let tmp = tempfile::tempdir().expect("failed to create tempdir");
        let result = modified_files(tmp.path());
        assert!(result.is_empty());
    }

    #[test]
    fn modified_files_clean_repo_returns_empty() {
        let tmp = tempfile::tempdir().expect("failed to create tempdir");
        let dir = tmp.path();
        init_repo_with_file(dir, "test.txt", "hello\n");

        let result = modified_files(dir);
        assert!(
            result.is_empty(),
            "clean repo should have no modified files"
        );
    }

    #[test]
    fn modified_files_detects_changed_file() {
        let tmp = tempfile::tempdir().expect("failed to create tempdir");
        let dir = tmp.path();
        init_repo_with_file(dir, "test.txt", "hello\n");

        // Modify the file.
        std::fs::write(dir.join("test.txt"), "changed\n").expect("write failed");

        let result = modified_files(dir);
        assert!(
            result.iter().any(|p| p.ends_with("test.txt")),
            "should contain the modified file"
        );
    }

    #[test]
    fn modified_files_detects_new_untracked_file() {
        let tmp = tempfile::tempdir().expect("failed to create tempdir");
        let dir = tmp.path();
        init_repo_with_file(dir, "test.txt", "hello\n");

        // Add an untracked file.
        std::fs::write(dir.join("new.txt"), "new\n").expect("write failed");

        let result = modified_files(dir);
        assert!(
            result.iter().any(|p| p.ends_with("new.txt")),
            "should contain the untracked file"
        );
    }

    #[test]
    fn modified_files_detects_deleted_file() {
        let tmp = tempfile::tempdir().expect("failed to create tempdir");
        let dir = tmp.path();
        init_repo_with_file(dir, "test.txt", "hello\n");

        std::fs::remove_file(dir.join("test.txt")).expect("remove failed");

        let result = modified_files(dir);
        assert!(
            result.iter().any(|p| p.ends_with("test.txt")),
            "should contain the deleted file"
        );
    }

    // ── dirty_parent_dirs tests ──────────────────────────────────────

    #[test]
    fn dirty_parent_dirs_empty() {
        let root = PathBuf::from("/project");
        let modified = HashSet::new();
        let result = dirty_parent_dirs(&modified, &root);
        assert!(result.is_empty());
    }

    #[test]
    fn dirty_parent_dirs_single_file() {
        let root = PathBuf::from("/project");
        let modified = HashSet::from([PathBuf::from("/project/src/main.rs")]);
        let result = dirty_parent_dirs(&modified, &root);
        assert_eq!(result, HashSet::from([PathBuf::from("/project/src")]));
    }

    #[test]
    fn dirty_parent_dirs_nested() {
        let root = PathBuf::from("/project");
        let modified = HashSet::from([PathBuf::from("/project/src/handlers/auth/login.rs")]);
        let result = dirty_parent_dirs(&modified, &root);
        assert_eq!(
            result,
            HashSet::from([
                PathBuf::from("/project/src"),
                PathBuf::from("/project/src/handlers"),
                PathBuf::from("/project/src/handlers/auth"),
            ])
        );
    }

    #[test]
    fn dirty_parent_dirs_excludes_root() {
        let root = PathBuf::from("/project");
        let modified = HashSet::from([PathBuf::from("/project/src/lib.rs")]);
        let result = dirty_parent_dirs(&modified, &root);
        assert!(
            !result.contains(&PathBuf::from("/project")),
            "project root must not be in the result"
        );
    }

    #[test]
    fn dirty_parent_dirs_shared_ancestors() {
        let root = PathBuf::from("/project");
        let modified = HashSet::from([
            PathBuf::from("/project/src/a.rs"),
            PathBuf::from("/project/src/b.rs"),
        ]);
        let result = dirty_parent_dirs(&modified, &root);
        assert_eq!(result, HashSet::from([PathBuf::from("/project/src")]));
    }

    #[test]
    fn dirty_parent_dirs_file_at_root_level() {
        let root = PathBuf::from("/project");
        let modified = HashSet::from([PathBuf::from("/project/Cargo.toml")]);
        let result = dirty_parent_dirs(&modified, &root);
        assert!(
            result.is_empty(),
            "file directly under root should produce no dirty dirs"
        );
    }
}
