// IMPACT ANALYSIS — ProjectSearch
// Parents: KeyEvent(Ctrl+Shift+F / F2) -> Command::OpenProjectSearch -> AppState::execute()
// Children: Command::OpenFile(path) dispatched when Enter pressed on a match result
// Siblings: FileFinder (same overlay pattern), CommandPalette (same overlay pattern),
//           show_help (CloseOverlay priority), confirm_dialog (higher priority)
// Risk: Background search thread must be cancelable; channel must be drained each frame
//       to prevent unbounded growth. Must intercept keys after command_palette but before file_finder.

use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc};

use ignore::overrides::OverrideBuilder;
use ignore::WalkBuilder;
use regex::Regex;

/// Maximum number of results before the search stops to prevent unbounded growth.
const MAX_RESULTS: usize = 5000;
/// Maximum file size (in bytes) to search; files larger than this are skipped.
const MAX_FILE_SIZE: u64 = 1_048_576; // 1 MB
/// Number of bytes to check for null bytes (binary file detection).
const BINARY_CHECK_SIZE: usize = 512;
/// Number of results to batch before sending through the channel.
const BATCH_SIZE: usize = 50;

/// A single match from the project-wide search.
#[derive(Debug, Clone)]
pub struct ProjectSearchMatch {
    /// Path relative to the project root.
    pub relative_path: String,
    /// Absolute path on disk.
    pub absolute_path: PathBuf,
    /// 1-based line number within the file.
    pub line_number: usize,
    /// Full text of the matching line (trimmed trailing newline).
    pub line_text: String,
    /// Byte offset of the match start within `line_text`.
    pub match_start: usize,
    /// Byte offset of the match end within `line_text`.
    pub match_end: usize,
}

/// Which input field is currently active in the search overlay.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SearchField {
    Query,
    Include,
    Exclude,
}

/// An item in the display list — either a file header or a match line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DisplayItem {
    /// Group header showing the file path and match count.
    FileHeader {
        relative_path: String,
        match_count: usize,
    },
    /// A single matching line, referencing by index into `results`.
    MatchLine { result_index: usize },
}

/// Events sent from the background search thread to the main thread.
pub enum SearchEvent {
    /// A batch of search results.
    Results(Vec<ProjectSearchMatch>),
    /// Progress update: how many files have been searched so far.
    Progress { files_searched: usize },
    /// The search is complete.
    Done,
}

/// State for the project-wide search overlay.
pub struct ProjectSearch {
    /// Current search query text.
    pub query: String,
    /// Whether the search is case-sensitive.
    pub case_sensitive: bool,
    /// Whether the query is interpreted as a regex.
    pub regex_mode: bool,
    /// Glob pattern for files to include (e.g., "*.rs").
    pub include_pattern: String,
    /// Glob pattern for files to exclude (e.g., "*.test.*").
    pub exclude_pattern: String,
    /// Which input field is currently focused.
    pub active_field: SearchField,
    /// All search results collected so far.
    pub results: Vec<ProjectSearchMatch>,
    /// Flattened display list (file headers + match lines).
    pub display_items: Vec<DisplayItem>,
    /// Index of the selected item in `display_items`.
    pub selected: usize,
    /// Scroll offset for rendering.
    pub scroll_offset: usize,
    /// Whether a background search is currently running.
    pub searching: bool,
    /// Number of files searched so far (from progress events).
    pub files_searched: usize,
    /// Number of distinct files with at least one match.
    pub files_with_matches: usize,
    /// Receiver for search events from the background thread.
    result_rx: Option<mpsc::Receiver<SearchEvent>>,
    /// Flag to signal the background thread to cancel.
    cancel_flag: Option<Arc<AtomicBool>>,
}

impl Default for ProjectSearch {
    fn default() -> Self {
        Self::new()
    }
}

impl ProjectSearch {
    /// Creates a new empty project search state.
    pub fn new() -> Self {
        Self {
            query: String::new(),
            case_sensitive: false,
            regex_mode: false,
            include_pattern: String::new(),
            exclude_pattern: String::new(),
            active_field: SearchField::Query,
            results: Vec::new(),
            display_items: Vec::new(),
            selected: 0,
            scroll_offset: 0,
            searching: false,
            files_searched: 0,
            files_with_matches: 0,
            result_rx: None,
            cancel_flag: None,
        }
    }

    /// Appends a character to the currently active input field.
    pub fn input_char(&mut self, c: char) {
        match self.active_field {
            SearchField::Query => self.query.push(c),
            SearchField::Include => self.include_pattern.push(c),
            SearchField::Exclude => self.exclude_pattern.push(c),
        }
    }

    /// Removes the last character from the currently active input field.
    pub fn input_backspace(&mut self) {
        match self.active_field {
            SearchField::Query => {
                self.query.pop();
            }
            SearchField::Include => {
                self.include_pattern.pop();
            }
            SearchField::Exclude => {
                self.exclude_pattern.pop();
            }
        }
    }

    /// Cycles the active field: Query -> Include -> Exclude -> Query.
    pub fn cycle_field(&mut self) {
        self.active_field = match self.active_field {
            SearchField::Query => SearchField::Include,
            SearchField::Include => SearchField::Exclude,
            SearchField::Exclude => SearchField::Query,
        };
    }

    /// Toggles case sensitivity.
    pub fn toggle_case(&mut self) {
        self.case_sensitive = !self.case_sensitive;
    }

    /// Toggles regex mode.
    pub fn toggle_regex(&mut self) {
        self.regex_mode = !self.regex_mode;
    }

    /// Moves selection up by one, wrapping to the end.
    pub fn move_up(&mut self) {
        if self.display_items.is_empty() {
            return;
        }
        if self.selected == 0 {
            self.selected = self.display_items.len() - 1;
        } else {
            self.selected -= 1;
        }
    }

    /// Moves selection down by one, wrapping to the start.
    pub fn move_down(&mut self) {
        if self.display_items.is_empty() {
            return;
        }
        if self.selected >= self.display_items.len() - 1 {
            self.selected = 0;
        } else {
            self.selected += 1;
        }
    }

    /// Returns the search result for the currently selected display item,
    /// or `None` if the selection is on a file header or the list is empty.
    pub fn selected_result(&self) -> Option<&ProjectSearchMatch> {
        let item = self.display_items.get(self.selected)?;
        match item {
            DisplayItem::MatchLine { result_index } => self.results.get(*result_index),
            DisplayItem::FileHeader { .. } => None,
        }
    }

    /// Returns the total number of matches found.
    pub fn total_matches(&self) -> usize {
        self.results.len()
    }

    /// Cancels any running background search.
    pub fn cancel_search(&mut self) {
        if let Some(ref flag) = self.cancel_flag {
            flag.store(true, Ordering::Relaxed);
        }
        self.result_rx = None;
        self.cancel_flag = None;
        self.searching = false;
    }

    /// Starts a new background search, canceling any previous one.
    pub fn start_search(&mut self, root: &Path) {
        self.cancel_search();

        if self.query.is_empty() {
            self.results.clear();
            self.display_items.clear();
            self.selected = 0;
            self.scroll_offset = 0;
            self.files_searched = 0;
            self.files_with_matches = 0;
            return;
        }

        self.results.clear();
        self.display_items.clear();
        self.selected = 0;
        self.scroll_offset = 0;
        self.files_searched = 0;
        self.files_with_matches = 0;
        self.searching = true;

        let (tx, rx) = mpsc::channel();
        let cancel = Arc::new(AtomicBool::new(false));
        self.result_rx = Some(rx);
        self.cancel_flag = Some(Arc::clone(&cancel));

        let params = SearchParams {
            root: root.to_path_buf(),
            query: self.query.clone(),
            case_sensitive: self.case_sensitive,
            regex_mode: self.regex_mode,
            include_pattern: self.include_pattern.clone(),
            exclude_pattern: self.exclude_pattern.clone(),
            tx,
            cancel,
        };

        std::thread::spawn(move || {
            run_search(params);
        });
    }

    /// Drains available results from the background thread channel.
    ///
    /// Call this each frame from the main loop to progressively populate results.
    pub fn drain_results(&mut self) {
        let Some(ref rx) = self.result_rx else {
            return;
        };

        let mut got_results = false;
        loop {
            match rx.try_recv() {
                Ok(SearchEvent::Results(batch)) => {
                    self.results.extend(batch);
                    got_results = true;
                }
                Ok(SearchEvent::Progress { files_searched }) => {
                    self.files_searched = files_searched;
                }
                Ok(SearchEvent::Done) => {
                    self.searching = false;
                    self.result_rx = None;
                    self.cancel_flag = None;
                    got_results = true;
                    break;
                }
                Err(mpsc::TryRecvError::Empty) => break,
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.searching = false;
                    self.result_rx = None;
                    self.cancel_flag = None;
                    break;
                }
            }
        }

        if got_results {
            self.rebuild_display_items();
        }
    }

    /// Rebuilds the flattened display list from the current results,
    /// grouping matches by file path.
    pub fn rebuild_display_items(&mut self) {
        self.display_items.clear();
        self.files_with_matches = 0;

        if self.results.is_empty() {
            return;
        }

        // Group by relative_path, preserving order of first occurrence.
        let mut file_groups: Vec<(String, Vec<usize>)> = Vec::new();
        let mut file_index_map: std::collections::HashMap<String, usize> =
            std::collections::HashMap::new();

        for (i, result) in self.results.iter().enumerate() {
            if let Some(&group_idx) = file_index_map.get(&result.relative_path) {
                file_groups[group_idx].1.push(i);
            } else {
                let group_idx = file_groups.len();
                file_index_map.insert(result.relative_path.clone(), group_idx);
                file_groups.push((result.relative_path.clone(), vec![i]));
            }
        }

        self.files_with_matches = file_groups.len();

        for (path, indices) in &file_groups {
            self.display_items.push(DisplayItem::FileHeader {
                relative_path: path.clone(),
                match_count: indices.len(),
            });
            for &idx in indices {
                self.display_items
                    .push(DisplayItem::MatchLine { result_index: idx });
            }
        }

        // Clamp selection to valid range.
        if self.selected >= self.display_items.len() {
            self.selected = self.display_items.len().saturating_sub(1);
        }
    }
}

/// Parameters for a background search operation.
struct SearchParams {
    root: PathBuf,
    query: String,
    case_sensitive: bool,
    regex_mode: bool,
    include_pattern: String,
    exclude_pattern: String,
    tx: mpsc::Sender<SearchEvent>,
    cancel: Arc<AtomicBool>,
}

/// Runs the search in a background thread.
///
/// Walks the project directory respecting `.gitignore`, applies include/exclude
/// patterns, reads each file line-by-line, and sends matching results in batches.
fn run_search(params: SearchParams) {
    let SearchParams {
        root,
        query,
        case_sensitive,
        regex_mode,
        include_pattern,
        exclude_pattern,
        tx,
        cancel,
    } = params;
    // Build the regex pattern for matching.
    let pattern = if regex_mode {
        if case_sensitive {
            Regex::new(&query)
        } else {
            Regex::new(&format!("(?i){}", &query))
        }
    } else {
        let escaped = regex::escape(&query);
        if case_sensitive {
            Regex::new(&escaped)
        } else {
            Regex::new(&format!("(?i){}", &escaped))
        }
    };

    let Ok(pattern) = pattern else {
        let _ = tx.send(SearchEvent::Done);
        return;
    };

    // Build the file walker with optional include/exclude overrides.
    let mut walk_builder = WalkBuilder::new(&root);
    walk_builder.hidden(true).git_ignore(true);

    // Apply include/exclude patterns via overrides.
    let mut override_builder = OverrideBuilder::new(&root);
    let mut has_overrides = false;

    for pat in include_pattern.split_whitespace() {
        if !pat.is_empty() && override_builder.add(pat).is_ok() {
            has_overrides = true;
        }
    }

    for pat in exclude_pattern.split_whitespace() {
        if !pat.is_empty() {
            let negated = format!("!{pat}");
            if override_builder.add(&negated).is_ok() {
                has_overrides = true;
            }
        }
    }

    if has_overrides {
        if let Ok(overrides) = override_builder.build() {
            walk_builder.overrides(overrides);
        }
    }

    let walker = walk_builder.build();
    let mut batch: Vec<ProjectSearchMatch> = Vec::with_capacity(BATCH_SIZE);
    let mut total_results: usize = 0;
    let mut files_searched: usize = 0;

    for entry in walker {
        if cancel.load(Ordering::Relaxed) {
            break;
        }

        let Ok(entry) = entry else { continue };

        // Skip directories and non-file entries.
        if !entry.file_type().is_some_and(|ft| ft.is_file()) {
            continue;
        }

        let path = entry.path();

        // Skip large files.
        if let Ok(metadata) = path.metadata() {
            if metadata.len() > MAX_FILE_SIZE {
                continue;
            }
        }

        // Read the file and check for binary content.
        let Ok(file) = std::fs::File::open(path) else {
            continue;
        };
        let reader = BufReader::new(file);
        let mut is_first_chunk = true;
        let relative_path = path
            .strip_prefix(&root)
            .unwrap_or(path)
            .to_string_lossy()
            .to_string();

        for (line_idx, line_result) in reader.lines().enumerate() {
            if cancel.load(Ordering::Relaxed) {
                break;
            }

            let Ok(line) = line_result else { break };

            // Check for binary content in the first chunk.
            if is_first_chunk {
                is_first_chunk = false;
                let check_len = line.len().min(BINARY_CHECK_SIZE);
                if line.as_bytes()[..check_len].contains(&0) {
                    break;
                }
            }

            if let Some(m) = pattern.find(&line) {
                let start = m.start();
                let end = m.end();
                batch.push(ProjectSearchMatch {
                    relative_path: relative_path.clone(),
                    absolute_path: path.to_path_buf(),
                    line_number: line_idx + 1,
                    line_text: line,
                    match_start: start,
                    match_end: end,
                });
                total_results += 1;

                if batch.len() >= BATCH_SIZE {
                    if tx
                        .send(SearchEvent::Results(std::mem::take(&mut batch)))
                        .is_err()
                    {
                        return;
                    }
                    batch = Vec::with_capacity(BATCH_SIZE);
                }

                if total_results >= MAX_RESULTS {
                    break;
                }
            }
        }

        files_searched += 1;

        // Send progress periodically.
        if files_searched.is_multiple_of(100) {
            let _ = tx.send(SearchEvent::Progress { files_searched });
        }

        if total_results >= MAX_RESULTS {
            break;
        }
    }

    // Send remaining batch.
    if !batch.is_empty() {
        let _ = tx.send(SearchEvent::Results(batch));
    }
    let _ = tx.send(SearchEvent::Progress { files_searched });
    let _ = tx.send(SearchEvent::Done);
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn new_creates_empty_state() {
        let ps = ProjectSearch::new();
        assert!(ps.query.is_empty());
        assert!(!ps.case_sensitive);
        assert!(!ps.regex_mode);
        assert!(ps.include_pattern.is_empty());
        assert!(ps.exclude_pattern.is_empty());
        assert_eq!(ps.active_field, SearchField::Query);
        assert!(ps.results.is_empty());
        assert!(ps.display_items.is_empty());
        assert_eq!(ps.selected, 0);
        assert!(!ps.searching);
    }

    #[test]
    fn input_char_appends_to_query() {
        let mut ps = ProjectSearch::new();
        ps.input_char('h');
        ps.input_char('i');
        assert_eq!(ps.query, "hi");
    }

    #[test]
    fn input_char_appends_to_include_when_active() {
        let mut ps = ProjectSearch::new();
        ps.active_field = SearchField::Include;
        ps.input_char('*');
        assert_eq!(ps.include_pattern, "*");
        assert!(ps.query.is_empty());
    }

    #[test]
    fn input_char_appends_to_exclude_when_active() {
        let mut ps = ProjectSearch::new();
        ps.active_field = SearchField::Exclude;
        ps.input_char('!');
        assert_eq!(ps.exclude_pattern, "!");
    }

    #[test]
    fn input_backspace_removes_from_query() {
        let mut ps = ProjectSearch::new();
        ps.query = "abc".to_string();
        ps.input_backspace();
        assert_eq!(ps.query, "ab");
    }

    #[test]
    fn input_backspace_on_empty_query_is_noop() {
        let mut ps = ProjectSearch::new();
        ps.input_backspace();
        assert!(ps.query.is_empty());
    }

    #[test]
    fn cycle_field_rotates() {
        let mut ps = ProjectSearch::new();
        assert_eq!(ps.active_field, SearchField::Query);
        ps.cycle_field();
        assert_eq!(ps.active_field, SearchField::Include);
        ps.cycle_field();
        assert_eq!(ps.active_field, SearchField::Exclude);
        ps.cycle_field();
        assert_eq!(ps.active_field, SearchField::Query);
    }

    #[test]
    fn toggle_case_flips() {
        let mut ps = ProjectSearch::new();
        assert!(!ps.case_sensitive);
        ps.toggle_case();
        assert!(ps.case_sensitive);
        ps.toggle_case();
        assert!(!ps.case_sensitive);
    }

    #[test]
    fn toggle_regex_flips() {
        let mut ps = ProjectSearch::new();
        assert!(!ps.regex_mode);
        ps.toggle_regex();
        assert!(ps.regex_mode);
        ps.toggle_regex();
        assert!(!ps.regex_mode);
    }

    #[test]
    fn move_down_advances() {
        let mut ps = ProjectSearch::new();
        ps.display_items = vec![
            DisplayItem::FileHeader {
                relative_path: "a.rs".to_string(),
                match_count: 1,
            },
            DisplayItem::MatchLine { result_index: 0 },
        ];
        assert_eq!(ps.selected, 0);
        ps.move_down();
        assert_eq!(ps.selected, 1);
    }

    #[test]
    fn move_down_wraps() {
        let mut ps = ProjectSearch::new();
        ps.display_items = vec![
            DisplayItem::FileHeader {
                relative_path: "a.rs".to_string(),
                match_count: 1,
            },
            DisplayItem::MatchLine { result_index: 0 },
        ];
        ps.selected = 1;
        ps.move_down();
        assert_eq!(ps.selected, 0);
    }

    #[test]
    fn move_up_wraps() {
        let mut ps = ProjectSearch::new();
        ps.display_items = vec![
            DisplayItem::FileHeader {
                relative_path: "a.rs".to_string(),
                match_count: 1,
            },
            DisplayItem::MatchLine { result_index: 0 },
        ];
        assert_eq!(ps.selected, 0);
        ps.move_up();
        assert_eq!(ps.selected, 1);
    }

    #[test]
    fn move_on_empty_noop() {
        let mut ps = ProjectSearch::new();
        ps.move_up();
        ps.move_down();
        assert_eq!(ps.selected, 0);
    }

    #[test]
    fn selected_result_none_when_empty() {
        let ps = ProjectSearch::new();
        assert!(ps.selected_result().is_none());
    }

    #[test]
    fn selected_result_none_on_file_header() {
        let mut ps = ProjectSearch::new();
        ps.results.push(ProjectSearchMatch {
            relative_path: "a.rs".to_string(),
            absolute_path: PathBuf::from("/a.rs"),
            line_number: 1,
            line_text: "hello".to_string(),
            match_start: 0,
            match_end: 5,
        });
        ps.display_items = vec![
            DisplayItem::FileHeader {
                relative_path: "a.rs".to_string(),
                match_count: 1,
            },
            DisplayItem::MatchLine { result_index: 0 },
        ];
        ps.selected = 0; // FileHeader
        assert!(ps.selected_result().is_none());
    }

    #[test]
    fn selected_result_returns_match() {
        let mut ps = ProjectSearch::new();
        ps.results.push(ProjectSearchMatch {
            relative_path: "a.rs".to_string(),
            absolute_path: PathBuf::from("/a.rs"),
            line_number: 42,
            line_text: "hello world".to_string(),
            match_start: 0,
            match_end: 5,
        });
        ps.display_items = vec![
            DisplayItem::FileHeader {
                relative_path: "a.rs".to_string(),
                match_count: 1,
            },
            DisplayItem::MatchLine { result_index: 0 },
        ];
        ps.selected = 1; // MatchLine
        let result = ps.selected_result().unwrap();
        assert_eq!(result.line_number, 42);
    }

    #[test]
    fn rebuild_display_items_groups_by_file() {
        let mut ps = ProjectSearch::new();
        ps.results = vec![
            ProjectSearchMatch {
                relative_path: "a.rs".to_string(),
                absolute_path: PathBuf::from("/a.rs"),
                line_number: 1,
                line_text: "line1".to_string(),
                match_start: 0,
                match_end: 5,
            },
            ProjectSearchMatch {
                relative_path: "b.rs".to_string(),
                absolute_path: PathBuf::from("/b.rs"),
                line_number: 2,
                line_text: "line2".to_string(),
                match_start: 0,
                match_end: 5,
            },
            ProjectSearchMatch {
                relative_path: "a.rs".to_string(),
                absolute_path: PathBuf::from("/a.rs"),
                line_number: 5,
                line_text: "line5".to_string(),
                match_start: 0,
                match_end: 5,
            },
        ];
        ps.rebuild_display_items();

        assert_eq!(ps.files_with_matches, 2);
        assert_eq!(ps.display_items.len(), 5); // 2 headers + 3 match lines

        assert_eq!(
            ps.display_items[0],
            DisplayItem::FileHeader {
                relative_path: "a.rs".to_string(),
                match_count: 2,
            }
        );
        assert_eq!(
            ps.display_items[1],
            DisplayItem::MatchLine { result_index: 0 }
        );
        assert_eq!(
            ps.display_items[2],
            DisplayItem::MatchLine { result_index: 2 }
        );
        assert_eq!(
            ps.display_items[3],
            DisplayItem::FileHeader {
                relative_path: "b.rs".to_string(),
                match_count: 1,
            }
        );
        assert_eq!(
            ps.display_items[4],
            DisplayItem::MatchLine { result_index: 1 }
        );
    }

    #[test]
    fn cancel_search_sets_flag() {
        let mut ps = ProjectSearch::new();
        let flag = Arc::new(AtomicBool::new(false));
        ps.cancel_flag = Some(Arc::clone(&flag));
        ps.searching = true;

        ps.cancel_search();

        assert!(flag.load(Ordering::Relaxed));
        assert!(!ps.searching);
    }

    // --- Background search integration tests ---

    fn create_test_dir() -> TempDir {
        let dir = TempDir::new().unwrap();
        fs::write(
            dir.path().join("hello.rs"),
            "fn main() {\n    println!(\"hello world\");\n}\n",
        )
        .unwrap();
        fs::write(dir.path().join("test.txt"), "Hello World\nhello again\n").unwrap();
        fs::create_dir_all(dir.path().join("sub")).unwrap();
        fs::write(dir.path().join("sub/nested.rs"), "// hello from nested\n").unwrap();
        dir
    }

    fn search_sync(
        root: &Path,
        query: &str,
        case_sensitive: bool,
        regex_mode: bool,
        include: &str,
        exclude: &str,
    ) -> Vec<ProjectSearchMatch> {
        let (tx, rx) = mpsc::channel();
        let cancel = Arc::new(AtomicBool::new(false));

        run_search(SearchParams {
            root: root.to_path_buf(),
            query: query.to_string(),
            case_sensitive,
            regex_mode,
            include_pattern: include.to_string(),
            exclude_pattern: exclude.to_string(),
            tx,
            cancel,
        });

        let mut results = Vec::new();
        while let Ok(event) = rx.recv() {
            match event {
                SearchEvent::Results(batch) => results.extend(batch),
                SearchEvent::Done => break,
                SearchEvent::Progress { .. } => {}
            }
        }
        results
    }

    #[test]
    fn search_finds_literal_match() {
        let dir = create_test_dir();
        let results = search_sync(dir.path(), "hello", false, false, "", "");
        assert!(
            results.len() >= 3,
            "Expected at least 3 matches for 'hello', got {}",
            results.len()
        );
    }

    #[test]
    fn search_finds_regex_match() {
        let dir = create_test_dir();
        let results = search_sync(dir.path(), r"fn\s+main", false, true, "", "");
        assert!(!results.is_empty(), "Expected regex match for 'fn\\s+main'");
        assert!(results[0].line_text.contains("fn main"));
    }

    #[test]
    fn search_case_sensitive() {
        let dir = create_test_dir();
        // Case-sensitive: "Hello" should match fewer lines than case-insensitive "hello"
        let sensitive = search_sync(dir.path(), "Hello", true, false, "", "");
        let insensitive = search_sync(dir.path(), "Hello", false, false, "", "");
        assert!(
            insensitive.len() >= sensitive.len(),
            "Case-insensitive should find >= case-sensitive matches"
        );
    }

    #[test]
    fn search_include_pattern() {
        let dir = create_test_dir();
        let results = search_sync(dir.path(), "hello", false, false, "*.rs", "");
        // Should only match .rs files, not .txt
        for r in &results {
            assert!(
                r.relative_path.ends_with(".rs"),
                "Expected .rs file, got: {}",
                r.relative_path
            );
        }
        assert!(!results.is_empty());
    }

    #[test]
    fn search_exclude_pattern() {
        let dir = create_test_dir();
        let results = search_sync(dir.path(), "hello", false, false, "", "*.txt");
        // Should not match .txt files
        for r in &results {
            assert!(
                !r.relative_path.ends_with(".txt"),
                "Should not match .txt files, got: {}",
                r.relative_path
            );
        }
    }

    #[test]
    fn search_cancellation() {
        let dir = create_test_dir();
        let (tx, rx) = mpsc::channel();
        let cancel = Arc::new(AtomicBool::new(true)); // Cancel immediately

        run_search(SearchParams {
            root: dir.path().to_path_buf(),
            query: "hello".to_string(),
            case_sensitive: false,
            regex_mode: false,
            include_pattern: String::new(),
            exclude_pattern: String::new(),
            tx,
            cancel,
        });

        // Should complete quickly with Done and potentially no results.
        let mut got_done = false;
        while let Ok(event) = rx.recv() {
            if matches!(event, SearchEvent::Done) {
                got_done = true;
                break;
            }
        }
        assert!(got_done);
    }

    #[test]
    fn search_respects_gitignore() {
        let dir = TempDir::new().unwrap();
        // Initialize a git repo so .gitignore is respected.
        fs::write(dir.path().join(".gitignore"), "ignored.txt\n").unwrap();
        fs::write(dir.path().join("included.rs"), "hello\n").unwrap();
        fs::write(dir.path().join("ignored.txt"), "hello\n").unwrap();

        // Initialize git repo for ignore to work.
        std::process::Command::new("git")
            .args(["init"])
            .current_dir(dir.path())
            .output()
            .ok();

        let results = search_sync(dir.path(), "hello", false, false, "", "");
        let paths: Vec<&str> = results.iter().map(|r| r.relative_path.as_str()).collect();
        assert!(paths.contains(&"included.rs"), "Should find included.rs");
        assert!(
            !paths.contains(&"ignored.txt"),
            "Should not find ignored.txt (gitignored)"
        );
    }

    #[test]
    fn search_skips_large_files() {
        let dir = TempDir::new().unwrap();
        // Create a file just over the 1MB limit.
        let large_content = "hello\n".repeat(200_000); // ~1.2MB
        fs::write(dir.path().join("large.txt"), &large_content).unwrap();
        fs::write(dir.path().join("small.txt"), "hello\n").unwrap();

        let results = search_sync(dir.path(), "hello", false, false, "", "");
        let paths: Vec<&str> = results.iter().map(|r| r.relative_path.as_str()).collect();
        assert!(!paths.contains(&"large.txt"), "Should skip large file");
        assert!(paths.contains(&"small.txt"), "Should find small file");
    }

    #[test]
    fn drain_results_appends_incoming() {
        let mut ps = ProjectSearch::new();
        let (tx, rx) = mpsc::channel();
        ps.result_rx = Some(rx);
        ps.searching = true;

        tx.send(SearchEvent::Results(vec![ProjectSearchMatch {
            relative_path: "a.rs".to_string(),
            absolute_path: PathBuf::from("/a.rs"),
            line_number: 1,
            line_text: "hello".to_string(),
            match_start: 0,
            match_end: 5,
        }]))
        .unwrap();
        tx.send(SearchEvent::Done).unwrap();

        ps.drain_results();

        assert_eq!(ps.results.len(), 1);
        assert!(!ps.searching);
        assert!(!ps.display_items.is_empty());
    }

    #[test]
    fn start_search_with_empty_query_clears() {
        let mut ps = ProjectSearch::new();
        ps.results.push(ProjectSearchMatch {
            relative_path: "a.rs".to_string(),
            absolute_path: PathBuf::from("/a.rs"),
            line_number: 1,
            line_text: "hello".to_string(),
            match_start: 0,
            match_end: 5,
        });
        ps.rebuild_display_items();
        assert!(!ps.results.is_empty());

        ps.start_search(Path::new("/tmp"));
        assert!(ps.results.is_empty());
        assert!(ps.display_items.is_empty());
        assert!(!ps.searching);
    }
}
