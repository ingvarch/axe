use serde_json::Value;

/// A single parameter within a signature.
///
/// The label is either a plain string (fallback) or a byte offset pair into
/// the signature label. The renderer uses [`Signature::parameter_range`] to
/// resolve the active parameter to a display range.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParameterInfo {
    /// Literal label text (used when `offsets` is `None`).
    pub label: String,
    /// `(start, end)` byte offsets into the signature label, if provided.
    pub offsets: Option<(usize, usize)>,
    /// Optional documentation text, flattened to a single string.
    pub documentation: Option<String>,
}

/// A single callable signature returned by the server.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Signature {
    /// Full signature label (e.g. `fn f(a: i32, b: &str) -> bool`).
    pub label: String,
    /// Optional documentation text, flattened to a single string.
    pub documentation: Option<String>,
    /// All parameters belonging to this signature.
    pub parameters: Vec<ParameterInfo>,
    /// Active parameter index override from the server, if present.
    pub active_parameter: Option<usize>,
}

impl Signature {
    /// Resolves the active parameter index to a character range within the
    /// signature label, preferring byte offsets and falling back to a literal
    /// substring search.
    pub fn parameter_range(&self, index: usize) -> Option<(usize, usize)> {
        let param = self.parameters.get(index)?;
        if let Some((start, end)) = param.offsets {
            let max = self.label.chars().count();
            if start <= max && end <= max && end >= start {
                return Some((start, end));
            }
        }
        let needle = param.label.trim();
        if needle.is_empty() {
            return None;
        }
        let start = self.label.find(needle)?;
        let char_start = self.label[..start].chars().count();
        let char_end = char_start + needle.chars().count();
        Some((char_start, char_end))
    }
}

/// Full signature help state kept on [`AppState`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignatureHelpState {
    pub signatures: Vec<Signature>,
    /// Index into `signatures` of the currently displayed overload.
    pub active_signature: usize,
    /// Index into the active signature's parameter list.
    pub active_parameter: usize,
    /// Row the popup was anchored at when the request was issued.
    pub anchor_row: usize,
    /// Column the popup was anchored at when the request was issued.
    pub anchor_col: usize,
}

impl SignatureHelpState {
    /// Returns the currently active signature, if any.
    pub fn active(&self) -> Option<&Signature> {
        self.signatures.get(self.active_signature)
    }

    /// Returns `(start, end)` character offsets of the active parameter
    /// within the active signature label, if resolvable.
    pub fn active_parameter_range(&self) -> Option<(usize, usize)> {
        self.active()?.parameter_range(self.active_parameter)
    }
}

/// Parses a `textDocument/signatureHelp` response into a
/// [`SignatureHelpState`], attaching the provided anchor coordinates.
///
/// Returns `None` for `null`, an empty `signatures` array, or malformed
/// responses — callers then leave the popup closed.
pub fn parse_signature_help_response(
    value: &Value,
    anchor_row: usize,
    anchor_col: usize,
) -> Option<SignatureHelpState> {
    if value.is_null() {
        return None;
    }

    let signatures_value = value.get("signatures")?.as_array()?;
    if signatures_value.is_empty() {
        return None;
    }

    let top_active_param = value
        .get("activeParameter")
        .and_then(Value::as_u64)
        .map(|n| n as usize);

    let active_signature = value
        .get("activeSignature")
        .and_then(Value::as_u64)
        .map(|n| n as usize)
        .unwrap_or(0)
        .min(signatures_value.len().saturating_sub(1));

    let mut signatures = Vec::with_capacity(signatures_value.len());
    for sig_value in signatures_value {
        let label = sig_value
            .get("label")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();

        let documentation = sig_value.get("documentation").and_then(flatten_markup);

        let parameters: Vec<ParameterInfo> = sig_value
            .get("parameters")
            .and_then(Value::as_array)
            .map(|arr| {
                arr.iter()
                    .filter_map(|p| parse_parameter(p, &label))
                    .collect()
            })
            .unwrap_or_default();

        let active_parameter = sig_value
            .get("activeParameter")
            .and_then(Value::as_u64)
            .map(|n| n as usize);

        signatures.push(Signature {
            label,
            documentation,
            parameters,
            active_parameter,
        });
    }

    let active_signature_obj = &signatures[active_signature];
    let active_parameter = active_signature_obj
        .active_parameter
        .or(top_active_param)
        .unwrap_or(0)
        .min(active_signature_obj.parameters.len().saturating_sub(1));

    Some(SignatureHelpState {
        signatures,
        active_signature,
        active_parameter,
        anchor_row,
        anchor_col,
    })
}

fn parse_parameter(value: &Value, signature_label: &str) -> Option<ParameterInfo> {
    let label_value = value.get("label")?;
    let (label, offsets) = match label_value {
        Value::String(s) => (s.clone(), None),
        Value::Array(arr) if arr.len() == 2 => {
            let start = arr[0].as_u64().map(|n| n as usize)?;
            let end = arr[1].as_u64().map(|n| n as usize)?;
            let max = signature_label.chars().count();
            let safe_start = start.min(max);
            let safe_end = end.min(max).max(safe_start);
            let substring: String = signature_label
                .chars()
                .skip(safe_start)
                .take(safe_end - safe_start)
                .collect();
            (substring, Some((safe_start, safe_end)))
        }
        _ => return None,
    };

    let documentation = value.get("documentation").and_then(flatten_markup);

    Some(ParameterInfo {
        label,
        offsets,
        documentation,
    })
}

/// Flattens a hover/signature `MarkupContent`-like field to a plain string.
fn flatten_markup(value: &Value) -> Option<String> {
    match value {
        Value::String(s) if !s.is_empty() => Some(s.clone()),
        Value::Object(map) => map
            .get("value")
            .and_then(Value::as_str)
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string()),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parse_null_returns_none() {
        assert!(parse_signature_help_response(&Value::Null, 0, 0).is_none());
    }

    #[test]
    fn parse_empty_signatures_returns_none() {
        let value = json!({ "signatures": [] });
        assert!(parse_signature_help_response(&value, 0, 0).is_none());
    }

    #[test]
    fn parse_single_signature_string_params() {
        let value = json!({
            "signatures": [{
                "label": "fn add(a: i32, b: i32) -> i32",
                "parameters": [
                    { "label": "a: i32" },
                    { "label": "b: i32" }
                ]
            }],
            "activeSignature": 0,
            "activeParameter": 1
        });
        let state = parse_signature_help_response(&value, 3, 5).unwrap();
        assert_eq!(state.signatures.len(), 1);
        assert_eq!(state.active_signature, 0);
        assert_eq!(state.active_parameter, 1);
        assert_eq!(state.anchor_row, 3);
        assert_eq!(state.anchor_col, 5);
        let sig = state.active().unwrap();
        assert_eq!(sig.parameters.len(), 2);
        assert_eq!(sig.parameters[0].label, "a: i32");
    }

    #[test]
    fn parse_offset_based_parameters() {
        // "fn f(a, b)" — param "a" spans chars 5..6, param "b" spans 8..9.
        let value = json!({
            "signatures": [{
                "label": "fn f(a, b)",
                "parameters": [
                    { "label": [5, 6] },
                    { "label": [8, 9] }
                ]
            }],
            "activeParameter": 0
        });
        let state = parse_signature_help_response(&value, 0, 0).unwrap();
        let sig = state.active().unwrap();
        assert_eq!(sig.parameters[0].offsets, Some((5, 6)));
        assert_eq!(sig.parameters[0].label, "a");
        assert_eq!(sig.parameters[1].offsets, Some((8, 9)));
        assert_eq!(sig.parameters[1].label, "b");
        assert_eq!(state.active_parameter_range(), Some((5, 6)));
    }

    #[test]
    fn parameter_range_prefers_offsets_over_substring_search() {
        let sig = Signature {
            label: "fn f(a, a)".to_string(),
            documentation: None,
            parameters: vec![
                ParameterInfo {
                    label: "a".to_string(),
                    offsets: Some((5, 6)),
                    documentation: None,
                },
                ParameterInfo {
                    label: "a".to_string(),
                    offsets: Some((8, 9)),
                    documentation: None,
                },
            ],
            active_parameter: None,
        };
        assert_eq!(sig.parameter_range(0), Some((5, 6)));
        assert_eq!(sig.parameter_range(1), Some((8, 9)));
    }

    #[test]
    fn parameter_range_falls_back_to_substring_when_no_offsets() {
        let sig = Signature {
            label: "fn f(name: &str)".to_string(),
            documentation: None,
            parameters: vec![ParameterInfo {
                label: "name: &str".to_string(),
                offsets: None,
                documentation: None,
            }],
            active_parameter: None,
        };
        assert_eq!(sig.parameter_range(0), Some((5, 15)));
    }

    #[test]
    fn active_parameter_defaults_to_zero() {
        let value = json!({
            "signatures": [{ "label": "f()", "parameters": [{ "label": "" }] }]
        });
        let state = parse_signature_help_response(&value, 0, 0).unwrap();
        assert_eq!(state.active_parameter, 0);
    }

    #[test]
    fn active_signature_clamps_out_of_range() {
        let value = json!({
            "signatures": [{ "label": "one" }, { "label": "two" }],
            "activeSignature": 99
        });
        let state = parse_signature_help_response(&value, 0, 0).unwrap();
        assert_eq!(state.active_signature, 1);
    }

    #[test]
    fn per_signature_active_parameter_overrides_top_level() {
        let value = json!({
            "signatures": [{
                "label": "fn f(a, b)",
                "parameters": [
                    { "label": "a" },
                    { "label": "b" }
                ],
                "activeParameter": 1
            }],
            "activeParameter": 0
        });
        let state = parse_signature_help_response(&value, 0, 0).unwrap();
        assert_eq!(state.active_parameter, 1);
    }

    #[test]
    fn documentation_as_markup_object_flattens() {
        let value = json!({
            "signatures": [{
                "label": "f()",
                "documentation": { "kind": "markdown", "value": "doc text" }
            }]
        });
        let state = parse_signature_help_response(&value, 0, 0).unwrap();
        assert_eq!(
            state.active().unwrap().documentation.as_deref(),
            Some("doc text")
        );
    }

    #[test]
    fn documentation_empty_string_becomes_none() {
        let value = json!({
            "signatures": [{ "label": "f()", "documentation": "" }]
        });
        let state = parse_signature_help_response(&value, 0, 0).unwrap();
        assert!(state.active().unwrap().documentation.is_none());
    }

    #[test]
    fn signatures_without_parameters_still_parse() {
        let value = json!({
            "signatures": [{ "label": "fn f()" }]
        });
        let state = parse_signature_help_response(&value, 0, 0).unwrap();
        assert!(state.active().unwrap().parameters.is_empty());
        assert_eq!(state.active_parameter, 0);
    }
}
