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
/// Holds the index of the buffer currently displayed in the split and a
/// small amount of per-split viewport state so each split scrolls
/// independently. The buffer itself lives in the global `BufferManager`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Split {
    /// Index into `BufferManager.buffers` of the buffer shown here.
    pub active_buffer: usize,
}

impl Split {
    /// Creates a new split referencing `active_buffer`.
    pub fn new(active_buffer: usize) -> Self {
        Self { active_buffer }
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
        let new_split = Split::new(self.splits[self.focused].active_buffer);
        let insert_at = self.focused + 1;
        self.splits.insert(insert_at, new_split);
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
        let new_split = Split::new(self.splits[self.focused].active_buffer);
        let insert_at = self.focused + 1;
        self.splits.insert(insert_at, new_split);
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

    /// Replaces the active buffer of the focused split.
    ///
    /// Used when the user opens a new file: the open-file command routes
    /// the buffer into the currently focused split.
    pub fn set_focused_buffer(&mut self, buffer_idx: usize) {
        self.splits[self.focused].active_buffer = buffer_idx;
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
        assert_eq!(layout.focused().active_buffer, 0);
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
        assert_eq!(layout.splits()[0].active_buffer, 3);
        assert_eq!(layout.splits()[1].active_buffer, 3);
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
    fn set_focused_buffer_updates_focused_split_only() {
        let mut layout = EditorLayout::single(0);
        layout.split_right().unwrap();
        // focused = 1 (the new split).
        layout.set_focused_buffer(7);
        assert_eq!(layout.splits()[0].active_buffer, 0);
        assert_eq!(layout.splits()[1].active_buffer, 7);
    }

    #[test]
    fn set_focused_clamps_into_range() {
        let mut layout = EditorLayout::single(0);
        layout.split_right().unwrap();
        layout.set_focused(42);
        assert_eq!(layout.focused_index(), 1);
    }
}
