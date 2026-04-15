use std::path::PathBuf;

use axe_editor::{FileEdit, TextEdit, WorkspaceEdit};
use serde_json::Value;
use url::Url;

/// State for the inline rename dialog.
///
/// The popup is anchored to the symbol under the cursor. The user types
/// a replacement name; Enter sends `textDocument/rename`, Esc cancels.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenameState {
    /// Row of the symbol being renamed (for popup anchoring and the request).
    pub origin_row: usize,
    /// Column of the cursor when rename was triggered.
    pub origin_col: usize,
    /// The new-name text buffer the user is editing.
    pub input: String,
    /// Caret position inside `input` (in chars).
    pub cursor: usize,
    /// File the rename request will be issued against.
    pub path: PathBuf,
}

impl RenameState {
    /// Creates a new rename state pre-filled with `initial` as the new name,
    /// caret placed at the end of the text.
    pub fn new(path: PathBuf, origin_row: usize, origin_col: usize, initial: String) -> Self {
        let cursor = initial.chars().count();
        Self {
            origin_row,
            origin_col,
            input: initial,
            cursor,
            path,
        }
    }

    /// Inserts a character at the caret and advances it.
    pub fn insert_char(&mut self, ch: char) {
        let byte_idx = self.byte_index_of_caret();
        self.input.insert(byte_idx, ch);
        self.cursor += 1;
    }

    /// Removes the character before the caret and steps it back.
    pub fn backspace(&mut self) {
        if self.cursor == 0 {
            return;
        }
        let end_byte = self.byte_index_of_caret();
        let prev_char = self.input[..end_byte]
            .chars()
            .next_back()
            .expect("cursor > 0 implies at least one preceding char");
        let prev_start = end_byte - prev_char.len_utf8();
        self.input.drain(prev_start..end_byte);
        self.cursor -= 1;
    }

    /// Returns `true` if the input is non-empty (safe to submit).
    pub fn is_submittable(&self) -> bool {
        !self.input.trim().is_empty()
    }

    fn byte_index_of_caret(&self) -> usize {
        self.input
            .char_indices()
            .nth(self.cursor)
            .map(|(i, _)| i)
            .unwrap_or(self.input.len())
    }
}

/// Parses an LSP `WorkspaceEdit` response from `textDocument/rename` into a
/// [`WorkspaceEdit`] usable by [`axe_editor::BufferManager::apply_workspace_edit`].
///
/// Supports both the `changes` map (URI → TextEdit[]) and the
/// `documentChanges` array (`TextDocumentEdit[]`). Returns `None` if the
/// response is null or malformed; returns an empty [`WorkspaceEdit`] if the
/// server explicitly sent no edits.
pub fn parse_workspace_edit_response(value: &Value) -> Option<WorkspaceEdit> {
    if value.is_null() {
        return None;
    }

    let mut files: Vec<FileEdit> = Vec::new();

    // `changes`: { uri: [TextEdit, ...], ... }
    if let Some(changes) = value.get("changes").and_then(Value::as_object) {
        for (uri_str, edits_value) in changes {
            let Some(path) = uri_string_to_path(uri_str) else {
                continue;
            };
            let Some(edits_array) = edits_value.as_array() else {
                continue;
            };
            let edits = parse_text_edits(edits_array);
            if !edits.is_empty() {
                files.push(FileEdit { path, edits });
            }
        }
    }

    // `documentChanges`: [{ textDocument: { uri, version }, edits: [...] }, ...]
    if let Some(doc_changes) = value.get("documentChanges").and_then(Value::as_array) {
        for entry in doc_changes {
            // Skip create/rename/delete file operations — we only honor edits.
            let Some(text_document) = entry.get("textDocument") else {
                continue;
            };
            let Some(uri_str) = text_document.get("uri").and_then(Value::as_str) else {
                continue;
            };
            let Some(path) = uri_string_to_path(uri_str) else {
                continue;
            };
            let Some(edits_array) = entry.get("edits").and_then(Value::as_array) else {
                continue;
            };
            let edits = parse_text_edits(edits_array);
            if !edits.is_empty() {
                files.push(FileEdit { path, edits });
            }
        }
    }

    Some(WorkspaceEdit { files })
}

fn parse_text_edits(array: &[Value]) -> Vec<TextEdit> {
    array
        .iter()
        .filter_map(|edit| {
            let range = edit.get("range")?;
            let start = range.get("start")?;
            let end = range.get("end")?;
            Some(TextEdit {
                start_line: start.get("line")?.as_u64()? as usize,
                start_col: start.get("character")?.as_u64()? as usize,
                end_line: end.get("line")?.as_u64()? as usize,
                end_col: end.get("character")?.as_u64()? as usize,
                new_text: edit.get("newText")?.as_str()?.to_string(),
            })
        })
        .collect()
}

fn uri_string_to_path(uri: &str) -> Option<PathBuf> {
    let url = Url::parse(uri).ok()?;
    if url.scheme() != "file" {
        return None;
    }
    url.to_file_path().ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parse_null_returns_none() {
        assert!(parse_workspace_edit_response(&Value::Null).is_none());
    }

    #[test]
    fn parse_empty_object_returns_empty_edit() {
        let edit = parse_workspace_edit_response(&json!({})).unwrap();
        assert!(edit.files.is_empty());
    }

    #[test]
    fn parse_changes_map_single_file() {
        let value = json!({
            "changes": {
                "file:///tmp/a.rs": [
                    {
                        "range": {
                            "start": { "line": 0, "character": 0 },
                            "end": { "line": 0, "character": 3 }
                        },
                        "newText": "foo"
                    }
                ]
            }
        });
        let edit = parse_workspace_edit_response(&value).unwrap();
        assert_eq!(edit.files.len(), 1);
        assert_eq!(edit.files[0].path, PathBuf::from("/tmp/a.rs"));
        assert_eq!(edit.files[0].edits.len(), 1);
        assert_eq!(edit.files[0].edits[0].new_text, "foo");
    }

    #[test]
    fn parse_document_changes_array() {
        let value = json!({
            "documentChanges": [
                {
                    "textDocument": { "uri": "file:///tmp/a.rs", "version": 1 },
                    "edits": [
                        {
                            "range": {
                                "start": { "line": 1, "character": 5 },
                                "end": { "line": 1, "character": 8 }
                            },
                            "newText": "bar"
                        }
                    ]
                }
            ]
        });
        let edit = parse_workspace_edit_response(&value).unwrap();
        assert_eq!(edit.files.len(), 1);
        assert_eq!(edit.files[0].edits[0].start_line, 1);
        assert_eq!(edit.files[0].edits[0].start_col, 5);
        assert_eq!(edit.files[0].edits[0].end_col, 8);
        assert_eq!(edit.files[0].edits[0].new_text, "bar");
    }

    #[test]
    fn parse_document_changes_skips_create_file_operations() {
        // `textDocument` is missing — entry is a create/delete op we should skip.
        let value = json!({
            "documentChanges": [
                { "kind": "create", "uri": "file:///tmp/new.rs" },
                {
                    "textDocument": { "uri": "file:///tmp/a.rs", "version": 1 },
                    "edits": [
                        {
                            "range": {
                                "start": { "line": 0, "character": 0 },
                                "end": { "line": 0, "character": 1 }
                            },
                            "newText": "X"
                        }
                    ]
                }
            ]
        });
        let edit = parse_workspace_edit_response(&value).unwrap();
        assert_eq!(edit.files.len(), 1);
        assert_eq!(edit.files[0].edits[0].new_text, "X");
    }

    #[test]
    fn parse_skips_non_file_uris() {
        let value = json!({
            "changes": {
                "untitled:Untitled-1": [
                    {
                        "range": {
                            "start": { "line": 0, "character": 0 },
                            "end": { "line": 0, "character": 1 }
                        },
                        "newText": "X"
                    }
                ]
            }
        });
        let edit = parse_workspace_edit_response(&value).unwrap();
        assert!(edit.files.is_empty());
    }

    #[test]
    fn rename_state_insert_char_appends() {
        let mut state = RenameState::new(PathBuf::from("/tmp/a.rs"), 0, 0, String::new());
        state.insert_char('a');
        state.insert_char('b');
        state.insert_char('c');
        assert_eq!(state.input, "abc");
        assert_eq!(state.cursor, 3);
    }

    #[test]
    fn rename_state_prefilled_caret_at_end() {
        let state = RenameState::new(PathBuf::from("/tmp/a.rs"), 2, 4, "hello".to_string());
        assert_eq!(state.cursor, 5);
        assert!(state.is_submittable());
    }

    #[test]
    fn rename_state_backspace_removes_last_char() {
        let mut state = RenameState::new(PathBuf::from("/tmp/a.rs"), 0, 0, "abc".to_string());
        state.backspace();
        assert_eq!(state.input, "ab");
        assert_eq!(state.cursor, 2);
    }

    #[test]
    fn rename_state_backspace_on_empty_is_noop() {
        let mut state = RenameState::new(PathBuf::from("/tmp/a.rs"), 0, 0, String::new());
        state.backspace();
        assert_eq!(state.input, "");
        assert_eq!(state.cursor, 0);
    }

    #[test]
    fn rename_state_empty_input_not_submittable() {
        let state = RenameState::new(PathBuf::from("/tmp/a.rs"), 0, 0, "   ".to_string());
        assert!(!state.is_submittable());
    }
}
