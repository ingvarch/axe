use std::path::PathBuf;
use std::time::{Duration, Instant};

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};

use alacritty_terminal::index::{Column, Line};

use axe_editor::diagnostic::{BufferDiagnostic, DiagnosticSeverity};
use axe_tree::NodeKind;

use super::layout::{MAX_PANEL_PCT, MIN_PANEL_PCT};
use super::*;
use crate::command::Command;

// --- AppState basic tests ---

#[test]
fn app_state_starts_not_quit() {
    let app = AppState::new();
    assert!(!app.should_quit);
}

#[test]
fn app_state_quit_sets_flag() {
    let mut app = AppState::new();
    app.quit();
    assert!(app.should_quit);
}

#[test]
fn app_state_defaults_show_tree_true() {
    let app = AppState::new();
    assert!(app.show_tree);
}

#[test]
fn app_state_defaults_show_terminal_true() {
    let app = AppState::new();
    assert!(app.show_terminal);
}

#[test]
fn app_state_defaults_show_help_false() {
    let app = AppState::new();
    assert!(!app.show_help);
}

// --- FocusTarget tests ---

#[test]
fn focus_target_default_is_tree() {
    assert_eq!(FocusTarget::default(), FocusTarget::Tree);
}

#[test]
fn focus_target_next_cycles() {
    assert_eq!(FocusTarget::Tree.next(), FocusTarget::Editor);
    assert_eq!(FocusTarget::Editor.next(), FocusTarget::Terminal(0));
    assert_eq!(FocusTarget::Terminal(0).next(), FocusTarget::Tree);
}

#[test]
fn focus_target_prev_cycles() {
    assert_eq!(FocusTarget::Tree.prev(), FocusTarget::Terminal(0));
    assert_eq!(FocusTarget::Editor.prev(), FocusTarget::Tree);
    assert_eq!(FocusTarget::Terminal(0).prev(), FocusTarget::Editor);
}

#[test]
fn focus_target_label() {
    assert_eq!(FocusTarget::Tree.label(), "Files");
    assert_eq!(FocusTarget::Editor.label(), "Editor");
    assert_eq!(FocusTarget::Terminal(0).label(), "Terminal");
}

#[test]
fn app_state_default_focus_is_tree() {
    let app = AppState::new();
    assert_eq!(app.focus, FocusTarget::Tree);
}

// --- ConfirmDialog / ConfirmButton tests ---

#[test]
fn confirm_button_default_is_no() {
    assert_eq!(ConfirmButton::default(), ConfirmButton::No);
}

#[test]
fn confirm_dialog_quit_has_correct_fields() {
    let d = ConfirmDialog::quit();
    assert_eq!(d.title, "Quit");
    assert_eq!(d.message, vec!["Are you sure?"]);
    assert_eq!(d.selected, ConfirmButton::No);
    assert_eq!(d.on_confirm, Command::Quit);
    assert!(d.on_cancel.is_none());
}

#[test]
fn confirm_dialog_close_buffer_has_correct_fields() {
    let d = ConfirmDialog::close_buffer("main.rs");
    assert_eq!(d.title, "Close Buffer");
    assert_eq!(d.message[0], "main.rs");
    assert_eq!(d.message[2], "Unsaved changes will be lost.");
    assert_eq!(d.on_confirm, Command::ConfirmCloseBuffer);
    assert_eq!(d.on_cancel, Some(Command::CancelCloseBuffer));
}

#[test]
fn confirm_dialog_close_terminal_has_correct_fields() {
    let d = ConfirmDialog::close_terminal("bash");
    assert_eq!(d.title, "Close Terminal");
    assert_eq!(d.message[0], "bash");
    assert_eq!(d.message[2], "Process is still running.");
    assert_eq!(d.on_confirm, Command::ForceCloseTerminalTab);
    assert_eq!(d.on_cancel, Some(Command::CancelCloseTerminalTab));
}

#[test]
fn confirm_dialog_delete_tree_node_has_correct_fields() {
    let d = ConfirmDialog::delete_tree_node("file.txt");
    assert_eq!(d.title, "Delete");
    assert_eq!(d.message[0], "file.txt");
    assert_eq!(d.message[2], "This cannot be undone.");
    assert_eq!(d.on_confirm, Command::ConfirmTreeDelete);
    assert_eq!(d.on_cancel, Some(Command::CancelTreeDelete));
}

// --- Execute command tests ---

#[test]
fn execute_quit_sets_should_quit() {
    let mut app = AppState::new();
    app.execute(Command::Quit);
    assert!(app.should_quit);
}

#[test]
fn execute_focus_next_cycles_focus() {
    let mut app = AppState::new();
    assert_eq!(app.focus, FocusTarget::Tree);
    app.execute(Command::FocusNext);
    assert_eq!(app.focus, FocusTarget::Editor);
    app.execute(Command::FocusNext);
    assert_eq!(app.focus, FocusTarget::Terminal(0));
    app.execute(Command::FocusNext);
    assert_eq!(app.focus, FocusTarget::Tree);
}

#[test]
fn execute_focus_prev_cycles_focus() {
    let mut app = AppState::new();
    assert_eq!(app.focus, FocusTarget::Tree);
    app.execute(Command::FocusPrev);
    assert_eq!(app.focus, FocusTarget::Terminal(0));
    app.execute(Command::FocusPrev);
    assert_eq!(app.focus, FocusTarget::Editor);
    app.execute(Command::FocusPrev);
    assert_eq!(app.focus, FocusTarget::Tree);
}

#[test]
fn execute_focus_tree_sets_focus() {
    let mut app = AppState::new();
    app.execute(Command::FocusTree);
    assert_eq!(app.focus, FocusTarget::Tree);
}

#[test]
fn execute_focus_editor_sets_focus() {
    let mut app = AppState::new();
    app.focus = FocusTarget::Tree;
    app.execute(Command::FocusEditor);
    assert_eq!(app.focus, FocusTarget::Editor);
}

#[test]
fn execute_focus_terminal_sets_focus() {
    let mut app = AppState::new();
    app.execute(Command::FocusTerminal);
    assert_eq!(app.focus, FocusTarget::Terminal(0));
}

#[test]
fn execute_toggle_tree_hides_and_shows() {
    let mut app = AppState::new();
    assert!(app.show_tree);
    app.execute(Command::ToggleTree);
    assert!(!app.show_tree);
    app.execute(Command::ToggleTree);
    assert!(app.show_tree);
}

#[test]
fn execute_toggle_terminal_hides_and_shows() {
    let mut app = AppState::new();
    assert!(app.show_terminal);
    app.execute(Command::ToggleTerminal);
    assert!(!app.show_terminal);
    app.execute(Command::ToggleTerminal);
    assert!(app.show_terminal);
}

#[test]
fn toggle_tree_when_tree_focused_moves_focus_to_editor() {
    let mut app = AppState::new();
    app.focus = FocusTarget::Tree;
    app.execute(Command::ToggleTree);
    assert!(!app.show_tree);
    assert_eq!(app.focus, FocusTarget::Editor);
}

#[test]
fn toggle_terminal_when_terminal_focused_moves_focus_to_editor() {
    let mut app = AppState::new();
    app.focus = FocusTarget::Terminal(0);
    app.execute(Command::ToggleTerminal);
    assert!(!app.show_terminal);
    assert_eq!(app.focus, FocusTarget::Editor);
}

#[test]
fn execute_show_help_toggles() {
    let mut app = AppState::new();
    assert!(!app.show_help);
    app.execute(Command::ShowHelp);
    assert!(app.show_help);
    app.execute(Command::ShowHelp);
    assert!(!app.show_help);
}

#[test]
fn execute_close_overlay_closes_help() {
    let mut app = AppState::new();
    app.show_help = true;
    app.execute(Command::CloseOverlay);
    assert!(!app.show_help);
}

#[test]
fn close_overlay_noop_when_no_overlay() {
    let mut app = AppState::new();
    app.execute(Command::CloseOverlay);
    assert!(!app.show_help);
}

// --- Key event integration tests ---

#[test]
fn handle_key_ctrl_q_shows_confirm_quit() {
    let mut app = AppState::new();
    app.handle_key_event(KeyEvent::new(KeyCode::Char('q'), KeyModifiers::CONTROL));
    assert!(!app.should_quit, "Ctrl+Q should not quit immediately");
    assert!(
        app.confirm_dialog.is_some(),
        "Ctrl+Q should show quit confirmation"
    );
}

#[test]
fn confirm_dialog_left_selects_yes() {
    let mut app = AppState::new();
    app.confirm_dialog = Some(ConfirmDialog::quit());
    app.handle_key_event(KeyEvent::new(KeyCode::Left, KeyModifiers::NONE));
    assert_eq!(
        app.confirm_dialog.as_ref().unwrap().selected,
        ConfirmButton::Yes
    );
}

#[test]
fn confirm_dialog_right_selects_no() {
    let mut app = AppState::new();
    app.confirm_dialog = Some(ConfirmDialog::quit());
    // First move to Yes, then back to No.
    app.handle_key_event(KeyEvent::new(KeyCode::Left, KeyModifiers::NONE));
    app.handle_key_event(KeyEvent::new(KeyCode::Right, KeyModifiers::NONE));
    assert_eq!(
        app.confirm_dialog.as_ref().unwrap().selected,
        ConfirmButton::No
    );
}

#[test]
fn confirm_dialog_enter_on_yes_dispatches_confirm() {
    let mut app = AppState::new();
    app.confirm_dialog = Some(ConfirmDialog::quit());
    // Select Yes, then press Enter.
    app.handle_key_event(KeyEvent::new(KeyCode::Left, KeyModifiers::NONE));
    app.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    assert!(app.should_quit);
    assert!(app.confirm_dialog.is_none());
}

#[test]
fn confirm_dialog_enter_on_no_dispatches_cancel() {
    let mut app = AppState::new();
    app.confirm_dialog = Some(ConfirmDialog::quit());
    // Default is No, just press Enter.
    app.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    assert!(!app.should_quit);
    assert!(app.confirm_dialog.is_none());
}

#[test]
fn confirm_dialog_esc_dispatches_cancel() {
    let mut app = AppState::new();
    app.confirm_dialog = Some(ConfirmDialog::quit());
    app.handle_key_event(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
    assert!(!app.should_quit);
    assert!(app.confirm_dialog.is_none());
}

#[test]
fn confirm_dialog_other_keys_consumed() {
    let mut app = AppState::new();
    app.confirm_dialog = Some(ConfirmDialog::quit());
    app.handle_key_event(KeyEvent::new(KeyCode::Char('a'), KeyModifiers::NONE));
    // Dialog should still be open -- key consumed without action.
    assert!(app.confirm_dialog.is_some());
    assert!(!app.should_quit);
}

#[test]
fn handle_key_q_does_not_quit() {
    let mut app = AppState::new();
    app.handle_key_event(KeyEvent::new(KeyCode::Char('q'), KeyModifiers::NONE));
    assert!(!app.should_quit);
}

#[test]
fn ctrl_c_not_bound_globally() {
    let mut app = AppState::new();
    app.handle_key_event(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL));
    assert!(!app.should_quit);
    assert!(app.confirm_dialog.is_none());
}

#[test]
fn handle_other_key_does_not_quit() {
    let mut app = AppState::new();
    app.handle_key_event(KeyEvent::new(KeyCode::Char('a'), KeyModifiers::NONE));
    assert!(!app.should_quit);
}

#[test]
fn tab_not_bound_globally() {
    let mut app = AppState::new();
    assert_eq!(app.focus, FocusTarget::Tree);
    app.handle_key_event(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));
    // Tab is no longer a global binding -- focus should not change.
    assert_eq!(app.focus, FocusTarget::Tree);
}

#[test]
fn handle_alt_1_focuses_tree() {
    let mut app = AppState::new();
    app.handle_key_event(KeyEvent::new(KeyCode::Char('1'), KeyModifiers::ALT));
    assert_eq!(app.focus, FocusTarget::Tree);
}

#[test]
fn handle_alt_2_focuses_editor() {
    let mut app = AppState::new();
    app.focus = FocusTarget::Tree;
    app.handle_key_event(KeyEvent::new(KeyCode::Char('2'), KeyModifiers::ALT));
    assert_eq!(app.focus, FocusTarget::Editor);
}

#[test]
fn handle_alt_3_focuses_terminal() {
    let mut app = AppState::new();
    app.handle_key_event(KeyEvent::new(KeyCode::Char('3'), KeyModifiers::ALT));
    assert_eq!(app.focus, FocusTarget::Terminal(0));
}

#[test]
fn tab_from_terminal_forwarded_to_pty() {
    let mut app = AppState::new();
    app.focus = FocusTarget::Terminal(0);
    app.handle_key_event(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));
    // Tab is forwarded to PTY, not used for focus cycling.
    assert_eq!(app.focus, FocusTarget::Terminal(0));
}

#[test]
fn handle_key_ctrl_b_toggles_tree() {
    let mut app = AppState::new();
    assert!(app.show_tree);
    app.handle_key_event(KeyEvent::new(KeyCode::Char('b'), KeyModifiers::CONTROL));
    assert!(!app.show_tree);
    app.handle_key_event(KeyEvent::new(KeyCode::Char('b'), KeyModifiers::CONTROL));
    assert!(app.show_tree);
}

#[test]
fn handle_key_ctrl_t_toggles_terminal() {
    let mut app = AppState::new();
    assert!(app.show_terminal);
    app.handle_key_event(KeyEvent::new(KeyCode::Char('t'), KeyModifiers::CONTROL));
    assert!(!app.show_terminal);
    app.handle_key_event(KeyEvent::new(KeyCode::Char('t'), KeyModifiers::CONTROL));
    assert!(app.show_terminal);
}

#[test]
fn handle_key_f1_toggles_help() {
    let mut app = AppState::new();
    assert!(!app.show_help);
    app.handle_key_event(KeyEvent::new(KeyCode::F(1), KeyModifiers::NONE));
    assert!(app.show_help);
    app.handle_key_event(KeyEvent::new(KeyCode::F(1), KeyModifiers::NONE));
    assert!(!app.show_help);
}

#[test]
fn handle_esc_closes_help() {
    let mut app = AppState::new();
    app.show_help = true;
    app.handle_key_event(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
    assert!(!app.show_help);
}

#[test]
fn help_overlay_blocks_other_commands() {
    let mut app = AppState::new();
    app.show_help = true;
    // Tab should not cycle focus while help is open
    app.handle_key_event(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));
    assert_eq!(app.focus, FocusTarget::Tree);
}

#[test]
fn help_overlay_allows_request_quit() {
    let mut app = AppState::new();
    app.show_help = true;
    app.handle_key_event(KeyEvent::new(KeyCode::Char('q'), KeyModifiers::CONTROL));
    assert!(
        app.confirm_dialog.is_some(),
        "Ctrl+Q should show quit dialog even with help open"
    );
}

// --- Focus cycling with hidden panels ---

#[test]
fn focus_next_skips_hidden_tree() {
    let mut app = AppState::new();
    app.show_tree = false;
    app.focus = FocusTarget::Terminal(0);
    app.execute(Command::FocusNext);
    assert_eq!(app.focus, FocusTarget::Editor);
}

#[test]
fn focus_next_skips_hidden_terminal() {
    let mut app = AppState::new();
    app.show_terminal = false;
    app.focus = FocusTarget::Editor;
    app.execute(Command::FocusNext);
    assert_eq!(app.focus, FocusTarget::Tree);
}

#[test]
fn focus_prev_skips_hidden_tree() {
    let mut app = AppState::new();
    app.show_tree = false;
    app.focus = FocusTarget::Editor;
    app.execute(Command::FocusPrev);
    assert_eq!(app.focus, FocusTarget::Terminal(0));
}

#[test]
fn focus_prev_skips_hidden_terminal() {
    let mut app = AppState::new();
    app.show_terminal = false;
    app.focus = FocusTarget::Tree;
    app.execute(Command::FocusPrev);
    assert_eq!(app.focus, FocusTarget::Editor);
}

// --- Resize mode defaults ---

#[test]
fn resize_mode_inactive_by_default() {
    let app = AppState::new();
    assert!(!app.resize_mode.active);
}

#[test]
fn default_tree_width_pct_is_20() {
    let app = AppState::new();
    assert_eq!(app.tree_width_pct, 20);
}

#[test]
fn default_editor_height_pct_is_70() {
    let app = AppState::new();
    assert_eq!(app.editor_height_pct, 70);
}

// --- Resize command execution ---

#[test]
fn enter_resize_mode_activates() {
    let mut app = AppState::new();
    app.execute(Command::EnterResizeMode);
    assert!(app.resize_mode.active);
}

#[test]
fn exit_resize_mode_deactivates() {
    let mut app = AppState::new();
    app.resize_mode.active = true;
    app.execute(Command::ExitResizeMode);
    assert!(!app.resize_mode.active);
}

#[test]
fn resize_left_decreases_tree_width() {
    let mut app = AppState::new();
    app.focus = FocusTarget::Tree;
    let original = app.tree_width_pct;
    app.execute(Command::ResizeLeft);
    assert_eq!(app.tree_width_pct, original - 2);
}

#[test]
fn resize_right_increases_tree_width() {
    let mut app = AppState::new();
    app.focus = FocusTarget::Tree;
    let original = app.tree_width_pct;
    app.execute(Command::ResizeRight);
    assert_eq!(app.tree_width_pct, original + 2);
}

#[test]
fn resize_up_decreases_editor_height() {
    let mut app = AppState::new();
    app.focus = FocusTarget::Editor;
    let original = app.editor_height_pct;
    app.execute(Command::ResizeUp);
    // Up = border moves up = editor shrinks
    assert_eq!(app.editor_height_pct, original - 2);
}

#[test]
fn resize_down_increases_editor_height() {
    let mut app = AppState::new();
    app.focus = FocusTarget::Editor;
    let original = app.editor_height_pct;
    app.execute(Command::ResizeDown);
    // Down = border moves down = editor grows
    assert_eq!(app.editor_height_pct, original + 2);
}

#[test]
fn resize_clamps_at_minimum() {
    let mut app = AppState::new();
    app.focus = FocusTarget::Tree;
    app.tree_width_pct = 10;
    app.execute(Command::ResizeLeft);
    assert_eq!(app.tree_width_pct, 10);
}

#[test]
fn resize_clamps_at_maximum() {
    let mut app = AppState::new();
    app.focus = FocusTarget::Tree;
    app.tree_width_pct = 90;
    app.execute(Command::ResizeRight);
    assert_eq!(app.tree_width_pct, 90);
}

#[test]
fn equalize_layout_resets_defaults() {
    let mut app = AppState::new();
    app.tree_width_pct = 50;
    app.editor_height_pct = 50;
    app.execute(Command::EqualizeLayout);
    assert_eq!(app.tree_width_pct, 20);
    assert_eq!(app.editor_height_pct, 70);
}

#[test]
fn resize_left_noop_when_editor_focused() {
    let mut app = AppState::new();
    app.focus = FocusTarget::Editor;
    let original = app.tree_width_pct;
    app.execute(Command::ResizeLeft);
    assert_eq!(app.tree_width_pct, original);
}

#[test]
fn resize_up_noop_when_tree_focused() {
    let mut app = AppState::new();
    app.focus = FocusTarget::Tree;
    let original = app.editor_height_pct;
    app.execute(Command::ResizeUp);
    assert_eq!(app.editor_height_pct, original);
}

#[test]
fn resize_up_moves_border_up_when_terminal_focused() {
    let mut app = AppState::new();
    app.focus = FocusTarget::Terminal(0);
    let original = app.editor_height_pct;
    app.execute(Command::ResizeUp);
    // Up = border moves up = editor shrinks, terminal grows
    assert_eq!(app.editor_height_pct, original - 2);
}

#[test]
fn resize_down_moves_border_down_when_terminal_focused() {
    let mut app = AppState::new();
    app.focus = FocusTarget::Terminal(0);
    let original = app.editor_height_pct;
    app.execute(Command::ResizeDown);
    // Down = border moves down = editor grows, terminal shrinks
    assert_eq!(app.editor_height_pct, original + 2);
}

// --- Resize mode key routing ---

#[test]
fn resize_mode_arrow_left_resizes() {
    let mut app = AppState::new();
    app.resize_mode.active = true;
    app.focus = FocusTarget::Tree;
    let original = app.tree_width_pct;
    app.handle_key_event(KeyEvent::new(KeyCode::Left, KeyModifiers::NONE));
    assert_eq!(app.tree_width_pct, original - 2);
}

#[test]
fn resize_mode_arrow_right_resizes() {
    let mut app = AppState::new();
    app.resize_mode.active = true;
    app.focus = FocusTarget::Tree;
    let original = app.tree_width_pct;
    app.handle_key_event(KeyEvent::new(KeyCode::Right, KeyModifiers::NONE));
    assert_eq!(app.tree_width_pct, original + 2);
}

#[test]
fn resize_mode_arrow_up_resizes() {
    let mut app = AppState::new();
    app.resize_mode.active = true;
    app.focus = FocusTarget::Editor;
    let original = app.editor_height_pct;
    app.handle_key_event(KeyEvent::new(KeyCode::Up, KeyModifiers::NONE));
    // Up = border moves up = editor shrinks
    assert_eq!(app.editor_height_pct, original - 2);
}

#[test]
fn resize_mode_arrow_down_resizes() {
    let mut app = AppState::new();
    app.resize_mode.active = true;
    app.focus = FocusTarget::Editor;
    let original = app.editor_height_pct;
    app.handle_key_event(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
    // Down = border moves down = editor grows
    assert_eq!(app.editor_height_pct, original + 2);
}

#[test]
fn resize_mode_esc_exits() {
    let mut app = AppState::new();
    app.resize_mode.active = true;
    app.handle_key_event(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
    assert!(!app.resize_mode.active);
}

#[test]
fn resize_mode_enter_exits() {
    let mut app = AppState::new();
    app.resize_mode.active = true;
    app.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    assert!(!app.resize_mode.active);
}

#[test]
fn resize_mode_equals_equalizes() {
    let mut app = AppState::new();
    app.resize_mode.active = true;
    app.tree_width_pct = 50;
    app.editor_height_pct = 50;
    app.handle_key_event(KeyEvent::new(KeyCode::Char('='), KeyModifiers::NONE));
    assert_eq!(app.tree_width_pct, 20);
    assert_eq!(app.editor_height_pct, 70);
}

#[test]
fn resize_mode_blocks_focus_commands() {
    let mut app = AppState::new();
    app.resize_mode.active = true;
    app.focus = FocusTarget::Editor;
    // Tab should not cycle focus while resize mode is active
    app.handle_key_event(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));
    assert_eq!(app.focus, FocusTarget::Editor);
}

#[test]
fn resize_mode_allows_request_quit() {
    let mut app = AppState::new();
    app.resize_mode.active = true;
    app.handle_key_event(KeyEvent::new(KeyCode::Char('q'), KeyModifiers::CONTROL));
    assert!(
        app.confirm_dialog.is_some(),
        "Ctrl+Q should show quit dialog in resize mode"
    );
}

#[test]
fn handle_ctrl_n_enters_resize_mode() {
    let mut app = AppState::new();
    app.handle_key_event(KeyEvent::new(KeyCode::Char('n'), KeyModifiers::CONTROL));
    assert!(app.resize_mode.active);
}

// --- Mouse drag resize tests ---

fn mouse_event(kind: MouseEventKind, col: u16, row: u16) -> MouseEvent {
    MouseEvent {
        kind,
        column: col,
        row,
        modifiers: KeyModifiers::NONE,
    }
}

#[test]
fn mouse_drag_inactive_by_default() {
    let app = AppState::new();
    assert_eq!(app.mouse_drag.border, None);
}

#[test]
fn mouse_down_near_vertical_border_starts_drag() {
    let mut app = AppState::new();
    // tree_width_pct = 20, screen_width = 100 -> border at col 20
    let evt = mouse_event(MouseEventKind::Down(MouseButton::Left), 20, 5);
    app.handle_mouse_event(evt, 100, 30);
    assert_eq!(app.mouse_drag.border, Some(DragBorder::Vertical));
}

#[test]
fn mouse_down_near_horizontal_border_starts_drag() {
    let mut app = AppState::new();
    // editor_height_pct = 70, main_height = 29 (30-1 status), border_y = 29*70/100 = 20
    // col must be >= tree border (col 20 for 20% of 100)
    let evt = mouse_event(MouseEventKind::Down(MouseButton::Left), 50, 20);
    app.handle_mouse_event(evt, 100, 30);
    assert_eq!(app.mouse_drag.border, Some(DragBorder::Horizontal));
}

#[test]
fn mouse_down_away_from_border_no_drag() {
    let mut app = AppState::new();
    // Click in the middle of editor area, far from any border
    let evt = mouse_event(MouseEventKind::Down(MouseButton::Left), 60, 5);
    app.handle_mouse_event(evt, 100, 30);
    assert_eq!(app.mouse_drag.border, None);
}

#[test]
fn mouse_drag_vertical_updates_tree_width() {
    let mut app = AppState::new();
    // Start drag on vertical border
    app.mouse_drag.border = Some(DragBorder::Vertical);
    // Drag to col 30 of 100 -> 30%
    let evt = mouse_event(MouseEventKind::Drag(MouseButton::Left), 30, 5);
    app.handle_mouse_event(evt, 100, 30);
    assert_eq!(app.tree_width_pct, 30);
}

#[test]
fn mouse_drag_horizontal_updates_editor_height() {
    let mut app = AppState::new();
    app.mouse_drag.border = Some(DragBorder::Horizontal);
    // main_height = 29, drag to row 14 -> 14*100/29 = 48%
    let evt = mouse_event(MouseEventKind::Drag(MouseButton::Left), 50, 14);
    app.handle_mouse_event(evt, 100, 30);
    assert_eq!(app.editor_height_pct, 48);
}

#[test]
fn mouse_up_ends_drag() {
    let mut app = AppState::new();
    app.mouse_drag.border = Some(DragBorder::Vertical);
    let evt = mouse_event(MouseEventKind::Up(MouseButton::Left), 30, 5);
    app.handle_mouse_event(evt, 100, 30);
    assert_eq!(app.mouse_drag.border, None);
}

#[test]
fn mouse_drag_clamps_at_minimum() {
    let mut app = AppState::new();
    app.mouse_drag.border = Some(DragBorder::Vertical);
    // Drag to col 2 of 100 -> 2%, should clamp to 10%
    let evt = mouse_event(MouseEventKind::Drag(MouseButton::Left), 2, 5);
    app.handle_mouse_event(evt, 100, 30);
    assert_eq!(app.tree_width_pct, MIN_PANEL_PCT);
}

#[test]
fn mouse_drag_clamps_at_maximum() {
    let mut app = AppState::new();
    app.mouse_drag.border = Some(DragBorder::Vertical);
    // Drag to col 98 of 100 -> 98%, should clamp to 90%
    let evt = mouse_event(MouseEventKind::Drag(MouseButton::Left), 98, 5);
    app.handle_mouse_event(evt, 100, 30);
    assert_eq!(app.tree_width_pct, MAX_PANEL_PCT);
}

#[test]
fn mouse_drag_vertical_noop_when_tree_hidden() {
    let mut app = AppState::new();
    app.show_tree = false;
    app.mouse_drag.border = Some(DragBorder::Vertical);
    let original = app.tree_width_pct;
    let evt = mouse_event(MouseEventKind::Drag(MouseButton::Left), 30, 5);
    app.handle_mouse_event(evt, 100, 30);
    assert_eq!(app.tree_width_pct, original);
}

#[test]
fn mouse_drag_horizontal_noop_when_terminal_hidden() {
    let mut app = AppState::new();
    app.show_terminal = false;
    app.mouse_drag.border = Some(DragBorder::Horizontal);
    let original = app.editor_height_pct;
    let evt = mouse_event(MouseEventKind::Drag(MouseButton::Left), 50, 14);
    app.handle_mouse_event(evt, 100, 30);
    assert_eq!(app.editor_height_pct, original);
}

#[test]
fn mouse_drag_ignores_right_button() {
    let mut app = AppState::new();
    // Right-click near vertical border should not start drag
    let evt = mouse_event(MouseEventKind::Down(MouseButton::Right), 20, 5);
    app.handle_mouse_event(evt, 100, 30);
    assert_eq!(app.mouse_drag.border, None);
}

// --- Mouse click focus tests ---

#[test]
fn mouse_click_in_tree_focuses_tree() {
    let mut app = AppState::new();
    app.focus = FocusTarget::Editor;
    // tree_width_pct = 20, screen_width = 100 -> tree occupies cols 0..20
    // Click at col 5 (well inside tree area)
    let evt = mouse_event(MouseEventKind::Down(MouseButton::Left), 5, 5);
    app.handle_mouse_event(evt, 100, 30);
    assert_eq!(app.focus, FocusTarget::Tree);
}

#[test]
fn mouse_click_in_editor_focuses_editor() {
    let mut app = AppState::new();
    app.focus = FocusTarget::Tree;
    // Click at col 60, row 5 (well inside editor area, above horizontal border)
    let evt = mouse_event(MouseEventKind::Down(MouseButton::Left), 60, 5);
    app.handle_mouse_event(evt, 100, 30);
    assert_eq!(app.focus, FocusTarget::Editor);
}

#[test]
fn mouse_click_in_terminal_focuses_terminal() {
    let mut app = AppState::new();
    app.focus = FocusTarget::Editor;
    // editor_height_pct = 70, main_height = 29, border_y = 20
    // Click at col 60, row 25 (below the horizontal border)
    let evt = mouse_event(MouseEventKind::Down(MouseButton::Left), 60, 25);
    app.handle_mouse_event(evt, 100, 30);
    assert_eq!(app.focus, FocusTarget::Terminal(0));
}

#[test]
fn mouse_click_in_editor_when_tree_hidden() {
    let mut app = AppState::new();
    app.show_tree = false;
    app.focus = FocusTarget::Terminal(0);
    // Tree hidden, click at col 5 row 5 -> editor (no tree to click)
    let evt = mouse_event(MouseEventKind::Down(MouseButton::Left), 5, 5);
    app.handle_mouse_event(evt, 100, 30);
    assert_eq!(app.focus, FocusTarget::Editor);
}

#[test]
fn mouse_click_in_editor_when_terminal_hidden() {
    let mut app = AppState::new();
    app.show_terminal = false;
    app.focus = FocusTarget::Tree;
    // Terminal hidden, click at col 60 row 25 -> editor (no terminal)
    let evt = mouse_event(MouseEventKind::Down(MouseButton::Left), 60, 25);
    app.handle_mouse_event(evt, 100, 30);
    assert_eq!(app.focus, FocusTarget::Editor);
}

#[test]
fn mouse_click_on_border_does_not_change_focus() {
    let mut app = AppState::new();
    assert_eq!(app.focus, FocusTarget::Tree);
    // Click right on the vertical border -> starts drag, does NOT change focus
    let evt = mouse_event(MouseEventKind::Down(MouseButton::Left), 20, 5);
    app.handle_mouse_event(evt, 100, 30);
    assert_eq!(app.focus, FocusTarget::Tree);
    assert_eq!(app.mouse_drag.border, Some(DragBorder::Vertical));
}

#[test]
fn mouse_click_in_status_bar_does_not_change_focus() {
    let mut app = AppState::new();
    app.focus = FocusTarget::Tree;
    // Status bar is the last row (row 29 for screen_height=30)
    let evt = mouse_event(MouseEventKind::Down(MouseButton::Left), 50, 29);
    app.handle_mouse_event(evt, 100, 30);
    assert_eq!(app.focus, FocusTarget::Tree);
}

// --- Zoom panel tests ---

#[test]
fn zoomed_panel_none_by_default() {
    let app = AppState::new();
    assert_eq!(app.zoomed_panel, None);
}

#[test]
fn zoom_panel_sets_zoomed_to_current_focus() {
    let mut app = AppState::new();
    app.focus = FocusTarget::Editor;
    app.execute(Command::ZoomPanel);
    assert_eq!(app.zoomed_panel, Some(FocusTarget::Editor));
}

#[test]
fn zoom_panel_again_unzooms() {
    let mut app = AppState::new();
    app.focus = FocusTarget::Editor;
    app.execute(Command::ZoomPanel);
    app.execute(Command::ZoomPanel);
    assert_eq!(app.zoomed_panel, None);
}

#[test]
fn zoom_panel_switches_zoom_to_new_focus() {
    let mut app = AppState::new();
    app.focus = FocusTarget::Editor;
    app.execute(Command::ZoomPanel);
    app.focus = FocusTarget::Tree;
    app.execute(Command::ZoomPanel);
    assert_eq!(app.zoomed_panel, Some(FocusTarget::Tree));
}

#[test]
fn zoom_panel_exits_resize_mode() {
    let mut app = AppState::new();
    app.resize_mode.active = true;
    app.execute(Command::ZoomPanel);
    assert!(!app.resize_mode.active);
}

#[test]
fn handle_key_alt_z_toggles_zoom() {
    let mut app = AppState::new();
    app.focus = FocusTarget::Editor;
    app.handle_key_event(KeyEvent::new(KeyCode::Char('z'), KeyModifiers::ALT));
    assert_eq!(app.zoomed_panel, Some(FocusTarget::Editor));
    app.handle_key_event(KeyEvent::new(KeyCode::Char('z'), KeyModifiers::ALT));
    assert_eq!(app.zoomed_panel, None);
}

#[test]
fn mouse_right_click_does_not_change_focus() {
    let mut app = AppState::new();
    app.focus = FocusTarget::Editor;
    // Right-click in tree area should not change focus
    let evt = mouse_event(MouseEventKind::Down(MouseButton::Right), 5, 5);
    app.handle_mouse_event(evt, 100, 30);
    assert_eq!(app.focus, FocusTarget::Editor);
}

// --- FileTree integration tests ---

#[test]
fn new_has_no_file_tree() {
    let app = AppState::new();
    assert!(app.file_tree.is_none());
}

#[test]
fn new_with_root_has_file_tree() {
    let tmp = tempfile::tempdir().expect("failed to create temp dir");
    let app = AppState::new_with_root(tmp.path().to_path_buf());
    assert!(app.file_tree.is_some());
}

#[test]
fn new_with_root_invalid_path_has_no_file_tree() {
    let app = AppState::new_with_root(PathBuf::from("/nonexistent/path/12345"));
    assert!(app.file_tree.is_none());
}

// --- Tree navigation key routing tests ---

fn app_with_tree_focused() -> (AppState, tempfile::TempDir) {
    let tmp = tempfile::tempdir().expect("failed to create temp dir");
    // Create files so the tree has entries to navigate.
    std::fs::write(tmp.path().join("a.txt"), "").unwrap();
    std::fs::write(tmp.path().join("b.txt"), "").unwrap();
    let mut app = AppState::new_with_root(tmp.path().to_path_buf());
    app.focus = FocusTarget::Tree;
    (app, tmp)
}

#[test]
fn tree_down_when_focused() {
    let (mut app, _tmp) = app_with_tree_focused();
    let initial = app.file_tree.as_ref().unwrap().selected();
    app.handle_key_event(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
    assert_ne!(app.file_tree.as_ref().unwrap().selected(), initial);
}

#[test]
fn tree_up_when_focused() {
    let (mut app, _tmp) = app_with_tree_focused();
    // Move down first, then up
    app.handle_key_event(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
    let after_down = app.file_tree.as_ref().unwrap().selected();
    app.handle_key_event(KeyEvent::new(KeyCode::Up, KeyModifiers::NONE));
    assert_ne!(app.file_tree.as_ref().unwrap().selected(), after_down);
}

#[test]
fn arrows_not_intercepted_when_editor_focused() {
    let tmp = tempfile::tempdir().expect("failed to create temp dir");
    let mut app = AppState::new_with_root(tmp.path().to_path_buf());
    app.focus = FocusTarget::Editor;
    let initial = app.file_tree.as_ref().unwrap().selected();
    app.handle_key_event(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
    // Down arrow in editor mode should not affect tree
    assert_eq!(app.file_tree.as_ref().unwrap().selected(), initial);
}

#[test]
fn tree_keys_blocked_when_help_open() {
    let (mut app, _tmp) = app_with_tree_focused();
    app.show_help = true;
    let initial = app.file_tree.as_ref().unwrap().selected();
    app.handle_key_event(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
    assert_eq!(app.file_tree.as_ref().unwrap().selected(), initial);
}

#[test]
fn global_keys_work_when_tree_focused() {
    let (mut app, _tmp) = app_with_tree_focused();
    // Ctrl+Q should show quit confirmation
    app.handle_key_event(KeyEvent::new(KeyCode::Char('q'), KeyModifiers::CONTROL));
    assert!(app.confirm_dialog.is_some());
}

#[test]
fn tab_not_intercepted_when_tree_focused() {
    let (mut app, _tmp) = app_with_tree_focused();
    app.handle_key_event(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));
    // Tab is not a global binding, and not a tree-specific key.
    // It falls through to global keymap which returns None.
    assert_eq!(app.focus, FocusTarget::Tree);
}

// --- Toggle ignored tests ---

#[test]
fn toggle_ignored_toggles_filter() {
    let (mut app, _tmp) = app_with_tree_focused();
    // Default config has show_hidden=false, so show_ignored starts as false.
    assert!(!app.file_tree.as_ref().unwrap().show_ignored());
    app.execute(Command::ToggleIgnored);
    assert!(app.file_tree.as_ref().unwrap().show_ignored());
}

#[test]
fn ctrl_shift_g_toggles_ignored() {
    let (mut app, _tmp) = app_with_tree_focused();
    // Default config has show_hidden=false, so show_ignored starts as false.
    assert!(!app.file_tree.as_ref().unwrap().show_ignored());
    app.handle_key_event(KeyEvent::new(
        KeyCode::Char('G'),
        KeyModifiers::CONTROL | KeyModifiers::SHIFT,
    ));
    assert!(app.file_tree.as_ref().unwrap().show_ignored());
}

#[test]
fn ctrl_g_opens_go_to_line_dialog() {
    let (mut app, _tmp) = app_with_tree_focused();
    // Need an active buffer for GoToLine to work.
    let file = _tmp.path().join("test.txt");
    std::fs::write(&file, "line1\nline2\nline3\n").unwrap();
    app.execute(Command::OpenFile(file));
    assert!(app.go_to_line.is_none());
    app.handle_key_event(KeyEvent::new(KeyCode::Char('g'), KeyModifiers::CONTROL));
    assert!(app.go_to_line.is_some());
    let dialog = app.go_to_line.as_ref().unwrap();
    assert_eq!(dialog.input, "");
}

#[test]
fn go_to_line_no_op_without_buffer() {
    let mut app = AppState::new();
    app.focus = FocusTarget::Editor;
    app.execute(Command::GoToLine);
    assert!(app.go_to_line.is_none());
}

#[test]
fn go_to_line_no_op_when_not_editor_focused() {
    let (mut app, _tmp) = app_with_tree_focused();
    let file = _tmp.path().join("test.txt");
    std::fs::write(&file, "line1\nline2\n").unwrap();
    app.execute(Command::OpenFile(file));
    // Switch focus away from editor.
    app.focus = FocusTarget::Tree;
    app.execute(Command::GoToLine);
    assert!(app.go_to_line.is_none());
}

#[test]
fn go_to_line_esc_closes_dialog() {
    let (mut app, _tmp) = app_with_tree_focused();
    let file = _tmp.path().join("test.txt");
    std::fs::write(&file, "line1\nline2\n").unwrap();
    app.execute(Command::OpenFile(file));
    app.execute(Command::GoToLine);
    assert!(app.go_to_line.is_some());
    app.handle_key_event(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
    assert!(app.go_to_line.is_none());
}

#[test]
fn go_to_line_enter_jumps_to_line() {
    let (mut app, _tmp) = app_with_tree_focused();
    let file = _tmp.path().join("test.txt");
    std::fs::write(&file, "line1\nline2\nline3\nline4\nline5\n").unwrap();
    app.execute(Command::OpenFile(file));
    app.execute(Command::GoToLine);
    // Type "3" then Enter.
    app.handle_key_event(KeyEvent::new(KeyCode::Char('3'), KeyModifiers::NONE));
    app.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    assert!(app.go_to_line.is_none());
    let buf = app.buffer_manager.active_buffer().unwrap();
    assert_eq!(buf.cursor.row, 2); // 0-indexed line 2 = user line 3
    assert_eq!(buf.cursor.col, 0);
}

#[test]
fn go_to_line_rejects_non_digit() {
    let (mut app, _tmp) = app_with_tree_focused();
    let file = _tmp.path().join("test.txt");
    std::fs::write(&file, "line1\nline2\n").unwrap();
    app.execute(Command::OpenFile(file));
    app.execute(Command::GoToLine);
    app.handle_key_event(KeyEvent::new(KeyCode::Char('a'), KeyModifiers::NONE));
    assert_eq!(app.go_to_line.as_ref().unwrap().input, "");
}

#[test]
fn new_has_no_terminal_manager() {
    let app = AppState::new();
    assert!(app.terminal_manager.is_none());
}

#[test]
fn poll_terminal_noop_without_manager() {
    let mut app = AppState::new();
    app.poll_terminal(); // Should not panic.
}

// --- LSP integration tests ---

#[test]
fn poll_lsp_noop_without_manager() {
    let mut app = AppState::new();
    app.poll_lsp(); // Should not panic.
}

#[test]
fn go_to_definition_without_lsp_noop() {
    let mut app = AppState::new();
    app.execute(Command::GoToDefinition); // Should not panic.
}

#[test]
fn find_references_without_lsp_noop() {
    let mut app = AppState::new();
    app.execute(Command::FindReferences); // Should not panic.
}

#[test]
fn go_to_definition_promotes_preview_buffer() {
    let dir = tempfile::tempdir().expect("create temp dir");
    let file = dir.path().join("test.rs");
    std::fs::write(&file, "fn main() {}").expect("write file");
    let mut app = AppState::new();
    app.buffer_manager
        .open_file_as_preview(&file)
        .expect("open preview");
    assert!(app.buffer_manager.active_buffer().unwrap().is_preview);
    // Calling request_definition should promote the preview.
    app.request_definition();
    assert!(
        !app.buffer_manager.active_buffer().unwrap().is_preview,
        "preview buffer should be promoted on GoToDefinition"
    );
}

#[test]
fn find_references_promotes_preview_buffer() {
    let dir = tempfile::tempdir().expect("create temp dir");
    let file = dir.path().join("test.rs");
    std::fs::write(&file, "fn main() {}").expect("write file");
    let mut app = AppState::new();
    app.buffer_manager
        .open_file_as_preview(&file)
        .expect("open preview");
    assert!(app.buffer_manager.active_buffer().unwrap().is_preview);
    app.request_references();
    assert!(
        !app.buffer_manager.active_buffer().unwrap().is_preview,
        "preview buffer should be promoted on FindReferences"
    );
}

#[test]
fn location_list_esc_closes() {
    let mut app = AppState::new();
    app.location_list = Some(crate::location_list::LocationList::new(
        "Test",
        vec![crate::location_list::LocationItem {
            path: std::path::PathBuf::from("/tmp/test.rs"),
            display_path: "test.rs".to_string(),
            line: 0,
            col: 0,
            line_text: String::new(),
        }],
    ));
    assert!(app.location_list.is_some());
    let key = KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE);
    app.handle_key_event(key);
    assert!(app.location_list.is_none());
}

#[test]
fn location_list_up_down_moves() {
    let mut app = AppState::new();
    let items = vec![
        crate::location_list::LocationItem {
            path: std::path::PathBuf::from("/a.rs"),
            display_path: "a.rs".to_string(),
            line: 0,
            col: 0,
            line_text: String::new(),
        },
        crate::location_list::LocationItem {
            path: std::path::PathBuf::from("/b.rs"),
            display_path: "b.rs".to_string(),
            line: 1,
            col: 0,
            line_text: String::new(),
        },
    ];
    app.location_list = Some(crate::location_list::LocationList::new("Test", items));
    assert_eq!(app.location_list.as_ref().unwrap().selected, 0);

    let key_down = KeyEvent::new(KeyCode::Down, KeyModifiers::NONE);
    app.handle_key_event(key_down);
    assert_eq!(app.location_list.as_ref().unwrap().selected, 1);

    let key_up = KeyEvent::new(KeyCode::Up, KeyModifiers::NONE);
    app.handle_key_event(key_up);
    assert_eq!(app.location_list.as_ref().unwrap().selected, 0);
}

#[test]
fn lsp_manager_initialized_on_new_with_root() {
    let dir = tempfile::tempdir().expect("should create temp dir");
    let app = AppState::new_with_root(dir.path().to_path_buf());
    assert!(app.lsp_manager.is_some());
}

// --- Format on save tests ---

#[test]
fn format_document_without_lsp_shows_status() {
    let mut app = AppState::new();
    assert!(app.status_message.is_none());
    app.execute(Command::FormatDocument);
    assert!(
        app.status_message.is_some(),
        "FormatDocument without LSP should show status message"
    );
    let (msg, _) = app.status_message.as_ref().unwrap();
    assert!(
        msg.contains("not available"),
        "Status message should indicate formatting not available, got: {msg}"
    );
}

#[test]
fn editor_save_without_format_on_save_saves_directly() {
    let mut tmp = tempfile::NamedTempFile::new().unwrap();
    std::io::Write::write_all(&mut tmp, b"data").unwrap();
    std::io::Write::flush(&mut tmp).unwrap();

    let mut app = AppState::new();
    app.config.editor.format_on_save = false;
    app.buffer_manager.open_file(tmp.path()).expect("open file");
    app.focus = FocusTarget::Editor;

    app.execute(Command::EditorInsertChar('x'));
    assert!(app.buffer_manager.active_buffer().unwrap().modified);

    app.execute(Command::EditorSave);
    assert!(
        !app.buffer_manager.active_buffer().unwrap().modified,
        "Save without format_on_save should save directly"
    );
    assert!(!app.pending_format_save);
}

#[test]
fn apply_formatting_edits_empty_array_noop() {
    let mut app = AppState::new();
    let dir = tempfile::tempdir().expect("create temp dir");
    let file = dir.path().join("test.rs");
    std::fs::write(&file, "fn main() {}").expect("write file");
    app.buffer_manager.open_file(&file).expect("open");

    let value = serde_json::json!([]);
    app.apply_formatting_edits(&value);
    assert_eq!(
        app.buffer_manager.active_buffer().unwrap().content_string(),
        "fn main() {}"
    );
}

#[test]
fn apply_formatting_edits_single_edit() {
    let mut app = AppState::new();
    let dir = tempfile::tempdir().expect("create temp dir");
    let file = dir.path().join("test.rs");
    std::fs::write(&file, "fn  main() {}").expect("write file");
    app.buffer_manager.open_file(&file).expect("open");

    // Replace double space with single space.
    let value = serde_json::json!([{
        "range": {
            "start": {"line": 0, "character": 2},
            "end": {"line": 0, "character": 4}
        },
        "newText": " "
    }]);
    app.apply_formatting_edits(&value);
    assert_eq!(
        app.buffer_manager.active_buffer().unwrap().content_string(),
        "fn main() {}"
    );
}

#[test]
fn apply_formatting_edits_reverse_order() {
    let mut app = AppState::new();
    let dir = tempfile::tempdir().expect("create temp dir");
    let file = dir.path().join("test.rs");
    std::fs::write(&file, "aa  bb  cc").expect("write file");
    app.buffer_manager.open_file(&file).expect("open");

    // Two edits: replace "  " at positions 2-4 and 6-8 with " ".
    // Should be applied in reverse order to preserve positions.
    let value = serde_json::json!([
        {
            "range": {
                "start": {"line": 0, "character": 2},
                "end": {"line": 0, "character": 4}
            },
            "newText": " "
        },
        {
            "range": {
                "start": {"line": 0, "character": 6},
                "end": {"line": 0, "character": 8}
            },
            "newText": " "
        }
    ]);
    app.apply_formatting_edits(&value);
    assert_eq!(
        app.buffer_manager.active_buffer().unwrap().content_string(),
        "aa bb cc"
    );
}

#[test]
fn pending_format_save_default_false() {
    let app = AppState::new();
    assert!(!app.pending_format_save);
}

// --- Terminal key interception tests ---

#[test]
fn terminal_focused_printable_key_not_handled_as_command() {
    let mut app = AppState::new();
    app.focus = FocusTarget::Terminal(0);
    // Typing 'a' should not trigger quit or any command side effect.
    app.handle_key_event(KeyEvent::new(KeyCode::Char('a'), KeyModifiers::NONE));
    assert!(!app.should_quit);
    // Focus should remain on terminal (not cycled).
    assert_eq!(app.focus, FocusTarget::Terminal(0));
}

#[test]
fn terminal_focused_ctrl_q_shows_confirm() {
    let mut app = AppState::new();
    app.focus = FocusTarget::Terminal(0);
    app.handle_key_event(KeyEvent::new(KeyCode::Char('q'), KeyModifiers::CONTROL));
    assert!(
        app.confirm_dialog.is_some(),
        "Ctrl+Q should show quit dialog from terminal"
    );
    assert!(!app.should_quit);
}

#[test]
fn terminal_focused_tab_forwarded_to_pty() {
    let mut app = AppState::new();
    app.focus = FocusTarget::Terminal(0);
    app.handle_key_event(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));
    // Tab is forwarded to PTY, not used for focus cycling.
    assert_eq!(app.focus, FocusTarget::Terminal(0));
}

#[test]
fn terminal_focused_ctrl_c_forwarded_to_pty() {
    // Ctrl+C is no longer a global binding -- it's forwarded to the PTY
    // so shell processes can be interrupted.
    let mut app = AppState::new();
    app.focus = FocusTarget::Terminal(0);
    app.handle_key_event(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL));
    assert!(!app.should_quit);
    assert!(app.confirm_dialog.is_none());
    assert_eq!(app.focus, FocusTarget::Terminal(0));
}

#[test]
fn terminal_focused_enter_not_handled_as_command() {
    let mut app = AppState::new();
    app.focus = FocusTarget::Terminal(0);
    app.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    assert!(!app.should_quit);
    assert_eq!(app.focus, FocusTarget::Terminal(0));
}

#[test]
fn terminal_focused_arrow_keys_not_handled_as_command() {
    let mut app = AppState::new();
    app.focus = FocusTarget::Terminal(0);
    app.handle_key_event(KeyEvent::new(KeyCode::Up, KeyModifiers::NONE));
    assert_eq!(app.focus, FocusTarget::Terminal(0));
    assert!(!app.should_quit);
}

#[test]
fn terminal_focused_esc_forwarded_to_pty_not_close_overlay() {
    let mut app = AppState::new();
    app.focus = FocusTarget::Terminal(0);
    // Esc should NOT trigger CloseOverlay when terminal is focused without overlay.
    // It should be forwarded to PTY (for shell vi-mode, cancel completion, etc.).
    app.handle_key_event(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
    assert_eq!(app.focus, FocusTarget::Terminal(0));
    assert!(!app.should_quit);
    // show_help was already false, stays false -- verifying no side effect.
    assert!(!app.show_help);
}

// --- Mouse scroll tests ---

#[test]
fn mouse_scroll_up_over_terminal_does_not_panic() {
    let mut app = AppState::new();
    app.show_terminal = true;
    // Scroll over the terminal area (bottom-right area with default layout).
    let evt = mouse_event(MouseEventKind::ScrollUp, 60, 25);
    app.handle_mouse_event(evt, 100, 30);
    // No terminal_manager -- should be a no-op, no panic.
}

#[test]
fn mouse_scroll_down_over_terminal_does_not_panic() {
    let mut app = AppState::new();
    app.show_terminal = true;
    let evt = mouse_event(MouseEventKind::ScrollDown, 60, 25);
    app.handle_mouse_event(evt, 100, 30);
}

#[test]
fn mouse_scroll_over_editor_does_not_scroll_terminal() {
    let mut app = AppState::new();
    app.show_terminal = true;
    // Scroll over the editor area (top-right with default layout).
    let evt = mouse_event(MouseEventKind::ScrollUp, 60, 5);
    app.handle_mouse_event(evt, 100, 30);
    // Should not panic, terminal not scrolled.
}

#[test]
fn mouse_scroll_ignored_when_terminal_hidden() {
    let mut app = AppState::new();
    app.show_terminal = false;
    let evt = mouse_event(MouseEventKind::ScrollUp, 60, 25);
    app.handle_mouse_event(evt, 100, 30);
    // No-op, no panic.
}

// --- Terminal selection tests ---

#[test]
fn terminal_grid_area_initially_none() {
    let app = AppState::new();
    assert_eq!(app.terminal_grid_area, None);
}

#[test]
fn screen_to_terminal_point_none_without_grid_area() {
    let app = AppState::new();
    assert!(app.screen_to_terminal_point(10, 10).is_none());
}

#[test]
fn screen_to_terminal_point_converts_correctly() {
    let mut app = AppState::new();
    app.terminal_grid_area = Some((20, 15, 60, 10)); // grid starts at (20,15), 60x10

    // Point inside the grid.
    let point = app.screen_to_terminal_point(25, 17);
    assert!(point.is_some());
    let p = point.unwrap();
    assert_eq!(p.column, Column(5)); // 25 - 20
    assert_eq!(p.line, Line(2)); // 17 - 15, no display_offset
}

#[test]
fn screen_to_terminal_point_none_outside_grid() {
    let mut app = AppState::new();
    app.terminal_grid_area = Some((20, 15, 60, 10));

    // Left of grid.
    assert!(app.screen_to_terminal_point(19, 17).is_none());
    // Above grid.
    assert!(app.screen_to_terminal_point(25, 14).is_none());
    // Right of grid.
    assert!(app.screen_to_terminal_point(80, 17).is_none());
    // Below grid.
    assert!(app.screen_to_terminal_point(25, 25).is_none());
}

#[test]
fn terminal_selecting_default_false() {
    let app = AppState::new();
    assert!(!app.terminal_selecting);
    assert!(app.terminal_select_start.is_none());
}

#[test]
fn mouse_down_in_terminal_grid_starts_selection() {
    let mut app = AppState::new();
    app.show_terminal = true;
    app.terminal_grid_area = Some((20, 15, 60, 10));

    // Set up terminal manager with a tab.
    let mut mgr = axe_terminal::TerminalManager::new();
    mgr.spawn_default_tab(60, 10, &std::env::current_dir().unwrap())
        .unwrap();
    app.terminal_manager = Some(mgr);

    // Click inside terminal grid.
    let evt = mouse_event(MouseEventKind::Down(MouseButton::Left), 25, 17);
    app.handle_mouse_event(evt, 100, 30);

    assert!(app.terminal_selecting, "Selection drag should be active");
    assert_eq!(app.terminal_select_start, Some((25, 17)));
    assert!(
        app.terminal_manager
            .as_ref()
            .unwrap()
            .active_tab()
            .unwrap()
            .has_selection(),
        "Terminal should have an active selection"
    );
}

#[test]
fn mouse_click_without_drag_clears_selection() {
    let mut app = AppState::new();
    app.show_terminal = true;
    app.terminal_grid_area = Some((20, 15, 60, 10));

    let mut mgr = axe_terminal::TerminalManager::new();
    mgr.spawn_default_tab(60, 10, &std::env::current_dir().unwrap())
        .unwrap();
    app.terminal_manager = Some(mgr);

    // Mouse down.
    let down = mouse_event(MouseEventKind::Down(MouseButton::Left), 25, 17);
    app.handle_mouse_event(down, 100, 30);
    assert!(app.terminal_selecting);

    // Mouse up at same position (click, no drag).
    let up = mouse_event(MouseEventKind::Up(MouseButton::Left), 25, 17);
    app.handle_mouse_event(up, 100, 30);

    assert!(!app.terminal_selecting, "Selection drag should end");
    assert!(
        !app.terminal_manager
            .as_ref()
            .unwrap()
            .active_tab()
            .unwrap()
            .has_selection(),
        "Selection should be cleared on click without drag"
    );
}

// --- Tree mouse click tests ---

#[test]
fn screen_to_tree_returns_none_without_area() {
    let app = AppState::new();
    assert!(app.screen_to_tree_node_index(5, 5).is_none());
}

#[test]
fn screen_to_tree_returns_correct_index() {
    let tmp = tempfile::TempDir::new().unwrap();
    std::fs::write(tmp.path().join("a.txt"), "hello").unwrap();
    std::fs::write(tmp.path().join("b.txt"), "world").unwrap();
    let mut app = AppState::new_with_root(tmp.path().to_path_buf());
    // tree: root(0), a.txt(1), b.txt(2) -- 3 nodes
    app.tree_inner_area = Some((0, 0, 20, 10));
    // Click row 1 => node index scroll(0) + 1 = 1
    assert_eq!(app.screen_to_tree_node_index(5, 1), Some(1));
    // Click row 0 => node index 0
    assert_eq!(app.screen_to_tree_node_index(5, 0), Some(0));
}

#[test]
fn screen_to_tree_returns_none_outside_area() {
    let tmp = tempfile::TempDir::new().unwrap();
    std::fs::write(tmp.path().join("a.txt"), "hello").unwrap();
    let mut app = AppState::new_with_root(tmp.path().to_path_buf());
    app.tree_inner_area = Some((5, 5, 20, 10));
    // Click outside -- above the area
    assert!(app.screen_to_tree_node_index(10, 3).is_none());
    // Click outside -- left of the area
    assert!(app.screen_to_tree_node_index(2, 7).is_none());
}

#[test]
fn screen_to_tree_returns_none_outside_right_boundary() {
    let tmp = tempfile::TempDir::new().unwrap();
    std::fs::write(tmp.path().join("a.txt"), "hello").unwrap();
    let mut app = AppState::new_with_root(tmp.path().to_path_buf());
    // Tree area: x=0, y=0, width=20, height=10
    app.tree_inner_area = Some((0, 0, 20, 10));
    // Click at column 25, which is outside the tree width (0-19)
    assert!(
        app.screen_to_tree_node_index(25, 1).is_none(),
        "click right of tree panel should return None"
    );
    // Click at column 20 (exactly at the right boundary, should be rejected)
    assert!(
        app.screen_to_tree_node_index(20, 1).is_none(),
        "click at exact right boundary should return None"
    );
    // Click at column 19 (last valid column) should work
    assert!(
        app.screen_to_tree_node_index(19, 1).is_some(),
        "click at last valid column should return Some"
    );
}

#[test]
fn screen_to_tree_respects_x_offset_and_width() {
    let tmp = tempfile::TempDir::new().unwrap();
    std::fs::write(tmp.path().join("a.txt"), "hello").unwrap();
    let mut app = AppState::new_with_root(tmp.path().to_path_buf());
    // Tree area: x=10, y=5, width=15, height=10
    // Valid columns: 10-24 (inclusive), valid rows: 5-14 (inclusive)
    app.tree_inner_area = Some((10, 5, 15, 10));
    // Right of tree (col 25+)
    assert!(
        app.screen_to_tree_node_index(30, 7).is_none(),
        "click right of offset tree should return None"
    );
    // At right boundary (col 25 = 10 + 15)
    assert!(
        app.screen_to_tree_node_index(25, 7).is_none(),
        "click at right boundary of offset tree should return None"
    );
    // Inside tree (col 15, row 5 -- first valid position)
    assert!(
        app.screen_to_tree_node_index(15, 5).is_some(),
        "click inside offset tree should return Some"
    );
}

#[test]
fn screen_to_tree_respects_scroll() {
    let tmp = tempfile::TempDir::new().unwrap();
    for i in 0..20 {
        std::fs::write(tmp.path().join(format!("file{i:02}.txt")), "x").unwrap();
    }
    let mut app = AppState::new_with_root(tmp.path().to_path_buf());
    // Scroll tree down
    if let Some(ref mut tree) = app.file_tree {
        tree.set_viewport_height(5);
        for _ in 0..10 {
            tree.move_down();
        }
    }
    let scroll = app.file_tree.as_ref().unwrap().scroll();
    app.tree_inner_area = Some((0, 0, 20, 5));
    // Click row 0 => node at scroll + 0
    assert_eq!(app.screen_to_tree_node_index(5, 0), Some(scroll));
}

#[test]
fn screen_to_tree_returns_none_below_last_node() {
    let tmp = tempfile::TempDir::new().unwrap();
    std::fs::write(tmp.path().join("a.txt"), "hello").unwrap();
    let mut app = AppState::new_with_root(tmp.path().to_path_buf());
    // 2 nodes (root + a.txt), but area has 10 rows
    app.tree_inner_area = Some((0, 0, 20, 10));
    // Click row 5 => index 5, but only 2 nodes exist
    assert!(app.screen_to_tree_node_index(5, 5).is_none());
}

#[test]
fn mouse_click_in_tree_selects_node() {
    let tmp = tempfile::TempDir::new().unwrap();
    std::fs::write(tmp.path().join("a.txt"), "hello").unwrap();
    std::fs::write(tmp.path().join("b.txt"), "world").unwrap();
    let mut app = AppState::new_with_root(tmp.path().to_path_buf());
    app.tree_inner_area = Some((0, 0, 20, 10));
    // Click on row 2 => node index 2 (b.txt)
    let evt = mouse_event(MouseEventKind::Down(MouseButton::Left), 5, 2);
    app.handle_mouse_event(evt, 80, 30);
    assert_eq!(app.file_tree.as_ref().unwrap().selected(), 2);
    assert_eq!(app.focus, FocusTarget::Tree);
}

#[test]
fn mouse_single_click_on_file_opens_as_preview() {
    let tmp = tempfile::TempDir::new().unwrap();
    std::fs::write(tmp.path().join("a.txt"), "hello").unwrap();
    let mut app = AppState::new_with_root(tmp.path().to_path_buf());
    app.tree_inner_area = Some((0, 0, 20, 10));
    // Single click on row 1 => a.txt (file node)
    let evt = mouse_event(MouseEventKind::Down(MouseButton::Left), 5, 1);
    app.handle_mouse_event(evt, 80, 30);
    assert_eq!(app.buffer_manager.buffer_count(), 1);
    assert!(
        app.buffer_manager.active_buffer().unwrap().is_preview,
        "single click should open as preview"
    );
}

#[test]
fn mouse_double_click_on_file_opens_permanently() {
    let tmp = tempfile::TempDir::new().unwrap();
    std::fs::write(tmp.path().join("a.txt"), "hello").unwrap();
    let mut app = AppState::new_with_root(tmp.path().to_path_buf());
    app.tree_inner_area = Some((0, 0, 20, 10));
    // First click
    let evt = mouse_event(MouseEventKind::Down(MouseButton::Left), 5, 1);
    app.handle_mouse_event(evt, 80, 30);
    // Second click (double-click)
    let evt = mouse_event(MouseEventKind::Down(MouseButton::Left), 5, 1);
    app.handle_mouse_event(evt, 80, 30);
    assert_eq!(app.buffer_manager.buffer_count(), 1);
    assert!(
        !app.buffer_manager.active_buffer().unwrap().is_preview,
        "double click should promote to permanent"
    );
}

#[test]
fn single_click_preview_replaced_by_next_preview() {
    let tmp = tempfile::TempDir::new().unwrap();
    std::fs::write(tmp.path().join("a.txt"), "hello").unwrap();
    std::fs::write(tmp.path().join("b.txt"), "world").unwrap();
    let mut app = AppState::new_with_root(tmp.path().to_path_buf());
    app.tree_inner_area = Some((0, 0, 20, 10));
    // Click a.txt (row 1)
    let evt = mouse_event(MouseEventKind::Down(MouseButton::Left), 5, 1);
    app.handle_mouse_event(evt, 80, 30);
    assert_eq!(app.buffer_manager.buffer_count(), 1);
    // Click b.txt (row 2)
    // Need to reset last_tree_click to avoid double-click detection
    app.last_tree_click = None;
    let evt = mouse_event(MouseEventKind::Down(MouseButton::Left), 5, 2);
    app.handle_mouse_event(evt, 80, 30);
    assert_eq!(
        app.buffer_manager.buffer_count(),
        1,
        "preview should be replaced, not added"
    );
}

#[test]
fn mouse_click_on_directory_toggles_expand() {
    let tmp = tempfile::TempDir::new().unwrap();
    std::fs::create_dir(tmp.path().join("subdir")).unwrap();
    std::fs::write(tmp.path().join("subdir").join("f.txt"), "x").unwrap();
    let mut app = AppState::new_with_root(tmp.path().to_path_buf());
    app.tree_inner_area = Some((0, 0, 20, 10));
    // Node 0 is root (expanded), node 1 is subdir (collapsed by default)
    let was_expanded = app.file_tree.as_ref().unwrap().visible_nodes()[1].expanded;
    assert!(!was_expanded, "subdir should start collapsed");
    // Click on row 1 => subdir
    let evt = mouse_event(MouseEventKind::Down(MouseButton::Left), 5, 1);
    app.handle_mouse_event(evt, 80, 30);
    let is_expanded = app.file_tree.as_ref().unwrap().visible_nodes()[1].expanded;
    assert!(is_expanded, "subdir should be expanded after click");
}

#[test]
fn mouse_click_outside_tree_nodes_no_change() {
    let tmp = tempfile::TempDir::new().unwrap();
    std::fs::write(tmp.path().join("a.txt"), "hello").unwrap();
    let mut app = AppState::new_with_root(tmp.path().to_path_buf());
    app.tree_inner_area = Some((0, 0, 20, 10));
    let before = app.file_tree.as_ref().unwrap().selected();
    // Click on row 5, but only 2 nodes exist
    let evt = mouse_event(MouseEventKind::Down(MouseButton::Left), 5, 5);
    app.handle_mouse_event(evt, 80, 30);
    assert_eq!(app.file_tree.as_ref().unwrap().selected(), before);
}

// --- BufferManager integration tests ---

#[test]
fn new_app_has_empty_buffer_manager() {
    let app = AppState::new();
    assert_eq!(app.buffer_manager.buffer_count(), 0);
}

#[test]
fn execute_open_file_adds_buffer() {
    let mut tmp = tempfile::NamedTempFile::new().unwrap();
    std::io::Write::write_all(&mut tmp, b"hello\n").unwrap();
    std::io::Write::flush(&mut tmp).unwrap();

    let mut app = AppState::new();
    app.execute(Command::OpenFile(tmp.path().to_path_buf()));

    assert!(app.buffer_manager.active_buffer().is_some());
    assert_eq!(app.buffer_manager.buffer_count(), 1);
}

#[test]
fn execute_open_file_switches_focus() {
    let mut tmp = tempfile::NamedTempFile::new().unwrap();
    std::io::Write::write_all(&mut tmp, b"hello\n").unwrap();
    std::io::Write::flush(&mut tmp).unwrap();

    let mut app = AppState::new();
    assert_eq!(app.focus, FocusTarget::Tree);
    app.execute(Command::OpenFile(tmp.path().to_path_buf()));
    assert_eq!(app.focus, FocusTarget::Editor);
}

#[test]
fn tree_toggle_on_file_opens_it() {
    let tmp = tempfile::TempDir::new().unwrap();
    std::fs::write(tmp.path().join("test.rs"), "fn main() {}").unwrap();

    let mut app = AppState::new_with_root(tmp.path().to_path_buf());
    assert!(app.file_tree.is_some());

    // Move down to the file (first child after root dir).
    app.execute(Command::TreeDown);

    // Verify we selected the file.
    let node = app.file_tree.as_ref().unwrap().selected_node().unwrap();
    assert!(
        matches!(node.kind, NodeKind::File { .. }),
        "expected file node, got {:?}",
        node.kind
    );

    // TreeToggle on a file should open it.
    app.execute(Command::TreeToggle);
    assert_eq!(app.focus, FocusTarget::Editor);
    assert_eq!(app.buffer_manager.buffer_count(), 1);
}

// --- Editor cursor movement tests ---

fn app_with_editor_buffer(content: &str) -> AppState {
    let mut tmp = tempfile::NamedTempFile::new().unwrap();
    std::io::Write::write_all(&mut tmp, content.as_bytes()).unwrap();
    std::io::Write::flush(&mut tmp).unwrap();

    let mut app = AppState::new();
    app.buffer_manager.open_file(tmp.path()).expect("open file");
    app.focus = FocusTarget::Editor;
    // Leak the tempfile so the path remains valid for the test.
    let _ = tmp.into_temp_path();
    app
}

#[test]
fn editor_up_moves_cursor() {
    let mut app = app_with_editor_buffer("line1\nline2\nline3");
    app.execute(Command::EditorDown);
    assert_eq!(app.buffer_manager.active_buffer().unwrap().cursor.row, 1);
    app.execute(Command::EditorUp);
    assert_eq!(app.buffer_manager.active_buffer().unwrap().cursor.row, 0);
}

#[test]
fn editor_arrow_keys_intercepted_when_editor_focused() {
    let mut app = app_with_editor_buffer("hello\nworld");
    app.handle_key_event(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
    assert_eq!(app.buffer_manager.active_buffer().unwrap().cursor.row, 1);
    app.handle_key_event(KeyEvent::new(KeyCode::Right, KeyModifiers::NONE));
    assert_eq!(app.buffer_manager.active_buffer().unwrap().cursor.col, 1);
}

#[test]
fn editor_home_end_work() {
    let mut app = app_with_editor_buffer("hello world");
    app.execute(Command::EditorEnd);
    assert_eq!(app.buffer_manager.active_buffer().unwrap().cursor.col, 11);
    app.execute(Command::EditorHome);
    assert_eq!(app.buffer_manager.active_buffer().unwrap().cursor.col, 0);
}

#[test]
fn editor_page_down_uses_viewport() {
    let content = (0..50)
        .map(|i| format!("line{i}"))
        .collect::<Vec<_>>()
        .join("\n");
    let mut app = app_with_editor_buffer(&content);
    app.editor_inner_area = Some((0, 0, 80, 10));
    app.execute(Command::EditorPageDown);
    assert_eq!(app.buffer_manager.active_buffer().unwrap().cursor.row, 10);
}

#[test]
fn editor_word_movement_works() {
    let mut app = app_with_editor_buffer("hello world foo");
    app.handle_key_event(KeyEvent::new(KeyCode::Right, KeyModifiers::CONTROL));
    assert_eq!(app.buffer_manager.active_buffer().unwrap().cursor.col, 6);
    app.handle_key_event(KeyEvent::new(KeyCode::Left, KeyModifiers::CONTROL));
    assert_eq!(app.buffer_manager.active_buffer().unwrap().cursor.col, 0);
}

// --- Editor edit command tests ---

#[test]
fn editor_insert_char_modifies_buffer() {
    let mut app = app_with_editor_buffer("hello");
    app.execute(Command::EditorInsertChar('X'));
    let buf = app.buffer_manager.active_buffer().unwrap();
    assert_eq!(buf.line_at(0).unwrap().to_string(), "Xhello");
    assert!(buf.modified);
    assert!(app.last_edit_time.is_some());
}

#[test]
fn editor_backspace_deletes_char() {
    let mut app = app_with_editor_buffer("hello");
    // Move cursor to col 3
    app.buffer_manager.active_buffer_mut().unwrap().cursor.col = 3;
    app.execute(Command::EditorBackspace);
    let buf = app.buffer_manager.active_buffer().unwrap();
    assert_eq!(buf.line_at(0).unwrap().to_string(), "helo");
    assert_eq!(buf.cursor.col, 2);
}

#[test]
fn editor_enter_splits_line() {
    let mut app = app_with_editor_buffer("hello");
    app.buffer_manager.active_buffer_mut().unwrap().cursor.col = 3;
    app.execute(Command::EditorNewline);
    let buf = app.buffer_manager.active_buffer().unwrap();
    assert_eq!(buf.cursor.row, 1);
    assert_eq!(buf.line_at(0).unwrap().to_string(), "hel\n");
}

#[test]
fn editor_save_clears_modified() {
    let mut tmp = tempfile::NamedTempFile::new().unwrap();
    std::io::Write::write_all(&mut tmp, b"data").unwrap();
    std::io::Write::flush(&mut tmp).unwrap();

    let mut app = AppState::new();
    app.buffer_manager.open_file(tmp.path()).expect("open file");
    app.focus = FocusTarget::Editor;

    app.execute(Command::EditorInsertChar('x'));
    assert!(app.buffer_manager.active_buffer().unwrap().modified);
    assert!(app.last_edit_time.is_some());

    app.execute(Command::EditorSave);
    assert!(!app.buffer_manager.active_buffer().unwrap().modified);
    assert!(app.last_edit_time.is_none());
}

#[test]
fn autosave_triggers_after_delay() {
    let mut tmp = tempfile::NamedTempFile::new().unwrap();
    std::io::Write::write_all(&mut tmp, b"data").unwrap();
    std::io::Write::flush(&mut tmp).unwrap();

    let mut app = AppState::new();
    app.config.editor.auto_save = true;
    app.config.editor.auto_save_delay_ms = 2000;
    app.buffer_manager.open_file(tmp.path()).expect("open file");
    app.focus = FocusTarget::Editor;

    app.execute(Command::EditorInsertChar('z'));
    assert!(app.buffer_manager.active_buffer().unwrap().modified);

    // Simulate time passing by backdating last_edit_time.
    app.last_edit_time = Some(Instant::now() - Duration::from_secs(3));
    app.check_autosave();

    assert!(!app.buffer_manager.active_buffer().unwrap().modified);
    assert!(app.last_edit_time.is_none());
}

#[test]
fn printable_chars_intercepted_when_editor_focused() {
    let mut app = app_with_editor_buffer("hello");
    // Type 'a' -- should be intercepted as EditorInsertChar, not fall through.
    app.handle_key_event(KeyEvent::new(KeyCode::Char('a'), KeyModifiers::NONE));
    let buf = app.buffer_manager.active_buffer().unwrap();
    assert_eq!(buf.line_at(0).unwrap().to_string(), "ahello");
}

#[test]
fn editor_undo_reverses_insert() {
    let mut app = app_with_editor_buffer("hello");
    app.execute(Command::EditorInsertChar('X'));
    assert_eq!(
        app.buffer_manager
            .active_buffer()
            .unwrap()
            .line_at(0)
            .unwrap()
            .to_string(),
        "Xhello"
    );
    app.execute(Command::EditorUndo);
    assert_eq!(
        app.buffer_manager
            .active_buffer()
            .unwrap()
            .line_at(0)
            .unwrap()
            .to_string(),
        "hello"
    );
}

#[test]
fn editor_redo_restores_insert() {
    let mut app = app_with_editor_buffer("hello");
    app.execute(Command::EditorInsertChar('X'));
    app.execute(Command::EditorUndo);
    app.execute(Command::EditorRedo);
    assert_eq!(
        app.buffer_manager
            .active_buffer()
            .unwrap()
            .line_at(0)
            .unwrap()
            .to_string(),
        "Xhello"
    );
}

#[test]
fn editor_undo_does_not_set_last_edit_time() {
    let mut app = app_with_editor_buffer("hello");
    app.execute(Command::EditorInsertChar('X'));
    app.last_edit_time = None;
    app.execute(Command::EditorUndo);
    assert!(app.last_edit_time.is_none());
}

// --- Editor mouse selection tests ---

#[test]
fn editor_mouse_click_positions_cursor() {
    let mut app = app_with_editor_buffer("hello\nworld\nfoo");
    // Set editor area at screen position (5, 2) with 40x10
    app.editor_inner_area = Some((5, 2, 40, 10));

    // Click at screen (8, 3) => relative (3, 1) => buffer row=1, col=3
    let mouse = MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: 8,
        row: 3,
        modifiers: KeyModifiers::NONE,
    };
    app.handle_mouse_event(mouse, 80, 24);

    let buf = app.buffer_manager.active_buffer().unwrap();
    assert_eq!(buf.cursor.row, 1);
    assert_eq!(buf.cursor.col, 3);
    assert!(buf.selection.is_none());
    assert_eq!(app.focus, FocusTarget::Editor);
}

#[test]
fn editor_mouse_drag_creates_selection() {
    let mut app = app_with_editor_buffer("hello\nworld\nfoo");
    app.editor_inner_area = Some((5, 2, 40, 10));

    // Mouse down at (5, 2) => buffer (0, 0)
    app.handle_mouse_event(
        MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: 5,
            row: 2,
            modifiers: KeyModifiers::NONE,
        },
        80,
        24,
    );

    // Drag to (10, 2) => buffer (0, 5)
    app.handle_mouse_event(
        MouseEvent {
            kind: MouseEventKind::Drag(MouseButton::Left),
            column: 10,
            row: 2,
            modifiers: KeyModifiers::NONE,
        },
        80,
        24,
    );

    let buf = app.buffer_manager.active_buffer().unwrap();
    assert!(buf.selection.is_some());
    let sel = buf.selection.as_ref().unwrap();
    assert_eq!(sel.anchor_row, 0);
    assert_eq!(sel.anchor_col, 0);
    assert_eq!(buf.cursor.row, 0);
    assert_eq!(buf.cursor.col, 5);
}

#[test]
fn editor_mouse_click_without_drag_clears_selection_on_up() {
    let mut app = app_with_editor_buffer("hello");
    app.editor_inner_area = Some((5, 2, 40, 10));

    // Click down
    app.handle_mouse_event(
        MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: 7,
            row: 2,
            modifiers: KeyModifiers::NONE,
        },
        80,
        24,
    );

    // Release without drag -- selection should be cleared
    app.handle_mouse_event(
        MouseEvent {
            kind: MouseEventKind::Up(MouseButton::Left),
            column: 7,
            row: 2,
            modifiers: KeyModifiers::NONE,
        },
        80,
        24,
    );

    let buf = app.buffer_manager.active_buffer().unwrap();
    assert!(buf.selection.is_none());
}

#[test]
fn editor_mouse_click_clamps_col_to_line_length() {
    let mut app = app_with_editor_buffer("hi");
    app.editor_inner_area = Some((5, 2, 40, 10));

    // Click far past end of "hi" (col 2) => should clamp to col 2
    app.handle_mouse_event(
        MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: 30,
            row: 2,
            modifiers: KeyModifiers::NONE,
        },
        80,
        24,
    );

    let buf = app.buffer_manager.active_buffer().unwrap();
    assert_eq!(buf.cursor.col, 2);
}

// --- Status message tests ---

#[test]
fn copy_sets_status_message() {
    let mut app = app_with_editor_buffer("hello world");
    app.execute(Command::EditorSelectAll);
    app.execute(Command::EditorCopy);
    assert!(app.status_message.is_some());
    let (msg, _) = app.status_message.as_ref().unwrap();
    assert!(
        msg.contains("Copied"),
        "expected 'Copied' in message, got: {msg}"
    );
    assert!(
        msg.contains("1 line(s)"),
        "expected '1 line(s)' in message, got: {msg}"
    );
}

#[test]
fn cut_sets_status_message() {
    let mut app = app_with_editor_buffer("hello\nworld");
    app.execute(Command::EditorSelectAll);
    app.execute(Command::EditorCut);
    assert!(app.status_message.is_some());
    let (msg, _) = app.status_message.as_ref().unwrap();
    assert!(msg.contains("Cut"), "expected 'Cut' in message, got: {msg}");
    assert!(
        msg.contains("2 line(s)"),
        "expected '2 line(s)' in message, got: {msg}"
    );
}

#[test]
fn copy_without_selection_no_status_message() {
    let mut app = app_with_editor_buffer("hello");
    app.execute(Command::EditorCopy);
    assert!(app.status_message.is_none());
}

#[test]
fn status_message_expires() {
    let mut app = AppState::new();
    app.set_status_message("test".to_string());
    assert!(app.status_message.is_some());
    // Simulate time passing by replacing the instant.
    app.status_message = Some(("test".to_string(), Instant::now() - Duration::from_secs(5)));
    app.expire_status_message();
    assert!(app.status_message.is_none());
}

#[test]
fn editor_find_opens_search() {
    let mut app = AppState::new();
    assert!(app.search.is_none());
    app.execute(Command::EditorFind);
    assert!(app.search.is_some());
}

#[test]
fn search_close_clears_search() {
    let mut app = AppState::new();
    app.execute(Command::EditorFind);
    assert!(app.search.is_some());
    app.execute(Command::SearchClose);
    assert!(app.search.is_none());
}

#[test]
fn search_input_updates_query() {
    let mut tmp = tempfile::NamedTempFile::new().unwrap();
    std::io::Write::write_all(&mut tmp, b"hello world\n").unwrap();
    std::io::Write::flush(&mut tmp).unwrap();

    let mut app = AppState::new();
    app.execute(Command::OpenFile(tmp.path().to_path_buf()));
    app.focus = FocusTarget::Editor;
    app.execute(Command::EditorFind);

    // Simulate typing "he" via key events.
    app.handle_key_event(KeyEvent::new(KeyCode::Char('h'), KeyModifiers::NONE));
    app.handle_key_event(KeyEvent::new(KeyCode::Char('e'), KeyModifiers::NONE));

    let search = app.search.as_ref().unwrap();
    assert_eq!(search.query, "he");
    assert_eq!(search.matches.len(), 1);
}

#[test]
fn search_next_match_moves_cursor() {
    let mut tmp = tempfile::NamedTempFile::new().unwrap();
    std::io::Write::write_all(&mut tmp, b"aaa\naaa\n").unwrap();
    std::io::Write::flush(&mut tmp).unwrap();

    let mut app = AppState::new();
    app.execute(Command::OpenFile(tmp.path().to_path_buf()));
    app.focus = FocusTarget::Editor;
    app.execute(Command::EditorFind);

    // Type "aaa" to find matches.
    if let Some(ref mut search) = app.search {
        if let Some(buf) = app.buffer_manager.active_buffer() {
            search.input_char('a', buf);
            search.input_char('a', buf);
            search.input_char('a', buf);
        }
    }

    // Navigate to next match.
    app.execute(Command::SearchNextMatch);
    let buf = app.buffer_manager.active_buffer().unwrap();
    let search = app.search.as_ref().unwrap();
    assert_eq!(search.current, 1);
    assert_eq!(buf.cursor.row, 1);
}

#[test]
fn search_prev_match_moves_cursor_back() {
    let mut tmp = tempfile::NamedTempFile::new().unwrap();
    std::io::Write::write_all(&mut tmp, b"aaa\naaa\n").unwrap();
    std::io::Write::flush(&mut tmp).unwrap();

    let mut app = AppState::new();
    app.execute(Command::OpenFile(tmp.path().to_path_buf()));
    app.focus = FocusTarget::Editor;
    app.execute(Command::EditorFind);

    if let Some(ref mut search) = app.search {
        if let Some(buf) = app.buffer_manager.active_buffer() {
            search.input_char('a', buf);
            search.input_char('a', buf);
            search.input_char('a', buf);
        }
    }

    // prev from 0 wraps to last.
    app.execute(Command::SearchPrevMatch);
    let search = app.search.as_ref().unwrap();
    assert_eq!(search.current, 1);
}

#[test]
fn search_wraps_around() {
    let mut tmp = tempfile::NamedTempFile::new().unwrap();
    std::io::Write::write_all(&mut tmp, b"aa\n").unwrap();
    std::io::Write::flush(&mut tmp).unwrap();

    let mut app = AppState::new();
    app.execute(Command::OpenFile(tmp.path().to_path_buf()));
    app.focus = FocusTarget::Editor;
    app.execute(Command::EditorFind);

    if let Some(ref mut search) = app.search {
        if let Some(buf) = app.buffer_manager.active_buffer() {
            search.input_char('a', buf);
        }
    }

    // 2 matches: a at col 0 and a at col 1
    let count = app.search.as_ref().unwrap().matches.len();
    assert_eq!(count, 2);

    // next twice wraps back to 0.
    app.execute(Command::SearchNextMatch);
    app.execute(Command::SearchNextMatch);
    assert_eq!(app.search.as_ref().unwrap().current, 0);
}

#[test]
fn editor_find_prefills_selection() {
    let mut tmp = tempfile::NamedTempFile::new().unwrap();
    std::io::Write::write_all(&mut tmp, b"hello world hello\n").unwrap();
    std::io::Write::flush(&mut tmp).unwrap();

    let mut app = AppState::new();
    app.execute(Command::OpenFile(tmp.path().to_path_buf()));
    app.focus = FocusTarget::Editor;

    // Select "hello" (first 5 chars) via selection commands.
    for _ in 0..5 {
        app.execute(Command::EditorSelectRight);
    }

    app.execute(Command::EditorFind);
    let search = app.search.as_ref().unwrap();
    assert_eq!(search.query, "hello");
    assert_eq!(search.matches.len(), 2);
}

#[test]
fn search_esc_closes_via_key_event() {
    let mut app = AppState::new();
    app.focus = FocusTarget::Editor;
    app.execute(Command::EditorFind);
    assert!(app.search.is_some());

    app.handle_key_event(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
    assert!(app.search.is_none());
}

// --- Buffer tab management tests ---

fn open_two_temp_files(app: &mut AppState) {
    let mut tmp1 = tempfile::NamedTempFile::new().unwrap();
    std::io::Write::write_all(&mut tmp1, b"file1\n").unwrap();
    std::io::Write::flush(&mut tmp1).unwrap();
    let mut tmp2 = tempfile::NamedTempFile::new().unwrap();
    std::io::Write::write_all(&mut tmp2, b"file2\n").unwrap();
    std::io::Write::flush(&mut tmp2).unwrap();

    app.buffer_manager
        .open_file(tmp1.path())
        .expect("open file1");
    app.buffer_manager
        .open_file(tmp2.path())
        .expect("open file2");

    // Leak so paths remain valid.
    let _ = tmp1.into_temp_path();
    let _ = tmp2.into_temp_path();
}

#[test]
fn next_buffer_switches_active() {
    let mut app = AppState::new();
    open_two_temp_files(&mut app);
    // After opening two files, active is 1 (last opened).
    assert_eq!(app.buffer_manager.active_index(), 1);
    app.execute(Command::NextBuffer);
    assert_eq!(app.buffer_manager.active_index(), 0);
}

#[test]
fn prev_buffer_switches_active() {
    let mut app = AppState::new();
    open_two_temp_files(&mut app);
    app.buffer_manager.set_active(0);
    assert_eq!(app.buffer_manager.active_index(), 0);
    app.execute(Command::PrevBuffer);
    assert_eq!(app.buffer_manager.active_index(), 1);
}

#[test]
fn close_buffer_unmodified_removes() {
    let mut tmp = tempfile::NamedTempFile::new().unwrap();
    std::io::Write::write_all(&mut tmp, b"data\n").unwrap();
    std::io::Write::flush(&mut tmp).unwrap();

    let mut app = AppState::new();
    app.buffer_manager.open_file(tmp.path()).expect("open file");
    assert_eq!(app.buffer_manager.buffer_count(), 1);

    app.execute(Command::CloseBuffer);
    assert_eq!(app.buffer_manager.buffer_count(), 0);
}

#[test]
fn close_buffer_modified_shows_confirmation() {
    let mut tmp = tempfile::NamedTempFile::new().unwrap();
    std::io::Write::write_all(&mut tmp, b"data\n").unwrap();
    std::io::Write::flush(&mut tmp).unwrap();

    let mut app = AppState::new();
    app.buffer_manager.open_file(tmp.path()).expect("open file");
    app.buffer_manager.active_buffer_mut().unwrap().modified = true;

    app.execute(Command::CloseBuffer);
    assert!(app.confirm_dialog.is_some());
    assert_eq!(app.buffer_manager.buffer_count(), 1);
}

#[test]
fn confirm_close_buffer_removes() {
    let mut tmp = tempfile::NamedTempFile::new().unwrap();
    std::io::Write::write_all(&mut tmp, b"data\n").unwrap();
    std::io::Write::flush(&mut tmp).unwrap();

    let mut app = AppState::new();
    app.buffer_manager.open_file(tmp.path()).expect("open file");
    app.buffer_manager.active_buffer_mut().unwrap().modified = true;

    app.execute(Command::CloseBuffer);
    assert!(app.confirm_dialog.is_some());

    // Simulate pressing Left (Yes) + Enter via the dialog.
    app.handle_key_event(KeyEvent::new(KeyCode::Left, KeyModifiers::NONE));
    app.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    assert!(app.confirm_dialog.is_none());
    assert_eq!(app.buffer_manager.buffer_count(), 0);
}

#[test]
fn cancel_close_buffer_keeps_buffer() {
    let mut tmp = tempfile::NamedTempFile::new().unwrap();
    std::io::Write::write_all(&mut tmp, b"data\n").unwrap();
    std::io::Write::flush(&mut tmp).unwrap();

    let mut app = AppState::new();
    app.buffer_manager.open_file(tmp.path()).expect("open file");
    app.buffer_manager.active_buffer_mut().unwrap().modified = true;

    app.execute(Command::CloseBuffer);
    assert!(app.confirm_dialog.is_some());

    // Default is No -- press Enter to cancel.
    app.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    assert!(app.confirm_dialog.is_none());
    assert_eq!(app.buffer_manager.buffer_count(), 1);
}

#[test]
fn activate_buffer_switches() {
    let mut app = AppState::new();
    open_two_temp_files(&mut app);
    assert_eq!(app.buffer_manager.active_index(), 1);

    app.execute(Command::ActivateBuffer(0));
    assert_eq!(app.buffer_manager.active_index(), 0);
}

#[test]
fn switching_buffer_clears_search() {
    let mut app = AppState::new();
    open_two_temp_files(&mut app);
    app.focus = FocusTarget::Editor;

    app.execute(Command::EditorFind);
    assert!(app.search.is_some());

    app.execute(Command::NextBuffer);
    assert!(app.search.is_none());
}

#[test]
fn close_buffer_confirmation_intercepts_keys() {
    let mut tmp = tempfile::NamedTempFile::new().unwrap();
    std::io::Write::write_all(&mut tmp, b"data\n").unwrap();
    std::io::Write::flush(&mut tmp).unwrap();

    let mut app = AppState::new();
    app.buffer_manager.open_file(tmp.path()).expect("open file");
    app.buffer_manager.active_buffer_mut().unwrap().modified = true;

    app.execute(Command::CloseBuffer);
    assert!(app.confirm_dialog.is_some());

    // Pressing Esc should cancel the confirmation and keep the buffer.
    app.handle_key_event(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
    assert!(app.confirm_dialog.is_none());
    assert_eq!(app.buffer_manager.buffer_count(), 1);
}

// --- Editor tab bar mouse click tests ---

#[test]
fn editor_tab_index_at_col_finds_correct_tab() {
    let mut app = AppState::new();
    open_two_temp_files(&mut app);
    // Two buffers are open. Format: "[1:name]" = 1 + 1 + 1 + name.len() + 1
    // First tab starts at col 0.
    let idx0 = app.editor_tab_index_at_col(0);
    assert_eq!(idx0, Some(0), "col 0 should be inside first tab");

    // First tab width = "[1:" + name + "]" = 3 + name.len() + 1, then 1 space separator.
    let name0 = app.buffer_manager.buffers()[0].file_name().unwrap().len();
    let first_tab_width = 3 + name0 as u16 + 1; // "[1:name]"
    let second_tab_start = first_tab_width + 1; // +1 for space between tabs
    let idx1 = app.editor_tab_index_at_col(second_tab_start);
    assert_eq!(
        idx1,
        Some(1),
        "col at second tab start should be second tab"
    );
}

#[test]
fn editor_tab_index_at_col_returns_none_past_tabs() {
    let mut app = AppState::new();
    open_two_temp_files(&mut app);
    // Very large column past all tabs.
    let idx = app.editor_tab_index_at_col(500);
    assert_eq!(idx, None, "col far past all tabs should return None");
}

#[test]
fn mouse_click_on_editor_tab_switches_buffer() {
    let mut app = AppState::new();
    open_two_temp_files(&mut app);
    // After opening two files, active index is 1 (last opened).
    assert_eq!(app.buffer_manager.active_index(), 1);

    // Set up editor tab bar area at screen row 5, starting at column 2.
    app.editor_tab_bar_area = Some((2, 5, 80, 1));

    // Click on first tab (col 2, row 5) => relative col 0 => first tab.
    let mouse = MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: 2,
        row: 5,
        modifiers: KeyModifiers::NONE,
    };
    app.handle_mouse_event(mouse, 100, 30);

    assert_eq!(
        app.buffer_manager.active_index(),
        0,
        "clicking first tab should activate buffer 0"
    );
    assert_eq!(app.focus, FocusTarget::Editor);
}

// --- autosave config tests ---

#[test]
fn autosave_disabled_by_default_config() {
    let app = AppState::new();
    assert!(!app.config.editor.auto_save);
}

#[test]
fn autosave_skipped_when_auto_save_disabled() {
    let mut tmp = tempfile::NamedTempFile::new().unwrap();
    std::io::Write::write_all(&mut tmp, b"data").unwrap();
    std::io::Write::flush(&mut tmp).unwrap();

    let mut app = AppState::new();
    // auto_save is false by default
    app.buffer_manager.open_file(tmp.path()).expect("open file");
    app.focus = FocusTarget::Editor;

    app.execute(Command::EditorInsertChar('z'));
    assert!(app.buffer_manager.active_buffer().unwrap().modified);

    // Backdate to well past the delay.
    app.last_edit_time = Some(Instant::now() - Duration::from_secs(10));
    app.check_autosave();

    // Buffer should still be modified because auto_save is disabled.
    assert!(app.buffer_manager.active_buffer().unwrap().modified);
}

#[test]
fn autosave_triggers_when_auto_save_enabled() {
    let mut tmp = tempfile::NamedTempFile::new().unwrap();
    std::io::Write::write_all(&mut tmp, b"data").unwrap();
    std::io::Write::flush(&mut tmp).unwrap();

    let mut app = AppState::new();
    app.config.editor.auto_save = true;
    app.config.editor.auto_save_delay_ms = 500;
    app.buffer_manager.open_file(tmp.path()).expect("open file");
    app.focus = FocusTarget::Editor;

    app.execute(Command::EditorInsertChar('z'));
    assert!(app.buffer_manager.active_buffer().unwrap().modified);

    // Backdate past the configured delay.
    app.last_edit_time = Some(Instant::now() - Duration::from_secs(2));
    app.check_autosave();

    assert!(!app.buffer_manager.active_buffer().unwrap().modified);
    assert!(app.last_edit_time.is_none());
}

#[test]
fn autosave_uses_configured_delay() {
    let mut tmp = tempfile::NamedTempFile::new().unwrap();
    std::io::Write::write_all(&mut tmp, b"data").unwrap();
    std::io::Write::flush(&mut tmp).unwrap();

    let mut app = AppState::new();
    app.config.editor.auto_save = true;
    app.config.editor.auto_save_delay_ms = 5000; // 5 seconds
    app.buffer_manager.open_file(tmp.path()).expect("open file");
    app.focus = FocusTarget::Editor;

    app.execute(Command::EditorInsertChar('z'));

    // Only 1 second has passed -- should NOT trigger.
    app.last_edit_time = Some(Instant::now() - Duration::from_secs(1));
    app.check_autosave();

    assert!(app.buffer_manager.active_buffer().unwrap().modified);
    assert!(app.last_edit_time.is_some());
}

// --- buffer_manager config wiring test ---

#[test]
fn new_with_root_passes_editor_config_to_buffer_manager() {
    // AppState::new() uses default config; verify buffer_manager has matching defaults.
    let app = AppState::new();
    assert_eq!(app.buffer_manager.tab_size(), app.config.editor.tab_size);
    assert_eq!(
        app.buffer_manager.insert_spaces(),
        app.config.editor.insert_spaces
    );
}

// --- Terminal close confirmation tests ---

/// Helper: create an AppState with a live terminal tab.
fn app_with_terminal_tab() -> AppState {
    let mut app = AppState::new();
    let cwd = std::env::current_dir().unwrap();
    let mut mgr = axe_terminal::TerminalManager::new();
    mgr.spawn_default_tab(80, 24, &cwd).unwrap();
    app.terminal_manager = Some(mgr);
    app.focus = FocusTarget::Terminal(0);
    app
}

#[test]
fn close_terminal_tab_running_shows_confirmation() {
    let mut app = app_with_terminal_tab();
    assert!(
        app.terminal_manager.as_mut().unwrap().active_tab_is_alive(),
        "Tab should be alive"
    );

    app.execute(Command::CloseTerminalTab);
    assert!(
        app.confirm_dialog.is_some(),
        "Should show confirmation for running process"
    );
    assert_eq!(
        app.terminal_manager.as_ref().unwrap().tab_count(),
        1,
        "Tab should still exist"
    );
}

#[test]
fn force_close_terminal_tab_closes() {
    let mut app = app_with_terminal_tab();
    app.confirm_dialog = Some(ConfirmDialog::close_terminal("test"));

    app.execute(Command::ForceCloseTerminalTab);
    assert_eq!(
        app.terminal_manager.as_ref().unwrap().tab_count(),
        0,
        "Tab should be removed"
    );
}

#[test]
fn cancel_close_terminal_tab_keeps_tab() {
    let mut app = app_with_terminal_tab();
    app.confirm_dialog = Some(ConfirmDialog::close_terminal("test"));

    app.execute(Command::CancelCloseTerminalTab);
    assert_eq!(
        app.terminal_manager.as_ref().unwrap().tab_count(),
        1,
        "Tab should still exist"
    );
}

#[test]
fn close_terminal_confirmation_enter_yes_confirms() {
    let mut app = app_with_terminal_tab();
    app.confirm_dialog = Some(ConfirmDialog::close_terminal("test"));

    // Select Yes, then press Enter.
    app.handle_key_event(KeyEvent::new(KeyCode::Left, KeyModifiers::NONE));
    app.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    assert!(
        app.confirm_dialog.is_none(),
        "Dialog should be dismissed after Enter"
    );
    assert_eq!(
        app.terminal_manager.as_ref().unwrap().tab_count(),
        0,
        "Tab should be closed after confirming"
    );
}

#[test]
fn close_terminal_confirmation_esc_cancels() {
    let mut app = app_with_terminal_tab();
    app.confirm_dialog = Some(ConfirmDialog::close_terminal("test"));

    app.handle_key_event(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
    assert!(
        app.confirm_dialog.is_none(),
        "Dialog should be dismissed after Esc"
    );
    assert_eq!(
        app.terminal_manager.as_ref().unwrap().tab_count(),
        1,
        "Tab should still exist after Esc"
    );
}

#[test]
fn close_last_terminal_tab_hides_panel() {
    let mut app = app_with_terminal_tab();
    app.show_terminal = true;
    app.confirm_dialog = Some(ConfirmDialog::close_terminal("test"));

    app.execute(Command::ForceCloseTerminalTab);
    assert_eq!(
        app.terminal_manager.as_ref().unwrap().tab_count(),
        0,
        "Tab should be removed"
    );
    assert!(
        !app.show_terminal,
        "Terminal panel should be hidden when last tab is closed"
    );
    assert_eq!(
        app.focus,
        FocusTarget::Editor,
        "Focus should move to editor when last terminal tab is closed"
    );
}

#[test]
fn close_non_last_terminal_tab_keeps_panel() {
    let mut app = app_with_terminal_tab();
    app.show_terminal = true;

    // Spawn a second terminal tab.
    let cwd = std::env::current_dir().unwrap();
    app.terminal_manager
        .as_mut()
        .unwrap()
        .spawn_default_tab(80, 24, &cwd)
        .unwrap();
    assert_eq!(app.terminal_manager.as_ref().unwrap().tab_count(), 2);

    // Force close without confirmation (skip alive check).
    app.confirm_dialog = Some(ConfirmDialog::close_terminal("test"));
    app.execute(Command::ForceCloseTerminalTab);

    assert_eq!(
        app.terminal_manager.as_ref().unwrap().tab_count(),
        1,
        "One tab should remain"
    );
    assert!(
        app.show_terminal,
        "Terminal panel should remain visible with tabs remaining"
    );
    assert!(
        matches!(app.focus, FocusTarget::Terminal(_)),
        "Focus should stay on terminal"
    );
}

// --- tab_bar_hit with stored area tests ---

#[test]
fn tab_bar_hit_uses_stored_area() {
    let mut app = AppState::new();
    let cwd = std::env::current_dir().unwrap();
    let mut mgr = axe_terminal::TerminalManager::new();
    mgr.spawn_tab(80, 24, &cwd).unwrap();
    mgr.spawn_tab(80, 24, &cwd).unwrap();
    app.terminal_manager = Some(mgr);
    // Simulate stored tab bar area at row 20, starting at x=10, width=60.
    app.terminal_tab_bar_area = Some((10, 20, 60, 1));

    // Click on stored row at x=10 -> should hit tab 0.
    let result = app.tab_bar_hit(10, 20);
    assert!(result.is_some(), "expected hit on stored tab bar row");
    assert_eq!(result.unwrap(), 0);
}

#[test]
fn tab_bar_hit_misses_wrong_row() {
    let mut app = AppState::new();
    let cwd = std::env::current_dir().unwrap();
    let mut mgr = axe_terminal::TerminalManager::new();
    mgr.spawn_tab(80, 24, &cwd).unwrap();
    mgr.spawn_tab(80, 24, &cwd).unwrap();
    app.terminal_manager = Some(mgr);
    app.terminal_tab_bar_area = Some((10, 20, 60, 1));

    // Click above the stored row.
    assert!(app.tab_bar_hit(10, 19).is_none(), "row above should miss");
    // Click below the stored row.
    assert!(app.tab_bar_hit(10, 21).is_none(), "row below should miss");
}

#[test]
fn tab_bar_hit_returns_none_when_area_not_set() {
    let mut app = AppState::new();
    let cwd = std::env::current_dir().unwrap();
    let mut mgr = axe_terminal::TerminalManager::new();
    mgr.spawn_tab(80, 24, &cwd).unwrap();
    app.terminal_manager = Some(mgr);
    app.terminal_tab_bar_area = None;

    assert!(
        app.tab_bar_hit(10, 20).is_none(),
        "should return None when area not set"
    );
}

// --- Unified tab commands ---

#[test]
fn close_tab_closes_buffer_when_editor_focused() {
    let mut app = AppState::new();
    app.focus = FocusTarget::Editor;
    let tmp = tempfile::NamedTempFile::new().unwrap();
    std::fs::write(tmp.path(), "content").unwrap();
    app.buffer_manager.open_file(tmp.path()).unwrap();
    assert_eq!(app.buffer_manager.buffer_count(), 1);

    app.execute(Command::CloseTab);
    assert_eq!(app.buffer_manager.buffer_count(), 0);
}

#[test]
fn close_tab_shows_confirmation_for_live_terminal() {
    let mut app = app_with_terminal_tab();
    app.focus = FocusTarget::Terminal(0);

    app.execute(Command::CloseTab);
    // Live terminal should trigger confirmation dialog.
    assert!(
        app.confirm_dialog.is_some(),
        "CloseTab on live terminal should show confirmation"
    );
}

#[test]
fn next_tab_cycles_editor_buffers_when_editor_focused() {
    let mut app = AppState::new();
    app.focus = FocusTarget::Editor;
    let tmp1 = tempfile::NamedTempFile::new().unwrap();
    let tmp2 = tempfile::NamedTempFile::new().unwrap();
    std::fs::write(tmp1.path(), "a").unwrap();
    std::fs::write(tmp2.path(), "b").unwrap();
    app.buffer_manager.open_file(tmp1.path()).unwrap();
    app.buffer_manager.open_file(tmp2.path()).unwrap();
    assert_eq!(app.buffer_manager.active_index(), 1);

    app.execute(Command::NextTab);
    assert_eq!(app.buffer_manager.active_index(), 0);
}

#[test]
fn prev_tab_cycles_editor_buffers_when_editor_focused() {
    let mut app = AppState::new();
    app.focus = FocusTarget::Editor;
    let tmp1 = tempfile::NamedTempFile::new().unwrap();
    let tmp2 = tempfile::NamedTempFile::new().unwrap();
    std::fs::write(tmp1.path(), "a").unwrap();
    std::fs::write(tmp2.path(), "b").unwrap();
    app.buffer_manager.open_file(tmp1.path()).unwrap();
    app.buffer_manager.open_file(tmp2.path()).unwrap();
    assert_eq!(app.buffer_manager.active_index(), 1);

    app.execute(Command::PrevTab);
    assert_eq!(app.buffer_manager.active_index(), 0);
}

#[test]
fn next_tab_cycles_terminal_tabs_when_terminal_focused() {
    let mut app = AppState::new();
    let cwd = std::env::current_dir().unwrap();
    let mut mgr = axe_terminal::TerminalManager::new();
    mgr.spawn_tab(80, 24, &cwd).unwrap();
    mgr.spawn_tab(80, 24, &cwd).unwrap();
    mgr.activate_tab(0);
    app.terminal_manager = Some(mgr);
    app.focus = FocusTarget::Terminal(0);

    app.execute(Command::NextTab);
    assert_eq!(app.focus, FocusTarget::Terminal(1));
}

#[test]
fn prev_tab_wraps_terminal_tabs_when_terminal_focused() {
    let mut app = AppState::new();
    let cwd = std::env::current_dir().unwrap();
    let mut mgr = axe_terminal::TerminalManager::new();
    mgr.spawn_tab(80, 24, &cwd).unwrap();
    mgr.spawn_tab(80, 24, &cwd).unwrap();
    mgr.activate_tab(0);
    app.terminal_manager = Some(mgr);
    app.focus = FocusTarget::Terminal(0);

    app.execute(Command::PrevTab);
    assert_eq!(app.focus, FocusTarget::Terminal(1));
}

#[test]
fn new_tab_creates_terminal_when_terminal_focused() {
    let mut app = AppState::new();
    let cwd = std::env::current_dir().unwrap();
    let mut mgr = axe_terminal::TerminalManager::new();
    mgr.spawn_tab(80, 24, &cwd).unwrap();
    app.terminal_manager = Some(mgr);
    app.focus = FocusTarget::Terminal(0);

    app.execute(Command::NewTab);
    assert_eq!(app.terminal_manager.as_ref().unwrap().tab_count(), 2);
}

// --- ClickState tests ---

#[test]
fn click_state_first_click_returns_one() {
    let mut state = ClickState::default();
    let now = Instant::now();
    assert_eq!(state.register(now, 5, 10, DOUBLE_CLICK_THRESHOLD), 1);
}

#[test]
fn click_state_increments_same_position() {
    let mut state = ClickState::default();
    let now = Instant::now();
    assert_eq!(state.register(now, 5, 10, DOUBLE_CLICK_THRESHOLD), 1);
    assert_eq!(state.register(now, 5, 10, DOUBLE_CLICK_THRESHOLD), 2);
    assert_eq!(state.register(now, 5, 10, DOUBLE_CLICK_THRESHOLD), 3);
}

#[test]
fn click_state_caps_at_three() {
    let mut state = ClickState::default();
    let now = Instant::now();
    state.register(now, 0, 0, DOUBLE_CLICK_THRESHOLD);
    state.register(now, 0, 0, DOUBLE_CLICK_THRESHOLD);
    state.register(now, 0, 0, DOUBLE_CLICK_THRESHOLD);
    assert_eq!(state.register(now, 0, 0, DOUBLE_CLICK_THRESHOLD), 3);
}

#[test]
fn click_state_resets_on_different_position() {
    let mut state = ClickState::default();
    let now = Instant::now();
    state.register(now, 5, 10, DOUBLE_CLICK_THRESHOLD);
    state.register(now, 5, 10, DOUBLE_CLICK_THRESHOLD);
    assert_eq!(state.click_count, 2);
    // Position (6,10) is within tolerance (abs_diff=1), so it still counts
    assert_eq!(state.register(now, 6, 10, DOUBLE_CLICK_THRESHOLD), 3);
}

#[test]
fn click_state_resets_on_far_position() {
    let mut state = ClickState::default();
    let now = Instant::now();
    state.register(now, 5, 10, DOUBLE_CLICK_THRESHOLD);
    state.register(now, 5, 10, DOUBLE_CLICK_THRESHOLD);
    assert_eq!(state.click_count, 2);
    // Position more than CLICK_POSITION_TOLERANCE away resets
    assert_eq!(state.register(now, 8, 10, DOUBLE_CLICK_THRESHOLD), 1);
}

#[test]
fn click_state_resets_after_threshold() {
    let mut state = ClickState::default();
    let threshold = Duration::from_millis(400);
    let t1 = Instant::now();
    state.register(t1, 5, 10, threshold);
    // Simulate waiting past threshold
    std::thread::sleep(Duration::from_millis(500));
    let t2 = Instant::now();
    assert_eq!(state.register(t2, 5, 10, threshold), 1);
}

#[test]
fn click_state_tolerates_nearby_position() {
    let mut state = ClickState::default();
    let now = Instant::now();
    state.register(now, 5, 10, DOUBLE_CLICK_THRESHOLD);
    // 1 cell away should still count as same position
    assert_eq!(state.register(now, 5, 11, DOUBLE_CLICK_THRESHOLD), 2);
}

// --- Editor multi-click tests ---

#[test]
fn editor_double_click_selects_word() {
    let mut app = app_with_editor_buffer("hello world");
    app.editor_inner_area = Some((0, 0, 80, 24));

    // First click at col 2 (inside "hello")
    let down1 = mouse_event(MouseEventKind::Down(MouseButton::Left), 2, 0);
    app.handle_mouse_event(down1, 100, 30);
    let up1 = mouse_event(MouseEventKind::Up(MouseButton::Left), 2, 0);
    app.handle_mouse_event(up1, 100, 30);

    // Second click at same position (double-click)
    let down2 = mouse_event(MouseEventKind::Down(MouseButton::Left), 2, 0);
    app.handle_mouse_event(down2, 100, 30);

    let buf = app.buffer_manager.active_buffer().unwrap();
    assert_eq!(buf.selected_text(), Some("hello".to_string()));
}

#[test]
fn editor_triple_click_selects_line() {
    let mut app = app_with_editor_buffer("hello world\nsecond line");
    app.editor_inner_area = Some((0, 0, 80, 24));

    // Three rapid clicks
    for _ in 0..3 {
        let down = mouse_event(MouseEventKind::Down(MouseButton::Left), 5, 0);
        app.handle_mouse_event(down, 100, 30);
        let up = mouse_event(MouseEventKind::Up(MouseButton::Left), 5, 0);
        app.handle_mouse_event(up, 100, 30);
    }

    let buf = app.buffer_manager.active_buffer().unwrap();
    assert_eq!(buf.selected_text(), Some("hello world".to_string()));
}

#[test]
fn editor_single_click_still_positions_cursor() {
    let mut app = app_with_editor_buffer("hello world");
    app.editor_inner_area = Some((0, 0, 80, 24));

    let down = mouse_event(MouseEventKind::Down(MouseButton::Left), 3, 0);
    app.handle_mouse_event(down, 100, 30);

    let buf = app.buffer_manager.active_buffer().unwrap();
    assert_eq!(buf.cursor.col, 3);
    assert!(buf.selection.is_none());
}

#[test]
fn editor_double_click_does_not_enable_drag() {
    let mut app = app_with_editor_buffer("hello world");
    app.editor_inner_area = Some((0, 0, 80, 24));

    // Double-click
    let down1 = mouse_event(MouseEventKind::Down(MouseButton::Left), 2, 0);
    app.handle_mouse_event(down1, 100, 30);
    let up1 = mouse_event(MouseEventKind::Up(MouseButton::Left), 2, 0);
    app.handle_mouse_event(up1, 100, 30);
    let down2 = mouse_event(MouseEventKind::Down(MouseButton::Left), 2, 0);
    app.handle_mouse_event(down2, 100, 30);

    assert!(
        !app.editor_selecting,
        "Drag should not be active after double-click"
    );
}

// --- Terminal multi-click tests ---

#[test]
fn terminal_double_click_uses_semantic_selection() {
    let mut app = AppState::new();
    app.show_terminal = true;
    app.terminal_grid_area = Some((20, 15, 60, 10));

    let mut mgr = axe_terminal::TerminalManager::new();
    mgr.spawn_default_tab(60, 10, &std::env::current_dir().unwrap())
        .unwrap();
    app.terminal_manager = Some(mgr);

    // First click
    let down1 = mouse_event(MouseEventKind::Down(MouseButton::Left), 25, 17);
    app.handle_mouse_event(down1, 100, 30);
    let up1 = mouse_event(MouseEventKind::Up(MouseButton::Left), 25, 17);
    app.handle_mouse_event(up1, 100, 30);

    // Second click (double-click)
    let down2 = mouse_event(MouseEventKind::Down(MouseButton::Left), 25, 17);
    app.handle_mouse_event(down2, 100, 30);

    assert!(
        app.terminal_manager
            .as_ref()
            .unwrap()
            .active_tab()
            .unwrap()
            .has_selection(),
        "Terminal should have selection after double-click"
    );
    assert!(
        !app.terminal_selecting,
        "Drag should not be active after double-click"
    );
}

#[test]
fn terminal_triple_click_uses_lines_selection() {
    let mut app = AppState::new();
    app.show_terminal = true;
    app.terminal_grid_area = Some((20, 15, 60, 10));

    let mut mgr = axe_terminal::TerminalManager::new();
    mgr.spawn_default_tab(60, 10, &std::env::current_dir().unwrap())
        .unwrap();
    app.terminal_manager = Some(mgr);

    // Three rapid clicks
    for _ in 0..3 {
        let down = mouse_event(MouseEventKind::Down(MouseButton::Left), 25, 17);
        app.handle_mouse_event(down, 100, 30);
        let up = mouse_event(MouseEventKind::Up(MouseButton::Left), 25, 17);
        app.handle_mouse_event(up, 100, 30);
    }

    assert!(
        app.terminal_manager
            .as_ref()
            .unwrap()
            .active_tab()
            .unwrap()
            .has_selection(),
        "Terminal should have selection after triple-click"
    );
}

#[test]
fn terminal_single_click_enables_drag() {
    let mut app = AppState::new();
    app.show_terminal = true;
    app.terminal_grid_area = Some((20, 15, 60, 10));

    let mut mgr = axe_terminal::TerminalManager::new();
    mgr.spawn_default_tab(60, 10, &std::env::current_dir().unwrap())
        .unwrap();
    app.terminal_manager = Some(mgr);

    let down = mouse_event(MouseEventKind::Down(MouseButton::Left), 25, 17);
    app.handle_mouse_event(down, 100, 30);

    assert!(app.terminal_selecting, "Single click should enable drag");
}

// --- File finder tests ---

#[test]
fn open_file_finder_sets_file_finder_with_root() {
    let tmp = tempfile::TempDir::new().unwrap();
    std::fs::write(tmp.path().join("test.txt"), "").unwrap();
    let mut app = AppState::new();
    app.project_root = Some(tmp.path().to_path_buf());
    assert!(app.file_finder.is_none());
    app.execute(Command::OpenFileFinder);
    assert!(app.file_finder.is_some());
}

#[test]
fn open_file_finder_noop_without_root() {
    let mut app = AppState::new();
    assert!(app.project_root.is_none());
    app.execute(Command::OpenFileFinder);
    assert!(app.file_finder.is_none());
}

#[test]
fn file_finder_esc_closes() {
    let tmp = tempfile::TempDir::new().unwrap();
    std::fs::write(tmp.path().join("test.txt"), "").unwrap();
    let mut app = AppState::new();
    app.project_root = Some(tmp.path().to_path_buf());
    app.execute(Command::OpenFileFinder);
    assert!(app.file_finder.is_some());
    app.handle_key_event(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
    assert!(app.file_finder.is_none());
}

#[test]
fn file_finder_char_input_updates_query() {
    let tmp = tempfile::TempDir::new().unwrap();
    std::fs::write(tmp.path().join("test.txt"), "").unwrap();
    let mut app = AppState::new();
    app.project_root = Some(tmp.path().to_path_buf());
    app.execute(Command::OpenFileFinder);
    app.handle_key_event(KeyEvent::new(KeyCode::Char('t'), KeyModifiers::NONE));
    assert_eq!(app.file_finder.as_ref().unwrap().query, "t");
}

#[test]
fn file_finder_up_down_navigate() {
    let tmp = tempfile::TempDir::new().unwrap();
    std::fs::write(tmp.path().join("a.txt"), "").unwrap();
    std::fs::write(tmp.path().join("b.txt"), "").unwrap();
    let mut app = AppState::new();
    app.project_root = Some(tmp.path().to_path_buf());
    app.execute(Command::OpenFileFinder);
    assert_eq!(app.file_finder.as_ref().unwrap().selected, 0);
    app.handle_key_event(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
    assert_eq!(app.file_finder.as_ref().unwrap().selected, 1);
    app.handle_key_event(KeyEvent::new(KeyCode::Up, KeyModifiers::NONE));
    assert_eq!(app.file_finder.as_ref().unwrap().selected, 0);
}

#[test]
fn file_finder_enter_opens_file_and_closes() {
    let tmp = tempfile::TempDir::new().unwrap();
    let file_path = tmp.path().join("hello.rs");
    std::fs::write(&file_path, "fn main() {}").unwrap();
    let mut app = AppState::new();
    app.project_root = Some(tmp.path().to_path_buf());
    app.execute(Command::OpenFileFinder);
    assert!(app.file_finder.is_some());
    app.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    assert!(app.file_finder.is_none(), "Finder should close after Enter");
    assert!(
        app.buffer_manager.active_buffer().is_some(),
        "File should be opened in editor"
    );
}

#[test]
fn file_finder_backspace_removes_query_char() {
    let tmp = tempfile::TempDir::new().unwrap();
    std::fs::write(tmp.path().join("test.txt"), "").unwrap();
    let mut app = AppState::new();
    app.project_root = Some(tmp.path().to_path_buf());
    app.execute(Command::OpenFileFinder);
    app.handle_key_event(KeyEvent::new(KeyCode::Char('a'), KeyModifiers::NONE));
    app.handle_key_event(KeyEvent::new(KeyCode::Char('b'), KeyModifiers::NONE));
    assert_eq!(app.file_finder.as_ref().unwrap().query, "ab");
    app.handle_key_event(KeyEvent::new(KeyCode::Backspace, KeyModifiers::NONE));
    assert_eq!(app.file_finder.as_ref().unwrap().query, "a");
}

#[test]
fn close_overlay_closes_file_finder_first() {
    let tmp = tempfile::TempDir::new().unwrap();
    std::fs::write(tmp.path().join("test.txt"), "").unwrap();
    let mut app = AppState::new();
    app.project_root = Some(tmp.path().to_path_buf());
    app.show_help = true;
    app.execute(Command::OpenFileFinder);
    assert!(app.file_finder.is_some());
    assert!(app.show_help);
    app.execute(Command::CloseOverlay);
    assert!(
        app.file_finder.is_none(),
        "CloseOverlay should close finder first"
    );
    assert!(app.show_help, "Help should remain open");
    app.execute(Command::CloseOverlay);
    assert!(!app.show_help, "Second CloseOverlay should close help");
}

#[test]
fn file_finder_keys_consumed_no_editor_side_effects() {
    let tmp = tempfile::TempDir::new().unwrap();
    std::fs::write(tmp.path().join("test.txt"), "").unwrap();
    let mut app = AppState::new();
    app.project_root = Some(tmp.path().to_path_buf());
    app.focus = FocusTarget::Editor;
    app.execute(Command::OpenFileFinder);
    // Typing should not insert into editor buffer.
    app.handle_key_event(KeyEvent::new(KeyCode::Char('x'), KeyModifiers::NONE));
    assert_eq!(app.file_finder.as_ref().unwrap().query, "x");
    // No buffer is open, so nothing should have been inserted.
    assert!(app.buffer_manager.active_buffer().is_none());
}

// --- Command palette tests ---

#[test]
fn open_command_palette_sets_field() {
    let mut app = AppState::new();
    assert!(app.command_palette.is_none());
    app.execute(Command::OpenCommandPalette);
    assert!(app.command_palette.is_some());
}

#[test]
fn command_palette_esc_closes() {
    let mut app = AppState::new();
    app.execute(Command::OpenCommandPalette);
    assert!(app.command_palette.is_some());
    app.handle_key_event(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
    assert!(app.command_palette.is_none());
}

#[test]
fn command_palette_char_input_updates_query() {
    let mut app = AppState::new();
    app.execute(Command::OpenCommandPalette);
    app.handle_key_event(KeyEvent::new(KeyCode::Char('s'), KeyModifiers::NONE));
    assert_eq!(app.command_palette.as_ref().unwrap().query, "s");
}

#[test]
fn command_palette_up_down_navigate() {
    let mut app = AppState::new();
    app.execute(Command::OpenCommandPalette);
    assert_eq!(app.command_palette.as_ref().unwrap().selected, 0);
    app.handle_key_event(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
    assert_eq!(app.command_palette.as_ref().unwrap().selected, 1);
    app.handle_key_event(KeyEvent::new(KeyCode::Up, KeyModifiers::NONE));
    assert_eq!(app.command_palette.as_ref().unwrap().selected, 0);
}

#[test]
fn command_palette_enter_executes_and_closes() {
    let mut app = AppState::new();
    app.execute(Command::OpenCommandPalette);
    assert!(app.command_palette.is_some());
    // First item is "Quit" (RequestQuit) -- pressing Enter should trigger it.
    app.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    assert!(
        app.command_palette.is_none(),
        "Palette should close after Enter"
    );
    // RequestQuit opens a confirm dialog.
    assert!(
        app.confirm_dialog.is_some(),
        "Enter on Quit should trigger RequestQuit confirmation"
    );
}

#[test]
fn command_palette_backspace_removes_query_char() {
    let mut app = AppState::new();
    app.execute(Command::OpenCommandPalette);
    app.handle_key_event(KeyEvent::new(KeyCode::Char('a'), KeyModifiers::NONE));
    app.handle_key_event(KeyEvent::new(KeyCode::Char('b'), KeyModifiers::NONE));
    assert_eq!(app.command_palette.as_ref().unwrap().query, "ab");
    app.handle_key_event(KeyEvent::new(KeyCode::Backspace, KeyModifiers::NONE));
    assert_eq!(app.command_palette.as_ref().unwrap().query, "a");
}

#[test]
fn close_overlay_closes_command_palette_first() {
    let mut app = AppState::new();
    let tmp = tempfile::TempDir::new().unwrap();
    std::fs::write(tmp.path().join("test.txt"), "").unwrap();
    app.project_root = Some(tmp.path().to_path_buf());
    app.show_help = true;
    app.execute(Command::OpenFileFinder);
    app.execute(Command::OpenCommandPalette);
    assert!(app.command_palette.is_some());
    assert!(app.file_finder.is_some());
    assert!(app.show_help);
    // First CloseOverlay closes palette.
    app.execute(Command::CloseOverlay);
    assert!(
        app.command_palette.is_none(),
        "CloseOverlay should close palette first"
    );
    assert!(app.file_finder.is_some(), "Finder should remain open");
    assert!(app.show_help, "Help should remain open");
    // Second closes finder.
    app.execute(Command::CloseOverlay);
    assert!(app.file_finder.is_none());
    assert!(app.show_help);
    // Third closes help.
    app.execute(Command::CloseOverlay);
    assert!(!app.show_help);
}

#[test]
fn open_project_search_creates_state() {
    let mut app = AppState::new();
    assert!(app.project_search.is_none());
    app.execute(Command::OpenProjectSearch);
    assert!(app.project_search.is_some());
}

#[test]
fn project_search_esc_closes() {
    let mut app = AppState::new();
    app.execute(Command::OpenProjectSearch);
    assert!(app.project_search.is_some());
    app.handle_key_event(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
    assert!(app.project_search.is_none());
}

#[test]
fn close_overlay_priority_includes_project_search() {
    let mut app = AppState::new();
    let tmp = tempfile::TempDir::new().unwrap();
    std::fs::write(tmp.path().join("test.txt"), "").unwrap();
    app.project_root = Some(tmp.path().to_path_buf());
    app.show_help = true;
    app.execute(Command::OpenFileFinder);
    app.execute(Command::OpenProjectSearch);

    assert!(app.project_search.is_some());
    assert!(app.file_finder.is_some());

    // First CloseOverlay closes project search.
    app.execute(Command::CloseOverlay);
    assert!(app.project_search.is_none());
    assert!(app.file_finder.is_some());

    // Second closes file finder.
    app.execute(Command::CloseOverlay);
    assert!(app.file_finder.is_none());

    // Third closes help.
    app.execute(Command::CloseOverlay);
    assert!(!app.show_help);
}

#[test]
fn project_search_char_input_updates_query() {
    let mut app = AppState::new();
    app.execute(Command::OpenProjectSearch);
    app.handle_key_event(KeyEvent::new(KeyCode::Char('t'), KeyModifiers::NONE));
    assert_eq!(app.project_search.as_ref().unwrap().query, "t");
}

#[test]
fn project_search_tab_cycles_field() {
    let mut app = AppState::new();
    app.execute(Command::OpenProjectSearch);
    assert_eq!(
        app.project_search.as_ref().unwrap().active_field,
        crate::project_search::SearchField::Query
    );
    app.handle_key_event(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));
    assert_eq!(
        app.project_search.as_ref().unwrap().active_field,
        crate::project_search::SearchField::Include
    );
}

#[test]
fn project_search_alt_c_toggles_case() {
    let mut app = AppState::new();
    app.execute(Command::OpenProjectSearch);
    assert!(!app.project_search.as_ref().unwrap().case_sensitive);
    app.handle_key_event(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::ALT));
    assert!(app.project_search.as_ref().unwrap().case_sensitive);
}

#[test]
fn project_search_alt_r_toggles_regex() {
    let mut app = AppState::new();
    app.execute(Command::OpenProjectSearch);
    assert!(!app.project_search.as_ref().unwrap().regex_mode);
    app.handle_key_event(KeyEvent::new(KeyCode::Char('r'), KeyModifiers::ALT));
    assert!(app.project_search.as_ref().unwrap().regex_mode);
}

// --- convert_lsp_diagnostics ---

fn make_lsp_diag(
    severity: Option<lsp_types::DiagnosticSeverity>,
    line: u32,
) -> lsp_types::Diagnostic {
    lsp_types::Diagnostic {
        range: lsp_types::Range {
            start: lsp_types::Position { line, character: 0 },
            end: lsp_types::Position { line, character: 5 },
        },
        severity,
        code: None,
        code_description: None,
        source: None,
        message: format!("msg on line {line}"),
        related_information: None,
        tags: None,
        data: None,
    }
}

#[test]
fn convert_error() {
    let diags =
        convert_lsp_diagnostics(&[make_lsp_diag(Some(lsp_types::DiagnosticSeverity::ERROR), 0)]);
    assert_eq!(diags.len(), 1);
    assert_eq!(diags[0].severity, DiagnosticSeverity::Error);
}

#[test]
fn convert_warning() {
    let diags = convert_lsp_diagnostics(&[make_lsp_diag(
        Some(lsp_types::DiagnosticSeverity::WARNING),
        1,
    )]);
    assert_eq!(diags[0].severity, DiagnosticSeverity::Warning);
}

#[test]
fn convert_no_severity_defaults_warning() {
    let diags = convert_lsp_diagnostics(&[make_lsp_diag(None, 2)]);
    assert_eq!(diags[0].severity, DiagnosticSeverity::Warning);
}

#[test]
fn convert_with_code() {
    let mut d = make_lsp_diag(Some(lsp_types::DiagnosticSeverity::ERROR), 0);
    d.code = Some(lsp_types::NumberOrString::String("E0308".to_string()));
    let diags = convert_lsp_diagnostics(&[d]);
    assert_eq!(diags[0].code.as_deref(), Some("E0308"));
}

// --- Diagnostic navigation ---

#[test]
fn next_diagnostic_wraps() {
    let mut app = AppState::new();
    let mut tmp = tempfile::NamedTempFile::new().unwrap();
    use std::io::Write;
    writeln!(tmp, "line0\nline1\nline2\nline3\nline4").unwrap();
    tmp.flush().unwrap();
    app.buffer_manager.open_file(tmp.path()).unwrap();

    let diags = vec![
        BufferDiagnostic {
            line: 1,
            col_start: 0,
            col_end: 5,
            severity: DiagnosticSeverity::Error,
            message: "err".to_string(),
            source: None,
            code: None,
        },
        BufferDiagnostic {
            line: 3,
            col_start: 0,
            col_end: 5,
            severity: DiagnosticSeverity::Warning,
            message: "warn".to_string(),
            source: None,
            code: None,
        },
    ];
    app.buffer_manager
        .active_buffer_mut()
        .unwrap()
        .set_diagnostics(diags);

    // Cursor at line 0 -> next should go to line 1.
    app.execute(Command::GoToNextDiagnostic);
    assert_eq!(app.buffer_manager.active_buffer().unwrap().cursor.row, 1);

    // Next should go to line 3.
    app.execute(Command::GoToNextDiagnostic);
    assert_eq!(app.buffer_manager.active_buffer().unwrap().cursor.row, 3);

    // Next should wrap to line 1.
    app.execute(Command::GoToNextDiagnostic);
    assert_eq!(app.buffer_manager.active_buffer().unwrap().cursor.row, 1);
}

#[test]
fn prev_diagnostic_wraps() {
    let mut app = AppState::new();
    let mut tmp = tempfile::NamedTempFile::new().unwrap();
    use std::io::Write;
    writeln!(tmp, "line0\nline1\nline2\nline3").unwrap();
    tmp.flush().unwrap();
    app.buffer_manager.open_file(tmp.path()).unwrap();

    let diags = vec![
        BufferDiagnostic {
            line: 1,
            col_start: 0,
            col_end: 5,
            severity: DiagnosticSeverity::Error,
            message: "err".to_string(),
            source: None,
            code: None,
        },
        BufferDiagnostic {
            line: 3,
            col_start: 0,
            col_end: 5,
            severity: DiagnosticSeverity::Warning,
            message: "warn".to_string(),
            source: None,
            code: None,
        },
    ];
    app.buffer_manager
        .active_buffer_mut()
        .unwrap()
        .set_diagnostics(diags);

    // Start at line 0 -> prev should wrap to line 3 (last diagnostic).
    app.execute(Command::GoToPrevDiagnostic);
    assert_eq!(app.buffer_manager.active_buffer().unwrap().cursor.row, 3);

    // Prev should go to line 1.
    app.execute(Command::GoToPrevDiagnostic);
    assert_eq!(app.buffer_manager.active_buffer().unwrap().cursor.row, 1);
}

#[test]
fn no_diagnostics_noop() {
    let mut app = AppState::new();
    let mut tmp = tempfile::NamedTempFile::new().unwrap();
    use std::io::Write;
    writeln!(tmp, "hello").unwrap();
    tmp.flush().unwrap();
    app.buffer_manager.open_file(tmp.path()).unwrap();

    app.execute(Command::GoToNextDiagnostic);
    assert_eq!(app.buffer_manager.active_buffer().unwrap().cursor.row, 0);

    app.execute(Command::GoToPrevDiagnostic);
    assert_eq!(app.buffer_manager.active_buffer().unwrap().cursor.row, 0);
}

#[test]
fn show_hover_without_lsp_noop() {
    let mut app = AppState::new();
    app.execute(Command::ShowHover);
    // No LSP manager, so no hover info should be set.
    assert!(app.hover_info.is_none());
}

#[test]
fn hover_dismissed_on_any_key() {
    let mut app = AppState::new();
    app.hover_info = Some(crate::hover::HoverInfo {
        lines: vec![crate::hover::HoverLine {
            spans: vec![crate::hover::HoverSpan {
                text: "test".to_string(),
                bold: false,
                italic: false,
                code: false,
            }],
            is_code_block: false,
        }],
        trigger_row: 0,
        trigger_col: 0,
    });
    assert!(app.hover_info.is_some());

    // Any key (e.g., 'a') should dismiss hover and pass through.
    let key = KeyEvent::new(KeyCode::Char('a'), KeyModifiers::NONE);
    app.handle_key_event(key);
    assert!(app.hover_info.is_none());
}

#[test]
fn hover_dismissed_on_esc() {
    let mut app = AppState::new();
    app.hover_info = Some(crate::hover::HoverInfo {
        lines: vec![crate::hover::HoverLine {
            spans: vec![crate::hover::HoverSpan {
                text: "test".to_string(),
                bold: false,
                italic: false,
                code: false,
            }],
            is_code_block: false,
        }],
        trigger_row: 0,
        trigger_col: 0,
    });
    assert!(app.hover_info.is_some());

    // Esc should dismiss hover but NOT propagate (other overlays stay).
    let key = KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE);
    app.handle_key_event(key);
    assert!(app.hover_info.is_none());
}

#[test]
fn hover_dismissed_on_cursor_movement() {
    let mut app = AppState::new();
    // Open a buffer first.
    let dir = tempfile::tempdir().expect("tmpdir");
    let file = dir.path().join("test.txt");
    std::fs::write(&file, "hello world\nsecond line").expect("write");
    app.execute(Command::OpenFile(file));

    app.hover_info = Some(crate::hover::HoverInfo {
        lines: vec![crate::hover::HoverLine {
            spans: vec![crate::hover::HoverSpan {
                text: "info".to_string(),
                bold: false,
                italic: false,
                code: false,
            }],
            is_code_block: false,
        }],
        trigger_row: 0,
        trigger_col: 0,
    });
    assert!(app.hover_info.is_some());

    app.execute(Command::EditorDown);
    assert!(app.hover_info.is_none());
}

// --- Editor scrollbar drag tests ---

/// Creates an AppState with a 100-line file open and scrollbar area set.
fn app_with_scrollbar() -> AppState {
    let mut app = AppState::new();
    let dir = tempfile::tempdir().expect("tmpdir");
    let file = dir.path().join("big.txt");
    let content: String = (0..100).map(|i| format!("line {i}\n")).collect();
    std::fs::write(&file, &content).expect("write");
    app.execute(Command::OpenFile(file));
    // Simulate editor area: content 80 cols x 20 rows starting at (10, 2).
    app.editor_inner_area = Some((10, 2, 80, 20));
    // Scrollbar is the 1 column to the right of content.
    app.editor_scrollbar_area = Some((90, 2, 1, 20));
    // Leak tempdir so file remains valid.
    std::mem::forget(dir);
    app
}

#[test]
fn scrollbar_hit_detects_click_in_scrollbar_area() {
    let app = app_with_scrollbar();
    assert!(app.scrollbar_hit(90, 5), "expected hit in scrollbar column");
    assert!(
        !app.scrollbar_hit(89, 5),
        "expected no hit outside scrollbar"
    );
    assert!(
        !app.scrollbar_hit(90, 1),
        "expected no hit above scrollbar area"
    );
}

#[test]
fn scrollbar_click_sets_scroll_position() {
    let mut app = app_with_scrollbar();
    let max_scroll = {
        let buf = app.buffer_manager.active_buffer().unwrap();
        buf.line_count().saturating_sub(20) // viewport_height = 20
    };
    // Click at the bottom of the scrollbar.
    app.scrollbar_jump_to(21); // sy + sh - 1 = 2 + 20 - 1 = 21
    let buf = app.buffer_manager.active_buffer().unwrap();
    assert_eq!(buf.scroll_row, max_scroll);
}

#[test]
fn scrollbar_click_top_scrolls_to_beginning() {
    let mut app = app_with_scrollbar();
    // First scroll somewhere.
    if let Some(buf) = app.buffer_manager.active_buffer_mut() {
        buf.scroll_row = 50;
    }
    // Click at top of scrollbar.
    app.scrollbar_jump_to(2); // sy = 2
    let buf = app.buffer_manager.active_buffer().unwrap();
    assert_eq!(buf.scroll_row, 0);
}

#[test]
fn scrollbar_mouse_down_starts_drag() {
    let mut app = app_with_scrollbar();
    // Click on the scrollbar column.
    let evt = mouse_event(MouseEventKind::Down(MouseButton::Left), 90, 10);
    app.handle_mouse_event(evt, 100, 30);
    assert!(app.scrollbar_dragging, "expected scrollbar_dragging = true");
}

#[test]
fn scrollbar_mouse_up_stops_drag() {
    let mut app = app_with_scrollbar();
    app.scrollbar_dragging = true;
    let evt = mouse_event(MouseEventKind::Up(MouseButton::Left), 90, 10);
    app.handle_mouse_event(evt, 100, 30);
    assert!(
        !app.scrollbar_dragging,
        "expected scrollbar_dragging = false after mouse up"
    );
}

#[test]
fn scrollbar_drag_updates_scroll_position() {
    let mut app = app_with_scrollbar();
    app.scrollbar_dragging = true;
    // Drag to middle of scrollbar (sy=2, sh=20 -> middle = row 12).
    let evt = mouse_event(MouseEventKind::Drag(MouseButton::Left), 90, 12);
    app.handle_mouse_event(evt, 100, 30);
    let buf = app.buffer_manager.active_buffer().unwrap();
    // fraction = (12 - 2) / 19 = 0.526, scroll = round(0.526 * 80) = 42
    assert!(
        buf.scroll_row > 0 && buf.scroll_row < 80,
        "expected scroll_row in middle range, got {}",
        buf.scroll_row
    );
}

// --- Find and Replace tests ---

fn app_with_text(text: &str) -> AppState {
    let tmp = tempfile::NamedTempFile::new().expect("create temp file");
    std::fs::write(tmp.path(), text).expect("write temp file");
    let mut app = AppState::new();
    app.focus = FocusTarget::Editor;
    app.buffer_manager.open_file(tmp.path()).expect("open file");
    app
}

#[test]
fn find_replace_opens_with_replace_visible() {
    let mut app = app_with_text("hello");
    app.execute(Command::EditorFindReplace);
    let search = app.search.as_ref().expect("search should be open");
    assert!(search.replace_visible);
}

#[test]
fn find_replace_reuses_existing_search() {
    let mut app = app_with_text("hello");
    app.execute(Command::EditorFind);
    assert!(app.search.is_some());
    assert!(!app.search.as_ref().unwrap().replace_visible);
    app.execute(Command::EditorFindReplace);
    let search = app.search.as_ref().unwrap();
    assert!(search.replace_visible);
    assert_eq!(search.active_field, crate::search::SearchField::Replace);
}

#[test]
fn replace_next_replaces_current_match() {
    let mut app = app_with_text("foo foo foo");
    app.execute(Command::EditorFind);
    if let Some(ref mut search) = app.search {
        if let Some(buf) = app.buffer_manager.active_buffer() {
            search.query = "foo".to_string();
            search.replace_query = "bar".to_string();
            search.replace_visible = true;
            search.update_matches(buf);
        }
    }
    app.execute(Command::ReplaceNext);
    let content = app.buffer_manager.active_buffer().unwrap().content_string();
    assert_eq!(content, "bar foo foo");
}

#[test]
fn replace_next_advances_to_next() {
    let mut app = app_with_text("foo foo foo");
    app.execute(Command::EditorFind);
    if let Some(ref mut search) = app.search {
        if let Some(buf) = app.buffer_manager.active_buffer() {
            search.query = "foo".to_string();
            search.replace_query = "bar".to_string();
            search.replace_visible = true;
            search.update_matches(buf);
        }
    }
    app.execute(Command::ReplaceNext);
    let search = app.search.as_ref().unwrap();
    // After replacing first "foo" with "bar", there should be 2 matches left.
    assert_eq!(search.matches.len(), 2);
}

#[test]
fn replace_next_no_match_noop() {
    let mut app = app_with_text("hello world");
    app.execute(Command::EditorFind);
    if let Some(ref mut search) = app.search {
        if let Some(buf) = app.buffer_manager.active_buffer() {
            search.query = "xyz".to_string();
            search.replace_query = "abc".to_string();
            search.replace_visible = true;
            search.update_matches(buf);
        }
    }
    app.execute(Command::ReplaceNext);
    let content = app.buffer_manager.active_buffer().unwrap().content_string();
    assert_eq!(content, "hello world");
}

#[test]
fn replace_all_replaces_all() {
    let mut app = app_with_text("foo foo foo");
    app.execute(Command::EditorFind);
    if let Some(ref mut search) = app.search {
        if let Some(buf) = app.buffer_manager.active_buffer() {
            search.query = "foo".to_string();
            search.replace_query = "bar".to_string();
            search.replace_visible = true;
            search.update_matches(buf);
        }
    }
    app.execute(Command::ReplaceAll);
    let content = app.buffer_manager.active_buffer().unwrap().content_string();
    assert_eq!(content, "bar bar bar");
}

#[test]
fn replace_all_single_undo() {
    let mut app = app_with_text("foo foo foo");
    app.execute(Command::EditorFind);
    if let Some(ref mut search) = app.search {
        if let Some(buf) = app.buffer_manager.active_buffer() {
            search.query = "foo".to_string();
            search.replace_query = "bar".to_string();
            search.replace_visible = true;
            search.update_matches(buf);
        }
    }
    app.execute(Command::ReplaceAll);
    // Single undo should restore original.
    app.execute(Command::EditorUndo);
    let content = app.buffer_manager.active_buffer().unwrap().content_string();
    assert_eq!(content, "foo foo foo");
}

#[test]
fn replace_all_different_length() {
    let mut app = app_with_text("ab ab");
    app.execute(Command::EditorFind);
    if let Some(ref mut search) = app.search {
        if let Some(buf) = app.buffer_manager.active_buffer() {
            search.query = "ab".to_string();
            search.replace_query = "xyz".to_string();
            search.replace_visible = true;
            search.update_matches(buf);
        }
    }
    app.execute(Command::ReplaceAll);
    let content = app.buffer_manager.active_buffer().unwrap().content_string();
    assert_eq!(content, "xyz xyz");
}

#[test]
fn replace_all_empty_matches_noop() {
    let mut app = app_with_text("hello");
    app.execute(Command::EditorFind);
    if let Some(ref mut search) = app.search {
        if let Some(buf) = app.buffer_manager.active_buffer() {
            search.query = "xyz".to_string();
            search.replace_query = "abc".to_string();
            search.replace_visible = true;
            search.update_matches(buf);
        }
    }
    app.execute(Command::ReplaceAll);
    let content = app.buffer_manager.active_buffer().unwrap().content_string();
    assert_eq!(content, "hello");
}
