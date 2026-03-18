use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

use serde_json::Value;

/// A single location result from an LSP definition or references response.
pub struct LocationItem {
    /// Absolute path to the file.
    pub path: PathBuf,
    /// Display path relative to the project root.
    pub display_path: String,
    /// 0-based line number.
    pub line: usize,
    /// 0-based column number.
    pub col: usize,
    /// Content of the line at this location (trimmed).
    pub line_text: String,
}

/// A list of locations shown in an overlay for definition/references results.
pub struct LocationList {
    /// Title displayed in the overlay border (e.g., "Definition" or "References").
    pub title: String,
    /// The location items to display.
    pub items: Vec<LocationItem>,
    /// Currently selected item index.
    pub selected: usize,
    /// Scroll offset for the visible window.
    pub scroll_offset: usize,
}

impl LocationList {
    /// Creates a new location list with the given title and items.
    pub fn new(title: impl Into<String>, items: Vec<LocationItem>) -> Self {
        Self {
            title: title.into(),
            items,
            selected: 0,
            scroll_offset: 0,
        }
    }

    /// Moves selection up, wrapping to the last item.
    pub fn move_up(&mut self) {
        if self.items.is_empty() {
            return;
        }
        if self.selected == 0 {
            self.selected = self.items.len() - 1;
        } else {
            self.selected -= 1;
        }
    }

    /// Moves selection down, wrapping to the first item.
    pub fn move_down(&mut self) {
        if self.items.is_empty() {
            return;
        }
        if self.selected >= self.items.len() - 1 {
            self.selected = 0;
        } else {
            self.selected += 1;
        }
    }

    /// Returns the currently selected item, if any.
    pub fn selected_item(&self) -> Option<&LocationItem> {
        self.items.get(self.selected)
    }
}

/// Parses an LSP `textDocument/definition` response into location items.
///
/// Handles: single `Location`, `Location[]`, `LocationLink[]`, and `null`.
pub fn parse_definition_response(value: &Value, project_root: &Path) -> Vec<LocationItem> {
    if value.is_null() {
        return Vec::new();
    }

    // Single Location object: { "uri": ..., "range": ... }
    if value.is_object() && value.get("uri").is_some() {
        if let Some(item) = parse_location(value, project_root) {
            return vec![item];
        }
        return Vec::new();
    }

    // Array of Location or LocationLink
    if let Some(arr) = value.as_array() {
        return arr
            .iter()
            .filter_map(|v| {
                // LocationLink: has "targetUri" and "targetRange"
                if v.get("targetUri").is_some() {
                    parse_location_link(v, project_root)
                } else {
                    parse_location(v, project_root)
                }
            })
            .collect();
    }

    Vec::new()
}

/// Parses an LSP `textDocument/references` response into location items.
///
/// Handles: `Location[]` and `null`.
pub fn parse_references_response(value: &Value, project_root: &Path) -> Vec<LocationItem> {
    if value.is_null() {
        return Vec::new();
    }

    if let Some(arr) = value.as_array() {
        return arr
            .iter()
            .filter_map(|v| parse_location(v, project_root))
            .collect();
    }

    Vec::new()
}

/// Parses a single LSP `Location` object into a `LocationItem`.
fn parse_location(value: &Value, project_root: &Path) -> Option<LocationItem> {
    let uri = value.get("uri")?.as_str()?;
    let range = value.get("range")?;
    let start = range.get("start")?;
    let line = start.get("line")?.as_u64()? as usize;
    let col = start.get("character")?.as_u64()? as usize;

    let path = uri_to_path(uri)?;
    let display_path = make_display_path(&path, project_root);
    let line_text = read_line_from_file(&path, line);

    Some(LocationItem {
        path,
        display_path,
        line,
        col,
        line_text,
    })
}

/// Parses a single LSP `LocationLink` object into a `LocationItem`.
fn parse_location_link(value: &Value, project_root: &Path) -> Option<LocationItem> {
    let uri = value.get("targetUri")?.as_str()?;
    let range = value
        .get("targetSelectionRange")
        .or_else(|| value.get("targetRange"))?;
    let start = range.get("start")?;
    let line = start.get("line")?.as_u64()? as usize;
    let col = start.get("character")?.as_u64()? as usize;

    let path = uri_to_path(uri)?;
    let display_path = make_display_path(&path, project_root);
    let line_text = read_line_from_file(&path, line);

    Some(LocationItem {
        path,
        display_path,
        line,
        col,
        line_text,
    })
}

/// Converts a `file://` URI to a filesystem path.
fn uri_to_path(uri: &str) -> Option<PathBuf> {
    let url = url::Url::parse(uri).ok()?;
    url.to_file_path().ok()
}

/// Creates a display path relative to the project root.
fn make_display_path(path: &Path, project_root: &Path) -> String {
    path.strip_prefix(project_root)
        .unwrap_or(path)
        .to_string_lossy()
        .to_string()
}

/// Reads a specific 0-based line from a file on disk.
///
/// Returns the trimmed content, or an empty string if the line is out of bounds
/// or the file cannot be read.
pub fn read_line_from_file(path: &Path, line: usize) -> String {
    let Ok(file) = File::open(path) else {
        return String::new();
    };
    let reader = BufReader::new(file);
    reader
        .lines()
        .nth(line)
        .and_then(|r| r.ok())
        .map(|s| s.trim().to_string())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn parse_definition_single_location() {
        let root = Path::new("/tmp/project");
        let value = serde_json::json!({
            "uri": "file:///tmp/project/src/main.rs",
            "range": {
                "start": { "line": 10, "character": 5 },
                "end": { "line": 10, "character": 15 }
            }
        });
        let items = parse_definition_response(&value, root);
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].line, 10);
        assert_eq!(items[0].col, 5);
        assert_eq!(items[0].display_path, "src/main.rs");
    }

    #[test]
    fn parse_definition_array() {
        let root = Path::new("/tmp/project");
        let value = serde_json::json!([
            {
                "uri": "file:///tmp/project/src/a.rs",
                "range": { "start": { "line": 1, "character": 0 }, "end": { "line": 1, "character": 5 } }
            },
            {
                "uri": "file:///tmp/project/src/b.rs",
                "range": { "start": { "line": 2, "character": 3 }, "end": { "line": 2, "character": 8 } }
            }
        ]);
        let items = parse_definition_response(&value, root);
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].display_path, "src/a.rs");
        assert_eq!(items[1].display_path, "src/b.rs");
        assert_eq!(items[1].line, 2);
        assert_eq!(items[1].col, 3);
    }

    #[test]
    fn parse_definition_null() {
        let root = Path::new("/tmp/project");
        let items = parse_definition_response(&Value::Null, root);
        assert!(items.is_empty());
    }

    #[test]
    fn parse_definition_location_link() {
        let root = Path::new("/tmp/project");
        let value = serde_json::json!([{
            "targetUri": "file:///tmp/project/src/lib.rs",
            "targetRange": {
                "start": { "line": 5, "character": 0 },
                "end": { "line": 5, "character": 10 }
            },
            "targetSelectionRange": {
                "start": { "line": 5, "character": 4 },
                "end": { "line": 5, "character": 8 }
            }
        }]);
        let items = parse_definition_response(&value, root);
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].line, 5);
        // Should use targetSelectionRange's start col.
        assert_eq!(items[0].col, 4);
        assert_eq!(items[0].display_path, "src/lib.rs");
    }

    #[test]
    fn parse_references_array() {
        let root = Path::new("/tmp/project");
        let value = serde_json::json!([
            {
                "uri": "file:///tmp/project/src/main.rs",
                "range": { "start": { "line": 0, "character": 0 }, "end": { "line": 0, "character": 5 } }
            }
        ]);
        let items = parse_references_response(&value, root);
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].line, 0);
    }

    #[test]
    fn parse_references_null() {
        let root = Path::new("/tmp/project");
        let items = parse_references_response(&Value::Null, root);
        assert!(items.is_empty());
    }

    #[test]
    fn move_up_wraps() {
        let items = vec![
            LocationItem {
                path: PathBuf::from("/a.rs"),
                display_path: "a.rs".to_string(),
                line: 0,
                col: 0,
                line_text: String::new(),
            },
            LocationItem {
                path: PathBuf::from("/b.rs"),
                display_path: "b.rs".to_string(),
                line: 1,
                col: 0,
                line_text: String::new(),
            },
        ];
        let mut list = LocationList::new("Test", items);
        assert_eq!(list.selected, 0);
        list.move_up();
        assert_eq!(list.selected, 1);
        list.move_up();
        assert_eq!(list.selected, 0);
    }

    #[test]
    fn move_down_wraps() {
        let items = vec![
            LocationItem {
                path: PathBuf::from("/a.rs"),
                display_path: "a.rs".to_string(),
                line: 0,
                col: 0,
                line_text: String::new(),
            },
            LocationItem {
                path: PathBuf::from("/b.rs"),
                display_path: "b.rs".to_string(),
                line: 1,
                col: 0,
                line_text: String::new(),
            },
        ];
        let mut list = LocationList::new("Test", items);
        list.move_down();
        assert_eq!(list.selected, 1);
        list.move_down();
        assert_eq!(list.selected, 0);
    }

    #[test]
    fn move_on_empty_noop() {
        let mut list = LocationList::new("Test", Vec::new());
        list.move_up();
        assert_eq!(list.selected, 0);
        list.move_down();
        assert_eq!(list.selected, 0);
    }

    #[test]
    fn selected_item_correct() {
        let items = vec![
            LocationItem {
                path: PathBuf::from("/a.rs"),
                display_path: "a.rs".to_string(),
                line: 0,
                col: 0,
                line_text: String::new(),
            },
            LocationItem {
                path: PathBuf::from("/b.rs"),
                display_path: "b.rs".to_string(),
                line: 5,
                col: 3,
                line_text: String::new(),
            },
        ];
        let mut list = LocationList::new("Test", items);
        assert_eq!(list.selected_item().unwrap().line, 0);
        list.move_down();
        assert_eq!(list.selected_item().unwrap().line, 5);
    }

    #[test]
    fn selected_item_none_when_empty() {
        let list = LocationList::new("Test", Vec::new());
        assert!(list.selected_item().is_none());
    }

    #[test]
    fn read_line_from_file_valid() {
        let mut tmp = tempfile::NamedTempFile::new().expect("create temp file");
        writeln!(tmp, "line zero").expect("write");
        writeln!(tmp, "line one").expect("write");
        writeln!(tmp, "  line two  ").expect("write");
        let path = tmp.path();

        assert_eq!(read_line_from_file(path, 0), "line zero");
        assert_eq!(read_line_from_file(path, 1), "line one");
        assert_eq!(read_line_from_file(path, 2), "line two");
    }

    #[test]
    fn read_line_from_file_out_of_bounds() {
        let mut tmp = tempfile::NamedTempFile::new().expect("create temp file");
        writeln!(tmp, "only line").expect("write");
        let path = tmp.path();

        assert_eq!(read_line_from_file(path, 99), "");
    }
}
