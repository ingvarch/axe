// IMPACT ANALYSIS — CompletionState
// Parents: LspEvent::CompletionResponse → parse_completion_response → CompletionState::new
// Children: UI renders popup from CompletionState, AppState applies selected item
// Siblings: Other overlays (command palette, file finder) — completion is non-modal unlike those

use serde_json::Value;

/// The kind of a completion item, mapped from LSP `CompletionItemKind`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompletionKind {
    Function,
    Method,
    Variable,
    Field,
    Type,
    Module,
    Keyword,
    Snippet,
    Constant,
    Property,
    Other,
}

/// A single completion suggestion from the language server.
#[derive(Debug, Clone)]
pub struct CompletionItem {
    pub label: String,
    pub kind: CompletionKind,
    pub detail: Option<String>,
    pub insert_text: Option<String>,
    pub sort_text: Option<String>,
    pub filter_text: Option<String>,
}

/// Tracks the state of an active completion popup.
pub struct CompletionState {
    pub items: Vec<CompletionItem>,
    /// Indices into `items` that pass the current filter.
    pub filtered: Vec<usize>,
    /// Index into `filtered` for the currently selected item.
    pub selected: usize,
    /// The text prefix typed since the trigger point.
    pub prefix: String,
    /// Row where completion was triggered.
    pub trigger_row: usize,
    /// Column where completion was triggered.
    pub trigger_col: usize,
    /// Scroll offset for the visible window of filtered items.
    pub scroll_offset: usize,
}

impl CompletionState {
    /// Creates a new completion state with all items visible.
    pub fn new(items: Vec<CompletionItem>, trigger_row: usize, trigger_col: usize) -> Self {
        let filtered: Vec<usize> = (0..items.len()).collect();
        Self {
            items,
            filtered,
            selected: 0,
            prefix: String::new(),
            trigger_row,
            trigger_col,
            scroll_offset: 0,
        }
    }

    /// Updates the filter based on the current prefix text.
    ///
    /// Performs case-insensitive prefix matching on `filter_text` (if present)
    /// or `label`. Resets selection to 0 if the current selection is out of bounds.
    pub fn update_filter(&mut self, prefix: &str) {
        self.prefix = prefix.to_string();
        let lower_prefix = prefix.to_lowercase();
        self.filtered = self
            .items
            .iter()
            .enumerate()
            .filter(|(_, item)| {
                let text = item.filter_text.as_deref().unwrap_or(&item.label);
                text.to_lowercase().starts_with(&lower_prefix)
            })
            .map(|(i, _)| i)
            .collect();
        if self.selected >= self.filtered.len() {
            self.selected = 0;
        }
        // Reset scroll when filter changes.
        self.scroll_offset = 0;
    }

    /// Moves the selection up, wrapping from first to last.
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

    /// Moves the selection down, wrapping from last to first.
    pub fn move_down(&mut self) {
        if self.filtered.is_empty() {
            return;
        }
        self.selected = (self.selected + 1) % self.filtered.len();
    }

    /// Returns the currently selected completion item, if any.
    pub fn selected_item(&self) -> Option<&CompletionItem> {
        self.filtered
            .get(self.selected)
            .and_then(|&idx| self.items.get(idx))
    }
}

/// Maps an LSP `CompletionItemKind` numeric value to our `CompletionKind`.
pub fn map_completion_kind(kind: u64) -> CompletionKind {
    match kind {
        1 => CompletionKind::Other,     // Text
        2 => CompletionKind::Method,    // Method
        3 => CompletionKind::Function,  // Function
        4 => CompletionKind::Function,  // Constructor
        5 => CompletionKind::Field,     // Field
        6 => CompletionKind::Variable,  // Variable
        7 => CompletionKind::Type,      // Class
        8 => CompletionKind::Type,      // Interface
        9 => CompletionKind::Module,    // Module
        10 => CompletionKind::Property, // Property
        11 => CompletionKind::Other,    // Unit
        12 => CompletionKind::Other,    // Value
        13 => CompletionKind::Type,     // Enum
        14 => CompletionKind::Keyword,  // Keyword
        15 => CompletionKind::Snippet,  // Snippet
        16 => CompletionKind::Other,    // Color
        17 => CompletionKind::Other,    // File
        18 => CompletionKind::Other,    // Reference
        19 => CompletionKind::Other,    // Folder
        20 => CompletionKind::Constant, // EnumMember
        21 => CompletionKind::Constant, // Constant
        22 => CompletionKind::Type,     // Struct
        23 => CompletionKind::Other,    // Event
        24 => CompletionKind::Other,    // Operator
        25 => CompletionKind::Other,    // TypeParameter
        _ => CompletionKind::Other,
    }
}

/// Parses an LSP completion response into a list of `CompletionItem`s.
///
/// Handles both `CompletionList { items: [...] }` and direct `[...]` array formats.
pub fn parse_completion_response(value: &Value) -> Vec<CompletionItem> {
    let items_array = if let Some(items) = value.get("items") {
        // CompletionList format: { "items": [...], "isIncomplete": ... }
        items
    } else if value.is_array() {
        // Direct array format: [...]
        value
    } else {
        return Vec::new();
    };

    let Some(arr) = items_array.as_array() else {
        return Vec::new();
    };

    arr.iter()
        .filter_map(|item| {
            let label = item.get("label")?.as_str()?.to_string();
            let kind = item
                .get("kind")
                .and_then(|k| k.as_u64())
                .map(map_completion_kind)
                .unwrap_or(CompletionKind::Other);
            let detail = item
                .get("detail")
                .and_then(|d| d.as_str())
                .map(String::from);
            let insert_text = item
                .get("insertText")
                .and_then(|t| t.as_str())
                .map(String::from);
            let sort_text = item
                .get("sortText")
                .and_then(|t| t.as_str())
                .map(String::from);
            let filter_text = item
                .get("filterText")
                .and_then(|t| t.as_str())
                .map(String::from);

            Some(CompletionItem {
                label,
                kind,
                detail,
                insert_text,
                sort_text,
                filter_text,
            })
        })
        .collect()
}

/// Returns a short icon/label string for a completion kind.
pub fn kind_icon(kind: CompletionKind) -> &'static str {
    match kind {
        CompletionKind::Function | CompletionKind::Method => "fn",
        CompletionKind::Variable => " v",
        CompletionKind::Field => " f",
        CompletionKind::Type => " T",
        CompletionKind::Module => " M",
        CompletionKind::Keyword => "kw",
        CompletionKind::Snippet => "sn",
        CompletionKind::Constant => " C",
        CompletionKind::Property => " p",
        CompletionKind::Other => "  ",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_completion_list() {
        let value = serde_json::json!({
            "isIncomplete": false,
            "items": [
                {"label": "println", "kind": 3, "detail": "macro"},
                {"label": "print", "kind": 3}
            ]
        });
        let items = parse_completion_response(&value);
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].label, "println");
        assert_eq!(items[0].kind, CompletionKind::Function);
        assert_eq!(items[0].detail.as_deref(), Some("macro"));
        assert_eq!(items[1].label, "print");
        assert!(items[1].detail.is_none());
    }

    #[test]
    fn parse_completion_array() {
        let value = serde_json::json!([
            {"label": "foo", "kind": 6},
            {"label": "bar", "kind": 5}
        ]);
        let items = parse_completion_response(&value);
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].label, "foo");
        assert_eq!(items[0].kind, CompletionKind::Variable);
        assert_eq!(items[1].label, "bar");
        assert_eq!(items[1].kind, CompletionKind::Field);
    }

    #[test]
    fn parse_completion_empty_object() {
        let value = serde_json::json!({});
        let items = parse_completion_response(&value);
        assert!(items.is_empty());
    }

    #[test]
    fn parse_completion_null() {
        let value = serde_json::Value::Null;
        let items = parse_completion_response(&value);
        assert!(items.is_empty());
    }

    #[test]
    fn parse_completion_with_insert_text() {
        let value = serde_json::json!([
            {"label": "println!()", "kind": 3, "insertText": "println!(\"$1\")"}
        ]);
        let items = parse_completion_response(&value);
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].insert_text.as_deref(), Some("println!(\"$1\")"));
    }

    #[test]
    fn filter_narrows() {
        let items = vec![
            CompletionItem {
                label: "println".to_string(),
                kind: CompletionKind::Function,
                detail: None,
                insert_text: None,
                sort_text: None,
                filter_text: None,
            },
            CompletionItem {
                label: "print".to_string(),
                kind: CompletionKind::Function,
                detail: None,
                insert_text: None,
                sort_text: None,
                filter_text: None,
            },
            CompletionItem {
                label: "format".to_string(),
                kind: CompletionKind::Function,
                detail: None,
                insert_text: None,
                sort_text: None,
                filter_text: None,
            },
        ];
        let mut state = CompletionState::new(items, 0, 0);
        assert_eq!(state.filtered.len(), 3);

        state.update_filter("pri");
        assert_eq!(state.filtered.len(), 2);
        assert_eq!(state.items[state.filtered[0]].label, "println");
        assert_eq!(state.items[state.filtered[1]].label, "print");
    }

    #[test]
    fn filter_case_insensitive() {
        let items = vec![CompletionItem {
            label: "String".to_string(),
            kind: CompletionKind::Type,
            detail: None,
            insert_text: None,
            sort_text: None,
            filter_text: None,
        }];
        let mut state = CompletionState::new(items, 0, 0);
        state.update_filter("str");
        assert_eq!(state.filtered.len(), 1);
    }

    #[test]
    fn filter_empty_dismisses() {
        let items = vec![CompletionItem {
            label: "foo".to_string(),
            kind: CompletionKind::Variable,
            detail: None,
            insert_text: None,
            sort_text: None,
            filter_text: None,
        }];
        let mut state = CompletionState::new(items, 0, 0);
        state.update_filter("xyz");
        assert!(state.filtered.is_empty());
    }

    #[test]
    fn filter_uses_filter_text_when_present() {
        let items = vec![CompletionItem {
            label: "Display Label".to_string(),
            kind: CompletionKind::Function,
            detail: None,
            insert_text: None,
            sort_text: None,
            filter_text: Some("actual_filter".to_string()),
        }];
        let mut state = CompletionState::new(items, 0, 0);
        state.update_filter("actual");
        assert_eq!(state.filtered.len(), 1);
        state.update_filter("Display");
        assert!(state.filtered.is_empty());
    }

    #[test]
    fn move_up_wraps() {
        let items = vec![
            CompletionItem {
                label: "a".to_string(),
                kind: CompletionKind::Other,
                detail: None,
                insert_text: None,
                sort_text: None,
                filter_text: None,
            },
            CompletionItem {
                label: "b".to_string(),
                kind: CompletionKind::Other,
                detail: None,
                insert_text: None,
                sort_text: None,
                filter_text: None,
            },
        ];
        let mut state = CompletionState::new(items, 0, 0);
        assert_eq!(state.selected, 0);
        state.move_up();
        assert_eq!(state.selected, 1);
        state.move_up();
        assert_eq!(state.selected, 0);
    }

    #[test]
    fn move_down_wraps() {
        let items = vec![
            CompletionItem {
                label: "a".to_string(),
                kind: CompletionKind::Other,
                detail: None,
                insert_text: None,
                sort_text: None,
                filter_text: None,
            },
            CompletionItem {
                label: "b".to_string(),
                kind: CompletionKind::Other,
                detail: None,
                insert_text: None,
                sort_text: None,
                filter_text: None,
            },
        ];
        let mut state = CompletionState::new(items, 0, 0);
        assert_eq!(state.selected, 0);
        state.move_down();
        assert_eq!(state.selected, 1);
        state.move_down();
        assert_eq!(state.selected, 0);
    }

    #[test]
    fn selected_item_correct() {
        let items = vec![
            CompletionItem {
                label: "first".to_string(),
                kind: CompletionKind::Variable,
                detail: None,
                insert_text: None,
                sort_text: None,
                filter_text: None,
            },
            CompletionItem {
                label: "second".to_string(),
                kind: CompletionKind::Function,
                detail: None,
                insert_text: None,
                sort_text: None,
                filter_text: None,
            },
        ];
        let mut state = CompletionState::new(items, 0, 0);
        assert_eq!(state.selected_item().unwrap().label, "first");
        state.move_down();
        assert_eq!(state.selected_item().unwrap().label, "second");
    }

    #[test]
    fn selected_item_none_when_empty() {
        let state = CompletionState::new(Vec::new(), 0, 0);
        assert!(state.selected_item().is_none());
    }

    #[test]
    fn map_kind_function() {
        assert_eq!(map_completion_kind(3), CompletionKind::Function);
    }

    #[test]
    fn map_kind_method() {
        assert_eq!(map_completion_kind(2), CompletionKind::Method);
    }

    #[test]
    fn map_kind_variable() {
        assert_eq!(map_completion_kind(6), CompletionKind::Variable);
    }

    #[test]
    fn map_kind_keyword() {
        assert_eq!(map_completion_kind(14), CompletionKind::Keyword);
    }

    #[test]
    fn map_kind_unknown() {
        assert_eq!(map_completion_kind(999), CompletionKind::Other);
    }

    #[test]
    fn kind_icon_strings() {
        assert_eq!(kind_icon(CompletionKind::Function), "fn");
        assert_eq!(kind_icon(CompletionKind::Method), "fn");
        assert_eq!(kind_icon(CompletionKind::Variable), " v");
        assert_eq!(kind_icon(CompletionKind::Type), " T");
        assert_eq!(kind_icon(CompletionKind::Module), " M");
        assert_eq!(kind_icon(CompletionKind::Keyword), "kw");
    }

    #[test]
    fn move_up_noop_on_empty() {
        let mut state = CompletionState::new(Vec::new(), 0, 0);
        state.move_up(); // Should not panic.
        assert_eq!(state.selected, 0);
    }

    #[test]
    fn move_down_noop_on_empty() {
        let mut state = CompletionState::new(Vec::new(), 0, 0);
        state.move_down(); // Should not panic.
        assert_eq!(state.selected, 0);
    }
}
