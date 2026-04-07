use std::time::Instant;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};

use alacritty_terminal::index::Direction;
use alacritty_terminal::selection::SelectionType;

use axe_tree::NodeKind;

use super::layout::{MAX_PANEL_PCT, MIN_PANEL_PCT};
use super::types::DOUBLE_CLICK_THRESHOLD;
use super::{AppState, ConfirmButton, DragBorder, FocusTarget};
use crate::command::Command;

/// Number of lines to scroll per mouse wheel tick.
const MOUSE_SCROLL_LINES: i32 = 3;
/// Number of columns to scroll per Shift+mouse wheel tick.
const MOUSE_SCROLL_COLS: i32 = 6;

impl AppState {
    /// Processes a key event by resolving it through the keymap and executing
    /// the resulting command, if any.
    ///
    /// When resize mode is active, arrow keys and special keys are intercepted
    /// before normal dispatch. When a help overlay is open, only Quit, ShowHelp,
    /// and CloseOverlay commands are processed; all other keys are consumed silently.
    pub fn handle_key_event(&mut self, key: KeyEvent) {
        // Confirmation dialog intercepts all keys.
        if let Some(ref mut dialog) = self.confirm_dialog {
            match key.code {
                KeyCode::Left => dialog.selected = ConfirmButton::Yes,
                KeyCode::Right => dialog.selected = ConfirmButton::No,
                KeyCode::Enter => {
                    let cmd = match dialog.selected {
                        ConfirmButton::Yes => Some(dialog.on_confirm.clone()),
                        ConfirmButton::No => dialog.on_cancel.clone(),
                    };
                    self.confirm_dialog = None;
                    if let Some(cmd) = cmd {
                        self.execute(cmd);
                    }
                }
                KeyCode::Esc => {
                    let cmd = dialog.on_cancel.clone();
                    self.confirm_dialog = None;
                    if let Some(cmd) = cmd {
                        self.execute(cmd);
                    }
                }
                _ => {} // Consume all other keys
            }
            return;
        }

        // Go to Line dialog intercepts all keys when open.
        if let Some(ref mut dialog) = self.go_to_line {
            match key.code {
                KeyCode::Esc => {
                    self.go_to_line = None;
                }
                KeyCode::Enter => {
                    if let Some(line) = dialog.parse_line() {
                        let (h, w) = self.editor_viewport();
                        if let Some(buf) = self.buffer_manager.active_buffer_mut() {
                            buf.cursor.row = line;
                            buf.cursor.col = 0;
                            buf.cursor.desired_col = 0;
                            buf.clear_selection();
                            buf.ensure_cursor_visible(h, w);
                        }
                    }
                    self.go_to_line = None;
                }
                KeyCode::Backspace => {
                    dialog.input_backspace();
                }
                KeyCode::Char(c) => {
                    dialog.input_char(c);
                }
                _ => {} // Consume all other keys
            }
            return;
        }

        // Command palette overlay intercepts all keys when open.
        if let Some(ref mut palette) = self.command_palette {
            match key.code {
                KeyCode::Esc => {
                    self.command_palette = None;
                }
                KeyCode::Enter => {
                    if let Some(cmd) = palette.selected_command().cloned() {
                        self.command_palette = None;
                        self.execute(cmd);
                    }
                }
                KeyCode::Up => palette.move_up(),
                KeyCode::Down => palette.move_down(),
                KeyCode::Backspace => palette.input_backspace(),
                KeyCode::Char(c) => palette.input_char(c),
                _ => {}
            }
            return;
        }

        // SSH host finder overlay intercepts all keys when open.
        if let Some(ref mut finder) = self.ssh_host_finder {
            match key.code {
                KeyCode::Esc => {
                    self.ssh_host_finder = None;
                }
                KeyCode::Enter => {
                    if let Some(host) = finder.selected_host().cloned() {
                        self.ssh_host_finder = None;
                        self.spawn_ssh_tab(host);
                    }
                }
                KeyCode::Up => finder.move_up(),
                KeyCode::Down => finder.move_down(),
                KeyCode::PageUp => {
                    finder.move_page_up(crate::ssh_host_finder::SSH_FINDER_PAGE_SIZE)
                }
                KeyCode::PageDown => {
                    finder.move_page_down(crate::ssh_host_finder::SSH_FINDER_PAGE_SIZE)
                }
                KeyCode::Backspace => finder.input_backspace(),
                KeyCode::Char(c) => finder.input_char(c),
                _ => {}
            }
            return;
        }

        // Project search overlay intercepts all keys when open.
        if let Some(ref mut search) = self.project_search {
            match (key.modifiers, key.code) {
                (_, KeyCode::Esc) => {
                    search.cancel_search();
                    self.project_search = None;
                }
                (_, KeyCode::Enter) => {
                    if let Some(result) = search.selected_result() {
                        let path = result.absolute_path.clone();
                        let line = result.line_number.saturating_sub(1);
                        self.project_search = None;
                        self.execute(Command::OpenFile(path));
                        // Jump to the matching line.
                        if let Some(buf) = self.buffer_manager.active_buffer_mut() {
                            buf.cursor.row = line;
                            buf.cursor.col = 0;
                        }
                    }
                }
                (_, KeyCode::Up) => search.move_up(),
                (_, KeyCode::Down) => search.move_down(),
                (_, KeyCode::Tab) => search.cycle_field(),
                (m, KeyCode::Char('c')) if m.contains(KeyModifiers::ALT) => {
                    search.toggle_case();
                    if let Some(ref root) = self.project_root {
                        let root = root.clone();
                        // Re-borrow after root clone.
                        if let Some(ref mut search) = self.project_search {
                            search.start_search(&root);
                        }
                    }
                }
                (m, KeyCode::Char('r')) if m.contains(KeyModifiers::ALT) => {
                    search.toggle_regex();
                    if let Some(ref root) = self.project_root {
                        let root = root.clone();
                        if let Some(ref mut search) = self.project_search {
                            search.start_search(&root);
                        }
                    }
                }
                (_, KeyCode::Backspace) => {
                    search.input_backspace();
                    if let Some(ref root) = self.project_root {
                        let root = root.clone();
                        if let Some(ref mut search) = self.project_search {
                            search.start_search(&root);
                        }
                    }
                }
                (_, KeyCode::Char(c)) => {
                    search.input_char(c);
                    if let Some(ref root) = self.project_root {
                        let root = root.clone();
                        if let Some(ref mut search) = self.project_search {
                            search.start_search(&root);
                        }
                    }
                }
                _ => {}
            }
            return;
        }

        // Hover tooltip is dismissed by any key press.
        if self.hover_info.is_some() {
            self.hover_info = None;
            // Esc only dismisses hover (don't propagate to close other overlays).
            if key.code == KeyCode::Esc {
                return;
            }
        }

        // Location list overlay intercepts all keys when open.
        if let Some(ref mut loc_list) = self.location_list {
            match key.code {
                KeyCode::Esc => {
                    self.location_list = None;
                }
                KeyCode::Enter => {
                    if let Some(item) = loc_list.selected_item() {
                        let path = item.path.clone();
                        let line = item.line;
                        let col = item.col;
                        self.location_list = None;
                        self.execute(Command::OpenFile(path));
                        let (h, w) = self.editor_viewport();
                        if let Some(buf) = self.buffer_manager.active_buffer_mut() {
                            buf.cursor.row = line;
                            buf.cursor.col = col;
                            buf.ensure_cursor_visible(h, w);
                        }
                    }
                }
                KeyCode::Up => loc_list.move_up(),
                KeyCode::Down => loc_list.move_down(),
                _ => {}
            }
            return;
        }

        // File finder overlay intercepts all keys when open.
        if let Some(ref mut finder) = self.file_finder {
            match key.code {
                KeyCode::Esc => {
                    self.file_finder = None;
                }
                KeyCode::Enter => {
                    if let Some(path) = finder.selected_path().map(|p| p.to_path_buf()) {
                        self.file_finder = None;
                        self.execute(Command::OpenFile(path));
                    }
                }
                KeyCode::Up => finder.move_up(),
                KeyCode::Down => finder.move_down(),
                KeyCode::PageUp => finder.move_page_up(crate::file_finder::FILE_FINDER_PAGE_SIZE),
                KeyCode::PageDown => {
                    finder.move_page_down(crate::file_finder::FILE_FINDER_PAGE_SIZE)
                }
                KeyCode::Backspace => finder.input_backspace(),
                KeyCode::Char(c) => finder.input_char(c),
                _ => {}
            }
            return;
        }

        // Resize mode intercepts keys before normal keymap resolution.
        if self.resize_mode.active {
            let cmd = match (key.modifiers, key.code) {
                (KeyModifiers::NONE, KeyCode::Left) => Some(Command::ResizeLeft),
                (KeyModifiers::NONE, KeyCode::Right) => Some(Command::ResizeRight),
                (KeyModifiers::NONE, KeyCode::Up) => Some(Command::ResizeUp),
                (KeyModifiers::NONE, KeyCode::Down) => Some(Command::ResizeDown),
                (KeyModifiers::NONE, KeyCode::Char('=')) => Some(Command::EqualizeLayout),
                (KeyModifiers::NONE, KeyCode::Esc) | (KeyModifiers::NONE, KeyCode::Enter) => {
                    Some(Command::ExitResizeMode)
                }
                (KeyModifiers::CONTROL, KeyCode::Char('q')) => Some(Command::RequestQuit),
                _ => None, // All other keys consumed silently
            };
            if let Some(cmd) = cmd {
                self.execute(cmd);
            }
            return;
        }

        // Search bar active: intercept keys for search input before editor keys.
        if self.search.is_some() && self.focus == FocusTarget::Editor && !self.show_help {
            // Ctrl+Alt+Enter: Replace All (works from either field).
            if key.modifiers == (KeyModifiers::CONTROL | KeyModifiers::ALT)
                && key.code == KeyCode::Enter
            {
                self.execute(Command::ReplaceAll);
                return;
            }

            // Determine active field for field-specific handling.
            let replace_visible = self.search.as_ref().is_some_and(|s| s.replace_visible);
            let in_replace_field = replace_visible
                && self
                    .search
                    .as_ref()
                    .is_some_and(|s| s.active_field == crate::search::SearchField::Replace);

            // Tab / BackTab: toggle between Find and Replace fields.
            if replace_visible
                && matches!(key.code, KeyCode::Tab | KeyCode::BackTab)
                && (key.modifiers == KeyModifiers::NONE || key.modifiers == KeyModifiers::SHIFT)
            {
                if let Some(ref mut search) = self.search {
                    search.toggle_field();
                }
                return;
            }

            // Common toggles that work from either field.
            match (key.modifiers, key.code) {
                (KeyModifiers::NONE, KeyCode::Esc) => {
                    self.execute(Command::SearchClose);
                    return;
                }
                (KeyModifiers::ALT, KeyCode::Char('c')) => {
                    self.execute(Command::SearchToggleCase);
                    return;
                }
                (KeyModifiers::ALT, KeyCode::Char('r')) => {
                    self.execute(Command::SearchToggleRegex);
                    return;
                }
                _ => {}
            }

            if in_replace_field {
                // Replace field input handling.
                match (key.modifiers, key.code) {
                    (KeyModifiers::NONE, KeyCode::Enter) => {
                        self.execute(Command::ReplaceNext);
                        return;
                    }
                    (KeyModifiers::NONE, KeyCode::Backspace) => {
                        if let Some(ref mut search) = self.search {
                            search.replace_input_backspace();
                        }
                        return;
                    }
                    (KeyModifiers::NONE, KeyCode::Char(c))
                    | (KeyModifiers::SHIFT, KeyCode::Char(c)) => {
                        if let Some(ref mut search) = self.search {
                            search.replace_input_char(c);
                        }
                        return;
                    }
                    _ => {
                        // Let Ctrl+F, Ctrl+Q, etc. fall through to global keymap.
                    }
                }
            } else {
                // Find field input handling (original behavior).
                match (key.modifiers, key.code) {
                    (KeyModifiers::NONE, KeyCode::Enter) => {
                        self.execute(Command::SearchNextMatch);
                        return;
                    }
                    (KeyModifiers::SHIFT, KeyCode::Enter) => {
                        self.execute(Command::SearchPrevMatch);
                        return;
                    }
                    (KeyModifiers::NONE, KeyCode::Backspace) => {
                        if let Some(ref mut search) = self.search {
                            if let Some(buf) = self.buffer_manager.active_buffer() {
                                search.input_backspace(buf);
                            }
                        }
                        return;
                    }
                    (KeyModifiers::NONE, KeyCode::Char(c))
                    | (KeyModifiers::SHIFT, KeyCode::Char(c)) => {
                        if let Some(ref mut search) = self.search {
                            if let Some(buf) = self.buffer_manager.active_buffer() {
                                search.input_char(c, buf);
                            }
                        }
                        return;
                    }
                    _ => {
                        // Let Ctrl+F, Ctrl+Q, etc. fall through to global keymap.
                    }
                }
            }
        }

        // Completion popup interception (non-modal: typing falls through).
        if self.completion.is_some() && self.focus == FocusTarget::Editor {
            match (key.modifiers, key.code) {
                (KeyModifiers::NONE, KeyCode::Esc) => {
                    self.completion = None;
                    return;
                }
                (KeyModifiers::NONE, KeyCode::Enter) | (KeyModifiers::NONE, KeyCode::Tab) => {
                    self.execute(Command::AcceptCompletion);
                    return;
                }
                (KeyModifiers::NONE, KeyCode::Up) => {
                    if let Some(ref mut comp) = self.completion {
                        comp.move_up();
                    }
                    return;
                }
                (KeyModifiers::NONE, KeyCode::Down) => {
                    if let Some(ref mut comp) = self.completion {
                        comp.move_down();
                    }
                    return;
                }
                _ => {
                    // All other keys fall through to editor handling.
                    // Completion will be updated after the edit.
                }
            }
        }

        // Editor-focus key interception: cursor movement and navigation.
        if self.focus == FocusTarget::Editor && !self.show_help {
            let editor_cmd = match (key.modifiers, key.code) {
                (KeyModifiers::NONE, KeyCode::Up) => Some(Command::EditorUp),
                (KeyModifiers::NONE, KeyCode::Down) => Some(Command::EditorDown),
                (KeyModifiers::NONE, KeyCode::Left) => Some(Command::EditorLeft),
                (KeyModifiers::NONE, KeyCode::Right) => Some(Command::EditorRight),
                (KeyModifiers::NONE, KeyCode::Home) => Some(Command::EditorHome),
                (KeyModifiers::NONE, KeyCode::End) => Some(Command::EditorEnd),
                (KeyModifiers::CONTROL, KeyCode::Home) => Some(Command::EditorFileStart),
                (KeyModifiers::CONTROL, KeyCode::End) => Some(Command::EditorFileEnd),
                (KeyModifiers::NONE, KeyCode::PageUp) => Some(Command::EditorPageUp),
                (KeyModifiers::NONE, KeyCode::PageDown) => Some(Command::EditorPageDown),
                (KeyModifiers::CONTROL, KeyCode::Left) => Some(Command::EditorWordLeft),
                (KeyModifiers::CONTROL, KeyCode::Right) => Some(Command::EditorWordRight),
                // Selection movement: Shift+Arrow extends selection.
                (KeyModifiers::SHIFT, KeyCode::Up) => Some(Command::EditorSelectUp),
                (KeyModifiers::SHIFT, KeyCode::Down) => Some(Command::EditorSelectDown),
                (KeyModifiers::SHIFT, KeyCode::Left) => Some(Command::EditorSelectLeft),
                (KeyModifiers::SHIFT, KeyCode::Right) => Some(Command::EditorSelectRight),
                (KeyModifiers::SHIFT, KeyCode::Home) => Some(Command::EditorSelectHome),
                (KeyModifiers::SHIFT, KeyCode::End) => Some(Command::EditorSelectEnd),
                (m, KeyCode::Home) if m == KeyModifiers::CONTROL | KeyModifiers::SHIFT => {
                    Some(Command::EditorSelectFileStart)
                }
                (m, KeyCode::End) if m == KeyModifiers::CONTROL | KeyModifiers::SHIFT => {
                    Some(Command::EditorSelectFileEnd)
                }
                (m, KeyCode::Left) if m == KeyModifiers::CONTROL | KeyModifiers::SHIFT => {
                    Some(Command::EditorSelectWordLeft)
                }
                (m, KeyCode::Right) if m == KeyModifiers::CONTROL | KeyModifiers::SHIFT => {
                    Some(Command::EditorSelectWordRight)
                }
                (KeyModifiers::NONE, KeyCode::Backspace) => Some(Command::EditorBackspace),
                (KeyModifiers::NONE, KeyCode::Delete) => Some(Command::EditorDelete),
                (KeyModifiers::NONE, KeyCode::Enter) => Some(Command::EditorNewline),
                (KeyModifiers::NONE, KeyCode::Tab) => Some(Command::EditorTab),
                (KeyModifiers::NONE, KeyCode::Char(c)) => Some(Command::EditorInsertChar(c)),
                (KeyModifiers::SHIFT, KeyCode::Char(c)) => Some(Command::EditorInsertChar(c)),
                _ => None,
            };
            if let Some(cmd) = editor_cmd {
                self.execute(cmd);
                return;
            }
            // Fall through to global keymap for Ctrl+Q, Tab, etc.
        }

        // Tree-focus key interception: handle active actions, navigation, and file operations.
        if self.focus == FocusTarget::Tree && !self.show_help {
            // Layer 1: Active action input handling -- consumes ALL keys while active.
            if let Some(ref mut tree) = self.file_tree {
                if tree.is_action_active() {
                    match tree.action().clone() {
                        axe_tree::TreeAction::ConfirmDelete { .. } => {
                            // Handled by the unified confirm dialog above; should not reach here.
                        }
                        axe_tree::TreeAction::Creating { .. }
                        | axe_tree::TreeAction::Renaming { .. } => match key.code {
                            KeyCode::Enter => {
                                let _ = tree.confirm_action();
                            }
                            KeyCode::Esc => {
                                tree.cancel_action();
                            }
                            KeyCode::Backspace => {
                                tree.input_backspace();
                            }
                            KeyCode::Char(c) => {
                                tree.input_char(c);
                            }
                            _ => {}
                        },
                        axe_tree::TreeAction::Idle => {}
                    }
                    return;
                }
            }

            // Layer 2: Navigation and file operation keys.
            let tree_cmd = match (key.modifiers, key.code) {
                (KeyModifiers::NONE, KeyCode::Up) => Some(Command::TreeUp),
                (KeyModifiers::NONE, KeyCode::Down) => Some(Command::TreeDown),
                (KeyModifiers::NONE, KeyCode::Enter) => Some(Command::TreeToggle),
                (KeyModifiers::SHIFT, KeyCode::Left) => Some(Command::TreeScrollLeft),
                (KeyModifiers::SHIFT, KeyCode::Right) => Some(Command::TreeScrollRight),
                (KeyModifiers::NONE, KeyCode::Right) => Some(Command::TreeExpand),
                (KeyModifiers::NONE, KeyCode::Left) => Some(Command::TreeCollapseOrParent),
                (KeyModifiers::NONE, KeyCode::Home) => Some(Command::TreeHome),
                (KeyModifiers::NONE, KeyCode::End) => Some(Command::TreeEnd),
                (KeyModifiers::NONE, KeyCode::Char('n')) => Some(Command::TreeCreateFile),
                (KeyModifiers::SHIFT, KeyCode::Char('N')) => Some(Command::TreeCreateDir),
                (KeyModifiers::NONE, KeyCode::Char('r')) => Some(Command::TreeRename),
                (KeyModifiers::NONE, KeyCode::Char('d')) => Some(Command::TreeDelete),
                _ => None,
            };
            if let Some(cmd) = tree_cmd {
                self.execute(cmd);
                return;
            }
            // Fall through to global keymap for Ctrl+Q, Tab, etc.
        }

        // Terminal-focus key interception: only specific global bindings are intercepted,
        // everything else is forwarded to the PTY as raw bytes.
        // CloseOverlay (Esc) is NOT intercepted here -- shell needs Esc for vi mode,
        // completion cancel, etc. Also prevents SGR mouse sequence splitting: if crossterm
        // splits a mouse escape, the leading Esc would be consumed while `[<65;...M` would
        // leak into the PTY as visible text.
        if matches!(self.focus, FocusTarget::Terminal(_)) && !self.show_help {
            if let Some(cmd) = self.keymap.resolve(&key) {
                if cmd == Command::CloseOverlay {
                    // Esc with no overlay open -- forward to PTY.
                    self.write_terminal_input(&key);
                } else {
                    self.execute(cmd);
                }
            } else {
                self.write_terminal_input(&key);
            }
            return;
        }

        if let Some(cmd) = self.keymap.resolve(&key) {
            if self.show_help {
                match cmd {
                    Command::Quit
                    | Command::RequestQuit
                    | Command::ShowHelp
                    | Command::CloseOverlay => {
                        self.execute(cmd);
                    }
                    _ => {}
                }
            } else {
                self.execute(cmd);
            }
        }
    }

    /// Processes a mouse event for panel border drag-resizing.
    ///
    /// Mouse drag works without entering resize mode -- it's always available.
    /// The caller must provide the current screen dimensions so border positions
    /// can be computed from stored percentages.
    pub fn handle_mouse_event(&mut self, mouse: MouseEvent, screen_width: u16, screen_height: u16) {
        /// Border detection tolerance in cells.
        const BORDER_TOLERANCE: u16 = 1;
        /// Status bar height in rows.
        const STATUS_BAR_HEIGHT: u16 = 1;

        let main_height = screen_height.saturating_sub(STATUS_BAR_HEIGHT);

        match mouse.kind {
            MouseEventKind::Down(MouseButton::Left) => {
                let col = mouse.column;
                let row = mouse.row;

                // Tab bar click takes priority -- its row overlaps with border tolerance.
                if self.show_terminal {
                    if let Some(hit) = self.tab_bar_hit(col, row) {
                        match hit {
                            axe_terminal::TabBarHit::Tab(idx) => {
                                self.activate_terminal_tab(idx);
                            }
                            axe_terminal::TabBarHit::PlusButton => {
                                if let Some(ref mgr) = self.terminal_manager {
                                    if !mgr.is_at_tab_limit() {
                                        self.execute(Command::NewTerminalTab);
                                    }
                                }
                            }
                        }
                        return;
                    }
                }

                // Check vertical border (tree/editor boundary)
                if self.show_tree {
                    let border_x =
                        (u32::from(screen_width) * u32::from(self.tree_width_pct) / 100) as u16;
                    if col.abs_diff(border_x) <= BORDER_TOLERANCE && row < main_height {
                        self.mouse_drag.border = Some(DragBorder::Vertical);
                        return;
                    }
                }

                // Check horizontal border (editor/terminal boundary)
                if self.show_terminal && self.show_tree {
                    let right_x =
                        (u32::from(screen_width) * u32::from(self.tree_width_pct) / 100) as u16;
                    let right_height = main_height;
                    let border_y =
                        (u32::from(right_height) * u32::from(self.editor_height_pct) / 100) as u16;
                    if row.abs_diff(border_y) <= BORDER_TOLERANCE && col >= right_x {
                        self.mouse_drag.border = Some(DragBorder::Horizontal);
                        return;
                    }
                } else if self.show_terminal {
                    // Tree hidden: horizontal border spans entire width
                    let border_y =
                        (u32::from(main_height) * u32::from(self.editor_height_pct) / 100) as u16;
                    if row.abs_diff(border_y) <= BORDER_TOLERANCE {
                        self.mouse_drag.border = Some(DragBorder::Horizontal);
                        return;
                    }
                }

                // Editor tab bar click: switch to the clicked buffer tab.
                if let Some((tx, ty, tw, _th)) = self.editor_tab_bar_area {
                    if row == ty && col >= tx && col < tx + tw {
                        if let Some(idx) = self.editor_tab_index_at_col(col - tx) {
                            self.execute(Command::ActivateBuffer(idx));
                            self.focus = FocusTarget::Editor;
                            return;
                        }
                    }
                }

                // IMPACT ANALYSIS — Tree item mouse click (single/double)
                // Parents: MouseEvent from crossterm, routed through handle_mouse_event.
                // Children: FileTree::select() changes selection,
                //           Single click on file -> PreviewFile (preview buffer),
                //           Double click on file -> OpenFile (permanent buffer),
                //           Click on directory -> toggle expand/collapse.
                // Siblings: tree_inner_area (must be set by main.rs each frame),
                //           last_tree_click (tracks timing for double-click detection),
                //           TreeAction (active rename/create should not be interrupted).
                // Risk: None -- select + toggle/open are safe, idempotent operations.

                // Tree item click -- select and preview/open/toggle.
                if let Some(node_idx) = self.screen_to_tree_node_index(col, row) {
                    // Detect double-click: same node within threshold.
                    let is_double_click = self.last_tree_click.is_some_and(|(t, idx)| {
                        idx == node_idx && t.elapsed() < DOUBLE_CLICK_THRESHOLD
                    });
                    self.last_tree_click = Some((Instant::now(), node_idx));

                    if let Some(ref mut tree) = self.file_tree {
                        tree.select(node_idx);
                        if let Some(node) = tree.selected_node() {
                            match node.kind {
                                NodeKind::File { .. } => {
                                    let path = node.path.clone();
                                    if is_double_click {
                                        self.execute(Command::OpenFile(path));
                                    } else {
                                        self.execute(Command::PreviewFile(path));
                                    }
                                }
                                NodeKind::Directory { .. } => {
                                    if let Err(e) = tree.toggle() {
                                        log::warn!("Failed to toggle directory: {e}");
                                    }
                                }
                                NodeKind::Symlink { .. } => {}
                            }
                        }
                    }
                    self.focus = FocusTarget::Tree;
                    return;
                }

                // Editor scrollbar click -- scroll to clicked position.
                if self.scrollbar_hit(col, row) {
                    self.scrollbar_jump_to(row);
                    self.scrollbar_dragging = true;
                    self.focus = FocusTarget::Editor;
                    return;
                }

                // IMPACT ANALYSIS — Editor mouse text selection (Down/Drag/Up)
                // Parents: MouseEvent from crossterm, routed through handle_mouse_event.
                // Children: EditorBuffer cursor/selection state.
                // Siblings: mouse_drag.border (mutually exclusive, checked first),
                //           editor_inner_area must be kept in sync by main.rs each frame.
                // Risk: editor_selecting flag must be cleared on Up to avoid stale drag state.

                // Check if click is in editor content area -- multi-click detection.
                if let Some((erow, ecol)) = self.screen_to_editor_pos(col, row) {
                    let now = Instant::now();
                    let click_count =
                        self.editor_click_state
                            .register(now, erow, ecol, DOUBLE_CLICK_THRESHOLD);

                    if let Some(buf) = self.buffer_manager.active_buffer_mut() {
                        match click_count {
                            1 => {
                                // Single click: position cursor, clear selection.
                                buf.clear_selection();
                                buf.cursor.row = erow;
                                buf.cursor.col = ecol;
                                buf.cursor.desired_col = ecol;
                            }
                            2 => {
                                // Double-click: select word at cursor.
                                buf.clear_selection();
                                buf.cursor.row = erow;
                                buf.cursor.col = ecol;
                                buf.select_word_at_cursor();
                            }
                            _ => {
                                // Triple-click: select entire line.
                                buf.cursor.row = erow;
                                buf.select_line_at_cursor();
                            }
                        }
                    }
                    // Only enable drag selection on single click.
                    self.editor_selecting = click_count == 1;
                    self.focus = FocusTarget::Editor;
                    return;
                }

                // IMPACT ANALYSIS — Terminal mouse text selection (Down/Drag/Up)
                // Parents: MouseEvent from crossterm, routed through handle_mouse_event.
                // Children: terminal_manager selection state, system clipboard (on drag release).
                // Siblings: mouse_drag.border (panel border resize -- mutually exclusive, border check
                //           runs first and returns early), tab_bar_hit (also checked before selection).
                //           terminal_grid_area must be kept in sync by main.rs each frame.
                // Risk: terminal_selecting flag must be cleared on Up to avoid stale drag state.

                // Check if click is in terminal grid area -- multi-click detection.
                if let Some(point) = self.screen_to_terminal_point(col, row) {
                    let grid_row = point.line.0 as usize;
                    let grid_col = point.column.0;
                    let now = Instant::now();
                    let click_count = self.terminal_click_state.register(
                        now,
                        grid_row,
                        grid_col,
                        DOUBLE_CLICK_THRESHOLD,
                    );

                    if let Some(ref mut mgr) = self.terminal_manager {
                        mgr.clear_selection_active();
                        let selection_type = match click_count {
                            1 => SelectionType::Simple,
                            2 => SelectionType::Semantic,
                            _ => SelectionType::Lines,
                        };
                        mgr.start_selection_active(selection_type, point, Direction::Left);
                    }
                    // Only enable drag selection on single click.
                    self.terminal_selecting = click_count == 1;
                    self.terminal_select_start = Some((col, row));
                    self.focus = FocusTarget::Terminal(0);
                    return;
                }

                // No border, tab bar, or terminal grid hit -- focus the clicked panel.
                if row < main_height {
                    self.focus = self.panel_at(col, row, screen_width, main_height);
                }
            }
            MouseEventKind::Drag(MouseButton::Left) => {
                // Editor scrollbar drag -- update scroll position.
                if self.scrollbar_dragging {
                    self.scrollbar_jump_to(mouse.row);
                    return;
                }

                // Editor text selection drag.
                if self.editor_selecting {
                    // Clamp mouse to editor content area.
                    let pos = if let Some((ex, ey, ew, eh)) = self.editor_inner_area {
                        let clamped_col = mouse.column.clamp(ex, ex + ew.saturating_sub(1));
                        let clamped_row = mouse.row.clamp(ey, ey + eh.saturating_sub(1));
                        self.screen_to_editor_pos(clamped_col, clamped_row)
                    } else {
                        None
                    };
                    if let Some((erow, ecol)) = pos {
                        if let Some(buf) = self.buffer_manager.active_buffer_mut() {
                            buf.start_or_extend_selection();
                            buf.cursor.row = erow;
                            buf.cursor.col = ecol;
                            buf.cursor.desired_col = ecol;
                        }
                    }
                    return;
                }

                // Terminal text selection drag.
                if self.terminal_selecting {
                    // Clamp coordinates to the terminal grid area.
                    let point = if let Some((gx, gy, gw, gh)) = self.terminal_grid_area {
                        let clamped_col = mouse.column.clamp(gx, gx + gw.saturating_sub(1));
                        let clamped_row = mouse.row.clamp(gy, gy + gh.saturating_sub(1));
                        self.screen_to_terminal_point(clamped_col, clamped_row)
                    } else {
                        None
                    };
                    if let Some(point) = point {
                        if let Some(ref mut mgr) = self.terminal_manager {
                            mgr.update_selection_active(point, Direction::Right);
                        }
                    }
                    return;
                }

                // Panel border drag.
                match self.mouse_drag.border {
                    Some(DragBorder::Vertical) => {
                        if !self.show_tree || screen_width == 0 {
                            return;
                        }
                        let new_pct =
                            (u32::from(mouse.column) * 100 / u32::from(screen_width)) as u16;
                        self.tree_width_pct = new_pct.clamp(MIN_PANEL_PCT, MAX_PANEL_PCT);
                    }
                    Some(DragBorder::Horizontal) => {
                        if !self.show_terminal || main_height == 0 {
                            return;
                        }
                        // Compute right area start (0 if tree hidden)
                        let right_area_start_y: u16 = 0;
                        let right_area_height = main_height;
                        let relative_row = mouse.row.saturating_sub(right_area_start_y);
                        let new_pct =
                            (u32::from(relative_row) * 100 / u32::from(right_area_height)) as u16;
                        self.editor_height_pct = new_pct.clamp(MIN_PANEL_PCT, MAX_PANEL_PCT);
                    }
                    None => {}
                }
            }
            MouseEventKind::Up(MouseButton::Left) => {
                self.scrollbar_dragging = false;

                if self.editor_selecting {
                    self.editor_selecting = false;
                    // Clean up empty selection (click without drag).
                    if let Some(buf) = self.buffer_manager.active_buffer_mut() {
                        if buf
                            .selection
                            .as_ref()
                            .is_some_and(|s| s.is_empty(buf.cursor.row, buf.cursor.col))
                        {
                            buf.clear_selection();
                        }
                    }
                }
                if self.terminal_selecting {
                    self.terminal_selecting = false;

                    // Check if this was a click without drag (no movement).
                    let was_click = self
                        .terminal_select_start
                        .is_none_or(|(sx, sy)| sx == mouse.column && sy == mouse.row);
                    self.terminal_select_start = None;

                    if was_click {
                        // Click without drag -- clear selection.
                        if let Some(ref mut mgr) = self.terminal_manager {
                            mgr.clear_selection_active();
                        }
                    } else {
                        // Drag completed -- copy selection to clipboard.
                        let text = self
                            .terminal_manager
                            .as_ref()
                            .and_then(|mgr| mgr.copy_selection_active());
                        if let Some(ref text) = text {
                            if !text.is_empty() {
                                Self::copy_to_clipboard(text);
                            }
                        }
                    }
                } else if self.terminal_click_state.click_count > 1 {
                    // Multi-click (double/triple) completed -- copy selection to clipboard.
                    let text = self
                        .terminal_manager
                        .as_ref()
                        .and_then(|mgr| mgr.copy_selection_active());
                    if let Some(ref text) = text {
                        if !text.is_empty() {
                            Self::copy_to_clipboard(text);
                        }
                    }
                }
                if self.mouse_drag.border.is_some() {
                    self.needs_full_redraw = true;
                }
                self.mouse_drag.border = None;
            }
            MouseEventKind::ScrollUp => {
                let shift = mouse.modifiers.contains(KeyModifiers::SHIFT);
                match self.panel_at(mouse.column, mouse.row, screen_width, main_height) {
                    FocusTarget::Terminal(_) if self.show_terminal => {
                        self.terminal_scroll(alacritty_terminal::grid::Scroll::Delta(
                            MOUSE_SCROLL_LINES,
                        ));
                    }
                    FocusTarget::Editor if shift => {
                        self.editor_scroll_horizontal(-MOUSE_SCROLL_COLS);
                    }
                    FocusTarget::Editor => {
                        self.editor_scroll(-MOUSE_SCROLL_LINES);
                    }
                    FocusTarget::Tree if shift => {
                        self.tree_scroll_horizontal(-MOUSE_SCROLL_COLS);
                    }
                    FocusTarget::Tree => {
                        self.tree_scroll(-MOUSE_SCROLL_LINES);
                    }
                    _ => {}
                }
            }
            MouseEventKind::ScrollDown => {
                let shift = mouse.modifiers.contains(KeyModifiers::SHIFT);
                match self.panel_at(mouse.column, mouse.row, screen_width, main_height) {
                    FocusTarget::Terminal(_) if self.show_terminal => {
                        self.terminal_scroll(alacritty_terminal::grid::Scroll::Delta(
                            -MOUSE_SCROLL_LINES,
                        ));
                    }
                    FocusTarget::Editor if shift => {
                        self.editor_scroll_horizontal(MOUSE_SCROLL_COLS);
                    }
                    FocusTarget::Editor => {
                        self.editor_scroll(MOUSE_SCROLL_LINES);
                    }
                    FocusTarget::Tree if shift => {
                        self.tree_scroll_horizontal(MOUSE_SCROLL_COLS);
                    }
                    FocusTarget::Tree => {
                        self.tree_scroll(MOUSE_SCROLL_LINES);
                    }
                    _ => {}
                }
            }
            MouseEventKind::ScrollLeft => {
                match self.panel_at(mouse.column, mouse.row, screen_width, main_height) {
                    FocusTarget::Editor => {
                        self.editor_scroll_horizontal(-MOUSE_SCROLL_COLS);
                    }
                    FocusTarget::Tree => {
                        self.tree_scroll_horizontal(-MOUSE_SCROLL_COLS);
                    }
                    _ => {}
                }
            }
            MouseEventKind::ScrollRight => {
                match self.panel_at(mouse.column, mouse.row, screen_width, main_height) {
                    FocusTarget::Editor => {
                        self.editor_scroll_horizontal(MOUSE_SCROLL_COLS);
                    }
                    FocusTarget::Tree => {
                        self.tree_scroll_horizontal(MOUSE_SCROLL_COLS);
                    }
                    _ => {}
                }
            }
            _ => {}
        }
    }

    /// Determines which panel occupies the given screen position.
    pub(super) fn panel_at(
        &self,
        col: u16,
        row: u16,
        screen_width: u16,
        main_height: u16,
    ) -> FocusTarget {
        let tree_border_x = (u32::from(screen_width) * u32::from(self.tree_width_pct) / 100) as u16;

        if self.show_tree && col < tree_border_x {
            return FocusTarget::Tree;
        }

        if self.show_terminal {
            let border_y =
                (u32::from(main_height) * u32::from(self.editor_height_pct) / 100) as u16;
            if row >= border_y {
                return FocusTarget::Terminal(0);
            }
        }

        FocusTarget::Editor
    }
}
