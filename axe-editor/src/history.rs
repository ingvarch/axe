use std::time::Instant;

use crate::cursor::CursorState;

/// Duration within which contiguous edits are merged into one undo group.
const EDIT_GROUP_TIMEOUT: std::time::Duration = std::time::Duration::from_millis(500);

/// A single atomic text change in the buffer.
#[derive(Debug, Clone)]
pub struct Edit {
    /// Char index in the rope where the change starts.
    pub char_idx: usize,
    /// Text that was removed (empty for pure insertions).
    pub old_text: String,
    /// Text that was inserted (empty for pure deletions).
    pub new_text: String,
}

/// A group of edits that form a single undo step.
#[derive(Debug, Clone)]
pub struct EditGroup {
    /// The edits in this group (applied in order).
    pub edits: Vec<Edit>,
    /// Cursor state before the first edit in this group.
    pub cursor_before: CursorState,
    /// Cursor state after the last edit in this group.
    pub cursor_after: CursorState,
    /// Timestamp of the last edit added to this group.
    pub timestamp: Instant,
    /// Optional human-readable label for the group (e.g. "Rename", "Code Action").
    /// Populated by multi-edit operations; ordinary typing leaves it as `None`.
    pub label: Option<String>,
}

/// Manages undo/redo stacks with time-based edit grouping.
pub struct EditHistory {
    undo_stack: Vec<EditGroup>,
    redo_stack: Vec<EditGroup>,
    /// When true, all new edits merge into the current group regardless
    /// of contiguity or timeout. Used for Replace All undo grouping.
    force_merge: bool,
    /// When true, the next edit always starts a new group even when
    /// `force_merge` is enabled. Cleared after that edit is recorded.
    force_new_group: bool,
    /// Label applied to the next new `EditGroup` (reset after it lands).
    pending_label: Option<String>,
}

impl EditHistory {
    /// Creates an empty history with no undo/redo entries.
    pub fn new() -> Self {
        Self {
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            force_merge: false,
            force_new_group: false,
            pending_label: None,
        }
    }

    /// Begins a new isolated labeled undo group.
    ///
    /// The next edit will start a brand-new group (unmergeable with any
    /// prior group) carrying the given label, and subsequent edits merge
    /// into it until [`set_force_merge(false)`] is called. Used by
    /// multi-edit operations like Rename and Code Actions that must appear
    /// as a single undo step distinct from surrounding typing.
    pub fn begin_isolated_group(&mut self, label: String) {
        self.force_merge = true;
        self.force_new_group = true;
        self.pending_label = Some(label);
    }

    /// Enables or disables forced merge mode.
    ///
    /// When enabled, all subsequent edits merge into the current undo group
    /// regardless of contiguity or timeout. Used by Replace All to create
    /// a single undo step for multiple replacements.
    pub fn set_force_merge(&mut self, enabled: bool) {
        self.force_merge = enabled;
    }

    /// Sets the label that will be attached to the next new `EditGroup`.
    ///
    /// Used by multi-edit operations (rename, code actions) that want their
    /// undo entry identifiable in the history — cleared automatically once
    /// the group has been opened.
    pub fn set_pending_label(&mut self, label: Option<String>) {
        self.pending_label = label;
    }

    /// Records an edit, merging it into the current group if it is
    /// contiguous and within the grouping timeout, otherwise starting
    /// a new group. Any new edit clears the redo stack.
    pub fn record(&mut self, edit: Edit, cursor_before: CursorState, cursor_after: CursorState) {
        let now = Instant::now();
        self.redo_stack.clear();

        let should_merge = !self.force_new_group
            && self.undo_stack.last().is_some_and(|group| {
                self.force_merge
                    || (now.duration_since(group.timestamp) < EDIT_GROUP_TIMEOUT
                        && is_contiguous(group.edits.last(), &edit))
            });

        if should_merge {
            let group = self.undo_stack.last_mut().expect("checked above");
            group.edits.push(edit);
            group.cursor_after = cursor_after;
            group.timestamp = now;
        } else {
            self.undo_stack.push(EditGroup {
                edits: vec![edit],
                cursor_before,
                cursor_after,
                timestamp: now,
                label: self.pending_label.take(),
            });
            self.force_new_group = false;
        }
    }

    /// Records an edit with a custom timestamp (for testing).
    #[cfg(test)]
    fn record_at(
        &mut self,
        edit: Edit,
        cursor_before: CursorState,
        cursor_after: CursorState,
        at: Instant,
    ) {
        self.redo_stack.clear();

        let should_merge = self.undo_stack.last().is_some_and(|group| {
            self.force_merge
                || (at.duration_since(group.timestamp) < EDIT_GROUP_TIMEOUT
                    && is_contiguous(group.edits.last(), &edit))
        });

        if should_merge {
            let group = self.undo_stack.last_mut().expect("checked above");
            group.edits.push(edit);
            group.cursor_after = cursor_after;
            group.timestamp = at;
        } else {
            self.undo_stack.push(EditGroup {
                edits: vec![edit],
                cursor_before,
                cursor_after,
                timestamp: at,
                label: self.pending_label.take(),
            });
        }
    }

    /// Pops the most recent undo group, pushes it to redo, and returns it.
    pub fn undo(&mut self) -> Option<EditGroup> {
        let group = self.undo_stack.pop()?;
        self.redo_stack.push(group.clone());
        Some(group)
    }

    /// Pops the most recent redo group, pushes it to undo, and returns it.
    pub fn redo(&mut self) -> Option<EditGroup> {
        let group = self.redo_stack.pop()?;
        self.undo_stack.push(group.clone());
        Some(group)
    }

    /// Returns true if there are entries to undo.
    pub fn can_undo(&self) -> bool {
        !self.undo_stack.is_empty()
    }

    /// Returns true if there are entries to redo.
    pub fn can_redo(&self) -> bool {
        !self.redo_stack.is_empty()
    }

    /// Clears all undo and redo history.
    pub fn clear(&mut self) {
        self.undo_stack.clear();
        self.redo_stack.clear();
    }
}

impl Default for EditHistory {
    fn default() -> Self {
        Self::new()
    }
}

/// Returns true if `new_edit` is contiguous with `prev_edit` (positions touch).
fn is_contiguous(prev: Option<&Edit>, new_edit: &Edit) -> bool {
    let Some(prev) = prev else {
        return false;
    };

    if !prev.new_text.is_empty() {
        // Previous was an insertion — new edit should start where previous ended.
        prev.char_idx + prev.new_text.chars().count() == new_edit.char_idx
    } else if !prev.old_text.is_empty() {
        // Previous was a deletion — new edit at same position (forward delete)
        // or one before (backspace).
        new_edit.char_idx == prev.char_idx || new_edit.char_idx + 1 == prev.char_idx
    } else {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cursor(row: usize, col: usize) -> CursorState {
        CursorState {
            row,
            col,
            desired_col: col,
        }
    }

    fn insert_edit(char_idx: usize, text: &str) -> Edit {
        Edit {
            char_idx,
            old_text: String::new(),
            new_text: text.to_string(),
        }
    }

    fn delete_edit(char_idx: usize, text: &str) -> Edit {
        Edit {
            char_idx,
            old_text: text.to_string(),
            new_text: String::new(),
        }
    }

    #[test]
    fn new_history_is_empty() {
        let history = EditHistory::new();
        assert!(!history.can_undo());
        assert!(!history.can_redo());
    }

    #[test]
    fn record_edit_creates_undo_entry() {
        let mut history = EditHistory::new();
        history.record(insert_edit(0, "a"), cursor(0, 0), cursor(0, 1));
        assert!(history.can_undo());
        assert!(!history.can_redo());
    }

    #[test]
    fn undo_returns_edit_group() {
        let mut history = EditHistory::new();
        history.record(insert_edit(0, "a"), cursor(0, 0), cursor(0, 1));
        let group = history.undo().unwrap();
        assert_eq!(group.edits.len(), 1);
        assert_eq!(group.edits[0].new_text, "a");
    }

    #[test]
    fn undo_then_redo() {
        let mut history = EditHistory::new();
        history.record(insert_edit(0, "a"), cursor(0, 0), cursor(0, 1));
        history.undo();
        assert!(history.can_redo());
        let group = history.redo().unwrap();
        assert_eq!(group.edits[0].new_text, "a");
        assert!(history.can_undo());
        assert!(!history.can_redo());
    }

    #[test]
    fn new_edit_clears_redo() {
        let mut history = EditHistory::new();
        history.record(insert_edit(0, "a"), cursor(0, 0), cursor(0, 1));
        history.undo();
        assert!(history.can_redo());
        history.record(insert_edit(0, "b"), cursor(0, 0), cursor(0, 1));
        assert!(!history.can_redo());
    }

    #[test]
    fn rapid_edits_grouped() {
        let mut history = EditHistory::new();
        let now = Instant::now();
        history.record_at(insert_edit(0, "a"), cursor(0, 0), cursor(0, 1), now);
        history.record_at(
            insert_edit(1, "b"),
            cursor(0, 1),
            cursor(0, 2),
            now + std::time::Duration::from_millis(100),
        );
        // Should be 1 group with 2 edits.
        assert_eq!(history.undo_stack.len(), 1);
        assert_eq!(history.undo_stack[0].edits.len(), 2);
    }

    #[test]
    fn slow_edits_separate_groups() {
        let mut history = EditHistory::new();
        let now = Instant::now();
        history.record_at(insert_edit(0, "a"), cursor(0, 0), cursor(0, 1), now);
        history.record_at(
            insert_edit(1, "b"),
            cursor(0, 1),
            cursor(0, 2),
            now + std::time::Duration::from_millis(600),
        );
        assert_eq!(history.undo_stack.len(), 2);
    }

    #[test]
    fn non_contiguous_edits_separate_groups() {
        let mut history = EditHistory::new();
        let now = Instant::now();
        history.record_at(insert_edit(0, "a"), cursor(0, 0), cursor(0, 1), now);
        // Non-contiguous: char_idx 10, not adjacent to previous (0 + 1 = 1).
        history.record_at(
            insert_edit(10, "b"),
            cursor(0, 10),
            cursor(0, 11),
            now + std::time::Duration::from_millis(100),
        );
        assert_eq!(history.undo_stack.len(), 2);
    }

    #[test]
    fn undo_empty_returns_none() {
        let mut history = EditHistory::new();
        assert!(history.undo().is_none());
    }

    #[test]
    fn redo_empty_returns_none() {
        let mut history = EditHistory::new();
        assert!(history.redo().is_none());
    }

    #[test]
    fn undo_restores_cursor_before() {
        let mut history = EditHistory::new();
        let before = cursor(5, 10);
        history.record(insert_edit(50, "x"), before.clone(), cursor(5, 11));
        let group = history.undo().unwrap();
        assert_eq!(group.cursor_before, before);
    }

    #[test]
    fn clear_empties_both_stacks() {
        let mut history = EditHistory::new();
        history.record(insert_edit(0, "a"), cursor(0, 0), cursor(0, 1));
        history.undo();
        assert!(history.can_redo());
        history.clear();
        assert!(!history.can_undo());
        assert!(!history.can_redo());
    }

    #[test]
    fn force_merge_groups_non_contiguous_edits() {
        let mut history = EditHistory::new();
        history.set_force_merge(true);
        let now = Instant::now();
        history.record_at(insert_edit(0, "a"), cursor(0, 0), cursor(0, 1), now);
        // Non-contiguous: char_idx 10, but force_merge is on.
        history.record_at(
            insert_edit(10, "b"),
            cursor(0, 10),
            cursor(0, 11),
            now + std::time::Duration::from_millis(100),
        );
        assert_eq!(history.undo_stack.len(), 1);
        assert_eq!(history.undo_stack[0].edits.len(), 2);
    }

    #[test]
    fn force_merge_disabled_creates_separate_groups() {
        let mut history = EditHistory::new();
        history.set_force_merge(true);
        let now = Instant::now();
        history.record_at(insert_edit(0, "a"), cursor(0, 0), cursor(0, 1), now);
        history.set_force_merge(false);
        history.record_at(
            insert_edit(10, "b"),
            cursor(0, 10),
            cursor(0, 11),
            now + std::time::Duration::from_millis(100),
        );
        assert_eq!(history.undo_stack.len(), 2);
    }

    #[test]
    fn begin_isolated_group_forces_new_group_and_label() {
        let mut history = EditHistory::new();
        // An unrelated prior group.
        history.record(insert_edit(0, "a"), cursor(0, 0), cursor(0, 1));
        assert_eq!(history.undo_stack.len(), 1);

        // Begin isolated labeled group — even though force_merge is set,
        // the next edit must land in a fresh group with the label.
        history.begin_isolated_group("Rename".to_string());
        history.record(insert_edit(10, "X"), cursor(0, 10), cursor(0, 11));
        assert_eq!(history.undo_stack.len(), 2);
        assert_eq!(history.undo_stack[1].label.as_deref(), Some("Rename"));

        // Subsequent edits merge into the isolated group (force_merge still on).
        history.record(insert_edit(20, "Y"), cursor(0, 20), cursor(0, 21));
        assert_eq!(history.undo_stack.len(), 2);
        assert_eq!(history.undo_stack[1].edits.len(), 2);
    }

    #[test]
    fn pending_label_lands_on_next_group() {
        let mut history = EditHistory::new();
        history.set_pending_label(Some("Rename".to_string()));
        history.record(insert_edit(0, "a"), cursor(0, 0), cursor(0, 1));
        assert_eq!(history.undo_stack[0].label.as_deref(), Some("Rename"));
        // Label is consumed — next group must not inherit it.
        history.record_at(
            insert_edit(10, "b"),
            cursor(0, 10),
            cursor(0, 11),
            Instant::now() + std::time::Duration::from_secs(10),
        );
        assert_eq!(history.undo_stack[1].label, None);
    }

    #[test]
    fn contiguous_deletions_grouped() {
        let mut history = EditHistory::new();
        let now = Instant::now();
        // Backspace at pos 5 (deletes char at 5, cursor was at 6)
        history.record_at(delete_edit(5, "c"), cursor(0, 6), cursor(0, 5), now);
        // Backspace at pos 4 (adjacent — 4 + 1 == 5)
        history.record_at(
            delete_edit(4, "b"),
            cursor(0, 5),
            cursor(0, 4),
            now + std::time::Duration::from_millis(50),
        );
        assert_eq!(history.undo_stack.len(), 1);
        assert_eq!(history.undo_stack[0].edits.len(), 2);
    }
}
