use std::path::Path;

use git2::Repository;

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

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command as ProcessCommand;

    #[test]
    fn non_git_dir_returns_none() {
        let tmp = tempfile::tempdir().expect("failed to create tempdir");
        assert_eq!(current_branch(tmp.path()), None);
    }

    #[test]
    fn git_repo_returns_branch_name() {
        let tmp = tempfile::tempdir().expect("failed to create tempdir");
        let dir = tmp.path();

        // Init a repo and create an initial commit so HEAD exists.
        ProcessCommand::new("git")
            .args(["init", "-b", "main"])
            .current_dir(dir)
            .output()
            .expect("git init failed");
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

        // Init, commit, then detach HEAD.
        ProcessCommand::new("git")
            .args(["init", "-b", "main"])
            .current_dir(dir)
            .output()
            .expect("git init failed");
        ProcessCommand::new("git")
            .args(["commit", "--allow-empty", "-m", "init"])
            .current_dir(dir)
            .output()
            .expect("git commit failed");

        // Get the commit hash.
        let output = ProcessCommand::new("git")
            .args(["rev-parse", "--short", "HEAD"])
            .current_dir(dir)
            .output()
            .expect("git rev-parse failed");
        let short_hash = String::from_utf8_lossy(&output.stdout).trim().to_string();

        // Detach HEAD.
        ProcessCommand::new("git")
            .args(["checkout", "--detach"])
            .current_dir(dir)
            .output()
            .expect("git checkout --detach failed");

        assert_eq!(current_branch(dir), Some(short_hash));
    }
}
