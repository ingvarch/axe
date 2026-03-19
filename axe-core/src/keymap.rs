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
        "new_tab" => Some(Command::NewTab),
        "close_tab" => Some(Command::CloseTab),
        "next_tab" => Some(Command::NextTab),
        "prev_tab" => Some(Command::PrevTab),
        "new_terminal_tab" => Some(Command::NewTerminalTab),
        "close_terminal_tab" => Some(Command::CloseTerminalTab),
        "force_close_terminal_tab" => Some(Command::ForceCloseTerminalTab),
        "cancel_close_terminal_tab" => Some(Command::CancelCloseTerminalTab),
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
        "open_file_finder" => Some(Command::OpenFileFinder),
        "open_command_palette" => Some(Command::OpenCommandPalette),
        "open_project_search" => Some(Command::OpenProjectSearch),
        "go_to_next_diagnostic" => Some(Command::GoToNextDiagnostic),
        "go_to_prev_diagnostic" => Some(Command::GoToPrevDiagnostic),
        "trigger_completion" => Some(Command::TriggerCompletion),
        "accept_completion" => Some(Command::AcceptCompletion),
        "dismiss_completion" => Some(Command::DismissCompletion),
        "go_to_definition" => Some(Command::GoToDefinition),
        "find_references" => Some(Command::FindReferences),
        "show_hover" => Some(Command::ShowHover),
        "format_document" => Some(Command::FormatDocument),
        "go_to_line" => Some(Command::GoToLine),
        _ => None,
    }
}

/// Formats a key combo into a human-readable string (inverse of `parse_key_combo`).
///
/// # Examples
/// - `(CONTROL, Char('s'))` -> `"Ctrl+S"`
/// - `(ALT, Char(']'))` -> `"Alt+]"`
/// - `(NONE, Esc)` -> `"Esc"`
pub fn format_key_combo(modifiers: KeyModifiers, code: KeyCode) -> String {
    let mut parts = Vec::new();

    if modifiers.contains(KeyModifiers::CONTROL) {
        parts.push("Ctrl".to_string());
    }
    if modifiers.contains(KeyModifiers::ALT) {
        parts.push("Alt".to_string());
    }
    if modifiers.contains(KeyModifiers::SHIFT) {
        parts.push("Shift".to_string());
    }

    let key = match code {
        KeyCode::Esc => "Esc".to_string(),
        KeyCode::Enter => "Enter".to_string(),
        KeyCode::Tab => "Tab".to_string(),
        KeyCode::BackTab => "BackTab".to_string(),
        KeyCode::Backspace => "Backspace".to_string(),
        KeyCode::Delete => "Delete".to_string(),
        KeyCode::Up => "Up".to_string(),
        KeyCode::Down => "Down".to_string(),
        KeyCode::Left => "Left".to_string(),
        KeyCode::Right => "Right".to_string(),
        KeyCode::Home => "Home".to_string(),
        KeyCode::End => "End".to_string(),
        KeyCode::PageUp => "PageUp".to_string(),
        KeyCode::PageDown => "PageDown".to_string(),
        KeyCode::Char(' ') => "Space".to_string(),
        KeyCode::Char(c) => c.to_uppercase().to_string(),
        KeyCode::F(n) => format!("F{n}"),
        _ => format!("{code:?}"),
    };

    parts.push(key);
    parts.join("+")
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
        resolver.bind(KeyModifiers::ALT, KeyCode::Char('1'), Command::FocusTree);
        resolver.bind(KeyModifiers::ALT, KeyCode::Char('2'), Command::FocusEditor);
        resolver.bind(
            KeyModifiers::ALT,
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
        resolver.bind(KeyModifiers::CONTROL, KeyCode::Char('g'), Command::GoToLine);
        // Ctrl+Shift+G: toggle ignored files (was Ctrl+G before GoToLine took that binding).
        resolver.bind(
            KeyModifiers::CONTROL | KeyModifiers::SHIFT,
            KeyCode::Char('G'),
            Command::ToggleIgnored,
        );
        resolver.bind(
            KeyModifiers::CONTROL | KeyModifiers::SHIFT,
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
        resolver.bind(
            KeyModifiers::CONTROL,
            KeyCode::Char('p'),
            Command::OpenFileFinder,
        );
        // Ctrl+Shift+P: terminals without Kitty protocol report uppercase 'P'.
        resolver.bind(
            KeyModifiers::CONTROL | KeyModifiers::SHIFT,
            KeyCode::Char('P'),
            Command::OpenCommandPalette,
        );
        // Ctrl+Shift+P: terminals with Kitty protocol report lowercase 'p' + SHIFT.
        resolver.bind(
            KeyModifiers::CONTROL | KeyModifiers::SHIFT,
            KeyCode::Char('p'),
            Command::OpenCommandPalette,
        );
        // F1 as universal fallback — works in terminals without Kitty keyboard protocol.
        resolver.bind(
            KeyModifiers::NONE,
            KeyCode::F(1),
            Command::OpenCommandPalette,
        );
        // Ctrl+Shift+F: terminals without Kitty protocol report uppercase 'F'.
        resolver.bind(
            KeyModifiers::CONTROL | KeyModifiers::SHIFT,
            KeyCode::Char('F'),
            Command::OpenProjectSearch,
        );
        // Ctrl+Shift+F: terminals with Kitty protocol report lowercase 'f' + SHIFT.
        resolver.bind(
            KeyModifiers::CONTROL | KeyModifiers::SHIFT,
            KeyCode::Char('f'),
            Command::OpenProjectSearch,
        );
        // F2 as universal fallback for project search.
        resolver.bind(
            KeyModifiers::NONE,
            KeyCode::F(2),
            Command::OpenProjectSearch,
        );

        // Unified tab management — same hotkeys work in both Editor and Terminal
        // based on current focus. Alt+] / Alt+[ avoids terminal emulator conflicts
        // (Ctrl+Tab is intercepted by many terminals like Rio, iTerm2, etc.)
        resolver.bind(KeyModifiers::ALT, KeyCode::Char(']'), Command::NextTab);
        resolver.bind(KeyModifiers::ALT, KeyCode::Char('['), Command::PrevTab);
        resolver.bind(KeyModifiers::CONTROL, KeyCode::Char('w'), Command::CloseTab);
        resolver.bind(KeyModifiers::ALT, KeyCode::Char('t'), Command::NewTab);
        resolver.bind(KeyModifiers::ALT, KeyCode::Char('w'), Command::CloseTab);

        // Code completion (Alt+/).
        resolver.bind(
            KeyModifiers::ALT,
            KeyCode::Char('/'),
            Command::TriggerCompletion,
        );
        // F3 as universal fallback for completion trigger.
        resolver.bind(
            KeyModifiers::NONE,
            KeyCode::F(3),
            Command::TriggerCompletion,
        );

        // Go to definition / Find references.
        resolver.bind(KeyModifiers::NONE, KeyCode::F(12), Command::GoToDefinition);
        resolver.bind(KeyModifiers::SHIFT, KeyCode::F(12), Command::FindReferences);

        // Hover info (Ctrl+Shift+K / F4).
        resolver.bind(
            KeyModifiers::CONTROL | KeyModifiers::SHIFT,
            KeyCode::Char('K'),
            Command::ShowHover,
        );
        resolver.bind(
            KeyModifiers::CONTROL | KeyModifiers::SHIFT,
            KeyCode::Char('k'),
            Command::ShowHover,
        );
        resolver.bind(KeyModifiers::NONE, KeyCode::F(4), Command::ShowHover);

        // Format document.
        resolver.bind(
            KeyModifiers::CONTROL | KeyModifiers::SHIFT,
            KeyCode::Char('I'),
            Command::FormatDocument,
        );
        resolver.bind(
            KeyModifiers::CONTROL | KeyModifiers::SHIFT,
            KeyCode::Char('i'),
            Command::FormatDocument,
        );

        // Diagnostic navigation.
        resolver.bind(
            KeyModifiers::ALT,
            KeyCode::Char('.'),
            Command::GoToNextDiagnostic,
        );
        resolver.bind(
            KeyModifiers::ALT,
            KeyCode::Char(','),
            Command::GoToPrevDiagnostic,
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

        resolver
    }

    /// Adds or overwrites a keybinding.
    pub fn bind(&mut self, modifiers: KeyModifiers, code: KeyCode, cmd: Command) {
        self.global.insert((modifiers, code), cmd);
    }

    /// Returns the human-readable keybinding string for a command, if bound.
    ///
    /// When multiple bindings exist for the same command, returns the shortest
    /// string representation (simplest binding) for deterministic display.
    pub fn binding_for(&self, cmd: &Command) -> Option<String> {
        self.global
            .iter()
            .filter(|(_, c)| *c == cmd)
            .map(|((modifiers, code), _)| format_key_combo(*modifiers, *code))
            .min_by_key(|s| s.len())
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
    fn default_bindings_alt_1_focus_tree() {
        let resolver = KeymapResolver::with_defaults();
        let key = KeyEvent::new(KeyCode::Char('1'), KeyModifiers::ALT);
        assert_eq!(resolver.resolve(&key), Some(Command::FocusTree));
    }

    #[test]
    fn default_bindings_alt_2_focus_editor() {
        let resolver = KeymapResolver::with_defaults();
        let key = KeyEvent::new(KeyCode::Char('2'), KeyModifiers::ALT);
        assert_eq!(resolver.resolve(&key), Some(Command::FocusEditor));
    }

    #[test]
    fn default_bindings_alt_3_focus_terminal() {
        let resolver = KeymapResolver::with_defaults();
        let key = KeyEvent::new(KeyCode::Char('3'), KeyModifiers::ALT);
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
    fn default_bindings_ctrl_g_go_to_line() {
        let resolver = KeymapResolver::with_defaults();
        let key = KeyEvent::new(KeyCode::Char('g'), KeyModifiers::CONTROL);
        assert_eq!(resolver.resolve(&key), Some(Command::GoToLine));
    }

    #[test]
    fn default_bindings_ctrl_shift_g_toggle_ignored() {
        let resolver = KeymapResolver::with_defaults();
        let key = KeyEvent::new(
            KeyCode::Char('G'),
            KeyModifiers::CONTROL | KeyModifiers::SHIFT,
        );
        assert_eq!(resolver.resolve(&key), Some(Command::ToggleIgnored));
    }

    #[test]
    fn default_bindings_ctrl_i_toggle_icons() {
        let resolver = KeymapResolver::with_defaults();
        let key = KeyEvent::new(KeyCode::Char('i'), KeyModifiers::CONTROL);
        assert_eq!(resolver.resolve(&key), Some(Command::ToggleIcons));
    }

    // Alt+T and Alt+W are now bound to unified NewTab/CloseTab (tested above).
    // Legacy NewTerminalTab/CloseTerminalTab are still available via command_from_str.

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

    // Alt+]/[ and Ctrl+W are now bound to unified NextTab/PrevTab/CloseTab (tested above).
    // Legacy NextBuffer/PrevBuffer/CloseBuffer are still available via command_from_str.

    // --- Unified tab command bindings ---

    #[test]
    fn default_bindings_alt_t_new_tab() {
        let resolver = KeymapResolver::with_defaults();
        let key = KeyEvent::new(KeyCode::Char('t'), KeyModifiers::ALT);
        assert_eq!(resolver.resolve(&key), Some(Command::NewTab));
    }

    #[test]
    fn default_bindings_alt_w_close_tab() {
        let resolver = KeymapResolver::with_defaults();
        let key = KeyEvent::new(KeyCode::Char('w'), KeyModifiers::ALT);
        assert_eq!(resolver.resolve(&key), Some(Command::CloseTab));
    }

    #[test]
    fn default_bindings_ctrl_w_close_tab() {
        let resolver = KeymapResolver::with_defaults();
        let key = KeyEvent::new(KeyCode::Char('w'), KeyModifiers::CONTROL);
        assert_eq!(resolver.resolve(&key), Some(Command::CloseTab));
    }

    #[test]
    fn default_bindings_alt_bracket_right_next_tab() {
        let resolver = KeymapResolver::with_defaults();
        let key = KeyEvent::new(KeyCode::Char(']'), KeyModifiers::ALT);
        assert_eq!(resolver.resolve(&key), Some(Command::NextTab));
    }

    #[test]
    fn default_bindings_alt_bracket_left_prev_tab() {
        let resolver = KeymapResolver::with_defaults();
        let key = KeyEvent::new(KeyCode::Char('['), KeyModifiers::ALT);
        assert_eq!(resolver.resolve(&key), Some(Command::PrevTab));
    }

    #[test]
    fn command_from_str_new_tab() {
        assert_eq!(command_from_str("new_tab"), Some(Command::NewTab));
    }

    #[test]
    fn command_from_str_close_tab() {
        assert_eq!(command_from_str("close_tab"), Some(Command::CloseTab));
    }

    #[test]
    fn command_from_str_next_tab() {
        assert_eq!(command_from_str("next_tab"), Some(Command::NextTab));
    }

    #[test]
    fn command_from_str_prev_tab() {
        assert_eq!(command_from_str("prev_tab"), Some(Command::PrevTab));
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
    fn command_from_str_force_close_terminal_tab() {
        assert_eq!(
            command_from_str("force_close_terminal_tab"),
            Some(Command::ForceCloseTerminalTab)
        );
    }

    #[test]
    fn command_from_str_cancel_close_terminal_tab() {
        assert_eq!(
            command_from_str("cancel_close_terminal_tab"),
            Some(Command::CancelCloseTerminalTab)
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
        // Override replaces the default OpenFileFinder binding with EditorFind.
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

    // --- File finder ---

    #[test]
    fn default_bindings_ctrl_p_opens_file_finder() {
        let resolver = KeymapResolver::with_defaults();
        let key = KeyEvent::new(KeyCode::Char('p'), KeyModifiers::CONTROL);
        assert_eq!(resolver.resolve(&key), Some(Command::OpenFileFinder));
    }

    #[test]
    fn command_from_str_open_file_finder() {
        assert_eq!(
            command_from_str("open_file_finder"),
            Some(Command::OpenFileFinder)
        );
    }

    #[test]
    fn default_bindings_ctrl_shift_p_opens_command_palette() {
        let resolver = KeymapResolver::with_defaults();
        let key = KeyEvent::new(
            KeyCode::Char('P'),
            KeyModifiers::CONTROL | KeyModifiers::SHIFT,
        );
        assert_eq!(resolver.resolve(&key), Some(Command::OpenCommandPalette));
    }

    #[test]
    fn default_bindings_ctrl_shift_lowercase_p_opens_command_palette() {
        let resolver = KeymapResolver::with_defaults();
        let key = KeyEvent::new(
            KeyCode::Char('p'),
            KeyModifiers::CONTROL | KeyModifiers::SHIFT,
        );
        assert_eq!(resolver.resolve(&key), Some(Command::OpenCommandPalette));
    }

    #[test]
    fn default_bindings_f1_opens_command_palette() {
        let resolver = KeymapResolver::with_defaults();
        let key = KeyEvent::new(KeyCode::F(1), KeyModifiers::NONE);
        assert_eq!(resolver.resolve(&key), Some(Command::OpenCommandPalette));
    }

    #[test]
    fn command_from_str_open_command_palette() {
        assert_eq!(
            command_from_str("open_command_palette"),
            Some(Command::OpenCommandPalette)
        );
    }

    #[test]
    fn default_bindings_ctrl_shift_f_opens_project_search() {
        let resolver = KeymapResolver::with_defaults();
        let key = KeyEvent::new(
            KeyCode::Char('F'),
            KeyModifiers::CONTROL | KeyModifiers::SHIFT,
        );
        assert_eq!(resolver.resolve(&key), Some(Command::OpenProjectSearch));
    }

    #[test]
    fn default_bindings_ctrl_shift_lowercase_f_opens_project_search() {
        let resolver = KeymapResolver::with_defaults();
        let key = KeyEvent::new(
            KeyCode::Char('f'),
            KeyModifiers::CONTROL | KeyModifiers::SHIFT,
        );
        assert_eq!(resolver.resolve(&key), Some(Command::OpenProjectSearch));
    }

    #[test]
    fn default_bindings_f2_opens_project_search() {
        let resolver = KeymapResolver::with_defaults();
        let key = KeyEvent::new(KeyCode::F(2), KeyModifiers::NONE);
        assert_eq!(resolver.resolve(&key), Some(Command::OpenProjectSearch));
    }

    #[test]
    fn command_from_str_open_project_search() {
        assert_eq!(
            command_from_str("open_project_search"),
            Some(Command::OpenProjectSearch)
        );
    }

    // --- Diagnostic navigation ---

    #[test]
    fn default_bindings_alt_dot_next_diagnostic() {
        let resolver = KeymapResolver::with_defaults();
        let key = KeyEvent::new(KeyCode::Char('.'), KeyModifiers::ALT);
        assert_eq!(resolver.resolve(&key), Some(Command::GoToNextDiagnostic));
    }

    #[test]
    fn default_bindings_alt_comma_prev_diagnostic() {
        let resolver = KeymapResolver::with_defaults();
        let key = KeyEvent::new(KeyCode::Char(','), KeyModifiers::ALT);
        assert_eq!(resolver.resolve(&key), Some(Command::GoToPrevDiagnostic));
    }

    #[test]
    fn command_from_str_go_to_next_diagnostic() {
        assert_eq!(
            command_from_str("go_to_next_diagnostic"),
            Some(Command::GoToNextDiagnostic)
        );
    }

    // --- Completion ---

    #[test]
    fn alt_slash_triggers_completion() {
        let resolver = KeymapResolver::with_defaults();
        let key = KeyEvent::new(KeyCode::Char('/'), KeyModifiers::ALT);
        assert_eq!(resolver.resolve(&key), Some(Command::TriggerCompletion));
    }

    #[test]
    fn f3_triggers_completion() {
        let resolver = KeymapResolver::with_defaults();
        let key = KeyEvent::new(KeyCode::F(3), KeyModifiers::NONE);
        assert_eq!(resolver.resolve(&key), Some(Command::TriggerCompletion));
    }

    #[test]
    fn command_from_str_trigger_completion() {
        assert_eq!(
            command_from_str("trigger_completion"),
            Some(Command::TriggerCompletion)
        );
    }

    #[test]
    fn command_from_str_accept_completion() {
        assert_eq!(
            command_from_str("accept_completion"),
            Some(Command::AcceptCompletion)
        );
    }

    #[test]
    fn command_from_str_dismiss_completion() {
        assert_eq!(
            command_from_str("dismiss_completion"),
            Some(Command::DismissCompletion)
        );
    }

    #[test]
    fn f12_go_to_definition() {
        let resolver = KeymapResolver::with_defaults();
        let key = KeyEvent::new(KeyCode::F(12), KeyModifiers::NONE);
        assert_eq!(resolver.resolve(&key), Some(Command::GoToDefinition));
    }

    #[test]
    fn shift_f12_find_references() {
        let resolver = KeymapResolver::with_defaults();
        let key = KeyEvent::new(KeyCode::F(12), KeyModifiers::SHIFT);
        assert_eq!(resolver.resolve(&key), Some(Command::FindReferences));
    }

    #[test]
    fn command_from_str_go_to_definition() {
        assert_eq!(
            command_from_str("go_to_definition"),
            Some(Command::GoToDefinition)
        );
    }

    #[test]
    fn command_from_str_find_references() {
        assert_eq!(
            command_from_str("find_references"),
            Some(Command::FindReferences)
        );
    }

    #[test]
    fn ctrl_shift_k_show_hover() {
        let resolver = KeymapResolver::with_defaults();
        let key = KeyEvent::new(
            KeyCode::Char('K'),
            KeyModifiers::CONTROL | KeyModifiers::SHIFT,
        );
        assert_eq!(resolver.resolve(&key), Some(Command::ShowHover));
    }

    #[test]
    fn ctrl_shift_lowercase_k_show_hover() {
        let resolver = KeymapResolver::with_defaults();
        let key = KeyEvent::new(
            KeyCode::Char('k'),
            KeyModifiers::CONTROL | KeyModifiers::SHIFT,
        );
        assert_eq!(resolver.resolve(&key), Some(Command::ShowHover));
    }

    #[test]
    fn f4_show_hover() {
        let resolver = KeymapResolver::with_defaults();
        let key = KeyEvent::new(KeyCode::F(4), KeyModifiers::NONE);
        assert_eq!(resolver.resolve(&key), Some(Command::ShowHover));
    }

    #[test]
    fn command_from_str_show_hover() {
        assert_eq!(command_from_str("show_hover"), Some(Command::ShowHover));
    }

    #[test]
    fn command_from_str_go_to_prev_diagnostic() {
        assert_eq!(
            command_from_str("go_to_prev_diagnostic"),
            Some(Command::GoToPrevDiagnostic)
        );
    }

    // --- format_key_combo ---

    #[test]
    fn format_key_combo_ctrl_q() {
        let s = format_key_combo(KeyModifiers::CONTROL, KeyCode::Char('q'));
        assert_eq!(s, "Ctrl+Q");
    }

    #[test]
    fn format_key_combo_ctrl_shift_z() {
        let s = format_key_combo(
            KeyModifiers::CONTROL | KeyModifiers::SHIFT,
            KeyCode::Char('Z'),
        );
        assert_eq!(s, "Ctrl+Shift+Z");
    }

    #[test]
    fn format_key_combo_alt_bracket() {
        let s = format_key_combo(KeyModifiers::ALT, KeyCode::Char(']'));
        assert_eq!(s, "Alt+]");
    }

    #[test]
    fn format_key_combo_esc() {
        let s = format_key_combo(KeyModifiers::NONE, KeyCode::Esc);
        assert_eq!(s, "Esc");
    }

    #[test]
    fn format_key_combo_f12() {
        let s = format_key_combo(KeyModifiers::NONE, KeyCode::F(12));
        assert_eq!(s, "F12");
    }

    #[test]
    fn format_key_combo_shift_backtab() {
        let s = format_key_combo(KeyModifiers::SHIFT, KeyCode::BackTab);
        assert_eq!(s, "Shift+BackTab");
    }

    #[test]
    fn format_key_combo_space() {
        let s = format_key_combo(KeyModifiers::NONE, KeyCode::Char(' '));
        assert_eq!(s, "Space");
    }

    #[test]
    fn format_key_combo_shift_page_up() {
        let s = format_key_combo(KeyModifiers::SHIFT, KeyCode::PageUp);
        assert_eq!(s, "Shift+PageUp");
    }

    // --- binding_for ---

    #[test]
    fn binding_for_request_quit_returns_ctrl_q() {
        let resolver = KeymapResolver::with_defaults();
        assert_eq!(
            resolver.binding_for(&Command::RequestQuit),
            Some("Ctrl+Q".to_string())
        );
    }

    #[test]
    fn binding_for_unbound_command_returns_none() {
        let resolver = KeymapResolver::with_defaults();
        // Quit (not RequestQuit) has no direct binding.
        assert_eq!(resolver.binding_for(&Command::Quit), None);
    }

    #[test]
    fn ctrl_shift_i_format_document() {
        let resolver = KeymapResolver::with_defaults();
        let key = KeyEvent::new(
            KeyCode::Char('I'),
            KeyModifiers::CONTROL | KeyModifiers::SHIFT,
        );
        assert_eq!(resolver.resolve(&key), Some(Command::FormatDocument));
    }

    #[test]
    fn ctrl_shift_lowercase_i_format_document() {
        let resolver = KeymapResolver::with_defaults();
        let key = KeyEvent::new(
            KeyCode::Char('i'),
            KeyModifiers::CONTROL | KeyModifiers::SHIFT,
        );
        assert_eq!(resolver.resolve(&key), Some(Command::FormatDocument));
    }

    #[test]
    fn command_from_str_format_document() {
        assert_eq!(
            command_from_str("format_document"),
            Some(Command::FormatDocument)
        );
    }

    #[test]
    fn command_from_str_go_to_line() {
        assert_eq!(command_from_str("go_to_line"), Some(Command::GoToLine));
    }

    #[test]
    fn format_key_combo_roundtrips_with_parse() {
        // Test that format -> parse roundtrip works for common combos.
        let combos = vec![
            (KeyModifiers::CONTROL, KeyCode::Char('s')),
            (KeyModifiers::ALT, KeyCode::Char('t')),
            (KeyModifiers::NONE, KeyCode::Esc),
            (KeyModifiers::NONE, KeyCode::F(5)),
        ];
        for (mods, code) in combos {
            let formatted = format_key_combo(mods, code);
            let parsed = parse_key_combo(&formatted.to_lowercase());
            assert!(parsed.is_some(), "Failed to roundtrip: {formatted}");
            let (pm, pc) = parsed.unwrap();
            assert_eq!(pm, mods, "Modifiers mismatch for {formatted}");
            assert_eq!(pc, code, "KeyCode mismatch for {formatted}");
        }
    }
}
