use super::*;
use std::fs::File;
use std::io::Write;

#[test]
fn new_buffer_has_lf_line_ending() {
    let buf = EditorBuffer::new();
    assert_eq!(buf.line_ending(), LineEnding::Lf);
}

#[test]
fn from_file_detects_lf() {
    let mut tmp = tempfile::NamedTempFile::new().unwrap();
    tmp.write_all(b"line1\nline2\nline3").unwrap();
    tmp.flush().unwrap();
    let buf = EditorBuffer::from_file(tmp.path()).unwrap();
    assert_eq!(buf.line_ending(), LineEnding::Lf);
}

#[test]
fn from_file_detects_crlf() {
    let mut tmp = tempfile::NamedTempFile::new().unwrap();
    tmp.write_all(b"line1\r\nline2\r\nline3").unwrap();
    tmp.flush().unwrap();
    let buf = EditorBuffer::from_file(tmp.path()).unwrap();
    assert_eq!(buf.line_ending(), LineEnding::CrLf);
}

#[test]
fn line_ending_display() {
    assert_eq!(LineEnding::Lf.as_str(), "LF");
    assert_eq!(LineEnding::CrLf.as_str(), "CRLF");
}

#[test]
fn new_empty_buffer() {
    let buf = EditorBuffer::new();
    assert!(buf.path().is_none());
    assert!(!buf.modified);
    assert!(!buf.is_preview);
    // An empty rope has 1 line (the empty line).
    assert_eq!(buf.line_count(), 1);
}

#[test]
fn preview_flag_defaults_to_false() {
    let buf = EditorBuffer::new();
    assert!(!buf.is_preview);
}

#[test]
fn content_string_returns_buffer_text() {
    let mut tmp = tempfile::NamedTempFile::new().unwrap();
    write!(tmp, "hello world").unwrap();
    tmp.flush().unwrap();
    let buf = EditorBuffer::from_file(tmp.path()).unwrap();
    assert_eq!(buf.content_string(), "hello world");
}

#[test]
fn content_string_empty_buffer() {
    let buf = EditorBuffer::new();
    assert_eq!(buf.content_string(), "");
}

#[test]
fn from_file_loads_content() {
    let mut tmp = tempfile::NamedTempFile::new().unwrap();
    writeln!(tmp, "line1").unwrap();
    writeln!(tmp, "line2").unwrap();
    writeln!(tmp, "line3").unwrap();
    writeln!(tmp, "line4").unwrap();
    write!(tmp, "line5").unwrap();
    tmp.flush().unwrap();

    let buf = EditorBuffer::from_file(tmp.path()).unwrap();
    assert_eq!(buf.line_count(), 5);
    assert!(buf.path().is_some());
    assert!(!buf.modified);
}

#[test]
fn from_file_nonexistent_returns_error() {
    let result = EditorBuffer::from_file(std::path::Path::new("/nonexistent/file/12345.txt"));
    assert!(result.is_err());
}

#[test]
fn line_count_correct() {
    let mut tmp = tempfile::NamedTempFile::new().unwrap();
    writeln!(tmp, "a").unwrap();
    writeln!(tmp, "b").unwrap();
    write!(tmp, "c").unwrap();
    tmp.flush().unwrap();

    let buf = EditorBuffer::from_file(tmp.path()).unwrap();
    assert_eq!(buf.line_count(), 3);
}

#[test]
fn file_name_from_path() {
    let mut tmp = tempfile::Builder::new().suffix(".rs").tempfile().unwrap();
    write!(tmp, "fn main() {{}}").unwrap();
    tmp.flush().unwrap();

    let buf = EditorBuffer::from_file(tmp.path()).unwrap();
    let name = buf.file_name().unwrap();
    assert!(name.ends_with(".rs"), "expected .rs extension, got {name}");
}

#[test]
fn file_name_none_for_untitled() {
    let buf = EditorBuffer::new();
    assert!(buf.file_name().is_none());
}

#[test]
fn file_type_known_extensions() {
    let cases = vec![
        ("test.rs", "Rust"),
        ("test.py", "Python"),
        ("test.js", "JavaScript"),
        ("test.ts", "TypeScript"),
        ("test.go", "Go"),
        ("test.toml", "TOML"),
        ("test.json", "JSON"),
        ("test.md", "Markdown"),
        ("test.html", "HTML"),
        ("test.css", "CSS"),
    ];

    for (filename, expected_type) in cases {
        let buf = EditorBuffer {
            content: ropey::Rope::new(),
            path: Some(std::path::PathBuf::from(filename)),
            modified: false,
            is_preview: false,
            cursor: crate::cursor::CursorState::default(),
            scroll_row: 0,
            scroll_col: 0,
            viewport_width: usize::MAX,
            history: crate::history::EditHistory::new(),
            selection: None,
            highlight: None,
            diagnostics: Vec::new(),
            diff_hunks: Vec::new(),
            tab_size: DEFAULT_TAB_SIZE,
            insert_spaces: true,
            line_ending: LineEnding::Lf,
        };
        assert_eq!(buf.file_type(), expected_type, "wrong type for {filename}");
    }
}

#[test]
fn file_type_unknown_extension() {
    let buf = EditorBuffer {
        content: ropey::Rope::new(),
        path: Some(std::path::PathBuf::from("test.xyz")),
        modified: false,
        is_preview: false,
        cursor: crate::cursor::CursorState::default(),
        scroll_row: 0,
        scroll_col: 0,
        viewport_width: usize::MAX,
        history: crate::history::EditHistory::new(),
        selection: None,
        highlight: None,
        diagnostics: Vec::new(),
        diff_hunks: Vec::new(),
        tab_size: DEFAULT_TAB_SIZE,
        insert_spaces: true,
        line_ending: LineEnding::Lf,
    };
    assert_eq!(buf.file_type(), "Plain Text");
}

#[test]
fn line_at_returns_correct() {
    let mut tmp = tempfile::NamedTempFile::new().unwrap();
    writeln!(tmp, "first").unwrap();
    writeln!(tmp, "second").unwrap();
    write!(tmp, "third").unwrap();
    tmp.flush().unwrap();

    let buf = EditorBuffer::from_file(tmp.path()).unwrap();

    let first = buf.line_at(0).unwrap().to_string();
    assert!(first.starts_with("first"), "got: {first}");

    let last = buf.line_at(2).unwrap().to_string();
    assert!(last.starts_with("third"), "got: {last}");
}

#[test]
fn line_at_out_of_bounds() {
    let buf = EditorBuffer::new();
    assert!(buf.line_at(999).is_none());
}

#[test]
fn line_length_returns_char_count() {
    let mut tmp = tempfile::NamedTempFile::new().unwrap();
    writeln!(tmp, "hello").unwrap();
    tmp.flush().unwrap();
    let buf = EditorBuffer::from_file(tmp.path()).unwrap();
    assert_eq!(buf.line_length(0), 5);
}

#[test]
fn line_length_empty_line() {
    let mut tmp = tempfile::NamedTempFile::new().unwrap();
    writeln!(tmp).unwrap();
    write!(tmp, "x").unwrap();
    tmp.flush().unwrap();
    let buf = EditorBuffer::from_file(tmp.path()).unwrap();
    assert_eq!(buf.line_length(0), 0);
}

#[test]
fn line_length_last_line_no_newline() {
    let mut tmp = tempfile::NamedTempFile::new().unwrap();
    write!(tmp, "abc").unwrap();
    tmp.flush().unwrap();
    let buf = EditorBuffer::from_file(tmp.path()).unwrap();
    assert_eq!(buf.line_length(0), 3);
}

#[test]
fn line_length_out_of_bounds() {
    let buf = EditorBuffer::new();
    assert_eq!(buf.line_length(999), 0);
}

#[test]
fn scroll_defaults_to_zero() {
    let buf = EditorBuffer::new();
    assert_eq!(buf.scroll_row, 0);
    assert_eq!(buf.scroll_col, 0);
}

// --- Cursor movement tests ---

fn buffer_from_str(s: &str) -> EditorBuffer {
    let mut tmp = tempfile::NamedTempFile::new().unwrap();
    write!(tmp, "{s}").unwrap();
    tmp.flush().unwrap();
    EditorBuffer::from_file(tmp.path()).unwrap()
}

#[test]
fn move_right_advances_col() {
    let mut buf = buffer_from_str("hello");
    buf.move_right();
    assert_eq!(buf.cursor.col, 1);
}

#[test]
fn move_right_at_eol_wraps_to_next_line() {
    let mut buf = buffer_from_str("ab\ncd");
    buf.cursor.col = 2;
    buf.move_right();
    assert_eq!(buf.cursor.row, 1);
    assert_eq!(buf.cursor.col, 0);
}

#[test]
fn move_right_at_eof_does_nothing() {
    let mut buf = buffer_from_str("ab\ncd");
    buf.cursor.row = 1;
    buf.cursor.col = 2;
    buf.move_right();
    assert_eq!(buf.cursor.row, 1);
    assert_eq!(buf.cursor.col, 2);
}

#[test]
fn move_left_decreases_col() {
    let mut buf = buffer_from_str("hello");
    buf.cursor.col = 5;
    buf.move_left();
    assert_eq!(buf.cursor.col, 4);
}

#[test]
fn move_left_at_bol_wraps_to_prev_line() {
    let mut buf = buffer_from_str("ab\ncd");
    buf.cursor.row = 1;
    buf.cursor.col = 0;
    buf.move_left();
    assert_eq!(buf.cursor.row, 0);
    assert_eq!(buf.cursor.col, 2);
}

#[test]
fn move_left_at_bof_does_nothing() {
    let mut buf = buffer_from_str("hello");
    buf.move_left();
    assert_eq!(buf.cursor.row, 0);
    assert_eq!(buf.cursor.col, 0);
}

#[test]
fn move_down_advances_row() {
    let mut buf = buffer_from_str("a\nb");
    buf.move_down();
    assert_eq!(buf.cursor.row, 1);
}

#[test]
fn move_down_clamps_col_to_line_length() {
    let mut buf = buffer_from_str("hello\nab");
    buf.cursor.col = 5;
    buf.cursor.desired_col = 5;
    buf.move_down();
    assert_eq!(buf.cursor.row, 1);
    assert_eq!(buf.cursor.col, 2);
}

#[test]
fn move_down_restores_desired_col() {
    let mut buf = buffer_from_str("hello\nab\nworld");
    buf.cursor.col = 4;
    buf.cursor.desired_col = 4;
    buf.move_down(); // row 1 "ab" -> col clamped to 2
    assert_eq!(buf.cursor.col, 2);
    buf.move_down(); // row 2 "world" -> col restored to 4
    assert_eq!(buf.cursor.row, 2);
    assert_eq!(buf.cursor.col, 4);
}

#[test]
fn move_down_at_last_line_does_nothing() {
    let mut buf = buffer_from_str("only");
    buf.move_down();
    assert_eq!(buf.cursor.row, 0);
}

#[test]
fn move_up_decreases_row() {
    let mut buf = buffer_from_str("a\nb");
    buf.cursor.row = 1;
    buf.move_up();
    assert_eq!(buf.cursor.row, 0);
}

#[test]
fn move_up_restores_desired_col() {
    let mut buf = buffer_from_str("hello\nab\nworld");
    buf.cursor.row = 2;
    buf.cursor.col = 4;
    buf.cursor.desired_col = 4;
    buf.move_up(); // row 1 "ab" -> col clamped to 2
    assert_eq!(buf.cursor.col, 2);
    buf.move_up(); // row 0 "hello" -> col restored to 4
    assert_eq!(buf.cursor.row, 0);
    assert_eq!(buf.cursor.col, 4);
}

#[test]
fn move_up_at_first_line_does_nothing() {
    let mut buf = buffer_from_str("only");
    buf.move_up();
    assert_eq!(buf.cursor.row, 0);
}

#[test]
fn move_home_goes_to_col_zero() {
    let mut buf = buffer_from_str("hello");
    buf.cursor.col = 3;
    buf.move_home();
    assert_eq!(buf.cursor.col, 0);
}

#[test]
fn move_end_goes_to_end_of_line() {
    let mut buf = buffer_from_str("hello\nworld");
    buf.move_end();
    assert_eq!(buf.cursor.col, 5);
}

#[test]
fn move_file_start_goes_to_0_0() {
    let mut buf = buffer_from_str("hello\nworld");
    buf.cursor.row = 1;
    buf.cursor.col = 3;
    buf.move_file_start();
    assert_eq!(buf.cursor.row, 0);
    assert_eq!(buf.cursor.col, 0);
}

#[test]
fn move_file_end_goes_to_last_line_end() {
    let mut buf = buffer_from_str("hello\nworld");
    buf.move_file_end();
    assert_eq!(buf.cursor.row, 1);
    assert_eq!(buf.cursor.col, 5);
}

#[test]
fn move_page_down_moves_by_viewport() {
    let mut buf = buffer_from_str(&"line\n".repeat(50));
    buf.move_page_down(10);
    assert_eq!(buf.cursor.row, 10);
}

#[test]
fn move_page_down_clamps_to_last_line() {
    let mut buf = buffer_from_str("a\nb\nc");
    buf.move_page_down(100);
    assert_eq!(buf.cursor.row, 2);
}

#[test]
fn move_page_up_moves_by_viewport() {
    let mut buf = buffer_from_str(&"line\n".repeat(50));
    buf.cursor.row = 30;
    buf.move_page_up(10);
    assert_eq!(buf.cursor.row, 20);
}

#[test]
fn move_page_up_clamps_to_zero() {
    let mut buf = buffer_from_str("a\nb\nc");
    buf.cursor.row = 1;
    buf.move_page_up(100);
    assert_eq!(buf.cursor.row, 0);
}

#[test]
fn move_word_right_skips_word() {
    let mut buf = buffer_from_str("hello world");
    buf.move_word_right();
    assert_eq!(buf.cursor.col, 6);
}

#[test]
fn move_word_right_at_eol_wraps() {
    let mut buf = buffer_from_str("hello\nworld");
    buf.cursor.col = 5;
    buf.move_word_right();
    assert_eq!(buf.cursor.row, 1);
    assert_eq!(buf.cursor.col, 0);
}

#[test]
fn move_word_left_skips_word() {
    let mut buf = buffer_from_str("hello world");
    buf.cursor.col = 11;
    buf.move_word_left();
    assert_eq!(buf.cursor.col, 6);
}

#[test]
fn move_word_left_at_bol_wraps() {
    let mut buf = buffer_from_str("hello\nworld");
    buf.cursor.row = 1;
    buf.cursor.col = 0;
    buf.move_word_left();
    assert_eq!(buf.cursor.row, 0);
    assert_eq!(buf.cursor.col, 5);
}

// --- ensure_cursor_visible tests ---

#[test]
fn ensure_visible_scrolls_down_when_cursor_below() {
    let mut buf = buffer_from_str(&"line\n".repeat(50));
    buf.cursor.row = 30;
    buf.ensure_cursor_visible(20, 80);
    // cursor.row (30) >= scroll_row + viewport_height(20) - margin(5)
    // scroll_row = 30 + 5 + 1 - 20 = 16
    assert_eq!(buf.scroll_row, 16);
}

#[test]
fn ensure_visible_scrolls_up_when_cursor_above() {
    let mut buf = buffer_from_str(&"line\n".repeat(50));
    buf.scroll_row = 10;
    buf.cursor.row = 2;
    buf.ensure_cursor_visible(20, 80);
    // cursor.row (2) < scroll_row(10) + margin(5) => scroll_row = 2 - 5 = 0
    assert_eq!(buf.scroll_row, 0);
}

#[test]
fn ensure_visible_no_change_when_in_view() {
    let mut buf = buffer_from_str(&"line\n".repeat(50));
    buf.scroll_row = 5;
    buf.cursor.row = 12;
    buf.ensure_cursor_visible(20, 80);
    assert_eq!(buf.scroll_row, 5);
}

#[test]
fn ensure_visible_horizontal_scroll() {
    let mut buf = buffer_from_str(&"x".repeat(200));
    buf.cursor.col = 100;
    buf.ensure_cursor_visible(20, 80);
    // cursor.col (100) >= scroll_col(0) + viewport_width(80)
    // scroll_col = 100 + 1 - 80 = 21
    assert_eq!(buf.scroll_col, 21);
}

// --- insert_char tests ---

#[test]
fn insert_char_at_start() {
    let mut buf = buffer_from_str("hello");
    buf.insert_char('x');
    assert_eq!(buf.line_at(0).unwrap().to_string(), "xhello");
    assert_eq!(buf.cursor.col, 1);
}

#[test]
fn insert_char_in_middle() {
    let mut buf = buffer_from_str("hello");
    buf.cursor.col = 3;
    buf.insert_char('x');
    assert_eq!(buf.line_at(0).unwrap().to_string(), "helxlo");
    assert_eq!(buf.cursor.col, 4);
}

#[test]
fn insert_char_at_end() {
    let mut buf = buffer_from_str("hello");
    buf.cursor.col = 5;
    buf.insert_char('x');
    assert_eq!(buf.line_at(0).unwrap().to_string(), "hellox");
    assert_eq!(buf.cursor.col, 6);
}

#[test]
fn insert_char_sets_modified() {
    let mut buf = buffer_from_str("hello");
    assert!(!buf.modified);
    buf.insert_char('x');
    assert!(buf.modified);
}

#[test]
fn insert_char_in_empty_buffer() {
    let mut buf = EditorBuffer::new();
    buf.insert_char('a');
    assert_eq!(buf.line_at(0).unwrap().to_string(), "a");
    assert_eq!(buf.cursor.col, 1);
}

// --- insert_newline tests ---

#[test]
fn newline_splits_line() {
    let mut buf = buffer_from_str("hello");
    buf.cursor.col = 3;
    buf.insert_newline();
    assert_eq!(buf.line_at(0).unwrap().to_string(), "hel\n");
    assert_eq!(buf.line_at(1).unwrap().to_string(), "lo");
    assert_eq!(buf.cursor.row, 1);
    assert_eq!(buf.cursor.col, 0);
}

#[test]
fn newline_at_start() {
    let mut buf = buffer_from_str("hello");
    buf.insert_newline();
    assert_eq!(buf.line_at(0).unwrap().to_string(), "\n");
    assert!(buf.line_at(1).unwrap().to_string().starts_with("hello"));
    assert_eq!(buf.cursor.row, 1);
    assert_eq!(buf.cursor.col, 0);
}

#[test]
fn newline_at_end() {
    let mut buf = buffer_from_str("hello");
    buf.cursor.col = 5;
    buf.insert_newline();
    assert_eq!(buf.line_at(0).unwrap().to_string(), "hello\n");
    assert_eq!(buf.cursor.row, 1);
    assert_eq!(buf.cursor.col, 0);
}

#[test]
fn newline_auto_indents() {
    let mut buf = buffer_from_str("  hello");
    buf.cursor.col = 6;
    buf.insert_newline();
    assert_eq!(buf.line_at(0).unwrap().to_string(), "  hell\n");
    assert_eq!(buf.line_at(1).unwrap().to_string(), "  o");
    assert_eq!(buf.cursor.row, 1);
    assert_eq!(buf.cursor.col, 2);
}

#[test]
fn newline_sets_modified() {
    let mut buf = buffer_from_str("hello");
    assert!(!buf.modified);
    buf.insert_newline();
    assert!(buf.modified);
}

// --- insert_tab tests ---

#[test]
fn tab_inserts_spaces() {
    let mut buf = buffer_from_str("hello");
    buf.insert_tab();
    assert_eq!(buf.line_at(0).unwrap().to_string(), "    hello");
    assert_eq!(buf.cursor.col, 4);
}

#[test]
fn tab_sets_modified() {
    let mut buf = buffer_from_str("hello");
    assert!(!buf.modified);
    buf.insert_tab();
    assert!(buf.modified);
}

// --- delete_char_backward (backspace) tests ---

#[test]
fn backspace_deletes_char() {
    let mut buf = buffer_from_str("hello");
    buf.cursor.col = 3;
    buf.delete_char_backward();
    assert_eq!(buf.line_at(0).unwrap().to_string(), "helo");
    assert_eq!(buf.cursor.col, 2);
}

#[test]
fn backspace_at_bof_noop() {
    let mut buf = buffer_from_str("hello");
    buf.delete_char_backward();
    assert_eq!(buf.line_at(0).unwrap().to_string(), "hello");
    assert_eq!(buf.cursor.col, 0);
    assert!(!buf.modified);
}

#[test]
fn backspace_joins_lines() {
    let mut buf = buffer_from_str("hello\nworld");
    buf.cursor.row = 1;
    buf.cursor.col = 0;
    buf.delete_char_backward();
    assert_eq!(buf.line_at(0).unwrap().to_string(), "helloworld");
    assert_eq!(buf.cursor.row, 0);
    assert_eq!(buf.cursor.col, 5);
}

#[test]
fn backspace_sets_modified() {
    let mut buf = buffer_from_str("hello");
    buf.cursor.col = 1;
    assert!(!buf.modified);
    buf.delete_char_backward();
    assert!(buf.modified);
}

// --- delete_char_forward (delete) tests ---

#[test]
fn delete_forward_deletes_char() {
    let mut buf = buffer_from_str("hello");
    buf.cursor.col = 2;
    buf.delete_char_forward();
    assert_eq!(buf.line_at(0).unwrap().to_string(), "helo");
    assert_eq!(buf.cursor.col, 2);
}

#[test]
fn delete_forward_at_eof_noop() {
    let mut buf = buffer_from_str("hello");
    buf.cursor.col = 5;
    buf.delete_char_forward();
    assert_eq!(buf.line_at(0).unwrap().to_string(), "hello");
    assert!(!buf.modified);
}

#[test]
fn delete_forward_joins_lines() {
    let mut buf = buffer_from_str("hello\nworld");
    buf.cursor.col = 5;
    buf.delete_char_forward();
    assert_eq!(buf.line_at(0).unwrap().to_string(), "helloworld");
    assert_eq!(buf.cursor.row, 0);
    assert_eq!(buf.cursor.col, 5);
}

#[test]
fn delete_forward_sets_modified() {
    let mut buf = buffer_from_str("hello");
    assert!(!buf.modified);
    buf.delete_char_forward();
    assert!(buf.modified);
}

// --- save_to_file tests ---

#[test]
fn save_writes_content() {
    let mut tmp = tempfile::NamedTempFile::new().unwrap();
    write!(tmp, "original").unwrap();
    tmp.flush().unwrap();

    let mut buf = EditorBuffer::from_file(tmp.path()).unwrap();
    buf.insert_char('X');
    buf.save_to_file().unwrap();

    let content = std::fs::read_to_string(tmp.path()).unwrap();
    assert_eq!(content, "Xoriginal");
}

#[test]
fn save_clears_modified() {
    let mut tmp = tempfile::NamedTempFile::new().unwrap();
    write!(tmp, "data").unwrap();
    tmp.flush().unwrap();

    let mut buf = EditorBuffer::from_file(tmp.path()).unwrap();
    buf.insert_char('x');
    assert!(buf.modified);
    buf.save_to_file().unwrap();
    assert!(!buf.modified);
}

#[test]
fn save_no_path_returns_error() {
    let mut buf = EditorBuffer::new();
    buf.insert_char('x');
    assert!(buf.save_to_file().is_err());
}

#[test]
fn save_is_atomic() {
    let mut tmp = tempfile::NamedTempFile::new().unwrap();
    write!(tmp, "safe").unwrap();
    tmp.flush().unwrap();

    let mut buf = EditorBuffer::from_file(tmp.path()).unwrap();
    buf.cursor.col = 4;
    buf.insert_char('!');
    buf.save_to_file().unwrap();

    let content = std::fs::read_to_string(tmp.path()).unwrap();
    assert_eq!(content, "safe!");
}

// --- undo/redo tests ---

#[test]
fn undo_insert_char() {
    let mut buf = buffer_from_str("hello");
    buf.cursor.col = 5;
    buf.insert_char('x');
    assert_eq!(buf.line_at(0).unwrap().to_string(), "hellox");
    buf.undo();
    assert_eq!(buf.line_at(0).unwrap().to_string(), "hello");
    assert_eq!(buf.cursor.col, 5);
}

#[test]
fn redo_insert_char() {
    let mut buf = buffer_from_str("hello");
    buf.cursor.col = 5;
    buf.insert_char('x');
    buf.undo();
    buf.redo();
    assert_eq!(buf.line_at(0).unwrap().to_string(), "hellox");
    assert_eq!(buf.cursor.col, 6);
}

#[test]
fn undo_backspace() {
    let mut buf = buffer_from_str("hello");
    buf.cursor.col = 3;
    buf.delete_char_backward();
    assert_eq!(buf.line_at(0).unwrap().to_string(), "helo");
    buf.undo();
    assert_eq!(buf.line_at(0).unwrap().to_string(), "hello");
    assert_eq!(buf.cursor.col, 3);
}

#[test]
fn undo_newline() {
    let mut buf = buffer_from_str("hello");
    buf.cursor.col = 3;
    buf.insert_newline();
    assert_eq!(buf.line_count(), 2);
    buf.undo();
    assert_eq!(buf.line_at(0).unwrap().to_string(), "hello");
    assert_eq!(buf.cursor.row, 0);
    assert_eq!(buf.cursor.col, 3);
}

#[test]
fn undo_delete_forward() {
    let mut buf = buffer_from_str("hello");
    buf.cursor.col = 2;
    buf.delete_char_forward();
    assert_eq!(buf.line_at(0).unwrap().to_string(), "helo");
    buf.undo();
    assert_eq!(buf.line_at(0).unwrap().to_string(), "hello");
    assert_eq!(buf.cursor.col, 2);
}

#[test]
fn undo_tab() {
    let mut buf = buffer_from_str("hello");
    buf.insert_tab();
    assert_eq!(buf.line_at(0).unwrap().to_string(), "    hello");
    buf.undo();
    assert_eq!(buf.line_at(0).unwrap().to_string(), "hello");
    assert_eq!(buf.cursor.col, 0);
}

#[test]
fn undo_preserves_across_save() {
    let mut tmp = tempfile::NamedTempFile::new().unwrap();
    write!(tmp, "data").unwrap();
    tmp.flush().unwrap();

    let mut buf = EditorBuffer::from_file(tmp.path()).unwrap();
    buf.insert_char('x');
    buf.save_to_file().unwrap();
    buf.undo();
    assert_eq!(buf.line_at(0).unwrap().to_string(), "data");
}

#[test]
fn undo_redo_multiple_steps() {
    let mut buf = buffer_from_str("");
    // Use sleep to force separate undo groups.
    buf.insert_char('a');
    std::thread::sleep(std::time::Duration::from_millis(600));
    buf.insert_char('b');
    std::thread::sleep(std::time::Duration::from_millis(600));
    buf.insert_char('c');

    assert_eq!(buf.line_at(0).unwrap().to_string(), "abc");

    buf.undo(); // remove 'c'
    assert_eq!(buf.line_at(0).unwrap().to_string(), "ab");
    buf.undo(); // remove 'b'
    assert_eq!(buf.line_at(0).unwrap().to_string(), "a");
    buf.redo(); // restore 'b'
    assert_eq!(buf.line_at(0).unwrap().to_string(), "ab");
    buf.redo(); // restore 'c'
    assert_eq!(buf.line_at(0).unwrap().to_string(), "abc");
}

#[test]
fn undo_backspace_at_line_join() {
    let mut buf = buffer_from_str("hello\nworld");
    buf.cursor.row = 1;
    buf.cursor.col = 0;
    buf.delete_char_backward();
    assert_eq!(buf.line_at(0).unwrap().to_string(), "helloworld");
    buf.undo();
    assert_eq!(buf.line_at(0).unwrap().to_string(), "hello\n");
    assert_eq!(buf.line_at(1).unwrap().to_string(), "world");
    assert_eq!(buf.cursor.row, 1);
    assert_eq!(buf.cursor.col, 0);
}

#[test]
fn undo_on_empty_history_is_noop() {
    let mut buf = buffer_from_str("hello");
    buf.undo(); // Should not panic or change anything.
    assert_eq!(buf.line_at(0).unwrap().to_string(), "hello");
    assert!(!buf.modified);
}

#[test]
fn redo_on_empty_history_is_noop() {
    let mut buf = buffer_from_str("hello");
    buf.redo();
    assert_eq!(buf.line_at(0).unwrap().to_string(), "hello");
    assert!(!buf.modified);
}

// --- Selection tests ---

#[test]
fn new_buffer_has_no_selection() {
    let buf = EditorBuffer::new();
    assert!(buf.selection.is_none());
}

#[test]
fn from_file_has_no_selection() {
    let mut tmp = tempfile::NamedTempFile::new().unwrap();
    write!(tmp, "hello").unwrap();
    tmp.flush().unwrap();
    let buf = EditorBuffer::from_file(tmp.path()).unwrap();
    assert!(buf.selection.is_none());
}

#[test]
fn select_right_starts_selection() {
    let mut buf = buffer_from_str("hello");
    buf.select_right();
    assert!(buf.selection.is_some());
    let sel = buf.selection.as_ref().unwrap();
    assert_eq!(sel.anchor_row, 0);
    assert_eq!(sel.anchor_col, 0);
    assert_eq!(buf.cursor.col, 1);
}

#[test]
fn select_right_extends_selection() {
    let mut buf = buffer_from_str("hello");
    buf.select_right();
    buf.select_right();
    let sel = buf.selection.as_ref().unwrap();
    assert_eq!(sel.anchor_col, 0);
    assert_eq!(buf.cursor.col, 2);
}

#[test]
fn select_left_from_mid() {
    let mut buf = buffer_from_str("hello");
    buf.cursor.col = 3;
    buf.select_left();
    let sel = buf.selection.as_ref().unwrap();
    assert_eq!(sel.anchor_col, 3);
    assert_eq!(buf.cursor.col, 2);
}

#[test]
fn select_down() {
    let mut buf = buffer_from_str("hello\nworld");
    buf.select_down();
    let sel = buf.selection.as_ref().unwrap();
    assert_eq!(sel.anchor_row, 0);
    assert_eq!(buf.cursor.row, 1);
}

#[test]
fn select_up() {
    let mut buf = buffer_from_str("hello\nworld");
    buf.cursor.row = 1;
    buf.select_up();
    let sel = buf.selection.as_ref().unwrap();
    assert_eq!(sel.anchor_row, 1);
    assert_eq!(buf.cursor.row, 0);
}

#[test]
fn select_home() {
    let mut buf = buffer_from_str("hello");
    buf.cursor.col = 3;
    buf.select_home();
    let sel = buf.selection.as_ref().unwrap();
    assert_eq!(sel.anchor_col, 3);
    assert_eq!(buf.cursor.col, 0);
}

#[test]
fn select_end() {
    let mut buf = buffer_from_str("hello");
    buf.select_end();
    let sel = buf.selection.as_ref().unwrap();
    assert_eq!(sel.anchor_col, 0);
    assert_eq!(buf.cursor.col, 5);
}

#[test]
fn select_file_start() {
    let mut buf = buffer_from_str("hello\nworld");
    buf.cursor.row = 1;
    buf.cursor.col = 3;
    buf.select_file_start();
    let sel = buf.selection.as_ref().unwrap();
    assert_eq!(sel.anchor_row, 1);
    assert_eq!(sel.anchor_col, 3);
    assert_eq!(buf.cursor.row, 0);
    assert_eq!(buf.cursor.col, 0);
}

#[test]
fn select_file_end() {
    let mut buf = buffer_from_str("hello\nworld");
    buf.select_file_end();
    let sel = buf.selection.as_ref().unwrap();
    assert_eq!(sel.anchor_row, 0);
    assert_eq!(sel.anchor_col, 0);
    assert_eq!(buf.cursor.row, 1);
    assert_eq!(buf.cursor.col, 5);
}

#[test]
fn select_word_right() {
    let mut buf = buffer_from_str("hello world");
    buf.select_word_right();
    let sel = buf.selection.as_ref().unwrap();
    assert_eq!(sel.anchor_col, 0);
    assert_eq!(buf.cursor.col, 6);
}

#[test]
fn select_word_left() {
    let mut buf = buffer_from_str("hello world");
    buf.cursor.col = 11;
    buf.select_word_left();
    let sel = buf.selection.as_ref().unwrap();
    assert_eq!(sel.anchor_col, 11);
    assert_eq!(buf.cursor.col, 6);
}

#[test]
fn select_all() {
    let mut buf = buffer_from_str("hello\nworld");
    buf.select_all();
    let sel = buf.selection.as_ref().unwrap();
    assert_eq!(sel.anchor_row, 0);
    assert_eq!(sel.anchor_col, 0);
    assert_eq!(buf.cursor.row, 1);
    assert_eq!(buf.cursor.col, 5);
}

#[test]
fn clear_selection_clears() {
    let mut buf = buffer_from_str("hello");
    buf.select_right();
    assert!(buf.selection.is_some());
    buf.clear_selection();
    assert!(buf.selection.is_none());
}

// --- selected_text tests ---

#[test]
fn selected_text_single_line() {
    let mut buf = buffer_from_str("hello world");
    buf.selection = Some(crate::selection::Selection {
        anchor_row: 0,
        anchor_col: 0,
    });
    buf.cursor.col = 5;
    assert_eq!(buf.selected_text(), Some("hello".to_string()));
}

#[test]
fn selected_text_multi_line() {
    let mut buf = buffer_from_str("hello\nworld");
    buf.selection = Some(crate::selection::Selection {
        anchor_row: 0,
        anchor_col: 3,
    });
    buf.cursor.row = 1;
    buf.cursor.col = 2;
    assert_eq!(buf.selected_text(), Some("lo\nwo".to_string()));
}

#[test]
fn selected_text_backward() {
    let mut buf = buffer_from_str("hello world");
    buf.selection = Some(crate::selection::Selection {
        anchor_row: 0,
        anchor_col: 8,
    });
    buf.cursor.col = 3;
    assert_eq!(buf.selected_text(), Some("lo wo".to_string()));
}

#[test]
fn selected_text_none() {
    let buf = buffer_from_str("hello");
    assert_eq!(buf.selected_text(), None);
}

// --- delete_selection tests ---

#[test]
fn delete_selection_single_line() {
    let mut buf = buffer_from_str("hello world");
    buf.selection = Some(crate::selection::Selection {
        anchor_row: 0,
        anchor_col: 0,
    });
    buf.cursor.col = 5;
    buf.delete_selection();
    assert_eq!(buf.line_at(0).unwrap().to_string(), " world");
    assert_eq!(buf.cursor.col, 0);
    assert!(buf.selection.is_none());
}

#[test]
fn delete_selection_multi_line() {
    let mut buf = buffer_from_str("hello\nworld");
    buf.selection = Some(crate::selection::Selection {
        anchor_row: 0,
        anchor_col: 3,
    });
    buf.cursor.row = 1;
    buf.cursor.col = 2;
    buf.delete_selection();
    assert_eq!(buf.line_at(0).unwrap().to_string(), "helrld");
    assert_eq!(buf.cursor.row, 0);
    assert_eq!(buf.cursor.col, 3);
}

#[test]
fn delete_selection_records_undo() {
    let mut buf = buffer_from_str("hello");
    buf.selection = Some(crate::selection::Selection {
        anchor_row: 0,
        anchor_col: 1,
    });
    buf.cursor.col = 4;
    buf.delete_selection();
    assert_eq!(buf.line_at(0).unwrap().to_string(), "ho");
    buf.undo();
    assert_eq!(buf.line_at(0).unwrap().to_string(), "hello");
}

#[test]
fn delete_selection_returns_text() {
    let mut buf = buffer_from_str("hello");
    buf.selection = Some(crate::selection::Selection {
        anchor_row: 0,
        anchor_col: 0,
    });
    buf.cursor.col = 3;
    let deleted = buf.delete_selection();
    assert_eq!(deleted, Some("hel".to_string()));
}

#[test]
fn delete_selection_no_selection_noop() {
    let mut buf = buffer_from_str("hello");
    let result = buf.delete_selection();
    assert_eq!(result, None);
    assert_eq!(buf.line_at(0).unwrap().to_string(), "hello");
}

// --- insert_text tests ---

#[test]
fn insert_text_single_char() {
    let mut buf = buffer_from_str("hello");
    buf.cursor.col = 2;
    buf.insert_text("x");
    assert_eq!(buf.line_at(0).unwrap().to_string(), "hexllo");
    assert_eq!(buf.cursor.col, 3);
}

#[test]
fn insert_text_multiline() {
    let mut buf = buffer_from_str("hello");
    buf.cursor.col = 2;
    buf.insert_text("a\nb");
    assert_eq!(buf.line_at(0).unwrap().to_string(), "hea\n");
    assert_eq!(buf.line_at(1).unwrap().to_string(), "bllo");
    assert_eq!(buf.cursor.row, 1);
    assert_eq!(buf.cursor.col, 1);
}

#[test]
fn insert_text_replaces_selection() {
    let mut buf = buffer_from_str("hello world");
    buf.selection = Some(crate::selection::Selection {
        anchor_row: 0,
        anchor_col: 0,
    });
    buf.cursor.col = 5;
    buf.insert_text("hi");
    assert_eq!(buf.line_at(0).unwrap().to_string(), "hi world");
    assert!(buf.selection.is_none());
}

#[test]
fn insert_text_empty_noop() {
    let mut buf = buffer_from_str("hello");
    buf.insert_text("");
    assert_eq!(buf.line_at(0).unwrap().to_string(), "hello");
    assert!(!buf.modified);
}

// --- Edit methods with selection tests ---

#[test]
fn insert_char_with_selection_replaces() {
    let mut buf = buffer_from_str("hello");
    buf.selection = Some(crate::selection::Selection {
        anchor_row: 0,
        anchor_col: 1,
    });
    buf.cursor.col = 4;
    buf.insert_char('x');
    assert_eq!(buf.line_at(0).unwrap().to_string(), "hxo");
}

#[test]
fn insert_newline_with_selection_replaces() {
    let mut buf = buffer_from_str("hello");
    buf.selection = Some(crate::selection::Selection {
        anchor_row: 0,
        anchor_col: 1,
    });
    buf.cursor.col = 4;
    buf.insert_newline();
    assert_eq!(buf.line_at(0).unwrap().to_string(), "h\n");
    assert!(buf.line_at(1).unwrap().to_string().starts_with("o"));
}

#[test]
fn insert_tab_with_selection_replaces() {
    let mut buf = buffer_from_str("hello");
    buf.selection = Some(crate::selection::Selection {
        anchor_row: 0,
        anchor_col: 1,
    });
    buf.cursor.col = 4;
    buf.insert_tab();
    assert_eq!(buf.line_at(0).unwrap().to_string(), "h    o");
}

#[test]
fn backspace_with_selection_deletes_selection() {
    let mut buf = buffer_from_str("hello");
    buf.selection = Some(crate::selection::Selection {
        anchor_row: 0,
        anchor_col: 1,
    });
    buf.cursor.col = 4;
    buf.delete_char_backward();
    assert_eq!(buf.line_at(0).unwrap().to_string(), "ho");
    assert!(buf.selection.is_none());
}

#[test]
fn delete_with_selection_deletes_selection() {
    let mut buf = buffer_from_str("hello");
    buf.selection = Some(crate::selection::Selection {
        anchor_row: 0,
        anchor_col: 1,
    });
    buf.cursor.col = 4;
    buf.delete_char_forward();
    assert_eq!(buf.line_at(0).unwrap().to_string(), "ho");
    assert!(buf.selection.is_none());
}

// --- Syntax highlighting integration tests ---

#[test]
fn from_file_initializes_highlight_for_rust() {
    use std::io::Write;
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("test.rs");
    let mut file = File::create(&path).unwrap();
    writeln!(file, "fn main() {{}}").unwrap();

    let buf = EditorBuffer::from_file(&path).unwrap();
    // highlight should be Some for .rs files.
    assert!(buf.highlight.is_some());
}

#[test]
fn from_file_no_highlight_for_unknown_ext() {
    use std::io::Write;
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("test.xyz");
    let mut file = File::create(&path).unwrap();
    writeln!(file, "hello world").unwrap();

    let buf = EditorBuffer::from_file(&path).unwrap();
    assert!(buf.highlight.is_none());
}

#[test]
fn highlight_range_returns_spans_for_rust_file() {
    use std::io::Write;
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("test.rs");
    let mut file = File::create(&path).unwrap();
    writeln!(file, "fn main() {{}}").unwrap();

    let buf = EditorBuffer::from_file(&path).unwrap();
    let spans = buf.highlight_range(0, 1);
    assert_eq!(spans.len(), 1);
    // "fn" should produce a Keyword span.
    let has_keyword = spans[0]
        .iter()
        .any(|s| s.kind == crate::highlight::HighlightKind::Keyword);
    assert!(
        has_keyword,
        "expected keyword highlight, got: {:?}",
        spans[0]
    );
}

#[test]
fn highlight_range_returns_empty_for_plain_text() {
    let buf = buffer_from_str("hello world\n");
    let spans = buf.highlight_range(0, 1);
    assert_eq!(spans.len(), 1);
    assert!(spans[0].is_empty());
}

#[test]
fn highlight_updates_after_insert_char() {
    use std::io::Write;
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("test.rs");
    let mut file = File::create(&path).unwrap();
    writeln!(file, "// hello").unwrap();

    let mut buf = EditorBuffer::from_file(&path).unwrap();
    // Insert at start of file: "let x = 1;\n"
    buf.cursor.row = 0;
    buf.cursor.col = 0;
    for ch in "let x = 1;\n".chars() {
        if ch == '\n' {
            buf.insert_newline();
        } else {
            buf.insert_char(ch);
        }
    }
    let spans = buf.highlight_range(0, 1);
    // First line should now have "let" keyword.
    let has_let = spans[0]
        .iter()
        .any(|s| s.kind == crate::highlight::HighlightKind::Keyword);
    assert!(
        has_let,
        "expected 'let' keyword after insert, got: {:?}",
        spans[0]
    );
}

#[test]
fn highlight_updates_after_undo() {
    use std::io::Write;
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("test.rs");
    let mut file = File::create(&path).unwrap();
    writeln!(file, "fn main() {{}}").unwrap();

    let mut buf = EditorBuffer::from_file(&path).unwrap();
    // Type a character.
    buf.cursor.row = 0;
    buf.cursor.col = 0;
    buf.insert_char('x');
    // Undo — should restore the original parse.
    buf.undo();
    let spans = buf.highlight_range(0, 1);
    let has_fn = spans[0].iter().any(|s| {
        s.kind == crate::highlight::HighlightKind::Keyword && s.col_start == 0 && s.col_end == 2
    });
    assert!(
        has_fn,
        "expected 'fn' keyword after undo, got: {:?}",
        spans[0]
    );
}

// --- tab_size / insert_spaces config tests ---

#[test]
fn buffer_default_tab_size_is_4() {
    let buf = EditorBuffer::new();
    assert_eq!(buf.tab_size(), 4);
}

#[test]
fn buffer_default_insert_spaces_is_true() {
    let buf = EditorBuffer::new();
    assert!(buf.insert_spaces());
}

#[test]
fn buffer_with_custom_tab_size() {
    let buf = EditorBuffer::with_tab_config(2, true);
    assert_eq!(buf.tab_size(), 2);
}

#[test]
fn buffer_with_insert_spaces_false() {
    let buf = EditorBuffer::with_tab_config(4, false);
    assert!(!buf.insert_spaces());
}

#[test]
fn buffer_insert_tab_uses_configured_size() {
    let mut buf = EditorBuffer::with_tab_config(2, true);
    buf.insert_tab();
    assert_eq!(buf.line_at(0).unwrap().to_string(), "  ");
    assert_eq!(buf.cursor.col, 2);
}

#[test]
fn buffer_insert_tab_with_spaces_false_inserts_tab_char() {
    let mut buf = EditorBuffer::with_tab_config(4, false);
    buf.insert_tab();
    assert_eq!(buf.line_at(0).unwrap().to_string(), "\t");
    assert_eq!(buf.cursor.col, 1);
}

#[test]
fn buffer_insert_tab_with_spaces_false_and_custom_size() {
    // insert_spaces=false always inserts a single \t regardless of tab_size
    let mut buf = EditorBuffer::with_tab_config(8, false);
    buf.insert_tab();
    assert_eq!(buf.line_at(0).unwrap().to_string(), "\t");
    assert_eq!(buf.cursor.col, 1);
}

// --- select_word_at_cursor tests ---

#[test]
fn select_word_at_cursor_middle_of_word() {
    let mut buf = buffer_from_str("hello world");
    buf.cursor.col = 2;
    buf.select_word_at_cursor();
    assert_eq!(buf.selected_text(), Some("hello".to_string()));
    let sel = buf.selection.as_ref().unwrap();
    assert_eq!(sel.anchor_col, 0);
    assert_eq!(buf.cursor.col, 5);
}

#[test]
fn select_word_at_cursor_second_word() {
    let mut buf = buffer_from_str("hello world");
    buf.cursor.col = 8;
    buf.select_word_at_cursor();
    assert_eq!(buf.selected_text(), Some("world".to_string()));
}

#[test]
fn select_word_at_cursor_on_whitespace_does_nothing() {
    let mut buf = buffer_from_str("hello world");
    buf.cursor.col = 5;
    buf.select_word_at_cursor();
    assert!(buf.selection.is_none());
}

#[test]
fn select_word_at_cursor_snake_case() {
    let mut buf = buffer_from_str("snake_case_var = 42");
    buf.cursor.col = 6;
    buf.select_word_at_cursor();
    assert_eq!(buf.selected_text(), Some("snake_case_var".to_string()));
}

#[test]
fn select_word_at_cursor_start_of_word() {
    let mut buf = buffer_from_str("hello world");
    buf.cursor.col = 0;
    buf.select_word_at_cursor();
    assert_eq!(buf.selected_text(), Some("hello".to_string()));
}

#[test]
fn select_word_at_cursor_end_of_word() {
    let mut buf = buffer_from_str("hello world");
    buf.cursor.col = 4;
    buf.select_word_at_cursor();
    assert_eq!(buf.selected_text(), Some("hello".to_string()));
}

#[test]
fn select_word_at_cursor_empty_line() {
    let mut buf = buffer_from_str("");
    buf.select_word_at_cursor();
    assert!(buf.selection.is_none());
}

#[test]
fn select_word_at_cursor_past_line_end() {
    let mut buf = buffer_from_str("hi");
    buf.cursor.col = 5;
    buf.select_word_at_cursor();
    assert!(buf.selection.is_none());
}

// --- select_line_at_cursor tests ---

#[test]
fn select_line_at_cursor_first_line() {
    let mut buf = buffer_from_str("hello world\nsecond line");
    buf.cursor.row = 0;
    buf.cursor.col = 3;
    buf.select_line_at_cursor();
    let sel = buf.selection.as_ref().unwrap();
    assert_eq!(sel.anchor_row, 0);
    assert_eq!(sel.anchor_col, 0);
    assert_eq!(buf.cursor.col, 11);
    assert_eq!(buf.selected_text(), Some("hello world".to_string()));
}

#[test]
fn select_line_at_cursor_second_line() {
    let mut buf = buffer_from_str("first\nsecond\nthird");
    buf.cursor.row = 1;
    buf.select_line_at_cursor();
    assert_eq!(buf.selected_text(), Some("second".to_string()));
}

#[test]
fn select_line_at_cursor_empty_line() {
    let mut buf = buffer_from_str("hello\n\nworld");
    buf.cursor.row = 1;
    buf.select_line_at_cursor();
    assert!(buf.selection.is_some());
    assert_eq!(buf.cursor.col, 0);
}

// --- Diagnostics ---

#[test]
fn new_buffer_has_no_diagnostics() {
    let buf = EditorBuffer::new();
    assert!(buf.diagnostics().is_empty());
}

#[test]
fn set_and_get_diagnostics() {
    let mut buf = EditorBuffer::new();
    let diags = vec![crate::diagnostic::BufferDiagnostic {
        line: 0,
        col_start: 0,
        col_end: 5,
        severity: crate::diagnostic::DiagnosticSeverity::Error,
        message: "test error".to_string(),
        source: None,
        code: None,
    }];
    buf.set_diagnostics(diags.clone());
    assert_eq!(buf.diagnostics().len(), 1);
    assert_eq!(buf.diagnostics()[0].message, "test error");
}

#[test]
fn clear_diagnostics() {
    let mut buf = EditorBuffer::new();
    buf.set_diagnostics(vec![crate::diagnostic::BufferDiagnostic {
        line: 0,
        col_start: 0,
        col_end: 5,
        severity: crate::diagnostic::DiagnosticSeverity::Warning,
        message: "warning".to_string(),
        source: None,
        code: None,
    }]);
    assert!(!buf.diagnostics().is_empty());
    buf.clear_diagnostics();
    assert!(buf.diagnostics().is_empty());
}

// --- line_text ---

#[test]
fn line_text_returns_content() {
    let mut buf = EditorBuffer::new();
    buf.insert_text("hello\nworld");
    assert_eq!(buf.line_text(0), "hello");
    assert_eq!(buf.line_text(1), "world");
}

#[test]
fn line_text_out_of_bounds_returns_empty() {
    let buf = EditorBuffer::new();
    assert_eq!(buf.line_text(999), "");
}

// --- apply_text_edit ---

#[test]
fn apply_text_edit_single_line() {
    let mut buf = EditorBuffer::new();
    buf.insert_text("hello world");
    // Replace "world" (col 6..11) with "rust"
    buf.apply_text_edit(0, 6, 0, 11, "rust");
    assert_eq!(buf.content_string(), "hello rust");
    assert_eq!(buf.cursor.row, 0);
    assert_eq!(buf.cursor.col, 10);
}

#[test]
fn apply_text_edit_replaces_range() {
    let mut buf = EditorBuffer::new();
    buf.insert_text("fn foo()");
    // Replace "foo" (col 3..6) with "bar"
    buf.apply_text_edit(0, 3, 0, 6, "bar");
    assert_eq!(buf.content_string(), "fn bar()");
    assert_eq!(buf.cursor.col, 6);
}

#[test]
fn apply_text_edit_insert_without_delete() {
    let mut buf = EditorBuffer::new();
    buf.insert_text("ab");
    // Insert at col 1 with zero-width range
    buf.apply_text_edit(0, 1, 0, 1, "XY");
    assert_eq!(buf.content_string(), "aXYb");
    assert_eq!(buf.cursor.col, 3);
}

#[test]
fn apply_text_edit_records_in_history() {
    let mut buf = EditorBuffer::new();
    buf.insert_text("hello");
    buf.apply_text_edit(0, 0, 0, 5, "bye");
    assert_eq!(buf.content_string(), "bye");
    buf.undo();
    assert_eq!(buf.content_string(), "hello");
}

// --- scroll_by tests ---

/// Helper: creates a buffer with `n` lines of content.
fn buffer_with_lines(n: usize) -> EditorBuffer {
    let mut buf = EditorBuffer::new();
    let text: String = (0..n).map(|i| format!("line {i}\n")).collect();
    buf.insert_text(&text);
    buf
}

#[test]
fn scroll_by_positive_scrolls_down() {
    let mut buf = buffer_with_lines(100);
    buf.scroll_by(5, 20);
    assert_eq!(buf.scroll_row, 5);
}

#[test]
fn scroll_by_negative_scrolls_up() {
    let mut buf = buffer_with_lines(100);
    buf.scroll_row = 10;
    buf.scroll_by(-3, 20);
    assert_eq!(buf.scroll_row, 7);
}

#[test]
fn scroll_by_clamps_to_zero() {
    let mut buf = buffer_with_lines(100);
    buf.scroll_row = 2;
    buf.scroll_by(-10, 20);
    assert_eq!(buf.scroll_row, 0);
}

#[test]
fn scroll_by_clamps_to_max() {
    let mut buf = buffer_with_lines(50);
    let viewport_height = 20;
    buf.scroll_by(100, viewport_height);
    // Max scroll = line_count - viewport_height
    let max = buf.line_count().saturating_sub(viewport_height);
    assert_eq!(buf.scroll_row, max);
}

#[test]
fn scroll_by_no_op_for_small_file() {
    let mut buf = buffer_with_lines(5);
    let viewport_height = 20;
    buf.scroll_by(10, viewport_height);
    assert_eq!(buf.scroll_row, 0);
}

#[test]
fn scroll_by_does_not_move_cursor() {
    let mut buf = buffer_with_lines(100);
    buf.cursor.row = 5;
    buf.cursor.col = 3;
    buf.scroll_by(10, 20);
    assert_eq!(buf.cursor.row, 5);
    assert_eq!(buf.cursor.col, 3);
}

// --- scroll_horizontally_by tests ---

#[test]
fn scroll_horizontally_positive_scrolls_right() {
    let mut buf = EditorBuffer::new();
    buf.insert_text("a]".repeat(100).as_str());
    buf.set_viewport_width(20);
    buf.scroll_horizontally_by(5);
    assert_eq!(buf.scroll_col, 5);
}

#[test]
fn scroll_horizontally_negative_scrolls_left() {
    let mut buf = EditorBuffer::new();
    buf.insert_text(&"x".repeat(100));
    buf.set_viewport_width(20);
    buf.scroll_col = 10;
    buf.scroll_horizontally_by(-3);
    assert_eq!(buf.scroll_col, 7);
}

#[test]
fn scroll_horizontally_clamps_to_zero() {
    let mut buf = EditorBuffer::new();
    buf.insert_text(&"x".repeat(100));
    buf.set_viewport_width(20);
    buf.scroll_col = 2;
    buf.scroll_horizontally_by(-10);
    assert_eq!(buf.scroll_col, 0);
}

#[test]
fn scroll_horizontally_does_not_move_cursor() {
    let mut buf = EditorBuffer::new();
    buf.insert_text(&"x".repeat(200));
    buf.set_viewport_width(20);
    buf.cursor.row = 0;
    buf.cursor.col = 5;
    buf.scroll_horizontally_by(10);
    assert_eq!(buf.cursor.row, 0);
    assert_eq!(buf.cursor.col, 5);
}

#[test]
fn scroll_horizontally_clamps_at_max_line_width() {
    let mut buf = EditorBuffer::new();
    buf.insert_text(&"x".repeat(50));
    buf.set_viewport_width(20);
    // max_scroll = 50 - 20 = 30
    buf.scroll_horizontally_by(1000);
    assert_eq!(buf.scroll_col, 30);
}

#[test]
fn scroll_horizontally_no_scroll_when_content_fits() {
    let mut buf = EditorBuffer::new();
    buf.insert_text(&"x".repeat(10));
    buf.set_viewport_width(50);
    buf.scroll_horizontally_by(100);
    assert_eq!(buf.scroll_col, 0);
}

#[test]
fn max_line_width_returns_longest_line() {
    let mut buf = EditorBuffer::new();
    buf.insert_text("short\nthis is a longer line\nmed");
    assert_eq!(buf.max_line_width(), 21); // "this is a longer line"
}

// --- reload_from_disk tests ---

#[test]
fn reload_from_disk_updates_content() {
    let mut tmp = tempfile::NamedTempFile::new().unwrap();
    tmp.write_all(b"original\n").unwrap();
    tmp.flush().unwrap();

    let mut buf = EditorBuffer::from_file(tmp.path()).unwrap();
    assert_eq!(buf.content_string(), "original\n");

    // Externally modify the file.
    std::fs::write(tmp.path(), "changed\n").unwrap();

    let reloaded = buf.reload_from_disk();
    assert!(reloaded, "should reload when disk content differs");
    assert_eq!(buf.content_string(), "changed\n");
    assert!(!buf.modified, "modified flag should be false after reload");
}

#[test]
fn reload_from_disk_no_op_when_content_matches() {
    let mut tmp = tempfile::NamedTempFile::new().unwrap();
    tmp.write_all(b"same\n").unwrap();
    tmp.flush().unwrap();

    let mut buf = EditorBuffer::from_file(tmp.path()).unwrap();
    let reloaded = buf.reload_from_disk();
    assert!(!reloaded, "should not reload when content matches");
}

#[test]
fn reload_from_disk_skips_modified_buffer() {
    let mut tmp = tempfile::NamedTempFile::new().unwrap();
    tmp.write_all(b"original\n").unwrap();
    tmp.flush().unwrap();

    let mut buf = EditorBuffer::from_file(tmp.path()).unwrap();
    buf.insert_char('x'); // Mark as modified
    assert!(buf.modified);

    std::fs::write(tmp.path(), "changed\n").unwrap();

    let reloaded = buf.reload_from_disk();
    assert!(!reloaded, "should not reload a user-modified buffer");
    // Buffer should still have user's edit, not disk content.
    assert!(buf.content_string().contains('x'));
}

// --- Toggle comment tests ---

fn set_selection(buf: &mut EditorBuffer, ar: usize, ac: usize, cr: usize, cc: usize) {
    buf.selection = Some(crate::selection::Selection {
        anchor_row: ar,
        anchor_col: ac,
    });
    buf.cursor.row = cr;
    buf.cursor.col = cc;
    buf.cursor.desired_col = cc;
}

#[test]
fn toggle_line_comment_rust_single_line_round_trip() {
    let mut buf = buffer_from_str("fn main() {}\n");
    buf.cursor.row = 0;
    buf.cursor.col = 0;
    buf.toggle_line_comment("//");
    assert_eq!(buf.line_text(0), "// fn main() {}");
    buf.toggle_line_comment("//");
    assert_eq!(buf.line_text(0), "fn main() {}");
}

#[test]
fn toggle_line_comment_multi_line_all_commented_uncomments() {
    let mut buf = buffer_from_str("// a\n// b\n// c\n");
    set_selection(&mut buf, 0, 0, 2, 4);
    buf.toggle_line_comment("//");
    assert_eq!(buf.line_text(0), "a");
    assert_eq!(buf.line_text(1), "b");
    assert_eq!(buf.line_text(2), "c");
}

#[test]
fn toggle_line_comment_mixed_comments_all() {
    let mut buf = buffer_from_str("a\n// b\nc\n");
    set_selection(&mut buf, 0, 0, 2, 1);
    buf.toggle_line_comment("//");
    assert_eq!(buf.line_text(0), "// a");
    assert_eq!(buf.line_text(1), "// // b");
    assert_eq!(buf.line_text(2), "// c");
}

#[test]
fn toggle_line_comment_preserves_common_indent() {
    let mut buf = buffer_from_str("    foo\n      bar\n    baz\n");
    set_selection(&mut buf, 0, 0, 2, 7);
    buf.toggle_line_comment("//");
    assert_eq!(buf.line_text(0), "    // foo");
    assert_eq!(buf.line_text(1), "    //   bar");
    assert_eq!(buf.line_text(2), "    // baz");
}

#[test]
fn toggle_line_comment_skips_blank_lines_when_commenting() {
    let mut buf = buffer_from_str("a\n\nb\n");
    set_selection(&mut buf, 0, 0, 2, 1);
    buf.toggle_line_comment("//");
    assert_eq!(buf.line_text(0), "// a");
    assert_eq!(buf.line_text(1), "");
    assert_eq!(buf.line_text(2), "// b");
}

#[test]
fn toggle_line_comment_selection_ends_at_col0_excludes_last() {
    let mut buf = buffer_from_str("a\nb\nc\n");
    // Selection: row 0 col 0 → row 2 col 0 (cursor on row 2). VS Code excludes row 2.
    set_selection(&mut buf, 0, 0, 2, 0);
    buf.toggle_line_comment("//");
    assert_eq!(buf.line_text(0), "// a");
    assert_eq!(buf.line_text(1), "// b");
    assert_eq!(
        buf.line_text(2),
        "c",
        "row 2 (cursor at col 0) must stay unchanged"
    );
}

#[test]
fn toggle_line_comment_uses_cursor_row_when_no_selection() {
    let mut buf = buffer_from_str("a\nb\nc\n");
    buf.cursor.row = 1;
    buf.cursor.col = 0;
    buf.toggle_line_comment("//");
    assert_eq!(buf.line_text(0), "a");
    assert_eq!(buf.line_text(1), "// b");
    assert_eq!(buf.line_text(2), "c");
}

#[test]
fn toggle_line_comment_single_undo_reverts_whole_action() {
    let mut buf = buffer_from_str("a\nb\nc\n");
    set_selection(&mut buf, 0, 0, 2, 1);
    buf.toggle_line_comment("//");
    assert_eq!(buf.line_text(0), "// a");
    assert_eq!(buf.line_text(1), "// b");
    assert_eq!(buf.line_text(2), "// c");
    buf.undo();
    assert_eq!(buf.line_text(0), "a");
    assert_eq!(buf.line_text(1), "b");
    assert_eq!(buf.line_text(2), "c");
}

#[test]
fn toggle_line_comment_uncomment_handles_no_space_after_token() {
    let mut buf = buffer_from_str("//a\n//b\n");
    set_selection(&mut buf, 0, 0, 1, 3);
    buf.toggle_line_comment("//");
    assert_eq!(buf.line_text(0), "a");
    assert_eq!(buf.line_text(1), "b");
}

#[test]
fn toggle_line_comment_python_hash_token() {
    let mut buf = buffer_from_str("x = 1\ny = 2\n");
    set_selection(&mut buf, 0, 0, 1, 5);
    buf.toggle_line_comment("#");
    assert_eq!(buf.line_text(0), "# x = 1");
    assert_eq!(buf.line_text(1), "# y = 2");
}

#[test]
fn toggle_block_comment_wraps_selection() {
    let mut buf = buffer_from_str("let x = foo;\n");
    // Select "foo" (cols 8..11 on row 0).
    set_selection(&mut buf, 0, 8, 0, 11);
    buf.toggle_block_comment("/*", "*/");
    assert_eq!(buf.line_text(0), "let x = /*foo*/;");
}

#[test]
fn toggle_block_comment_unwraps_exact_match() {
    let mut buf = buffer_from_str("let x = /*foo*/;\n");
    // Select "/*foo*/" (cols 8..15 on row 0).
    set_selection(&mut buf, 0, 8, 0, 15);
    buf.toggle_block_comment("/*", "*/");
    assert_eq!(buf.line_text(0), "let x = foo;");
}

#[test]
fn toggle_block_comment_empty_selection_is_noop() {
    let mut buf = buffer_from_str("hello\n");
    buf.cursor.row = 0;
    buf.cursor.col = 2;
    buf.toggle_block_comment("/*", "*/");
    assert_eq!(buf.line_text(0), "hello");
}

#[test]
fn toggle_block_comment_undo_reverts_wrap() {
    let mut buf = buffer_from_str("let x = foo;\n");
    set_selection(&mut buf, 0, 8, 0, 11);
    buf.toggle_block_comment("/*", "*/");
    assert_eq!(buf.line_text(0), "let x = /*foo*/;");
    buf.undo();
    assert_eq!(buf.line_text(0), "let x = foo;");
}

#[test]
fn reload_from_disk_clamps_cursor() {
    let mut tmp = tempfile::NamedTempFile::new().unwrap();
    tmp.write_all(b"line 1\nline 2\nline 3\nline 4\n").unwrap();
    tmp.flush().unwrap();

    let mut buf = EditorBuffer::from_file(tmp.path()).unwrap();
    buf.cursor.row = 3; // On "line 4"

    // Shrink the file to 2 lines.
    std::fs::write(tmp.path(), "line 1\nline 2\n").unwrap();

    let reloaded = buf.reload_from_disk();
    assert!(reloaded);
    assert!(
        buf.cursor.row < buf.line_count(),
        "cursor row should be clamped"
    );
}
