pub mod layout;
pub mod theme;

mod ai_overlay;
mod editor_panel;
mod inlay;
mod overlays;
mod status_bar;
mod terminal_panel;
mod tree_panel;

#[cfg(test)]
mod tests;

use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::widgets::{Block, BorderType, Borders, Paragraph};
use ratatui::Frame;

use axe_core::{AppState, FocusTarget};

use editor_panel::{
    editor_title, gutter_width, render_editor_content, render_startup_screen,
    EDITOR_SCROLLBAR_WIDTH,
};
use layout::LayoutManager;
use overlays::{
    render_code_actions_popup, render_command_palette, render_completion_popup,
    render_confirm_dialog, render_diff_popup, render_file_finder, render_go_to_line,
    render_help_overlay, render_hover_tooltip, render_location_list, render_password_dialog,
    render_project_search, render_rename_input, render_signature_help, render_ssh_host_finder,
};
use status_bar::build_status_bar;
use terminal_panel::{adjust_terminal_rect, render_right_panels, render_terminal_content};
use theme::Theme;
use tree_panel::render_tree_content;

/// Renders every editor split inside `area`, dividing it according to the
/// current orientation. Each split draws its own buffer (or the startup
/// screen if the buffer index is out of range); only the focused split
/// receives the bright cursor/border treatment. The tab bar is shown on
/// the focused split only so other splits use their full height.
pub(crate) fn render_editor_splits(app: &AppState, area: Rect, frame: &mut Frame, theme: &Theme) {
    let splits = app.editor_layout.splits();
    let orientation = app.editor_layout.orientation();
    let focused_split_idx = app.editor_layout.focused_index();
    let editor_is_focused_panel = app.focus == FocusTarget::Editor;

    let rects = layout::split_rects(area, splits.len(), orientation);
    for (i, split) in splits.iter().enumerate() {
        let split_area = rects.get(i).copied().unwrap_or(area);
        let is_focused_split = i == focused_split_idx && editor_is_focused_panel;
        let buffer = app.buffer_manager.buffers().get(split.active_buffer);
        if let Some(buffer) = buffer {
            let tab_bar = if i == focused_split_idx {
                Some((
                    app.buffer_manager.buffers(),
                    app.buffer_manager.active_index(),
                ))
            } else {
                None
            };
            let hints: &[axe_core::InlayHint] = buffer
                .path()
                .and_then(|p| app.inlay_hints.get(p))
                .map(|entry| entry.hints.as_slice())
                .unwrap_or(&[]);
            render_editor_content(
                buffer,
                split_area,
                frame,
                theme,
                is_focused_split,
                if is_focused_split {
                    app.search.as_ref()
                } else {
                    None
                },
                tab_bar,
                hints,
            );
        } else {
            render_startup_screen(split_area, frame, theme, &app.build_version);
        }
    }
}

/// Returns the border style for a panel based on whether it has focus and resize mode.
pub(crate) fn border_style_for(
    focus: &FocusTarget,
    panel: &FocusTarget,
    theme: &Theme,
    resize_active: bool,
) -> Style {
    if resize_active && focus == panel {
        Style::default().fg(theme.resize_border)
    } else if focus == panel {
        Style::default().fg(theme.panel_border_active)
    } else {
        Style::default().fg(theme.panel_border)
    }
}

/// Returns the title style for a panel — bold when focused. Uses resize color in resize mode.
pub(crate) fn title_style_for(
    focus: &FocusTarget,
    panel: &FocusTarget,
    theme: &Theme,
    resize_active: bool,
) -> Style {
    if resize_active && focus == panel {
        Style::default()
            .fg(theme.resize_border)
            .add_modifier(Modifier::BOLD)
    } else if focus == panel {
        Style::default()
            .fg(theme.panel_border_active)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(theme.panel_border)
    }
}

/// Creates a styled panel block with the given title and focus state.
pub(crate) fn panel_block<'a>(
    title: &'a str,
    focus: &FocusTarget,
    panel: &FocusTarget,
    theme: &Theme,
    resize_active: bool,
) -> Block<'a> {
    let panel_style = Style::default().bg(theme.background).fg(theme.foreground);

    Block::default()
        .title(title)
        .title_style(title_style_for(focus, panel, theme, resize_active))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(border_style_for(focus, panel, theme, resize_active))
        .style(panel_style)
}

/// Returns the inner area of the file tree panel, if visible.
///
/// Used by the main loop to sync `AppState::tree_inner_area` each frame
/// for mouse click detection on tree nodes.
pub fn tree_inner_rect(app: &AppState, area: Rect) -> Option<Rect> {
    if !app.show_tree {
        return None;
    }

    let layout_mgr = LayoutManager {
        show_tree: app.show_tree,
        show_terminal: app.show_terminal,
        tree_width_pct: app.tree_width_pct,
        editor_height_pct: app.editor_height_pct,
    };

    if let Some(ref zoomed) = app.zoomed_panel {
        if matches!(zoomed, FocusTarget::Tree) {
            let vertical =
                Layout::vertical([Constraint::Min(0), Constraint::Length(1)]).split(area);
            let block = panel_block(
                " Files (zoomed) ",
                &app.focus,
                &FocusTarget::Tree,
                &Theme::default(),
                false,
            );
            return Some(block.inner(vertical[0]));
        }
        return None;
    }

    let vertical = Layout::vertical([Constraint::Min(0), Constraint::Length(1)]).split(area);
    let main_area = vertical[0];

    let horizontal = Layout::horizontal([
        Constraint::Percentage(layout_mgr.tree_width_pct),
        Constraint::Percentage(100 - layout_mgr.tree_width_pct),
    ])
    .split(main_area);

    let tree_area = horizontal[0];
    let block = panel_block(
        " Files ",
        &app.focus,
        &FocusTarget::Tree,
        &Theme::default(),
        false,
    );
    Some(block.inner(tree_area))
}

/// Returns the inner area of the terminal panel (excluding tab bar), if terminal is visible.
///
/// Used by the main loop to sync PTY size with panel dimensions.
/// Subtracts 1 row for the tab bar when the terminal manager has tabs.
pub fn terminal_inner_rect(app: &AppState, area: Rect) -> Option<Rect> {
    let layout_mgr = LayoutManager {
        show_tree: app.show_tree,
        show_terminal: app.show_terminal,
        tree_width_pct: app.tree_width_pct,
        editor_height_pct: app.editor_height_pct,
    };

    if !layout_mgr.show_terminal {
        return None;
    }

    let has_tabs = app
        .terminal_manager
        .as_ref()
        .is_some_and(|mgr| mgr.has_tabs());

    if let Some(ref zoomed) = app.zoomed_panel {
        if matches!(zoomed, FocusTarget::Terminal(_)) {
            let vertical =
                Layout::vertical([Constraint::Min(0), Constraint::Length(1)]).split(area);
            let block = panel_block(
                " Terminal (zoomed) ",
                &app.focus,
                &FocusTarget::Terminal(0),
                &Theme::default(),
                false,
            );
            let inner = block.inner(vertical[0]);
            return Some(adjust_terminal_rect(inner, has_tabs));
        }
        return None;
    }

    let vertical = Layout::vertical([Constraint::Min(0), Constraint::Length(1)]).split(area);
    let main_area = vertical[0];

    let right_area = if layout_mgr.show_tree {
        let horizontal = Layout::horizontal([
            Constraint::Percentage(layout_mgr.tree_width_pct),
            Constraint::Percentage(100 - layout_mgr.tree_width_pct),
        ])
        .split(main_area);
        horizontal[1]
    } else {
        main_area
    };

    let right_split = Layout::vertical([
        Constraint::Percentage(layout_mgr.editor_height_pct),
        Constraint::Percentage(100 - layout_mgr.editor_height_pct),
    ])
    .split(right_area);

    let term_block = panel_block(
        " Terminal ",
        &app.focus,
        &FocusTarget::Terminal(0),
        &Theme::default(),
        false,
    );
    let inner = term_block.inner(right_split[1]);
    Some(adjust_terminal_rect(inner, has_tabs))
}

/// Returns the outer rect of the terminal panel (including borders), if visible.
///
/// Used by the main loop to poison ratatui's front buffer for only the terminal
/// area, avoiding ghost characters without affecting the editor or tree panels.
pub fn terminal_outer_rect(app: &AppState, area: Rect) -> Option<Rect> {
    let layout_mgr = LayoutManager {
        show_tree: app.show_tree,
        show_terminal: app.show_terminal,
        tree_width_pct: app.tree_width_pct,
        editor_height_pct: app.editor_height_pct,
    };

    if !layout_mgr.show_terminal {
        return None;
    }

    if let Some(ref zoomed) = app.zoomed_panel {
        return if matches!(zoomed, FocusTarget::Terminal(_)) {
            let vertical =
                Layout::vertical([Constraint::Min(0), Constraint::Length(1)]).split(area);
            Some(vertical[0])
        } else {
            None
        };
    }

    let vertical = Layout::vertical([Constraint::Min(0), Constraint::Length(1)]).split(area);
    let main_area = vertical[0];

    let right_area = if layout_mgr.show_tree {
        let horizontal = Layout::horizontal([
            Constraint::Percentage(layout_mgr.tree_width_pct),
            Constraint::Percentage(100 - layout_mgr.tree_width_pct),
        ])
        .split(main_area);
        horizontal[1]
    } else {
        main_area
    };

    let right_split = Layout::vertical([
        Constraint::Percentage(layout_mgr.editor_height_pct),
        Constraint::Percentage(100 - layout_mgr.editor_height_pct),
    ])
    .split(right_area);

    Some(right_split[1])
}

/// Computes the editor content area rect (after borders and gutter).
///
/// Used by main.rs to sync `AppState::editor_inner_area` each frame.
/// Returns the screen rectangle for the editor tab bar, or `None` if the tab
/// bar is not visible (single buffer or editor not shown).
///
/// The tab bar occupies the first row of the editor panel inner area when
/// at least one buffer is open.
pub fn editor_tab_bar_rect(app: &AppState, area: Rect) -> Option<Rect> {
    if app.buffer_manager.buffer_count() == 0 {
        return None;
    }

    let layout_mgr = LayoutManager {
        show_tree: app.show_tree,
        show_terminal: app.show_terminal,
        tree_width_pct: app.tree_width_pct,
        editor_height_pct: app.editor_height_pct,
    };

    // If zoomed to a non-editor panel, editor is not visible.
    if let Some(ref zoomed) = app.zoomed_panel {
        if !matches!(zoomed, FocusTarget::Editor) {
            return None;
        }
        let vertical = Layout::vertical([Constraint::Min(0), Constraint::Length(1)]).split(area);
        let block = panel_block(
            editor_title(app, true),
            &app.focus,
            &FocusTarget::Editor,
            &Theme::default(),
            false,
        );
        let inner = block.inner(vertical[0]);
        if inner.height <= 2 {
            return None;
        }
        return Some(Rect {
            x: inner.x,
            y: inner.y,
            width: inner.width,
            height: 1,
        });
    }

    let vertical = Layout::vertical([Constraint::Min(0), Constraint::Length(1)]).split(area);
    let main_area = vertical[0];

    let right_area = if layout_mgr.show_tree {
        let horizontal = Layout::horizontal([
            Constraint::Percentage(layout_mgr.tree_width_pct),
            Constraint::Percentage(100 - layout_mgr.tree_width_pct),
        ])
        .split(main_area);
        horizontal[1]
    } else {
        main_area
    };

    let editor_outer = if layout_mgr.show_terminal {
        let right_split = Layout::vertical([
            Constraint::Percentage(layout_mgr.editor_height_pct),
            Constraint::Percentage(100 - layout_mgr.editor_height_pct),
        ])
        .split(right_area);
        right_split[0]
    } else {
        right_area
    };

    let block = panel_block(
        editor_title(app, false),
        &app.focus,
        &FocusTarget::Editor,
        &Theme::default(),
        false,
    );
    let inner = block.inner(editor_outer);
    if inner.height <= 2 {
        return None;
    }
    Some(Rect {
        x: inner.x,
        y: inner.y,
        width: inner.width,
        height: 1,
    })
}

/// Computes the terminal tab bar rect in screen coordinates.
///
/// Returns the first row inside the terminal panel border where tab labels are
/// rendered. Returns `None` if the terminal is hidden, has no tabs, or is not
/// visible (e.g., another panel is zoomed).
pub fn terminal_tab_bar_rect(app: &AppState, area: Rect) -> Option<Rect> {
    let has_tabs = app
        .terminal_manager
        .as_ref()
        .is_some_and(|mgr| mgr.has_tabs());
    if !has_tabs {
        return None;
    }

    let layout_mgr = LayoutManager {
        show_tree: app.show_tree,
        show_terminal: app.show_terminal,
        tree_width_pct: app.tree_width_pct,
        editor_height_pct: app.editor_height_pct,
    };

    if !layout_mgr.show_terminal {
        return None;
    }

    if let Some(ref zoomed) = app.zoomed_panel {
        if !matches!(zoomed, FocusTarget::Terminal(_)) {
            return None;
        }
        let vertical = Layout::vertical([Constraint::Min(0), Constraint::Length(1)]).split(area);
        let block = panel_block(
            " Terminal (zoomed) ",
            &app.focus,
            &FocusTarget::Terminal(0),
            &Theme::default(),
            false,
        );
        let inner = block.inner(vertical[0]);
        if inner.height < 2 {
            return None;
        }
        return Some(Rect {
            x: inner.x,
            y: inner.y,
            width: inner.width,
            height: 1,
        });
    }

    let vertical = Layout::vertical([Constraint::Min(0), Constraint::Length(1)]).split(area);
    let main_area = vertical[0];

    let right_area = if layout_mgr.show_tree {
        let horizontal = Layout::horizontal([
            Constraint::Percentage(layout_mgr.tree_width_pct),
            Constraint::Percentage(100 - layout_mgr.tree_width_pct),
        ])
        .split(main_area);
        horizontal[1]
    } else {
        main_area
    };

    let right_split = Layout::vertical([
        Constraint::Percentage(layout_mgr.editor_height_pct),
        Constraint::Percentage(100 - layout_mgr.editor_height_pct),
    ])
    .split(right_area);

    let term_block = panel_block(
        " Terminal ",
        &app.focus,
        &FocusTarget::Terminal(0),
        &Theme::default(),
        false,
    );
    let inner = term_block.inner(right_split[1]);
    if inner.height < 2 {
        return None;
    }
    Some(Rect {
        x: inner.x,
        y: inner.y,
        width: inner.width,
        height: 1,
    })
}

pub fn editor_inner_rect(app: &AppState, area: Rect) -> Option<Rect> {
    let layout_mgr = LayoutManager {
        show_tree: app.show_tree,
        show_terminal: app.show_terminal,
        tree_width_pct: app.tree_width_pct,
        editor_height_pct: app.editor_height_pct,
    };

    // If zoomed to a non-editor panel, editor is not visible.
    if let Some(ref zoomed) = app.zoomed_panel {
        if !matches!(zoomed, FocusTarget::Editor) {
            return None;
        }
        // Editor is zoomed — it fills the main area minus status bar.
        let vertical = Layout::vertical([Constraint::Min(0), Constraint::Length(1)]).split(area);
        let block = panel_block(
            editor_title(app, true),
            &app.focus,
            &FocusTarget::Editor,
            &Theme::default(),
            false,
        );
        let mut inner = block.inner(vertical[0]);
        // Account for tab bar row when at least one buffer is open.
        if app.buffer_manager.buffer_count() >= 1 && inner.height > 2 {
            inner.y += 1;
            inner.height = inner.height.saturating_sub(1);
        }
        let gutter_w = app
            .buffer_manager
            .active_buffer()
            .map(|b| gutter_width(b.line_count()))
            .unwrap_or(3);
        // Subtract scrollbar column from content width.
        let content_w = inner
            .width
            .saturating_sub(gutter_w)
            .saturating_sub(EDITOR_SCROLLBAR_WIDTH);
        return Some(Rect {
            x: inner.x + gutter_w,
            y: inner.y,
            width: content_w,
            height: inner.height,
        });
    }

    let vertical = Layout::vertical([Constraint::Min(0), Constraint::Length(1)]).split(area);
    let main_area = vertical[0];

    let right_area = if layout_mgr.show_tree {
        let horizontal = Layout::horizontal([
            Constraint::Percentage(layout_mgr.tree_width_pct),
            Constraint::Percentage(100 - layout_mgr.tree_width_pct),
        ])
        .split(main_area);
        horizontal[1]
    } else {
        main_area
    };

    let editor_outer = if layout_mgr.show_terminal {
        let right_split = Layout::vertical([
            Constraint::Percentage(layout_mgr.editor_height_pct),
            Constraint::Percentage(100 - layout_mgr.editor_height_pct),
        ])
        .split(right_area);
        right_split[0]
    } else {
        right_area
    };

    let block = panel_block(
        editor_title(app, false),
        &app.focus,
        &FocusTarget::Editor,
        &Theme::default(),
        false,
    );
    let mut inner = block.inner(editor_outer);
    // Account for tab bar row when at least one buffer is open.
    if app.buffer_manager.buffer_count() >= 1 && inner.height > 2 {
        inner.y += 1;
        inner.height = inner.height.saturating_sub(1);
    }
    let gutter_w = app
        .buffer_manager
        .active_buffer()
        .map(|b| gutter_width(b.line_count()))
        .unwrap_or(3);
    // Subtract scrollbar column from content width.
    let content_w = inner
        .width
        .saturating_sub(gutter_w)
        .saturating_sub(EDITOR_SCROLLBAR_WIDTH);
    Some(Rect {
        x: inner.x + gutter_w,
        y: inner.y,
        width: content_w,
        height: inner.height,
    })
}

/// Returns the editor scrollbar area in screen coordinates.
///
/// The scrollbar is a 1-column strip on the right edge of the editor content area,
/// spanning the content rows (below tab bar and search bar, if present).
/// Returns `None` if the editor is not visible.
pub fn editor_scrollbar_rect(app: &AppState, area: Rect) -> Option<Rect> {
    // Get the content area (which already has scrollbar subtracted from its width).
    let content_rect = editor_inner_rect(app, area)?;
    // Scrollbar sits immediately to the right of the content area.
    Some(Rect {
        x: content_rect.x + content_rect.width,
        y: content_rect.y,
        width: EDITOR_SCROLLBAR_WIDTH,
        height: content_rect.height,
    })
}

/// Renders the full IDE interface with conditional panel visibility and a status bar.
pub fn render(app: &AppState, frame: &mut Frame, theme: &Theme) {
    let layout_mgr = LayoutManager {
        show_tree: app.show_tree,
        show_terminal: app.show_terminal,
        tree_width_pct: app.tree_width_pct,
        editor_height_pct: app.editor_height_pct,
    };
    let area = frame.area();

    // Split vertically: main area + status bar
    let vertical = Layout::vertical([Constraint::Min(0), Constraint::Length(1)]).split(area);
    let main_area = vertical[0];
    let status_area = vertical[1];

    let resize_active = app.resize_mode.active;

    if let Some(ref zoomed) = app.zoomed_panel {
        let (title, panel_target) = match zoomed {
            FocusTarget::Tree => (" Files (zoomed) ", FocusTarget::Tree),
            FocusTarget::Editor => (editor_title(app, true), FocusTarget::Editor),
            FocusTarget::Terminal(id) => (" Terminal (zoomed) ", FocusTarget::Terminal(*id)),
        };
        let block = panel_block(title, &app.focus, &panel_target, theme, resize_active);
        let inner = block.inner(main_area);
        frame.render_widget(block, main_area);
        match zoomed {
            FocusTarget::Tree => {
                if let Some(ref tree) = app.file_tree {
                    render_tree_content(
                        tree,
                        inner,
                        frame,
                        theme,
                        &app.git_modified_files,
                        &app.git_dirty_dirs,
                    );
                }
            }
            FocusTarget::Terminal(_) => {
                if let Some(ref mgr) = app.terminal_manager {
                    render_terminal_content(mgr, inner, frame, theme);
                }
            }
            FocusTarget::Editor => {
                render_editor_splits(app, inner, frame, theme);
            }
        }
    } else if layout_mgr.show_tree {
        let horizontal = Layout::horizontal([
            Constraint::Percentage(layout_mgr.tree_width_pct),
            Constraint::Percentage(100 - layout_mgr.tree_width_pct),
        ])
        .split(main_area);

        let tree_area = horizontal[0];
        let right_area = horizontal[1];

        let tree_block = panel_block(
            " Files ",
            &app.focus,
            &FocusTarget::Tree,
            theme,
            resize_active,
        );
        let tree_inner = tree_block.inner(tree_area);
        frame.render_widget(tree_block, tree_area);
        if let Some(ref tree) = app.file_tree {
            render_tree_content(
                tree,
                tree_inner,
                frame,
                theme,
                &app.git_modified_files,
                &app.git_dirty_dirs,
            );
        }

        render_right_panels(app, frame, right_area, &layout_mgr, theme, resize_active);
    } else {
        render_right_panels(app, frame, main_area, &layout_mgr, theme, resize_active);
    }

    // Status bar with hotkey hints
    let status_line = build_status_bar(app, theme, status_area.width);
    let status_bar = Paragraph::new(status_line).style(
        Style::default()
            .bg(theme.status_bar_bg)
            .fg(theme.status_bar_fg),
    );
    frame.render_widget(status_bar, status_area);

    // Completion popup (non-modal, rendered below modal overlays).
    if let Some(ref comp) = app.completion {
        if let Some(buffer) = app.buffer_manager.active_buffer() {
            render_completion_popup(comp, buffer, app, frame, theme);
        }
    }

    // Hover tooltip (non-modal, positioned near cursor).
    if let Some(ref hover) = app.hover_info {
        if let Some(buffer) = app.buffer_manager.active_buffer() {
            render_hover_tooltip(hover, buffer, app, frame, theme);
        }
    }

    // Signature help popup (non-modal, drawn above completion so it wins
    // the z-order when both are open).
    if let Some(ref sig) = app.signature_help {
        if let Some(buffer) = app.buffer_manager.active_buffer() {
            render_signature_help(sig, buffer, app, frame, theme);
        }
    }

    // Code actions picker — non-modal, anchored to the cursor.
    if let Some(ref actions) = app.code_actions {
        if let Some(buffer) = app.buffer_manager.active_buffer() {
            render_code_actions_popup(actions, buffer, app, frame, theme);
        }
    }

    // Inline rename input — drawn last so it sits above other popups and
    // captures visual focus while the user types the new name.
    if let Some(ref rename) = app.rename {
        if let Some(buffer) = app.buffer_manager.active_buffer() {
            render_rename_input(rename, buffer, app, frame, theme);
        }
    }

    // Diff hunk popup (modal, positioned near changed lines).
    if let Some(ref popup) = app.diff_popup {
        if let Some(buffer) = app.buffer_manager.active_buffer() {
            render_diff_popup(popup, buffer, app, frame, theme);
        }
    }

    // AI chat overlay — drawn before other modal overlays so things like the
    // "kill current session?" confirm dialog render on top of it.
    crate::ai_overlay::render_ai_overlay(frame, frame.area(), &app.ai_overlay, theme);

    // Overlays (on top of everything)
    if let Some(ref dialog) = app.confirm_dialog {
        render_confirm_dialog(dialog, frame, theme);
    } else if let Some(ref dialog) = app.password_dialog {
        render_password_dialog(dialog, frame, theme);
    } else if let Some(ref go_to_line) = app.go_to_line {
        render_go_to_line(go_to_line, frame, theme);
    } else if let Some(ref palette) = app.command_palette {
        render_command_palette(palette, frame, theme);
    } else if let Some(ref finder) = app.ssh_host_finder {
        render_ssh_host_finder(finder, frame, theme);
    } else if let Some(ref search) = app.project_search {
        render_project_search(search, frame, theme);
    } else if let Some(ref loc_list) = app.location_list {
        render_location_list(loc_list, frame, theme);
    } else if let Some(ref finder) = app.file_finder {
        render_file_finder(finder, frame, theme);
    } else if app.show_help {
        render_help_overlay(frame, theme);
    }
}
