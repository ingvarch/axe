/// Represents a text selection anchor point.
///
/// The selection range spans from the anchor to the current cursor position.
/// The anchor stays fixed while the cursor moves to extend/shrink the selection.
///
/// # Design
///
/// Only stores the anchor. The "head" of the selection is always `buffer.cursor`,
/// which avoids dual-cursor bookkeeping.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Selection {
    /// Row of the anchor (where selection started).
    pub anchor_row: usize,
    /// Column of the anchor (where selection started).
    pub anchor_col: usize,
}

impl Selection {
    /// Returns the selection range in document order: `(start_row, start_col, end_row, end_col)`.
    ///
    /// The start is always before or equal to the end, regardless of whether
    /// the anchor is before or after the cursor.
    pub fn normalized(&self, cursor_row: usize, cursor_col: usize) -> (usize, usize, usize, usize) {
        if self.anchor_row < cursor_row
            || (self.anchor_row == cursor_row && self.anchor_col <= cursor_col)
        {
            (self.anchor_row, self.anchor_col, cursor_row, cursor_col)
        } else {
            (cursor_row, cursor_col, self.anchor_row, self.anchor_col)
        }
    }

    /// Returns true if the selection is empty (anchor equals cursor).
    pub fn is_empty(&self, cursor_row: usize, cursor_col: usize) -> bool {
        self.anchor_row == cursor_row && self.anchor_col == cursor_col
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalized_forward() {
        let sel = Selection {
            anchor_row: 0,
            anchor_col: 0,
        };
        assert_eq!(sel.normalized(2, 5), (0, 0, 2, 5));
    }

    #[test]
    fn normalized_backward() {
        let sel = Selection {
            anchor_row: 3,
            anchor_col: 10,
        };
        assert_eq!(sel.normalized(1, 2), (1, 2, 3, 10));
    }

    #[test]
    fn normalized_same_line() {
        let sel = Selection {
            anchor_row: 1,
            anchor_col: 8,
        };
        assert_eq!(sel.normalized(1, 3), (1, 3, 1, 8));
    }

    #[test]
    fn normalized_same_position() {
        let sel = Selection {
            anchor_row: 2,
            anchor_col: 4,
        };
        assert_eq!(sel.normalized(2, 4), (2, 4, 2, 4));
    }

    #[test]
    fn is_empty_true() {
        let sel = Selection {
            anchor_row: 1,
            anchor_col: 5,
        };
        assert!(sel.is_empty(1, 5));
    }

    #[test]
    fn is_empty_false() {
        let sel = Selection {
            anchor_row: 1,
            anchor_col: 5,
        };
        assert!(!sel.is_empty(1, 6));
    }
}
