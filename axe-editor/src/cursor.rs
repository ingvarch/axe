/// Cursor position within an editor buffer.
///
/// Tracks the row and column of the editing cursor. Both are zero-indexed.
/// The `desired_col` field remembers the intended column for vertical movement
/// across lines shorter than the desired position.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CursorState {
    /// Zero-based line index.
    pub row: usize,
    /// Zero-based column index.
    pub col: usize,
    /// Remembered column for vertical movement on lines shorter than desired position.
    pub desired_col: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_cursor_is_at_origin() {
        let cursor = CursorState::default();
        assert_eq!(cursor.row, 0);
        assert_eq!(cursor.col, 0);
        assert_eq!(cursor.desired_col, 0);
    }

    #[test]
    fn desired_col_independent_of_col() {
        let cursor = CursorState {
            row: 0,
            col: 5,
            desired_col: 10,
        };
        assert_eq!(cursor.col, 5);
        assert_eq!(cursor.desired_col, 10);
    }
}
