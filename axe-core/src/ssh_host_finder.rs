// IMPACT ANALYSIS — SshHostFinder
// Parents: Command::OpenSshHostFinder -> AppState::open_ssh_host_finder() creates this.
//          KeyEvent in input.rs controls navigation and selection.
// Children: Command::ConnectSshHost(host) dispatched on Enter.
// Siblings: FileFinder (same pattern), CommandPalette (same pattern).
// Risk: Must intercept keys before editor/tree/terminal handlers.

use nucleo_matcher::pattern::{AtomKind, CaseMatching, Normalization, Pattern};
use nucleo_matcher::{Matcher, Utf32Str};

use crate::fuzzy::{FilteredItem, MAX_RESULTS};
use crate::ssh_host::SshHost;

/// Default number of items to skip when using PageUp/PageDown.
pub const SSH_FINDER_PAGE_SIZE: usize = 10;

/// Fuzzy finder state for SSH hosts.
pub struct SshHostFinder {
    /// Current search query text.
    pub query: String,
    /// All available SSH hosts.
    pub items: Vec<SshHost>,
    /// Fuzzy-matched results sorted by score (best first).
    pub filtered: Vec<FilteredItem>,
    /// Index of the selected item within `filtered`.
    pub selected: usize,
    /// Scroll offset for rendering the results list.
    pub scroll_offset: usize,
}

impl SshHostFinder {
    /// Creates a new `SshHostFinder` from a list of SSH hosts.
    pub fn new(items: Vec<SshHost>) -> Self {
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

    /// Moves selection up by `page_size` items, clamping to the first item.
    pub fn move_page_up(&mut self, page_size: usize) {
        if self.filtered.is_empty() {
            return;
        }
        self.selected = self.selected.saturating_sub(page_size);
    }

    /// Moves selection down by `page_size` items, clamping to the last item.
    pub fn move_page_down(&mut self, page_size: usize) {
        if self.filtered.is_empty() {
            return;
        }
        self.selected = (self.selected + page_size).min(self.filtered.len() - 1);
    }

    /// Returns the currently selected SSH host, if any.
    pub fn selected_host(&self) -> Option<&SshHost> {
        let filtered_item = self.filtered.get(self.selected)?;
        self.items.get(filtered_item.index)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ssh_host::SshHostSource;

    fn make_hosts(names: &[&str]) -> Vec<SshHost> {
        names
            .iter()
            .map(|name| SshHost {
                name: name.to_string(),
                hostname: format!("{name}.example.com"),
                port: 22,
                user: "testuser".to_string(),
                identity_file: None,
                source: SshHostSource::SshConfig,
                display_name: name.to_string(),
            })
            .collect()
    }

    #[test]
    fn new_shows_all_hosts() {
        let finder = SshHostFinder::new(make_hosts(&["alpha", "beta", "gamma"]));
        assert_eq!(finder.filtered.len(), 3);
        assert_eq!(finder.selected, 0);
    }

    #[test]
    fn new_empty_hosts() {
        let finder = SshHostFinder::new(Vec::new());
        assert!(finder.filtered.is_empty());
        assert!(finder.selected_host().is_none());
    }

    #[test]
    fn fuzzy_match_filters_results() {
        let mut finder = SshHostFinder::new(make_hosts(&["production", "staging", "dev"]));
        finder.input_char('p');
        finder.input_char('r');
        assert!(!finder.filtered.is_empty());
        let selected = finder.selected_host().unwrap();
        assert_eq!(selected.name, "production");
    }

    #[test]
    fn input_backspace_widens_results() {
        let mut finder = SshHostFinder::new(make_hosts(&["production", "staging"]));
        finder.input_char('p');
        finder.input_char('r');
        let narrow = finder.filtered.len();
        finder.input_backspace();
        assert!(finder.filtered.len() >= narrow);
    }

    #[test]
    fn move_down_advances_selection() {
        let mut finder = SshHostFinder::new(make_hosts(&["a", "b", "c"]));
        assert_eq!(finder.selected, 0);
        finder.move_down();
        assert_eq!(finder.selected, 1);
    }

    #[test]
    fn move_down_wraps() {
        let mut finder = SshHostFinder::new(make_hosts(&["a", "b"]));
        finder.move_down();
        finder.move_down();
        assert_eq!(finder.selected, 0);
    }

    #[test]
    fn move_up_wraps() {
        let mut finder = SshHostFinder::new(make_hosts(&["a", "b", "c"]));
        finder.move_up();
        assert_eq!(finder.selected, 2);
    }

    #[test]
    fn move_page_down_clamps() {
        let mut finder = SshHostFinder::new(make_hosts(&["a", "b", "c"]));
        finder.move_page_down(10);
        assert_eq!(finder.selected, 2);
    }

    #[test]
    fn move_page_up_clamps() {
        let mut finder = SshHostFinder::new(make_hosts(&["a", "b", "c"]));
        finder.selected = 1;
        finder.move_page_up(10);
        assert_eq!(finder.selected, 0);
    }

    #[test]
    fn selected_host_returns_correct_item() {
        let finder = SshHostFinder::new(make_hosts(&["alpha", "beta"]));
        let host = finder.selected_host().unwrap();
        assert_eq!(host.name, "alpha");
    }

    #[test]
    fn selection_resets_on_query_change() {
        let mut finder = SshHostFinder::new(make_hosts(&["a", "b", "c"]));
        finder.move_down();
        finder.move_down();
        assert_eq!(finder.selected, 2);
        finder.input_char('a');
        assert_eq!(finder.selected, 0);
    }

    #[test]
    fn move_on_empty_is_noop() {
        let mut finder = SshHostFinder::new(make_hosts(&["a"]));
        finder.query = "zzzzz".to_string();
        finder.update_matches();
        assert!(finder.filtered.is_empty());
        finder.move_up();
        finder.move_down();
        finder.move_page_up(10);
        finder.move_page_down(10);
        // Should not panic.
    }
}
