use std::path::Path;
use std::sync::mpsc;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use notify::{EventKind, RecommendedWatcher, RecursiveMode, Watcher};

/// Debounce interval: ignore rapid consecutive events within this window.
const DEBOUNCE_INTERVAL: Duration = Duration::from_millis(300);

/// Maximum events to drain per `has_changes()` call to avoid blocking.
const MAX_DRAIN_EVENTS: usize = 1000;

/// Watches a directory tree for filesystem changes (create, remove, rename).
///
/// Uses the `notify` crate with a standard mpsc channel. The `has_changes()`
/// method drains pending events and returns `true` if relevant changes were
/// detected and the debounce interval has elapsed.
pub struct FileWatcher {
    _watcher: RecommendedWatcher,
    rx: mpsc::Receiver<notify::Result<notify::Event>>,
    last_event_time: Option<Instant>,
    has_pending: bool,
}

impl FileWatcher {
    /// Creates a new recursive filesystem watcher for the given root directory.
    pub fn new(root: &Path) -> Result<Self> {
        let (tx, rx) = mpsc::channel();

        let mut watcher =
            notify::recommended_watcher(tx).context("Failed to create filesystem watcher")?;

        watcher
            .watch(root, RecursiveMode::Recursive)
            .with_context(|| format!("Failed to watch directory: {}", root.display()))?;

        Ok(Self {
            _watcher: watcher,
            rx,
            last_event_time: None,
            has_pending: false,
        })
    }

    /// Drains pending filesystem events and returns `true` if relevant changes
    /// were detected and the debounce interval has elapsed.
    ///
    /// Filters to `Create`, `Remove`, and `Modify(Name)` (rename) events only.
    /// Drains at most `MAX_DRAIN_EVENTS` per call to avoid blocking.
    pub fn has_changes(&mut self) -> bool {
        let mut drained = 0;
        while drained < MAX_DRAIN_EVENTS {
            match self.rx.try_recv() {
                Ok(Ok(event)) if Self::is_relevant(&event.kind) => {
                    self.last_event_time = Some(Instant::now());
                    self.has_pending = true;
                }
                Ok(_) => {
                    // Irrelevant event or error — skip.
                }
                Err(mpsc::TryRecvError::Empty) => break,
                Err(mpsc::TryRecvError::Disconnected) => break,
            }
            drained += 1;
        }

        if self.has_pending {
            if let Some(last) = self.last_event_time {
                if last.elapsed() >= DEBOUNCE_INTERVAL {
                    self.has_pending = false;
                    self.last_event_time = None;
                    return true;
                }
            }
        }

        false
    }

    /// Returns `true` if the event kind is relevant for tree or buffer refresh.
    ///
    /// Includes data modifications (not just renames) so that external changes
    /// like `git checkout .` trigger buffer reloads and diff recalculations.
    fn is_relevant(kind: &EventKind) -> bool {
        matches!(
            kind,
            EventKind::Create(_)
                | EventKind::Remove(_)
                | EventKind::Modify(
                    notify::event::ModifyKind::Name(_) | notify::event::ModifyKind::Data(_)
                )
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn file_watcher_no_changes_initially() {
        let tmp = TempDir::new().unwrap();
        let mut watcher = FileWatcher::new(tmp.path()).unwrap();
        assert!(!watcher.has_changes());
    }

    #[test]
    #[ignore] // Timing-sensitive: may be flaky in CI.
    fn file_watcher_detects_new_file() {
        let tmp = TempDir::new().unwrap();
        let mut watcher = FileWatcher::new(tmp.path()).unwrap();

        fs::write(tmp.path().join("new.txt"), "hello").unwrap();

        // Wait for debounce + OS event propagation.
        std::thread::sleep(Duration::from_millis(500));

        assert!(watcher.has_changes());
    }

    #[test]
    #[ignore] // Timing-sensitive: may be flaky in CI.
    fn file_watcher_detects_deletion() {
        let tmp = TempDir::new().unwrap();
        let file = tmp.path().join("to_delete.txt");
        fs::write(&file, "content").unwrap();

        // Small delay so the watcher is set up after file creation.
        std::thread::sleep(Duration::from_millis(100));
        let mut watcher = FileWatcher::new(tmp.path()).unwrap();
        // Drain any creation events from the initial file.
        std::thread::sleep(Duration::from_millis(400));
        let _ = watcher.has_changes();

        fs::remove_file(&file).unwrap();
        std::thread::sleep(Duration::from_millis(500));

        assert!(watcher.has_changes());
    }
}
