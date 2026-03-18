use serde_json::Value;

/// A styled span within a hover line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HoverSpan {
    pub text: String,
    pub bold: bool,
    pub italic: bool,
    /// Inline code style.
    pub code: bool,
}

impl HoverSpan {
    fn plain(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            bold: false,
            italic: false,
            code: false,
        }
    }

    fn bold(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            bold: true,
            italic: false,
            code: false,
        }
    }

    fn italic(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            bold: false,
            italic: true,
            code: false,
        }
    }

    fn code(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            bold: false,
            italic: false,
            code: true,
        }
    }
}

/// A single line in the hover tooltip.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HoverLine {
    pub spans: Vec<HoverSpan>,
    /// Whether the entire line is part of a code block.
    pub is_code_block: bool,
}

/// Hover information to display in a tooltip.
#[derive(Debug, Clone)]
pub struct HoverInfo {
    pub lines: Vec<HoverLine>,
    /// Cursor row when hover was triggered (for rendering near cursor).
    pub trigger_row: usize,
    /// Cursor column when hover was triggered (for rendering near cursor).
    pub trigger_col: usize,
}

/// Parses an LSP hover response into `HoverInfo`.
///
/// Handles the various response formats:
/// - `null` -> None
/// - `{ "contents": { "kind": "markdown"|"plaintext", "value": "..." } }` (MarkupContent)
/// - `{ "contents": "string" }` (MarkedString as plain string)
/// - `{ "contents": { "language": "...", "value": "..." } }` (MarkedString with language)
/// - `{ "contents": [...] }` (array of MarkedString)
pub fn parse_hover_response(value: &Value) -> Option<HoverInfo> {
    if value.is_null() {
        return None;
    }

    let contents = value.get("contents")?;

    let text = extract_hover_text(contents)?;
    if text.is_empty() {
        return None;
    }

    let lines = markdown_to_hover_lines(&text);
    if lines.is_empty() {
        return None;
    }

    Some(HoverInfo {
        lines,
        trigger_row: 0,
        trigger_col: 0,
    })
}

/// Extracts text content from the various LSP hover content formats.
fn extract_hover_text(contents: &Value) -> Option<String> {
    // MarkedString with language: { "language": "...", "value": "..." }
    // Must check before MarkupContent since both have "value".
    if let Some(lang) = contents.get("language") {
        let lang = lang.as_str().unwrap_or("");
        let value = contents.get("value").and_then(|v| v.as_str()).unwrap_or("");
        return Some(format!("```{lang}\n{value}\n```"));
    }

    // MarkupContent: { "kind": "markdown"|"plaintext", "value": "..." }
    if let Some(value) = contents.get("value") {
        return value.as_str().map(|s| s.to_string());
    }

    // Plain MarkedString: just a string.
    if let Some(s) = contents.as_str() {
        return Some(s.to_string());
    }

    // Array of MarkedString.
    if let Some(arr) = contents.as_array() {
        let mut parts = Vec::new();
        for item in arr {
            if let Some(s) = item.as_str() {
                parts.push(s.to_string());
            } else if let Some(lang) = item.get("language") {
                let lang = lang.as_str().unwrap_or("");
                let value = item.get("value").and_then(|v| v.as_str()).unwrap_or("");
                parts.push(format!("```{lang}\n{value}\n```"));
            } else if let Some(value) = item.get("value") {
                if let Some(s) = value.as_str() {
                    parts.push(s.to_string());
                }
            }
        }
        if parts.is_empty() {
            return None;
        }
        return Some(parts.join("\n\n"));
    }

    None
}

/// Converts a markdown string into styled hover lines.
///
/// Supports basic markdown: headers, code blocks, bold, italic, inline code, separators.
pub fn markdown_to_hover_lines(text: &str) -> Vec<HoverLine> {
    let mut lines = Vec::new();
    let mut in_code_block = false;
    let raw_lines: Vec<&str> = text.lines().collect();

    for raw_line in &raw_lines {
        // Code block fence.
        if raw_line.starts_with("```") {
            in_code_block = !in_code_block;
            continue;
        }

        if in_code_block {
            lines.push(HoverLine {
                spans: vec![HoverSpan::plain(*raw_line)],
                is_code_block: true,
            });
            continue;
        }

        // Separator.
        let trimmed = raw_line.trim();
        if trimmed == "---" || trimmed == "***" || trimmed == "___" {
            lines.push(HoverLine {
                spans: vec![],
                is_code_block: false,
            });
            continue;
        }

        // Header.
        if let Some(content) = trimmed.strip_prefix("# ") {
            lines.push(HoverLine {
                spans: vec![HoverSpan::bold(content)],
                is_code_block: false,
            });
            continue;
        }
        if let Some(content) = trimmed.strip_prefix("## ") {
            lines.push(HoverLine {
                spans: vec![HoverSpan::bold(content)],
                is_code_block: false,
            });
            continue;
        }
        if let Some(content) = trimmed.strip_prefix("### ") {
            lines.push(HoverLine {
                spans: vec![HoverSpan::bold(content)],
                is_code_block: false,
            });
            continue;
        }

        // Parse inline formatting.
        let spans = parse_inline_spans(raw_line);
        lines.push(HoverLine {
            spans,
            is_code_block: false,
        });
    }

    lines
}

/// Parses inline markdown formatting (bold, italic, inline code) within a line.
fn parse_inline_spans(line: &str) -> Vec<HoverSpan> {
    let mut spans = Vec::new();
    let mut chars = line.chars().peekable();
    let mut current = String::new();

    while let Some(ch) = chars.next() {
        if ch == '`' {
            // Inline code.
            if !current.is_empty() {
                spans.push(HoverSpan::plain(std::mem::take(&mut current)));
            }
            let mut code_text = String::new();
            let mut found_end = false;
            for c in chars.by_ref() {
                if c == '`' {
                    found_end = true;
                    break;
                }
                code_text.push(c);
            }
            if found_end {
                spans.push(HoverSpan::code(code_text));
            } else {
                // No closing backtick — treat as literal.
                current.push('`');
                current.push_str(&code_text);
            }
        } else if ch == '*' {
            if chars.peek() == Some(&'*') {
                // Bold: **...**
                chars.next();
                if !current.is_empty() {
                    spans.push(HoverSpan::plain(std::mem::take(&mut current)));
                }
                let mut bold_text = String::new();
                let mut found_end = false;
                while let Some(c) = chars.next() {
                    if c == '*' && chars.peek() == Some(&'*') {
                        chars.next();
                        found_end = true;
                        break;
                    }
                    bold_text.push(c);
                }
                if found_end {
                    spans.push(HoverSpan::bold(bold_text));
                } else {
                    current.push_str("**");
                    current.push_str(&bold_text);
                }
            } else {
                // Italic: *...*
                if !current.is_empty() {
                    spans.push(HoverSpan::plain(std::mem::take(&mut current)));
                }
                let mut italic_text = String::new();
                let mut found_end = false;
                for c in chars.by_ref() {
                    if c == '*' {
                        found_end = true;
                        break;
                    }
                    italic_text.push(c);
                }
                if found_end {
                    spans.push(HoverSpan::italic(italic_text));
                } else {
                    current.push('*');
                    current.push_str(&italic_text);
                }
            }
        } else {
            current.push(ch);
        }
    }

    if !current.is_empty() {
        spans.push(HoverSpan::plain(current));
    }

    if spans.is_empty() {
        spans.push(HoverSpan::plain(String::new()));
    }

    spans
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_hover_null() {
        assert!(parse_hover_response(&Value::Null).is_none());
    }

    #[test]
    fn parse_hover_markup_content_markdown() {
        let value = serde_json::json!({
            "contents": {
                "kind": "markdown",
                "value": "**bold** text"
            }
        });
        let info = parse_hover_response(&value).expect("should parse");
        assert!(!info.lines.is_empty());
        // First line should contain bold and plain spans.
        assert_eq!(info.lines[0].spans.len(), 2);
        assert!(info.lines[0].spans[0].bold);
        assert_eq!(info.lines[0].spans[0].text, "bold");
        assert_eq!(info.lines[0].spans[1].text, " text");
    }

    #[test]
    fn parse_hover_markup_content_plaintext() {
        let value = serde_json::json!({
            "contents": {
                "kind": "plaintext",
                "value": "plain text"
            }
        });
        let info = parse_hover_response(&value).expect("should parse");
        assert_eq!(info.lines.len(), 1);
        assert_eq!(info.lines[0].spans[0].text, "plain text");
        assert!(!info.lines[0].spans[0].bold);
    }

    #[test]
    fn parse_hover_marked_string() {
        let value = serde_json::json!({
            "contents": "simple string"
        });
        let info = parse_hover_response(&value).expect("should parse");
        assert_eq!(info.lines.len(), 1);
        assert_eq!(info.lines[0].spans[0].text, "simple string");
    }

    #[test]
    fn parse_hover_marked_string_with_language() {
        let value = serde_json::json!({
            "contents": {
                "language": "rust",
                "value": "fn foo()"
            }
        });
        let info = parse_hover_response(&value).expect("should parse");
        // Should produce a code block line.
        assert!(info.lines.iter().any(|l| l.is_code_block));
        let code_line = info.lines.iter().find(|l| l.is_code_block).unwrap();
        assert_eq!(code_line.spans[0].text, "fn foo()");
    }

    #[test]
    fn parse_hover_array_of_marked_strings() {
        let value = serde_json::json!({
            "contents": [
                { "language": "rust", "value": "fn bar()" },
                "Some documentation"
            ]
        });
        let info = parse_hover_response(&value).expect("should parse");
        // Should have code block lines and plain text lines.
        assert!(info.lines.iter().any(|l| l.is_code_block));
        assert!(info.lines.iter().any(|l| !l.is_code_block));
    }

    #[test]
    fn parse_hover_empty_value_returns_none() {
        let value = serde_json::json!({
            "contents": {
                "kind": "plaintext",
                "value": ""
            }
        });
        assert!(parse_hover_response(&value).is_none());
    }

    #[test]
    fn markdown_bold() {
        let lines = markdown_to_hover_lines("**hello**");
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].spans.len(), 1);
        assert!(lines[0].spans[0].bold);
        assert_eq!(lines[0].spans[0].text, "hello");
    }

    #[test]
    fn markdown_italic() {
        let lines = markdown_to_hover_lines("*world*");
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].spans.len(), 1);
        assert!(lines[0].spans[0].italic);
        assert_eq!(lines[0].spans[0].text, "world");
    }

    #[test]
    fn markdown_inline_code() {
        let lines = markdown_to_hover_lines("`code`");
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].spans.len(), 1);
        assert!(lines[0].spans[0].code);
        assert_eq!(lines[0].spans[0].text, "code");
    }

    #[test]
    fn markdown_code_block() {
        let text = "```rust\nfn main() {}\n```";
        let lines = markdown_to_hover_lines(text);
        assert_eq!(lines.len(), 1);
        assert!(lines[0].is_code_block);
        assert_eq!(lines[0].spans[0].text, "fn main() {}");
    }

    #[test]
    fn markdown_header() {
        let lines = markdown_to_hover_lines("# Title");
        assert_eq!(lines.len(), 1);
        assert!(lines[0].spans[0].bold);
        assert_eq!(lines[0].spans[0].text, "Title");
    }

    #[test]
    fn markdown_plain_text() {
        let lines = markdown_to_hover_lines("plain text");
        assert_eq!(lines.len(), 1);
        assert!(!lines[0].spans[0].bold);
        assert!(!lines[0].spans[0].italic);
        assert!(!lines[0].spans[0].code);
        assert_eq!(lines[0].spans[0].text, "plain text");
    }

    #[test]
    fn markdown_separator() {
        let lines = markdown_to_hover_lines("before\n---\nafter");
        assert_eq!(lines.len(), 3);
        // Separator line has empty spans.
        assert!(lines[1].spans.is_empty());
    }

    #[test]
    fn markdown_mixed_inline() {
        let lines = markdown_to_hover_lines("a **bold** and `code` end");
        assert_eq!(lines.len(), 1);
        let spans = &lines[0].spans;
        assert_eq!(spans[0].text, "a ");
        assert!(spans[1].bold);
        assert_eq!(spans[1].text, "bold");
        assert_eq!(spans[2].text, " and ");
        assert!(spans[3].code);
        assert_eq!(spans[3].text, "code");
        assert_eq!(spans[4].text, " end");
    }

    #[test]
    fn markdown_multiline_code_block() {
        let text = "```\nline1\nline2\n```";
        let lines = markdown_to_hover_lines(text);
        assert_eq!(lines.len(), 2);
        assert!(lines[0].is_code_block);
        assert!(lines[1].is_code_block);
        assert_eq!(lines[0].spans[0].text, "line1");
        assert_eq!(lines[1].spans[0].text, "line2");
    }
}
