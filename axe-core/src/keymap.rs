use std::collections::HashMap;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::command::Command;

/// Parses a key combo string into crossterm modifiers and key code.
///
/// Format: `[modifier+]*key` where modifiers are `ctrl`, `alt`, `shift`
/// (case-insensitive) and key is a character or special key name.
///
/// # Examples
///
/// ```
/// use axe_core::keymap::parse_key_combo;
/// use crossterm::event::{KeyCode, KeyModifiers};
///
/// assert_eq!(
///     parse_key_combo("ctrl+q"),
///     Some((KeyModifiers::CONTROL, KeyCode::Char('q')))
/// );
/// ```
pub fn parse_key_combo(s: &str) -> Option<(KeyModifiers, KeyCode)> {
    if s.is_empty() {
        return None;
    }

    let parts: Vec<&str> = s.split('+').collect();
    if parts.is_empty() {
        return None;
    }

    // Last part is the key, everything before is modifiers.
    let key_part = *parts.last()?;
    if key_part.is_empty() {
        return None;
    }

    let modifier_parts = &parts[..parts.len() - 1];
    let mut modifiers = KeyModifiers::NONE;
    for &m in modifier_parts {
        match m.to_lowercase().as_str() {
            "ctrl" => modifiers |= KeyModifiers::CONTROL,
            "alt" => modifiers |= KeyModifiers::ALT,
            "shift" => modifiers |= KeyModifiers::SHIFT,
            _ => return None,
        }
    }

    let code = parse_key_name(key_part)?;
    Some((modifiers, code))
}

/// Parses a key name string into a `KeyCode`.
fn parse_key_name(s: &str) -> Option<KeyCode> {
    let lower = s.to_lowercase();
    match lower.as_str() {
        "esc" => Some(KeyCode::Esc),
        "enter" | "return" => Some(KeyCode::Enter),
        "tab" => Some(KeyCode::Tab),
        "backtab" => Some(KeyCode::BackTab),
        "backspace" => Some(KeyCode::Backspace),
        "delete" | "del" => Some(KeyCode::Delete),
        "up" => Some(KeyCode::Up),
        "down" => Some(KeyCode::Down),
        "left" => Some(KeyCode::Left),
        "right" => Some(KeyCode::Right),
        "home" => Some(KeyCode::Home),
        "end" => Some(KeyCode::End),
        "pageup" => Some(KeyCode::PageUp),
        "pagedown" => Some(KeyCode::PageDown),
        "space" => Some(KeyCode::Char(' ')),
        _ if lower.starts_with('f') && lower.len() >= 2 => {
            let num: u8 = lower[1..].parse().ok()?;
            if (1..=12).contains(&num) {
                Some(KeyCode::F(num))
            } else {
                None
            }
        }
        _ => {
            let chars: Vec<char> = s.chars().collect();
            if chars.len() == 1 {
                Some(KeyCode::Char(chars[0]))
            } else {
                None
            }
        }
    }
}

/// Parses a snake_case command name string into a `Command`.
///
/// Parameterized commands use `:` as separator (e.g., `activate_terminal_tab:3`).
/// Returns `None` for unknown command names.
pub fn command_from_str(s: &str) -> Option<Command> {
    // Check for parameterized commands first.
    if let Some((name, param)) = s.split_once(':') {
        return match name {
            "activate_terminal_tab" => {
                let idx: usize = param.parse().ok()?;
                Some(Command::ActivateTerminalTab(idx))
            }
            "activate_buffer" => {
                let idx: usize = param.parse().ok()?;
                Some(Command::ActivateBuffer(idx))
            }
            _ => None,
        };
    }

    match s {
        "quit" => Some(Command::Quit),
        "request_quit" => Some(Command::RequestQuit),
        "save" => Some(Command::EditorSave),
        "toggle_tree" => Some(Command::ToggleTree),
        "toggle_terminal" => Some(Command::ToggleTerminal),
        "focus_next" => Some(Command::FocusNext),
        "focus_prev" => Some(Command::FocusPrev),
        "focus_tree" => Some(Command::FocusTree),
        "focus_editor" => Some(Command::FocusEditor),
        "focus_terminal" => Some(Command::FocusTerminal),
        "show_help" => Some(Command::ShowHelp),
        "close_overlay" => Some(Command::CloseOverlay),
        "enter_resize_mode" => Some(Command::EnterResizeMode),
        "exit_resize_mode" => Some(Command::ExitResizeMode),
        "resize_left" => Some(Command::ResizeLeft),
        "resize_right" => Some(Command::ResizeRight),
        "resize_up" => Some(Command::ResizeUp),
        "resize_down" => Some(Command::ResizeDown),
        "equalize_layout" => Some(Command::EqualizeLayout),
        "zoom_panel" => Some(Command::ZoomPanel),
        "undo" => Some(Command::EditorUndo),
        "redo" => Some(Command::EditorRedo),
        "copy" => Some(Command::EditorCopy),
        "cut" => Some(Command::EditorCut),
        "paste" => Some(Command::EditorPaste),
        "select_all" => Some(Command::EditorSelectAll),
        "find" => Some(Command::EditorFind),
        "close_buffer" => Some(Command::CloseBuffer),
        "next_buffer" => Some(Command::NextBuffer),
        "prev_buffer" => Some(Command::PrevBuffer),
        "new_terminal_tab" => Some(Command::NewTerminalTab),
        "close_terminal_tab" => Some(Command::CloseTerminalTab),
        "toggle_icons" => Some(Command::ToggleIcons),
        "toggle_ignored" => Some(Command::ToggleIgnored),
        "scroll_terminal_up" => Some(Command::TerminalScrollPageUp),
        "scroll_terminal_down" => Some(Command::TerminalScrollPageDown),
        "scroll_terminal_top" => Some(Command::TerminalScrollTop),
        "scroll_terminal_bottom" => Some(Command::TerminalScrollBottom),
        "tree_up" => Some(Command::TreeUp),
        "tree_down" => Some(Command::TreeDown),
        "tree_toggle" => Some(Command::TreeToggle),
        "tree_expand" => Some(Command::TreeExpand),
        "tree_collapse_or_parent" => Some(Command::TreeCollapseOrParent),
        "tree_home" => Some(Command::TreeHome),
        "tree_end" => Some(Command::TreeEnd),
        "tree_create_file" => Some(Command::TreeCreateFile),
        "tree_create_dir" => Some(Command::TreeCreateDir),
        "tree_rename" => Some(Command::TreeRename),
        "tree_delete" => Some(Command::TreeDelete),
        "confirm_close_buffer" => Some(Command::ConfirmCloseBuffer),
        "cancel_close_buffer" => Some(Command::CancelCloseBuffer),
        "search_close" => Some(Command::SearchClose),
        "search_next_match" => Some(Command::SearchNextMatch),
        "search_prev_match" => Some(Command::SearchPrevMatch),
        "search_toggle_case" => Some(Command::SearchToggleCase),
        "search_toggle_regex" => Some(Command::SearchToggleRegex),
        _ => None,
    }
}

/// Resolves key events to commands using a configurable binding map.
///
/// Currently supports global-only bindings. Context-aware resolution
/// (per-panel bindings) will be added when the architecture requires it.
pub struct KeymapResolver {
    global: HashMap<(KeyModifiers, KeyCode), Command>,
}

impl KeymapResolver {
    /// Creates an empty resolver with no bindings.
    pub fn new() -> Self {
        Self {
            global: HashMap::new(),
        }
    }

    /// Creates a resolver pre-loaded with the default keybindings.
    pub fn with_defaults() -> Self {
        let mut resolver = Self::new();

        resolver.bind(
            KeyModifiers::CONTROL,
            KeyCode::Char('q'),
            Command::RequestQuit,
        );
        resolver.bind(KeyModifiers::SHIFT, KeyCode::BackTab, Command::FocusPrev);
        resolver.bind(
            KeyModifiers::CONTROL,
            KeyCode::Char('1'),
            Command::FocusTree,
        );
        resolver.bind(
            KeyModifiers::CONTROL,
            KeyCode::Char('2'),
            Command::FocusEditor,
        );
        resolver.bind(
            KeyModifiers::CONTROL,
            KeyCode::Char('3'),
            Command::FocusTerminal,
        );
        resolver.bind(
            KeyModifiers::CONTROL,
            KeyCode::Char('b'),
            Command::ToggleTree,
        );
        resolver.bind(
            KeyModifiers::CONTROL,
            KeyCode::Char('t'),
            Command::ToggleTerminal,
        );
        resolver.bind(KeyModifiers::CONTROL, KeyCode::Char('h'), Command::ShowHelp);
        resolver.bind(KeyModifiers::NONE, KeyCode::Esc, Command::CloseOverlay);
        resolver.bind(
            KeyModifiers::CONTROL,
            KeyCode::Char('r'),
            Command::EnterResizeMode,
        );
        resolver.bind(
            KeyModifiers::CONTROL,
            KeyCode::Char('z'),
            Command::EditorUndo,
        );
        resolver.bind(KeyModifiers::ALT, KeyCode::Char('z'), Command::ZoomPanel);
        resolver.bind(
            KeyModifiers::CONTROL,
            KeyCode::Char('y'),
            Command::EditorRedo,
        );
        // Ctrl+Shift+Z for redo — crossterm reports uppercase 'Z' with CONTROL|SHIFT.
        // Note: Ctrl+Shift is unreliable in many terminals, so Ctrl+Y is the primary binding.
        resolver.bind(
            KeyModifiers::CONTROL | KeyModifiers::SHIFT,
            KeyCode::Char('Z'),
            Command::EditorRedo,
        );
        resolver.bind(
            KeyModifiers::CONTROL,
            KeyCode::Char('g'),
            Command::ToggleIgnored,
        );
        resolver.bind(
            KeyModifiers::CONTROL,
            KeyCode::Char('i'),
            Command::ToggleIcons,
        );
        resolver.bind(
            KeyModifiers::CONTROL,
            KeyCode::Char('s'),
            Command::EditorSave,
        );
        resolver.bind(
            KeyModifiers::CONTROL,
            KeyCode::Char('a'),
            Command::EditorSelectAll,
        );
        resolver.bind(
            KeyModifiers::CONTROL,
            KeyCode::Char('c'),
            Command::EditorCopy,
        );
        resolver.bind(
            KeyModifiers::CONTROL,
            KeyCode::Char('x'),
            Command::EditorCut,
        );
        resolver.bind(
            KeyModifiers::CONTROL,
            KeyCode::Char('v'),
            Command::EditorPaste,
        );
        resolver.bind(
            KeyModifiers::CONTROL,
            KeyCode::Char('f'),
            Command::EditorFind,
        );

        // Buffer tab management — Alt+] / Alt+[ to avoid terminal emulator conflicts
        // (Ctrl+Tab is intercepted by many terminals like Rio, iTerm2, etc.)
        resolver.bind(KeyModifiers::ALT, KeyCode::Char(']'), Command::NextBuffer);
        resolver.bind(KeyModifiers::ALT, KeyCode::Char('['), Command::PrevBuffer);
        resolver.bind(
            KeyModifiers::CONTROL,
            KeyCode::Char('w'),
            Command::CloseBuffer,
        );

        // Terminal tab management — using Alt to avoid Ctrl+Shift unreliability
        // in terminal emulators (many report Ctrl+Shift+T as plain Ctrl+T).
        resolver.bind(
            KeyModifiers::ALT,
            KeyCode::Char('t'),
            Command::NewTerminalTab,
        );
        resolver.bind(
            KeyModifiers::ALT,
            KeyCode::Char('w'),
            Command::CloseTerminalTab,
        );

        // Terminal scrollback navigation.
        resolver.bind(
            KeyModifiers::SHIFT,
            KeyCode::PageUp,
            Command::TerminalScrollPageUp,
        );
        resolver.bind(
            KeyModifiers::SHIFT,
            KeyCode::PageDown,
            Command::TerminalScrollPageDown,
        );
        resolver.bind(
            KeyModifiers::SHIFT,
            KeyCode::Home,
            Command::TerminalScrollTop,
        );
        resolver.bind(
            KeyModifiers::SHIFT,
            KeyCode::End,
            Command::TerminalScrollBottom,
        );

        // Alt+1-9: direct terminal tab access (1-indexed for the user, 0-indexed internally).
        for i in 1..=9u8 {
            resolver.bind(
                KeyModifiers::ALT,
                KeyCode::Char((b'0' + i) as char),
                Command::ActivateTerminalTab((i - 1) as usize),
            );
        }

        resolver
    }

    /// Adds or overwrites a keybinding.
    pub fn bind(&mut self, modifiers: KeyModifiers, code: KeyCode, cmd: Command) {
        self.global.insert((modifiers, code), cmd);
    }

    /// Resolves a key event to a command, if one is bound.
    pub fn resolve(&self, key: &KeyEvent) -> Option<Command> {
        self.global.get(&(key.modifiers, key.code)).cloned()
    }

    /// Applies user-configured keybinding overrides on top of defaults.
    ///
    /// Each entry maps a key combo string (e.g., "ctrl+q") to a command name
    /// (e.g., "request_quit"). Invalid entries are collected as warnings.
    pub fn apply_overrides(&mut self, bindings: &HashMap<String, String>) -> Vec<String> {
        let mut warnings = Vec::new();

        for (key_str, cmd_str) in bindings {
            let Some((modifiers, code)) = parse_key_combo(key_str) else {
                warnings.push(format!("Invalid key combo: {key_str:?}"));
                continue;
            };
            let Some(cmd) = command_from_str(cmd_str) else {
                warnings.push(format!(
                    "Unknown command {cmd_str:?} for key combo {key_str:?}"
                ));
                continue;
            };
            self.bind(modifiers, code, cmd);
        }

        warnings
    }
}

impl Default for KeymapResolver {
    fn default() -> Self {
        Self::with_defaults()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolver_returns_none_for_unknown_key() {
        let resolver = KeymapResolver::with_defaults();
        let key = KeyEvent::new(KeyCode::Char('z'), KeyModifiers::NONE);
        assert_eq!(resolver.resolve(&key), None);
    }

    #[test]
    fn resolver_returns_command_for_bound_key() {
        let mut resolver = KeymapResolver::new();
        resolver.bind(KeyModifiers::NONE, KeyCode::Char('x'), Command::Quit);
        let key = KeyEvent::new(KeyCode::Char('x'), KeyModifiers::NONE);
        assert_eq!(resolver.resolve(&key), Some(Command::Quit));
    }

    #[test]
    fn default_bindings_ctrl_q_requests_quit() {
        let resolver = KeymapResolver::with_defaults();
        let key = KeyEvent::new(KeyCode::Char('q'), KeyModifiers::CONTROL);
        assert_eq!(resolver.resolve(&key), Some(Command::RequestQuit));
    }

    #[test]
    fn tab_is_not_bound() {
        let resolver = KeymapResolver::with_defaults();
        let key = KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE);
        assert_eq!(resolver.resolve(&key), None);
    }

    #[test]
    fn default_bindings_ctrl_c_copies() {
        let resolver = KeymapResolver::with_defaults();
        let key = KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL);
        assert_eq!(resolver.resolve(&key), Some(Command::EditorCopy));
    }

    #[test]
    fn default_bindings_backtab_focus_prev() {
        let resolver = KeymapResolver::with_defaults();
        let key = KeyEvent::new(KeyCode::BackTab, KeyModifiers::SHIFT);
        assert_eq!(resolver.resolve(&key), Some(Command::FocusPrev));
    }

    #[test]
    fn default_bindings_ctrl_1_focus_tree() {
        let resolver = KeymapResolver::with_defaults();
        let key = KeyEvent::new(KeyCode::Char('1'), KeyModifiers::CONTROL);
        assert_eq!(resolver.resolve(&key), Some(Command::FocusTree));
    }

    #[test]
    fn default_bindings_ctrl_2_focus_editor() {
        let resolver = KeymapResolver::with_defaults();
        let key = KeyEvent::new(KeyCode::Char('2'), KeyModifiers::CONTROL);
        assert_eq!(resolver.resolve(&key), Some(Command::FocusEditor));
    }

    #[test]
    fn default_bindings_ctrl_3_focus_terminal() {
        let resolver = KeymapResolver::with_defaults();
        let key = KeyEvent::new(KeyCode::Char('3'), KeyModifiers::CONTROL);
        assert_eq!(resolver.resolve(&key), Some(Command::FocusTerminal));
    }

    #[test]
    fn default_bindings_ctrl_b_toggle_tree() {
        let resolver = KeymapResolver::with_defaults();
        let key = KeyEvent::new(KeyCode::Char('b'), KeyModifiers::CONTROL);
        assert_eq!(resolver.resolve(&key), Some(Command::ToggleTree));
    }

    #[test]
    fn default_bindings_ctrl_t_toggle_terminal() {
        let resolver = KeymapResolver::with_defaults();
        let key = KeyEvent::new(KeyCode::Char('t'), KeyModifiers::CONTROL);
        assert_eq!(resolver.resolve(&key), Some(Command::ToggleTerminal));
    }

    #[test]
    fn default_bindings_ctrl_h_show_help() {
        let resolver = KeymapResolver::with_defaults();
        let key = KeyEvent::new(KeyCode::Char('h'), KeyModifiers::CONTROL);
        assert_eq!(resolver.resolve(&key), Some(Command::ShowHelp));
    }

    #[test]
    fn default_bindings_esc_close_overlay() {
        let resolver = KeymapResolver::with_defaults();
        let key = KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE);
        assert_eq!(resolver.resolve(&key), Some(Command::CloseOverlay));
    }

    #[test]
    fn default_bindings_ctrl_r_enters_resize_mode() {
        let resolver = KeymapResolver::with_defaults();
        let key = KeyEvent::new(KeyCode::Char('r'), KeyModifiers::CONTROL);
        assert_eq!(resolver.resolve(&key), Some(Command::EnterResizeMode));
    }

    #[test]
    fn default_bindings_ctrl_z_undoes() {
        let resolver = KeymapResolver::with_defaults();
        let key = KeyEvent::new(KeyCode::Char('z'), KeyModifiers::CONTROL);
        assert_eq!(resolver.resolve(&key), Some(Command::EditorUndo));
    }

    #[test]
    fn default_bindings_alt_z_zoom_panel() {
        let resolver = KeymapResolver::with_defaults();
        let key = KeyEvent::new(KeyCode::Char('z'), KeyModifiers::ALT);
        assert_eq!(resolver.resolve(&key), Some(Command::ZoomPanel));
    }

    #[test]
    fn default_bindings_ctrl_y_redoes() {
        let resolver = KeymapResolver::with_defaults();
        let key = KeyEvent::new(KeyCode::Char('y'), KeyModifiers::CONTROL);
        assert_eq!(resolver.resolve(&key), Some(Command::EditorRedo));
    }

    #[test]
    fn default_bindings_ctrl_shift_z_redoes() {
        let resolver = KeymapResolver::with_defaults();
        let key = KeyEvent::new(
            KeyCode::Char('Z'),
            KeyModifiers::CONTROL | KeyModifiers::SHIFT,
        );
        assert_eq!(resolver.resolve(&key), Some(Command::EditorRedo));
    }

    #[test]
    fn default_bindings_ctrl_g_toggle_ignored() {
        let resolver = KeymapResolver::with_defaults();
        let key = KeyEvent::new(KeyCode::Char('g'), KeyModifiers::CONTROL);
        assert_eq!(resolver.resolve(&key), Some(Command::ToggleIgnored));
    }

    #[test]
    fn default_bindings_ctrl_i_toggle_icons() {
        let resolver = KeymapResolver::with_defaults();
        let key = KeyEvent::new(KeyCode::Char('i'), KeyModifiers::CONTROL);
        assert_eq!(resolver.resolve(&key), Some(Command::ToggleIcons));
    }

    #[test]
    fn default_bindings_alt_t_new_terminal_tab() {
        let resolver = KeymapResolver::with_defaults();
        let key = KeyEvent::new(KeyCode::Char('t'), KeyModifiers::ALT);
        assert_eq!(resolver.resolve(&key), Some(Command::NewTerminalTab));
    }

    #[test]
    fn default_bindings_alt_w_close_terminal_tab() {
        let resolver = KeymapResolver::with_defaults();
        let key = KeyEvent::new(KeyCode::Char('w'), KeyModifiers::ALT);
        assert_eq!(resolver.resolve(&key), Some(Command::CloseTerminalTab));
    }

    #[test]
    fn default_bindings_alt_1_activates_terminal_tab_0() {
        let resolver = KeymapResolver::with_defaults();
        let key = KeyEvent::new(KeyCode::Char('1'), KeyModifiers::ALT);
        assert_eq!(
            resolver.resolve(&key),
            Some(Command::ActivateTerminalTab(0))
        );
    }

    #[test]
    fn default_bindings_alt_9_activates_terminal_tab_8() {
        let resolver = KeymapResolver::with_defaults();
        let key = KeyEvent::new(KeyCode::Char('9'), KeyModifiers::ALT);
        assert_eq!(
            resolver.resolve(&key),
            Some(Command::ActivateTerminalTab(8))
        );
    }

    #[test]
    fn default_bindings_shift_page_up_scrolls_terminal() {
        let resolver = KeymapResolver::with_defaults();
        let key = KeyEvent::new(KeyCode::PageUp, KeyModifiers::SHIFT);
        assert_eq!(resolver.resolve(&key), Some(Command::TerminalScrollPageUp));
    }

    #[test]
    fn default_bindings_shift_page_down_scrolls_terminal() {
        let resolver = KeymapResolver::with_defaults();
        let key = KeyEvent::new(KeyCode::PageDown, KeyModifiers::SHIFT);
        assert_eq!(
            resolver.resolve(&key),
            Some(Command::TerminalScrollPageDown)
        );
    }

    #[test]
    fn default_bindings_shift_home_scrolls_terminal_top() {
        let resolver = KeymapResolver::with_defaults();
        let key = KeyEvent::new(KeyCode::Home, KeyModifiers::SHIFT);
        assert_eq!(resolver.resolve(&key), Some(Command::TerminalScrollTop));
    }

    #[test]
    fn default_bindings_shift_end_scrolls_terminal_bottom() {
        let resolver = KeymapResolver::with_defaults();
        let key = KeyEvent::new(KeyCode::End, KeyModifiers::SHIFT);
        assert_eq!(resolver.resolve(&key), Some(Command::TerminalScrollBottom));
    }

    #[test]
    fn default_bindings_ctrl_s_saves() {
        let resolver = KeymapResolver::with_defaults();
        let key = KeyEvent::new(KeyCode::Char('s'), KeyModifiers::CONTROL);
        assert_eq!(resolver.resolve(&key), Some(Command::EditorSave));
    }

    #[test]
    fn default_bindings_ctrl_a_selects_all() {
        let resolver = KeymapResolver::with_defaults();
        let key = KeyEvent::new(KeyCode::Char('a'), KeyModifiers::CONTROL);
        assert_eq!(resolver.resolve(&key), Some(Command::EditorSelectAll));
    }

    #[test]
    fn default_bindings_ctrl_x_cuts() {
        let resolver = KeymapResolver::with_defaults();
        let key = KeyEvent::new(KeyCode::Char('x'), KeyModifiers::CONTROL);
        assert_eq!(resolver.resolve(&key), Some(Command::EditorCut));
    }

    #[test]
    fn default_bindings_ctrl_v_pastes() {
        let resolver = KeymapResolver::with_defaults();
        let key = KeyEvent::new(KeyCode::Char('v'), KeyModifiers::CONTROL);
        assert_eq!(resolver.resolve(&key), Some(Command::EditorPaste));
    }

    #[test]
    fn default_bindings_ctrl_f_finds() {
        let resolver = KeymapResolver::with_defaults();
        let key = KeyEvent::new(KeyCode::Char('f'), KeyModifiers::CONTROL);
        assert_eq!(resolver.resolve(&key), Some(Command::EditorFind));
    }

    #[test]
    fn default_bindings_alt_bracket_right_next_buffer() {
        let resolver = KeymapResolver::with_defaults();
        let key = KeyEvent::new(KeyCode::Char(']'), KeyModifiers::ALT);
        assert_eq!(resolver.resolve(&key), Some(Command::NextBuffer));
    }

    #[test]
    fn default_bindings_alt_bracket_left_prev_buffer() {
        let resolver = KeymapResolver::with_defaults();
        let key = KeyEvent::new(KeyCode::Char('['), KeyModifiers::ALT);
        assert_eq!(resolver.resolve(&key), Some(Command::PrevBuffer));
    }

    #[test]
    fn default_bindings_ctrl_w_close_buffer() {
        let resolver = KeymapResolver::with_defaults();
        let key = KeyEvent::new(KeyCode::Char('w'), KeyModifiers::CONTROL);
        assert_eq!(resolver.resolve(&key), Some(Command::CloseBuffer));
    }

    #[test]
    fn default_bindings_alt_5_activates_terminal_tab_4() {
        let resolver = KeymapResolver::with_defaults();
        let key = KeyEvent::new(KeyCode::Char('5'), KeyModifiers::ALT);
        assert_eq!(
            resolver.resolve(&key),
            Some(Command::ActivateTerminalTab(4))
        );
    }

    // --- Key combo parsing ---

    #[test]
    fn parse_key_combo_ctrl_q() {
        assert_eq!(
            parse_key_combo("ctrl+q"),
            Some((KeyModifiers::CONTROL, KeyCode::Char('q')))
        );
    }

    #[test]
    fn parse_key_combo_ctrl_shift_z() {
        assert_eq!(
            parse_key_combo("ctrl+shift+z"),
            Some((
                KeyModifiers::CONTROL | KeyModifiers::SHIFT,
                KeyCode::Char('z')
            ))
        );
    }

    #[test]
    fn parse_key_combo_alt_bracket_right() {
        assert_eq!(
            parse_key_combo("alt+]"),
            Some((KeyModifiers::ALT, KeyCode::Char(']')))
        );
    }

    #[test]
    fn parse_key_combo_esc() {
        assert_eq!(
            parse_key_combo("esc"),
            Some((KeyModifiers::NONE, KeyCode::Esc))
        );
    }

    #[test]
    fn parse_key_combo_f12() {
        assert_eq!(
            parse_key_combo("f12"),
            Some((KeyModifiers::NONE, KeyCode::F(12)))
        );
    }

    #[test]
    fn parse_key_combo_enter() {
        assert_eq!(
            parse_key_combo("enter"),
            Some((KeyModifiers::NONE, KeyCode::Enter))
        );
    }

    #[test]
    fn parse_key_combo_backtab() {
        assert_eq!(
            parse_key_combo("backtab"),
            Some((KeyModifiers::NONE, KeyCode::BackTab))
        );
    }

    #[test]
    fn parse_key_combo_space() {
        assert_eq!(
            parse_key_combo("space"),
            Some((KeyModifiers::NONE, KeyCode::Char(' ')))
        );
    }

    #[test]
    fn parse_key_combo_invalid_empty() {
        assert_eq!(parse_key_combo(""), None);
    }

    #[test]
    fn parse_key_combo_invalid_modifier_only() {
        assert_eq!(parse_key_combo("ctrl+"), None);
    }

    // --- Command name parsing ---

    #[test]
    fn command_from_str_request_quit() {
        assert_eq!(command_from_str("request_quit"), Some(Command::RequestQuit));
    }

    #[test]
    fn command_from_str_save() {
        assert_eq!(command_from_str("save"), Some(Command::EditorSave));
    }

    #[test]
    fn command_from_str_toggle_tree() {
        assert_eq!(command_from_str("toggle_tree"), Some(Command::ToggleTree));
    }

    #[test]
    fn command_from_str_activate_terminal_tab() {
        assert_eq!(
            command_from_str("activate_terminal_tab:3"),
            Some(Command::ActivateTerminalTab(3))
        );
    }

    #[test]
    fn command_from_str_unknown() {
        assert_eq!(command_from_str("nonexistent"), None);
    }

    // --- Apply overrides ---

    #[test]
    fn apply_overrides_replaces_binding() {
        let mut resolver = KeymapResolver::with_defaults();
        let mut bindings = HashMap::new();
        bindings.insert("ctrl+q".to_string(), "toggle_tree".to_string());
        let warnings = resolver.apply_overrides(&bindings);
        assert!(warnings.is_empty());
        let event = KeyEvent::new(KeyCode::Char('q'), KeyModifiers::CONTROL);
        assert_eq!(resolver.resolve(&event), Some(Command::ToggleTree));
    }

    #[test]
    fn apply_overrides_adds_new_binding() {
        let mut resolver = KeymapResolver::with_defaults();
        let mut bindings = HashMap::new();
        bindings.insert("ctrl+p".to_string(), "find".to_string());
        let warnings = resolver.apply_overrides(&bindings);
        assert!(warnings.is_empty());
        let event = KeyEvent::new(KeyCode::Char('p'), KeyModifiers::CONTROL);
        assert_eq!(resolver.resolve(&event), Some(Command::EditorFind));
    }

    #[test]
    fn apply_overrides_invalid_key_returns_warning() {
        let mut resolver = KeymapResolver::with_defaults();
        let mut bindings = HashMap::new();
        bindings.insert(String::new(), "save".to_string());
        let warnings = resolver.apply_overrides(&bindings);
        assert_eq!(warnings.len(), 1);
    }

    #[test]
    fn apply_overrides_invalid_command_returns_warning() {
        let mut resolver = KeymapResolver::with_defaults();
        let mut bindings = HashMap::new();
        bindings.insert("ctrl+q".to_string(), "nonexistent".to_string());
        let warnings = resolver.apply_overrides(&bindings);
        assert_eq!(warnings.len(), 1);
    }
}
