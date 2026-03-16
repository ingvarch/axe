use std::collections::HashMap;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::command::Command;

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
    fn default_bindings_alt_5_activates_terminal_tab_4() {
        let resolver = KeymapResolver::with_defaults();
        let key = KeyEvent::new(KeyCode::Char('5'), KeyModifiers::ALT);
        assert_eq!(
            resolver.resolve(&key),
            Some(Command::ActivateTerminalTab(4))
        );
    }
}
