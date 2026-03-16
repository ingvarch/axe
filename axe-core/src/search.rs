// IMPACT ANALYSIS — SearchState
// Parents: KeyEvent → Ctrl+F → Command::EditorFind → AppState creates SearchState
// Children: UI renders search bar + match highlights, cursor jumps to matches
// Siblings: Selection (coexists), editor key interception (search layer runs first),
//           buffer content (read-only during search)

use axe_editor::EditorBuffer;

/// A single match location within the buffer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchMatch {
    pub row: usize,
    pub col_start: usize,
    pub col_end: usize,
}

/// State for the in-file search feature.
///
/// Tracks the query, match positions, navigation index, and search flags.
/// Matches are recomputed on every query change by iterating buffer lines.
pub struct SearchState {
    pub query: String,
    pub matches: Vec<SearchMatch>,
    pub current: usize,
    pub case_sensitive: bool,
    pub regex_mode: bool,
    /// Set when regex compilation fails.
    pub regex_error: bool,
}

impl SearchState {
    /// Creates a new empty search state.
    pub fn new() -> Self {
        Self {
            query: String::new(),
            matches: Vec::new(),
            current: 0,
            case_sensitive: false,
            regex_mode: false,
            regex_error: false,
        }
    }

    /// Recomputes matches by searching the buffer line by line.
    pub fn update_matches(&mut self, buffer: &EditorBuffer) {
        self.regex_error = false;
        self.matches.clear();
        self.current = 0;

        if self.query.is_empty() {
            return;
        }

        if self.regex_mode {
            self.find_matches_regex(buffer);
        } else {
            self.find_matches_literal(buffer);
        }
    }

    /// Literal search: finds all occurrences line by line.
    fn find_matches_literal(&mut self, buffer: &EditorBuffer) {
        let query = if self.case_sensitive {
            self.query.clone()
        } else {
            self.query.to_lowercase()
        };

        for row in 0..buffer.line_count() {
            if let Some(line_slice) = buffer.line_at(row) {
                let line_text: String = line_slice.chars().collect();
                let haystack = if self.case_sensitive {
                    line_text.clone()
                } else {
                    line_text.to_lowercase()
                };

                let mut start = 0;
                while let Some(pos) = haystack[start..].find(&query) {
                    let col_start = start + pos;
                    let col_end = col_start + query.len();
                    self.matches.push(SearchMatch {
                        row,
                        col_start,
                        col_end,
                    });
                    start = col_start + 1;
                    if start >= haystack.len() {
                        break;
                    }
                }
            }
        }
    }

    /// Regex search: finds all occurrences line by line.
    fn find_matches_regex(&mut self, buffer: &EditorBuffer) {
        let pattern = if self.case_sensitive {
            self.query.clone()
        } else {
            format!("(?i){}", self.query)
        };

        let re = match regex::Regex::new(&pattern) {
            Ok(re) => re,
            Err(_) => {
                self.regex_error = true;
                return;
            }
        };

        for row in 0..buffer.line_count() {
            if let Some(line_slice) = buffer.line_at(row) {
                let line_text: String = line_slice.chars().collect();
                for m in re.find_iter(&line_text) {
                    if m.start() != m.end() {
                        self.matches.push(SearchMatch {
                            row,
                            col_start: m.start(),
                            col_end: m.end(),
                        });
                    }
                }
            }
        }
    }

    /// Advances to the next match, wrapping around.
    pub fn next_match(&mut self) {
        if !self.matches.is_empty() {
            self.current = (self.current + 1) % self.matches.len();
        }
    }

    /// Goes to the previous match, wrapping around.
    pub fn prev_match(&mut self) {
        if !self.matches.is_empty() {
            if self.current == 0 {
                self.current = self.matches.len() - 1;
            } else {
                self.current -= 1;
            }
        }
    }

    /// Returns the currently active match, if any.
    pub fn current_match(&self) -> Option<&SearchMatch> {
        self.matches.get(self.current)
    }

    /// Sets `current` to the first match at or after the given position.
    pub fn nearest_match_from(&mut self, row: usize, col: usize) {
        if self.matches.is_empty() {
            return;
        }
        for (i, m) in self.matches.iter().enumerate() {
            if m.row > row || (m.row == row && m.col_start >= col) {
                self.current = i;
                return;
            }
        }
        // Wrap to first match.
        self.current = 0;
    }

    /// Appends a character to the query and recomputes matches.
    pub fn input_char(&mut self, c: char, buffer: &EditorBuffer) {
        self.query.push(c);
        self.update_matches(buffer);
    }

    /// Removes the last character from the query and recomputes matches.
    pub fn input_backspace(&mut self, buffer: &EditorBuffer) {
        self.query.pop();
        self.update_matches(buffer);
    }

    /// Toggles case sensitivity and recomputes matches.
    pub fn toggle_case(&mut self, buffer: &EditorBuffer) {
        self.case_sensitive = !self.case_sensitive;
        self.update_matches(buffer);
    }

    /// Toggles regex mode and recomputes matches.
    pub fn toggle_regex(&mut self, buffer: &EditorBuffer) {
        self.regex_mode = !self.regex_mode;
        self.update_matches(buffer);
    }

    /// Returns a display string for the match count.
    pub fn match_count_display(&self) -> String {
        if self.query.is_empty() {
            return String::new();
        }
        if self.regex_error {
            return "Invalid regex".to_string();
        }
        if self.matches.is_empty() {
            return "No matches".to_string();
        }
        format!("{} of {}", self.current + 1, self.matches.len())
    }
}

impl Default for SearchState {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Creates a buffer with the given text content.
    fn buffer_with(text: &str) -> EditorBuffer {
        let mut buf = EditorBuffer::new();
        buf.insert_text(text);
        buf
    }

    #[test]
    fn find_matches_literal_single() {
        let buf = buffer_with("hello world");
        let mut search = SearchState::new();
        search.query = "world".to_string();
        search.update_matches(&buf);
        assert_eq!(search.matches.len(), 1);
        assert_eq!(
            search.matches[0],
            SearchMatch {
                row: 0,
                col_start: 6,
                col_end: 11,
            }
        );
    }

    #[test]
    fn find_matches_literal_multiple_per_line() {
        let buf = buffer_with("abcabc");
        let mut search = SearchState::new();
        search.query = "abc".to_string();
        search.update_matches(&buf);
        assert_eq!(search.matches.len(), 2);
        assert_eq!(search.matches[0].col_start, 0);
        assert_eq!(search.matches[1].col_start, 3);
    }

    #[test]
    fn find_matches_case_insensitive() {
        let buf = buffer_with("Hello HELLO hello");
        let mut search = SearchState::new();
        search.query = "hello".to_string();
        search.case_sensitive = false;
        search.update_matches(&buf);
        assert_eq!(search.matches.len(), 3);
    }

    #[test]
    fn find_matches_case_sensitive() {
        let buf = buffer_with("Hello HELLO hello");
        let mut search = SearchState::new();
        search.query = "hello".to_string();
        search.case_sensitive = true;
        search.update_matches(&buf);
        assert_eq!(search.matches.len(), 1);
        assert_eq!(search.matches[0].col_start, 12);
    }

    #[test]
    fn find_matches_regex() {
        let buf = buffer_with("foo123 bar456");
        let mut search = SearchState::new();
        search.query = r"\d+".to_string();
        search.regex_mode = true;
        search.update_matches(&buf);
        assert_eq!(search.matches.len(), 2);
        assert_eq!(search.matches[0].col_start, 3);
        assert_eq!(search.matches[0].col_end, 6);
        assert_eq!(search.matches[1].col_start, 10);
        assert_eq!(search.matches[1].col_end, 13);
    }

    #[test]
    fn find_matches_invalid_regex() {
        let buf = buffer_with("test");
        let mut search = SearchState::new();
        search.query = "[invalid".to_string();
        search.regex_mode = true;
        search.update_matches(&buf);
        assert!(search.regex_error);
        assert!(search.matches.is_empty());
    }

    #[test]
    fn next_match_wraps() {
        let buf = buffer_with("aaa");
        let mut search = SearchState::new();
        search.query = "a".to_string();
        search.update_matches(&buf);
        assert_eq!(search.matches.len(), 3);
        assert_eq!(search.current, 0);
        search.next_match();
        assert_eq!(search.current, 1);
        search.next_match();
        assert_eq!(search.current, 2);
        search.next_match();
        assert_eq!(search.current, 0); // Wrapped
    }

    #[test]
    fn prev_match_wraps() {
        let buf = buffer_with("aaa");
        let mut search = SearchState::new();
        search.query = "a".to_string();
        search.update_matches(&buf);
        assert_eq!(search.current, 0);
        search.prev_match();
        assert_eq!(search.current, 2); // Wrapped to last
        search.prev_match();
        assert_eq!(search.current, 1);
    }

    #[test]
    fn nearest_match_from_cursor() {
        let buf = buffer_with("abc\nabc\nabc");
        let mut search = SearchState::new();
        search.query = "abc".to_string();
        search.update_matches(&buf);
        assert_eq!(search.matches.len(), 3);

        search.nearest_match_from(1, 0);
        assert_eq!(search.current, 1);

        // Past all matches — wraps to 0.
        search.nearest_match_from(5, 0);
        assert_eq!(search.current, 0);
    }

    #[test]
    fn input_char_updates_matches() {
        let buf = buffer_with("hello world");
        let mut search = SearchState::new();
        search.input_char('w', &buf);
        assert_eq!(search.query, "w");
        assert_eq!(search.matches.len(), 1);
        search.input_char('o', &buf);
        assert_eq!(search.query, "wo");
        assert_eq!(search.matches.len(), 1);
        assert_eq!(search.matches[0].col_start, 6);
    }

    #[test]
    fn input_backspace_updates_matches() {
        let buf = buffer_with("hello world");
        let mut search = SearchState::new();
        search.query = "world".to_string();
        search.update_matches(&buf);
        assert_eq!(search.matches.len(), 1);
        search.input_backspace(&buf);
        assert_eq!(search.query, "worl");
        assert_eq!(search.matches.len(), 1);
    }

    #[test]
    fn toggle_case_recomputes() {
        let buf = buffer_with("Hello hello");
        let mut search = SearchState::new();
        search.query = "hello".to_string();
        search.update_matches(&buf);
        assert_eq!(search.matches.len(), 2); // Case insensitive

        search.toggle_case(&buf);
        assert!(search.case_sensitive);
        assert_eq!(search.matches.len(), 1); // Only lowercase match
    }

    #[test]
    fn toggle_regex_recomputes() {
        let buf = buffer_with("foo123 bar456");
        let mut search = SearchState::new();
        search.query = r"\d+".to_string();
        search.update_matches(&buf);
        assert_eq!(search.matches.len(), 0); // Literal search for "\d+"

        search.toggle_regex(&buf);
        assert!(search.regex_mode);
        assert_eq!(search.matches.len(), 2); // Regex finds digits
    }

    #[test]
    fn match_count_display_formats() {
        let buf = buffer_with("aaa");
        let mut search = SearchState::new();

        // Empty query.
        assert_eq!(search.match_count_display(), "");

        // With matches.
        search.query = "a".to_string();
        search.update_matches(&buf);
        assert_eq!(search.match_count_display(), "1 of 3");

        search.next_match();
        assert_eq!(search.match_count_display(), "2 of 3");

        // No matches.
        search.query = "z".to_string();
        search.update_matches(&buf);
        assert_eq!(search.match_count_display(), "No matches");

        // Invalid regex.
        search.query = "[bad".to_string();
        search.regex_mode = true;
        search.update_matches(&buf);
        assert_eq!(search.match_count_display(), "Invalid regex");
    }

    #[test]
    fn empty_query_produces_no_matches() {
        let buf = buffer_with("hello");
        let mut search = SearchState::new();
        search.update_matches(&buf);
        assert!(search.matches.is_empty());
    }

    #[test]
    fn current_match_returns_none_when_empty() {
        let search = SearchState::new();
        assert!(search.current_match().is_none());
    }

    #[test]
    fn multiline_search() {
        let buf = buffer_with("foo\nbar\nfoo bar foo");
        let mut search = SearchState::new();
        search.query = "foo".to_string();
        search.update_matches(&buf);
        assert_eq!(search.matches.len(), 3);
        assert_eq!(search.matches[0].row, 0);
        assert_eq!(search.matches[1].row, 2);
        assert_eq!(search.matches[1].col_start, 0);
        assert_eq!(search.matches[2].row, 2);
        assert_eq!(search.matches[2].col_start, 8);
    }
}
