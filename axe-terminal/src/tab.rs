// IMPACT ANALYSIS — tab module
// Parents: TerminalManager creates and owns TerminalTab instances.
// Children: pty::spawn_shell() for PTY creation; Term<AltScreenListener> for VT parsing.
//           The background reader thread reads from the PTY reader.
// Siblings: manager.rs polls output and feeds it to the active tab via process_output().
//           axe-ui reads tab.term() for rendering.
//           Resize events from the main loop call tab.resize().

use std::io::{Read, Write};

use alacritty_terminal::grid::Dimensions;
use alacritty_terminal::term::Config as TermConfig;
use alacritty_terminal::vte::ansi;
use alacritty_terminal::Term;
use anyhow::{Context, Result};
use portable_pty::{Child, MasterPty, PtySize};

use crate::event_listener::AltScreenListener;
use crate::pty;

/// Dimensions adapter for creating and resizing a `Term`.
struct TermSize {
    cols: usize,
    rows: usize,
}

impl Dimensions for TermSize {
    fn total_lines(&self) -> usize {
        self.rows
    }

    fn screen_lines(&self) -> usize {
        self.rows
    }

    fn columns(&self) -> usize {
        self.cols
    }
}

/// A single terminal tab holding a PTY, child process, and VT state.
pub struct TerminalTab {
    title: String,
    master: Box<dyn MasterPty + Send>,
    writer: Box<dyn Write + Send>,
    child: Box<dyn Child + Send + Sync>,
    term: Term<AltScreenListener>,
    processor: ansi::Processor,
    last_cols: u16,
    last_rows: u16,
}

impl TerminalTab {
    /// Spawns a new shell in a PTY and returns the tab plus a reader for background I/O.
    ///
    /// The caller is responsible for reading from the returned reader in a background
    /// thread and feeding the data back through `process_output()`.
    pub fn new(cols: u16, rows: u16) -> Result<(Self, Box<dyn Read + Send>)> {
        let shell = pty::detect_shell();
        let (master, child, reader) =
            pty::spawn_shell(&shell, cols, rows).context("Failed to spawn terminal shell")?;

        let writer = master.take_writer().context("Failed to take PTY writer")?;

        let size = TermSize {
            cols: cols as usize,
            rows: rows as usize,
        };
        let term = Term::new(TermConfig::default(), &size, AltScreenListener);
        let processor = ansi::Processor::new();

        let tab = Self {
            title: shell,
            master,
            writer,
            child,
            term,
            processor,
            last_cols: cols,
            last_rows: rows,
        };

        Ok((tab, reader))
    }

    /// Writes raw bytes to the PTY, sending input to the shell process.
    pub fn write(&mut self, data: &[u8]) -> Result<()> {
        self.writer
            .write_all(data)
            .context("Failed to write to PTY")?;
        self.writer.flush().context("Failed to flush PTY writer")?;
        Ok(())
    }

    /// Feeds raw bytes from the PTY into the VT parser, updating the terminal grid.
    pub fn process_output(&mut self, data: &[u8]) {
        self.processor.advance(&mut self.term, data);
    }

    /// Resizes the PTY and terminal grid to new dimensions.
    pub fn resize(&mut self, cols: u16, rows: u16) -> Result<()> {
        if cols == self.last_cols && rows == self.last_rows {
            return Ok(());
        }

        self.master
            .resize(PtySize {
                rows,
                cols,
                pixel_width: 0,
                pixel_height: 0,
            })
            .context("Failed to resize PTY")?;

        let size = TermSize {
            cols: cols as usize,
            rows: rows as usize,
        };
        self.term.resize(size);

        self.last_cols = cols;
        self.last_rows = rows;

        Ok(())
    }

    /// Returns a reference to the alacritty terminal state for rendering.
    pub fn term(&self) -> &Term<AltScreenListener> {
        &self.term
    }

    /// Returns the tab title (typically the shell path).
    pub fn title(&self) -> &str {
        &self.title
    }

    /// Checks whether the child process is still running.
    pub fn is_alive(&mut self) -> bool {
        self.child
            .try_wait()
            .map(|status| status.is_none())
            .unwrap_or(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alacritty_terminal::grid::Dimensions;

    #[test]
    fn new_creates_valid_tab() {
        let result = TerminalTab::new(80, 24);
        assert!(
            result.is_ok(),
            "TerminalTab::new should succeed: {:?}",
            result.err()
        );

        let (mut tab, _reader) = result.unwrap();
        assert_eq!(tab.last_cols, 80);
        assert_eq!(tab.last_rows, 24);
        assert!(tab.is_alive(), "Child should be alive after creation");
    }

    #[test]
    fn process_output_updates_grid() {
        let (mut tab, _reader) = TerminalTab::new(80, 24).unwrap();

        // Feed "hello" as raw bytes through the VT parser.
        tab.process_output(b"hello");

        // Read back the first 5 cells of the first line from the grid.
        let grid = tab.term().grid();
        let mut text = String::new();
        for col in 0..5 {
            let cell =
                &grid[alacritty_terminal::index::Line(0)][alacritty_terminal::index::Column(col)];
            text.push(cell.c);
        }
        assert_eq!(
            text, "hello",
            "Grid should contain 'hello' after processing output"
        );
    }

    #[test]
    fn resize_updates_dimensions() {
        let (mut tab, _reader) = TerminalTab::new(80, 24).unwrap();

        let result = tab.resize(120, 40);
        assert!(result.is_ok(), "resize should succeed: {:?}", result.err());

        assert_eq!(tab.last_cols, 120);
        assert_eq!(tab.last_rows, 40);
        assert_eq!(tab.term().grid().columns(), 120);
        assert_eq!(tab.term().grid().screen_lines(), 40);
    }

    #[test]
    fn resize_noop_when_same_size() {
        let (mut tab, _reader) = TerminalTab::new(80, 24).unwrap();
        let result = tab.resize(80, 24);
        assert!(result.is_ok(), "noop resize should succeed");
    }

    #[test]
    fn title_returns_shell_name() {
        let (tab, _reader) = TerminalTab::new(80, 24).unwrap();
        assert!(!tab.title().is_empty(), "Tab title should not be empty");
    }

    #[test]
    fn write_sends_data_to_pty() {
        let (mut tab, _reader) = TerminalTab::new(80, 24).unwrap();
        // Writing to PTY should not error. The shell receives the bytes.
        let result = tab.write(b"echo hello\n");
        assert!(result.is_ok(), "write should succeed: {:?}", result.err());
        assert!(tab.is_alive(), "Child should still be alive after write");
    }
}
