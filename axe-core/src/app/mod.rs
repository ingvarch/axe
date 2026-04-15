mod ai_overlay;
mod clipboard;
mod diff_popup;
mod editor;
pub mod editor_layout;
mod execute;
mod git;
mod input;
mod layout;
mod lsp;
mod terminal;
mod tree;
mod types;

pub use editor_layout::{EditorLayout, Split, SplitError, SplitOrientation};
pub use types::*;

// Re-export free functions from submodules for test access.
#[cfg(test)]
pub(crate) use lsp::convert_lsp_diagnostics;

use std::path::PathBuf;
use std::time::{Duration, Instant};

use crate::keymap::KeymapResolver;

use layout::DEFAULT_EDITOR_HEIGHT_PCT;
use layout::DEFAULT_TREE_WIDTH_PCT;
use types::DEFAULT_TERMINAL_COLS;
use types::DEFAULT_TERMINAL_ROWS;

/// How long a status message remains visible.
const STATUS_MESSAGE_DURATION: Duration = Duration::from_secs(3);

/// Central application state shared across all subsystems.
pub struct AppState {
    pub should_quit: bool,
    pub focus: FocusTarget,
    pub show_tree: bool,
    pub show_terminal: bool,
    pub show_help: bool,
    /// Active confirmation dialog, if any.
    pub confirm_dialog: Option<ConfirmDialog>,
    /// Active diff hunk popup, if any.
    pub diff_popup: Option<DiffPopup>,
    pub resize_mode: ResizeModeState,
    pub mouse_drag: MouseDragState,
    /// Which panel is currently zoomed to full screen, if any.
    pub zoomed_panel: Option<FocusTarget>,
    pub tree_width_pct: u16,
    pub editor_height_pct: u16,
    /// File tree for the project directory, if loaded.
    pub file_tree: Option<axe_tree::FileTree>,
    /// Terminal emulator manager, if initialized.
    pub terminal_manager: Option<axe_terminal::TerminalManager>,
    /// Project root directory for spawning new terminals.
    pub project_root: Option<PathBuf>,
    /// Last known terminal panel width in columns.
    pub last_terminal_cols: u16,
    /// Last known terminal panel height in rows.
    pub last_terminal_rows: u16,
    /// Editor buffer manager holding all open file buffers.
    pub buffer_manager: axe_editor::BufferManager,
    /// Terminal grid area in screen coordinates (x, y, width, height).
    ///
    /// Updated each frame from `terminal_inner_rect()` in main.rs.
    /// Used for converting mouse screen coordinates to grid coordinates.
    pub terminal_grid_area: Option<(u16, u16, u16, u16)>,
    /// Temporary status message shown in the status bar.
    ///
    /// Cleared automatically after `STATUS_MESSAGE_DURATION` elapses.
    pub status_message: Option<(String, Instant)>,
    /// Tree panel inner area in screen coordinates (x, y, width, height).
    ///
    /// Updated each frame from `tree_inner_rect()` in main.rs.
    /// Used for converting mouse screen coordinates to tree node indices.
    pub tree_inner_area: Option<(u16, u16, u16, u16)>,
    /// Editor content area in screen coordinates (x, y, width, height).
    ///
    /// Updated each frame from `editor_inner_rect()` in main.rs.
    /// Used for viewport calculations and mouse coordinate conversion.
    pub editor_inner_area: Option<(u16, u16, u16, u16)>,
    /// Editor tab bar area in screen coordinates (x, y, width, height).
    ///
    /// Set each frame by the UI when tab bar is visible.
    /// Used for detecting mouse clicks on editor buffer tabs.
    pub editor_tab_bar_area: Option<(u16, u16, u16, u16)>,
    /// Terminal tab bar area in screen coordinates (x, y, width, height).
    ///
    /// Set each frame by the UI when terminal tab bar is visible.
    /// Used for detecting mouse clicks on terminal tabs.
    pub terminal_tab_bar_area: Option<(u16, u16, u16, u16)>,
    /// Timestamp of the last edit operation, used for autosave debouncing.
    pub last_edit_time: Option<Instant>,
    /// System clipboard for copy/paste operations.
    ///
    /// Lazily initialized on first use. `None` if clipboard access fails
    /// (e.g., headless/SSH environment).
    clipboard: Option<arboard::Clipboard>,
    /// Whether an editor text selection drag is currently in progress.
    editor_selecting: bool,
    /// Active search state, if the search bar is open.
    pub search: Option<crate::search::SearchState>,
    /// Active file finder overlay state, if open.
    pub file_finder: Option<crate::file_finder::FileFinder>,
    /// Active command palette overlay state, if open.
    pub command_palette: Option<crate::command_palette::CommandPalette>,
    /// Active project-wide search overlay state, if open.
    pub project_search: Option<crate::project_search::ProjectSearch>,
    /// Active SSH host finder overlay state, if open.
    pub ssh_host_finder: Option<crate::ssh_host_finder::SshHostFinder>,
    /// Last tree click time and node index, for double-click detection.
    last_tree_click: Option<(Instant, usize)>,
    /// Whether an editor scrollbar drag is currently in progress.
    scrollbar_dragging: bool,
    /// Editor scrollbar area in screen coordinates (x, y, width, height).
    ///
    /// Updated each frame from `editor_scrollbar_rect()` in main.rs.
    /// Used for detecting mouse clicks on the editor scrollbar.
    pub editor_scrollbar_area: Option<(u16, u16, u16, u16)>,
    /// Whether a terminal text selection drag is currently in progress.
    terminal_selecting: bool,
    /// Screen position where the last mouse-down occurred in the terminal grid.
    /// Used to distinguish clicks (no movement) from drags.
    terminal_select_start: Option<(u16, u16)>,
    /// Multi-click state for the editor panel.
    editor_click_state: ClickState,
    /// Multi-click state for the terminal panel.
    terminal_click_state: ClickState,
    /// Inner rectangle of the AI overlay's PTY grid, in screen coordinates.
    /// Updated each frame by `render_ai_overlay` while the overlay is visible.
    /// Used by mouse hit-testing to convert clicks into grid points.
    pub ai_overlay_grid_area: Option<(u16, u16, u16, u16)>,
    /// Whether an AI-overlay text selection drag is currently in progress.
    pub(crate) ai_overlay_selecting: bool,
    /// Screen position of the last mouse-down inside the AI overlay grid.
    /// Used to distinguish pure clicks from actual drags on `Up`.
    pub(crate) ai_overlay_select_start: Option<(u16, u16)>,
    /// Multi-click state for the AI overlay (single → simple, double →
    /// semantic, triple → line selection).
    pub(crate) ai_overlay_click_state: ClickState,
    keymap: KeymapResolver,
    /// Application configuration loaded from TOML files.
    pub config: axe_config::AppConfig,
    /// LSP manager for language server communication, if initialized.
    pub lsp_manager: Option<axe_lsp::LspManager>,
    /// Active completion popup state, if open.
    pub completion: Option<crate::completion::CompletionState>,
    /// Active location list overlay (definition/references results).
    pub location_list: Option<crate::location_list::LocationList>,
    /// Active Go to Line dialog, if open.
    pub go_to_line: Option<GoToLineDialog>,
    /// Active SSH password dialog, if open.
    pub password_dialog: Option<PasswordDialog>,
    /// Active hover tooltip, if showing.
    pub hover_info: Option<crate::hover::HoverInfo>,
    /// Active LSP signature help popup, if open.
    pub signature_help: Option<crate::signature_help::SignatureHelpState>,
    /// Active inline rename dialog, if open.
    pub rename: Option<crate::rename::RenameState>,
    /// Active code actions picker, if open.
    pub code_actions: Option<crate::code_actions::CodeActionsState>,
    /// Inlay hints indexed by buffer path, refreshed as the buffer changes.
    pub inlay_hints: crate::inlay::InlayHintStore,
    /// Monotonic content versions for open buffers — bumped on every edit.
    /// Used to discard stale `textDocument/inlayHint` responses.
    pub buffer_content_versions: std::collections::HashMap<PathBuf, u64>,
    /// Mouse hover state for delay-triggered hover: (timestamp, buffer_row, buffer_col).
    hover_mouse_state: Option<(Instant, usize, usize)>,
    /// Whether a format-on-save operation is pending (waiting for LSP formatting response).
    pending_format_save: bool,
    /// Full build version string (e.g. "v0.1.0-abc123"), set by the binary crate.
    pub build_version: String,
    /// Filesystem watcher for detecting external file changes (create, delete, rename).
    file_watcher: Option<axe_tree::FileWatcher>,
    /// Current git branch name (e.g. "main") or short commit hash for detached HEAD.
    pub git_branch: Option<String>,
    /// Timestamp of last git branch check, for periodic refresh.
    last_git_branch_check: Option<Instant>,
    /// Set of absolute file paths with uncommitted changes (modified, new, deleted).
    pub git_modified_files: std::collections::HashSet<std::path::PathBuf>,
    /// Set of absolute directory paths that transitively contain modified files.
    pub git_dirty_dirs: std::collections::HashSet<std::path::PathBuf>,
    /// AI chat overlay state — always present (not `Option`) so the overlay
    /// can be hidden while its PTY session keeps running.
    pub ai_overlay: crate::ai_overlay::AiOverlay,
    /// Override for the global AI config path. Normally `None` (so the real
    /// user config is used). Tests set this to a tempfile to keep side effects
    /// out of the user's actual `~/.config/axe/config.toml`.
    pub(crate) ai_config_path_override: Option<PathBuf>,
    /// When true, the next frame must call `terminal.clear()` before drawing
    /// to force ratatui to do a full redraw instead of a diff against stale geometry.
    /// Set on resize events, panel toggles, zoom changes, and border drag end.
    pub needs_full_redraw: bool,
    /// Set to `true` by `poll_terminal()` when PTY output is received.
    ///
    /// The main loop reads and clears this flag after each draw. When set,
    /// it "poisons" ratatui's front buffer so the next frame's diff resends
    /// all cells, catching any updates the real terminal missed.
    pub terminal_output_this_frame: bool,
    /// Editor split layout. Always contains at least one split. The
    /// focused split's `active_buffer` is kept in sync with
    /// `buffer_manager.active` via `AppState::set_focused_split`.
    pub editor_layout: editor_layout::EditorLayout,
    /// Per-split screen rectangles updated each frame by the renderer.
    /// Used by mouse click handling to resolve which split was clicked.
    pub split_areas: Vec<(u16, u16, u16, u16)>,
    /// First key of a pending keyboard chord (e.g. `Ctrl+K`), waiting for
    /// its continuation. Cleared after the next key press resolves it.
    pub pending_chord: Option<crossterm::event::KeyEvent>,
}

impl AppState {
    /// Creates a new `AppState` with default values and no file tree.
    pub fn new() -> Self {
        Self {
            should_quit: false,
            focus: FocusTarget::default(),
            show_tree: true,
            show_terminal: true,
            show_help: false,
            confirm_dialog: None,
            diff_popup: None,
            resize_mode: ResizeModeState::default(),
            mouse_drag: MouseDragState::default(),
            zoomed_panel: None,
            tree_width_pct: DEFAULT_TREE_WIDTH_PCT,
            editor_height_pct: DEFAULT_EDITOR_HEIGHT_PCT,
            file_tree: None,
            buffer_manager: axe_editor::BufferManager::new(),
            terminal_manager: None,
            project_root: None,
            last_terminal_cols: DEFAULT_TERMINAL_COLS,
            last_terminal_rows: DEFAULT_TERMINAL_ROWS,
            terminal_grid_area: None,
            status_message: None,
            tree_inner_area: None,
            editor_inner_area: None,
            editor_tab_bar_area: None,
            terminal_tab_bar_area: None,
            last_edit_time: None,
            clipboard: None,
            search: None,
            file_finder: None,
            command_palette: None,
            project_search: None,
            ssh_host_finder: None,
            go_to_line: None,
            password_dialog: None,
            editor_selecting: false,
            scrollbar_dragging: false,
            editor_scrollbar_area: None,
            last_tree_click: None,
            terminal_selecting: false,
            terminal_select_start: None,
            editor_click_state: ClickState::default(),
            terminal_click_state: ClickState::default(),
            ai_overlay_grid_area: None,
            ai_overlay_selecting: false,
            ai_overlay_select_start: None,
            ai_overlay_click_state: ClickState::default(),
            keymap: KeymapResolver::with_defaults(),
            config: axe_config::AppConfig::default(),
            lsp_manager: None,
            completion: None,
            location_list: None,
            hover_info: None,
            signature_help: None,
            rename: None,
            code_actions: None,
            inlay_hints: crate::inlay::InlayHintStore::new(),
            buffer_content_versions: std::collections::HashMap::new(),
            hover_mouse_state: None,
            pending_format_save: false,
            build_version: String::new(),
            git_branch: None,
            last_git_branch_check: None,
            file_watcher: None,
            git_modified_files: std::collections::HashSet::new(),
            git_dirty_dirs: std::collections::HashSet::new(),
            ai_overlay: crate::ai_overlay::AiOverlay::new(),
            ai_config_path_override: None,
            needs_full_redraw: true,
            editor_layout: editor_layout::EditorLayout::single(0),
            split_areas: Vec::new(),
            pending_chord: None,
            terminal_output_this_frame: false,
        }
    }

    /// Creates a new `AppState` with a file tree loaded from the given root directory.
    ///
    /// If the directory cannot be read, logs a warning and falls back to no file tree.
    /// Loads configuration from global and project-level config files and applies
    /// tree settings (show_icons, show_hidden) to the file tree.
    pub fn new_with_root(root: PathBuf) -> Self {
        let (config, mut warnings) = axe_config::AppConfig::load_with_warnings(Some(&root));
        let file_tree = match axe_tree::FileTree::new(root.clone()) {
            Ok(mut tree) => {
                tree.set_show_icons(config.tree.show_icons);
                tree.set_show_ignored(config.tree.show_hidden);
                Some(tree)
            }
            Err(e) => {
                log::warn!("Failed to load file tree: {e}");
                None
            }
        };
        let buffer_manager = axe_editor::BufferManager::with_editor_config(
            config.editor.tab_size,
            config.editor.insert_spaces,
        );
        let mut keymap = KeymapResolver::with_defaults();
        let keybinding_warnings = keymap.apply_overrides(&config.keybindings);
        warnings.extend(keybinding_warnings);

        // Initialize LSP manager: merge default configs with user overrides.
        let mut lsp_configs = axe_config::default_lsp_configs();
        for (lang, user_cfg) in &config.lsp {
            lsp_configs.insert(lang.clone(), user_cfg.clone());
        }
        let lsp_manager = match axe_lsp::LspManager::new(lsp_configs, &root) {
            Ok(mgr) => Some(mgr),
            Err(e) => {
                log::warn!("Failed to initialize LSP manager: {e}");
                None
            }
        };

        let status_message = if warnings.is_empty() {
            None
        } else {
            let msg = format!("Config: {}", warnings.join("; "));
            log::warn!("{msg}");
            Some((msg, Instant::now()))
        };

        let file_watcher = match axe_tree::FileWatcher::new(&root) {
            Ok(w) => Some(w),
            Err(e) => {
                log::warn!("Failed to create filesystem watcher: {e}");
                None
            }
        };

        let git_branch = crate::git::current_branch(&root);
        let git_modified_files = crate::git::modified_files(&root);
        let git_dirty_dirs = crate::git::dirty_parent_dirs(&git_modified_files, &root);

        Self {
            file_tree,
            file_watcher,
            project_root: Some(root),
            buffer_manager,
            config,
            keymap,
            status_message,
            lsp_manager,
            git_branch,
            last_git_branch_check: Some(Instant::now()),
            git_modified_files,
            git_dirty_dirs,
            ..Self::new()
        }
    }

    /// Signals the application to exit the event loop.
    pub fn quit(&mut self) {
        self.confirm_dialog = None;
        self.should_quit = true;
    }

    /// Sets the focused editor split and syncs `buffer_manager.active` to
    /// match the split's current buffer.
    ///
    /// This is the **only** approved way to change `editor_layout.focused`
    /// — all other call sites must go through it so `BufferManager.active`
    /// never drifts out of sync with the layout.
    pub fn set_focused_split(&mut self, idx: usize) {
        self.editor_layout.set_focused(idx);
        if let Some(buffer_idx) = self.editor_layout.focused().active_buffer() {
            if buffer_idx < self.buffer_manager.buffer_count() {
                self.buffer_manager.set_active(buffer_idx);
            }
        }
    }

    /// Records the focused split's active buffer after a direct mutation
    /// of `buffer_manager` (e.g. opening a new file). Keeps the split in
    /// sync with the live buffer index — the focused split adopts the
    /// currently active global buffer as a new tab (or reuses an
    /// existing tab if it already has one).
    pub fn sync_focused_split_to_active_buffer(&mut self) {
        let active = self.buffer_manager.active_index();
        if self.buffer_manager.buffer_count() == 0 {
            return;
        }
        self.editor_layout.open_in_focused(active);
    }

    /// Closes the entire focused split, including every tab it holds.
    ///
    /// Used by `Ctrl+W` when the layout has more than one split so the
    /// user can quickly dismiss a split without per-tab cycling. Each
    /// closed tab is also removed from `BufferManager` if no other
    /// split still references it, matching the single-tab close flow.
    pub fn close_focused_split_entirely(&mut self) {
        if self.editor_layout.len() <= 1 {
            return;
        }
        // Close every tab in the focused split one by one so we reuse
        // the existing per-tab cleanup (buffer removal, index shifting,
        // push-sync). `close_focused_tab` auto-drops the split when it
        // empties, so the loop naturally terminates.
        while self.editor_layout.len() > 1 && !self.editor_layout.focused().buffers.is_empty() {
            let (removed, split_was_closed) = self.editor_layout.close_focused_tab();
            if let Some(removed_idx) = removed {
                if !self.editor_layout.any_split_references(removed_idx) {
                    self.buffer_manager.close_buffer(removed_idx);
                    self.editor_layout
                        .shift_buffer_indices_after_removal(removed_idx);
                }
            }
            if split_was_closed {
                break;
            }
        }
        let idx = self.editor_layout.focused_index();
        self.set_focused_split(idx);
        self.needs_full_redraw = true;
    }

    /// Closes the focused split's currently active tab.
    ///
    /// If the tab was the split's only buffer, the split itself is
    /// removed too (unless it's the last remaining split — in that case
    /// we keep an empty split so the UI still has something to render).
    /// When no other split references the removed buffer, it is also
    /// closed from the global `BufferManager` and remaining split indices
    /// are shifted down to compensate.
    pub fn close_active_tab_in_focused_split(&mut self) {
        let (removed, _split_was_closed) = self.editor_layout.close_focused_tab();
        let Some(removed_idx) = removed else {
            return;
        };
        // If no split still references the buffer, remove it from the
        // global buffer manager and shift every remaining split index.
        if !self.editor_layout.any_split_references(removed_idx) {
            self.buffer_manager.close_buffer(removed_idx);
            self.editor_layout
                .shift_buffer_indices_after_removal(removed_idx);
        }
        // Push-sync: make the now-focused split's active tab the live
        // buffer_manager active.
        let idx = self.editor_layout.focused_index();
        self.set_focused_split(idx);
    }

    /// Sets a temporary status message that appears in the status bar.
    pub fn set_status_message(&mut self, msg: String) {
        self.status_message = Some((msg, Instant::now()));
    }

    /// Clears the status message if it has expired.
    pub fn expire_status_message(&mut self) {
        if let Some((_, created)) = &self.status_message {
            if created.elapsed() >= STATUS_MESSAGE_DURATION {
                self.status_message = None;
            }
        }
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests;
