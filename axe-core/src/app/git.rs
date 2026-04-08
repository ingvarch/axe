use std::time::{Duration, Instant};

use super::AppState;

impl AppState {
    /// Interval between git branch checks.
    const GIT_BRANCH_CHECK_INTERVAL: Duration = Duration::from_secs(5);

    /// Refreshes the git branch name if the check interval has elapsed.
    ///
    /// Called from the main loop on every tick. Skips the check if the interval
    /// has not elapsed yet. Also callable with a forced refresh after file save.
    pub fn refresh_git_branch(&mut self) {
        if let Some(last_check) = self.last_git_branch_check {
            if last_check.elapsed() < Self::GIT_BRANCH_CHECK_INTERVAL {
                return;
            }
        }
        if let Some(ref root) = self.project_root {
            self.git_branch = crate::git::current_branch(root);
        }
        self.last_git_branch_check = Some(Instant::now());
        // Periodic diff refresh as a safety net: catches git operations
        // that may not trigger filesystem events (e.g. HEAD changes).
        self.refresh_active_buffer_diff_hunks();
    }

    /// Refreshes the set of modified files for the tree panel.
    pub(super) fn refresh_git_modified_files(&mut self) {
        if let Some(ref root) = self.project_root {
            self.git_modified_files = crate::git::modified_files(root);
            self.git_dirty_dirs = crate::git::dirty_parent_dirs(&self.git_modified_files, root);
        }
    }

    /// Recalculates git diff hunks for the active buffer.
    ///
    /// Compares the HEAD version of the file against the current buffer content
    /// (not disk), so diff markers reflect what the user sees in the editor even
    /// when there are unsaved changes (e.g. after a hunk revert).
    pub(super) fn refresh_active_buffer_diff_hunks(&mut self) {
        if let Some(ref root) = self.project_root {
            if let Some(buf) = self.buffer_manager.active_buffer_mut() {
                if let Some(path) = buf.path().map(|p| p.to_path_buf()) {
                    let content = buf.content_string();
                    let hunks = crate::git::compute_diff_hunks_from_content(root, &path, &content);
                    buf.set_diff_hunks(hunks);
                }
            }
        }
    }

    /// Forces an immediate git branch refresh, bypassing the interval check.
    pub(super) fn force_refresh_git_branch(&mut self) {
        if let Some(ref root) = self.project_root {
            self.git_branch = crate::git::current_branch(root);
        }
        self.last_git_branch_check = Some(Instant::now());
    }
}
