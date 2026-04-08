use std::time::Instant;

use axe_tree::NodeKind;

use super::{AppState, ConfirmDialog, FocusTarget, GoToLineDialog};
use crate::command::Command;
use crate::command_palette::CommandPalette;
use crate::file_finder::FileFinder;
use crate::project_search::ProjectSearch;
use crate::search::SearchState;

/// Number of columns to scroll per Shift+mouse wheel tick.
const MOUSE_SCROLL_COLS: i32 = 6;

impl AppState {
    /// Dispatches a command to update application state.
    pub fn execute(&mut self, cmd: Command) {
        match cmd {
            Command::Quit => self.quit(),
            Command::RequestQuit => self.confirm_dialog = Some(ConfirmDialog::quit()),
            Command::FocusNext => self.cycle_focus_next(),
            Command::FocusPrev => self.cycle_focus_prev(),
            Command::FocusTree => self.focus = FocusTarget::Tree,
            Command::FocusEditor => self.focus = FocusTarget::Editor,
            Command::FocusTerminal => self.focus = FocusTarget::Terminal(0),
            Command::ToggleTree => self.toggle_tree(),
            Command::ToggleTerminal => self.toggle_terminal(),
            Command::ShowHelp => self.show_help = !self.show_help,
            Command::CloseOverlay => {
                if self.go_to_line.is_some() {
                    self.go_to_line = None;
                } else if self.command_palette.is_some() {
                    self.command_palette = None;
                } else if let Some(ref mut ps) = self.project_search {
                    ps.cancel_search();
                    self.project_search = None;
                } else if self.location_list.is_some() {
                    self.location_list = None;
                } else if self.file_finder.is_some() {
                    self.file_finder = None;
                } else {
                    self.show_help = false;
                }
            }
            Command::EnterResizeMode => self.resize_mode.active = true,
            Command::ExitResizeMode => self.resize_mode.active = false,
            Command::ResizeLeft => self.resize_horizontal(-1),
            Command::ResizeRight => self.resize_horizontal(1),
            Command::ResizeUp => self.resize_vertical(-1),
            Command::ResizeDown => self.resize_vertical(1),
            Command::EqualizeLayout => self.equalize_layout(),
            Command::ZoomPanel => self.toggle_zoom(),
            Command::TreeUp => {
                if let Some(ref mut tree) = self.file_tree {
                    tree.move_up();
                }
            }
            Command::TreeDown => {
                if let Some(ref mut tree) = self.file_tree {
                    tree.move_down();
                }
            }
            // IMPACT ANALYSIS — TreeToggle with file open
            // Parents: KeyEvent(Enter) when tree focused -> Command::TreeToggle
            // Children: BufferManager::open_file() -> UI render, FocusTarget::Editor
            // Siblings: Tree selection (unchanged), terminal (unaffected)
            Command::TreeToggle => {
                if let Some(ref tree) = self.file_tree {
                    if let Some(node) = tree.selected_node() {
                        if matches!(node.kind, NodeKind::File { .. }) {
                            let path = node.path.clone();
                            self.execute(Command::OpenFile(path));
                            return;
                        }
                    }
                }
                if let Some(ref mut tree) = self.file_tree {
                    let _ = tree.toggle();
                }
            }
            Command::TreeExpand => {
                if let Some(ref mut tree) = self.file_tree {
                    let _ = tree.expand();
                }
            }
            Command::TreeCollapseOrParent => {
                if let Some(ref mut tree) = self.file_tree {
                    tree.collapse_or_parent();
                }
            }
            Command::TreeHome => {
                if let Some(ref mut tree) = self.file_tree {
                    tree.move_home();
                }
            }
            Command::TreeEnd => {
                if let Some(ref mut tree) = self.file_tree {
                    tree.move_end();
                }
            }
            Command::TreeScrollLeft => {
                self.tree_scroll_horizontal(-MOUSE_SCROLL_COLS);
            }
            Command::TreeScrollRight => {
                self.tree_scroll_horizontal(MOUSE_SCROLL_COLS);
            }
            Command::ToggleIgnored => {
                if let Some(ref mut tree) = self.file_tree {
                    tree.toggle_show_ignored();
                }
            }
            Command::TreeCreateFile => {
                if let Some(ref mut tree) = self.file_tree {
                    tree.start_create_file();
                }
            }
            Command::TreeCreateDir => {
                if let Some(ref mut tree) = self.file_tree {
                    tree.start_create_dir();
                }
            }
            Command::TreeRename => {
                if let Some(ref mut tree) = self.file_tree {
                    tree.start_rename();
                }
            }
            Command::TreeDelete => {
                if let Some(ref mut tree) = self.file_tree {
                    // Get the node name before starting the action.
                    let node_name = tree
                        .selected_node()
                        .map(|n| n.name.clone())
                        .unwrap_or_default();
                    tree.start_delete();
                    // Only show dialog if start_delete actually set the action
                    // (it's a noop on root node).
                    if matches!(tree.action(), axe_tree::TreeAction::ConfirmDelete { .. }) {
                        self.confirm_dialog = Some(ConfirmDialog::delete_tree_node(&node_name));
                    }
                }
            }
            Command::ConfirmTreeDelete => {
                if let Some(ref mut tree) = self.file_tree {
                    let _ = tree.confirm_delete();
                }
            }
            Command::CancelTreeDelete => {
                if let Some(ref mut tree) = self.file_tree {
                    tree.cancel_action();
                }
            }
            Command::OpenFileFinder => {
                if let Some(ref root) = self.project_root {
                    self.file_finder = Some(FileFinder::new(root));
                }
            }
            Command::OpenCommandPalette => {
                self.command_palette = Some(CommandPalette::new(&self.keymap));
            }
            Command::OpenProjectSearch => {
                if let Some(ref mut ps) = self.project_search {
                    ps.cancel_search();
                }
                self.project_search = Some(ProjectSearch::new());
            }
            Command::ToggleIcons => {
                if let Some(ref mut tree) = self.file_tree {
                    tree.toggle_show_icons();
                }
            }
            // Unified tab commands — dispatch based on current focus.
            Command::NewTab => {
                if matches!(self.focus, FocusTarget::Terminal(_)) {
                    self.new_terminal_tab();
                }
            }
            Command::CloseTab => match self.focus {
                FocusTarget::Editor => self.execute(Command::CloseBuffer),
                FocusTarget::Terminal(_) => self.execute(Command::CloseTerminalTab),
                FocusTarget::Tree => {}
            },
            Command::NextTab => match self.focus {
                FocusTarget::Editor => {
                    self.search = None;
                    self.buffer_manager.next_buffer();
                }
                FocusTarget::Terminal(_) => self.next_terminal_tab(),
                FocusTarget::Tree => {}
            },
            Command::PrevTab => match self.focus {
                FocusTarget::Editor => {
                    self.search = None;
                    self.buffer_manager.prev_buffer();
                }
                FocusTarget::Terminal(_) => self.prev_terminal_tab(),
                FocusTarget::Tree => {}
            },
            Command::NewTerminalTab => self.new_terminal_tab(),
            Command::CloseTerminalTab => {
                if let Some(ref mut mgr) = self.terminal_manager {
                    if mgr.active_tab_is_alive() {
                        let tab_title = mgr
                            .active_tab()
                            .map(|t| t.title().to_string())
                            .unwrap_or_else(|| "terminal".to_string());
                        self.confirm_dialog = Some(ConfirmDialog::close_terminal(&tab_title));
                    } else {
                        self.close_terminal_tab();
                    }
                }
            }
            Command::ForceCloseTerminalTab => {
                self.close_terminal_tab();
            }
            Command::CancelCloseTerminalTab => {
                // Dialog already dismissed by the input handler.
            }
            Command::ActivateTerminalTab(idx) => self.activate_terminal_tab(idx),
            Command::TerminalScrollPageUp => {
                self.terminal_scroll(alacritty_terminal::grid::Scroll::PageUp);
            }
            Command::TerminalScrollPageDown => {
                self.terminal_scroll(alacritty_terminal::grid::Scroll::PageDown);
            }
            Command::TerminalScrollTop => {
                self.terminal_scroll(alacritty_terminal::grid::Scroll::Top);
            }
            Command::TerminalScrollBottom => {
                self.terminal_scroll(alacritty_terminal::grid::Scroll::Bottom);
            }
            // IMPACT ANALYSIS — OpenFile
            // Parents: TreeToggle on file node, future: command palette, fuzzy finder
            // Children: BufferManager adds buffer, focus switches to Editor
            // Siblings: Tree state (unchanged), terminal (unchanged)
            Command::OpenFile(path) => match self.buffer_manager.open_file(&path) {
                Ok(()) => {
                    self.buffer_manager.promote_preview();
                    self.focus = FocusTarget::Editor;
                    // Notify LSP about the newly opened file.
                    if let Some(ref mut lsp) = self.lsp_manager {
                        if let Some(buf) = self.buffer_manager.active_buffer() {
                            let text = buf.content_string();
                            if let Err(e) = lsp.file_opened(&path, &text) {
                                log::warn!("LSP didOpen failed: {e}");
                            }
                        }
                    }
                    // Compute initial git diff hunks.
                    self.refresh_active_buffer_diff_hunks();
                }
                Err(e) => log::warn!("Failed to open file: {e}"),
            },
            // IMPACT ANALYSIS — PreviewFile
            // Parents: Single click on file in tree
            // Children: BufferManager opens preview buffer (replaces previous preview)
            // Siblings: Tree state (unchanged), terminal (unchanged)
            Command::PreviewFile(path) => match self.buffer_manager.open_file_as_preview(&path) {
                Ok(()) => {
                    self.focus = FocusTarget::Editor;
                    self.refresh_active_buffer_diff_hunks();
                }
                Err(e) => log::warn!("Failed to preview file: {e}"),
            },
            // IMPACT ANALYSIS — Editor cursor movement commands
            // Parents: KeyEvent with editor focus -> editor-focus interception -> these commands
            // Children: EditorBuffer cursor/scroll state
            // Siblings: Tree/terminal unaffected; UI reads cursor/scroll to render
            Command::EditorUp
            | Command::EditorDown
            | Command::EditorLeft
            | Command::EditorRight
            | Command::EditorHome
            | Command::EditorEnd
            | Command::EditorFileStart
            | Command::EditorFileEnd
            | Command::EditorPageUp
            | Command::EditorPageDown
            | Command::EditorWordRight
            | Command::EditorWordLeft => {
                let (h, w) = self.editor_viewport();
                if let Some(buf) = self.buffer_manager.active_buffer_mut() {
                    buf.clear_selection();
                    match cmd {
                        Command::EditorUp => buf.move_up(),
                        Command::EditorDown => buf.move_down(),
                        Command::EditorLeft => buf.move_left(),
                        Command::EditorRight => buf.move_right(),
                        Command::EditorHome => buf.move_home(),
                        Command::EditorEnd => buf.move_end(),
                        Command::EditorFileStart => buf.move_file_start(),
                        Command::EditorFileEnd => buf.move_file_end(),
                        Command::EditorPageUp => buf.move_page_up(h),
                        Command::EditorPageDown => buf.move_page_down(h),
                        Command::EditorWordRight => buf.move_word_right(),
                        Command::EditorWordLeft => buf.move_word_left(),
                        _ => unreachable!(),
                    }
                    buf.ensure_cursor_visible(h, w);
                }
                // Dismiss completion if cursor moved away from trigger position.
                if let Some(ref comp) = self.completion {
                    if let Some(buf) = self.buffer_manager.active_buffer() {
                        if buf.cursor.row != comp.trigger_row || buf.cursor.col < comp.trigger_col {
                            self.completion = None;
                        }
                    }
                }
                // Dismiss hover on any cursor movement.
                self.hover_info = None;
            }
            // IMPACT ANALYSIS — Editor edit commands
            // Parents: KeyEvent with editor focus -> editor-focus interception -> these commands
            // Children: EditorBuffer content/cursor/modified state, last_edit_time for autosave
            // Siblings: UI reads modified flag for [+] indicator, status bar line count
            Command::EditorInsertChar(ch) => {
                let (h, w) = self.editor_viewport();
                if let Some(buf) = self.buffer_manager.active_buffer_mut() {
                    buf.insert_char(ch);
                    buf.ensure_cursor_visible(h, w);
                }
                self.last_edit_time = Some(Instant::now());
                self.notify_lsp_change();
                self.update_completion_after_edit();
                self.maybe_auto_trigger_completion(ch);
            }
            Command::EditorBackspace => {
                let (h, w) = self.editor_viewport();
                if let Some(buf) = self.buffer_manager.active_buffer_mut() {
                    buf.delete_char_backward();
                    buf.ensure_cursor_visible(h, w);
                }
                self.last_edit_time = Some(Instant::now());
                self.notify_lsp_change();
                self.update_completion_after_edit();
            }
            Command::EditorDelete => {
                let (h, w) = self.editor_viewport();
                if let Some(buf) = self.buffer_manager.active_buffer_mut() {
                    buf.delete_char_forward();
                    buf.ensure_cursor_visible(h, w);
                }
                self.last_edit_time = Some(Instant::now());
                self.notify_lsp_change();
                self.update_completion_after_edit();
            }
            Command::EditorNewline => {
                self.completion = None;
                let (h, w) = self.editor_viewport();
                if let Some(buf) = self.buffer_manager.active_buffer_mut() {
                    buf.insert_newline();
                    buf.ensure_cursor_visible(h, w);
                }
                self.last_edit_time = Some(Instant::now());
                self.notify_lsp_change();
            }
            Command::EditorTab => {
                let (h, w) = self.editor_viewport();
                if let Some(buf) = self.buffer_manager.active_buffer_mut() {
                    buf.insert_tab();
                    buf.ensure_cursor_visible(h, w);
                }
                self.last_edit_time = Some(Instant::now());
                self.notify_lsp_change();
            }
            // IMPACT ANALYSIS — EditorSave with format-on-save
            // Parents: KeyEvent -> Ctrl+S -> keymap -> this command; also autosave
            // Children: save_active_buffer() writes to disk, notifies LSP didSave
            // Siblings: pending_format_save flag coordinates with poll_lsp FormattingResponse
            // Risk: Must not lose data if LSP formatting fails -- save anyway on error
            Command::EditorSave => {
                if self.config.editor.format_on_save {
                    if self.request_format_for_active_buffer() {
                        self.pending_format_save = true;
                    } else {
                        self.save_active_buffer();
                    }
                } else {
                    self.save_active_buffer();
                }
            }
            // IMPACT ANALYSIS — EditorUndo / EditorRedo
            // Parents: KeyEvent -> Ctrl+Z / Ctrl+Y / Ctrl+Shift+Z -> keymap -> these commands
            // Children: EditorBuffer::undo()/redo() reverses/replays edits on rope, restores cursor
            // Siblings: Does NOT set last_edit_time (no autosave trigger for undo/redo)
            Command::EditorUndo => {
                let (h, w) = self.editor_viewport();
                if let Some(buf) = self.buffer_manager.active_buffer_mut() {
                    buf.undo();
                    buf.ensure_cursor_visible(h, w);
                }
                self.notify_lsp_change();
            }
            Command::EditorRedo => {
                let (h, w) = self.editor_viewport();
                if let Some(buf) = self.buffer_manager.active_buffer_mut() {
                    buf.redo();
                    buf.ensure_cursor_visible(h, w);
                }
                self.notify_lsp_change();
            }
            // IMPACT ANALYSIS — Selection commands
            // Parents: KeyEvent -> Shift+Arrow/Ctrl+Shift+Arrow -> these commands
            // Children: EditorBuffer selection state, cursor position
            // Siblings: Plain movement commands (clear selection), UI renders selection highlight
            Command::EditorSelectUp
            | Command::EditorSelectDown
            | Command::EditorSelectLeft
            | Command::EditorSelectRight
            | Command::EditorSelectHome
            | Command::EditorSelectEnd
            | Command::EditorSelectFileStart
            | Command::EditorSelectFileEnd
            | Command::EditorSelectWordLeft
            | Command::EditorSelectWordRight => {
                let (h, w) = self.editor_viewport();
                if let Some(buf) = self.buffer_manager.active_buffer_mut() {
                    match cmd {
                        Command::EditorSelectUp => buf.select_up(),
                        Command::EditorSelectDown => buf.select_down(),
                        Command::EditorSelectLeft => buf.select_left(),
                        Command::EditorSelectRight => buf.select_right(),
                        Command::EditorSelectHome => buf.select_home(),
                        Command::EditorSelectEnd => buf.select_end(),
                        Command::EditorSelectFileStart => buf.select_file_start(),
                        Command::EditorSelectFileEnd => buf.select_file_end(),
                        Command::EditorSelectWordLeft => buf.select_word_left(),
                        Command::EditorSelectWordRight => buf.select_word_right(),
                        _ => unreachable!(),
                    }
                    buf.ensure_cursor_visible(h, w);
                }
            }
            Command::EditorSelectAll => {
                if let Some(buf) = self.buffer_manager.active_buffer_mut() {
                    buf.select_all();
                }
            }
            // IMPACT ANALYSIS — Clipboard commands
            // Parents: KeyEvent -> Ctrl+C/X/V -> keymap -> these commands
            // Children: System clipboard (read/write), buffer content (cut/paste)
            // Siblings: Selection (copy/cut read it, paste may replace it),
            //           last_edit_time (cut/paste trigger autosave timer)
            Command::EditorCopy => {
                let text = self
                    .buffer_manager
                    .active_buffer()
                    .and_then(|buf| buf.selected_text());
                if let Some(ref text) = text {
                    if !text.is_empty() {
                        self.ensure_clipboard();
                        if let Some(ref mut cb) = self.clipboard {
                            if let Err(e) = cb.set_text(text) {
                                log::warn!("Failed to copy to clipboard: {e}");
                            }
                        }
                        let lines = text.lines().count();
                        let chars = text.len();
                        self.set_status_message(format!("Copied {lines} line(s), {chars} char(s)"));
                    }
                }
            }
            Command::EditorCut => {
                let (h, w) = self.editor_viewport();
                let cut_text = self.buffer_manager.active_buffer_mut().and_then(|buf| {
                    let text = buf.delete_selection();
                    buf.ensure_cursor_visible(h, w);
                    text
                });
                if let Some(ref text) = cut_text {
                    if !text.is_empty() {
                        self.ensure_clipboard();
                        if let Some(ref mut cb) = self.clipboard {
                            if let Err(e) = cb.set_text(text) {
                                log::warn!("Failed to copy to clipboard: {e}");
                            }
                        }
                        let lines = text.lines().count();
                        let chars = text.len();
                        self.set_status_message(format!("Cut {lines} line(s), {chars} char(s)"));
                    }
                }
                self.last_edit_time = Some(Instant::now());
                self.notify_lsp_change();
            }
            Command::EditorPaste => {
                let (h, w) = self.editor_viewport();
                self.ensure_clipboard();
                let text = self
                    .clipboard
                    .as_mut()
                    .and_then(|cb| cb.get_text().ok())
                    .unwrap_or_default();
                if !text.is_empty() {
                    if let Some(buf) = self.buffer_manager.active_buffer_mut() {
                        buf.insert_text(&text);
                        buf.ensure_cursor_visible(h, w);
                    }
                    self.last_edit_time = Some(Instant::now());
                    self.notify_lsp_change();
                }
            }
            // IMPACT ANALYSIS — Search commands
            // Parents: KeyEvent -> Ctrl+F / search interception layer -> these commands
            // Children: SearchState (created/modified), cursor position (match navigation)
            // Siblings: Selection (cleared on search open), editor key interception
            //           (search layer runs before editor layer when active)
            Command::EditorFind => {
                if self.search.is_none() {
                    let mut search = SearchState::new();
                    // Pre-fill from selection if available.
                    if let Some(buf) = self.buffer_manager.active_buffer() {
                        if let Some(text) = buf.selected_text() {
                            // Use only the first line for pre-fill.
                            let first_line = text.lines().next().unwrap_or("").to_string();
                            if !first_line.is_empty() {
                                search.query = first_line;
                                search.update_matches(buf);
                                let row = buf.cursor.row;
                                let col = buf.cursor.col;
                                search.nearest_match_from(row, col);
                            }
                        }
                    }
                    self.search = Some(search);
                }
                // If already open, no-op (focus stays on search bar).
            }
            // IMPACT ANALYSIS — EditorFindReplace
            // Parents: KeyEvent -> Ctrl+H -> keymap -> this command
            // Children: SearchState created/modified with replace_visible=true
            // Siblings: EditorFind (same search state, just without replace row)
            Command::EditorFindReplace => {
                use crate::search::SearchField;
                if let Some(ref mut search) = self.search {
                    search.replace_visible = true;
                    search.active_field = SearchField::Replace;
                } else {
                    let mut search = SearchState::new();
                    search.replace_visible = true;
                    // Pre-fill from selection if available.
                    if let Some(buf) = self.buffer_manager.active_buffer() {
                        if let Some(text) = buf.selected_text() {
                            let first_line = text.lines().next().unwrap_or("").to_string();
                            if !first_line.is_empty() {
                                search.query = first_line;
                                search.update_matches(buf);
                                let row = buf.cursor.row;
                                let col = buf.cursor.col;
                                search.nearest_match_from(row, col);
                            }
                        }
                    }
                    self.search = Some(search);
                }
            }
            // IMPACT ANALYSIS — ReplaceNext
            // Parents: Enter in replace field, or command dispatch
            // Children: Buffer content changes (apply_text_edit), search matches recomputed
            // Siblings: last_edit_time (autosave trigger), LSP didChange
            Command::ReplaceNext => {
                let (h, w) = self.editor_viewport();
                if let Some(ref search) = self.search {
                    if let Some(m) = search.current_match().cloned() {
                        let replace_text = search.replace_query.clone();
                        if let Some(buf) = self.buffer_manager.active_buffer_mut() {
                            buf.apply_text_edit(
                                m.row,
                                m.col_start,
                                m.row,
                                m.col_end,
                                &replace_text,
                            );
                            buf.ensure_cursor_visible(h, w);
                        }
                        self.last_edit_time = Some(Instant::now());
                        self.notify_lsp_change();
                        if let Some(ref mut search) = self.search {
                            if let Some(buf) = self.buffer_manager.active_buffer() {
                                search.update_matches(buf);
                                let cursor_row = buf.cursor.row;
                                let cursor_col = buf.cursor.col;
                                search.nearest_match_from(cursor_row, cursor_col);
                            }
                        }
                    }
                }
            }
            // IMPACT ANALYSIS — ReplaceAll
            // Parents: Ctrl+Alt+Enter in search/replace bar
            // Children: Buffer content changes (multiple apply_text_edit), undo grouped
            // Siblings: last_edit_time, LSP didChange, status message
            Command::ReplaceAll => {
                let (h, w) = self.editor_viewport();
                if let Some(ref search) = self.search {
                    let matches: Vec<_> = search.matches.clone();
                    let replace_text = search.replace_query.clone();
                    if matches.is_empty() {
                        return;
                    }
                    let count = matches.len();
                    if let Some(buf) = self.buffer_manager.active_buffer_mut() {
                        buf.begin_undo_group();
                        // Iterate in reverse so earlier indices stay valid.
                        for m in matches.iter().rev() {
                            buf.apply_text_edit(
                                m.row,
                                m.col_start,
                                m.row,
                                m.col_end,
                                &replace_text,
                            );
                        }
                        buf.end_undo_group();
                        buf.ensure_cursor_visible(h, w);
                    }
                    self.last_edit_time = Some(Instant::now());
                    self.notify_lsp_change();
                    if let Some(ref mut search) = self.search {
                        if let Some(buf) = self.buffer_manager.active_buffer() {
                            search.update_matches(buf);
                        }
                    }
                    self.set_status_message(format!("Replaced {count} occurrences"));
                }
            }
            Command::SearchClose => {
                self.search = None;
            }
            Command::SearchNextMatch => {
                let (h, w) = self.editor_viewport();
                if let Some(ref mut search) = self.search {
                    search.next_match();
                    if let Some(m) = search.current_match().cloned() {
                        if let Some(buf) = self.buffer_manager.active_buffer_mut() {
                            buf.cursor.row = m.row;
                            buf.cursor.col = m.col_start;
                            buf.clear_selection();
                            buf.ensure_cursor_visible(h, w);
                        }
                    }
                }
            }
            Command::SearchPrevMatch => {
                let (h, w) = self.editor_viewport();
                if let Some(ref mut search) = self.search {
                    search.prev_match();
                    if let Some(m) = search.current_match().cloned() {
                        if let Some(buf) = self.buffer_manager.active_buffer_mut() {
                            buf.cursor.row = m.row;
                            buf.cursor.col = m.col_start;
                            buf.clear_selection();
                            buf.ensure_cursor_visible(h, w);
                        }
                    }
                }
            }
            Command::SearchToggleCase => {
                if let Some(ref mut search) = self.search {
                    if let Some(buf) = self.buffer_manager.active_buffer() {
                        search.toggle_case(buf);
                    }
                }
            }
            Command::SearchToggleRegex => {
                if let Some(ref mut search) = self.search {
                    if let Some(buf) = self.buffer_manager.active_buffer() {
                        search.toggle_regex(buf);
                    }
                }
            }
            Command::NextBuffer => {
                self.search = None;
                self.buffer_manager.next_buffer();
            }
            Command::PrevBuffer => {
                self.search = None;
                self.buffer_manager.prev_buffer();
            }
            Command::ActivateBuffer(idx) => {
                self.search = None;
                self.buffer_manager.set_active(idx);
            }
            Command::CloseBuffer => {
                if let Some(buf) = self.buffer_manager.active_buffer() {
                    if buf.modified {
                        let file_name = buf.file_name().unwrap_or("[untitled]").to_string();
                        self.confirm_dialog = Some(ConfirmDialog::close_buffer(&file_name));
                    } else {
                        let idx = self.buffer_manager.active_index();
                        self.buffer_manager.close_buffer(idx);
                        self.search = None;
                    }
                }
            }
            Command::ConfirmCloseBuffer => {
                let idx = self.buffer_manager.active_index();
                self.buffer_manager.close_buffer(idx);
                self.search = None;
            }
            Command::CancelCloseBuffer => {
                // Dialog already dismissed by the input handler.
            }
            Command::GoToNextDiagnostic => {
                self.go_to_next_diagnostic();
            }
            Command::GoToPrevDiagnostic => {
                self.go_to_prev_diagnostic();
            }
            Command::TriggerCompletion => {
                self.request_completion();
            }
            Command::AcceptCompletion => {
                self.apply_completion();
            }
            Command::DismissCompletion => {
                self.completion = None;
            }
            Command::GoToDefinition => {
                self.request_definition();
            }
            Command::FindReferences => {
                self.request_references();
            }
            Command::ShowHover => {
                self.request_hover();
            }
            Command::FormatDocument => {
                if !self.request_format_for_active_buffer() {
                    self.set_status_message("Formatting not available".to_string());
                }
            }
            Command::GoToLine => {
                if self.focus == FocusTarget::Editor {
                    if let Some(buf) = self.buffer_manager.active_buffer() {
                        let line_count = buf.line_count();
                        self.go_to_line = Some(GoToLineDialog::new(line_count));
                    }
                }
            }
            Command::OpenSshHostFinder => {
                self.open_ssh_host_finder();
            }
            Command::ShowDiffHunk => {
                self.show_diff_hunk();
            }
            Command::RevertDiffHunk => {
                self.revert_diff_hunk();
            }
            Command::CloseDiffPopup => {
                self.diff_popup = None;
            }
        }
        // Auto-promote preview buffer if user started editing it.
        self.buffer_manager.auto_promote_if_modified();
    }
}
