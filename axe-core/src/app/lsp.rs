use std::path::PathBuf;
use std::time::Instant;

use axe_editor::diagnostic::{BufferDiagnostic, DiagnosticSeverity};

use crate::command::Command;

use super::AppState;

impl AppState {
    /// Polls LSP events from all active language servers.
    ///
    /// Handles initialization events (logs status), crash events (shows status
    /// message). Call this each frame from the main loop.
    pub fn poll_lsp(&mut self) {
        let Some(ref mut lsp) = self.lsp_manager else {
            return;
        };

        let events = lsp.poll_events();
        for event in events {
            match event {
                axe_lsp::LspEvent::Initialized { language_id } => {
                    log::info!("LSP server initialized for {language_id}");
                    self.set_status_message(format!("LSP: {language_id} ready"));
                }
                axe_lsp::LspEvent::ServerCrashed { language_id, error } => {
                    log::warn!("LSP server crashed for {language_id}: {error}");
                    self.set_status_message(format!("LSP: {language_id} crashed"));
                }
                axe_lsp::LspEvent::ServerNotification { method, params } => {
                    if method == "textDocument/publishDiagnostics" {
                        self.handle_publish_diagnostics(&params);
                    }
                }
                axe_lsp::LspEvent::Response { .. } => {
                    // Non-initialize, non-completion responses — ignored.
                }
                axe_lsp::LspEvent::CompletionResponse { result: Ok(value) } => {
                    let items = crate::completion::parse_completion_response(&value);
                    if !items.is_empty() {
                        if let Some(buf) = self.buffer_manager.active_buffer() {
                            self.completion = Some(crate::completion::CompletionState::new(
                                items,
                                buf.cursor().row,
                                buf.cursor().col,
                            ));
                        }
                    }
                }
                axe_lsp::LspEvent::CompletionResponse { result: Err(e) } => {
                    log::warn!("LSP completion error: {}", e.message);
                }
                axe_lsp::LspEvent::DefinitionResponse { result: Ok(value) } => {
                    let project_root = self
                        .project_root
                        .clone()
                        .unwrap_or_else(|| PathBuf::from("."));
                    let items =
                        crate::location_list::parse_definition_response(&value, &project_root);
                    match items.len() {
                        0 => {
                            self.set_status_message("No definition found".to_string());
                        }
                        1 => {
                            // Single result: jump directly without overlay.
                            let path = items[0].path.clone();
                            let line = items[0].line;
                            let col = items[0].col;
                            self.execute(Command::OpenFile(path));
                            let (h, w) = self.editor_viewport();
                            if let Some(buf) = self.buffer_manager.active_buffer_mut() {
                                buf.cursor_mut().row = line;
                                buf.cursor_mut().col = col;
                                buf.ensure_cursor_visible(h, w);
                            }
                        }
                        _ => {
                            self.location_list =
                                Some(crate::location_list::LocationList::new("Definition", items));
                        }
                    }
                }
                axe_lsp::LspEvent::DefinitionResponse { result: Err(e) } => {
                    log::warn!("LSP definition error: {}", e.message);
                }
                axe_lsp::LspEvent::ReferencesResponse { result: Ok(value) } => {
                    let project_root = self
                        .project_root
                        .clone()
                        .unwrap_or_else(|| PathBuf::from("."));
                    let items =
                        crate::location_list::parse_references_response(&value, &project_root);
                    if items.is_empty() {
                        self.set_status_message("No references found".to_string());
                    } else {
                        self.location_list =
                            Some(crate::location_list::LocationList::new("References", items));
                    }
                }
                axe_lsp::LspEvent::ReferencesResponse { result: Err(e) } => {
                    log::warn!("LSP references error: {}", e.message);
                }
                axe_lsp::LspEvent::HoverResponse { result: Ok(value) } => {
                    if let Some(mut info) = crate::hover::parse_hover_response(&value) {
                        // Attach current cursor position for rendering near cursor.
                        if let Some(buf) = self.buffer_manager.active_buffer() {
                            info.trigger_row = buf.cursor().row;
                            info.trigger_col = buf.cursor().col;
                        }
                        self.hover_info = Some(info);
                    } else {
                        self.set_status_message("No hover info available".to_string());
                    }
                }
                axe_lsp::LspEvent::HoverResponse { result: Err(e) } => {
                    log::warn!("LSP hover error: {}", e.message);
                }
                axe_lsp::LspEvent::FormattingResponse {
                    result: Ok(ref value),
                } => {
                    self.apply_formatting_edits(value);
                    if self.pending_format_save {
                        self.pending_format_save = false;
                        self.notify_lsp_change();
                        self.save_active_buffer();
                    }
                }
                axe_lsp::LspEvent::FormattingResponse { result: Err(e) } => {
                    log::warn!("LSP formatting error: {}", e.message);
                    if self.pending_format_save {
                        self.pending_format_save = false;
                        self.save_active_buffer(); // Save anyway on error.
                    }
                }
                axe_lsp::LspEvent::InlayHintResponse {
                    path,
                    version,
                    result: Ok(value),
                } => {
                    // Drop stale responses: if the buffer has moved on since
                    // the request was sent, the hint positions no longer line
                    // up with the text.
                    let current = self
                        .buffer_content_versions
                        .get(&path)
                        .copied()
                        .unwrap_or(0);
                    if version < current {
                        continue;
                    }
                    let hints = crate::inlay::parse_inlay_hint_response(&value);
                    self.inlay_hints
                        .set(path, crate::inlay::InlayHintEntry { version, hints });
                }
                axe_lsp::LspEvent::InlayHintResponse {
                    path: _,
                    version: _,
                    result: Err(e),
                } => {
                    log::warn!("LSP inlay hint error: {}", e.message);
                }
                axe_lsp::LspEvent::SignatureHelpResponse { result: Ok(value) } => {
                    let (row, col) = self
                        .buffer_manager
                        .active_buffer()
                        .map(|buf| (buf.cursor().row, buf.cursor().col))
                        .unwrap_or((0, 0));
                    if let Some(state) =
                        crate::signature_help::parse_signature_help_response(&value, row, col)
                    {
                        self.signature_help = Some(state);
                    } else {
                        // Empty response — drop any existing popup.
                        self.signature_help = None;
                    }
                }
                axe_lsp::LspEvent::SignatureHelpResponse { result: Err(e) } => {
                    log::warn!("LSP signature help error: {}", e.message);
                }
                axe_lsp::LspEvent::RenameResponse { result: Ok(value) } => {
                    match crate::rename::parse_workspace_edit_response(&value) {
                        Some(ws_edit) if !ws_edit.files.is_empty() => {
                            let label = "Rename";
                            match self.buffer_manager.apply_workspace_edit(&ws_edit, label) {
                                Ok(summary) => {
                                    self.rename = None;
                                    self.set_status_message(format!(
                                        "Renamed {} edit(s) across {} file(s)",
                                        summary.edits_applied, summary.files_affected
                                    ));
                                    self.last_edit_time = Some(Instant::now());
                                    self.notify_lsp_change();
                                }
                                Err(e) => {
                                    log::warn!("Failed to apply rename edit: {e}");
                                    self.set_status_message(
                                        "Rename failed: could not apply edits".to_string(),
                                    );
                                }
                            }
                        }
                        _ => {
                            self.rename = None;
                            self.set_status_message(
                                "Rename: server returned no changes".to_string(),
                            );
                        }
                    }
                }
                axe_lsp::LspEvent::RenameResponse { result: Err(e) } => {
                    log::warn!("LSP rename error: {}", e.message);
                    self.rename = None;
                    self.set_status_message(format!("Rename failed: {}", e.message));
                }
                axe_lsp::LspEvent::CodeActionsResponse { result: Ok(value) } => {
                    let actions = crate::code_actions::parse_code_actions_response(&value);
                    if actions.is_empty() {
                        self.code_actions = None;
                        self.set_status_message("No code actions available".to_string());
                    } else if let Some(buf) = self.buffer_manager.active_buffer() {
                        self.code_actions = Some(crate::code_actions::CodeActionsState::new(
                            actions,
                            buf.cursor().row,
                            buf.cursor().col,
                        ));
                    }
                }
                axe_lsp::LspEvent::CodeActionsResponse { result: Err(e) } => {
                    log::warn!("LSP code actions error: {}", e.message);
                    self.code_actions = None;
                    self.set_status_message(format!("Code actions failed: {}", e.message));
                }
                axe_lsp::LspEvent::ExecuteCommandResponse { result: Ok(value) } => {
                    // rust-analyzer and gopls sometimes return a WorkspaceEdit
                    // directly from workspace/executeCommand; apply it when
                    // present so server-side refactorings land in the buffers.
                    if let Some(ws_edit) = crate::rename::parse_workspace_edit_response(&value) {
                        if !ws_edit.files.is_empty() {
                            match self
                                .buffer_manager
                                .apply_workspace_edit(&ws_edit, "Code Action")
                            {
                                Ok(summary) => {
                                    self.set_status_message(format!(
                                        "Applied {} edit(s) in {} file(s)",
                                        summary.edits_applied, summary.files_affected
                                    ));
                                    self.last_edit_time = Some(Instant::now());
                                    self.notify_lsp_change();
                                }
                                Err(e) => {
                                    log::warn!("workspace/executeCommand apply failed: {e}");
                                }
                            }
                        }
                    }
                }
                axe_lsp::LspEvent::ExecuteCommandResponse { result: Err(e) } => {
                    log::warn!("workspace/executeCommand error: {}", e.message);
                }
            }
        }
    }

    /// Notifies the LSP manager that the active buffer content changed.
    ///
    /// Called after every edit command (insert, delete, paste, undo, redo, etc.).
    /// Also bumps the per-buffer content version and re-requests inlay hints
    /// so the display stays in sync with the text.
    pub(super) fn notify_lsp_change(&mut self) {
        let Some(path) = self
            .buffer_manager
            .active_buffer()
            .and_then(|buf| buf.path())
            .map(|p| p.to_path_buf())
        else {
            return;
        };

        let text = self
            .buffer_manager
            .active_buffer()
            .map(|buf| buf.content_string())
            .unwrap_or_default();

        let version = self.bump_content_version(&path);

        if let Some(ref mut lsp) = self.lsp_manager {
            if let Err(e) = lsp.file_changed(&path, &text) {
                log::warn!("LSP didChange failed: {e}");
            }
        }

        self.request_inlay_hints_for(&path, version);
    }

    /// Increments and returns the content version for the given buffer path.
    ///
    /// Used as a monotonic counter to discard stale inlay-hint (and future
    /// versioned) responses for a buffer whose contents have since changed.
    pub(super) fn bump_content_version(&mut self, path: &std::path::Path) -> u64 {
        let entry = self
            .buffer_content_versions
            .entry(path.to_path_buf())
            .or_insert(0);
        *entry += 1;
        *entry
    }

    /// Sends a `textDocument/inlayHint` request covering the whole document.
    ///
    /// Called after didOpen and after didChange. Silently no-ops if the server
    /// is not running or does not advertise the capability.
    pub(super) fn request_inlay_hints_for(&mut self, path: &std::path::Path, version: u64) {
        let line_count = match self.buffer_manager.active_buffer() {
            Some(buf) if buf.path() == Some(path) => buf.line_count(),
            _ => return,
        };
        if line_count == 0 {
            return;
        }
        let end_line = line_count.saturating_sub(1) as u32;
        if let Some(ref mut lsp) = self.lsp_manager {
            if let Err(e) = lsp.request_inlay_hints(path, version, 0, 0, end_line + 1, 0) {
                log::warn!("LSP inlay hint request failed: {e}");
            }
        }
    }

    /// Handles a `textDocument/publishDiagnostics` notification from an LSP server.
    ///
    /// Parses the params, converts LSP diagnostics to `BufferDiagnostic`, and stores
    /// them on the matching buffer.
    fn handle_publish_diagnostics(&mut self, params: &serde_json::Value) {
        let Ok(publish) =
            serde_json::from_value::<lsp_types::PublishDiagnosticsParams>(params.clone())
        else {
            log::warn!("Failed to parse publishDiagnostics params");
            return;
        };

        let Some(path) = uri_to_path(&publish.uri) else {
            log::warn!(
                "publishDiagnostics URI is not a file path: {:?}",
                publish.uri
            );
            return;
        };

        let diags = convert_lsp_diagnostics(&publish.diagnostics);

        if let Some(buf) = self.buffer_manager.buffer_mut_by_path(&path) {
            buf.set_diagnostics(diags);
        }
    }

    /// Jumps to the next diagnostic line in the active buffer, wrapping around.
    pub(super) fn go_to_next_diagnostic(&mut self) {
        let (h, w) = self.editor_viewport();
        if let Some(buf) = self.buffer_manager.active_buffer_mut() {
            let diags = buf.diagnostics();
            if diags.is_empty() {
                return;
            }
            let current_line = buf.cursor().row;
            // Find the first diagnostic line strictly after the cursor.
            let next = diags
                .iter()
                .map(|d| d.line)
                .find(|&l| l > current_line)
                .or_else(|| diags.iter().map(|d| d.line).min());
            if let Some(line) = next {
                buf.cursor_mut().row = line;
                buf.cursor_mut().col = 0;
                buf.ensure_cursor_visible(h, w);
            }
        }
    }

    /// Jumps to the previous diagnostic line in the active buffer, wrapping around.
    pub(super) fn go_to_prev_diagnostic(&mut self) {
        let (h, w) = self.editor_viewport();
        if let Some(buf) = self.buffer_manager.active_buffer_mut() {
            let diags = buf.diagnostics();
            if diags.is_empty() {
                return;
            }
            let current_line = buf.cursor().row;
            // Find the last diagnostic line strictly before the cursor.
            let prev = diags
                .iter()
                .map(|d| d.line)
                .rev()
                .find(|&l| l < current_line)
                .or_else(|| diags.iter().map(|d| d.line).max());
            if let Some(line) = prev {
                buf.cursor_mut().row = line;
                buf.cursor_mut().col = 0;
                buf.ensure_cursor_visible(h, w);
            }
        }
    }

    // IMPACT ANALYSIS — Completion methods
    // Parents: TriggerCompletion command, auto-trigger on '.' or ':'
    // Children: LspManager::request_completion, CompletionState, buffer edits
    // Siblings: Search bar (completion dismisses when search opens), overlays

    /// Sends a completion request to the LSP for the current cursor position.
    pub(super) fn request_completion(&mut self) {
        if let Some(ref mut lsp) = self.lsp_manager {
            if let Some(buf) = self.buffer_manager.active_buffer() {
                if let Some(path) = buf.path() {
                    let path = path.to_path_buf();
                    let line = buf.cursor().row as u32;
                    let col = buf.cursor().col as u32;
                    if let Err(e) = lsp.request_completion(&path, line, col) {
                        log::warn!("LSP completion request failed: {e}");
                    }
                }
            }
        }
    }

    /// Ensures the active buffer is promoted from preview and known to the LSP.
    ///
    /// Preview buffers are not sent to the LSP via `didOpen`. This method
    /// promotes the preview to a full buffer and notifies the LSP, so that
    /// features like Go To Definition work even when invoked from a preview.
    pub(super) fn ensure_lsp_open_for_active_buffer(&mut self) {
        // Promote preview buffer if needed.
        if let Some(buf) = self.buffer_manager.active_buffer() {
            if buf.is_preview {
                self.buffer_manager.promote_preview();
                // Notify LSP about the newly promoted file.
                if let Some(ref mut lsp) = self.lsp_manager {
                    if let Some(buf) = self.buffer_manager.active_buffer() {
                        if let Some(path) = buf.path() {
                            let text = buf.content_string();
                            if let Err(e) = lsp.file_opened(path, &text) {
                                log::warn!("LSP didOpen (from preview promote) failed: {e}");
                            }
                        }
                    }
                }
            }
        }
    }

    /// Sends a definition request to the LSP for the current cursor position.
    pub(super) fn request_definition(&mut self) {
        self.ensure_lsp_open_for_active_buffer();
        if let Some(ref mut lsp) = self.lsp_manager {
            if let Some(buf) = self.buffer_manager.active_buffer() {
                if let Some(path) = buf.path() {
                    let path = path.to_path_buf();
                    let line = buf.cursor().row as u32;
                    let col = buf.cursor().col as u32;
                    if let Err(e) = lsp.request_definition(&path, line, col) {
                        log::warn!("LSP definition request failed: {e}");
                    }
                }
            }
        }
    }

    /// Sends a references request to the LSP for the current cursor position.
    pub(super) fn request_references(&mut self) {
        self.ensure_lsp_open_for_active_buffer();
        if let Some(ref mut lsp) = self.lsp_manager {
            if let Some(buf) = self.buffer_manager.active_buffer() {
                if let Some(path) = buf.path() {
                    let path = path.to_path_buf();
                    let line = buf.cursor().row as u32;
                    let col = buf.cursor().col as u32;
                    if let Err(e) = lsp.request_references(&path, line, col) {
                        log::warn!("LSP references request failed: {e}");
                    }
                }
            }
        }
    }

    /// Sends a signature help request to the LSP for the current cursor position.
    ///
    /// Does nothing if the active buffer has no path or the language server
    /// does not advertise signature help support. The response arrives as
    /// `LspEvent::SignatureHelpResponse` via `poll_lsp()`.
    pub(super) fn request_signature_help(&mut self) {
        self.ensure_lsp_open_for_active_buffer();
        if let Some(ref mut lsp) = self.lsp_manager {
            if let Some(buf) = self.buffer_manager.active_buffer() {
                if let Some(path) = buf.path() {
                    let path = path.to_path_buf();
                    let line = buf.cursor().row as u32;
                    let col = buf.cursor().col as u32;
                    if let Err(e) = lsp.request_signature_help(&path, line, col) {
                        log::warn!("LSP signature help request failed: {e}");
                    }
                }
            }
        }
    }

    /// Returns the signature help trigger characters for the active buffer.
    pub(super) fn signature_help_trigger_chars(&self) -> Vec<char> {
        let Some(buf) = self.buffer_manager.active_buffer() else {
            return Vec::new();
        };
        let Some(path) = buf.path() else {
            return Vec::new();
        };
        self.lsp_manager
            .as_ref()
            .map(|lsp| lsp.signature_help_trigger_chars(path))
            .unwrap_or_default()
    }

    /// Requests code actions for the current cursor position.
    ///
    /// Builds a range covering just the cursor (LSP servers interpret a
    /// zero-width range as "at cursor") and forwards diagnostics overlapping
    /// the cursor line as the request context, so the server can surface
    /// quick fixes.
    pub(super) fn request_code_actions(&mut self) {
        self.ensure_lsp_open_for_active_buffer();
        let Some(buf) = self.buffer_manager.active_buffer() else {
            return;
        };
        let Some(path) = buf.path() else {
            self.set_status_message("Code actions: buffer has no file path".to_string());
            return;
        };
        let path = path.to_path_buf();
        let row = buf.cursor().row as u32;
        let col = buf.cursor().col as u32;

        // Collect diagnostics that cover the cursor line — LSP servers use
        // these as `context.diagnostics` when computing quick fixes.
        let diagnostics: Vec<serde_json::Value> = buf
            .diagnostics()
            .iter()
            .filter(|d| d.line == buf.cursor().row)
            .map(|d| {
                serde_json::json!({
                    "range": {
                        "start": { "line": d.line, "character": d.col_start },
                        "end":   { "line": d.line, "character": d.col_end },
                    },
                    "severity": match d.severity {
                        axe_editor::diagnostic::DiagnosticSeverity::Error => 1,
                        axe_editor::diagnostic::DiagnosticSeverity::Warning => 2,
                        axe_editor::diagnostic::DiagnosticSeverity::Info => 3,
                        axe_editor::diagnostic::DiagnosticSeverity::Hint => 4,
                    },
                    "message": d.message,
                    "source": d.source,
                    "code": d.code,
                })
            })
            .collect();

        if let Some(ref mut lsp) = self.lsp_manager {
            match lsp.request_code_actions(&path, row, col, row, col, diagnostics) {
                Ok(true) => {
                    self.set_status_message("Code actions: requesting…".to_string());
                    // Dismiss conflicting popups so the picker stands alone.
                    self.completion = None;
                    self.hover_info = None;
                    self.signature_help = None;
                }
                Ok(false) => {
                    self.set_status_message(
                        "Code actions not supported by this language server".to_string(),
                    );
                }
                Err(e) => {
                    log::warn!("LSP code actions request failed: {e}");
                    self.set_status_message("Code actions: request failed".to_string());
                }
            }
        } else {
            self.set_status_message("Code actions: no language server active".to_string());
        }
    }

    /// Applies the currently highlighted code action, honoring both the
    /// inline workspace edit (if present) and the server-side command.
    pub(super) fn apply_selected_code_action(&mut self) {
        let Some(state) = self.code_actions.as_ref() else {
            return;
        };
        let Some(action) = state.selected_action() else {
            return;
        };
        if !action.is_applicable() {
            if let Some(ref reason) = action.disabled_reason {
                self.set_status_message(format!("Code action disabled: {reason}"));
            }
            return;
        }

        let action = action.clone();
        self.code_actions = None;

        if let Some(ref edit) = action.edit {
            match self
                .buffer_manager
                .apply_workspace_edit(edit, "Code Action")
            {
                Ok(summary) => {
                    self.set_status_message(format!(
                        "Applied '{}' — {} edit(s) in {} file(s)",
                        action.title, summary.edits_applied, summary.files_affected
                    ));
                    self.last_edit_time = Some(Instant::now());
                    self.notify_lsp_change();
                }
                Err(e) => {
                    log::warn!("Failed to apply code action '{}': {e}", action.title);
                    self.set_status_message(format!("Code action failed: {}", action.title));
                    return;
                }
            }
        }

        if let Some(cmd) = action.command {
            let path = self
                .buffer_manager
                .active_buffer()
                .and_then(|b| b.path().map(|p| p.to_path_buf()));
            if let (Some(path), Some(ref mut lsp)) = (path, self.lsp_manager.as_mut()) {
                if let Err(e) = lsp.execute_command(&path, &cmd.command, cmd.arguments.clone()) {
                    log::warn!("workspace/executeCommand failed: {e}");
                }
            }
        }
    }

    /// Opens the inline rename dialog anchored at the word under the cursor.
    ///
    /// Pre-fills the input with the current word (if any) and positions the
    /// caret at the end. Does nothing if the buffer has no path, no word is
    /// at the cursor, or no LSP client is available.
    pub(super) fn start_rename(&mut self) {
        self.ensure_lsp_open_for_active_buffer();
        let Some(buf) = self.buffer_manager.active_buffer() else {
            return;
        };
        let Some(path) = buf.path() else {
            self.set_status_message("Rename: buffer has no file path".to_string());
            return;
        };
        let row = buf.cursor().row;
        let col = buf.cursor().col;
        let word = word_at_cursor(buf, row, col);
        let initial = word.unwrap_or_default();
        let state = crate::rename::RenameState::new(path.to_path_buf(), row, col, initial);
        self.rename = Some(state);
        // Dismiss conflicting popups so the rename input stands alone.
        self.completion = None;
        self.hover_info = None;
        self.signature_help = None;
    }

    /// Submits the pending rename state as a `textDocument/rename` request.
    pub(super) fn submit_rename(&mut self) {
        let Some(state) = self.rename.as_ref() else {
            return;
        };
        if !state.is_submittable() {
            return;
        }
        let path = state.path.clone();
        let row = state.origin_row as u32;
        let col = state.origin_col as u32;
        let new_name = state.input.clone();
        if let Some(ref mut lsp) = self.lsp_manager {
            match lsp.request_rename(&path, row, col, &new_name) {
                Ok(true) => {
                    self.set_status_message("Rename: requesting…".to_string());
                }
                Ok(false) => {
                    self.rename = None;
                    self.set_status_message(
                        "Rename not supported by this language server".to_string(),
                    );
                }
                Err(e) => {
                    log::warn!("LSP rename request failed: {e}");
                    self.rename = None;
                    self.set_status_message("Rename: request failed".to_string());
                }
            }
        } else {
            self.rename = None;
            self.set_status_message("Rename: no language server active".to_string());
        }
    }

    /// Auto-triggers or dismisses signature help in response to a typed
    /// character.
    ///
    /// Opens the popup when `ch` matches one of the server's advertised
    /// signature help trigger characters (typically `(` and `,`). Closes
    /// the popup when `ch` is a closing delimiter that ends the current
    /// call or bracket group.
    pub(super) fn maybe_auto_trigger_signature_help(&mut self, ch: char) {
        // Closing brackets always dismiss the popup.
        if matches!(ch, ')' | ']' | '}' | ';') {
            self.signature_help = None;
            return;
        }
        let triggers = self.signature_help_trigger_chars();
        if triggers.contains(&ch) {
            self.request_signature_help();
        }
    }

    /// Sends a hover request to the LSP for the current cursor position.
    pub(super) fn request_hover(&mut self) {
        self.ensure_lsp_open_for_active_buffer();
        if let Some(ref mut lsp) = self.lsp_manager {
            if let Some(buf) = self.buffer_manager.active_buffer() {
                if let Some(path) = buf.path() {
                    let path = path.to_path_buf();
                    let line = buf.cursor().row as u32;
                    let col = buf.cursor().col as u32;
                    if let Err(e) = lsp.request_hover(&path, line, col) {
                        log::warn!("LSP hover request failed: {e}");
                    }
                }
            }
        }
    }

    // IMPACT ANALYSIS — request_format_for_active_buffer
    // Parents: Command::FormatDocument, Command::EditorSave (when format_on_save)
    // Children: LspManager::request_formatting() sends textDocument/formatting
    // Siblings: ensure_lsp_open_for_active_buffer (same pattern as request_hover)
    // Risk: Returns false if LSP not available — callers must handle gracefully

    /// Sends a formatting request to the LSP for the active buffer.
    ///
    /// Returns `true` if the request was sent, `false` if formatting is
    /// unavailable (no LSP, no buffer, or server doesn't support formatting).
    pub(super) fn request_format_for_active_buffer(&mut self) -> bool {
        self.ensure_lsp_open_for_active_buffer();
        if let Some(ref mut lsp) = self.lsp_manager {
            if let Some(buf) = self.buffer_manager.active_buffer() {
                if let Some(path) = buf.path() {
                    let path = path.to_path_buf();
                    let tab_size = self.config.editor.tab_size as u32;
                    let insert_spaces = self.config.editor.insert_spaces;
                    match lsp.request_formatting(&path, tab_size, insert_spaces) {
                        Ok(sent) => return sent,
                        Err(e) => {
                            log::warn!("LSP formatting request failed: {e}");
                        }
                    }
                }
            }
        }
        false
    }

    // IMPACT ANALYSIS — apply_formatting_edits
    // Parents: poll_lsp() FormattingResponse handler
    // Children: EditorBuffer::apply_text_edit() modifies rope content
    // Siblings: Selection (cleared by apply_text_edit), cursor (repositioned),
    //           diagnostics (shifted by LSP after didChange)
    // Risk: Edits must be applied in reverse order to preserve line/col offsets

    /// Applies LSP formatting text edits to the active buffer.
    ///
    /// Parses `TextEdit[]` from the response value, sorts in reverse document
    /// order (end position descending), and applies each edit.
    pub(super) fn apply_formatting_edits(&mut self, value: &serde_json::Value) {
        let Some(edits) = value.as_array() else {
            return;
        };

        // Collect and sort edits in reverse document order so earlier edits
        // don't invalidate the positions of later ones.
        let mut parsed_edits: Vec<(usize, usize, usize, usize, String)> = edits
            .iter()
            .filter_map(|edit| {
                let range = edit.get("range")?;
                let start = range.get("start")?;
                let end = range.get("end")?;
                let new_text = edit.get("newText")?.as_str()?.to_string();
                Some((
                    start.get("line")?.as_u64()? as usize,
                    start.get("character")?.as_u64()? as usize,
                    end.get("line")?.as_u64()? as usize,
                    end.get("character")?.as_u64()? as usize,
                    new_text,
                ))
            })
            .collect();

        // Sort by end position descending (reverse document order).
        parsed_edits.sort_by(|a, b| b.2.cmp(&a.2).then_with(|| b.3.cmp(&a.3)));

        if let Some(buf) = self.buffer_manager.active_buffer_mut() {
            for (start_line, start_col, end_line, end_col, new_text) in parsed_edits {
                buf.apply_text_edit(start_line, start_col, end_line, end_col, &new_text);
            }
        }
    }
}

/// Returns the word at the given (row, col) in the buffer, if any.
///
/// A word is a run of alphanumeric characters plus `_`. Returns `None`
/// when the position is outside the buffer, on whitespace, or on an
/// empty line.
fn word_at_cursor(buf: &axe_editor::EditorBuffer, row: usize, col: usize) -> Option<String> {
    let slice = buf.line_at(row)?;
    let line_text: String = slice.to_string();
    let trimmed = line_text.trim_end_matches('\n').trim_end_matches('\r');
    let chars: Vec<char> = trimmed.chars().collect();
    if chars.is_empty() || col > chars.len() {
        return None;
    }
    let is_word = |c: char| c.is_alphanumeric() || c == '_';
    // When the cursor sits just after a word, step back one to find it.
    let pivot = if col == chars.len() || !is_word(chars[col]) {
        if col > 0 && is_word(chars[col - 1]) {
            col - 1
        } else {
            return None;
        }
    } else {
        col
    };
    let mut start = pivot;
    while start > 0 && is_word(chars[start - 1]) {
        start -= 1;
    }
    let mut end = pivot;
    while end < chars.len() && is_word(chars[end]) {
        end += 1;
    }
    if end <= start {
        return None;
    }
    Some(chars[start..end].iter().collect())
}

/// Converts an `lsp_types::Uri` to a `PathBuf`, if it has a `file` scheme.
pub(crate) fn uri_to_path(uri: &lsp_types::Uri) -> Option<std::path::PathBuf> {
    let s = uri.as_str();
    let url = url::Url::parse(s).ok()?;
    if url.scheme() != "file" {
        return None;
    }
    url.to_file_path().ok()
}

/// Converts LSP diagnostics to internal `BufferDiagnostic` format.
///
/// Pure function — no side effects. Maps `lsp_types::DiagnosticSeverity` to
/// `DiagnosticSeverity`, defaulting to `Warning` when severity is absent.
pub(crate) fn convert_lsp_diagnostics(
    lsp_diags: &[lsp_types::Diagnostic],
) -> Vec<BufferDiagnostic> {
    lsp_diags
        .iter()
        .map(|d| {
            let severity = match d.severity {
                Some(lsp_types::DiagnosticSeverity::ERROR) => DiagnosticSeverity::Error,
                Some(lsp_types::DiagnosticSeverity::WARNING) => DiagnosticSeverity::Warning,
                Some(lsp_types::DiagnosticSeverity::INFORMATION) => DiagnosticSeverity::Info,
                Some(lsp_types::DiagnosticSeverity::HINT) => DiagnosticSeverity::Hint,
                _ => DiagnosticSeverity::Warning,
            };

            let code = d.code.as_ref().map(|c| match c {
                lsp_types::NumberOrString::Number(n) => n.to_string(),
                lsp_types::NumberOrString::String(s) => s.clone(),
            });

            BufferDiagnostic {
                line: d.range.start.line as usize,
                col_start: d.range.start.character as usize,
                col_end: d.range.end.character as usize,
                severity,
                message: d.message.clone(),
                source: d.source.clone(),
                code,
            }
        })
        .collect()
}
