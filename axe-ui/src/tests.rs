use super::*;
use axe_core::{AppState, FocusTarget};
use ratatui::backend::TestBackend;
use ratatui::layout::Rect;
use ratatui::Terminal;

use crate::editor_panel::{
    char_to_display_col, expand_tabs, gutter_width, render_scrollbar, render_tab_bar,
};
use crate::overlays::{
    format_help_key, help_overlay_dimensions, HelpEntry, HELP_COLUMNS, HELP_COL_GAP,
    HELP_INNER_PAD, HELP_SECTIONS, HELP_SINGLE_COL_WIDTH, HELP_TABS,
};
use crate::theme::Theme;

fn render_app_to_string(app: &AppState, width: u16, height: u16) -> String {
    let backend = TestBackend::new(width, height);
    let mut terminal = Terminal::new(backend).unwrap();

    let theme = Theme::default();
    terminal.draw(|frame| render(app, frame, &theme)).unwrap();

    terminal
        .backend()
        .buffer()
        .content()
        .iter()
        .map(|cell| cell.symbol().chars().next().unwrap_or(' '))
        .collect()
}

fn render_to_string(width: u16, height: u16) -> String {
    let app = AppState::new();
    render_app_to_string(&app, width, height)
}

#[test]
fn render_shows_files_panel_title() {
    let content = render_to_string(80, 24);
    assert!(
        content.contains("Files"),
        "expected 'Files' in rendered output"
    );
}

#[test]
fn render_shows_editor_panel_title() {
    let content = render_to_string(80, 24);
    assert!(
        content.contains("Editor"),
        "expected 'Editor' in rendered output"
    );
}

#[test]
fn render_shows_terminal_panel_title() {
    let content = render_to_string(80, 24);
    assert!(
        content.contains("Terminal"),
        "expected 'Terminal' in rendered output"
    );
}

#[test]
fn render_shows_status_bar_version() {
    let content = render_to_string(100, 24);
    assert!(content.contains("Axe v"), "expected 'Axe v' in status bar");
}

#[test]
fn render_works_with_small_terminal() {
    let content = render_to_string(40, 10);
    assert!(!content.is_empty());
}

#[test]
fn render_hides_tree_when_show_tree_false() {
    let mut app = AppState::new();
    app.show_tree = false;
    app.focus = FocusTarget::Editor;
    let content = render_app_to_string(&app, 80, 24);
    assert!(
        !content.contains("Files"),
        "expected 'Files' to be absent when tree is hidden"
    );
}

#[test]
fn render_hides_terminal_when_show_terminal_false() {
    let mut app = AppState::new();
    app.show_terminal = false;
    let content = render_app_to_string(&app, 80, 24);
    assert!(
        !content.contains("Terminal"),
        "expected 'Terminal' to be absent when terminal is hidden"
    );
}

#[test]
fn render_editor_fills_width_when_tree_hidden() {
    let mut app = AppState::new();
    app.show_tree = false;
    app.focus = FocusTarget::Editor;
    let content = render_app_to_string(&app, 80, 24);
    assert!(
        content.contains("Editor"),
        "expected 'Editor' visible when tree is hidden"
    );
    assert!(
        !content.contains("Files"),
        "expected 'Files' absent when tree is hidden"
    );
}

#[test]
fn render_editor_fills_height_when_terminal_hidden() {
    let mut app = AppState::new();
    app.show_terminal = false;
    let content = render_app_to_string(&app, 80, 24);
    assert!(
        content.contains("Editor"),
        "expected 'Editor' visible when terminal is hidden"
    );
    assert!(
        !content.contains("Terminal"),
        "expected 'Terminal' absent when terminal is hidden"
    );
}

#[test]
fn render_help_overlay_when_show_help_true() {
    let mut app = AppState::new();
    app.show_help = true;
    let content = render_app_to_string(&app, 130, 40);
    assert!(content.contains("Help"), "expected 'Help' title in overlay");
    assert!(content.contains("Quit"), "expected 'Quit' in help content");
}

#[test]
fn render_no_help_overlay_by_default() {
    let content = render_to_string(80, 24);
    // "Help" appears in status bar hints, but the overlay key list items
    // like "Toggle this help" should NOT appear unless overlay is shown.
    assert!(
        !content.contains("Toggle this help"),
        "expected help overlay content absent by default"
    );
}

#[test]
fn render_help_overlay_shows_keybindings() {
    let mut app = AppState::new();
    app.show_help = true;
    let content = render_app_to_string(&app, 130, 50);
    assert!(content.contains("Ctrl+Q"), "expected 'Ctrl+Q' in help");
    assert!(content.contains("Ctrl+B"), "expected 'Ctrl+B' in help");
    assert!(content.contains("Ctrl+T"), "expected 'Ctrl+T' in help");
    assert!(content.contains("Ctrl+R"), "expected 'Ctrl+R' in help");
    assert!(content.contains("Ctrl+N"), "expected 'Ctrl+N' in help");
    assert!(content.contains("Esc"), "expected 'Esc' in help");
}

#[test]
fn format_help_key_with_fallback_and_primary() {
    let entry = HelpEntry {
        fallback_key: Some("F3"),
        primary_key: "Alt+/",
        description: "Code completion",
    };
    assert_eq!(format_help_key(&entry), "F3 / Alt+/");
}

#[test]
fn format_help_key_fallback_only() {
    let entry = HelpEntry {
        fallback_key: Some("F12"),
        primary_key: "",
        description: "Go to definition",
    };
    assert_eq!(format_help_key(&entry), "F12");
}

#[test]
fn format_help_key_primary_only() {
    let entry = HelpEntry {
        fallback_key: None,
        primary_key: "Ctrl+Q",
        description: "Quit",
    };
    assert_eq!(format_help_key(&entry), "Ctrl+Q");
}

#[test]
fn help_overlay_dimensions_clamps_to_small_screen() {
    let area = Rect::new(0, 0, 60, 15);
    let (w, h) = help_overlay_dimensions(area, 20);
    // Content width is ~122 but screen is only 60, so clamped to 58
    assert_eq!(w, 58);
    // Height: 20 + 3 + 2 = 25, clamped to 15-2=13
    assert_eq!(h, 13);
}

#[test]
fn help_overlay_dimensions_fits_content_on_large_screen() {
    let area = Rect::new(0, 0, 200, 80);
    let (w, h) = help_overlay_dimensions(area, 20);
    // Content width: 38*3 + 2*2 + 1*2 + 2 = 122
    let expected_w = HELP_SINGLE_COL_WIDTH * HELP_COLUMNS as u16
        + HELP_COL_GAP * (HELP_COLUMNS as u16 - 1)
        + HELP_INNER_PAD * 2
        + 2;
    assert_eq!(w, expected_w);
    // Height: 20 + 3 + 2 = 25
    assert_eq!(h, 25);
}

#[test]
fn help_overlay_wider_than_tall() {
    let area = Rect::new(0, 0, 160, 50);
    let (w, h) = help_overlay_dimensions(area, 20);
    assert!(w > h, "expected overlay to be wider ({w}) than tall ({h})");
}

#[test]
fn all_help_sections_have_entries() {
    for section in HELP_SECTIONS {
        assert!(
            !section.entries.is_empty(),
            "section '{}' has no entries",
            section.title,
        );
    }
}

#[test]
fn fallback_keys_appear_first() {
    // Verify that when both fallback and primary keys exist,
    // the formatted string starts with the fallback key.
    for section in HELP_SECTIONS {
        for entry in section.entries {
            if let Some(fallback) = entry.fallback_key {
                let formatted = format_help_key(entry);
                assert!(
                    formatted.starts_with(fallback),
                    "expected fallback key '{fallback}' to appear first in '{formatted}'"
                );
            }
        }
    }
}

#[test]
fn render_confirm_dialog_when_quit() {
    let mut app = AppState::new();
    app.confirm_dialog = Some(axe_core::ConfirmDialog::quit());
    let content = render_app_to_string(&app, 80, 24);
    assert!(
        content.contains("Yes"),
        "expected 'Yes' button in quit confirmation overlay"
    );
    assert!(
        content.contains("No"),
        "expected 'No' button in quit confirmation overlay"
    );
    assert!(
        content.contains("Quit"),
        "expected 'Quit' title in quit confirmation overlay"
    );
}

#[test]
fn render_no_confirm_dialog_by_default() {
    let app = AppState::new();
    let content = render_app_to_string(&app, 80, 24);
    assert!(
        !content.contains("[ Yes ]"),
        "confirm dialog should not appear by default"
    );
}

#[test]
fn render_confirm_dialog_close_buffer() {
    let mut app = AppState::new();
    let mut tmp = tempfile::NamedTempFile::new().unwrap();
    std::io::Write::write_all(&mut tmp, b"hello\n").unwrap();
    std::io::Write::flush(&mut tmp).unwrap();
    app.execute(axe_core::Command::OpenFile(tmp.path().to_path_buf()));
    app.buffer_manager.active_buffer_mut().unwrap().modified = true;
    app.confirm_dialog = Some(axe_core::ConfirmDialog::close_buffer("test.txt"));
    let content = render_app_to_string(&app, 80, 24);
    assert!(
        content.contains("Unsaved"),
        "expected 'Unsaved' in close-buffer overlay"
    );
    assert!(
        content.contains("Yes"),
        "expected 'Yes' button in close-buffer overlay"
    );
}

#[test]
fn render_shows_resize_indicator_when_resize_mode_active() {
    let mut app = AppState::new();
    app.resize_mode.active = true;
    let content = render_app_to_string(&app, 100, 24);
    assert!(
        content.contains("RESIZE"),
        "expected 'RESIZE' badge in status bar"
    );
}

#[test]
fn render_no_resize_indicator_by_default() {
    let content = render_to_string(100, 24);
    assert!(
        !content.contains("RESIZE"),
        "expected no 'RESIZE' by default"
    );
}

// --- Zoom rendering tests ---

#[test]
fn render_zoomed_shows_only_focused_panel() {
    let mut app = AppState::new();
    app.focus = FocusTarget::Editor;
    app.zoomed_panel = Some(FocusTarget::Editor);
    let content = render_app_to_string(&app, 100, 24);
    assert!(content.contains("Editor"), "expected 'Editor' panel");
    assert!(
        !content.contains("Files"),
        "expected 'Files' hidden when editor is zoomed"
    );
    assert!(
        !content.contains("Terminal"),
        "expected 'Terminal' hidden when editor is zoomed"
    );
}

#[test]
fn render_zoomed_tree_shows_only_tree() {
    let mut app = AppState::new();
    app.focus = FocusTarget::Tree;
    app.zoomed_panel = Some(FocusTarget::Tree);
    let content = render_app_to_string(&app, 100, 24);
    assert!(content.contains("Files"), "expected 'Files' panel");
    assert!(
        !content.contains("Editor"),
        "expected 'Editor' hidden when tree is zoomed"
    );
    assert!(
        !content.contains("Terminal"),
        "expected 'Terminal' hidden when tree is zoomed"
    );
}

#[test]
fn render_zoomed_terminal_shows_only_terminal() {
    let mut app = AppState::new();
    app.focus = FocusTarget::Terminal(0);
    app.zoomed_panel = Some(FocusTarget::Terminal(0));
    let content = render_app_to_string(&app, 100, 24);
    assert!(content.contains("Terminal"), "expected 'Terminal' panel");
    assert!(
        !content.contains("Files"),
        "expected 'Files' hidden when terminal is zoomed"
    );
    assert!(
        !content.contains("Editor"),
        "expected 'Editor' hidden when terminal is zoomed"
    );
}

#[test]
fn render_zoom_indicator_in_status_bar() {
    let mut app = AppState::new();
    app.zoomed_panel = Some(FocusTarget::Editor);
    let content = render_app_to_string(&app, 100, 24);
    assert!(
        content.contains("ZOOM"),
        "expected 'ZOOM' badge in status bar when zoomed"
    );
}

#[test]
fn render_no_zoom_indicator_by_default() {
    let content = render_to_string(100, 24);
    assert!(!content.contains("ZOOM"), "expected no 'ZOOM' by default");
}

#[test]
fn render_zoomed_panel_title_has_suffix() {
    let mut app = AppState::new();
    app.focus = FocusTarget::Editor;
    app.zoomed_panel = Some(FocusTarget::Editor);
    let content = render_app_to_string(&app, 100, 24);
    assert!(
        content.contains("(zoomed)"),
        "expected '(zoomed)' suffix in panel title"
    );
}

#[test]
fn render_uses_app_tree_width_pct() {
    let mut app = AppState::new();
    app.tree_width_pct = 40;
    // Just ensure it renders without panic; the visual difference
    // is verified by the layout using the custom percentage.
    let content = render_app_to_string(&app, 100, 24);
    assert!(content.contains("Files"), "expected 'Files' panel");
    assert!(content.contains("Editor"), "expected 'Editor' panel");
}

// --- File tree rendering tests ---

fn app_with_tree() -> (AppState, tempfile::TempDir) {
    let tmp = tempfile::TempDir::new().unwrap();
    std::fs::create_dir(tmp.path().join("src")).unwrap();
    std::fs::write(tmp.path().join("Cargo.toml"), "").unwrap();
    std::fs::write(tmp.path().join("README.md"), "").unwrap();
    let app = AppState::new_with_root(tmp.path().to_path_buf());
    (app, tmp)
}

#[test]
fn render_tree_shows_root_name() {
    let (app, tmp) = app_with_tree();
    let root_name = tmp
        .path()
        .canonicalize()
        .unwrap()
        .file_name()
        .unwrap()
        .to_string_lossy()
        .into_owned();
    let content = render_app_to_string(&app, 100, 24);
    assert!(
        content.contains(&root_name),
        "expected root name '{root_name}' in rendered output"
    );
}

#[test]
fn render_tree_shows_directory_prefix_without_icons() {
    let (mut app, _tmp) = app_with_tree();
    // Disable icons to get plain text prefixes.
    if let Some(ref mut tree) = app.file_tree {
        tree.toggle_show_icons();
    }
    let content = render_app_to_string(&app, 100, 24);
    assert!(
        content.contains('\u{25B8}'),
        "expected collapsed dir prefix '\u{25B8}' in rendered output when icons disabled"
    );
}

#[test]
fn render_tree_shows_file_entries() {
    let (app, _tmp) = app_with_tree();
    let content = render_app_to_string(&app, 100, 24);
    assert!(
        content.contains("Cargo.toml"),
        "expected 'Cargo.toml' in tree"
    );
    assert!(
        content.contains("README.md"),
        "expected 'README.md' in tree"
    );
}

#[test]
fn render_tree_shows_directory_name() {
    let (app, _tmp) = app_with_tree();
    let content = render_app_to_string(&app, 100, 24);
    assert!(content.contains("src"), "expected 'src' directory in tree");
}

// --- Editor content rendering tests ---

#[test]
fn gutter_width_includes_diagnostic_and_diff_columns() {
    // DIAGNOSTIC_GUTTER_WIDTH(2) + digits + GUTTER_PADDING(2) + DIFF_GUTTER_WIDTH(1)
    assert_eq!(gutter_width(1), 6); // 2 + 1 digit + 2 padding + 1 diff
    assert_eq!(gutter_width(9), 6); // 2 + 1 digit + 2 padding + 1 diff
    assert_eq!(gutter_width(10), 7); // 2 + 2 digits + 2 padding + 1 diff
    assert_eq!(gutter_width(99), 7);
    assert_eq!(gutter_width(100), 8); // 2 + 3 digits + 2 padding + 1 diff
    assert_eq!(gutter_width(999), 8);
}

fn app_with_open_file() -> (AppState, tempfile::TempDir) {
    let tmp = tempfile::TempDir::new().unwrap();
    let file_path = tmp.path().join("test.rs");
    std::fs::write(&file_path, "fn main() {\n    println!(\"hello\");\n}\n").unwrap();

    let mut app = AppState::new_with_root(tmp.path().to_path_buf());
    app.execute(axe_core::Command::OpenFile(file_path));
    (app, tmp)
}

#[test]
fn render_editor_shows_line_numbers() {
    let (app, _tmp) = app_with_open_file();
    let content = render_app_to_string(&app, 100, 24);
    assert!(content.contains('1'), "expected line number 1 in editor");
    assert!(content.contains('2'), "expected line number 2 in editor");
}

#[test]
fn render_editor_shows_file_content() {
    let (app, _tmp) = app_with_open_file();
    let content = render_app_to_string(&app, 100, 24);
    assert!(
        content.contains("fn main()"),
        "expected file content in editor, got: {content}"
    );
}

#[test]
fn status_bar_shows_filename() {
    let (app, _tmp) = app_with_open_file();
    let content = render_app_to_string(&app, 120, 24);
    assert!(
        content.contains("test.rs"),
        "expected filename in status bar"
    );
}

#[test]
fn status_bar_shows_encoding() {
    let (app, _tmp) = app_with_open_file();
    let content = render_app_to_string(&app, 120, 24);
    assert!(
        content.contains("UTF-8"),
        "expected 'UTF-8' encoding in status bar"
    );
}

#[test]
fn status_bar_shows_line_ending() {
    let (app, _tmp) = app_with_open_file();
    let content = render_app_to_string(&app, 120, 24);
    assert!(
        content.contains("LF"),
        "expected line ending indicator in status bar"
    );
}

#[test]
fn status_bar_shows_file_type() {
    let (app, _tmp) = app_with_open_file();
    let content = render_app_to_string(&app, 120, 24);
    assert!(
        content.contains("Rust"),
        "expected 'Rust' file type in status bar"
    );
}

// --- Tab bar rendering tests ---

fn app_with_two_files() -> (AppState, tempfile::TempDir) {
    let tmp = tempfile::TempDir::new().unwrap();
    let file1 = tmp.path().join("main.rs");
    let file2 = tmp.path().join("lib.rs");
    std::fs::write(&file1, "fn main() {}\n").unwrap();
    std::fs::write(&file2, "pub fn lib() {}\n").unwrap();

    let mut app = AppState::new_with_root(tmp.path().to_path_buf());
    app.execute(axe_core::Command::OpenFile(file1));
    app.execute(axe_core::Command::OpenFile(file2));
    (app, tmp)
}

#[test]
fn tab_bar_not_shown_with_single_buffer() {
    let (app, _tmp) = app_with_open_file();
    assert_eq!(app.buffer_manager.buffer_count(), 1);
    let content = render_app_to_string(&app, 100, 24);
    // With a single buffer, there should be no tab bar.
    // The file content should still be visible.
    assert!(
        content.contains("fn main()"),
        "expected file content with single buffer"
    );
}

#[test]
fn tab_bar_shown_with_multiple_buffers() {
    let (app, _tmp) = app_with_two_files();
    assert_eq!(app.buffer_manager.buffer_count(), 2);
    let content = render_app_to_string(&app, 100, 24);
    // Both filenames should appear in the tab bar.
    assert!(content.contains("main.rs"), "expected 'main.rs' in tab bar");
    assert!(content.contains("lib.rs"), "expected 'lib.rs' in tab bar");
}

#[test]
fn tab_bar_shows_modified_indicator() {
    let (mut app, _tmp) = app_with_two_files();
    // Modify the active buffer.
    app.execute(axe_core::Command::EditorInsertChar('x'));
    let content = render_app_to_string(&app, 100, 24);
    // Format: "[2:lib.rs+]" — "+" before closing bracket indicates modified.
    assert!(
        content.contains("+]"),
        "expected '+]' modified indicator in tab bar"
    );
}

#[test]
fn render_tab_bar_uses_theme_colors() {
    let buffers = vec![axe_editor::EditorBuffer::new()];
    let theme = Theme::default();
    let backend = TestBackend::new(40, 1);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|frame| {
            let area = Rect {
                x: 0,
                y: 0,
                width: 40,
                height: 1,
            };
            render_tab_bar(&buffers, 0, area, frame, &theme);
        })
        .unwrap();
    // Verify it renders without panic and the active tab color is applied.
    let buf = terminal.backend().buffer();
    let cell = &buf[(0, 0)];
    // The first (and only) tab is active, so it uses panel_border_active fg.
    assert_eq!(
        cell.fg, theme.panel_border_active,
        "expected panel_border_active on active tab"
    );
}

#[test]
fn editor_inner_rect_accounts_for_tab_bar() {
    let (app, _tmp) = app_with_two_files();
    let area = Rect {
        x: 0,
        y: 0,
        width: 100,
        height: 24,
    };
    let rect_multi = editor_inner_rect(&app, area);

    // Compare with single buffer.
    let (app_single, _tmp2) = app_with_open_file();
    let rect_single = editor_inner_rect(&app_single, area);

    // With multiple buffers, the editor content rect should start 1 row lower.
    assert!(
        rect_multi.is_some() && rect_single.is_some(),
        "expected both rects to be Some"
    );
    let multi = rect_multi.unwrap();
    let single = rect_single.unwrap();
    assert_eq!(
        multi.y,
        single.y + 1,
        "expected tab bar to shift editor content down by 1 row"
    );
    assert_eq!(
        multi.height,
        single.height - 1,
        "expected tab bar to reduce editor content height by 1"
    );
}

// --- Startup screen tests ---

#[test]
fn startup_screen_shows_logo() {
    let mut app = AppState::new();
    app.focus = FocusTarget::Editor;
    app.zoomed_panel = Some(FocusTarget::Editor);
    let content = render_app_to_string(&app, 100, 24);
    assert!(
        content.contains("@@@@@@") && content.contains("@!@!@!@!"),
        "expected logo chars in startup screen with no buffers"
    );
}

#[test]
fn startup_screen_shows_version() {
    let mut app = AppState::new();
    app.build_version = "v0.1.0-abc123".to_string();
    app.focus = FocusTarget::Editor;
    app.zoomed_panel = Some(FocusTarget::Editor);
    let content = render_app_to_string(&app, 100, 30);
    assert!(
        content.contains("v0.1.0-abc123"),
        "expected version string in startup screen"
    );
    // Must not double the 'v' prefix.
    assert!(
        !content.contains("vv0.1.0"),
        "expected no double 'v' prefix"
    );
}

#[test]
fn startup_screen_shows_shortcuts() {
    let mut app = AppState::new();
    app.focus = FocusTarget::Editor;
    app.zoomed_panel = Some(FocusTarget::Editor);
    let content = render_app_to_string(&app, 100, 40);
    assert!(
        content.contains("Ctrl+P"),
        "expected 'Ctrl+P' shortcut in startup screen"
    );
    assert!(
        content.contains("Open file finder"),
        "expected 'Open file finder' description in startup screen"
    );
    assert!(
        content.contains("F1"),
        "expected 'F1' for help in startup screen"
    );
    assert!(
        content.contains("Ctrl+T"),
        "expected 'Ctrl+T' for terminal in startup screen"
    );
}

#[test]
fn startup_screen_disappears_when_file_opened() {
    let mut app = AppState::new();
    let mut tmp = tempfile::NamedTempFile::new().unwrap();
    std::io::Write::write_all(&mut tmp, b"hello\n").unwrap();
    std::io::Write::flush(&mut tmp).unwrap();
    app.execute(axe_core::Command::OpenFile(tmp.path().to_path_buf()));
    app.focus = FocusTarget::Editor;
    app.zoomed_panel = Some(FocusTarget::Editor);
    let content = render_app_to_string(&app, 100, 24);
    assert!(
        !content.contains("@@@@@@"),
        "expected no logo when a buffer is open"
    );
}

#[test]
fn startup_screen_graceful_on_small_area() {
    let mut app = AppState::new();
    app.focus = FocusTarget::Editor;
    app.zoomed_panel = Some(FocusTarget::Editor);
    // Small terminal — should not panic.
    let content = render_app_to_string(&app, 40, 5);
    assert!(
        !content.is_empty(),
        "expected non-empty output on small terminal"
    );
}

#[test]
fn startup_screen_unzoomed_shows_logo() {
    let app = AppState::new();
    let content = render_app_to_string(&app, 100, 24);
    assert!(
        content.contains("@@@@@@"),
        "expected logo in unzoomed editor with no buffers"
    );
}

#[test]
fn help_sections_contain_tab_keybindings() {
    let has_next_tab = HELP_TABS
        .entries
        .iter()
        .any(|e| e.primary_key == "Ctrl+Shift+]/[");
    let has_close_tab = HELP_TABS
        .entries
        .iter()
        .any(|e| e.primary_key == "Ctrl+W");
    assert!(has_next_tab, "expected 'Ctrl+PgDn/PgUp' in HELP_TABS");
    assert!(has_close_tab, "expected 'Ctrl+W' in HELP_TABS");
}

#[test]
fn terminal_tab_bar_rect_returns_some_when_tabs_exist() {
    let mut app = AppState::new();
    let cwd = std::env::current_dir().unwrap();
    let mut mgr = axe_terminal::TerminalManager::new();
    mgr.spawn_tab(80, 24, &cwd).unwrap();
    mgr.spawn_tab(80, 24, &cwd).unwrap();
    app.terminal_manager = Some(mgr);

    let area = Rect::new(0, 0, 120, 40);
    let result = terminal_tab_bar_rect(&app, area);
    assert!(result.is_some(), "expected Some when terminal has tabs");
    let rect = result.unwrap();
    assert_eq!(rect.height, 1, "tab bar should be 1 row tall");
}

#[test]
fn terminal_tab_bar_rect_returns_none_when_no_tabs() {
    let app = AppState::new();
    // No terminal manager at all.
    let area = Rect::new(0, 0, 120, 40);
    let result = terminal_tab_bar_rect(&app, area);
    assert!(result.is_none(), "expected None when no terminal manager");
}

/// Verifies that the terminal grid area is explicitly cleared before
/// rendering content, so stale characters from a previous frame do not
/// persist when cells have no terminal output.
#[test]
fn terminal_grid_area_cleared_before_content_render() {
    let backend = TestBackend::new(80, 24);
    let mut terminal = Terminal::new(backend).unwrap();
    let theme = Theme::default();

    let mut app = AppState::new();
    let cwd = std::env::current_dir().unwrap();
    let mut mgr = axe_terminal::TerminalManager::new();
    mgr.spawn_tab(80, 24, &cwd).unwrap();
    app.terminal_manager = Some(mgr);

    // First pass: pre-fill the terminal grid region with stale "X" chars
    // to simulate leftover content from a previous frame.
    terminal
        .draw(|frame| {
            let buf = frame.buffer_mut();
            // Fill entire buffer with 'X' to simulate stale content.
            for y in 0..24u16 {
                for x in 0..80u16 {
                    if let Some(cell) = buf.cell_mut((x, y)) {
                        cell.set_char('X');
                    }
                }
            }
        })
        .unwrap();

    // Second pass: render the actual app. The terminal grid area should be
    // cleared — no 'X' chars should remain in the terminal grid region.
    terminal.draw(|frame| render(&app, frame, &theme)).unwrap();

    // Find the terminal grid area by checking where 'X' persists.
    // After a proper clear + render, the terminal grid cells should NOT
    // contain 'X' (they should be space/empty from the clear).
    let buf = terminal.backend().buffer();

    // The terminal panel occupies the bottom-right. Check the interior
    // cells (skip borders). With default layout, terminal starts roughly
    // at the bottom half of the right panel.
    let mut stale_x_count = 0;
    for y in 0..buf.area().height {
        for x in 0..buf.area().width {
            if let Some(cell) = buf.cell((x, y)) {
                // Only count 'X' in the lower portion (terminal area).
                // The terminal grid is roughly in the bottom-right.
                if cell.symbol() == "X" && y > buf.area().height / 2 {
                    stale_x_count += 1;
                }
            }
        }
    }

    assert_eq!(
        stale_x_count, 0,
        "Stale 'X' characters found in terminal grid area — grid not cleared before render"
    );
}

/// Verifies that the terminal scrollbar area is cleared before rendering,
/// preventing stale scrollbar artifacts when scroll position changes.
#[test]
fn terminal_scrollbar_area_cleared_before_render() {
    let backend = TestBackend::new(80, 24);
    let mut terminal = Terminal::new(backend).unwrap();
    let theme = Theme::default();

    let mut app = AppState::new();
    let cwd = std::env::current_dir().unwrap();
    let mut mgr = axe_terminal::TerminalManager::new();
    mgr.spawn_tab(80, 24, &cwd).unwrap();
    app.terminal_manager = Some(mgr);

    // Pre-fill buffer with stale content.
    terminal
        .draw(|frame| {
            let buf = frame.buffer_mut();
            for y in 0..24u16 {
                for x in 0..80u16 {
                    if let Some(cell) = buf.cell_mut((x, y)) {
                        cell.set_char('X');
                    }
                }
            }
        })
        .unwrap();

    // Render the app. The scrollbar column should not retain 'X'.
    terminal.draw(|frame| render(&app, frame, &theme)).unwrap();

    let buf = terminal.backend().buffer();
    // The scrollbar is the rightmost column of the terminal panel.
    // Check all cells in the lower-right region for stale 'X'.
    let mut stale_in_scrollbar = 0;
    for y in (buf.area().height / 2)..buf.area().height {
        // The scrollbar column is 1 column before the right border of the terminal panel.
        // With default layout (tree 20%, editor 50%), the right border is at col 79.
        // The scrollbar is at col 78.
        let x = buf.area().width - 2; // Just inside the right border.
        if let Some(cell) = buf.cell((x, y)) {
            if cell.symbol() == "X" {
                stale_in_scrollbar += 1;
            }
        }
    }

    assert_eq!(
        stale_in_scrollbar, 0,
        "Stale 'X' characters found in scrollbar area — scrollbar not cleared before render"
    );
}

#[test]
fn terminal_tab_bar_rect_returns_none_when_terminal_hidden() {
    let mut app = AppState::new();
    let cwd = std::env::current_dir().unwrap();
    let mut mgr = axe_terminal::TerminalManager::new();
    mgr.spawn_tab(80, 24, &cwd).unwrap();
    mgr.spawn_tab(80, 24, &cwd).unwrap();
    app.terminal_manager = Some(mgr);
    app.show_terminal = false;

    let area = Rect::new(0, 0, 120, 40);
    let result = terminal_tab_bar_rect(&app, area);
    assert!(result.is_none(), "expected None when terminal is hidden");
}

// --- Scrollbar rendering tests ---

#[test]
fn scrollbar_thumb_at_top_when_scroll_zero() {
    let theme = Theme::default();
    let area = Rect::new(0, 0, 1, 20);
    let mut buf = ratatui::buffer::Buffer::empty(area);
    render_scrollbar(100, 20, 0, area, &mut buf, &theme);
    // Thumb should start at row 0
    let cell = buf.cell((0, 0)).unwrap();
    assert_eq!(cell.symbol(), "\u{2588}", "expected thumb at top row");
}

#[test]
fn scrollbar_thumb_at_bottom_when_scroll_max() {
    let theme = Theme::default();
    let area = Rect::new(0, 0, 1, 20);
    let mut buf = ratatui::buffer::Buffer::empty(area);
    let total = 100;
    let visible = 20;
    let max_offset = total - visible;
    render_scrollbar(total, visible, max_offset, area, &mut buf, &theme);
    // Thumb should end at the bottom row
    let cell = buf.cell((0, 19)).unwrap();
    assert_eq!(cell.symbol(), "\u{2588}", "expected thumb at bottom row");
}

#[test]
fn scrollbar_hidden_when_content_fits() {
    let theme = Theme::default();
    let area = Rect::new(0, 0, 1, 20);
    let mut buf = ratatui::buffer::Buffer::empty(area);
    render_scrollbar(15, 20, 0, area, &mut buf, &theme);
    // No rendering — all cells should remain default (space)
    for y in 0..20 {
        let cell = buf.cell((0, y)).unwrap();
        assert_eq!(
            cell.symbol(),
            " ",
            "expected no scrollbar at row {y} when content fits"
        );
    }
}

#[test]
fn scrollbar_thumb_size_proportional() {
    let theme = Theme::default();
    let area = Rect::new(0, 0, 1, 20);

    // Small file (40 lines, 20 visible) — thumb should be large (10 rows)
    let mut buf = ratatui::buffer::Buffer::empty(area);
    render_scrollbar(40, 20, 0, area, &mut buf, &theme);
    let thumb_count_small: u16 = (0..20)
        .filter(|&y| buf.cell((0, y)).unwrap().symbol() == "\u{2588}")
        .count() as u16;

    // Large file (1000 lines, 20 visible) — thumb should be small
    let mut buf = ratatui::buffer::Buffer::empty(area);
    render_scrollbar(1000, 20, 0, area, &mut buf, &theme);
    let thumb_count_large: u16 = (0..20)
        .filter(|&y| buf.cell((0, y)).unwrap().symbol() == "\u{2588}")
        .count() as u16;

    assert!(
        thumb_count_small > thumb_count_large,
        "expected larger thumb for smaller file ({thumb_count_small}) \
         vs larger file ({thumb_count_large})"
    );
}

#[test]
fn scrollbar_minimum_thumb_size() {
    let theme = Theme::default();
    let area = Rect::new(0, 0, 1, 20);
    let mut buf = ratatui::buffer::Buffer::empty(area);
    // Very large file — thumb should be at least 1 row
    render_scrollbar(100_000, 20, 0, area, &mut buf, &theme);
    let thumb_count: u16 = (0..20)
        .filter(|&y| buf.cell((0, y)).unwrap().symbol() == "\u{2588}")
        .count() as u16;
    assert!(
        thumb_count >= 1,
        "expected at least 1 row thumb, got {thumb_count}"
    );
}

// --- expand_tabs tests ---

#[test]
fn expand_tabs_no_tabs() {
    let result = expand_tabs("hello", 4);
    assert_eq!(result.text, "hello");
    assert_eq!(result.char_to_col, vec![0, 1, 2, 3, 4, 5]);
}

#[test]
fn expand_tabs_single_tab_at_start() {
    let result = expand_tabs("\thello", 4);
    assert_eq!(result.text, "    hello");
    // \t at char 0 -> col 0, h at char 1 -> col 4
    assert_eq!(result.char_to_col[0], 0); // \t
    assert_eq!(result.char_to_col[1], 4); // h
    assert_eq!(result.char_to_col[6], 9); // sentinel
}

#[test]
fn expand_tabs_tab_after_text() {
    // "ab\tc" with tab_size=4: "ab" takes cols 0,1; tab at col 2 expands to 2 spaces (next stop at 4)
    let result = expand_tabs("ab\tc", 4);
    assert_eq!(result.text, "ab  c");
    assert_eq!(result.char_to_col[0], 0); // a
    assert_eq!(result.char_to_col[1], 1); // b
    assert_eq!(result.char_to_col[2], 2); // \t
    assert_eq!(result.char_to_col[3], 4); // c
}

#[test]
fn expand_tabs_multiple_tabs() {
    let result = expand_tabs("\t\thello", 4);
    assert_eq!(result.text, "        hello");
    assert_eq!(result.char_to_col[0], 0); // first \t
    assert_eq!(result.char_to_col[1], 4); // second \t
    assert_eq!(result.char_to_col[2], 8); // h
}

#[test]
fn expand_tabs_tab_size_2() {
    let result = expand_tabs("\thello", 2);
    assert_eq!(result.text, "  hello");
    assert_eq!(result.char_to_col[1], 2); // h starts at col 2
}

#[test]
fn expand_tabs_empty_string() {
    let result = expand_tabs("", 4);
    assert_eq!(result.text, "");
    assert_eq!(result.char_to_col, vec![0]); // just the sentinel
}

#[test]
fn char_to_display_col_basic() {
    let mapping = vec![0, 4, 8, 9, 10, 11]; // 5 chars + sentinel
    assert_eq!(char_to_display_col(&mapping, 0), 0);
    assert_eq!(char_to_display_col(&mapping, 1), 4);
    assert_eq!(char_to_display_col(&mapping, 5), 11); // sentinel
    assert_eq!(char_to_display_col(&mapping, 100), 11); // beyond -> sentinel
}
