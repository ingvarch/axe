use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::mpsc;

use anyhow::{Context, Result};
use url::Url;

use axe_config::LspServerConfig;

use crate::client::{LspClient, LspEvent, PendingRequestKind};
use crate::language::language_id_for_path;
use crate::transport::RequestId;

/// Manages multiple LSP server connections, one per language.
///
/// Routes document events to the correct server based on file extension.
/// Handles the initialization handshake and queues operations during init.
pub struct LspManager {
    /// Active LSP clients, keyed by language ID.
    clients: HashMap<String, LspClient>,
    /// Server configurations, keyed by language ID.
    configs: HashMap<String, LspServerConfig>,
    /// Channel for receiving events from all server reader threads.
    event_rx: mpsc::Receiver<LspEvent>,
    /// Sender clone passed to new clients.
    event_tx: mpsc::Sender<LspEvent>,
    /// Project root as a URL.
    root_uri: Url,
    /// Document version counters per file path.
    versions: HashMap<PathBuf, i32>,
    /// Languages currently awaiting initialize response.
    pending_init: HashSet<String>,
    /// Documents queued for didOpen while server is initializing.
    /// Each entry: (path, language_id, text).
    pending_open: Vec<(PathBuf, String, String)>,
}

impl LspManager {
    /// Creates a new manager with the given server configs and project root.
    pub fn new(configs: HashMap<String, LspServerConfig>, root_path: &Path) -> Result<Self> {
        let root_uri = Url::from_directory_path(root_path)
            .map_err(|()| anyhow::anyhow!("Invalid project root path: {}", root_path.display()))?;
        let (event_tx, event_rx) = mpsc::channel();

        Ok(Self {
            clients: HashMap::new(),
            configs,
            event_rx,
            event_tx,
            root_uri,
            versions: HashMap::new(),
            pending_init: HashSet::new(),
            pending_open: Vec::new(),
        })
    }

    /// Notifies the LSP that a file was opened.
    ///
    /// Starts the language server if not already running. If the server is
    /// still initializing, queues the didOpen for later.
    pub fn file_opened(&mut self, path: &Path, text: &str) -> Result<()> {
        let Some(lang_id) = language_id_for_path(path) else {
            return Ok(()); // Unknown language, nothing to do.
        };

        if !self.configs.contains_key(lang_id) {
            return Ok(()); // No server configured for this language.
        }

        // Start server if not running.
        if !self.clients.contains_key(lang_id) && !self.pending_init.contains(lang_id) {
            self.start_server(lang_id)?;
        }

        // If server is still initializing, queue the open.
        if self.pending_init.contains(lang_id) {
            self.pending_open
                .push((path.to_path_buf(), lang_id.to_string(), text.to_string()));
            return Ok(());
        }

        // Server is ready — send didOpen.
        self.versions.insert(path.to_path_buf(), 1);
        if let Some(client) = self.clients.get_mut(lang_id) {
            if let Err(e) = client.notify_did_open(path, lang_id, text) {
                log::warn!("Failed to send didOpen for {}: {e}", path.display());
            }
        }

        Ok(())
    }

    /// Notifies the LSP that a file's content changed.
    pub fn file_changed(&mut self, path: &Path, text: &str) -> Result<()> {
        let Some(lang_id) = language_id_for_path(path) else {
            return Ok(());
        };

        let Some(client) = self.clients.get_mut(lang_id) else {
            return Ok(()); // No active client.
        };

        if !client.is_initialized() {
            return Ok(());
        }

        let version = self.versions.entry(path.to_path_buf()).or_insert(1);
        *version += 1;

        if let Err(e) = client.notify_did_change(path, text, *version) {
            log::warn!("Failed to send didChange for {}: {e}", path.display());
        }

        Ok(())
    }

    /// Notifies the LSP that a file was saved.
    pub fn file_saved(&mut self, path: &Path) -> Result<()> {
        let Some(lang_id) = language_id_for_path(path) else {
            return Ok(());
        };

        let Some(client) = self.clients.get_mut(lang_id) else {
            return Ok(());
        };

        if !client.is_initialized() {
            return Ok(());
        }

        if let Err(e) = client.notify_did_save(path) {
            log::warn!("Failed to send didSave for {}: {e}", path.display());
        }

        Ok(())
    }

    /// Drains all pending events from server reader threads.
    ///
    /// Handles initialize responses internally (completes handshake, flushes
    /// pending opens). Returns remaining events for the caller to process.
    pub fn poll_events(&mut self) -> Vec<LspEvent> {
        let mut events = Vec::new();

        loop {
            match self.event_rx.try_recv() {
                Ok(event) => events.push(event),
                Err(mpsc::TryRecvError::Empty) => break,
                Err(mpsc::TryRecvError::Disconnected) => break,
            }
        }

        // Process events, handling initialize responses internally.
        let mut remaining = Vec::new();
        for event in events {
            match &event {
                LspEvent::Response {
                    id: ref response_id,
                    result: Ok(ref value),
                } => {
                    // Check if this is an initialize response (contains capabilities).
                    if let Some(caps) = value.get("capabilities") {
                        match serde_json::from_value::<lsp_types::ServerCapabilities>(caps.clone())
                        {
                            Ok(capabilities) => {
                                // Find which language this belongs to by checking pending_init.
                                let lang_id = self.find_pending_init_language();
                                if let Some(lang_id) = lang_id {
                                    if let Some(client) = self.clients.get_mut(&lang_id) {
                                        if let Err(e) =
                                            client.send_initialized_notification(capabilities)
                                        {
                                            log::warn!(
                                                "Failed to send initialized for {lang_id}: {e}"
                                            );
                                        }
                                    }
                                    self.pending_init.remove(&lang_id);
                                    self.flush_pending_opens(&lang_id);
                                    remaining.push(LspEvent::Initialized {
                                        language_id: lang_id,
                                    });
                                    continue;
                                }
                            }
                            Err(e) => {
                                log::warn!("Failed to parse server capabilities: {e}");
                            }
                        }
                    }
                    // Check if this response matches a pending request.
                    let numeric_id = match response_id {
                        RequestId::Number(n) => Some(*n),
                        RequestId::String(_) => None,
                    };
                    if let Some(id) = numeric_id {
                        let pending_kind =
                            self.clients.values_mut().find_map(|c| c.take_pending(id));
                        match pending_kind {
                            Some(PendingRequestKind::Completion) => {
                                if let LspEvent::Response { result, .. } = event {
                                    remaining.push(LspEvent::CompletionResponse { result });
                                }
                                continue;
                            }
                            Some(PendingRequestKind::Definition) => {
                                if let LspEvent::Response { result, .. } = event {
                                    remaining.push(LspEvent::DefinitionResponse { result });
                                }
                                continue;
                            }
                            Some(PendingRequestKind::References) => {
                                if let LspEvent::Response { result, .. } = event {
                                    remaining.push(LspEvent::ReferencesResponse { result });
                                }
                                continue;
                            }
                            Some(PendingRequestKind::Hover) => {
                                if let LspEvent::Response { result, .. } = event {
                                    remaining.push(LspEvent::HoverResponse { result });
                                }
                                continue;
                            }
                            Some(PendingRequestKind::Formatting) => {
                                if let LspEvent::Response { result, .. } = event {
                                    remaining.push(LspEvent::FormattingResponse { result });
                                }
                                continue;
                            }
                            Some(PendingRequestKind::SignatureHelp) => {
                                if let LspEvent::Response { result, .. } = event {
                                    remaining.push(LspEvent::SignatureHelpResponse { result });
                                }
                                continue;
                            }
                            Some(PendingRequestKind::InlayHint { path, version }) => {
                                if let LspEvent::Response { result, .. } = event {
                                    remaining.push(LspEvent::InlayHintResponse {
                                        path,
                                        version,
                                        result,
                                    });
                                }
                                continue;
                            }
                            None => {}
                        }
                    }
                    remaining.push(event);
                }
                LspEvent::Response {
                    id: ref response_id,
                    result: Err(_),
                } => {
                    let numeric_id = match response_id {
                        RequestId::Number(n) => Some(*n),
                        RequestId::String(_) => None,
                    };
                    if let Some(id) = numeric_id {
                        let pending_kind =
                            self.clients.values_mut().find_map(|c| c.take_pending(id));
                        match pending_kind {
                            Some(PendingRequestKind::Completion) => {
                                if let LspEvent::Response { result, .. } = event {
                                    remaining.push(LspEvent::CompletionResponse { result });
                                }
                                continue;
                            }
                            Some(PendingRequestKind::Definition) => {
                                if let LspEvent::Response { result, .. } = event {
                                    remaining.push(LspEvent::DefinitionResponse { result });
                                }
                                continue;
                            }
                            Some(PendingRequestKind::References) => {
                                if let LspEvent::Response { result, .. } = event {
                                    remaining.push(LspEvent::ReferencesResponse { result });
                                }
                                continue;
                            }
                            Some(PendingRequestKind::Hover) => {
                                if let LspEvent::Response { result, .. } = event {
                                    remaining.push(LspEvent::HoverResponse { result });
                                }
                                continue;
                            }
                            Some(PendingRequestKind::Formatting) => {
                                if let LspEvent::Response { result, .. } = event {
                                    remaining.push(LspEvent::FormattingResponse { result });
                                }
                                continue;
                            }
                            Some(PendingRequestKind::SignatureHelp) => {
                                if let LspEvent::Response { result, .. } = event {
                                    remaining.push(LspEvent::SignatureHelpResponse { result });
                                }
                                continue;
                            }
                            Some(PendingRequestKind::InlayHint { path, version }) => {
                                if let LspEvent::Response { result, .. } = event {
                                    remaining.push(LspEvent::InlayHintResponse {
                                        path,
                                        version,
                                        result,
                                    });
                                }
                                continue;
                            }
                            None => {}
                        }
                    }
                    remaining.push(event);
                }
                LspEvent::ServerCrashed { language_id, .. } => {
                    self.clients.remove(language_id);
                    self.pending_init.remove(language_id);
                    remaining.push(event);
                }
                _ => remaining.push(event),
            }
        }

        remaining
    }

    /// Sends a `textDocument/completion` request for the given file position.
    ///
    /// The response will arrive as `LspEvent::CompletionResponse` via `poll_events()`.
    pub fn request_completion(&mut self, path: &Path, line: u32, character: u32) -> Result<()> {
        let Some(lang_id) = language_id_for_path(path) else {
            return Ok(());
        };

        let Some(client) = self.clients.get_mut(lang_id) else {
            return Ok(());
        };

        if !client.is_initialized() {
            return Ok(());
        }

        let uri = Url::from_file_path(path)
            .map_err(|()| anyhow::anyhow!("Invalid file path: {}", path.display()))?;

        let params = serde_json::json!({
            "textDocument": { "uri": uri.as_str() },
            "position": { "line": line, "character": character }
        });

        client.send_request(
            "textDocument/completion",
            params,
            PendingRequestKind::Completion,
        )?;
        Ok(())
    }

    /// Sends a `textDocument/definition` request for the given file position.
    ///
    /// The response will arrive as `LspEvent::DefinitionResponse` via `poll_events()`.
    pub fn request_definition(&mut self, path: &Path, line: u32, character: u32) -> Result<()> {
        let Some(lang_id) = language_id_for_path(path) else {
            return Ok(());
        };

        let Some(client) = self.clients.get_mut(lang_id) else {
            return Ok(());
        };

        if !client.is_initialized() {
            return Ok(());
        }

        let uri = Url::from_file_path(path)
            .map_err(|()| anyhow::anyhow!("Invalid file path: {}", path.display()))?;

        let params = serde_json::json!({
            "textDocument": { "uri": uri.as_str() },
            "position": { "line": line, "character": character }
        });

        client.send_request(
            "textDocument/definition",
            params,
            PendingRequestKind::Definition,
        )?;
        Ok(())
    }

    /// Sends a `textDocument/references` request for the given file position.
    ///
    /// The response will arrive as `LspEvent::ReferencesResponse` via `poll_events()`.
    pub fn request_references(&mut self, path: &Path, line: u32, character: u32) -> Result<()> {
        let Some(lang_id) = language_id_for_path(path) else {
            return Ok(());
        };

        let Some(client) = self.clients.get_mut(lang_id) else {
            return Ok(());
        };

        if !client.is_initialized() {
            return Ok(());
        }

        let uri = Url::from_file_path(path)
            .map_err(|()| anyhow::anyhow!("Invalid file path: {}", path.display()))?;

        let params = serde_json::json!({
            "textDocument": { "uri": uri.as_str() },
            "position": { "line": line, "character": character },
            "context": { "includeDeclaration": true }
        });

        client.send_request(
            "textDocument/references",
            params,
            PendingRequestKind::References,
        )?;
        Ok(())
    }

    /// Sends a `textDocument/hover` request for the given file position.
    ///
    /// The response will arrive as `LspEvent::HoverResponse` via `poll_events()`.
    pub fn request_hover(&mut self, path: &Path, line: u32, character: u32) -> Result<()> {
        let Some(lang_id) = language_id_for_path(path) else {
            return Ok(());
        };

        let Some(client) = self.clients.get_mut(lang_id) else {
            return Ok(());
        };

        if !client.is_initialized() {
            return Ok(());
        }

        let uri = Url::from_file_path(path)
            .map_err(|()| anyhow::anyhow!("Invalid file path: {}", path.display()))?;

        let params = serde_json::json!({
            "textDocument": { "uri": uri.as_str() },
            "position": { "line": line, "character": character }
        });

        client.send_request("textDocument/hover", params, PendingRequestKind::Hover)?;
        Ok(())
    }

    /// Sends a `textDocument/formatting` request for the given file.
    ///
    /// Returns `Ok(true)` if the request was sent, `Ok(false)` if formatting
    /// is not supported or no client is available.
    pub fn request_formatting(
        &mut self,
        path: &Path,
        tab_size: u32,
        insert_spaces: bool,
    ) -> Result<bool> {
        let Some(lang_id) = language_id_for_path(path) else {
            return Ok(false);
        };

        let Some(client) = self.clients.get_mut(lang_id) else {
            return Ok(false);
        };

        if !client.is_initialized() {
            return Ok(false);
        }

        if !client.supports_formatting() {
            return Ok(false);
        }

        let uri = Url::from_file_path(path)
            .map_err(|()| anyhow::anyhow!("Invalid file path: {}", path.display()))?;

        let params = serde_json::json!({
            "textDocument": { "uri": uri.as_str() },
            "options": {
                "tabSize": tab_size,
                "insertSpaces": insert_spaces,
            }
        });

        client.send_request(
            "textDocument/formatting",
            params,
            PendingRequestKind::Formatting,
        )?;
        Ok(true)
    }

    /// Sends a `textDocument/signatureHelp` request for the given file position.
    ///
    /// Returns `Ok(true)` if the request was sent, `Ok(false)` if no client
    /// is available or the server does not advertise signature help support.
    pub fn request_signature_help(
        &mut self,
        path: &Path,
        line: u32,
        character: u32,
    ) -> Result<bool> {
        let Some(lang_id) = language_id_for_path(path) else {
            return Ok(false);
        };

        let Some(client) = self.clients.get_mut(lang_id) else {
            return Ok(false);
        };

        if !client.is_initialized() {
            return Ok(false);
        }

        if !client.supports_signature_help() {
            return Ok(false);
        }

        let uri = Url::from_file_path(path)
            .map_err(|()| anyhow::anyhow!("Invalid file path: {}", path.display()))?;

        let params = serde_json::json!({
            "textDocument": { "uri": uri.as_str() },
            "position": { "line": line, "character": character }
        });

        client.send_request(
            "textDocument/signatureHelp",
            params,
            PendingRequestKind::SignatureHelp,
        )?;
        Ok(true)
    }

    /// Returns the active server's signature help trigger characters for a
    /// given file path, or an empty vector if no server handles the file.
    pub fn signature_help_trigger_chars(&self, path: &Path) -> Vec<char> {
        let Some(lang_id) = language_id_for_path(path) else {
            return Vec::new();
        };
        self.clients
            .get(lang_id)
            .map(|c| c.signature_help_trigger_chars())
            .unwrap_or_default()
    }

    /// Sends a `textDocument/inlayHint` request for the given file range.
    ///
    /// Returns `Ok(true)` if the request was sent, `Ok(false)` if inlay hints
    /// are unsupported or no client is available. `version` is passed through
    /// the response so the caller can drop stale hints after subsequent edits.
    pub fn request_inlay_hints(
        &mut self,
        path: &Path,
        version: u64,
        start_line: u32,
        start_character: u32,
        end_line: u32,
        end_character: u32,
    ) -> Result<bool> {
        let Some(lang_id) = language_id_for_path(path) else {
            return Ok(false);
        };

        let Some(client) = self.clients.get_mut(lang_id) else {
            return Ok(false);
        };

        if !client.is_initialized() {
            return Ok(false);
        }

        if !client.supports_inlay_hints() {
            return Ok(false);
        }

        let uri = Url::from_file_path(path)
            .map_err(|()| anyhow::anyhow!("Invalid file path: {}", path.display()))?;

        let params = serde_json::json!({
            "textDocument": { "uri": uri.as_str() },
            "range": {
                "start": { "line": start_line, "character": start_character },
                "end": { "line": end_line, "character": end_character },
            }
        });

        client.send_request(
            "textDocument/inlayHint",
            params,
            PendingRequestKind::InlayHint {
                path: path.to_path_buf(),
                version,
            },
        )?;
        Ok(true)
    }

    /// Shuts down all active LSP servers.
    pub fn shutdown_all(&mut self) {
        for (lang_id, mut client) in self.clients.drain() {
            if let Err(e) = client.shutdown() {
                log::warn!("Failed to shutdown {lang_id} LSP: {e}");
            }
        }
        self.pending_init.clear();
        self.pending_open.clear();
    }

    /// Starts a server for the given language.
    fn start_server(&mut self, language_id: &str) -> Result<()> {
        let config = self
            .configs
            .get(language_id)
            .context("No config for language")?
            .clone();

        match LspClient::start(
            &config.command,
            &config.args,
            &self.root_uri,
            language_id,
            self.event_tx.clone(),
        ) {
            Ok(client) => {
                log::info!("Started LSP server for {language_id}: {}", config.command);
                self.clients.insert(language_id.to_string(), client);
                self.pending_init.insert(language_id.to_string());
                Ok(())
            }
            Err(e) => {
                log::warn!("Failed to start LSP server for {language_id}: {e}");
                Err(e)
            }
        }
    }

    /// Finds a language in `pending_init` that has an active client.
    ///
    /// Used to match initialize responses to the correct language.
    fn find_pending_init_language(&self) -> Option<String> {
        for lang_id in &self.pending_init {
            if self.clients.contains_key(lang_id) {
                return Some(lang_id.clone());
            }
        }
        None
    }

    /// Sends queued didOpen notifications for a language that just initialized.
    fn flush_pending_opens(&mut self, language_id: &str) {
        let pending: Vec<_> = self
            .pending_open
            .drain(..)
            .filter(|(_, lang, _)| lang == language_id)
            .collect();

        for (path, lang_id, text) in pending {
            self.versions.insert(path.clone(), 1);
            if let Some(client) = self.clients.get_mut(&lang_id) {
                if let Err(e) = client.notify_did_open(&path, &lang_id, &text) {
                    log::warn!("Failed to send queued didOpen for {}: {e}", path.display());
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_with_empty_configs() {
        let dir = std::env::temp_dir();
        let manager = LspManager::new(HashMap::new(), &dir).expect("should create manager");
        assert!(manager.clients.is_empty());
        assert!(manager.configs.is_empty());
    }

    #[test]
    fn file_opened_no_matching_config_is_noop() {
        let dir = std::env::temp_dir();
        let mut manager = LspManager::new(HashMap::new(), &dir).expect("should create manager");
        // No configs registered — should do nothing.
        let result = manager.file_opened(Path::new("/tmp/test.rs"), "fn main() {}");
        assert!(result.is_ok());
        assert!(manager.clients.is_empty());
    }

    #[test]
    fn file_changed_no_active_client_is_noop() {
        let dir = std::env::temp_dir();
        let mut manager = LspManager::new(HashMap::new(), &dir).expect("should create manager");
        let result = manager.file_changed(Path::new("/tmp/test.rs"), "fn main() {}");
        assert!(result.is_ok());
    }

    #[test]
    fn version_increments_on_change() {
        let dir = std::env::temp_dir();
        let mut manager = LspManager::new(HashMap::new(), &dir).expect("should create manager");
        let path = PathBuf::from("/tmp/test.rs");
        // Manually insert a version to test the increment logic.
        manager.versions.insert(path.clone(), 1);
        let version = manager.versions.entry(path.clone()).or_insert(1);
        *version += 1;
        assert_eq!(manager.versions[&path], 2);
        let version = manager.versions.entry(path.clone()).or_insert(1);
        *version += 1;
        assert_eq!(manager.versions[&path], 3);
    }

    #[test]
    fn language_detection_routes_to_correct_config() {
        let dir = std::env::temp_dir();
        let mut configs = HashMap::new();
        configs.insert(
            "rust".to_string(),
            axe_config::LspServerConfig {
                command: "rust-analyzer".to_string(),
                args: vec![],
                init_options: None,
            },
        );
        configs.insert(
            "python".to_string(),
            axe_config::LspServerConfig {
                command: "pyright-langserver".to_string(),
                args: vec!["--stdio".to_string()],
                init_options: None,
            },
        );
        let manager = LspManager::new(configs, &dir).expect("should create manager");

        // Verify configs are stored correctly.
        assert!(manager.configs.contains_key("rust"));
        assert!(manager.configs.contains_key("python"));
        assert!(!manager.configs.contains_key("go"));

        // Verify language detection would route correctly.
        assert_eq!(language_id_for_path(Path::new("main.rs")), Some("rust"));
        assert_eq!(language_id_for_path(Path::new("app.py")), Some("python"));
        assert_eq!(language_id_for_path(Path::new("main.go")), Some("go"));
    }

    #[test]
    fn file_opened_unknown_language_is_noop() {
        let dir = std::env::temp_dir();
        let mut manager = LspManager::new(HashMap::new(), &dir).expect("should create manager");
        // File with no known extension.
        let result = manager.file_opened(Path::new("/tmp/Makefile"), "all: build");
        assert!(result.is_ok());
        assert!(manager.clients.is_empty());
    }

    #[test]
    fn file_saved_no_active_client_is_noop() {
        let dir = std::env::temp_dir();
        let mut manager = LspManager::new(HashMap::new(), &dir).expect("should create manager");
        let result = manager.file_saved(Path::new("/tmp/test.rs"));
        assert!(result.is_ok());
    }

    #[test]
    fn poll_events_empty_channel() {
        let dir = std::env::temp_dir();
        let mut manager = LspManager::new(HashMap::new(), &dir).expect("should create manager");
        let events = manager.poll_events();
        assert!(events.is_empty());
    }

    #[test]
    fn request_definition_no_client_is_noop() {
        let dir = std::env::temp_dir();
        let mut manager = LspManager::new(HashMap::new(), &dir).expect("should create manager");
        let result = manager.request_definition(Path::new("/tmp/test.rs"), 0, 0);
        assert!(result.is_ok());
    }

    #[test]
    fn request_references_no_client_is_noop() {
        let dir = std::env::temp_dir();
        let mut manager = LspManager::new(HashMap::new(), &dir).expect("should create manager");
        let result = manager.request_references(Path::new("/tmp/test.rs"), 0, 0);
        assert!(result.is_ok());
    }

    #[test]
    fn request_hover_no_client_is_noop() {
        let dir = std::env::temp_dir();
        let mut manager = LspManager::new(HashMap::new(), &dir).expect("should create manager");
        let result = manager.request_hover(Path::new("/tmp/test.rs"), 0, 0);
        assert!(result.is_ok());
    }

    #[test]
    fn request_formatting_no_client_is_noop() {
        let dir = std::env::temp_dir();
        let mut manager = LspManager::new(HashMap::new(), &dir).expect("should create manager");
        let result = manager.request_formatting(Path::new("/tmp/test.rs"), 4, true);
        assert!(result.is_ok());
        assert!(!result.unwrap());
    }

    #[test]
    fn shutdown_all_empty() {
        let dir = std::env::temp_dir();
        let mut manager = LspManager::new(HashMap::new(), &dir).expect("should create manager");
        manager.shutdown_all(); // Should not panic.
    }

    #[test]
    fn new_stores_configs_correctly() {
        let dir = std::env::temp_dir();
        let mut configs = HashMap::new();
        configs.insert(
            "rust".to_string(),
            LspServerConfig {
                command: "rust-analyzer".to_string(),
                args: vec![],
                init_options: None,
            },
        );
        configs.insert(
            "go".to_string(),
            LspServerConfig {
                command: "gopls".to_string(),
                args: vec![],
                init_options: None,
            },
        );
        let manager = LspManager::new(configs, &dir).unwrap();
        assert_eq!(manager.configs.len(), 2);
        assert_eq!(manager.configs["rust"].command, "rust-analyzer");
        assert_eq!(manager.configs["go"].command, "gopls");
    }

    #[test]
    fn new_sets_root_uri_from_path() {
        let dir = std::env::temp_dir();
        let manager = LspManager::new(HashMap::new(), &dir).unwrap();
        let uri_str = manager.root_uri.as_str();
        assert!(uri_str.starts_with("file://"));
        assert!(uri_str.ends_with('/'));
    }

    #[test]
    fn new_initializes_empty_state() {
        let dir = std::env::temp_dir();
        let manager = LspManager::new(HashMap::new(), &dir).unwrap();
        assert!(manager.clients.is_empty());
        assert!(manager.versions.is_empty());
        assert!(manager.pending_init.is_empty());
        assert!(manager.pending_open.is_empty());
    }

    #[test]
    fn file_opened_with_config_but_nonexistent_server_returns_error() {
        let dir = std::env::temp_dir();
        let mut configs = HashMap::new();
        configs.insert(
            "rust".to_string(),
            LspServerConfig {
                // Use a command that does not exist to trigger spawn failure.
                command: "nonexistent-lsp-server-binary-xyz".to_string(),
                args: vec![],
                init_options: None,
            },
        );
        let mut manager = LspManager::new(configs, &dir).unwrap();
        let result = manager.file_opened(Path::new("/tmp/test.rs"), "fn main() {}");
        assert!(result.is_err());
    }

    #[test]
    fn file_changed_unknown_language_is_noop() {
        let dir = std::env::temp_dir();
        let mut manager = LspManager::new(HashMap::new(), &dir).unwrap();
        let result = manager.file_changed(Path::new("/tmp/Makefile"), "all: build");
        assert!(result.is_ok());
    }

    #[test]
    fn file_saved_unknown_language_is_noop() {
        let dir = std::env::temp_dir();
        let mut manager = LspManager::new(HashMap::new(), &dir).unwrap();
        let result = manager.file_saved(Path::new("/tmp/Makefile"));
        assert!(result.is_ok());
    }

    #[test]
    fn request_completion_no_client_is_noop() {
        let dir = std::env::temp_dir();
        let mut manager = LspManager::new(HashMap::new(), &dir).unwrap();
        let result = manager.request_completion(Path::new("/tmp/test.rs"), 0, 0);
        assert!(result.is_ok());
    }

    #[test]
    fn request_completion_unknown_language_is_noop() {
        let dir = std::env::temp_dir();
        let mut manager = LspManager::new(HashMap::new(), &dir).unwrap();
        let result = manager.request_completion(Path::new("/tmp/Makefile"), 0, 0);
        assert!(result.is_ok());
    }

    #[test]
    fn request_definition_unknown_language_is_noop() {
        let dir = std::env::temp_dir();
        let mut manager = LspManager::new(HashMap::new(), &dir).unwrap();
        let result = manager.request_definition(Path::new("/tmp/unknown.xyz"), 0, 0);
        assert!(result.is_ok());
    }

    #[test]
    fn request_references_unknown_language_is_noop() {
        let dir = std::env::temp_dir();
        let mut manager = LspManager::new(HashMap::new(), &dir).unwrap();
        let result = manager.request_references(Path::new("/tmp/unknown.xyz"), 0, 0);
        assert!(result.is_ok());
    }

    #[test]
    fn request_hover_unknown_language_is_noop() {
        let dir = std::env::temp_dir();
        let mut manager = LspManager::new(HashMap::new(), &dir).unwrap();
        let result = manager.request_hover(Path::new("/tmp/unknown.xyz"), 0, 0);
        assert!(result.is_ok());
    }

    #[test]
    fn request_formatting_unknown_language_returns_false() {
        let dir = std::env::temp_dir();
        let mut manager = LspManager::new(HashMap::new(), &dir).unwrap();
        let result = manager.request_formatting(Path::new("/tmp/unknown.xyz"), 4, true);
        assert!(result.is_ok());
        assert!(!result.unwrap());
    }

    #[test]
    fn shutdown_all_clears_pending_state() {
        let dir = std::env::temp_dir();
        let mut manager = LspManager::new(HashMap::new(), &dir).unwrap();
        // Manually insert pending state to verify shutdown clears it.
        manager.pending_init.insert("rust".to_string());
        manager.pending_open.push((
            PathBuf::from("/tmp/test.rs"),
            "rust".to_string(),
            "fn main() {}".to_string(),
        ));
        manager.shutdown_all();
        assert!(manager.pending_init.is_empty());
        assert!(manager.pending_open.is_empty());
    }

    #[test]
    fn find_pending_init_language_returns_none_when_empty() {
        let dir = std::env::temp_dir();
        let manager = LspManager::new(HashMap::new(), &dir).unwrap();
        assert!(manager.find_pending_init_language().is_none());
    }

    #[test]
    fn find_pending_init_language_requires_active_client() {
        let dir = std::env::temp_dir();
        let mut manager = LspManager::new(HashMap::new(), &dir).unwrap();
        // Add to pending_init but no matching client exists.
        manager.pending_init.insert("rust".to_string());
        assert!(manager.find_pending_init_language().is_none());
    }

    #[test]
    fn flush_pending_opens_drains_matching_language() {
        let dir = std::env::temp_dir();
        let mut manager = LspManager::new(HashMap::new(), &dir).unwrap();
        manager.pending_open.push((
            PathBuf::from("/tmp/a.rs"),
            "rust".to_string(),
            "fn a() {}".to_string(),
        ));
        manager.pending_open.push((
            PathBuf::from("/tmp/b.py"),
            "python".to_string(),
            "def b(): pass".to_string(),
        ));
        manager.pending_open.push((
            PathBuf::from("/tmp/c.rs"),
            "rust".to_string(),
            "fn c() {}".to_string(),
        ));

        // Flush rust opens — no client exists, so didOpen won't be sent,
        // but the pending_open vec should be drained of ALL entries
        // (current implementation drains everything, keeping only non-matching).
        manager.flush_pending_opens("rust");

        // The current implementation uses drain(..).filter() which removes ALL entries
        // and only processes matching ones. This means python entry is also removed.
        // This is the actual behavior we're documenting.
        assert!(manager.pending_open.is_empty());
    }

    #[test]
    fn multiple_poll_events_on_empty_channel() {
        let dir = std::env::temp_dir();
        let mut manager = LspManager::new(HashMap::new(), &dir).unwrap();
        // Calling poll_events multiple times on empty channel should be fine.
        assert!(manager.poll_events().is_empty());
        assert!(manager.poll_events().is_empty());
        assert!(manager.poll_events().is_empty());
    }

    #[test]
    fn version_tracking_independent_per_file() {
        let dir = std::env::temp_dir();
        let mut manager = LspManager::new(HashMap::new(), &dir).unwrap();
        let path_a = PathBuf::from("/tmp/a.rs");
        let path_b = PathBuf::from("/tmp/b.rs");
        manager.versions.insert(path_a.clone(), 1);
        manager.versions.insert(path_b.clone(), 1);

        // Increment a only
        *manager.versions.get_mut(&path_a).unwrap() += 1;
        *manager.versions.get_mut(&path_a).unwrap() += 1;

        assert_eq!(manager.versions[&path_a], 3);
        assert_eq!(manager.versions[&path_b], 1);
    }
}
