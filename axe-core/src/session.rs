use std::collections::HashSet;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::app::FocusTarget;
use crate::AppState;

/// Current session format version.
const SESSION_VERSION: u32 = 1;

/// Session file path relative to project root.
const SESSION_DIR: &str = ".axe";
const SESSION_FILE: &str = "session.local.json";
/// Pattern for the global gitignore (`~/.config/git/ignore`).
const GLOBAL_GITIGNORE_PATTERN: &str = "**/.axe/session.local.json";

/// Persisted session state for a project.
///
/// Captures open buffers, cursor positions, tree expansion, layout,
/// and focus so that reopening a project restores the previous working state.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Session {
    pub version: u32,
    pub buffers: Vec<BufferSession>,
    pub active_buffer: usize,
    pub tree: TreeSession,
    pub layout: LayoutSession,
    pub focus: String,
}

/// Persisted state for a single editor buffer.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BufferSession {
    pub path: PathBuf,
    pub cursor_row: usize,
    pub cursor_col: usize,
    pub scroll_row: usize,
    pub scroll_col: usize,
}

/// Persisted state for the file tree panel.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TreeSession {
    pub expanded_paths: Vec<PathBuf>,
    pub selected_path: Option<PathBuf>,
    pub scroll: usize,
}

/// Persisted layout dimensions.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LayoutSession {
    pub tree_width_pct: u16,
    pub editor_height_pct: u16,
    pub show_tree: bool,
    pub show_terminal: bool,
}

/// Ensures `GLOBAL_GITIGNORE_PATTERN` is present in `~/.config/git/ignore`.
///
/// Creates the file and parent directories if they don't exist.
/// Appends the pattern only if it's not already present.
fn ensure_global_gitignore() -> Result<()> {
    let home = dirs::home_dir().context("Could not determine home directory")?;
    ensure_global_gitignore_at(&home.join(".config/git/ignore"))
}

/// Testable helper: ensures `GLOBAL_GITIGNORE_PATTERN` is present in the given file.
fn ensure_global_gitignore_at(path: &Path) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create {}", parent.display()))?;
    }

    let content = std::fs::read_to_string(path).unwrap_or_default();

    if content.lines().any(|line| line.trim() == GLOBAL_GITIGNORE_PATTERN) {
        return Ok(());
    }

    use std::io::Write;
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .with_context(|| format!("Failed to open {}", path.display()))?;

    // Ensure we start on a new line if the file doesn't end with one.
    if !content.is_empty() && !content.ends_with('\n') {
        writeln!(file)?;
    }

    writeln!(file, "{GLOBAL_GITIGNORE_PATTERN}")?;

    Ok(())
}

impl Session {
    /// Snapshots the current application state into a serializable session.
    pub fn from_app(app: &AppState) -> Self {
        // Capture open buffers with cursor/scroll state.
        let buffers: Vec<BufferSession> = app
            .buffer_manager
            .buffers()
            .iter()
            .filter_map(|buf| {
                buf.path().map(|p| BufferSession {
                    path: p.to_path_buf(),
                    cursor_row: buf.cursor.row,
                    cursor_col: buf.cursor.col,
                    scroll_row: buf.scroll_row,
                    scroll_col: buf.scroll_col,
                })
            })
            .collect();

        let active_buffer = app.buffer_manager.active_index();

        // Capture tree expanded paths.
        let tree = if let Some(ref file_tree) = app.file_tree {
            let expanded_paths: Vec<PathBuf> = file_tree
                .nodes()
                .iter()
                .filter(|n| n.expanded && n.depth > 0)
                .map(|n| n.path.clone())
                .collect();
            let selected_path = file_tree.selected_node().map(|n| n.path.clone());
            TreeSession {
                expanded_paths,
                selected_path,
                scroll: file_tree.scroll(),
            }
        } else {
            TreeSession {
                expanded_paths: Vec::new(),
                selected_path: None,
                scroll: 0,
            }
        };

        let layout = LayoutSession {
            tree_width_pct: app.tree_width_pct,
            editor_height_pct: app.editor_height_pct,
            show_tree: app.show_tree,
            show_terminal: app.show_terminal,
        };

        let focus = match &app.focus {
            FocusTarget::Tree => "Tree".to_string(),
            FocusTarget::Editor => "Editor".to_string(),
            FocusTarget::Terminal(idx) => format!("Terminal({})", idx),
        };

        Self {
            version: SESSION_VERSION,
            buffers,
            active_buffer,
            tree,
            layout,
            focus,
        }
    }

    /// Saves the session atomically to `{project_root}/.axe/session.local.json`.
    ///
    /// Creates the `.axe/` directory if needed. Also creates `.axe/.gitignore`
    /// (if absent) to keep the session file out of version control.
    /// Writes to a temp file first, then renames to prevent partial writes.
    pub fn save(&self, project_root: &Path) -> Result<()> {
        let session_dir = project_root.join(SESSION_DIR);
        std::fs::create_dir_all(&session_dir)
            .with_context(|| format!("Failed to create {}", session_dir.display()))?;

        // Ensure the session file pattern is in the global gitignore.
        if let Err(e) = ensure_global_gitignore() {
            log::warn!("Failed to update global gitignore: {e}");
        }

        let session_path = session_dir.join(SESSION_FILE);
        let tmp_path = session_dir.join(format!("{SESSION_FILE}.tmp"));

        let json = serde_json::to_string_pretty(self).context("Failed to serialize session")?;

        std::fs::write(&tmp_path, &json)
            .with_context(|| format!("Failed to write temp session: {}", tmp_path.display()))?;

        std::fs::rename(&tmp_path, &session_path).with_context(|| {
            format!("Failed to rename session file: {}", session_path.display())
        })?;

        Ok(())
    }

    /// Loads a session from `{project_root}/.axe/session.local.json`.
    ///
    /// Returns `Ok(None)` if the session file does not exist.
    /// Returns `Err` if the file exists but cannot be read or parsed.
    pub fn load(project_root: &Path) -> Result<Option<Session>> {
        let session_path = project_root.join(SESSION_DIR).join(SESSION_FILE);
        if !session_path.exists() {
            return Ok(None);
        }

        let content = std::fs::read_to_string(&session_path)
            .with_context(|| format!("Failed to read session: {}", session_path.display()))?;

        let session: Session = serde_json::from_str(&content)
            .with_context(|| format!("Failed to parse session: {}", session_path.display()))?;

        Ok(Some(session))
    }

    /// Applies the session state to the application.
    ///
    /// Opens files, restores cursor/scroll positions, tree expansion, layout,
    /// and focus. Returns a list of warnings (e.g. files that no longer exist).
    pub fn apply(self, app: &mut AppState) -> Vec<String> {
        let mut warnings = Vec::new();

        // Restore buffers.
        for buf_session in &self.buffers {
            if !buf_session.path.exists() {
                warnings.push(format!(
                    "File no longer exists: {}",
                    buf_session.path.display()
                ));
                continue;
            }
            if let Err(e) = app.buffer_manager.open_file(&buf_session.path) {
                warnings.push(format!(
                    "Failed to open {}: {e}",
                    buf_session.path.display()
                ));
                continue;
            }
            // Set cursor and scroll on the just-opened buffer.
            if let Some(buf) = app.buffer_manager.active_buffer_mut() {
                buf.cursor.row = buf_session.cursor_row;
                buf.cursor.col = buf_session.cursor_col;
                buf.scroll_row = buf_session.scroll_row;
                buf.scroll_col = buf_session.scroll_col;
            }
        }

        // Restore active buffer index.
        if !self.buffers.is_empty() {
            app.buffer_manager.set_active(self.active_buffer);
        }

        // Restore tree state.
        if let Some(ref mut file_tree) = app.file_tree {
            let expanded_set: HashSet<PathBuf> = self.tree.expanded_paths.into_iter().collect();
            file_tree.restore_expanded(&expanded_set);

            if let Some(ref path) = self.tree.selected_path {
                file_tree.set_selected_by_path(path);
            }
            file_tree.set_scroll(self.tree.scroll);
        }

        // Restore layout.
        app.tree_width_pct = self.layout.tree_width_pct;
        app.editor_height_pct = self.layout.editor_height_pct;
        app.show_tree = self.layout.show_tree;
        app.show_terminal = self.layout.show_terminal;

        // Restore focus.
        app.focus = match self.focus.as_str() {
            "Editor" => FocusTarget::Editor,
            "Tree" => FocusTarget::Tree,
            s if s.starts_with("Terminal(") => {
                let idx = s
                    .trim_start_matches("Terminal(")
                    .trim_end_matches(')')
                    .parse::<usize>()
                    .unwrap_or(0);
                FocusTarget::Terminal(idx)
            }
            _ => FocusTarget::Editor,
        };

        warnings
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn session_roundtrip_serialization() {
        let session = Session {
            version: 1,
            buffers: vec![BufferSession {
                path: PathBuf::from("/tmp/test.rs"),
                cursor_row: 42,
                cursor_col: 10,
                scroll_row: 15,
                scroll_col: 0,
            }],
            active_buffer: 0,
            tree: TreeSession {
                expanded_paths: vec![PathBuf::from("/tmp/src")],
                selected_path: Some(PathBuf::from("/tmp/src/main.rs")),
                scroll: 5,
            },
            layout: LayoutSession {
                tree_width_pct: 20,
                editor_height_pct: 70,
                show_tree: true,
                show_terminal: true,
            },
            focus: "Editor".to_string(),
        };

        let json = serde_json::to_string_pretty(&session).unwrap();
        let deserialized: Session = serde_json::from_str(&json).unwrap();
        assert_eq!(session, deserialized);
    }

    #[test]
    fn session_from_app_captures_buffers() {
        let tmp = tempfile::TempDir::new().unwrap();
        let root = tmp.path().to_path_buf();
        let file_path = root.join("test.txt");
        std::fs::write(&file_path, "hello world").unwrap();

        let mut app = AppState::new_with_root(root);
        app.buffer_manager.open_file(&file_path).unwrap();
        app.buffer_manager.active_buffer_mut().unwrap().cursor.row = 5;
        app.buffer_manager.active_buffer_mut().unwrap().cursor.col = 3;

        let session = Session::from_app(&app);
        assert_eq!(session.buffers.len(), 1);
        assert_eq!(session.buffers[0].cursor_row, 5);
        assert_eq!(session.buffers[0].cursor_col, 3);
    }

    #[test]
    fn session_from_app_captures_layout() {
        let tmp = tempfile::TempDir::new().unwrap();
        let mut app = AppState::new_with_root(tmp.path().to_path_buf());
        app.tree_width_pct = 30;
        app.editor_height_pct = 60;
        app.show_tree = false;
        app.show_terminal = false;

        let session = Session::from_app(&app);
        assert_eq!(session.layout.tree_width_pct, 30);
        assert_eq!(session.layout.editor_height_pct, 60);
        assert!(!session.layout.show_tree);
        assert!(!session.layout.show_terminal);
    }

    #[test]
    fn session_from_app_captures_tree_expanded() {
        let tmp = tempfile::TempDir::new().unwrap();
        let root = tmp.path();
        std::fs::create_dir(root.join("src")).unwrap();
        std::fs::write(root.join("src/main.rs"), "").unwrap();

        let mut app = AppState::new_with_root(root.to_path_buf());
        // Expand "src" directory.
        if let Some(ref mut tree) = app.file_tree {
            let src_idx = tree.nodes().iter().position(|n| n.name == "src").unwrap();
            tree.select(src_idx);
            tree.expand().unwrap();
        }

        let session = Session::from_app(&app);
        assert!(!session.tree.expanded_paths.is_empty());
        let expanded_names: Vec<String> = session
            .tree
            .expanded_paths
            .iter()
            .filter_map(|p| p.file_name().map(|n| n.to_string_lossy().into_owned()))
            .collect();
        assert!(expanded_names.contains(&"src".to_string()));
    }

    #[test]
    fn session_save_creates_dotaxe_dir() {
        let tmp = tempfile::TempDir::new().unwrap();
        let session = Session {
            version: 1,
            buffers: Vec::new(),
            active_buffer: 0,
            tree: TreeSession {
                expanded_paths: Vec::new(),
                selected_path: None,
                scroll: 0,
            },
            layout: LayoutSession {
                tree_width_pct: 20,
                editor_height_pct: 70,
                show_tree: true,
                show_terminal: true,
            },
            focus: "Editor".to_string(),
        };

        session.save(tmp.path()).unwrap();
        assert!(tmp.path().join(SESSION_DIR).join(SESSION_FILE).exists());
    }

    #[test]
    fn global_gitignore_creates_file_with_pattern() {
        let tmp = tempfile::TempDir::new().unwrap();
        let ignore_path = tmp.path().join("config/git/ignore");

        ensure_global_gitignore_at(&ignore_path).unwrap();

        let content = std::fs::read_to_string(&ignore_path).unwrap();
        assert!(content.contains(GLOBAL_GITIGNORE_PATTERN));
    }

    #[test]
    fn global_gitignore_does_not_duplicate_pattern() {
        let tmp = tempfile::TempDir::new().unwrap();
        let ignore_path = tmp.path().join("config/git/ignore");

        ensure_global_gitignore_at(&ignore_path).unwrap();
        ensure_global_gitignore_at(&ignore_path).unwrap();

        let content = std::fs::read_to_string(&ignore_path).unwrap();
        let count = content.lines().filter(|l| l.trim() == GLOBAL_GITIGNORE_PATTERN).count();
        assert_eq!(count, 1);
    }

    #[test]
    fn global_gitignore_appends_to_existing_content() {
        let tmp = tempfile::TempDir::new().unwrap();
        let ignore_path = tmp.path().join("config/git/ignore");
        std::fs::create_dir_all(ignore_path.parent().unwrap()).unwrap();
        std::fs::write(&ignore_path, "*.log\n").unwrap();

        ensure_global_gitignore_at(&ignore_path).unwrap();

        let content = std::fs::read_to_string(&ignore_path).unwrap();
        assert!(content.starts_with("*.log\n"));
        assert!(content.contains(GLOBAL_GITIGNORE_PATTERN));
    }

    #[test]
    fn global_gitignore_handles_file_without_trailing_newline() {
        let tmp = tempfile::TempDir::new().unwrap();
        let ignore_path = tmp.path().join("config/git/ignore");
        std::fs::create_dir_all(ignore_path.parent().unwrap()).unwrap();
        std::fs::write(&ignore_path, "*.log").unwrap();

        ensure_global_gitignore_at(&ignore_path).unwrap();

        let content = std::fs::read_to_string(&ignore_path).unwrap();
        // Pattern should be on its own line, not appended to "*.log".
        assert!(content.contains(&format!("\n{GLOBAL_GITIGNORE_PATTERN}")));
    }

    #[test]
    fn session_save_atomic_write() {
        let tmp = tempfile::TempDir::new().unwrap();
        let session = Session {
            version: 1,
            buffers: vec![BufferSession {
                path: PathBuf::from("/tmp/file.rs"),
                cursor_row: 1,
                cursor_col: 2,
                scroll_row: 0,
                scroll_col: 0,
            }],
            active_buffer: 0,
            tree: TreeSession {
                expanded_paths: Vec::new(),
                selected_path: None,
                scroll: 0,
            },
            layout: LayoutSession {
                tree_width_pct: 20,
                editor_height_pct: 70,
                show_tree: true,
                show_terminal: true,
            },
            focus: "Editor".to_string(),
        };

        session.save(tmp.path()).unwrap();

        // Verify the file contains valid JSON.
        let content =
            std::fs::read_to_string(tmp.path().join(SESSION_DIR).join(SESSION_FILE)).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&content).unwrap();
        assert_eq!(parsed["version"], 1);

        // Temp file should not remain.
        assert!(!tmp
            .path()
            .join(SESSION_DIR)
            .join(format!("{SESSION_FILE}.tmp"))
            .exists());
    }

    #[test]
    fn session_load_returns_none_when_missing() {
        let tmp = tempfile::TempDir::new().unwrap();
        let result = Session::load(tmp.path()).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn session_load_returns_session_when_exists() {
        let tmp = tempfile::TempDir::new().unwrap();
        let session = Session {
            version: 1,
            buffers: vec![BufferSession {
                path: PathBuf::from("/tmp/file.rs"),
                cursor_row: 10,
                cursor_col: 5,
                scroll_row: 3,
                scroll_col: 0,
            }],
            active_buffer: 0,
            tree: TreeSession {
                expanded_paths: Vec::new(),
                selected_path: None,
                scroll: 0,
            },
            layout: LayoutSession {
                tree_width_pct: 25,
                editor_height_pct: 65,
                show_tree: true,
                show_terminal: false,
            },
            focus: "Tree".to_string(),
        };

        session.save(tmp.path()).unwrap();
        let loaded = Session::load(tmp.path()).unwrap().unwrap();
        assert_eq!(session, loaded);
    }

    #[test]
    fn session_apply_opens_buffers() {
        let tmp = tempfile::TempDir::new().unwrap();
        let root = tmp.path().to_path_buf();
        let file1 = root.join("file1.txt");
        let file2 = root.join("file2.txt");
        std::fs::write(&file1, "content1").unwrap();
        std::fs::write(&file2, "content2").unwrap();

        let mut app = AppState::new_with_root(root);

        let session = Session {
            version: 1,
            buffers: vec![
                BufferSession {
                    path: file1,
                    cursor_row: 0,
                    cursor_col: 0,
                    scroll_row: 0,
                    scroll_col: 0,
                },
                BufferSession {
                    path: file2,
                    cursor_row: 0,
                    cursor_col: 0,
                    scroll_row: 0,
                    scroll_col: 0,
                },
            ],
            active_buffer: 1,
            tree: TreeSession {
                expanded_paths: Vec::new(),
                selected_path: None,
                scroll: 0,
            },
            layout: LayoutSession {
                tree_width_pct: 20,
                editor_height_pct: 70,
                show_tree: true,
                show_terminal: true,
            },
            focus: "Editor".to_string(),
        };

        let warnings = session.apply(&mut app);
        assert!(warnings.is_empty());
        assert_eq!(app.buffer_manager.buffer_count(), 2);
        assert_eq!(app.buffer_manager.active_index(), 1);
    }

    #[test]
    fn session_apply_skips_missing_files() {
        let tmp = tempfile::TempDir::new().unwrap();
        let root = tmp.path().to_path_buf();
        let existing = root.join("exists.txt");
        std::fs::write(&existing, "content").unwrap();

        let mut app = AppState::new_with_root(root);

        let session = Session {
            version: 1,
            buffers: vec![
                BufferSession {
                    path: PathBuf::from("/nonexistent/file.rs"),
                    cursor_row: 0,
                    cursor_col: 0,
                    scroll_row: 0,
                    scroll_col: 0,
                },
                BufferSession {
                    path: existing,
                    cursor_row: 0,
                    cursor_col: 0,
                    scroll_row: 0,
                    scroll_col: 0,
                },
            ],
            active_buffer: 0,
            tree: TreeSession {
                expanded_paths: Vec::new(),
                selected_path: None,
                scroll: 0,
            },
            layout: LayoutSession {
                tree_width_pct: 20,
                editor_height_pct: 70,
                show_tree: true,
                show_terminal: true,
            },
            focus: "Editor".to_string(),
        };

        let warnings = session.apply(&mut app);
        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].contains("no longer exists"));
        assert_eq!(app.buffer_manager.buffer_count(), 1);
    }

    #[test]
    fn session_apply_restores_layout() {
        let tmp = tempfile::TempDir::new().unwrap();
        let mut app = AppState::new_with_root(tmp.path().to_path_buf());

        let session = Session {
            version: 1,
            buffers: Vec::new(),
            active_buffer: 0,
            tree: TreeSession {
                expanded_paths: Vec::new(),
                selected_path: None,
                scroll: 0,
            },
            layout: LayoutSession {
                tree_width_pct: 30,
                editor_height_pct: 60,
                show_tree: false,
                show_terminal: false,
            },
            focus: "Terminal(0)".to_string(),
        };

        let warnings = session.apply(&mut app);
        assert!(warnings.is_empty());
        assert_eq!(app.tree_width_pct, 30);
        assert_eq!(app.editor_height_pct, 60);
        assert!(!app.show_tree);
        assert!(!app.show_terminal);
        assert_eq!(app.focus, FocusTarget::Terminal(0));
    }

    #[test]
    fn session_apply_restores_cursor_positions() {
        let tmp = tempfile::TempDir::new().unwrap();
        let root = tmp.path().to_path_buf();
        let file_path = root.join("test.txt");
        std::fs::write(&file_path, "line1\nline2\nline3\nline4\nline5").unwrap();

        let mut app = AppState::new_with_root(root);

        let session = Session {
            version: 1,
            buffers: vec![BufferSession {
                path: file_path,
                cursor_row: 3,
                cursor_col: 2,
                scroll_row: 1,
                scroll_col: 0,
            }],
            active_buffer: 0,
            tree: TreeSession {
                expanded_paths: Vec::new(),
                selected_path: None,
                scroll: 0,
            },
            layout: LayoutSession {
                tree_width_pct: 20,
                editor_height_pct: 70,
                show_tree: true,
                show_terminal: true,
            },
            focus: "Editor".to_string(),
        };

        let warnings = session.apply(&mut app);
        assert!(warnings.is_empty());

        let buf = app.buffer_manager.active_buffer().unwrap();
        assert_eq!(buf.cursor.row, 3);
        assert_eq!(buf.cursor.col, 2);
        assert_eq!(buf.scroll_row, 1);
    }

    #[test]
    fn session_from_app_captures_focus() {
        let tmp = tempfile::TempDir::new().unwrap();
        let mut app = AppState::new_with_root(tmp.path().to_path_buf());
        app.focus = FocusTarget::Editor;

        let session = Session::from_app(&app);
        assert_eq!(session.focus, "Editor");

        app.focus = FocusTarget::Terminal(2);
        let session = Session::from_app(&app);
        assert_eq!(session.focus, "Terminal(2)");
    }
}
