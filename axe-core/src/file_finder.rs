// IMPACT ANALYSIS — FileFinder
// Parents: KeyEvent(Ctrl+P) -> Command::OpenFileFinder -> AppState::execute()
// Children: Command::OpenFile(path) when Enter pressed on selected item
// Siblings: show_help (CloseOverlay priority), confirm_dialog (higher priority),
//           SearchState (independent, different overlay), ResizeModeState (lower priority)
// Risk: Must intercept keys before editor/tree/terminal handlers but after confirm_dialog

use std::path::{Path, PathBuf};

use nucleo_matcher::pattern::{AtomKind, CaseMatching, Normalization, Pattern};
use nucleo_matcher::{Matcher, Utf32Str};

use crate::fuzzy::{FilteredItem, MAX_RESULTS};

/// Default number of items to skip when using PageUp/PageDown in the file finder.
pub const FILE_FINDER_PAGE_SIZE: usize = 10;

/// A single file entry in the finder.
#[derive(Debug, Clone)]
pub struct FileFinderItem {
    /// Relative path displayed to the user.
    pub relative_path: String,
    /// Absolute path used for opening the file.
    pub absolute_path: PathBuf,
}

/// Fuzzy file finder state.
///
/// Walks the project directory (respecting `.gitignore`), stores all file paths,
/// and provides real-time fuzzy matching via `nucleo-matcher`.
pub struct FileFinder {
    /// Current search query text.
    pub query: String,
    /// All project files collected on creation.
    pub items: Vec<FileFinderItem>,
    /// Fuzzy-matched results sorted by score (best first).
    pub filtered: Vec<FilteredItem>,
    /// Index of the selected item within `filtered`.
    pub selected: usize,
    /// Scroll offset for rendering the results list.
    pub scroll_offset: usize,
}

impl FileFinder {
    /// Creates a new `FileFinder` by walking the filesystem under `root`.
    ///
    /// Uses `ignore::WalkBuilder` to respect `.gitignore` rules.
    /// Files are stored as relative paths from `root`.
    pub fn new(root: &Path) -> Self {
        let mut items = Vec::new();

        let walker = ignore::WalkBuilder::new(root)
            .hidden(false)
            .git_ignore(true)
            .git_global(true)
            .git_exclude(true)
            .build();

        for entry in walker.flatten() {
            if !entry.file_type().is_some_and(|ft| ft.is_file()) {
                continue;
            }
            let abs = entry.into_path();
            // Skip VCS internals (.git directory contents).
            if abs.components().any(|c| c.as_os_str() == ".git") {
                continue;
            }
            if let Ok(rel) = abs.strip_prefix(root) {
                let relative_path = rel.to_string_lossy().to_string();
                items.push(FileFinderItem {
                    relative_path,
                    absolute_path: abs,
                });
            }
        }

        items.sort_by(|a, b| a.relative_path.cmp(&b.relative_path));

        // Start with all files shown (empty query matches everything).
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
                    let haystack = Utf32Str::new(&item.relative_path, &mut buf);
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

        // Reset selection when results change.
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

    /// Returns the absolute path of the currently selected item, if any.
    pub fn selected_path(&self) -> Option<&Path> {
        let filtered_item = self.filtered.get(self.selected)?;
        let item = self.items.get(filtered_item.index)?;
        Some(&item.absolute_path)
    }

    /// Returns the total number of files indexed.
    pub fn total_files(&self) -> usize {
        self.items.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    /// Helper to create a temp dir with given file paths.
    fn setup_temp_dir(files: &[&str]) -> TempDir {
        let dir = TempDir::new().expect("create temp dir");
        for file in files {
            let path = dir.path().join(file);
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).expect("create parent dirs");
            }
            fs::write(&path, "").expect("create file");
        }
        dir
    }

    #[test]
    fn new_collects_files_from_temp_dir() {
        let dir = setup_temp_dir(&["a.txt", "b.txt", "src/main.rs"]);
        let finder = FileFinder::new(dir.path());
        assert_eq!(finder.items.len(), 3);
    }

    #[test]
    fn new_stores_relative_paths() {
        let dir = setup_temp_dir(&["src/main.rs"]);
        let finder = FileFinder::new(dir.path());
        assert_eq!(finder.items.len(), 1);
        assert_eq!(finder.items[0].relative_path, "src/main.rs");
    }

    #[test]
    fn new_stores_absolute_paths() {
        let dir = setup_temp_dir(&["hello.txt"]);
        let finder = FileFinder::new(dir.path());
        assert!(finder.items[0].absolute_path.is_absolute());
        assert!(finder.items[0].absolute_path.ends_with("hello.txt"));
    }

    #[test]
    fn empty_query_shows_all_files() {
        let dir = setup_temp_dir(&["a.txt", "b.txt", "c.txt"]);
        let finder = FileFinder::new(dir.path());
        assert_eq!(finder.filtered.len(), 3);
    }

    #[test]
    fn update_matches_filters_by_query() {
        let dir = setup_temp_dir(&["src/main.rs", "src/lib.rs", "Cargo.toml"]);
        let mut finder = FileFinder::new(dir.path());
        finder.query = "main".to_string();
        finder.update_matches();
        assert_eq!(finder.filtered.len(), 1);
        let matched_item = &finder.items[finder.filtered[0].index];
        assert_eq!(matched_item.relative_path, "src/main.rs");
    }

    #[test]
    fn update_matches_returns_scores() {
        let dir = setup_temp_dir(&["src/main.rs", "src/lib.rs"]);
        let mut finder = FileFinder::new(dir.path());
        finder.query = "main".to_string();
        finder.update_matches();
        assert!(!finder.filtered.is_empty());
        // Score should be positive for a match.
        assert!(finder.filtered[0].score > 0);
    }

    #[test]
    fn update_matches_provides_match_indices() {
        let dir = setup_temp_dir(&["src/main.rs"]);
        let mut finder = FileFinder::new(dir.path());
        finder.query = "main".to_string();
        finder.update_matches();
        assert!(!finder.filtered.is_empty());
        // "src/main.rs" — "main" starts at index 4
        let indices = &finder.filtered[0].match_indices;
        assert!(!indices.is_empty());
        // Should contain indices 4,5,6,7 for "main"
        assert!(indices.contains(&4));
        assert!(indices.contains(&5));
        assert!(indices.contains(&6));
        assert!(indices.contains(&7));
    }

    #[test]
    fn update_matches_sorts_by_score_descending() {
        let dir = setup_temp_dir(&[
            "src/main.rs",
            "src/lib.rs",
            "tests/main_test.rs",
            "main.txt",
        ]);
        let mut finder = FileFinder::new(dir.path());
        finder.query = "main".to_string();
        finder.update_matches();
        // All matched items should be sorted by score descending.
        for window in finder.filtered.windows(2) {
            assert!(window[0].score >= window[1].score);
        }
    }

    #[test]
    fn input_char_appends_and_rematches() {
        let dir = setup_temp_dir(&["src/main.rs", "src/lib.rs"]);
        let mut finder = FileFinder::new(dir.path());
        finder.input_char('m');
        assert_eq!(finder.query, "m");
        // "main.rs" should match "m", "lib.rs" might also match
        // At minimum, the results should be re-computed.
        assert!(finder.filtered.len() <= finder.items.len());
    }

    #[test]
    fn input_backspace_removes_and_rematches() {
        let dir = setup_temp_dir(&["src/main.rs", "src/lib.rs"]);
        let mut finder = FileFinder::new(dir.path());
        finder.input_char('m');
        finder.input_char('a');
        let filtered_after_ma = finder.filtered.len();
        finder.input_backspace();
        assert_eq!(finder.query, "m");
        // After removing a char, we should get more (or equal) results.
        assert!(finder.filtered.len() >= filtered_after_ma);
    }

    #[test]
    fn input_backspace_on_empty_query_is_noop() {
        let dir = setup_temp_dir(&["a.txt"]);
        let mut finder = FileFinder::new(dir.path());
        finder.input_backspace();
        assert_eq!(finder.query, "");
        assert_eq!(finder.filtered.len(), 1);
    }

    #[test]
    fn move_down_advances_selection() {
        let dir = setup_temp_dir(&["a.txt", "b.txt", "c.txt"]);
        let mut finder = FileFinder::new(dir.path());
        assert_eq!(finder.selected, 0);
        finder.move_down();
        assert_eq!(finder.selected, 1);
    }

    #[test]
    fn move_down_wraps_to_start() {
        let dir = setup_temp_dir(&["a.txt", "b.txt"]);
        let mut finder = FileFinder::new(dir.path());
        finder.move_down(); // 0 -> 1
        finder.move_down(); // 1 -> 0 (wrap)
        assert_eq!(finder.selected, 0);
    }

    #[test]
    fn move_up_wraps_to_end() {
        let dir = setup_temp_dir(&["a.txt", "b.txt", "c.txt"]);
        let mut finder = FileFinder::new(dir.path());
        assert_eq!(finder.selected, 0);
        finder.move_up(); // wrap to 2
        assert_eq!(finder.selected, 2);
    }

    #[test]
    fn move_up_decrements_selection() {
        let dir = setup_temp_dir(&["a.txt", "b.txt", "c.txt"]);
        let mut finder = FileFinder::new(dir.path());
        finder.move_down(); // 0 -> 1
        finder.move_up(); // 1 -> 0
        assert_eq!(finder.selected, 0);
    }

    #[test]
    fn move_up_on_empty_results_is_noop() {
        let dir = setup_temp_dir(&["a.txt"]);
        let mut finder = FileFinder::new(dir.path());
        finder.query = "zzzzz_nonexistent".to_string();
        finder.update_matches();
        assert!(finder.filtered.is_empty());
        finder.move_up(); // Should not panic.
    }

    #[test]
    fn move_down_on_empty_results_is_noop() {
        let dir = setup_temp_dir(&["a.txt"]);
        let mut finder = FileFinder::new(dir.path());
        finder.query = "zzzzz_nonexistent".to_string();
        finder.update_matches();
        assert!(finder.filtered.is_empty());
        finder.move_down(); // Should not panic.
    }

    #[test]
    fn selected_path_returns_correct_path() {
        let dir = setup_temp_dir(&["src/main.rs"]);
        let finder = FileFinder::new(dir.path());
        let path = finder.selected_path().expect("should have selected path");
        assert!(path.ends_with("src/main.rs"));
        assert!(path.is_absolute());
    }

    #[test]
    fn selected_path_returns_none_when_no_results() {
        let dir = setup_temp_dir(&["a.txt"]);
        let mut finder = FileFinder::new(dir.path());
        finder.query = "zzzzz_nonexistent".to_string();
        finder.update_matches();
        assert!(finder.selected_path().is_none());
    }

    #[test]
    fn gitignored_files_excluded() {
        let dir = TempDir::new().expect("create temp dir");
        // Create .gitignore
        fs::write(dir.path().join(".gitignore"), "ignored.txt\n").expect("write gitignore");
        // Create files
        fs::write(dir.path().join("kept.txt"), "").expect("write kept");
        fs::write(dir.path().join("ignored.txt"), "").expect("write ignored");

        // Initialize git repo so .gitignore is respected.
        std::process::Command::new("git")
            .args(["init"])
            .current_dir(dir.path())
            .output()
            .expect("git init");

        let finder = FileFinder::new(dir.path());
        let paths: Vec<&str> = finder
            .items
            .iter()
            .map(|i| i.relative_path.as_str())
            .collect();
        assert!(paths.contains(&"kept.txt"), "kept.txt should be present");
        assert!(
            !paths.contains(&"ignored.txt"),
            "ignored.txt should be excluded by .gitignore"
        );
    }

    #[test]
    fn selection_resets_on_update_matches() {
        let dir = setup_temp_dir(&["a.txt", "b.txt", "c.txt"]);
        let mut finder = FileFinder::new(dir.path());
        finder.move_down();
        finder.move_down();
        assert_eq!(finder.selected, 2);
        finder.input_char('a');
        assert_eq!(
            finder.selected, 0,
            "Selection should reset after query change"
        );
    }

    #[test]
    fn total_files_returns_item_count() {
        let dir = setup_temp_dir(&["a.txt", "b.txt"]);
        let finder = FileFinder::new(dir.path());
        assert_eq!(finder.total_files(), 2);
    }

    #[test]
    fn fuzzy_matching_works_with_gaps() {
        let dir = setup_temp_dir(&["src/file_finder.rs", "src/lib.rs"]);
        let mut finder = FileFinder::new(dir.path());
        // "ffr" should fuzzy-match "file_finder.rs" (f-f-r with gaps)
        finder.query = "ffr".to_string();
        finder.update_matches();
        assert!(
            !finder.filtered.is_empty(),
            "Fuzzy match with gaps should find results"
        );
        let matched = &finder.items[finder.filtered[0].index];
        assert!(
            matched.relative_path.contains("file_finder"),
            "Should match file_finder.rs"
        );
    }

    #[test]
    fn empty_directory_produces_no_items() {
        let dir = TempDir::new().expect("create temp dir");
        let finder = FileFinder::new(dir.path());
        assert!(finder.items.is_empty());
        assert!(finder.filtered.is_empty());
    }

    #[test]
    fn git_directory_files_excluded() {
        let dir = TempDir::new().expect("create temp dir");
        // Create .git directory with internal files (simulating a git repo)
        fs::create_dir_all(dir.path().join(".git/objects")).expect("create .git/objects");
        fs::write(dir.path().join(".git/HEAD"), "ref: refs/heads/main\n").expect("write HEAD");
        fs::write(dir.path().join(".git/config"), "").expect("write git config");
        // Create a normal project file
        fs::write(dir.path().join("main.rs"), "").expect("write main.rs");

        let finder = FileFinder::new(dir.path());
        let paths: Vec<&str> = finder
            .items
            .iter()
            .map(|i| i.relative_path.as_str())
            .collect();
        assert!(
            paths.contains(&"main.rs"),
            "project files should be present"
        );
        assert!(
            !paths.iter().any(|p| p.contains(".git/")),
            "files inside .git/ should be excluded, but found: {:?}",
            paths
        );
    }

    #[test]
    fn dotfiles_are_included() {
        let dir = setup_temp_dir(&[".env", ".gitignore", "src/main.rs"]);
        let finder = FileFinder::new(dir.path());
        let paths: Vec<&str> = finder
            .items
            .iter()
            .map(|i| i.relative_path.as_str())
            .collect();
        assert!(paths.contains(&".env"), ".env should be included");
        assert!(
            paths.contains(&".gitignore"),
            ".gitignore should be included"
        );
    }

    #[test]
    fn move_page_down_advances_by_page_size() {
        let files: Vec<String> = (0..20).map(|i| format!("file{i:02}.txt")).collect();
        let file_refs: Vec<&str> = files.iter().map(|s| s.as_str()).collect();
        let dir = setup_temp_dir(&file_refs);
        let mut finder = FileFinder::new(dir.path());
        assert_eq!(finder.selected, 0);
        finder.move_page_down(5);
        assert_eq!(finder.selected, 5);
    }

    #[test]
    fn move_page_down_clamps_to_last() {
        let dir = setup_temp_dir(&["a.txt", "b.txt", "c.txt"]);
        let mut finder = FileFinder::new(dir.path());
        finder.move_page_down(10);
        assert_eq!(finder.selected, 2);
    }

    #[test]
    fn move_page_up_retreats_by_page_size() {
        let files: Vec<String> = (0..20).map(|i| format!("file{i:02}.txt")).collect();
        let file_refs: Vec<&str> = files.iter().map(|s| s.as_str()).collect();
        let dir = setup_temp_dir(&file_refs);
        let mut finder = FileFinder::new(dir.path());
        finder.selected = 10;
        finder.move_page_up(5);
        assert_eq!(finder.selected, 5);
    }

    #[test]
    fn move_page_up_clamps_to_zero() {
        let dir = setup_temp_dir(&["a.txt", "b.txt", "c.txt"]);
        let mut finder = FileFinder::new(dir.path());
        finder.selected = 2;
        finder.move_page_up(10);
        assert_eq!(finder.selected, 0);
    }

    #[test]
    fn move_page_down_on_empty_is_noop() {
        let dir = setup_temp_dir(&["a.txt"]);
        let mut finder = FileFinder::new(dir.path());
        finder.query = "zzzzz_nonexistent".to_string();
        finder.update_matches();
        assert!(finder.filtered.is_empty());
        finder.move_page_down(10); // Should not panic.
    }

    #[test]
    fn move_page_up_on_empty_is_noop() {
        let dir = setup_temp_dir(&["a.txt"]);
        let mut finder = FileFinder::new(dir.path());
        finder.query = "zzzzz_nonexistent".to_string();
        finder.update_matches();
        assert!(finder.filtered.is_empty());
        finder.move_page_up(10); // Should not panic.
    }
}
