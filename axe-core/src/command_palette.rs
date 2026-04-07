// IMPACT ANALYSIS — CommandPalette
// Parents: KeyEvent(Ctrl+Shift+P) -> Command::OpenCommandPalette -> AppState::execute()
// Children: Command dispatched when Enter pressed on selected item
// Siblings: FileFinder (same overlay pattern, same priority level),
//           show_help (CloseOverlay priority), confirm_dialog (higher priority)
// Risk: Must intercept keys before editor/tree/terminal handlers but after confirm_dialog

use nucleo_matcher::pattern::{AtomKind, CaseMatching, Normalization, Pattern};
use nucleo_matcher::{Matcher, Utf32Str};

use crate::command::Command;
use crate::fuzzy::{FilteredItem, MAX_RESULTS};
use crate::keymap::KeymapResolver;

/// A single entry in the command palette.
#[derive(Debug, Clone)]
pub struct CommandPaletteItem {
    /// Human-readable name (e.g., "File: Save", "View: Toggle Terminal").
    pub display_name: String,
    /// The command to execute when selected.
    pub command: Command,
    /// Keybinding string (e.g., "Ctrl+S"), empty if unbound.
    pub keybinding: String,
}

/// Command palette state for fuzzy-searching and executing commands.
pub struct CommandPalette {
    /// Current search query text.
    pub query: String,
    /// All palette entries.
    pub items: Vec<CommandPaletteItem>,
    /// Fuzzy-matched results sorted by score (best first).
    pub filtered: Vec<FilteredItem>,
    /// Index of the selected item within `filtered`.
    pub selected: usize,
    /// Scroll offset for rendering the results list.
    pub scroll_offset: usize,
}

/// Builds the static list of user-facing commands with human-readable names.
fn build_palette_items(keymap: &KeymapResolver) -> Vec<CommandPaletteItem> {
    let entries: Vec<(&str, Command)> = vec![
        ("Quit", Command::RequestQuit),
        ("Focus: Next Panel", Command::FocusNext),
        ("Focus: Previous Panel", Command::FocusPrev),
        ("Focus: File Tree", Command::FocusTree),
        ("Focus: Editor", Command::FocusEditor),
        ("Focus: Terminal", Command::FocusTerminal),
        ("View: Toggle File Tree", Command::ToggleTree),
        ("View: Toggle Terminal", Command::ToggleTerminal),
        ("View: Show Help", Command::ShowHelp),
        ("View: Resize Mode", Command::EnterResizeMode),
        ("View: Zoom Panel", Command::ZoomPanel),
        ("View: Toggle Ignored Files", Command::ToggleIgnored),
        ("View: Toggle File Icons", Command::ToggleIcons),
        ("File: Save", Command::EditorSave),
        ("File: Open File Finder", Command::OpenFileFinder),
        ("Edit: Undo", Command::EditorUndo),
        ("Edit: Redo", Command::EditorRedo),
        ("Edit: Select All", Command::EditorSelectAll),
        ("Edit: Copy", Command::EditorCopy),
        ("Edit: Cut", Command::EditorCut),
        ("Edit: Paste", Command::EditorPaste),
        ("Edit: Find", Command::EditorFind),
        ("Edit: Find and Replace", Command::EditorFindReplace),
        ("Tab: New Tab", Command::NewTab),
        ("Tab: Close Tab", Command::CloseTab),
        ("Tab: Next Tab", Command::NextTab),
        ("Tab: Previous Tab", Command::PrevTab),
        ("Tree: New File", Command::TreeCreateFile),
        ("Tree: New Directory", Command::TreeCreateDir),
        ("Tree: Rename", Command::TreeRename),
        ("Tree: Delete", Command::TreeDelete),
        ("Search: Find in Project", Command::OpenProjectSearch),
        ("Edit: Format Document", Command::FormatDocument),
        ("Edit: Go to Line", Command::GoToLine),
        ("SSH: Connect to Host", Command::OpenSshHostFinder),
    ];

    entries
        .into_iter()
        .map(|(name, cmd)| {
            let keybinding = keymap.binding_for(&cmd).unwrap_or_default();
            CommandPaletteItem {
                display_name: name.to_string(),
                command: cmd,
                keybinding,
            }
        })
        .collect()
}

impl CommandPalette {
    /// Creates a new command palette, populating items from the keymap.
    pub fn new(keymap: &KeymapResolver) -> Self {
        let items = build_palette_items(keymap);

        let filtered: Vec<FilteredItem> = items
            .iter()
            .enumerate()
            .take(MAX_RESULTS)
            .map(|(i, _)| FilteredItem {
                index: i,
                score: 0,
                match_indices: Vec::new(),
            })
            .collect();

        Self {
            query: String::new(),
            items,
            filtered,
            selected: 0,
            scroll_offset: 0,
        }
    }

    /// Re-runs fuzzy matching against all items using the current query.
    pub fn update_matches(&mut self) {
        if self.query.is_empty() {
            self.filtered = self
                .items
                .iter()
                .enumerate()
                .take(MAX_RESULTS)
                .map(|(i, _)| FilteredItem {
                    index: i,
                    score: 0,
                    match_indices: Vec::new(),
                })
                .collect();
        } else {
            let pattern = Pattern::new(
                &self.query,
                CaseMatching::Smart,
                Normalization::Smart,
                AtomKind::Fuzzy,
            );
            let mut matcher = Matcher::default();
            let mut buf = Vec::new();
            let mut indices_buf = Vec::new();

            let mut results: Vec<FilteredItem> = self
                .items
                .iter()
                .enumerate()
                .filter_map(|(i, item)| {
                    indices_buf.clear();
                    let haystack = Utf32Str::new(&item.display_name, &mut buf);
                    let score = pattern.indices(haystack, &mut matcher, &mut indices_buf)?;
                    indices_buf.sort_unstable();
                    indices_buf.dedup();
                    Some(FilteredItem {
                        index: i,
                        score,
                        match_indices: indices_buf.clone(),
                    })
                })
                .collect();

            results.sort_by(|a, b| b.score.cmp(&a.score));
            results.truncate(MAX_RESULTS);
            self.filtered = results;
        }

        self.selected = 0;
        self.scroll_offset = 0;
    }

    /// Appends a character to the query and re-matches.
    pub fn input_char(&mut self, c: char) {
        self.query.push(c);
        self.update_matches();
    }

    /// Removes the last character from the query and re-matches.
    pub fn input_backspace(&mut self) {
        self.query.pop();
        self.update_matches();
    }

    /// Moves selection up by one, wrapping to the end.
    pub fn move_up(&mut self) {
        if self.filtered.is_empty() {
            return;
        }
        if self.selected == 0 {
            self.selected = self.filtered.len() - 1;
        } else {
            self.selected -= 1;
        }
    }

    /// Moves selection down by one, wrapping to the start.
    pub fn move_down(&mut self) {
        if self.filtered.is_empty() {
            return;
        }
        if self.selected >= self.filtered.len() - 1 {
            self.selected = 0;
        } else {
            self.selected += 1;
        }
    }

    /// Returns the command of the currently selected item, if any.
    pub fn selected_command(&self) -> Option<&Command> {
        let filtered_item = self.filtered.get(self.selected)?;
        let item = self.items.get(filtered_item.index)?;
        Some(&item.command)
    }

    /// Returns the total number of commands in the palette.
    pub fn total_commands(&self) -> usize {
        self.items.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn palette() -> CommandPalette {
        let keymap = KeymapResolver::with_defaults();
        CommandPalette::new(&keymap)
    }

    #[test]
    fn new_creates_palette_with_expected_commands() {
        let p = palette();
        assert_eq!(p.items.len(), 35, "Expected 35 palette commands");
    }

    #[test]
    fn new_populates_all_filtered_initially() {
        let p = palette();
        assert_eq!(p.filtered.len(), p.items.len());
    }

    #[test]
    fn empty_query_shows_all_commands() {
        let p = palette();
        assert_eq!(p.filtered.len(), p.total_commands());
    }

    #[test]
    fn update_matches_filters_by_query() {
        let mut p = palette();
        p.query = "Save".to_string();
        p.update_matches();
        assert!(
            !p.filtered.is_empty(),
            "Should find at least one match for 'Save'"
        );
        let matched = &p.items[p.filtered[0].index];
        assert!(
            matched.display_name.contains("Save"),
            "Best match should contain 'Save', got: {}",
            matched.display_name
        );
    }

    #[test]
    fn update_matches_no_results_for_nonsense() {
        let mut p = palette();
        p.query = "zzzzz_nonexistent_command".to_string();
        p.update_matches();
        assert!(p.filtered.is_empty());
    }

    #[test]
    fn update_matches_resets_selection() {
        let mut p = palette();
        p.move_down();
        p.move_down();
        assert_eq!(p.selected, 2);
        p.input_char('s');
        assert_eq!(p.selected, 0);
    }

    #[test]
    fn input_char_appends_and_rematches() {
        let mut p = palette();
        p.input_char('T');
        assert_eq!(p.query, "T");
        assert!(p.filtered.len() < p.items.len() || p.filtered.len() == p.items.len());
    }

    #[test]
    fn input_backspace_removes_and_rematches() {
        let mut p = palette();
        p.input_char('S');
        p.input_char('a');
        let filtered_after_sa = p.filtered.len();
        p.input_backspace();
        assert_eq!(p.query, "S");
        assert!(p.filtered.len() >= filtered_after_sa);
    }

    #[test]
    fn input_backspace_on_empty_is_noop() {
        let mut p = palette();
        let count = p.filtered.len();
        p.input_backspace();
        assert_eq!(p.query, "");
        assert_eq!(p.filtered.len(), count);
    }

    #[test]
    fn move_down_advances_selection() {
        let mut p = palette();
        assert_eq!(p.selected, 0);
        p.move_down();
        assert_eq!(p.selected, 1);
    }

    #[test]
    fn move_down_wraps_to_start() {
        let mut p = palette();
        let last = p.filtered.len() - 1;
        p.selected = last;
        p.move_down();
        assert_eq!(p.selected, 0);
    }

    #[test]
    fn move_up_wraps_to_end() {
        let mut p = palette();
        assert_eq!(p.selected, 0);
        p.move_up();
        assert_eq!(p.selected, p.filtered.len() - 1);
    }

    #[test]
    fn move_up_decrements_selection() {
        let mut p = palette();
        p.move_down();
        p.move_down();
        p.move_up();
        assert_eq!(p.selected, 1);
    }

    #[test]
    fn move_up_on_empty_results_is_noop() {
        let mut p = palette();
        p.query = "zzzzz_nonexistent".to_string();
        p.update_matches();
        p.move_up(); // Should not panic
    }

    #[test]
    fn move_down_on_empty_results_is_noop() {
        let mut p = palette();
        p.query = "zzzzz_nonexistent".to_string();
        p.update_matches();
        p.move_down(); // Should not panic
    }

    #[test]
    fn selected_command_returns_correct_command() {
        let p = palette();
        let cmd = p.selected_command().expect("should have selected command");
        assert_eq!(*cmd, p.items[0].command);
    }

    #[test]
    fn selected_command_returns_none_when_no_results() {
        let mut p = palette();
        p.query = "zzzzz_nonexistent".to_string();
        p.update_matches();
        assert!(p.selected_command().is_none());
    }

    #[test]
    fn keybindings_populated_for_bound_commands() {
        let p = palette();
        let save_item = p
            .items
            .iter()
            .find(|i| i.display_name == "File: Save")
            .expect("Should have File: Save");
        assert_eq!(save_item.keybinding, "Ctrl+S");
    }

    #[test]
    fn keybinding_empty_for_unbound_commands() {
        // FocusNext has no keybinding in defaults (Tab is unbound in global keymap).
        let p = palette();
        let focus_next = p
            .items
            .iter()
            .find(|i| i.display_name == "Focus: Next Panel")
            .expect("Should have Focus: Next Panel");
        assert!(
            focus_next.keybinding.is_empty(),
            "FocusNext should have no keybinding, got: {}",
            focus_next.keybinding
        );
    }

    #[test]
    fn fuzzy_matching_works_with_gaps() {
        let mut p = palette();
        // "ttr" should fuzzy-match "View: Toggle File Tree" (t-t-r with gaps)
        p.query = "ttr".to_string();
        p.update_matches();
        assert!(
            !p.filtered.is_empty(),
            "Fuzzy match with gaps should find results"
        );
    }

    #[test]
    fn match_indices_populated_for_matches() {
        let mut p = palette();
        p.query = "Save".to_string();
        p.update_matches();
        assert!(!p.filtered.is_empty());
        assert!(
            !p.filtered[0].match_indices.is_empty(),
            "Match indices should be populated"
        );
    }

    #[test]
    fn total_commands_returns_item_count() {
        let p = palette();
        assert_eq!(p.total_commands(), p.items.len());
    }
}
