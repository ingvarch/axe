use std::io::{self, stdout};
use std::panic;

use clap::Parser;
use crossterm::event::{self, Event, KeyEventKind};
use crossterm::terminal::{
    self, EnterAlternateScreen, LeaveAlternateScreen,
};
use crossterm::ExecutableCommand;
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;

use axe_core::AppState;

#[derive(Parser)]
#[command(name = "axe", version = axe_core::version(), about = "Axe IDE")]
struct Cli {}

type Term = Terminal<CrosstermBackend<io::Stdout>>;

fn setup_terminal() -> io::Result<Term> {
    terminal::enable_raw_mode()?;
    stdout().execute(EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout());
    Terminal::new(backend)
}

fn restore_terminal() {
    let _ = terminal::disable_raw_mode();
    let _ = stdout().execute(LeaveAlternateScreen);
}

fn install_panic_hook() {
    let original = panic::take_hook();
    panic::set_hook(Box::new(move |info| {
        restore_terminal();
        original(info);
    }));
}

#[tokio::main]
async fn main() -> io::Result<()> {
    Cli::parse();

    install_panic_hook();

    let mut terminal = setup_terminal()?;
    let mut app = AppState::new();

    while !app.should_quit {
        terminal.draw(|frame| axe_ui::render(&app, frame))?;

        if event::poll(std::time::Duration::from_millis(50))? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    app.handle_key_event(key);
                }
            }
        }
    }

    restore_terminal();
    Ok(())
}
