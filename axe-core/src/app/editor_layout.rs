//! Layout of editor splits inside the main editor area.
//!
//! `EditorLayout` owns a flat, non-empty list of `Split`s plus a single
//! orientation (horizontal or vertical). Each split references a buffer
//! from the global [`axe_editor::BufferManager`] via an index; the
//! `BufferManager.active` field remains the live "which buffer is shown"
//! source of truth and is push-synced whenever the focused split changes
//! (see `AppState::set_focused_split`).
//!
//! Phase-E v1 deliberately supports only a flat layout (no nesting), a
//! single orientation, and equal-width tiling. Per-split resize and
//! nested splits are explicitly out of scope.

/// Direction in which splits are laid out.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SplitOrientation {
    /// Splits tiled side by side (multiple columns).
    #[default]
    Horizontal,
    /// Splits stacked (multiple rows).
    Vertical,
}

/// A single editor split.
///
/// Each split owns its own list of buffer indices (pointing into the
/// global `BufferManager`) plus an active index inside that list. This
/// gives every split its own independent tab bar in the VS Code style:
/// opening a file in one split does not affect the tab list of other
/// splits, and closing a tab inside a split only removes it from that
/// split — the buffer stays alive as long as some split references it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Split {
    /// Buffer indices shown in this split, in tab order.
    pub buffers: Vec<usize>,
    /// Index into `buffers` of the currently active tab within this split.
    pub active: usize,
}

impl Split {
    /// Creates a new split containing a single buffer.
    pub fn new(initial_buffer: usize) -> Self {
        Self {
            buffers: vec![initial_buffer],
            active: 0,
        }
    }

    /// Returns the global buffer index currently shown in the split, if any.
    pub fn active_buffer(&self) -> Option<usize> {
        self.buffers.get(self.active).copied()
    }

    /// Returns `true` when the split contains no buffers.
    pub fn is_empty(&self) -> bool {
        self.buffers.is_empty()
    }

    /// Adds `buffer_idx` to the end of the tab list and makes it active.
    ///
    /// If the buffer is already present, just switches to it without
    /// duplicating the tab.
    pub fn open_buffer(&mut self, buffer_idx: usize) {
        if let Some(pos) = self.buffers.iter().position(|&b| b == buffer_idx) {
            self.active = pos;
            return;
        }
        self.buffers.push(buffer_idx);
        self.active = self.buffers.len() - 1;
    }

    /// Removes the currently active tab and shifts focus to a neighbour.
    ///
    /// Returns the global buffer index that was removed, or `None` if the
    /// split was already empty.
    pub fn close_active(&mut self) -> Option<usize> {
        if self.buffers.is_empty() {
            return None;
        }
        let removed = self.buffers.remove(self.active);
        if self.active >= self.buffers.len() && !self.buffers.is_empty() {
            self.active = self.buffers.len() - 1;
        }
        Some(removed)
    }

    /// Cycles to the next tab, wrapping.
    pub fn next_tab(&mut self) {
        if self.buffers.len() <= 1 {
            return;
        }
        self.active = (self.active + 1) % self.buffers.len();
    }

    /// Cycles to the previous tab, wrapping.
    pub fn prev_tab(&mut self) {
        if self.buffers.len() <= 1 {
            return;
        }
        if self.active == 0 {
            self.active = self.buffers.len() - 1;
        } else {
            self.active -= 1;
        }
    }

    /// Adjusts stored buffer indices after `removed_idx` was deleted from
    /// the global buffer list, so references stay valid.
    ///
    /// Any entry pointing at `removed_idx` itself should have been cleared
    /// by the caller beforehand — this method only shifts down indices
    /// that were greater.
    pub fn shift_indices_after_removal(&mut self, removed_idx: usize) {
        for buffer in &mut self.buffers {
            if *buffer > removed_idx {
                *buffer -= 1;
            }
        }
    }
}

/// Error returned when a split operation cannot proceed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SplitError {
    /// Cannot close the last remaining split.
    LastSplit,
    /// A split-down / split-right call conflicts with the current
    /// orientation and there is more than one split, so we can't switch.
    OrientationConflict,
}

/// Flat collection of editor splits with a focused index.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EditorLayout {
    splits: Vec<Split>,
    focused: usize,
    orientation: SplitOrientation,
}

impl EditorLayout {
    /// Creates a layout with a single split showing `initial_buffer`.
    pub fn single(initial_buffer: usize) -> Self {
        Self {
            splits: vec![Split::new(initial_buffer)],
            focused: 0,
            orientation: SplitOrientation::Horizontal,
        }
    }

    /// Number of splits (always `>= 1`).
    pub fn len(&self) -> usize {
        self.splits.len()
    }

    /// Always `false` — `EditorLayout` is never empty by invariant.
    pub fn is_empty(&self) -> bool {
        false
    }

    /// Returns the flat slice of splits in display order.
    pub fn splits(&self) -> &[Split] {
        &self.splits
    }

    /// Returns the index of the focused split.
    pub fn focused_index(&self) -> usize {
        self.focused
    }

    /// Returns the currently focused split.
    pub fn focused(&self) -> &Split {
        &self.splits[self.focused]
    }

    /// Returns a mutable reference to the focused split.
    pub fn focused_mut(&mut self) -> &mut Split {
        &mut self.splits[self.focused]
    }

    /// Alias for [`focused_mut`] kept for clarity at command dispatch sites.
    pub fn splits_mut_focused(&mut self) -> &mut Split {
        &mut self.splits[self.focused]
    }

    /// Returns the current orientation.
    pub fn orientation(&self) -> SplitOrientation {
        self.orientation
    }

    /// Sets the focused split index, clamping into the valid range.
    pub fn set_focused(&mut self, idx: usize) {
        self.focused = idx.min(self.splits.len() - 1);
    }

    /// Adds a new split to the right of the focused one.
    ///
    /// If the layout currently has more than one split and the orientation
    /// is `Vertical`, returns `OrientationConflict` — v1 does not support
    /// mixing axes. A layout with exactly one split always accepts the
    /// operation and sets orientation to `Horizontal`.
    ///
    /// Focus moves to the newly created split (VS Code behaviour).
    pub fn split_right(&mut self) -> Result<(), SplitError> {
        if self.splits.len() > 1 && self.orientation != SplitOrientation::Horizontal {
            return Err(SplitError::OrientationConflict);
        }
        self.orientation = SplitOrientation::Horizontal;
        let clone = self.splits[self.focused].clone();
        let insert_at = self.focused + 1;
        self.splits.insert(insert_at, clone);
        self.focused = insert_at;
        Ok(())
    }

    /// Adds a new split below the focused one.
    ///
    /// Mirrors [`split_right`] but along the vertical axis.
    pub fn split_down(&mut self) -> Result<(), SplitError> {
        if self.splits.len() > 1 && self.orientation != SplitOrientation::Vertical {
            return Err(SplitError::OrientationConflict);
        }
        self.orientation = SplitOrientation::Vertical;
        let clone = self.splits[self.focused].clone();
        let insert_at = self.focused + 1;
        self.splits.insert(insert_at, clone);
        self.focused = insert_at;
        Ok(())
    }

    /// Closes the focused split.
    ///
    /// Returns `LastSplit` if closing would empty the layout.
    pub fn close_focused(&mut self) -> Result<(), SplitError> {
        if self.splits.len() <= 1 {
            return Err(SplitError::LastSplit);
        }
        self.splits.remove(self.focused);
        if self.focused >= self.splits.len() {
            self.focused = self.splits.len() - 1;
        }
        Ok(())
    }

    /// Moves the focus to the next split, wrapping around.
    pub fn focus_next(&mut self) {
        if self.splits.len() <= 1 {
            return;
        }
        self.focused = (self.focused + 1) % self.splits.len();
    }

    /// Moves the focus to the previous split, wrapping around.
    pub fn focus_prev(&mut self) {
        if self.splits.len() <= 1 {
            return;
        }
        if self.focused == 0 {
            self.focused = self.splits.len() - 1;
        } else {
            self.focused -= 1;
        }
    }

    /// Opens `buffer_idx` as a new tab in the focused split, or switches
    /// to it if it's already there.
    pub fn open_in_focused(&mut self, buffer_idx: usize) {
        self.splits[self.focused].open_buffer(buffer_idx);
    }

    /// Closes the focused split's currently active tab.
    ///
    /// Returns `(removed, split_was_closed)`:
    /// - `removed` is the buffer index that was closed, if any.
    /// - `split_was_closed` is `true` when the split ran out of tabs and
    ///   was removed from the layout.
    pub fn close_focused_tab(&mut self) -> (Option<usize>, bool) {
        let removed = self.splits[self.focused].close_active();
        let mut split_closed = false;
        if self.splits[self.focused].is_empty() && self.splits.len() > 1 {
            self.splits.remove(self.focused);
            if self.focused >= self.splits.len() {
                self.focused = self.splits.len() - 1;
            }
            split_closed = true;
        }
        (removed, split_closed)
    }

    /// Shifts every buffer index greater than `removed_idx` down by one,
    /// in every split. Used after a buffer is deleted from the global
    /// `BufferManager` so the per-split tab lists stay pointed at the
    /// right buffers.
    pub fn shift_buffer_indices_after_removal(&mut self, removed_idx: usize) {
        for split in &mut self.splits {
            split.shift_indices_after_removal(removed_idx);
        }
    }

    /// Returns `true` when the given buffer index is shown in at least
    /// one split anywhere in the layout.
    pub fn any_split_references(&self, buffer_idx: usize) -> bool {
        self.splits.iter().any(|s| s.buffers.contains(&buffer_idx))
    }
}

impl Default for EditorLayout {
    fn default() -> Self {
        Self::single(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn single_starts_with_one_split() {
        let layout = EditorLayout::single(0);
        assert_eq!(layout.len(), 1);
        assert_eq!(layout.focused_index(), 0);
        assert_eq!(layout.focused().active_buffer(), Some(0));
    }

    #[test]
    fn single_is_never_empty() {
        let layout = EditorLayout::single(5);
        assert!(!layout.is_empty());
    }

    #[test]
    fn split_right_adds_split_and_focuses_new_one() {
        let mut layout = EditorLayout::single(3);
        layout.split_right().unwrap();
        assert_eq!(layout.len(), 2);
        assert_eq!(layout.focused_index(), 1);
        assert_eq!(layout.orientation(), SplitOrientation::Horizontal);
        // Both splits reference the original buffer (VS Code duplicates).
        assert_eq!(layout.splits()[0].active_buffer(), Some(3));
        assert_eq!(layout.splits()[1].active_buffer(), Some(3));
    }

    #[test]
    fn split_down_adds_split_and_sets_vertical() {
        let mut layout = EditorLayout::single(0);
        layout.split_down().unwrap();
        assert_eq!(layout.len(), 2);
        assert_eq!(layout.orientation(), SplitOrientation::Vertical);
    }

    #[test]
    fn split_down_errors_on_horizontal_layout_with_multiple_splits() {
        let mut layout = EditorLayout::single(0);
        layout.split_right().unwrap();
        assert_eq!(layout.orientation(), SplitOrientation::Horizontal);
        assert_eq!(layout.split_down(), Err(SplitError::OrientationConflict));
    }

    #[test]
    fn split_right_allowed_when_only_one_split_even_after_vertical_then_close() {
        let mut layout = EditorLayout::single(0);
        layout.split_down().unwrap();
        layout.close_focused().unwrap();
        assert_eq!(layout.len(), 1);
        // One split left; switching orientation is allowed.
        layout.split_right().unwrap();
        assert_eq!(layout.orientation(), SplitOrientation::Horizontal);
    }

    #[test]
    fn close_focused_refuses_last_split() {
        let mut layout = EditorLayout::single(0);
        assert_eq!(layout.close_focused(), Err(SplitError::LastSplit));
    }

    #[test]
    fn close_focused_moves_focus_to_previous() {
        let mut layout = EditorLayout::single(0);
        layout.split_right().unwrap();
        layout.split_right().unwrap();
        // layout: [split0, split1, split2], focused=2.
        assert_eq!(layout.focused_index(), 2);
        layout.close_focused().unwrap();
        assert_eq!(layout.len(), 2);
        assert_eq!(layout.focused_index(), 1);
    }

    #[test]
    fn close_focused_middle_keeps_index() {
        let mut layout = EditorLayout::single(0);
        layout.split_right().unwrap();
        layout.split_right().unwrap();
        layout.set_focused(1);
        layout.close_focused().unwrap();
        // After removing index 1 from [0,1,2], we have [0,2] and focused stays at 1 (now pointing at what was split 2).
        assert_eq!(layout.len(), 2);
        assert_eq!(layout.focused_index(), 1);
    }

    #[test]
    fn focus_next_wraps_around() {
        let mut layout = EditorLayout::single(0);
        layout.split_right().unwrap();
        layout.split_right().unwrap();
        layout.set_focused(2);
        layout.focus_next();
        assert_eq!(layout.focused_index(), 0);
    }

    #[test]
    fn focus_prev_wraps_around() {
        let mut layout = EditorLayout::single(0);
        layout.split_right().unwrap();
        layout.set_focused(0);
        layout.focus_prev();
        assert_eq!(layout.focused_index(), 1);
    }

    #[test]
    fn focus_next_on_single_split_is_noop() {
        let mut layout = EditorLayout::single(0);
        layout.focus_next();
        assert_eq!(layout.focused_index(), 0);
    }

    #[test]
    fn open_in_focused_adds_tab_and_switches() {
        let mut layout = EditorLayout::single(0);
        layout.split_right().unwrap();
        // focused = 1 (the new split), currently showing buffer 0.
        layout.open_in_focused(7);
        assert_eq!(layout.splits()[0].active_buffer(), Some(0));
        assert_eq!(layout.splits()[1].active_buffer(), Some(7));
        // First split still only contains buffer 0.
        assert_eq!(layout.splits()[0].buffers, vec![0]);
        // Focused split has both tabs now.
        assert_eq!(layout.splits()[1].buffers, vec![0, 7]);
    }

    #[test]
    fn close_focused_tab_closes_split_when_empty() {
        let mut layout = EditorLayout::single(0);
        layout.split_right().unwrap();
        // Both splits have a single tab (buffer 0). Close the focused split's tab.
        let (removed, split_closed) = layout.close_focused_tab();
        assert_eq!(removed, Some(0));
        assert!(split_closed);
        assert_eq!(layout.len(), 1);
    }

    #[test]
    fn close_focused_tab_keeps_split_when_other_tabs_remain() {
        let mut layout = EditorLayout::single(0);
        layout.open_in_focused(1);
        layout.open_in_focused(2);
        // Split now has tabs [0, 1, 2], active = 2.
        let (removed, split_closed) = layout.close_focused_tab();
        assert_eq!(removed, Some(2));
        assert!(!split_closed);
        assert_eq!(layout.focused().buffers, vec![0, 1]);
        assert_eq!(layout.focused().active, 1);
    }

    #[test]
    fn shift_buffer_indices_after_removal_decrements_higher() {
        let mut layout = EditorLayout::single(0);
        layout.open_in_focused(3);
        layout.open_in_focused(5);
        layout.shift_buffer_indices_after_removal(2);
        assert_eq!(layout.focused().buffers, vec![0, 2, 4]);
    }

    #[test]
    fn any_split_references_detects_shared_buffer() {
        let mut layout = EditorLayout::single(0);
        layout.split_right().unwrap();
        assert!(layout.any_split_references(0));
        assert!(!layout.any_split_references(42));
    }

    #[test]
    fn set_focused_clamps_into_range() {
        let mut layout = EditorLayout::single(0);
        layout.split_right().unwrap();
        layout.set_focused(42);
        assert_eq!(layout.focused_index(), 1);
    }
}
