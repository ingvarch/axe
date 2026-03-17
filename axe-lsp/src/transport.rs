use std::io::BufRead;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

/// JSON-RPC 2.0 message used for LSP communication.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcMessage {
    pub jsonrpc: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<RequestId>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub method: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<JsonRpcError>,
}

/// Request identifier — either a number or a string.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(untagged)]
pub enum RequestId {
    Number(i64),
    String(String),
}

/// JSON-RPC error object returned by a server.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcError {
    pub code: i64,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
}

/// Encodes a JSON-RPC message with Content-Length header framing.
///
/// Returns bytes in the format: `Content-Length: N\r\n\r\n{json}`
pub fn encode_message(msg: &JsonRpcMessage) -> Result<Vec<u8>> {
    let body = serde_json::to_string(msg).context("Failed to serialize JSON-RPC message")?;
    let header = format!("Content-Length: {}\r\n\r\n", body.len());
    let mut result = Vec::with_capacity(header.len() + body.len());
    result.extend_from_slice(header.as_bytes());
    result.extend_from_slice(body.as_bytes());
    Ok(result)
}

/// Reads a single JSON-RPC message from a buffered reader.
///
/// Parses the Content-Length header, reads exactly that many bytes, and
/// deserializes the JSON body. Returns `Ok(None)` on EOF (empty first line).
pub fn read_message(reader: &mut impl BufRead) -> Result<Option<JsonRpcMessage>> {
    // Read headers until empty line.
    let mut content_length: Option<usize> = None;
    loop {
        let mut line = String::new();
        let bytes_read = reader
            .read_line(&mut line)
            .context("Failed to read header line")?;
        if bytes_read == 0 {
            // EOF before any header — no more messages.
            return Ok(None);
        }

        let trimmed = line.trim();
        if trimmed.is_empty() {
            // End of headers.
            break;
        }

        if let Some(value) = trimmed.strip_prefix("Content-Length:") {
            content_length = Some(
                value
                    .trim()
                    .parse::<usize>()
                    .context("Invalid Content-Length value")?,
            );
        }
        // Ignore other headers (e.g., Content-Type).
    }

    let length = content_length.context("Missing Content-Length header")?;

    let mut body = vec![0u8; length];
    reader
        .read_exact(&mut body)
        .context("Truncated message body")?;

    let msg: JsonRpcMessage =
        serde_json::from_slice(&body).context("Failed to parse JSON-RPC message body")?;

    Ok(Some(msg))
}

/// Creates a JSON-RPC request message.
pub fn make_request(id: i64, method: &str, params: serde_json::Value) -> JsonRpcMessage {
    JsonRpcMessage {
        jsonrpc: "2.0".to_string(),
        id: Some(RequestId::Number(id)),
        method: Some(method.to_string()),
        params: Some(params),
        result: None,
        error: None,
    }
}

/// Creates a JSON-RPC notification message (no id).
pub fn make_notification(method: &str, params: serde_json::Value) -> JsonRpcMessage {
    JsonRpcMessage {
        jsonrpc: "2.0".to_string(),
        id: None,
        method: Some(method.to_string()),
        params: Some(params),
        result: None,
        error: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn encode_produces_content_length_header() {
        let msg = make_notification("test/method", serde_json::json!({}));
        let encoded = encode_message(&msg).expect("encode should succeed");
        let text = String::from_utf8(encoded).expect("should be valid UTF-8");
        assert!(text.starts_with("Content-Length: "));
        assert!(text.contains("\r\n\r\n"));

        // Verify Content-Length value matches body length.
        let parts: Vec<&str> = text.splitn(2, "\r\n\r\n").collect();
        assert_eq!(parts.len(), 2);
        let header = parts[0];
        let body = parts[1];
        let claimed_len: usize = header
            .strip_prefix("Content-Length: ")
            .expect("should have prefix")
            .parse()
            .expect("should be number");
        assert_eq!(claimed_len, body.len());
    }

    #[test]
    fn read_parses_well_formed_message() {
        let msg = make_request(1, "initialize", serde_json::json!({"rootUri": null}));
        let encoded = encode_message(&msg).expect("encode should succeed");
        let mut reader = Cursor::new(encoded);
        let parsed = read_message(&mut reader)
            .expect("read should succeed")
            .expect("should not be None");
        assert_eq!(parsed.jsonrpc, "2.0");
        assert_eq!(parsed.id, Some(RequestId::Number(1)));
        assert_eq!(parsed.method.as_deref(), Some("initialize"));
    }

    #[test]
    fn read_returns_none_on_eof() {
        let mut reader = Cursor::new(Vec::<u8>::new());
        let result = read_message(&mut reader).expect("read should succeed");
        assert!(result.is_none());
    }

    #[test]
    fn read_errors_on_missing_content_length() {
        let data = b"\r\n{\"jsonrpc\":\"2.0\"}";
        let mut reader = Cursor::new(data.to_vec());
        let result = read_message(&mut reader);
        assert!(result.is_err());
        let err_msg = format!("{}", result.unwrap_err());
        assert!(
            err_msg.contains("Missing Content-Length"),
            "unexpected error: {err_msg}"
        );
    }

    #[test]
    fn read_errors_on_truncated_body() {
        // Header claims 1000 bytes but body is only 5.
        let data = b"Content-Length: 1000\r\n\r\nhello";
        let mut reader = Cursor::new(data.to_vec());
        let result = read_message(&mut reader);
        assert!(result.is_err());
    }

    #[test]
    fn encode_then_read_roundtrip() {
        let original = make_request(
            42,
            "textDocument/completion",
            serde_json::json!({"line": 10}),
        );
        let encoded = encode_message(&original).expect("encode should succeed");
        let mut reader = Cursor::new(encoded);
        let decoded = read_message(&mut reader)
            .expect("read should succeed")
            .expect("should not be None");
        assert_eq!(decoded.id, Some(RequestId::Number(42)));
        assert_eq!(decoded.method.as_deref(), Some("textDocument/completion"));
        assert_eq!(decoded.params, Some(serde_json::json!({"line": 10})));
    }

    #[test]
    fn read_two_sequential_messages() {
        let msg1 = make_request(1, "first", serde_json::json!(null));
        let msg2 = make_notification("second", serde_json::json!({"key": "value"}));
        let mut data = encode_message(&msg1).expect("encode msg1");
        data.extend(encode_message(&msg2).expect("encode msg2"));

        let mut reader = Cursor::new(data);
        let parsed1 = read_message(&mut reader)
            .expect("read1 should succeed")
            .expect("msg1 should not be None");
        let parsed2 = read_message(&mut reader)
            .expect("read2 should succeed")
            .expect("msg2 should not be None");

        assert_eq!(parsed1.id, Some(RequestId::Number(1)));
        assert_eq!(parsed1.method.as_deref(), Some("first"));
        assert!(parsed2.id.is_none());
        assert_eq!(parsed2.method.as_deref(), Some("second"));
    }

    #[test]
    fn make_notification_has_no_id() {
        let msg = make_notification("initialized", serde_json::json!({}));
        assert!(msg.id.is_none());
        assert_eq!(msg.method.as_deref(), Some("initialized"));
    }

    #[test]
    fn request_id_string_variant() {
        let id = RequestId::String("abc-123".to_string());
        let json = serde_json::to_value(&id).expect("serialize");
        assert_eq!(json, serde_json::json!("abc-123"));
    }
}
