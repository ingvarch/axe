use axe_core::InlayHint;

/// Formats an inlay hint for display, honoring the server's padding flags.
///
/// Keeps label text as-is (rust-analyzer already includes `: ` prefixes for
/// type hints and `:` suffixes for parameter hints), adding padding spaces
/// only when the server requests them.
pub fn format_inlay_label(hint: &InlayHint) -> String {
    let mut out = String::with_capacity(hint.label.len() + 2);
    if hint.padding_left {
        out.push(' ');
    }
    out.push_str(&hint.label);
    if hint.padding_right {
        out.push(' ');
    }
    out
}

/// A single visual cell on a rendered editor line.
///
/// `Char` is a real buffer character (taken from the tab-expanded line text);
/// `Hint` is a virtual character supplied by an inlay hint and does not
/// correspond to any logical column in the buffer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VisualCell {
    Char(char),
    Hint(char),
}

/// Merges a tab-expanded line with inlay hints at logical display columns.
///
/// `line` is the already-expanded display text for one buffer line. `hints`
/// is a sequence of `(display_col, label)` pairs, pre-sorted by column
/// ascending, where `display_col` is the column at which the hint should
/// appear — the hint is inserted **before** the character at that column.
///
/// Hints with a column beyond the line end are appended at the end in the
/// order provided. Hints at the same column are inserted in input order.
/// The caller applies horizontal scroll and content-width clipping to the
/// returned cells.
pub fn paint_line_with_hints(line: &str, hints: &[(usize, String)]) -> Vec<VisualCell> {
    let line_chars: Vec<char> = line.chars().collect();
    let mut out: Vec<VisualCell> = Vec::with_capacity(line_chars.len() + hints.len() * 4);
    let mut hint_idx = 0;

    for (i, ch) in line_chars.iter().enumerate() {
        while hint_idx < hints.len() && hints[hint_idx].0 == i {
            for hc in hints[hint_idx].1.chars() {
                out.push(VisualCell::Hint(hc));
            }
            hint_idx += 1;
        }
        out.push(VisualCell::Char(*ch));
    }

    // Trailing hints — anything at or past EOL goes after the last character.
    while hint_idx < hints.len() {
        for hc in hints[hint_idx].1.chars() {
            out.push(VisualCell::Hint(hc));
        }
        hint_idx += 1;
    }

    out
}

/// Counts the number of hint cells that precede `display_col` in the output
/// of [`paint_line_with_hints`].
///
/// Used by the cursor renderer so the drawn cursor lands on the real
/// character, not inside a preceding inlay hint.
pub fn hint_shift_before(hints: &[(usize, String)], display_col: usize) -> usize {
    hints
        .iter()
        .filter(|(col, _)| *col <= display_col)
        .map(|(_, label)| label.chars().count())
        .sum()
}

#[cfg(test)]
mod tests {
    use super::*;
    use axe_core::InlayHintKind;

    fn h(col: usize, label: &str) -> (usize, String) {
        (col, label.to_string())
    }

    fn inlay(label: &str, padding_left: bool, padding_right: bool) -> InlayHint {
        InlayHint {
            row: 0,
            col: 0,
            label: label.to_string(),
            kind: InlayHintKind::Type,
            padding_left,
            padding_right,
        }
    }

    #[test]
    fn format_label_no_padding() {
        let hint = inlay(": i32", false, false);
        assert_eq!(format_inlay_label(&hint), ": i32");
    }

    #[test]
    fn format_label_left_padding() {
        let hint = inlay(": i32", true, false);
        assert_eq!(format_inlay_label(&hint), " : i32");
    }

    #[test]
    fn format_label_both_padding() {
        let hint = inlay("i32", true, true);
        assert_eq!(format_inlay_label(&hint), " i32 ");
    }

    #[test]
    fn empty_hints_preserves_line() {
        let cells = paint_line_with_hints("let x = 5;", &[]);
        assert_eq!(cells.len(), 10);
        assert!(matches!(cells[0], VisualCell::Char('l')));
        assert!(matches!(cells[9], VisualCell::Char(';')));
        assert!(cells.iter().all(|c| matches!(c, VisualCell::Char(_))));
    }

    #[test]
    fn hint_at_col_zero_prepends() {
        let cells = paint_line_with_hints("abc", &[h(0, "!!")]);
        assert_eq!(cells.len(), 5);
        assert!(matches!(cells[0], VisualCell::Hint('!')));
        assert!(matches!(cells[1], VisualCell::Hint('!')));
        assert!(matches!(cells[2], VisualCell::Char('a')));
        assert!(matches!(cells[3], VisualCell::Char('b')));
        assert!(matches!(cells[4], VisualCell::Char('c')));
    }

    #[test]
    fn hint_in_middle_inserts_before_char() {
        // "let x = 5": indices 0 l, 1 e, 2 t, 3 space, 4 x, 5 space, 6 =, 7 space, 8 5.
        // Insert ": i32" before index 5 (the space after `x`) → rust-analyzer style.
        let cells = paint_line_with_hints("let x = 5", &[h(5, ": i32")]);
        let labels: String = cells
            .iter()
            .map(|c| match c {
                VisualCell::Char(c) | VisualCell::Hint(c) => *c,
            })
            .collect();
        assert_eq!(labels, "let x: i32 = 5");
    }

    #[test]
    fn hint_past_eol_appends() {
        let cells = paint_line_with_hints("abc", &[h(100, " /* tail */")]);
        let suffix: String = cells[3..]
            .iter()
            .map(|c| match c {
                VisualCell::Hint(c) => *c,
                VisualCell::Char(c) => *c,
            })
            .collect();
        assert_eq!(suffix, " /* tail */");
        // All tail cells are hint cells.
        for cell in &cells[3..] {
            assert!(matches!(cell, VisualCell::Hint(_)));
        }
    }

    #[test]
    fn multiple_hints_sorted_input() {
        let cells = paint_line_with_hints("abcdef", &[h(1, "X"), h(3, "YY"), h(5, "Z")]);
        let s: String = cells
            .iter()
            .map(|c| match c {
                VisualCell::Char(c) | VisualCell::Hint(c) => *c,
            })
            .collect();
        // "a [X] b c [YY] d e [Z] f"
        assert_eq!(s, "aXbcYYdeZf");
    }

    #[test]
    fn multiple_hints_same_col_insert_in_order() {
        let cells = paint_line_with_hints("abc", &[h(1, "X"), h(1, "Y")]);
        let s: String = cells
            .iter()
            .map(|c| match c {
                VisualCell::Char(c) | VisualCell::Hint(c) => *c,
            })
            .collect();
        assert_eq!(s, "aXYbc");
    }

    #[test]
    fn hint_chars_are_tagged_as_hint() {
        let cells = paint_line_with_hints("ab", &[h(1, "X")]);
        assert!(matches!(cells[0], VisualCell::Char('a')));
        assert!(matches!(cells[1], VisualCell::Hint('X')));
        assert!(matches!(cells[2], VisualCell::Char('b')));
    }

    #[test]
    fn empty_line_with_trailing_hint() {
        let cells = paint_line_with_hints("", &[h(0, "hi")]);
        assert_eq!(cells.len(), 2);
        assert!(matches!(cells[0], VisualCell::Hint('h')));
        assert!(matches!(cells[1], VisualCell::Hint('i')));
    }

    #[test]
    fn empty_line_no_hints_returns_empty() {
        let cells = paint_line_with_hints("", &[]);
        assert!(cells.is_empty());
    }

    #[test]
    fn hint_shift_before_counts_preceding_hint_chars() {
        let hints = [h(1, "X"), h(3, "YY"), h(7, "Z")];
        assert_eq!(hint_shift_before(&hints, 0), 0);
        assert_eq!(hint_shift_before(&hints, 1), 1);
        assert_eq!(hint_shift_before(&hints, 2), 1);
        assert_eq!(hint_shift_before(&hints, 3), 3);
        assert_eq!(hint_shift_before(&hints, 6), 3);
        assert_eq!(hint_shift_before(&hints, 7), 4);
        assert_eq!(hint_shift_before(&hints, 100), 4);
    }
}
