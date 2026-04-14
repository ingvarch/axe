// IMPACT ANALYSIS — cwd_parser module
// Parents: TerminalTab feeds every chunk of PTY output through CwdOscParser::feed().
// Children: Uses vte::Parser + a private Perform sink to extract OSC 7 (current working directory)
//           sequences emitted by shells like fish/zsh/bash that implement cwd notifications.
// Siblings: alacritty_terminal's ansi::Processor also consumes the same byte stream in tab.rs,
//           but it does not parse OSC 7, so this is not duplicate work — it is strictly additive.
// Risk: OSC 7 sequences may span multiple read chunks; vte::Parser handles buffering so
//       we must not reset it between feeds.

use std::path::PathBuf;

use vte::{Parser, Perform};

/// Parses OSC 7 (`\x1b]7;file://host/path\x07`) sequences out of a PTY byte stream.
///
/// Shells like fish emit this on every prompt to tell the terminal their current working
/// directory. We use a dedicated `vte::Parser` here because `alacritty_terminal` 0.25 does
/// not parse OSC 7 itself.
pub(crate) struct CwdOscParser {
    parser: Parser,
    sink: CwdSink,
}

impl CwdOscParser {
    pub(crate) fn new() -> Self {
        Self {
            parser: Parser::new(),
            sink: CwdSink::default(),
        }
    }

    /// Feeds a chunk of PTY output bytes through the parser.
    ///
    /// Returns the most recently captured cwd path if one surfaced in this chunk,
    /// otherwise `None`. Calling this consumes the captured path, so each new cwd
    /// is reported exactly once.
    pub(crate) fn feed(&mut self, bytes: &[u8]) -> Option<PathBuf> {
        self.parser.advance(&mut self.sink, bytes);
        self.sink.latest.take()
    }
}

#[derive(Default)]
struct CwdSink {
    latest: Option<PathBuf>,
}

impl Perform for CwdSink {
    fn osc_dispatch(&mut self, params: &[&[u8]], _bell_terminated: bool) {
        // OSC 7 has the form: `7 ; file://host/path`
        if params.len() < 2 || params[0] != b"7" {
            return;
        }
        if let Some(path) = parse_file_uri(params[1]) {
            self.latest = Some(path);
        }
    }
}

/// Parses a `file://[host]/path` URI into a `PathBuf`, percent-decoding the path segment.
///
/// Returns `None` for payloads that do not start with `file://` or that lack a path segment.
fn parse_file_uri(bytes: &[u8]) -> Option<PathBuf> {
    let rest = bytes.strip_prefix(b"file://")?;
    // Skip the optional host segment up to the first `/`. If there is no `/`, the URI is
    // malformed (no path).
    let slash = rest.iter().position(|&b| b == b'/')?;
    let path_bytes = &rest[slash..];
    let decoded = percent_decode(path_bytes);
    let s = String::from_utf8(decoded).ok()?;
    if s.is_empty() {
        return None;
    }
    Some(PathBuf::from(s))
}

/// Decodes `%NN` escapes in a URL path into raw bytes.
///
/// Invalid escapes are passed through verbatim.
fn percent_decode(bytes: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let (Some(hi), Some(lo)) = (hex_nibble(bytes[i + 1]), hex_nibble(bytes[i + 2])) {
                out.push((hi << 4) | lo);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    out
}

fn hex_nibble(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_osc_7_bell_terminated() {
        let mut parser = CwdOscParser::new();
        let got = parser.feed(b"\x1b]7;file:///Users/igor/Repos/axe\x07");
        assert_eq!(got, Some(PathBuf::from("/Users/igor/Repos/axe")));
    }

    #[test]
    fn parses_osc_7_st_terminated() {
        let mut parser = CwdOscParser::new();
        let got = parser.feed(b"\x1b]7;file:///Users/igor/Repos/axe\x1b\\");
        assert_eq!(got, Some(PathBuf::from("/Users/igor/Repos/axe")));
    }

    #[test]
    fn parses_osc_7_with_hostname() {
        let mut parser = CwdOscParser::new();
        let got = parser.feed(b"\x1b]7;file://myhost/Users/igor\x07");
        assert_eq!(got, Some(PathBuf::from("/Users/igor")));
    }

    #[test]
    fn decodes_percent_escapes() {
        let mut parser = CwdOscParser::new();
        let got = parser.feed(b"\x1b]7;file:///tmp/My%20Folder\x07");
        assert_eq!(got, Some(PathBuf::from("/tmp/My Folder")));
    }

    #[test]
    fn ignores_non_osc_7() {
        let mut parser = CwdOscParser::new();
        let got = parser.feed(b"\x1b]2;some window title\x07");
        assert_eq!(got, None);
    }

    #[test]
    fn ignores_malformed_uri() {
        let mut parser = CwdOscParser::new();
        let got = parser.feed(b"\x1b]7;notauri\x07");
        assert_eq!(got, None);
    }

    #[test]
    fn handles_split_chunks() {
        let mut parser = CwdOscParser::new();
        let seq = b"\x1b]7;file:///Users/igor/Repos/axe\x07";
        let mut result = None;
        for b in seq {
            if let Some(p) = parser.feed(std::slice::from_ref(b)) {
                result = Some(p);
            }
        }
        assert_eq!(result, Some(PathBuf::from("/Users/igor/Repos/axe")));
    }

    #[test]
    fn second_osc_7_overrides_first() {
        let mut parser = CwdOscParser::new();
        let _ = parser.feed(b"\x1b]7;file:///tmp/a\x07");
        let got = parser.feed(b"\x1b]7;file:///tmp/b\x07");
        assert_eq!(got, Some(PathBuf::from("/tmp/b")));
    }

    #[test]
    fn feed_without_new_cwd_returns_none() {
        let mut parser = CwdOscParser::new();
        let _ = parser.feed(b"\x1b]7;file:///tmp/a\x07");
        let got = parser.feed(b"plain text with no escape");
        assert_eq!(got, None);
    }
}
