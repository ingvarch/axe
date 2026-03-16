/// Cursor position within an editor buffer.
///
/// Tracks the row and column of the editing cursor. Both are zero-indexed.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CursorState {
    /// Zero-based line index.
    pub row: usize,
    /// Zero-based column index.
    pub col: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_cursor_is_at_origin() {
        let cursor = CursorState::default();
        assert_eq!(cursor.row, 0);
        assert_eq!(cursor.col, 0);
    }
}
