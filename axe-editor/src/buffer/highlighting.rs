use crate::highlight::{self, HighlightSpan};

use super::EditorBuffer;

impl EditorBuffer {
    /// Returns the width (in chars) of the longest line in the buffer.
    pub fn max_line_width(&self) -> usize {
        let line_count = self.content.len_lines();
        (0..line_count)
            .map(|i| {
                let line = self.content.line(i);
                let len = line.len_chars();
                // Strip trailing newline from the count.
                if len > 0 && line.char(len - 1) == '\n' {
                    len - 1
                } else {
                    len
                }
            })
            .max()
            .unwrap_or(0)
    }

    /// Returns highlight spans for the given line range `[start, end)`.
    ///
    /// Each element in the returned `Vec` corresponds to one line and
    /// contains the spans for that line. Returns an empty set of spans
    /// per line if no highlighting is available.
    pub fn highlight_range(&self, start: usize, end: usize) -> Vec<Vec<HighlightSpan>> {
        match self.highlight.as_ref() {
            Some(hl) => hl.highlights_for_range(start, end, &self.content),
            None => vec![Vec::new(); end.saturating_sub(start)],
        }
    }

    /// Notifies the tree-sitter highlighter about a text edit and re-parses.
    ///
    /// Must be called AFTER the rope has been mutated. The `start_char`,
    /// `old_end_char` refer to positions in the OLD rope (before the edit),
    /// and `new_end_char` refers to the position in the NEW rope.
    pub(super) fn notify_highlight_insert(&mut self, start_char: usize, chars_inserted: usize) {
        if let Some(hl) = self.highlight.as_mut() {
            // Build a snapshot of the old rope state for InputEdit.
            // Since the rope has already been mutated, we reconstruct old positions.
            let new_end_char = start_char + chars_inserted;
            let start_byte = self
                .content
                .char_to_byte(start_char.min(self.content.len_chars()));
            let new_end_byte = self
                .content
                .char_to_byte(new_end_char.min(self.content.len_chars()));

            let start_position = highlight::byte_to_point(&self.content, start_byte);
            // Old end = start (it was an insertion, no old text removed).
            let old_end_position = start_position;
            let new_end_position = highlight::byte_to_point(&self.content, new_end_byte);

            let edit = tree_sitter::InputEdit {
                start_byte,
                old_end_byte: start_byte,
                new_end_byte,
                start_position,
                old_end_position,
                new_end_position,
            };
            hl.edit_and_reparse(&edit, &self.content);
        }
    }

    /// Notifies the tree-sitter highlighter about a deletion and re-parses.
    ///
    /// `start_char` is the char index where deletion starts (in both old and new),
    /// `chars_deleted` is how many chars were removed.
    pub(super) fn notify_highlight_delete(
        &mut self,
        start_char: usize,
        _chars_deleted: usize,
        old_bytes: usize,
        old_end_position: tree_sitter::Point,
    ) {
        if let Some(hl) = self.highlight.as_mut() {
            let start_byte = self
                .content
                .char_to_byte(start_char.min(self.content.len_chars()));
            let start_position = highlight::byte_to_point(&self.content, start_byte);

            let edit = tree_sitter::InputEdit {
                start_byte,
                old_end_byte: start_byte + old_bytes,
                new_end_byte: start_byte,
                start_position,
                old_end_position,
                new_end_position: start_position,
            };
            hl.edit_and_reparse(&edit, &self.content);
        }
    }

    /// Re-parses the full content for highlight after undo/redo.
    pub(super) fn reparse_highlight_full(&mut self) {
        if let Some(hl) = self.highlight.as_mut() {
            hl.parse_full(&self.content);
        }
    }
}
