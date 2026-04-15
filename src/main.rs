use std::fs::File;
use std::io::{self, stdout};
use std::panic;
use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::Parser;
use crossterm::event::{
    self, DisableMouseCapture, EnableMouseCapture, Event, KeyEventKind, KeyboardEnhancementFlags,
    MouseEventKind, PopKeyboardEnhancementFlags, PushKeyboardEnhancementFlags,
};
use crossterm::terminal::{
    self, BeginSynchronizedUpdate, EndSynchronizedUpdate, EnterAlternateScreen,
    LeaveAlternateScreen,
};
use crossterm::ExecutableCommand;
use ratatui::backend::CrosstermBackend;
use ratatui::layout::Rect;
use ratatui::Terminal;
use simplelog::{CombinedLogger, Config as LogConfig, LevelFilter, WriteLogger};

use axe_config::theme::load_theme;
use axe_core::AppState;
use axe_ui::theme::Theme;

/// Full build version string injected by `build.rs` (e.g. "v0.1.0-abc123").
const BUILD_VERSION: &str = env!("AXE_BUILD_VERSION");

/// Initial terminal size used before the first frame is rendered.
const INITIAL_TERM_COLS: u16 = 80;
/// Initial terminal rows for the PTY.
const INITIAL_TERM_ROWS: u16 = 24;

#[derive(Parser)]
#[command(name = "axe", version = BUILD_VERSION, about = "Axe IDE")]
struct Cli {
    /// Directory or file to open (defaults to current directory).
    /// When a file is given, its enclosing git repo (or parent dir) becomes
    /// the workspace root and the file is opened in the editor.
    #[arg(default_value = ".")]
    path: PathBuf,

    /// Skip session restore on startup
    #[arg(long)]
    no_session: bool,
}

type Term = Terminal<CrosstermBackend<io::Stdout>>;

/// Resolves the CLI path argument into a workspace root and an optional
/// file to auto-open.
///
/// - Directory path -> `(canonical_dir, None)`.
/// - File path -> walks ancestors looking for a `.git` directory; if found
///   returns that ancestor as root, otherwise the file's parent directory.
///   The second tuple element is the canonical file path to open.
/// - Nonexistent path -> `(original_path, None)`, preserving the previous
///   fallback so `AppState::new_with_root` can handle the failure.
fn resolve_cli_target(path: PathBuf) -> (PathBuf, Option<PathBuf>) {
    let canonical = match path.canonicalize() {
        Ok(p) => p,
        Err(_) => return (path, None),
    };

    if canonical.is_dir() {
        return (canonical, None);
    }

    if !canonical.is_file() {
        // Exotic filesystem entry (symlink loop, socket, etc.) — fall through.
        return (canonical, None);
    }

    let parent = canonical
        .parent()
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));

    let root = parent
        .ancestors()
        .find(|ancestor| ancestor.join(".git").exists())
        .map(PathBuf::from)
        .unwrap_or(parent);

    (root, Some(canonical))
}

fn setup_terminal() -> Result<Term> {
    terminal::enable_raw_mode()?;
    stdout().execute(EnterAlternateScreen)?;
    stdout().execute(EnableMouseCapture)?;
    // Enable Kitty keyboard protocol for terminals that support it.
    // This allows correct reporting of Ctrl+Shift+<key> combos.
    if terminal::supports_keyboard_enhancement().unwrap_or(false) {
        stdout().execute(PushKeyboardEnhancementFlags(
            KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES,
        ))?;
    }
    let backend = CrosstermBackend::new(stdout());
    Ok(Terminal::new(backend)?)
}

fn restore_terminal() {
    // Pop keyboard enhancement if it was pushed (safe to call even if not pushed).
    let _ = stdout().execute(PopKeyboardEnhancementFlags);
    if let Err(e) = stdout().execute(DisableMouseCapture) {
        eprintln!("Failed to disable mouse capture: {e}");
    }
    if let Err(e) = terminal::disable_raw_mode() {
        eprintln!("Failed to disable raw mode: {e}");
    }
    if let Err(e) = stdout().execute(LeaveAlternateScreen) {
        eprintln!("Failed to leave alternate screen: {e}");
    }
}

fn install_panic_hook() {
    let original = panic::take_hook();
    panic::set_hook(Box::new(move |info| {
        restore_terminal();
        original(info);
    }));
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    // Initialize file-based logger. Writes to /tmp/axe.log at debug level.
    if let Ok(log_file) = File::create("/tmp/axe.log") {
        let _ = CombinedLogger::init(vec![WriteLogger::new(
            LevelFilter::Debug,
            LogConfig::default(),
            log_file,
        )]);
    }

    install_panic_hook();

    let mut terminal = setup_terminal()?;
    let (root, file_to_open) = resolve_cli_target(cli.path);

    let mut app = AppState::new_with_root(root.clone());
    app.build_version = BUILD_VERSION.to_string();

    // Restore session from previous run (unless --no-session).
    if !cli.no_session {
        if let Ok(Some(session)) = axe_core::session::Session::load(&root) {
            let warnings = session.apply(&mut app);
            for msg in warnings {
                log::warn!("Session restore: {msg}");
            }
        }
    }

    // If the CLI arg was a file path, open it on top of whatever the session
    // restored so the requested file becomes the focused buffer.
    if let Some(file) = file_to_open {
        app.execute(axe_core::Command::OpenFile(file));
    }

    // Persist initial session state so .axe/ directory and global gitignore
    // are created immediately, not only on exit.
    if let Err(e) = axe_core::session::Session::from_app(&app).save(&root) {
        log::warn!("Failed to save initial session: {e}");
    }

    // Build theme from config — resolved once at startup.
    let theme = load_theme(&app.config.ui.theme)
        .map(|tf| Theme::from_theme_file(&tf))
        .unwrap_or_default();

    // Initialize terminal emulator, using shell from config if set.
    let shell_override = if app.config.terminal.shell.is_empty() {
        None
    } else {
        Some(app.config.terminal.shell.as_str())
    };
    let mut mgr = axe_terminal::TerminalManager::new();
    mgr.spawn_default_tab_with_shell(INITIAL_TERM_COLS, INITIAL_TERM_ROWS, &root, shell_override)
        .context("Failed to spawn terminal")?;
    app.terminal_manager = Some(mgr);

    // Track terminal panel size to detect resize.
    let mut last_terminal_size: (u16, u16) = (0, 0);

    while !app.should_quit {
        let size = terminal.size()?;

        // Update tree viewport dimensions for scroll calculations.
        if let Some(ref mut tree) = app.file_tree {
            // Inner height: total height minus status bar (1) minus top/bottom borders (2).
            let inner_h = size.height.saturating_sub(3);
            tree.set_viewport_height(inner_h as usize);
            // Inner width: tree panel width minus left/right borders (2), minus scrollbar (1).
            let tree_cols = (u32::from(size.width) * u32::from(app.tree_width_pct) / 100) as u16;
            let inner_w = tree_cols.saturating_sub(3);
            tree.set_viewport_width(inner_w as usize);
        }

        // Poll terminal PTY output before drawing.
        app.poll_terminal();

        // Resize the AI overlay's PTY to match the inner area of the centered
        // modal before draining its output — so a resize never races with a
        // PTY read and the grid always fills the rendered overlay.
        app.ai_overlay.sync_pty_size(size.width, size.height);

        // Drain AI overlay PTY output — similar to poll_terminal but for the
        // single AI session living outside the TerminalManager.
        app.ai_overlay.drain_output();

        // Check if PTY output arrived this frame. If so, we'll poison
        // ratatui's front buffer after draw() so the NEXT frame resends
        // all cells (catching any the real terminal missed).
        // Clear the flag now so it's fresh for the next poll_terminal().
        let poison_buffer = app.terminal_output_this_frame;
        app.terminal_output_this_frame = false;

        // Poll filesystem watcher for external file changes.
        app.poll_fs_events();

        // Drain project search results from background thread.
        app.drain_project_search_results();

        // Poll LSP events from language servers.
        app.poll_lsp();

        // Check if autosave should trigger (debounced 2s after last edit).
        app.check_autosave();

        // Check if mouse hover delay has elapsed for LSP hover request.
        app.check_hover_timer();

        // Refresh git branch name periodically.
        app.refresh_git_branch();

        // Clear expired status messages.
        app.expire_status_message();

        // Pre-draw: sync terminal PTY size BEFORE rendering to avoid
        // one-frame lag between resize detection and PTY content reflow.
        let full_area = Rect::new(0, 0, size.width, size.height);
        if let Some(term_rect) = axe_ui::terminal_inner_rect(&app, full_area) {
            let new_size = (term_rect.width, term_rect.height);
            if new_size != last_terminal_size && new_size.0 > 0 && new_size.1 > 0 {
                app.last_terminal_cols = new_size.0;
                app.last_terminal_rows = new_size.1;
                if let Some(ref mut mgr) = app.terminal_manager {
                    if let Err(e) = mgr.resize_all(new_size.0, new_size.1) {
                        log::warn!("Failed to resize terminal tabs: {e}");
                    }
                }
                last_terminal_size = new_size;
            }
        }

        // Wrap draw in synchronized output to prevent tearing/flicker.
        // The terminal buffers all output until EndSynchronizedUpdate, then
        // renders atomically. Unsupported terminals silently ignore the sequences.
        crossterm::execute!(io::stdout(), BeginSynchronizedUpdate)?;

        // Force full redraw on layout/geometry changes (resize, panel toggle, zoom).
        // Inside the synchronized block so the clear + full redraw is atomic.
        if app.needs_full_redraw {
            terminal.clear()?;
            app.needs_full_redraw = false;
        }

        terminal.draw(|frame| axe_ui::render(&mut app, frame, &theme))?;
        crossterm::execute!(io::stdout(), EndSynchronizedUpdate)?;

        // After PTY output, poison ratatui's front buffer in the terminal panel
        // area so the next frame's diff will resend every terminal cell. This
        // catches any cells the real terminal missed during rapid output (e.g.
        // alternate screen exit). Unlike terminal.clear(), this does NOT send
        // ESC[2J and only affects the terminal panel — no flicker, no black
        // patches in the editor or tree panels.
        if poison_buffer {
            if let Some(term_panel) = axe_ui::terminal_outer_rect(&app, full_area) {
                let buf = terminal.current_buffer_mut();
                for pos in term_panel.positions() {
                    buf[pos].set_symbol("\x00");
                }
            }
        }

        // Sync panel dimensions after draw.

        // Update tree inner area for mouse click detection on tree nodes.
        if let Some(tree_rect) = axe_ui::tree_inner_rect(&app, full_area) {
            app.tree_inner_area =
                Some((tree_rect.x, tree_rect.y, tree_rect.width, tree_rect.height));
        } else {
            app.tree_inner_area = None;
        }

        // Update editor tab bar area for mouse click detection on tabs.
        if let Some(tab_rect) = axe_ui::editor_tab_bar_rect(&app, full_area) {
            app.editor_tab_bar_area =
                Some((tab_rect.x, tab_rect.y, tab_rect.width, tab_rect.height));
        } else {
            app.editor_tab_bar_area = None;
        }

        // Update terminal tab bar area for mouse click detection on terminal tabs.
        if let Some(tab_rect) = axe_ui::terminal_tab_bar_rect(&app, full_area) {
            app.terminal_tab_bar_area =
                Some((tab_rect.x, tab_rect.y, tab_rect.width, tab_rect.height));
        } else {
            app.terminal_tab_bar_area = None;
        }

        // Update editor content area for viewport-dependent cursor movement.
        if let Some(editor_rect) = axe_ui::editor_inner_rect(&app, full_area) {
            app.editor_inner_area = Some((
                editor_rect.x,
                editor_rect.y,
                editor_rect.width,
                editor_rect.height,
            ));
            // Set viewport width on active buffer for horizontal scroll clamping.
            if let Some(buf) = app.buffer_manager.active_buffer_mut() {
                buf.set_viewport_width(editor_rect.width as usize);
            }
        } else {
            app.editor_inner_area = None;
        }

        // Update editor scrollbar area for mouse click/drag detection.
        if let Some(sb_rect) = axe_ui::editor_scrollbar_rect(&app, full_area) {
            app.editor_scrollbar_area = Some((sb_rect.x, sb_rect.y, sb_rect.width, sb_rect.height));
        } else {
            app.editor_scrollbar_area = None;
        }

        // Update terminal grid area for mouse coordinate conversion (selection, etc.).
        if let Some(term_rect) = axe_ui::terminal_inner_rect(&app, full_area) {
            app.terminal_grid_area =
                Some((term_rect.x, term_rect.y, term_rect.width, term_rect.height));
        } else {
            app.terminal_grid_area = None;
        }

        // Wait for at least one event, then drain all pending events to prevent
        // input backlog and reduce the chance of split escape sequences.
        if event::poll(std::time::Duration::from_millis(50))? {
            loop {
                match event::read()? {
                    Event::Key(key) if key.kind == KeyEventKind::Press => {
                        app.handle_key_event(key);
                    }
                    Event::Mouse(mouse) => {
                        if mouse.kind == MouseEventKind::Moved {
                            app.handle_mouse_moved(mouse.column, mouse.row);
                        } else {
                            app.handle_mouse_event(mouse, size.width, size.height);
                        }
                    }
                    Event::Resize(_, _) => {
                        app.needs_full_redraw = true;
                    }
                    _ => {}
                }
                if app.should_quit || !event::poll(std::time::Duration::from_millis(0))? {
                    break;
                }
            }
        }
    }

    // Save session state before exit.
    if let Some(ref project_root) = app.project_root {
        if let Err(e) = axe_core::session::Session::from_app(&app).save(project_root) {
            log::warn!("Failed to save session: {e}");
        }
    }

    // Shut down all LSP servers before exit.
    if let Some(ref mut lsp) = app.lsp_manager {
        lsp.shutdown_all();
    }

    restore_terminal();
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn resolve_cli_target_directory_returns_dir_and_none() {
        let dir = tempdir().unwrap();
        let canonical = dir.path().canonicalize().unwrap();

        let (root, file) = resolve_cli_target(dir.path().to_path_buf());

        assert_eq!(root, canonical);
        assert_eq!(file, None);
    }

    #[test]
    fn resolve_cli_target_file_without_git_uses_parent() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("notes.md");
        fs::write(&file_path, "hello").unwrap();
        let canonical_dir = dir.path().canonicalize().unwrap();
        let canonical_file = file_path.canonicalize().unwrap();

        let (root, file) = resolve_cli_target(file_path);

        assert_eq!(root, canonical_dir);
        assert_eq!(file, Some(canonical_file));
    }

    #[test]
    fn resolve_cli_target_file_inside_git_uses_git_root() {
        let repo = tempdir().unwrap();
        fs::create_dir(repo.path().join(".git")).unwrap();
        let nested = repo.path().join("src").join("deep");
        fs::create_dir_all(&nested).unwrap();
        let file_path = nested.join("foo.rs");
        fs::write(&file_path, "fn main() {}").unwrap();
        let canonical_repo = repo.path().canonicalize().unwrap();
        let canonical_file = file_path.canonicalize().unwrap();

        let (root, file) = resolve_cli_target(file_path);

        assert_eq!(root, canonical_repo);
        assert_eq!(file, Some(canonical_file));
    }

    #[test]
    fn resolve_cli_target_nonexistent_path_falls_through() {
        let bogus = PathBuf::from("/definitely/does/not/exist/axe-test-xyz");

        let (root, file) = resolve_cli_target(bogus.clone());

        assert_eq!(root, bogus);
        assert_eq!(file, None);
    }
}
