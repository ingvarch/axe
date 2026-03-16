use std::io::{self, stdout};
use std::panic;
use std::path::PathBuf;

use anyhow::Result;
use clap::Parser;
use crossterm::event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyEventKind};
use crossterm::terminal::{self, EnterAlternateScreen, LeaveAlternateScreen};
use crossterm::ExecutableCommand;
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;

use axe_core::AppState;

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
    let mut app = AppState::new_with_root(root);

    while !app.should_quit {
        let size = terminal.size()?;

        // Update tree viewport height for scroll calculations.
        if let Some(ref mut tree) = app.file_tree {
            // Inner height: total height minus status bar (1) minus top/bottom borders (2).
            let inner_h = size.height.saturating_sub(3);
            tree.set_viewport_height(inner_h as usize);
        }

        terminal.draw(|frame| axe_ui::render(&app, frame))?;

        if event::poll(std::time::Duration::from_millis(50))? {
            match event::read()? {
                Event::Key(key) if key.kind == KeyEventKind::Press => {
                    app.handle_key_event(key);
                }
                Event::Mouse(mouse) => {
                    app.handle_mouse_event(mouse, size.width, size.height);
                }
                _ => {}
            }
        }
    }

    restore_terminal();
    Ok(())
}
