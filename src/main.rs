use std::io::{self, stdout};
use std::panic;
use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::Parser;
use crossterm::event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyEventKind};
use crossterm::terminal::{self, EnterAlternateScreen, LeaveAlternateScreen};
use crossterm::ExecutableCommand;
use ratatui::backend::CrosstermBackend;
use ratatui::layout::Rect;
use ratatui::Terminal;

use axe_core::AppState;

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
    let backend = CrosstermBackend::new(stdout());
    Ok(Terminal::new(backend)?)
}

fn restore_terminal() {
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

    // Initialize terminal emulator with default shell in the project directory.
    let mut mgr = axe_terminal::TerminalManager::new();
    mgr.spawn_default_tab(INITIAL_TERM_COLS, INITIAL_TERM_ROWS, &root)
        .context("Failed to spawn terminal")?;

    let mut app = AppState::new_with_root(root);
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

        terminal.draw(|frame| axe_ui::render(&app, frame))?;

        // Sync panel dimensions after draw.
        let full_area = Rect::new(0, 0, size.width, size.height);

        // Update editor content area for viewport-dependent cursor movement.
        if let Some(editor_rect) = axe_ui::editor_inner_rect(&app, full_area) {
            app.editor_inner_area = Some((editor_rect.width, editor_rect.height));
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

    restore_terminal();
    Ok(())
}
