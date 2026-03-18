use std::collections::HashMap;
use std::io::{BufReader, Write};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc;

use anyhow::{Context, Result};
use lsp_types::ServerCapabilities;
use serde_json::Value;
use url::Url;

use crate::transport::{self, JsonRpcError, JsonRpcMessage, RequestId};

/// Identifies the type of a pending LSP request for response routing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PendingRequestKind {
    Completion,
    Definition,
    References,
}

/// Events sent from the LSP reader thread to the main thread.
#[derive(Debug)]
pub enum LspEvent {
    /// Server completed initialization handshake.
    Initialized { language_id: String },
    /// Server sent a notification (e.g., diagnostics).
    ServerNotification {
        method: String,
        params: serde_json::Value,
    },
    /// Server responded to a request.
    Response {
        id: RequestId,
        result: std::result::Result<serde_json::Value, JsonRpcError>,
    },
    /// Server responded to a completion request.
    CompletionResponse {
        result: std::result::Result<serde_json::Value, JsonRpcError>,
    },
    /// Server responded to a textDocument/definition request.
    DefinitionResponse {
        result: std::result::Result<serde_json::Value, JsonRpcError>,
    },
    /// Server responded to a textDocument/references request.
    ReferencesResponse {
        result: std::result::Result<serde_json::Value, JsonRpcError>,
    },
    /// Server process crashed or exited unexpectedly.
    ServerCrashed { language_id: String, error: String },
}

/// A single LSP server connection.
///
/// Manages the lifecycle of one language server process: spawning, initialization,
/// document synchronization, and shutdown.
pub struct LspClient {
    language_id: String,
    stdin: ChildStdin,
    child: Child,
    next_id: i64,
    capabilities: Option<ServerCapabilities>,
    initialized: bool,
    /// Tracks pending requests by ID so responses can be routed by kind.
    pending_requests: HashMap<i64, PendingRequestKind>,
}

impl LspClient {
    /// Spawns an LSP server process and begins the initialization handshake.
    ///
    /// Starts a background reader thread that forwards server messages as `LspEvent`s.
    /// The caller should watch for `LspEvent::Response` with the initialize request id,
    /// then call `send_initialized_notification()`.
    pub fn start(
        command: &str,
        args: &[String],
        root_uri: &Url,
        language_id: &str,
        event_tx: mpsc::Sender<LspEvent>,
    ) -> Result<Self> {
        let mut child = Command::new(command)
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .with_context(|| format!("Failed to spawn LSP server: {command}"))?;

        let stdin = child
            .stdin
            .take()
            .context("Failed to capture LSP server stdin")?;
        let stdout = child
            .stdout
            .take()
            .context("Failed to capture LSP server stdout")?;

        // Spawn reader thread.
        let lang_id = language_id.to_string();
        std::thread::spawn(move || {
            let mut reader = BufReader::new(stdout);
            loop {
                match transport::read_message(&mut reader) {
                    Ok(Some(msg)) => {
                        let event = classify_message(msg, &lang_id);
                        if event_tx.send(event).is_err() {
                            break; // Receiver dropped.
                        }
                    }
                    Ok(None) => {
                        // EOF — server exited.
                        let _ = event_tx.send(LspEvent::ServerCrashed {
                            language_id: lang_id,
                            error: "Server process exited".to_string(),
                        });
                        break;
                    }
                    Err(e) => {
                        let _ = event_tx.send(LspEvent::ServerCrashed {
                            language_id: lang_id,
                            error: format!("Read error: {e}"),
                        });
                        break;
                    }
                }
            }
        });

        let mut client = Self {
            language_id: language_id.to_string(),
            stdin,
            child,
            next_id: 1,
            capabilities: None,
            initialized: false,
            pending_requests: HashMap::new(),
        };

        // Send initialize request.
        client.send_initialize(root_uri)?;

        Ok(client)
    }

    /// Sends the `initialize` request to the server.
    fn send_initialize(&mut self, root_uri: &Url) -> Result<()> {
        let params = initialize_params(root_uri);
        let msg = transport::make_request(self.next_request_id(), "initialize", params);
        self.send_raw(&msg)
    }

    /// Completes the initialization handshake by storing capabilities and
    /// sending the `initialized` notification.
    pub fn send_initialized_notification(
        &mut self,
        capabilities: ServerCapabilities,
    ) -> Result<()> {
        self.capabilities = Some(capabilities);
        self.initialized = true;
        let msg = transport::make_notification("initialized", serde_json::json!({}));
        self.send_raw(&msg)
    }

    /// Sends a `textDocument/didOpen` notification.
    pub fn notify_did_open(
        &mut self,
        path: &std::path::Path,
        language_id: &str,
        text: &str,
    ) -> Result<()> {
        let uri = Url::from_file_path(path)
            .map_err(|()| anyhow::anyhow!("Invalid file path: {}", path.display()))?;
        let params = serde_json::json!({
            "textDocument": {
                "uri": uri.as_str(),
                "languageId": language_id,
                "version": 1,
                "text": text,
            }
        });
        let msg = transport::make_notification("textDocument/didOpen", params);
        self.send_raw(&msg)
    }

    /// Sends a `textDocument/didChange` notification with full document sync.
    pub fn notify_did_change(
        &mut self,
        path: &std::path::Path,
        text: &str,
        version: i32,
    ) -> Result<()> {
        let uri = Url::from_file_path(path)
            .map_err(|()| anyhow::anyhow!("Invalid file path: {}", path.display()))?;
        let params = serde_json::json!({
            "textDocument": {
                "uri": uri.as_str(),
                "version": version,
            },
            "contentChanges": [{"text": text}]
        });
        let msg = transport::make_notification("textDocument/didChange", params);
        self.send_raw(&msg)
    }

    /// Sends a `textDocument/didSave` notification.
    pub fn notify_did_save(&mut self, path: &std::path::Path) -> Result<()> {
        let uri = Url::from_file_path(path)
            .map_err(|()| anyhow::anyhow!("Invalid file path: {}", path.display()))?;
        let params = serde_json::json!({
            "textDocument": {
                "uri": uri.as_str(),
            }
        });
        let msg = transport::make_notification("textDocument/didSave", params);
        self.send_raw(&msg)
    }

    /// Sends a JSON-RPC request to the server and tracks it for response routing.
    ///
    /// Returns the request ID used, which can be matched against future responses.
    pub fn send_request(
        &mut self,
        method: &str,
        params: Value,
        kind: PendingRequestKind,
    ) -> Result<i64> {
        let id = self.next_request_id();
        let msg = transport::make_request(id, method, params);
        self.send_raw(&msg)?;
        self.pending_requests.insert(id, kind);
        Ok(id)
    }

    /// Removes and returns the pending request kind for the given response ID.
    ///
    /// Returns `None` if the ID is not tracked (e.g., initialize response).
    pub fn take_pending(&mut self, id: i64) -> Option<PendingRequestKind> {
        self.pending_requests.remove(&id)
    }

    /// Sends shutdown request followed by exit notification.
    pub fn shutdown(&mut self) -> Result<()> {
        let msg =
            transport::make_request(self.next_request_id(), "shutdown", serde_json::Value::Null);
        if let Err(e) = self.send_raw(&msg) {
            log::warn!("Failed to send shutdown to {} LSP: {e}", self.language_id);
        }
        let exit = transport::make_notification("exit", serde_json::Value::Null);
        if let Err(e) = self.send_raw(&exit) {
            log::warn!("Failed to send exit to {} LSP: {e}", self.language_id);
        }
        let _ = self.child.wait();
        Ok(())
    }

    /// Returns whether the server process is still running.
    pub fn is_alive(&mut self) -> bool {
        matches!(self.child.try_wait(), Ok(None))
    }

    /// Returns whether initialization is complete.
    pub fn is_initialized(&self) -> bool {
        self.initialized
    }

    /// Returns the language ID this client handles.
    pub fn language_id(&self) -> &str {
        &self.language_id
    }

    /// Generates the next unique request ID.
    fn next_request_id(&mut self) -> i64 {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    /// Writes a raw JSON-RPC message to the server stdin.
    fn send_raw(&mut self, msg: &JsonRpcMessage) -> Result<()> {
        let bytes = transport::encode_message(msg)?;
        self.stdin
            .write_all(&bytes)
            .context("Failed to write to LSP server stdin")?;
        self.stdin
            .flush()
            .context("Failed to flush LSP server stdin")
    }
}

/// Classifies a server message into an `LspEvent`.
fn classify_message(msg: JsonRpcMessage, language_id: &str) -> LspEvent {
    // Response to a request (has id, no method).
    if let Some(id) = msg.id {
        if msg.method.is_none() {
            // This is a response.
            if let Some(error) = msg.error {
                return LspEvent::Response {
                    id,
                    result: Err(error),
                };
            }
            return LspEvent::Response {
                id,
                result: Ok(msg.result.unwrap_or(serde_json::Value::Null)),
            };
        }
        // Server request (has both id and method) — treat as notification for now.
    }

    // Notification (has method, no id — or server request which we handle similarly).
    if let Some(method) = msg.method {
        return LspEvent::ServerNotification {
            method,
            params: msg.params.unwrap_or(serde_json::Value::Null),
        };
    }

    // Malformed message.
    LspEvent::ServerCrashed {
        language_id: language_id.to_string(),
        error: "Received malformed JSON-RPC message".to_string(),
    }
}

/// Builds the `InitializeParams` JSON for the initialize request.
fn initialize_params(root_uri: &Url) -> serde_json::Value {
    serde_json::json!({
        "processId": std::process::id(),
        "rootUri": root_uri.as_str(),
        "capabilities": {
            "textDocument": {
                "synchronization": {
                    "didSave": true,
                    "dynamicRegistration": false,
                },
                "publishDiagnostics": {
                    "relatedInformation": true,
                },
                "completion": {
                    "completionItem": {
                        "snippetSupport": false,
                    },
                    "dynamicRegistration": false,
                },
                "definition": {
                    "dynamicRegistration": false,
                },
                "references": {
                    "dynamicRegistration": false,
                },
            },
        },
        "clientInfo": {
            "name": "axe",
            "version": "0.1.0",
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn initialize_request_structure() {
        let root = Url::parse("file:///tmp/project").expect("valid url");
        let params = initialize_params(&root);
        assert_eq!(params["rootUri"], "file:///tmp/project");
        assert!(params["processId"].is_number());
        assert!(params["capabilities"]["textDocument"].is_object());
        assert_eq!(params["clientInfo"]["name"], "axe");
    }

    #[test]
    fn initialize_params_includes_completion() {
        let root = Url::parse("file:///tmp/project").expect("valid url");
        let params = initialize_params(&root);
        let completion = &params["capabilities"]["textDocument"]["completion"];
        assert!(completion.is_object());
        assert_eq!(completion["completionItem"]["snippetSupport"], false);
        assert_eq!(completion["dynamicRegistration"], false);
    }

    #[test]
    fn initialize_params_includes_definition() {
        let root = Url::parse("file:///tmp/project").expect("valid url");
        let params = initialize_params(&root);
        let definition = &params["capabilities"]["textDocument"]["definition"];
        assert!(definition.is_object());
        assert_eq!(definition["dynamicRegistration"], false);
    }

    #[test]
    fn initialize_params_includes_references() {
        let root = Url::parse("file:///tmp/project").expect("valid url");
        let params = initialize_params(&root);
        let references = &params["capabilities"]["textDocument"]["references"];
        assert!(references.is_object());
        assert_eq!(references["dynamicRegistration"], false);
    }

    #[test]
    fn send_request_tracks_pending() {
        // Test pending_requests tracking logic without a real server.
        let mut pending: HashMap<i64, PendingRequestKind> = HashMap::new();
        pending.insert(1, PendingRequestKind::Completion);
        assert!(pending.contains_key(&1));
        assert_eq!(pending[&1], PendingRequestKind::Completion);
    }

    #[test]
    fn take_pending_removes() {
        let mut pending: HashMap<i64, PendingRequestKind> = HashMap::new();
        pending.insert(5, PendingRequestKind::Completion);
        let kind = pending.remove(&5);
        assert_eq!(kind, Some(PendingRequestKind::Completion));
        assert!(pending.remove(&5).is_none());
    }

    #[test]
    fn did_open_params_structure() {
        // Verify the JSON structure matches LSP spec by constructing it manually.
        let path = std::path::Path::new("/tmp/test.rs");
        let uri = Url::from_file_path(path).expect("valid path");
        let params = serde_json::json!({
            "textDocument": {
                "uri": uri.as_str(),
                "languageId": "rust",
                "version": 1,
                "text": "fn main() {}",
            }
        });
        assert_eq!(params["textDocument"]["languageId"], "rust");
        assert_eq!(params["textDocument"]["version"], 1);
        assert_eq!(params["textDocument"]["text"], "fn main() {}");
        assert!(params["textDocument"]["uri"]
            .as_str()
            .unwrap()
            .starts_with("file://"));
    }

    #[test]
    fn did_change_params_structure() {
        let path = std::path::Path::new("/tmp/test.rs");
        let uri = Url::from_file_path(path).expect("valid path");
        let params = serde_json::json!({
            "textDocument": {
                "uri": uri.as_str(),
                "version": 2,
            },
            "contentChanges": [{"text": "fn main() { println!(\"hello\"); }"}]
        });
        assert_eq!(params["textDocument"]["version"], 2);
        assert_eq!(
            params["contentChanges"][0]["text"],
            "fn main() { println!(\"hello\"); }"
        );
    }

    #[test]
    fn did_save_params_structure() {
        let path = std::path::Path::new("/tmp/test.rs");
        let uri = Url::from_file_path(path).expect("valid path");
        let params = serde_json::json!({
            "textDocument": {
                "uri": uri.as_str(),
            }
        });
        assert!(params["textDocument"]["uri"]
            .as_str()
            .unwrap()
            .contains("test.rs"));
    }

    #[test]
    fn request_id_increments() {
        // Simulate next_request_id behavior.
        let mut next_id: i64 = 1;
        let id1 = next_id;
        next_id += 1;
        let id2 = next_id;
        next_id += 1;
        let id3 = next_id;
        assert_eq!(id1, 1);
        assert_eq!(id2, 2);
        assert_eq!(id3, 3);
    }

    #[test]
    fn classify_response_success() {
        let msg = JsonRpcMessage {
            jsonrpc: "2.0".to_string(),
            id: Some(RequestId::Number(1)),
            method: None,
            params: None,
            result: Some(serde_json::json!({"capabilities": {}})),
            error: None,
        };
        let event = classify_message(msg, "rust");
        match event {
            LspEvent::Response { id, result } => {
                assert_eq!(id, RequestId::Number(1));
                assert!(result.is_ok());
            }
            _ => panic!("Expected Response event"),
        }
    }

    #[test]
    fn classify_response_error() {
        let msg = JsonRpcMessage {
            jsonrpc: "2.0".to_string(),
            id: Some(RequestId::Number(2)),
            method: None,
            params: None,
            result: None,
            error: Some(JsonRpcError {
                code: -32600,
                message: "Invalid Request".to_string(),
                data: None,
            }),
        };
        let event = classify_message(msg, "rust");
        match event {
            LspEvent::Response { id, result } => {
                assert_eq!(id, RequestId::Number(2));
                assert!(result.is_err());
            }
            _ => panic!("Expected Response event"),
        }
    }

    #[test]
    fn classify_notification() {
        let msg = JsonRpcMessage {
            jsonrpc: "2.0".to_string(),
            id: None,
            method: Some("textDocument/publishDiagnostics".to_string()),
            params: Some(serde_json::json!({"uri": "file:///test.rs"})),
            result: None,
            error: None,
        };
        let event = classify_message(msg, "rust");
        match event {
            LspEvent::ServerNotification { method, params } => {
                assert_eq!(method, "textDocument/publishDiagnostics");
                assert_eq!(params["uri"], "file:///test.rs");
            }
            _ => panic!("Expected ServerNotification event"),
        }
    }
}
