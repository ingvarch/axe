use axe_editor::WorkspaceEdit;
use serde_json::Value;

use crate::rename::parse_workspace_edit_response;

/// A command produced by the language server, to be executed via
/// `workspace/executeCommand`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LspCommand {
    pub command: String,
    pub arguments: Vec<Value>,
    pub title: Option<String>,
}

/// A single code action offered by the server.
///
/// Actions may carry an inline `WorkspaceEdit`, a `Command` to execute via
/// `workspace/executeCommand`, or both. When only a command is present and
/// no edit, applying the action means sending `workspace/executeCommand`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodeAction {
    /// Human-readable title displayed in the picker.
    pub title: String,
    /// LSP kind string (e.g. `"quickfix"`, `"refactor.extract"`) if any.
    pub kind: Option<String>,
    /// Inline workspace edit to apply, if present.
    pub edit: Option<WorkspaceEdit>,
    /// Server-side command to invoke, if present.
    pub command: Option<LspCommand>,
    /// Whether the server marked this action as preferred.
    pub is_preferred: bool,
    /// Whether the action is disabled (with an optional reason).
    pub disabled_reason: Option<String>,
}

impl CodeAction {
    /// Returns `true` if the action carries either an edit or a command the
    /// client can act on. Actions with neither are typically resolved via
    /// `codeAction/resolve`, which Axe does not request in v1.
    pub fn is_applicable(&self) -> bool {
        (self.edit.is_some() || self.command.is_some()) && self.disabled_reason.is_none()
    }
}

/// Picker state for the code-actions popup.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodeActionsState {
    pub actions: Vec<CodeAction>,
    /// Index of the currently highlighted action.
    pub selected: usize,
    /// Row the popup is anchored at (buffer coordinates).
    pub anchor_row: usize,
    /// Column the popup is anchored at (buffer coordinates).
    pub anchor_col: usize,
}

impl CodeActionsState {
    /// Creates a new state for the given action list anchored at the cursor.
    pub fn new(actions: Vec<CodeAction>, anchor_row: usize, anchor_col: usize) -> Self {
        Self {
            actions,
            selected: 0,
            anchor_row,
            anchor_col,
        }
    }

    /// Moves the highlight down one entry, wrapping at the end.
    pub fn select_next(&mut self) {
        if self.actions.is_empty() {
            return;
        }
        self.selected = (self.selected + 1) % self.actions.len();
    }

    /// Moves the highlight up one entry, wrapping at the start.
    pub fn select_prev(&mut self) {
        if self.actions.is_empty() {
            return;
        }
        if self.selected == 0 {
            self.selected = self.actions.len() - 1;
        } else {
            self.selected -= 1;
        }
    }

    /// Returns the currently highlighted action, if any.
    pub fn selected_action(&self) -> Option<&CodeAction> {
        self.actions.get(self.selected)
    }
}

/// Parses a `textDocument/codeAction` response into a flat list of
/// [`CodeAction`]s, honoring both legacy `Command[]` and modern
/// `CodeAction[]` shapes.
///
/// Returns an empty vector for `null` or a non-array response.
pub fn parse_code_actions_response(value: &Value) -> Vec<CodeAction> {
    let Some(array) = value.as_array() else {
        return Vec::new();
    };

    let mut actions = Vec::with_capacity(array.len());
    for entry in array {
        if let Some(action) = parse_entry(entry) {
            actions.push(action);
        }
    }
    actions
}

fn parse_entry(entry: &Value) -> Option<CodeAction> {
    // Legacy `Command` shape: { title, command, arguments? }.
    // Modern `CodeAction` shape: { title, kind?, edit?, command?, ... }.
    // Both have `title`; the command-only form has a top-level `command: string`.
    let title = entry.get("title").and_then(Value::as_str)?.to_string();

    let kind = entry
        .get("kind")
        .and_then(Value::as_str)
        .map(|s| s.to_string());

    let is_preferred = entry
        .get("isPreferred")
        .and_then(Value::as_bool)
        .unwrap_or(false);

    let disabled_reason = entry
        .get("disabled")
        .and_then(|d| d.get("reason"))
        .and_then(Value::as_str)
        .map(|s| s.to_string());

    let edit = entry
        .get("edit")
        .and_then(parse_workspace_edit_response)
        .filter(|e| !e.files.is_empty());

    // Command may be either a top-level string (legacy shape) or a nested
    // object `{ command, arguments?, title? }`.
    let command = parse_command(entry);

    Some(CodeAction {
        title,
        kind,
        edit,
        command,
        is_preferred,
        disabled_reason,
    })
}

fn parse_command(entry: &Value) -> Option<LspCommand> {
    // Modern shape: `command` is nested object.
    if let Some(cmd_obj) = entry.get("command").and_then(Value::as_object) {
        let command = cmd_obj.get("command").and_then(Value::as_str)?.to_string();
        let arguments = cmd_obj
            .get("arguments")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        let title = cmd_obj
            .get("title")
            .and_then(Value::as_str)
            .map(|s| s.to_string());
        return Some(LspCommand {
            command,
            arguments,
            title,
        });
    }

    // Legacy `Command` shape: command is a string on the entry itself.
    if let Some(cmd_str) = entry.get("command").and_then(Value::as_str) {
        let arguments = entry
            .get("arguments")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        return Some(LspCommand {
            command: cmd_str.to_string(),
            arguments,
            title: None,
        });
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parse_null_returns_empty() {
        assert!(parse_code_actions_response(&Value::Null).is_empty());
    }

    #[test]
    fn parse_non_array_returns_empty() {
        assert!(parse_code_actions_response(&json!({ "actions": [] })).is_empty());
    }

    #[test]
    fn parse_legacy_command_shape() {
        let value = json!([
            {
                "title": "Run tests",
                "command": "rust-analyzer.runTests",
                "arguments": [{"path": "/tmp/a.rs"}]
            }
        ]);
        let actions = parse_code_actions_response(&value);
        assert_eq!(actions.len(), 1);
        assert_eq!(actions[0].title, "Run tests");
        let cmd = actions[0].command.as_ref().unwrap();
        assert_eq!(cmd.command, "rust-analyzer.runTests");
        assert_eq!(cmd.arguments.len(), 1);
        assert!(actions[0].edit.is_none());
    }

    #[test]
    fn parse_modern_code_action_with_edit() {
        let value = json!([
            {
                "title": "Add missing import",
                "kind": "quickfix",
                "isPreferred": true,
                "edit": {
                    "changes": {
                        "file:///tmp/a.rs": [
                            {
                                "range": {
                                    "start": { "line": 0, "character": 0 },
                                    "end": { "line": 0, "character": 0 }
                                },
                                "newText": "use std::io;\n"
                            }
                        ]
                    }
                }
            }
        ]);
        let actions = parse_code_actions_response(&value);
        assert_eq!(actions.len(), 1);
        assert_eq!(actions[0].title, "Add missing import");
        assert_eq!(actions[0].kind.as_deref(), Some("quickfix"));
        assert!(actions[0].is_preferred);
        let edit = actions[0].edit.as_ref().unwrap();
        assert_eq!(edit.files.len(), 1);
        assert_eq!(edit.files[0].edits[0].new_text, "use std::io;\n");
    }

    #[test]
    fn parse_modern_code_action_with_nested_command() {
        let value = json!([
            {
                "title": "Open documentation",
                "command": {
                    "title": "Open",
                    "command": "rust-analyzer.openDocs",
                    "arguments": [{"uri": "file:///tmp/a.rs"}]
                }
            }
        ]);
        let actions = parse_code_actions_response(&value);
        assert_eq!(actions.len(), 1);
        let cmd = actions[0].command.as_ref().unwrap();
        assert_eq!(cmd.command, "rust-analyzer.openDocs");
        assert_eq!(cmd.title.as_deref(), Some("Open"));
    }

    #[test]
    fn parse_disabled_action_sets_reason() {
        let value = json!([
            {
                "title": "Apply fix",
                "disabled": { "reason": "No changes needed" }
            }
        ]);
        let actions = parse_code_actions_response(&value);
        assert_eq!(
            actions[0].disabled_reason.as_deref(),
            Some("No changes needed")
        );
        assert!(!actions[0].is_applicable());
    }

    #[test]
    fn parse_action_without_edit_or_command_is_not_applicable() {
        let value = json!([{ "title": "Placeholder" }]);
        let actions = parse_code_actions_response(&value);
        assert_eq!(actions.len(), 1);
        assert!(!actions[0].is_applicable());
    }

    #[test]
    fn parse_mixed_list() {
        let value = json!([
            { "title": "One", "command": "a.b" },
            { "title": "Two", "kind": "quickfix" },
            {
                "title": "Three",
                "command": { "command": "c.d", "arguments": [] }
            }
        ]);
        let actions = parse_code_actions_response(&value);
        assert_eq!(actions.len(), 3);
        assert_eq!(actions[0].command.as_ref().unwrap().command, "a.b");
        assert_eq!(actions[1].kind.as_deref(), Some("quickfix"));
        assert_eq!(actions[2].command.as_ref().unwrap().command, "c.d");
    }

    #[test]
    fn state_select_next_wraps() {
        let mut state = CodeActionsState::new(vec![action("a"), action("b"), action("c")], 0, 0);
        assert_eq!(state.selected, 0);
        state.select_next();
        assert_eq!(state.selected, 1);
        state.select_next();
        state.select_next();
        assert_eq!(state.selected, 0, "wraps to start");
    }

    #[test]
    fn state_select_prev_wraps() {
        let mut state = CodeActionsState::new(vec![action("a"), action("b")], 0, 0);
        state.select_prev();
        assert_eq!(state.selected, 1);
    }

    #[test]
    fn state_empty_select_is_noop() {
        let mut state = CodeActionsState::new(Vec::new(), 0, 0);
        state.select_next();
        state.select_prev();
        assert_eq!(state.selected, 0);
        assert!(state.selected_action().is_none());
    }

    fn action(title: &str) -> CodeAction {
        CodeAction {
            title: title.to_string(),
            kind: None,
            edit: None,
            command: None,
            is_preferred: false,
            disabled_reason: None,
        }
    }
}
