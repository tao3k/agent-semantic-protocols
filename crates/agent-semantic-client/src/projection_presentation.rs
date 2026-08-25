//! Presentation of typed exact-query responses owned by the ASP Client.

use agent_semantic_client_protocol::ClientFrame;
use serde_json::Value;

/// Output representation requested by an ASP Client command entry.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProjectionPresentation {
    /// Render the semantic projection as human-readable UTF-8 source text.
    Text,
    /// Preserve the complete typed wire response as JSON.
    MachineJson,
}

/// Render one exact-query response without provider-specific behavior.
pub fn render_exact_projection_response(
    frame: &ClientFrame,
    presentation: ProjectionPresentation,
) -> Result<String, String> {
    if presentation == ProjectionPresentation::MachineJson {
        return serde_json::to_string(frame)
            .map_err(|error| format!("encode exact projection response: {error}"));
    }
    let ClientFrame::Response {
        result: Some(result),
        error: None,
        ..
    } = frame
    else {
        return Err("exact projection did not return a successful typed response".to_owned());
    };
    render_projection_result(result)
}

fn render_projection_result(result: &Value) -> Result<String, String> {
    let payload = result.get("result").unwrap_or(result);
    if let Some(text) = payload.as_str() {
        return Ok(text.to_owned());
    }
    let bytes = payload
        .get("bytes")
        .and_then(Value::as_array)
        .ok_or_else(|| "exact projection response has no text or byte payload".to_owned())?;
    let bytes = bytes
        .iter()
        .map(|byte| {
            byte.as_u64()
                .filter(|byte| *byte <= u8::MAX as u64)
                .map(|byte| byte as u8)
                .ok_or_else(|| "exact projection response contains a non-byte value".to_owned())
        })
        .collect::<Result<Vec<_>, _>>()?;
    String::from_utf8(bytes)
        .map_err(|error| format!("exact projection response is not UTF-8 text: {error}"))
}
