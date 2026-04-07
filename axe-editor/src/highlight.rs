use ropey::Rope;
use streaming_iterator::StreamingIterator;
use tree_sitter::{InputEdit, Parser, Point, Query, QueryCursor, Tree};

use crate::languages;

/// The semantic kind of a syntax highlight.
///
/// Each variant maps to a color in the theme via `Theme::syntax_color()`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HighlightKind {
    Keyword,
    String,
    Comment,
    Function,
    Type,
    Variable,
    Constant,
    Number,
    Operator,
    Punctuation,
    Property,
    Attribute,
    Tag,
    Escape,
    Builtin,
}

/// A span of highlighted text within a single line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HighlightSpan {
    /// Char offset of the span start within the line.
    pub col_start: usize,
    /// Char offset of the span end (exclusive) within the line.
    pub col_end: usize,
    /// The semantic highlight kind.
    pub kind: HighlightKind,
}

/// Per-buffer syntax highlighting state backed by tree-sitter.
///
/// Holds the parser, parsed tree, and compiled query for a specific language.
/// Provides incremental parsing on edits and highlight span extraction for
/// visible line ranges.
pub struct HighlightState {
    parser: Parser,
    tree: Option<Tree>,
    query: Query,
    /// Maps each capture index in the query to its `HighlightKind`.
    capture_map: Vec<Option<HighlightKind>>,
}

/// Maps a tree-sitter capture name (e.g. "keyword", "function", "string.special")
/// to the corresponding `HighlightKind`.
///
/// Hierarchical: "function.builtin" first tries "function.builtin", then "function",
/// then the base word.
fn capture_name_to_kind(name: &str) -> Option<HighlightKind> {
    // Try exact match first.
    if let Some(kind) = match_capture_name(name) {
        return Some(kind);
    }
    // Hierarchical fallback: "function.builtin" -> "function".
    if let Some(prefix) = name.split('.').next() {
        if prefix != name {
            return match_capture_name(prefix);
        }
    }
    None
}

fn match_capture_name(name: &str) -> Option<HighlightKind> {
    match name {
        "keyword" => Some(HighlightKind::Keyword),
        "string" => Some(HighlightKind::String),
        "string.special" => Some(HighlightKind::String),
        "comment" => Some(HighlightKind::Comment),
        "function" | "function.method" => Some(HighlightKind::Function),
        "function.builtin" | "function.macro" => Some(HighlightKind::Builtin),
        "type" | "type.builtin" => Some(HighlightKind::Type),
        "variable" => Some(HighlightKind::Variable),
        "variable.builtin" | "variable.special" => Some(HighlightKind::Builtin),
        "variable.parameter" => Some(HighlightKind::Variable),
        "constant" | "constant.builtin" => Some(HighlightKind::Constant),
        "number" => Some(HighlightKind::Number),
        "operator" => Some(HighlightKind::Operator),
        "punctuation" | "punctuation.bracket" | "punctuation.delimiter" | "punctuation.special" => {
            Some(HighlightKind::Punctuation)
        }
        "property" => Some(HighlightKind::Property),
        "attribute" => Some(HighlightKind::Attribute),
        "tag" => Some(HighlightKind::Tag),
        "escape" => Some(HighlightKind::Escape),
        _ => None,
    }
}

impl HighlightState {
    /// Creates a new highlight state for the given file extension.
    ///
    /// Returns `None` if the extension is not supported or if the
    /// query fails to compile (logged as a warning).
    pub fn new(ext: &str) -> Option<Self> {
        let config = languages::language_for_extension(ext)?;
        let mut parser = Parser::new();
        if let Err(e) = parser.set_language(&config.language) {
            log::warn!("Failed to set tree-sitter language for .{ext}: {e}");
            return None;
        }
        let query = match Query::new(&config.language, config.highlights_query) {
            Ok(q) => q,
            Err(e) => {
                log::warn!("Failed to compile highlight query for .{ext}: {e}");
                return None;
            }
        };

        let capture_map = query
            .capture_names()
            .iter()
            .map(|name| capture_name_to_kind(name))
            .collect();

        Some(Self {
            parser,
            tree: None,
            query,
            capture_map,
        })
    }

    /// Parses the full rope content, replacing any previous tree.
    pub fn parse_full(&mut self, rope: &Rope) {
        let tree = self.parser.parse_with_options(
            &mut |byte_offset, _position| rope_callback(rope, byte_offset),
            None,
            None,
        );
        self.tree = tree;
    }

    /// Applies an edit to the existing tree and re-parses incrementally.
    pub fn edit_and_reparse(&mut self, edit: &InputEdit, rope: &Rope) {
        if let Some(tree) = self.tree.as_mut() {
            tree.edit(edit);
        }
        let new_tree = self.parser.parse_with_options(
            &mut |byte_offset, _position| rope_callback(rope, byte_offset),
            self.tree.as_ref(),
            None,
        );
        self.tree = new_tree;
    }

    /// Returns highlight spans for each line in `[start_line, end_line)`.
    ///
    /// The returned `Vec` has one inner `Vec<HighlightSpan>` per line.
    /// Lines outside the rope range produce empty span lists.
    pub fn highlights_for_range(
        &self,
        start_line: usize,
        end_line: usize,
        rope: &Rope,
    ) -> Vec<Vec<HighlightSpan>> {
        let line_count = end_line.saturating_sub(start_line);
        let mut result: Vec<Vec<HighlightSpan>> = vec![Vec::new(); line_count];

        let tree = match self.tree.as_ref() {
            Some(t) => t,
            None => return result,
        };

        let total_lines = rope.len_lines();
        if start_line >= total_lines {
            return result;
        }

        let clamped_end = end_line.min(total_lines);

        // Compute byte range for the visible lines.
        let start_byte = rope.line_to_byte(start_line);
        let end_byte = if clamped_end >= total_lines {
            rope.len_bytes()
        } else {
            rope.line_to_byte(clamped_end)
        };

        let mut cursor = QueryCursor::new();
        cursor.set_byte_range(start_byte..end_byte);

        let root = tree.root_node();
        let text = RopeTextProvider(rope);

        // Use captures() instead of matches() so results are sorted by the
        // captured node's start byte and pattern index. This ensures that
        // specific patterns (e.g. function names) defined after catch-all
        // patterns (e.g. identifier → variable) override them correctly
        // with a last-one-wins strategy in the renderer.
        let mut captures = cursor.captures(&self.query, root, &text);
        while let Some((qmatch, capture_idx)) = {
            captures.advance();
            captures.get()
        } {
            let capture = &qmatch.captures[*capture_idx];
            let kind = match self.capture_map.get(capture.index as usize) {
                Some(Some(k)) => *k,
                _ => continue,
            };

            let node = capture.node;
            let node_start = node.start_position();
            let node_end = node.end_position();

            // Map node rows to output line indices.
            for row in node_start.row..=node_end.row {
                if row < start_line || row >= clamped_end {
                    continue;
                }
                let line_idx = row - start_line;

                let line_start_byte = rope.line_to_byte(row);
                let line_start_char = rope.byte_to_char(line_start_byte);

                let col_start_byte = if row == node_start.row {
                    node.start_byte().saturating_sub(line_start_byte)
                } else {
                    0
                };
                let line_end_byte = if row + 1 < total_lines {
                    rope.line_to_byte(row + 1)
                } else {
                    rope.len_bytes()
                };
                let col_end_byte = if row == node_end.row {
                    node.end_byte().saturating_sub(line_start_byte)
                } else {
                    line_end_byte.saturating_sub(line_start_byte)
                };

                // Convert byte offsets within line to char offsets.
                let col_start_char =
                    rope.byte_to_char(line_start_byte + col_start_byte) - line_start_char;
                let col_end_char = rope
                    .byte_to_char((line_start_byte + col_end_byte).min(rope.len_bytes()))
                    - line_start_char;

                if col_start_char < col_end_char {
                    result[line_idx].push(HighlightSpan {
                        col_start: col_start_char,
                        col_end: col_end_char,
                        kind,
                    });
                }
            }
        }

        result
    }
}

/// Zero-copy rope callback for tree-sitter's `parse_with`.
///
/// Returns the chunk of bytes at the given byte offset, or an empty
/// slice if the offset is beyond the rope's length.
fn rope_callback(rope: &Rope, byte_offset: usize) -> &[u8] {
    if byte_offset >= rope.len_bytes() {
        return &[];
    }
    let (chunk, chunk_byte_offset, _, _) = rope.chunk_at_byte(byte_offset);
    let start = byte_offset - chunk_byte_offset;
    &chunk.as_bytes()[start..]
}

/// Adapter that lets tree-sitter query matching read from a `Rope`.
struct RopeTextProvider<'a>(&'a Rope);

impl<'a> tree_sitter::TextProvider<&'a [u8]> for &RopeTextProvider<'a> {
    type I = RopeChunks<'a>;

    fn text(&mut self, node: tree_sitter::Node) -> Self::I {
        let start = self
            .0
            .byte_to_char(node.start_byte().min(self.0.len_bytes()));
        let end = self.0.byte_to_char(node.end_byte().min(self.0.len_bytes()));
        RopeChunks {
            chunks: self.0.slice(start..end).chunks(),
        }
    }
}

struct RopeChunks<'a> {
    chunks: ropey::iter::Chunks<'a>,
}

impl<'a> Iterator for RopeChunks<'a> {
    type Item = &'a [u8];

    fn next(&mut self) -> Option<Self::Item> {
        self.chunks.next().map(|s| s.as_bytes())
    }
}

/// Constructs a `tree_sitter::InputEdit` from char-offset changes on a rope.
///
/// This must be called BEFORE the rope is mutated, since it needs the
/// old byte/position information. The `new_rope` parameter is the rope
/// AFTER the edit.
pub fn make_input_edit(
    old_rope: &Rope,
    start_char: usize,
    old_end_char: usize,
    new_rope: &Rope,
    new_end_char: usize,
) -> InputEdit {
    let start_byte = old_rope.char_to_byte(start_char.min(old_rope.len_chars()));
    let old_end_byte = old_rope.char_to_byte(old_end_char.min(old_rope.len_chars()));
    let new_end_byte = new_rope.char_to_byte(new_end_char.min(new_rope.len_chars()));

    let start_position = byte_to_point(old_rope, start_byte);
    let old_end_position = byte_to_point(old_rope, old_end_byte);
    let new_end_position = byte_to_point(new_rope, new_end_byte);

    InputEdit {
        start_byte,
        old_end_byte,
        new_end_byte,
        start_position,
        old_end_position,
        new_end_position,
    }
}

pub(crate) fn byte_to_point(rope: &Rope, byte_offset: usize) -> Point {
    let byte_offset = byte_offset.min(rope.len_bytes());
    let line = rope.byte_to_line(byte_offset);
    let line_start = rope.line_to_byte(line);
    Point {
        row: line,
        column: byte_offset - line_start,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn highlight_kind_enum_is_copy() {
        let kind = HighlightKind::Keyword;
        let copy = kind;
        assert_eq!(kind, copy);
    }

    #[test]
    fn capture_name_mapping_keywords() {
        assert_eq!(
            capture_name_to_kind("keyword"),
            Some(HighlightKind::Keyword)
        );
    }

    #[test]
    fn capture_name_mapping_hierarchical_fallback() {
        // "function.builtin" should match exactly to Builtin.
        assert_eq!(
            capture_name_to_kind("function.builtin"),
            Some(HighlightKind::Builtin)
        );
        // "function.method" should match exactly to Function.
        assert_eq!(
            capture_name_to_kind("function.method"),
            Some(HighlightKind::Function)
        );
    }

    #[test]
    fn capture_name_unknown_falls_back_via_prefix() {
        // "variable.parameter.special" has no exact match — fallback to "variable".
        // Actually, it will try the full name first, then split on first dot.
        // "variable.parameter.special" -> prefix "variable" -> Variable.
        assert_eq!(
            capture_name_to_kind("variable.parameter.special"),
            Some(HighlightKind::Variable)
        );
    }

    #[test]
    fn capture_name_completely_unknown() {
        assert_eq!(capture_name_to_kind("nonexistent"), None);
    }

    #[test]
    fn highlight_state_new_for_rust() {
        let state = HighlightState::new("rs");
        assert!(state.is_some(), "should create HighlightState for .rs");
    }

    #[test]
    fn highlight_state_new_for_unknown() {
        let state = HighlightState::new("xyz");
        assert!(state.is_none());
    }

    #[test]
    fn parse_full_produces_tree() {
        let mut state = HighlightState::new("rs").unwrap();
        let rope = Rope::from_str("fn main() {}\n");
        state.parse_full(&rope);
        assert!(state.tree.is_some());
    }

    #[test]
    fn highlights_for_rust_keywords() {
        let mut state = HighlightState::new("rs").unwrap();
        let rope = Rope::from_str("fn main() {}\n");
        state.parse_full(&rope);
        let spans = state.highlights_for_range(0, 1, &rope);
        assert_eq!(spans.len(), 1, "should have 1 line of spans");

        // "fn" should be highlighted as Keyword.
        let has_keyword = spans[0]
            .iter()
            .any(|s| s.kind == HighlightKind::Keyword && s.col_start == 0 && s.col_end == 2);
        assert!(
            has_keyword,
            "expected 'fn' to be a keyword, got: {:?}",
            spans[0]
        );
    }

    #[test]
    fn highlights_for_rust_function_name() {
        let mut state = HighlightState::new("rs").unwrap();
        let rope = Rope::from_str("fn main() {}\n");
        state.parse_full(&rope);
        let spans = state.highlights_for_range(0, 1, &rope);

        let has_function = spans[0]
            .iter()
            .any(|s| s.kind == HighlightKind::Function && s.col_start == 3 && s.col_end == 7);
        assert!(
            has_function,
            "expected 'main' to be a function, got: {:?}",
            spans[0]
        );
    }

    #[test]
    fn highlights_for_rust_string() {
        let mut state = HighlightState::new("rs").unwrap();
        let rope = Rope::from_str("let s = \"hello\";\n");
        state.parse_full(&rope);
        let spans = state.highlights_for_range(0, 1, &rope);

        let has_string = spans[0].iter().any(|s| s.kind == HighlightKind::String);
        assert!(
            has_string,
            "expected a string highlight, got: {:?}",
            spans[0]
        );
    }

    #[test]
    fn highlights_for_rust_comment() {
        let mut state = HighlightState::new("rs").unwrap();
        let rope = Rope::from_str("// a comment\n");
        state.parse_full(&rope);
        let spans = state.highlights_for_range(0, 1, &rope);

        let has_comment = spans[0].iter().any(|s| s.kind == HighlightKind::Comment);
        assert!(
            has_comment,
            "expected a comment highlight, got: {:?}",
            spans[0]
        );
    }

    #[test]
    fn highlights_for_range_returns_correct_line_count() {
        let mut state = HighlightState::new("rs").unwrap();
        let rope = Rope::from_str("fn a() {}\nfn b() {}\nfn c() {}\n");
        state.parse_full(&rope);
        let spans = state.highlights_for_range(0, 3, &rope);
        assert_eq!(spans.len(), 3);
    }

    #[test]
    fn highlights_for_range_partial_lines() {
        let mut state = HighlightState::new("rs").unwrap();
        let rope = Rope::from_str("fn a() {}\nfn b() {}\nfn c() {}\n");
        state.parse_full(&rope);
        // Only request lines 1-2 (skipping first line).
        let spans = state.highlights_for_range(1, 3, &rope);
        assert_eq!(spans.len(), 2);
        // Each line should have keyword "fn".
        for (i, line_spans) in spans.iter().enumerate() {
            let has_fn = line_spans
                .iter()
                .any(|s| s.kind == HighlightKind::Keyword && s.col_start == 0 && s.col_end == 2);
            assert!(has_fn, "line {i} missing 'fn' keyword: {:?}", line_spans);
        }
    }

    #[test]
    fn highlights_for_range_beyond_file_end() {
        let mut state = HighlightState::new("rs").unwrap();
        let rope = Rope::from_str("fn a() {}\n");
        state.parse_full(&rope);
        // Request more lines than exist.
        let spans = state.highlights_for_range(0, 10, &rope);
        assert_eq!(spans.len(), 10);
    }

    #[test]
    fn incremental_parse_updates_highlights() {
        let mut state = HighlightState::new("rs").unwrap();
        let old_rope = Rope::from_str("fn a() {}\n");
        state.parse_full(&old_rope);

        // Insert "let x = 1;\n" before the fn line.
        let mut new_rope = old_rope.clone();
        new_rope.insert(0, "let x = 1;\n");

        let edit = make_input_edit(&old_rope, 0, 0, &new_rope, 11);
        state.edit_and_reparse(&edit, &new_rope);

        let spans = state.highlights_for_range(0, 2, &new_rope);
        assert_eq!(spans.len(), 2);
        // First line: "let" should be keyword.
        let has_let = spans[0]
            .iter()
            .any(|s| s.kind == HighlightKind::Keyword && s.col_start == 0 && s.col_end == 3);
        assert!(has_let, "expected 'let' keyword, got: {:?}", spans[0]);
    }

    #[test]
    fn make_input_edit_basic() {
        let old_rope = Rope::from_str("hello\n");
        let mut new_rope = old_rope.clone();
        new_rope.insert(5, " world");

        let edit = make_input_edit(&old_rope, 5, 5, &new_rope, 11);
        assert_eq!(edit.start_byte, 5);
        assert_eq!(edit.old_end_byte, 5);
        assert_eq!(edit.new_end_byte, 11);
        assert_eq!(edit.start_position, Point { row: 0, column: 5 });
    }

    #[test]
    fn highlights_python_keywords() {
        let mut state = HighlightState::new("py").unwrap();
        let rope = Rope::from_str("def hello():\n    pass\n");
        state.parse_full(&rope);
        let spans = state.highlights_for_range(0, 2, &rope);

        let has_def = spans[0]
            .iter()
            .any(|s| s.kind == HighlightKind::Keyword && s.col_start == 0 && s.col_end == 3);
        assert!(has_def, "expected 'def' keyword, got: {:?}", spans[0]);
    }

    #[test]
    fn highlights_json() {
        let mut state = HighlightState::new("json").unwrap();
        let rope = Rope::from_str("{\"key\": 42}\n");
        state.parse_full(&rope);
        let spans = state.highlights_for_range(0, 1, &rope);

        let has_property = spans[0].iter().any(|s| s.kind == HighlightKind::Property);
        let has_number = spans[0].iter().any(|s| s.kind == HighlightKind::Number);
        assert!(
            has_property,
            "expected property highlight, got: {:?}",
            spans[0]
        );
        assert!(has_number, "expected number highlight, got: {:?}", spans[0]);
    }

    #[test]
    fn highlights_go_function_name_not_variable() {
        // Go function declarations should highlight the name as Function,
        // not as Variable (the catch-all identifier pattern must not override).
        let mut state = HighlightState::new("go").unwrap();
        let rope = Rope::from_str("func main() {}\n");
        state.parse_full(&rope);
        let spans = state.highlights_for_range(0, 1, &rope);

        // "func" at col 0..4 should be Keyword.
        let has_keyword = spans[0]
            .iter()
            .any(|s| s.kind == HighlightKind::Keyword && s.col_start == 0 && s.col_end == 4);
        assert!(
            has_keyword,
            "expected 'func' to be a keyword, got: {:?}",
            spans[0]
        );

        // "main" at col 5..9 — the LAST span covering this range must be Function.
        let last_span_for_main = spans[0]
            .iter()
            .rfind(|s| s.col_start == 5 && s.col_end == 9);
        assert_eq!(
            last_span_for_main.map(|s| s.kind),
            Some(HighlightKind::Function),
            "expected 'main' final highlight to be Function, got: {:?}",
            spans[0]
        );
    }

    #[test]
    fn highlights_go_type_not_variable() {
        // Type identifiers in Go should be highlighted as Type, not Variable.
        let mut state = HighlightState::new("go").unwrap();
        let rope = Rope::from_str("type Foo struct {}\n");
        state.parse_full(&rope);
        let spans = state.highlights_for_range(0, 1, &rope);

        // "Foo" at col 5..8 — the LAST span must be Type.
        let last_span_for_foo = spans[0]
            .iter()
            .rfind(|s| s.col_start == 5 && s.col_end == 8);
        assert_eq!(
            last_span_for_foo.map(|s| s.kind),
            Some(HighlightKind::Type),
            "expected 'Foo' final highlight to be Type, got: {:?}",
            spans[0]
        );
    }

    #[test]
    fn highlights_go_call_expression_as_function() {
        // Call expressions should highlight the callee as Function.
        let mut state = HighlightState::new("go").unwrap();
        let rope = Rope::from_str("package main\nfunc main() { println(\"hi\") }\n");
        state.parse_full(&rope);
        let spans = state.highlights_for_range(1, 2, &rope);

        // "println" should have Function as the last span.
        let has_function = spans[0].iter().any(|s| s.kind == HighlightKind::Function);
        assert!(
            has_function,
            "expected a function highlight for call expression, got: {:?}",
            spans[0]
        );
    }

    #[test]
    fn empty_rope_returns_empty_spans() {
        let mut state = HighlightState::new("rs").unwrap();
        let rope = Rope::from_str("");
        state.parse_full(&rope);
        let spans = state.highlights_for_range(0, 1, &rope);
        assert_eq!(spans.len(), 1);
        // Empty content should have no spans.
        assert!(spans[0].is_empty());
    }
}
