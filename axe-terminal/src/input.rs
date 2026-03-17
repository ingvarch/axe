// IMPACT ANALYSIS — input module
// Parents: AppState::handle_key_event() calls key_to_bytes() when terminal is focused.
// Children: The returned bytes are written to the PTY via TerminalTab::write().
// Siblings: Global keybindings (Ctrl+Q, Tab, etc.) are checked BEFORE this module is called,
//           so those keys never reach key_to_bytes(). Tree key interception is a sibling pattern.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

/// Converts a crossterm `KeyEvent` into the byte sequence expected by the PTY.
///
/// Returns `None` for keys that should not be forwarded (e.g., unrecognized function keys).
/// The `app_cursor` parameter controls whether arrow keys use application cursor mode
/// sequences (`\x1BO{A,B,C,D}`) or normal mode sequences (`\x1B[{A,B,C,D}`).
pub fn key_to_bytes(key: &KeyEvent, app_cursor: bool) -> Option<Vec<u8>> {
    let mods = key.modifiers;

    // Alt modifier: send ESC prefix followed by the character bytes.
    if mods.contains(KeyModifiers::ALT) && !mods.contains(KeyModifiers::CONTROL) {
        if let KeyCode::Char(c) = key.code {
            let mut bytes = vec![0x1B];
            let mut buf = [0u8; 4];
            bytes.extend_from_slice(c.encode_utf8(&mut buf).as_bytes());
            return Some(bytes);
        }
    }

    // Ctrl+char: produce control byte (ASCII 1-26).
    if mods.contains(KeyModifiers::CONTROL) {
        if let KeyCode::Char(c) = key.code {
            let lower = c.to_ascii_lowercase();
            if lower.is_ascii_lowercase() {
                let ctrl_byte = lower as u8 - b'a' + 1;
                return Some(vec![ctrl_byte]);
            }
        }
    }

    match key.code {
        KeyCode::Char(c) => {
            let mut buf = [0u8; 4];
            Some(c.encode_utf8(&mut buf).as_bytes().to_vec())
        }
        KeyCode::Enter => Some(vec![0x0D]),
        KeyCode::Backspace => Some(vec![0x7F]),
        KeyCode::Tab => Some(vec![0x09]),
        KeyCode::BackTab => Some(b"\x1B[Z".to_vec()),
        KeyCode::Esc => Some(vec![0x1B]),
        KeyCode::Up => Some(arrow_key(b'A', app_cursor)),
        KeyCode::Down => Some(arrow_key(b'B', app_cursor)),
        KeyCode::Right => Some(arrow_key(b'C', app_cursor)),
        KeyCode::Left => Some(arrow_key(b'D', app_cursor)),
        KeyCode::Home => Some(b"\x1B[H".to_vec()),
        KeyCode::End => Some(b"\x1B[F".to_vec()),
        KeyCode::Delete => Some(b"\x1B[3~".to_vec()),
        KeyCode::PageUp => Some(b"\x1B[5~".to_vec()),
        KeyCode::PageDown => Some(b"\x1B[6~".to_vec()),
        KeyCode::Insert => Some(b"\x1B[2~".to_vec()),
        KeyCode::F(n) => f_key_bytes(n),
        _ => None,
    }
}

/// Produces the escape sequence for an arrow key, respecting application cursor mode.
fn arrow_key(suffix: u8, app_cursor: bool) -> Vec<u8> {
    if app_cursor {
        vec![0x1B, b'O', suffix]
    } else {
        vec![0x1B, b'[', suffix]
    }
}

/// Produces the escape sequence for function keys F1-F12.
fn f_key_bytes(n: u8) -> Option<Vec<u8>> {
    let seq = match n {
        1 => b"\x1BOP".as_slice(),
        2 => b"\x1BOQ",
        3 => b"\x1BOR",
        4 => b"\x1BOS",
        5 => b"\x1B[15~",
        6 => b"\x1B[17~",
        7 => b"\x1B[18~",
        8 => b"\x1B[19~",
        9 => b"\x1B[20~",
        10 => b"\x1B[21~",
        11 => b"\x1B[23~",
        12 => b"\x1B[24~",
        _ => return None,
    };
    Some(seq.to_vec())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    fn key_with(code: KeyCode, mods: KeyModifiers) -> KeyEvent {
        KeyEvent::new(code, mods)
    }

    #[test]
    fn printable_char_a() {
        let bytes = key_to_bytes(&key(KeyCode::Char('a')), false);
        assert_eq!(bytes, Some(vec![0x61]));
    }

    #[test]
    fn printable_char_unicode() {
        let bytes = key_to_bytes(&key(KeyCode::Char('ё')), false);
        // 'ё' is U+0451, UTF-8 encoding: [0xD1, 0x91]
        assert_eq!(bytes, Some(vec![0xD1, 0x91]));
    }

    #[test]
    fn ctrl_c_sends_etx() {
        let bytes = key_to_bytes(&key_with(KeyCode::Char('c'), KeyModifiers::CONTROL), false);
        assert_eq!(bytes, Some(vec![0x03]));
    }

    #[test]
    fn ctrl_d_sends_eot() {
        let bytes = key_to_bytes(&key_with(KeyCode::Char('d'), KeyModifiers::CONTROL), false);
        assert_eq!(bytes, Some(vec![0x04]));
    }

    #[test]
    fn enter_sends_cr() {
        let bytes = key_to_bytes(&key(KeyCode::Enter), false);
        assert_eq!(bytes, Some(vec![0x0D]));
    }

    #[test]
    fn backspace_sends_del() {
        let bytes = key_to_bytes(&key(KeyCode::Backspace), false);
        assert_eq!(bytes, Some(vec![0x7F]));
    }

    #[test]
    fn tab_sends_ht() {
        let bytes = key_to_bytes(&key(KeyCode::Tab), false);
        assert_eq!(bytes, Some(vec![0x09]));
    }

    #[test]
    fn escape_sends_esc() {
        let bytes = key_to_bytes(&key(KeyCode::Esc), false);
        assert_eq!(bytes, Some(vec![0x1B]));
    }

    #[test]
    fn arrow_up_normal() {
        let bytes = key_to_bytes(&key(KeyCode::Up), false);
        assert_eq!(bytes, Some(vec![0x1B, b'[', b'A']));
    }

    #[test]
    fn arrow_up_app_cursor() {
        let bytes = key_to_bytes(&key(KeyCode::Up), true);
        assert_eq!(bytes, Some(vec![0x1B, b'O', b'A']));
    }

    #[test]
    fn arrow_down_normal() {
        let bytes = key_to_bytes(&key(KeyCode::Down), false);
        assert_eq!(bytes, Some(vec![0x1B, b'[', b'B']));
    }

    #[test]
    fn arrow_right_normal() {
        let bytes = key_to_bytes(&key(KeyCode::Right), false);
        assert_eq!(bytes, Some(vec![0x1B, b'[', b'C']));
    }

    #[test]
    fn arrow_left_normal() {
        let bytes = key_to_bytes(&key(KeyCode::Left), false);
        assert_eq!(bytes, Some(vec![0x1B, b'[', b'D']));
    }

    #[test]
    fn delete_sends_sequence() {
        let bytes = key_to_bytes(&key(KeyCode::Delete), false);
        assert_eq!(bytes, Some(b"\x1B[3~".to_vec()));
    }

    #[test]
    fn home_sends_sequence() {
        let bytes = key_to_bytes(&key(KeyCode::Home), false);
        assert_eq!(bytes, Some(b"\x1B[H".to_vec()));
    }

    #[test]
    fn end_sends_sequence() {
        let bytes = key_to_bytes(&key(KeyCode::End), false);
        assert_eq!(bytes, Some(b"\x1B[F".to_vec()));
    }

    #[test]
    fn page_up_sends_sequence() {
        let bytes = key_to_bytes(&key(KeyCode::PageUp), false);
        assert_eq!(bytes, Some(b"\x1B[5~".to_vec()));
    }

    #[test]
    fn page_down_sends_sequence() {
        let bytes = key_to_bytes(&key(KeyCode::PageDown), false);
        assert_eq!(bytes, Some(b"\x1B[6~".to_vec()));
    }

    #[test]
    fn insert_sends_sequence() {
        let bytes = key_to_bytes(&key(KeyCode::Insert), false);
        assert_eq!(bytes, Some(b"\x1B[2~".to_vec()));
    }

    #[test]
    fn f1_sends_sequence() {
        let bytes = key_to_bytes(&key(KeyCode::F(1)), false);
        assert_eq!(bytes, Some(b"\x1BOP".to_vec()));
    }

    #[test]
    fn f5_sends_sequence() {
        let bytes = key_to_bytes(&key(KeyCode::F(5)), false);
        assert_eq!(bytes, Some(b"\x1B[15~".to_vec()));
    }

    #[test]
    fn f12_sends_sequence() {
        let bytes = key_to_bytes(&key(KeyCode::F(12)), false);
        assert_eq!(bytes, Some(b"\x1B[24~".to_vec()));
    }

    #[test]
    fn alt_char_sends_esc_prefix() {
        let bytes = key_to_bytes(&key_with(KeyCode::Char('a'), KeyModifiers::ALT), false);
        assert_eq!(bytes, Some(vec![0x1B, 0x61]));
    }

    #[test]
    fn backtab_sends_reverse_tab_sequence() {
        let bytes = key_to_bytes(&key(KeyCode::BackTab), false);
        assert_eq!(bytes, Some(b"\x1B[Z".to_vec()));
    }

    #[test]
    fn unknown_key_returns_none() {
        let bytes = key_to_bytes(&key(KeyCode::F(20)), false);
        assert_eq!(bytes, None);
    }

    #[test]
    fn ctrl_a_sends_soh() {
        let bytes = key_to_bytes(&key_with(KeyCode::Char('a'), KeyModifiers::CONTROL), false);
        assert_eq!(bytes, Some(vec![0x01]));
    }

    #[test]
    fn ctrl_z_sends_sub() {
        let bytes = key_to_bytes(&key_with(KeyCode::Char('z'), KeyModifiers::CONTROL), false);
        assert_eq!(bytes, Some(vec![0x1A]));
    }
}
