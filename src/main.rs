use std::io::{self, stdout};
use std::panic;
use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::Parser;
use crossterm::event::{
    self, DisableMouseCapture, EnableMouseCapture, Event, KeyEventKind, KeyboardEnhancementFlags,
    PopKeyboardEnhancementFlags, PushKeyboardEnhancementFlags,
};
use crossterm::terminal::{
    self, BeginSynchronizedUpdate, EndSynchronizedUpdate, EnterAlternateScreen,
    LeaveAlternateScreen,
};
use crossterm::ExecutableCommand;
use ratatui::backend::CrosstermBackend;
use ratatui::layout::Rect;
use ratatui::Terminal;

use axe_config::theme::load_theme;
use axe_core::AppState;
use axe_ui::theme::Theme;

/// Initial terminal size used before the first frame is rendered.
const INITIAL_TERM_COLS: u16 = 80;
/// Initial terminal rows for the PTY.
const INITIAL_TERM_ROWS: u16 = 24;

#[derive(Parser)]
#[command(name = "axe", version = axe_core::version(), about = "Axe IDE")]
struct Cli {
    /// Directory to open (defaults to current directory)
    #[arg(default_value = ".")]
    path: PathBuf,
}

type Term = Terminal<CrosstermBackend<io::Stdout>>;

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

    install_panic_hook();

    let mut terminal = setup_terminal()?;
    let root = cli.path.canonicalize().unwrap_or(cli.path);

    let mut app = AppState::new_with_root(root.clone());

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

        // Update tree viewport height for scroll calculations.
        if let Some(ref mut tree) = app.file_tree {
            // Inner height: total height minus status bar (1) minus top/bottom borders (2).
            let inner_h = size.height.saturating_sub(3);
            tree.set_viewport_height(inner_h as usize);
        }

        // Poll terminal PTY output before drawing.
        app.poll_terminal();

        // Drain project search results from background thread.
        app.drain_project_search_results();

        // Poll LSP events from language servers.
        app.poll_lsp();

        // Check if autosave should trigger (debounced 2s after last edit).
        app.check_autosave();

        // Clear expired status messages.
        app.expire_status_message();

        // Wrap draw in synchronized output to prevent tearing/flicker.
        // The terminal buffers all output until EndSynchronizedUpdate, then
        // renders atomically. Unsupported terminals silently ignore the sequences.
        crossterm::execute!(io::stdout(), BeginSynchronizedUpdate)?;
        terminal.draw(|frame| axe_ui::render(&app, frame, &theme))?;
        crossterm::execute!(io::stdout(), EndSynchronizedUpdate)?;

        // Sync panel dimensions after draw.
        let full_area = Rect::new(0, 0, size.width, size.height);

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
        } else {
            app.editor_inner_area = None;
        }

        // Sync terminal PTY size with actual panel dimensions after draw.
        if let Some(term_rect) = axe_ui::terminal_inner_rect(&app, full_area) {
            // Store grid area for mouse coordinate conversion (selection, etc.).
            app.terminal_grid_area =
                Some((term_rect.x, term_rect.y, term_rect.width, term_rect.height));

            let new_size = (term_rect.width, term_rect.height);
            if new_size != last_terminal_size && new_size.0 > 0 && new_size.1 > 0 {
                // Update AppState so new terminal tabs use the right size.
                app.last_terminal_cols = new_size.0;
                app.last_terminal_rows = new_size.1;
                if let Some(ref mut mgr) = app.terminal_manager {
                    if let Err(e) = mgr.resize_active(new_size.0, new_size.1) {
                        log::warn!("Failed to resize terminal: {e}");
                    }
                }
                last_terminal_size = new_size;
            }
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
                        app.handle_mouse_event(mouse, size.width, size.height);
                    }
                    _ => {}
                }
                if app.should_quit || !event::poll(std::time::Duration::from_millis(0))? {
                    break;
                }
            }
        }
    }

    // Shut down all LSP servers before exit.
    if let Some(ref mut lsp) = app.lsp_manager {
        lsp.shutdown_all();
    }

    restore_terminal();
    Ok(())
}
