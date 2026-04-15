//! Multi-cursor container for a single editor buffer.
//!
//! `Cursors` holds one or more cursor positions plus a parallel vector of
//! optional selections — one per cursor. The container guarantees:
//!
//! - The list is never empty (there is always a primary cursor).
//! - Cursors are kept sorted by `(row, col)` ascending after `normalize()`.
//! - No two cursors share the same `(row, col)` — duplicates are dropped.
//! - `primary` always points to the caller-designated main cursor even as
//!   the list is mutated or sorted around it.
//!
//! This module exists on its own with no external consumers yet; the rest
//! of `axe-editor` still uses `EditorBuffer::cursor` / `selection` fields.
//! Later phase-C steps will migrate those fields to use `Cursors` directly.

use crate::cursor::CursorState;
use crate::selection::Selection;

/// A non-empty list of cursors and their matching optional selections.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cursors {
    /// Cursor positions, sorted ascending by `(row, col)` after normalization.
    cursors: Vec<CursorState>,
    /// Optional selection per cursor, parallel to `cursors`.
    selections: Vec<Option<Selection>>,
    /// Index of the primary (main) cursor inside `cursors`.
    primary: usize,
}

impl Cursors {
    /// Creates a new container with a single cursor and no selection.
    pub fn single(cursor: CursorState) -> Self {
        Self {
            cursors: vec![cursor],
            selections: vec![None],
            primary: 0,
        }
    }

    /// Creates a new container with a single cursor and a pre-existing selection.
    pub fn single_with_selection(cursor: CursorState, selection: Option<Selection>) -> Self {
        Self {
            cursors: vec![cursor],
            selections: vec![selection],
            primary: 0,
        }
    }

    /// Returns the primary cursor position.
    pub fn primary(&self) -> &CursorState {
        &self.cursors[self.primary]
    }

    /// Returns a mutable reference to the primary cursor position.
    pub fn primary_mut(&mut self) -> &mut CursorState {
        &mut self.cursors[self.primary]
    }

    /// Returns the selection attached to the primary cursor, if any.
    pub fn primary_selection(&self) -> Option<&Selection> {
        self.selections[self.primary].as_ref()
    }

    /// Returns a mutable reference to the primary cursor's selection slot.
    pub fn primary_selection_mut(&mut self) -> &mut Option<Selection> {
        &mut self.selections[self.primary]
    }

    /// Returns all cursors in sorted order.
    pub fn all(&self) -> &[CursorState] {
        &self.cursors
    }

    /// Returns the full list of selections, one per cursor.
    pub fn all_selections(&self) -> &[Option<Selection>] {
        &self.selections
    }

    /// Returns the number of cursors (always `>= 1`).
    pub fn len(&self) -> usize {
        self.cursors.len()
    }

    /// Always `false` by invariant — `Cursors` cannot be empty.
    ///
    /// Exists only so clippy's `len_without_is_empty` lint is satisfied.
    pub fn is_empty(&self) -> bool {
        false
    }

    /// Returns `true` when more than one cursor is active.
    pub fn has_secondaries(&self) -> bool {
        self.cursors.len() > 1
    }

    /// Returns the primary cursor's index inside the sorted list.
    pub fn primary_index(&self) -> usize {
        self.primary
    }

    /// Adds a new cursor with an optional selection.
    ///
    /// No-op if a cursor already exists at the same `(row, col)`. Keeps
    /// the list sorted and preserves the current primary.
    pub fn add(&mut self, cursor: CursorState, selection: Option<Selection>) {
        if self
            .cursors
            .iter()
            .any(|c| c.row == cursor.row && c.col == cursor.col)
        {
            return;
        }
        self.cursors.push(cursor);
        self.selections.push(selection);
        self.normalize();
    }

    /// Drops all non-primary cursors, leaving only the primary.
    pub fn clear_secondaries(&mut self) {
        let primary_cursor = self.cursors[self.primary].clone();
        let primary_selection = self.selections[self.primary].clone();
        self.cursors = vec![primary_cursor];
        self.selections = vec![primary_selection];
        self.primary = 0;
    }

    /// Replaces every cursor's position, keeping the same count.
    ///
    /// Calls `f(cursor, selection)` for each pair in the current (sorted)
    /// order. After the call, positions may be out of order — callers
    /// that need the invariant to hold should call [`normalize`] afterwards.
    pub fn map_in_place(&mut self, mut f: impl FnMut(&mut CursorState, &mut Option<Selection>)) {
        for (cursor, selection) in self.cursors.iter_mut().zip(self.selections.iter_mut()) {
            f(cursor, selection);
        }
    }

    /// Replaces the entire cursor set with the provided lists.
    ///
    /// The primary is set to `primary_idx` (or `0` if out of range), and
    /// the container is renormalised so the invariants hold. Panics if
    /// `cursors.is_empty()` or the two vectors have different lengths.
    ///
    /// Used by multi-cursor edit paths that compute fresh positions for
    /// every cursor in one pass.
    pub fn replace_with(
        &mut self,
        cursors: Vec<CursorState>,
        selections: Vec<Option<Selection>>,
        primary_idx: usize,
    ) {
        assert!(!cursors.is_empty(), "Cursors must remain non-empty");
        assert_eq!(
            cursors.len(),
            selections.len(),
            "cursor/selection lists must be parallel"
        );
        let primary = primary_idx.min(cursors.len() - 1);
        self.cursors = cursors;
        self.selections = selections;
        self.primary = primary;
        self.normalize();
    }

    /// Sorts cursors by `(row, col)` and drops duplicates, preserving the
    /// primary cursor's logical identity.
    ///
    /// The primary may land at a different index after sorting if the
    /// caller mutated positions in place. Its logical identity is
    /// preserved by tracking the pre-sort primary cursor value and
    /// relocating it after sorting.
    pub fn normalize(&mut self) {
        let primary_cursor = self.cursors[self.primary].clone();

        let mut pairs: Vec<(CursorState, Option<Selection>)> = self
            .cursors
            .drain(..)
            .zip(self.selections.drain(..))
            .collect();
        pairs.sort_by(|a, b| a.0.row.cmp(&b.0.row).then_with(|| a.0.col.cmp(&b.0.col)));
        pairs.dedup_by(|a, b| a.0.row == b.0.row && a.0.col == b.0.col);

        self.cursors = pairs.iter().map(|(c, _)| c.clone()).collect();
        self.selections = pairs.into_iter().map(|(_, s)| s).collect();

        self.primary = self
            .cursors
            .iter()
            .position(|c| c.row == primary_cursor.row && c.col == primary_cursor.col)
            .unwrap_or(0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cur(row: usize, col: usize) -> CursorState {
        CursorState {
            row,
            col,
            desired_col: col,
        }
    }

    fn sel(ar: usize, ac: usize) -> Selection {
        Selection {
            anchor_row: ar,
            anchor_col: ac,
        }
    }

    #[test]
    fn single_has_one_cursor() {
        let c = Cursors::single(cur(2, 5));
        assert_eq!(c.len(), 1);
        assert!(!c.has_secondaries());
        assert_eq!(c.primary(), &cur(2, 5));
        assert!(c.primary_selection().is_none());
    }

    #[test]
    fn single_is_never_empty() {
        let c = Cursors::single(cur(0, 0));
        assert!(!c.is_empty());
    }

    #[test]
    fn single_with_selection_keeps_selection() {
        let c = Cursors::single_with_selection(cur(1, 3), Some(sel(1, 0)));
        assert_eq!(c.primary_selection(), Some(&sel(1, 0)));
    }

    #[test]
    fn add_second_cursor_increments_len() {
        let mut c = Cursors::single(cur(0, 0));
        c.add(cur(5, 0), None);
        assert_eq!(c.len(), 2);
        assert!(c.has_secondaries());
    }

    #[test]
    fn add_duplicate_position_is_ignored() {
        let mut c = Cursors::single(cur(1, 2));
        c.add(cur(1, 2), None);
        assert_eq!(c.len(), 1);
    }

    #[test]
    fn add_sorts_by_row_then_col() {
        let mut c = Cursors::single(cur(5, 2));
        c.add(cur(3, 7), None);
        c.add(cur(5, 0), None);
        let all = c.all();
        assert_eq!(all.len(), 3);
        assert_eq!(all[0], cur(3, 7));
        assert_eq!(all[1], cur(5, 0));
        assert_eq!(all[2], cur(5, 2));
    }

    #[test]
    fn add_preserves_primary_after_sort() {
        let mut c = Cursors::single(cur(10, 0));
        // Primary is currently cur(10, 0) at index 0.
        c.add(cur(2, 0), None);
        c.add(cur(5, 0), None);
        // Primary should still point to cur(10, 0), now at index 2.
        assert_eq!(c.primary(), &cur(10, 0));
        assert_eq!(c.primary_index(), 2);
    }

    #[test]
    fn primary_selection_is_tied_to_primary_cursor() {
        let mut c = Cursors::single_with_selection(cur(0, 0), Some(sel(0, 0)));
        c.add(cur(5, 0), Some(sel(5, 5)));
        // Primary is still the first cursor; its selection must be the original.
        assert_eq!(c.primary_selection(), Some(&sel(0, 0)));
    }

    #[test]
    fn clear_secondaries_keeps_primary_alone() {
        let mut c = Cursors::single(cur(0, 0));
        c.add(cur(3, 0), None);
        c.add(cur(5, 0), None);
        c.clear_secondaries();
        assert_eq!(c.len(), 1);
        assert_eq!(c.primary(), &cur(0, 0));
    }

    #[test]
    fn clear_secondaries_preserves_primary_when_it_moved() {
        let mut c = Cursors::single(cur(10, 0));
        c.add(cur(2, 0), None);
        // primary is still cur(10,0) but now at index 1 after sort.
        assert_eq!(c.primary_index(), 1);
        c.clear_secondaries();
        assert_eq!(c.len(), 1);
        assert_eq!(c.primary(), &cur(10, 0));
    }

    #[test]
    fn map_in_place_visits_every_cursor() {
        let mut c = Cursors::single(cur(0, 0));
        c.add(cur(1, 0), None);
        c.add(cur(2, 0), None);
        c.map_in_place(|cursor, _| {
            cursor.col = 5;
            cursor.desired_col = 5;
        });
        assert!(c.all().iter().all(|cur| cur.col == 5));
    }

    #[test]
    fn map_in_place_can_touch_selection() {
        let mut c = Cursors::single(cur(0, 0));
        c.add(cur(5, 0), None);
        c.map_in_place(|cursor, selection| {
            *selection = Some(sel(cursor.row, cursor.col));
        });
        assert_eq!(c.all_selections()[0], Some(sel(0, 0)));
        assert_eq!(c.all_selections()[1], Some(sel(5, 0)));
    }

    #[test]
    fn normalize_sorts_after_in_place_mutation() {
        let mut c = Cursors::single(cur(0, 0));
        c.add(cur(5, 0), None);
        c.add(cur(10, 0), None);
        // Swap rows in place to create an unsorted state.
        c.map_in_place(|cursor, _| {
            if cursor.row == 0 {
                cursor.row = 20;
            }
        });
        c.normalize();
        // After normalize, cur at row 5, 10, 20 in that order.
        assert_eq!(c.all()[0].row, 5);
        assert_eq!(c.all()[1].row, 10);
        assert_eq!(c.all()[2].row, 20);
    }

    #[test]
    fn normalize_dedupes_overlapping_cursors() {
        let mut c = Cursors::single(cur(0, 0));
        c.add(cur(5, 0), None);
        // Force an overlap via map_in_place.
        c.map_in_place(|cursor, _| {
            if cursor.row == 5 {
                cursor.row = 0;
            }
        });
        c.normalize();
        assert_eq!(c.len(), 1);
    }

    #[test]
    fn normalize_preserves_primary_after_mutation() {
        let mut c = Cursors::single(cur(10, 0));
        c.add(cur(2, 0), None);
        c.add(cur(5, 0), None);
        // Primary is still cur(10,0). Move everyone by +1 row.
        c.map_in_place(|cursor, _| cursor.row += 1);
        c.normalize();
        // Primary cursor moved from (10,0) to (11,0).
        assert_eq!(c.primary(), &cur(11, 0));
    }

    #[test]
    fn primary_mut_mutates_primary_only() {
        let mut c = Cursors::single(cur(5, 5));
        c.add(cur(10, 0), None);
        // Primary starts as (5,5) at index 0.
        let primary = c.primary_mut();
        primary.col = 7;
        primary.desired_col = 7;
        // Primary identity is preserved (we didn't move it out of sort order).
        assert_eq!(c.primary(), &cur(5, 7));
    }

    #[test]
    fn replace_with_swaps_the_whole_set() {
        let mut c = Cursors::single(cur(0, 0));
        c.replace_with(
            vec![cur(1, 0), cur(3, 0), cur(5, 0)],
            vec![None, None, None],
            1,
        );
        assert_eq!(c.len(), 3);
        assert_eq!(c.primary(), &cur(3, 0));
    }

    #[test]
    fn replace_with_clamps_primary_idx() {
        let mut c = Cursors::single(cur(0, 0));
        c.replace_with(vec![cur(0, 0)], vec![None], 42);
        assert_eq!(c.primary_index(), 0);
    }

    #[test]
    fn primary_selection_mut_updates_only_primary_selection() {
        let mut c = Cursors::single(cur(0, 0));
        c.add(cur(5, 0), None);
        *c.primary_selection_mut() = Some(sel(0, 10));
        assert_eq!(c.primary_selection(), Some(&sel(0, 10)));
        assert_eq!(c.all_selections()[1], None);
    }
}
