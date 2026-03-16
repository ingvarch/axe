// IMPACT ANALYSIS — tab module
// Parents: TerminalManager creates and owns TerminalTab instances.
// Children: pty::spawn_shell() for PTY creation; Term<AltScreenListener> for VT parsing.
//           The background reader thread reads from the PTY reader.
// Siblings: manager.rs polls output and feeds it to the active tab via process_output().
//           axe-ui reads tab.term() for rendering.
//           Resize events from the main loop call tab.resize().

use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use alacritty_terminal::grid::{Dimensions, Scroll};
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
    pub fn new(cols: u16, rows: u16, cwd: &Path) -> Result<(Self, Box<dyn Read + Send>)> {
        let shell = pty::detect_shell();
        let (master, child, reader) =
            pty::spawn_shell(&shell, cols, rows, cwd).context("Failed to spawn terminal shell")?;

        let writer = master.take_writer().context("Failed to take PTY writer")?;

        let size = TermSize {
            cols: cols as usize,
            rows: rows as usize,
        };
        let term = Term::new(TermConfig::default(), &size, AltScreenListener);
        let processor = ansi::Processor::new();

        let tab = Self {
            title: abbreviate_path(cwd),
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

    /// Kills the child process.
    pub fn kill(&mut self) -> Result<()> {
        self.child.kill().context("Failed to kill terminal process")
    }

    /// Scrolls the terminal display by the given amount.
    pub fn scroll(&mut self, scroll: Scroll) {
        self.term.scroll_display(scroll);
    }

    /// Returns the current display offset (0 = at bottom, >0 = scrolled up).
    pub fn display_offset(&self) -> usize {
        self.term.grid().display_offset()
    }

    /// Returns a mutable reference to the alacritty terminal state.
    pub fn term_mut(&mut self) -> &mut Term<AltScreenListener> {
        &mut self.term
    }

    /// Checks whether the child process is still running.
    pub fn is_alive(&mut self) -> bool {
        self.child
            .try_wait()
            .map(|status| status.is_none())
            .unwrap_or(false)
    }
}

/// Abbreviates a path for display in tab titles.
///
/// Rules:
/// - Home directory prefix is replaced with `~`
/// - All intermediate components (between `~` and the last) are shortened to their first character
/// - The last component is always shown in full
///
/// Examples (assuming home = `/Users/igor`):
/// - `/Users/igor` → `~`
/// - `/Users/igor/Repos` → `~/Repos`
/// - `/Users/igor/Repos/ingvarch` → `~/R/ingvarch`
/// - `/Users/igor/Repos/ingvarch/axe` → `~/R/i/axe`
/// - `/tmp/build` → `/t/build`
fn abbreviate_path(path: &Path) -> String {
    let home = std::env::var("HOME").ok().map(PathBuf::from);

    let (use_tilde, components): (bool, Vec<String>) = match home {
        Some(ref home_path) => {
            if let Ok(relative) = path.strip_prefix(home_path) {
                let comps: Vec<String> = relative
                    .components()
                    .filter_map(|c| {
                        if let std::path::Component::Normal(s) = c {
                            Some(s.to_string_lossy().into_owned())
                        } else {
                            None
                        }
                    })
                    .collect();
                (true, comps)
            } else {
                (false, path_to_components(path))
            }
        }
        None => (false, path_to_components(path)),
    };

    if components.is_empty() {
        return if use_tilde {
            "~".to_string()
        } else {
            "/".to_string()
        };
    }

    let mut parts: Vec<String> = Vec::with_capacity(components.len() + 1);
    if use_tilde {
        parts.push("~".to_string());
    }

    let last = components.len() - 1;
    for (i, comp) in components.iter().enumerate() {
        if i == last {
            parts.push(comp.clone());
        } else {
            // Abbreviate to first character.
            let first_char = comp.chars().next().unwrap_or('?');
            parts.push(first_char.to_string());
        }
    }

    if use_tilde {
        parts.join("/")
    } else {
        // Absolute path: prefix with /
        format!("/{}", parts.join("/"))
    }
}

/// Extracts Normal components from an absolute path as strings.
fn path_to_components(path: &Path) -> Vec<String> {
    path.components()
        .filter_map(|c| {
            if let std::path::Component::Normal(s) = c {
                Some(s.to_string_lossy().into_owned())
            } else {
                None
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use alacritty_terminal::grid::{Dimensions, Scroll};

    // --- abbreviate_path tests ---

    #[test]
    fn abbreviate_path_home_dir() {
        let home = std::env::var("HOME").unwrap();
        let result = abbreviate_path(Path::new(&home));
        assert_eq!(result, "~");
    }

    #[test]
    fn abbreviate_path_one_level_below_home() {
        let home = std::env::var("HOME").unwrap();
        let path = PathBuf::from(&home).join("Repos");
        let result = abbreviate_path(&path);
        assert_eq!(result, "~/Repos");
    }

    #[test]
    fn abbreviate_path_two_levels_below_home() {
        let home = std::env::var("HOME").unwrap();
        let path = PathBuf::from(&home).join("Repos").join("ingvarch");
        let result = abbreviate_path(&path);
        assert_eq!(result, "~/R/ingvarch");
    }

    #[test]
    fn abbreviate_path_three_levels_below_home() {
        let home = std::env::var("HOME").unwrap();
        let path = PathBuf::from(&home)
            .join("Repos")
            .join("ingvarch")
            .join("axe");
        let result = abbreviate_path(&path);
        assert_eq!(result, "~/R/i/axe");
    }

    #[test]
    fn abbreviate_path_outside_home() {
        let result = abbreviate_path(Path::new("/tmp/build"));
        assert_eq!(result, "/t/build");
    }

    #[test]
    fn abbreviate_path_root() {
        let result = abbreviate_path(Path::new("/"));
        assert_eq!(result, "/");
    }

    #[test]
    fn abbreviate_path_single_component() {
        let result = abbreviate_path(Path::new("/usr"));
        assert_eq!(result, "/usr");
    }

    // --- TerminalTab tests ---

    #[test]
    fn new_creates_valid_tab() {
        let result = TerminalTab::new(80, 24, &std::env::current_dir().unwrap());
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
    fn new_tab_title_is_abbreviated_cwd() {
        let cwd = std::env::current_dir().unwrap();
        let (tab, _reader) = TerminalTab::new(80, 24, &cwd).unwrap();
        let expected = abbreviate_path(&cwd);
        assert_eq!(tab.title(), expected);
    }

    #[test]
    fn process_output_updates_grid() {
        let (mut tab, _reader) =
            TerminalTab::new(80, 24, &std::env::current_dir().unwrap()).unwrap();

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
        let (mut tab, _reader) =
            TerminalTab::new(80, 24, &std::env::current_dir().unwrap()).unwrap();

        let result = tab.resize(120, 40);
        assert!(result.is_ok(), "resize should succeed: {:?}", result.err());

        assert_eq!(tab.last_cols, 120);
        assert_eq!(tab.last_rows, 40);
        assert_eq!(tab.term().grid().columns(), 120);
        assert_eq!(tab.term().grid().screen_lines(), 40);
    }

    #[test]
    fn resize_noop_when_same_size() {
        let (mut tab, _reader) =
            TerminalTab::new(80, 24, &std::env::current_dir().unwrap()).unwrap();
        let result = tab.resize(80, 24);
        assert!(result.is_ok(), "noop resize should succeed");
    }

    #[test]
    fn title_returns_nonempty() {
        let (tab, _reader) = TerminalTab::new(80, 24, &std::env::current_dir().unwrap()).unwrap();
        assert!(!tab.title().is_empty(), "Tab title should not be empty");
    }

    #[test]
    fn display_offset_zero_by_default() {
        let (tab, _reader) = TerminalTab::new(80, 24, &std::env::current_dir().unwrap()).unwrap();
        assert_eq!(
            tab.display_offset(),
            0,
            "New tab should have display_offset 0"
        );
    }

    #[test]
    fn scroll_page_up_changes_offset() {
        let (mut tab, _reader) =
            TerminalTab::new(80, 24, &std::env::current_dir().unwrap()).unwrap();

        // Feed enough output to fill scrollback: 100 lines of text.
        for i in 0..100 {
            let line = format!("line {i}\r\n");
            tab.process_output(line.as_bytes());
        }

        tab.scroll(Scroll::PageUp);
        assert!(
            tab.display_offset() > 0,
            "After PageUp with scrollback content, display_offset should be > 0"
        );
    }

    #[test]
    fn scroll_bottom_resets_offset() {
        let (mut tab, _reader) =
            TerminalTab::new(80, 24, &std::env::current_dir().unwrap()).unwrap();

        // Fill scrollback.
        for i in 0..100 {
            let line = format!("line {i}\r\n");
            tab.process_output(line.as_bytes());
        }

        tab.scroll(Scroll::PageUp);
        assert!(tab.display_offset() > 0);

        tab.scroll(Scroll::Bottom);
        assert_eq!(
            tab.display_offset(),
            0,
            "Scroll::Bottom should reset display_offset to 0"
        );
    }

    #[test]
    fn scroll_top_and_bottom() {
        let (mut tab, _reader) =
            TerminalTab::new(80, 24, &std::env::current_dir().unwrap()).unwrap();

        // Fill scrollback.
        for i in 0..100 {
            let line = format!("line {i}\r\n");
            tab.process_output(line.as_bytes());
        }

        tab.scroll(Scroll::Top);
        let top_offset = tab.display_offset();
        assert!(
            top_offset > 0,
            "Scroll::Top should scroll to maximum offset"
        );

        tab.scroll(Scroll::Bottom);
        assert_eq!(tab.display_offset(), 0);
    }

    #[test]
    fn write_sends_data_to_pty() {
        let (mut tab, _reader) =
            TerminalTab::new(80, 24, &std::env::current_dir().unwrap()).unwrap();
        // Writing to PTY should not error. The shell receives the bytes.
        let result = tab.write(b"echo hello\n");
        assert!(result.is_ok(), "write should succeed: {:?}", result.err());
        assert!(tab.is_alive(), "Child should still be alive after write");
    }
}
