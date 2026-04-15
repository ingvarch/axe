use std::collections::HashMap;
use std::path::PathBuf;

use serde_json::Value;

/// Kind of an inlay hint, mirroring `lsp_types::InlayHintKind`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InlayHintKind {
    /// A type hint (e.g. inferred variable type).
    Type,
    /// A parameter name hint at a call site.
    Parameter,
    /// Any other hint the server sends without a kind.
    Other,
}

/// A single inlay hint resolved to buffer coordinates.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InlayHint {
    /// Zero-based row in the buffer.
    pub row: usize,
    /// Zero-based column in the buffer (logical, not visual).
    pub col: usize,
    /// Rendered label (label parts joined if the server sent an array).
    pub label: String,
    /// Hint kind for styling.
    pub kind: InlayHintKind,
    /// Whether the server requested padding before the hint.
    pub padding_left: bool,
    /// Whether the server requested padding after the hint.
    pub padding_right: bool,
}

/// Inlay hints for a single buffer at a specific content version.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct InlayHintEntry {
    /// Version the hints were computed against.
    pub version: u64,
    /// Hints sorted by (row, col) ascending.
    pub hints: Vec<InlayHint>,
}

/// Stores inlay hints per open buffer, keyed by path.
#[derive(Debug, Clone, Default)]
pub struct InlayHintStore {
    entries: HashMap<PathBuf, InlayHintEntry>,
}

impl InlayHintStore {
    /// Creates an empty store.
    pub fn new() -> Self {
        Self::default()
    }

    /// Replaces the hints for `path` with `entry`, unless the entry is stale.
    ///
    /// A store already holding a newer version ignores older responses.
    pub fn set(&mut self, path: PathBuf, entry: InlayHintEntry) -> bool {
        if let Some(existing) = self.entries.get(&path) {
            if existing.version > entry.version {
                return false;
            }
        }
        self.entries.insert(path, entry);
        true
    }

    /// Returns the hints for a buffer, if any.
    pub fn get(&self, path: &std::path::Path) -> Option<&InlayHintEntry> {
        self.entries.get(path)
    }

    /// Returns the hints for a specific line, if any.
    ///
    /// Hints are filtered to the given row, keeping input order.
    pub fn hints_for_line(&self, path: &std::path::Path, row: usize) -> Vec<&InlayHint> {
        self.entries
            .get(path)
            .map(|entry| entry.hints.iter().filter(|h| h.row == row).collect())
            .unwrap_or_default()
    }

    /// Drops all hints for the given path (e.g. when the buffer is closed).
    pub fn forget(&mut self, path: &std::path::Path) {
        self.entries.remove(path);
    }
}

/// Parses a `textDocument/inlayHint` response into [`InlayHint`]s.
///
/// Returns an empty vector for `null`, a non-array, or malformed entries.
/// Label arrays are flattened by joining each part's `value`.
pub fn parse_inlay_hint_response(value: &Value) -> Vec<InlayHint> {
    let Some(array) = value.as_array() else {
        return Vec::new();
    };

    let mut hints = Vec::with_capacity(array.len());
    for entry in array {
        let Some(position) = entry.get("position") else {
            continue;
        };
        let Some(row) = position.get("line").and_then(Value::as_u64) else {
            continue;
        };
        let Some(col) = position.get("character").and_then(Value::as_u64) else {
            continue;
        };

        let label = match entry.get("label") {
            Some(Value::String(s)) => s.clone(),
            Some(Value::Array(parts)) => parts
                .iter()
                .filter_map(|part| part.get("value").and_then(Value::as_str))
                .collect::<String>(),
            _ => continue,
        };
        if label.is_empty() {
            continue;
        }

        let kind = match entry.get("kind").and_then(Value::as_u64) {
            Some(1) => InlayHintKind::Type,
            Some(2) => InlayHintKind::Parameter,
            _ => InlayHintKind::Other,
        };

        let padding_left = entry
            .get("paddingLeft")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let padding_right = entry
            .get("paddingRight")
            .and_then(Value::as_bool)
            .unwrap_or(false);

        hints.push(InlayHint {
            row: row as usize,
            col: col as usize,
            label,
            kind,
            padding_left,
            padding_right,
        });
    }

    hints.sort_by(|a, b| a.row.cmp(&b.row).then_with(|| a.col.cmp(&b.col)));
    hints
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parse_null_returns_empty() {
        assert!(parse_inlay_hint_response(&Value::Null).is_empty());
    }

    #[test]
    fn parse_string_label() {
        let value = json!([
            {
                "position": { "line": 0, "character": 5 },
                "label": "i32",
                "kind": 1,
                "paddingLeft": true,
                "paddingRight": false,
            }
        ]);
        let hints = parse_inlay_hint_response(&value);
        assert_eq!(hints.len(), 1);
        assert_eq!(hints[0].row, 0);
        assert_eq!(hints[0].col, 5);
        assert_eq!(hints[0].label, "i32");
        assert_eq!(hints[0].kind, InlayHintKind::Type);
        assert!(hints[0].padding_left);
        assert!(!hints[0].padding_right);
    }

    #[test]
    fn parse_array_label_joins_parts() {
        let value = json!([
            {
                "position": { "line": 1, "character": 3 },
                "label": [
                    { "value": "Vec<" },
                    { "value": "i32" },
                    { "value": ">" }
                ],
                "kind": 1,
            }
        ]);
        let hints = parse_inlay_hint_response(&value);
        assert_eq!(hints.len(), 1);
        assert_eq!(hints[0].label, "Vec<i32>");
    }

    #[test]
    fn parse_parameter_kind() {
        let value = json!([
            {
                "position": { "line": 2, "character": 10 },
                "label": "name:",
                "kind": 2,
            }
        ]);
        let hints = parse_inlay_hint_response(&value);
        assert_eq!(hints[0].kind, InlayHintKind::Parameter);
    }

    #[test]
    fn parse_unknown_kind_defaults_to_other() {
        let value = json!([
            {
                "position": { "line": 0, "character": 0 },
                "label": "?",
            }
        ]);
        let hints = parse_inlay_hint_response(&value);
        assert_eq!(hints[0].kind, InlayHintKind::Other);
    }

    #[test]
    fn parse_skips_malformed_entries() {
        let value = json!([
            { "position": { "line": 0, "character": 0 }, "label": "ok" },
            { "label": "missing position" },
            { "position": { "character": 0 }, "label": "missing line" },
            { "position": { "line": 0, "character": 0 }, "label": "" },
        ]);
        let hints = parse_inlay_hint_response(&value);
        assert_eq!(hints.len(), 1);
        assert_eq!(hints[0].label, "ok");
    }

    #[test]
    fn parse_sorts_by_position() {
        let value = json!([
            { "position": { "line": 2, "character": 0 }, "label": "b" },
            { "position": { "line": 0, "character": 5 }, "label": "a" },
            { "position": { "line": 0, "character": 1 }, "label": "c" },
        ]);
        let hints = parse_inlay_hint_response(&value);
        assert_eq!(hints.len(), 3);
        assert_eq!(hints[0].label, "c");
        assert_eq!(hints[1].label, "a");
        assert_eq!(hints[2].label, "b");
    }

    #[test]
    fn store_set_replaces_existing() {
        let mut store = InlayHintStore::new();
        let path = PathBuf::from("/tmp/test.rs");
        store.set(
            path.clone(),
            InlayHintEntry {
                version: 1,
                hints: vec![hint(0, 0, "a")],
            },
        );
        assert!(store.set(
            path.clone(),
            InlayHintEntry {
                version: 2,
                hints: vec![hint(1, 0, "b")],
            },
        ));
        assert_eq!(store.get(&path).unwrap().version, 2);
        assert_eq!(store.get(&path).unwrap().hints.len(), 1);
        assert_eq!(store.get(&path).unwrap().hints[0].label, "b");
    }

    #[test]
    fn store_rejects_stale_response() {
        let mut store = InlayHintStore::new();
        let path = PathBuf::from("/tmp/test.rs");
        store.set(
            path.clone(),
            InlayHintEntry {
                version: 5,
                hints: vec![hint(0, 0, "new")],
            },
        );
        let ok = store.set(
            path.clone(),
            InlayHintEntry {
                version: 3,
                hints: vec![hint(0, 0, "stale")],
            },
        );
        assert!(!ok);
        assert_eq!(store.get(&path).unwrap().version, 5);
        assert_eq!(store.get(&path).unwrap().hints[0].label, "new");
    }

    #[test]
    fn store_hints_for_line_filters() {
        let mut store = InlayHintStore::new();
        let path = PathBuf::from("/tmp/test.rs");
        store.set(
            path.clone(),
            InlayHintEntry {
                version: 1,
                hints: vec![hint(0, 1, "a"), hint(1, 3, "b"), hint(1, 5, "c")],
            },
        );
        let row0 = store.hints_for_line(&path, 0);
        assert_eq!(row0.len(), 1);
        assert_eq!(row0[0].label, "a");
        let row1 = store.hints_for_line(&path, 1);
        assert_eq!(row1.len(), 2);
        assert_eq!(row1[0].label, "b");
        assert_eq!(row1[1].label, "c");
        assert!(store.hints_for_line(&path, 2).is_empty());
    }

    #[test]
    fn store_forget_drops_path() {
        let mut store = InlayHintStore::new();
        let path = PathBuf::from("/tmp/test.rs");
        store.set(
            path.clone(),
            InlayHintEntry {
                version: 1,
                hints: vec![hint(0, 0, "a")],
            },
        );
        store.forget(&path);
        assert!(store.get(&path).is_none());
    }

    fn hint(row: usize, col: usize, label: &str) -> InlayHint {
        InlayHint {
            row,
            col,
            label: label.to_string(),
            kind: InlayHintKind::Other,
            padding_left: false,
            padding_right: false,
        }
    }
}
