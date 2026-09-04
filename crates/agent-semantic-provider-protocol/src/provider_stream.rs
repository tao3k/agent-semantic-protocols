//! Validation for language-neutral Provider stream envelopes.

/// Stable schema identity for Provider stream envelopes.
pub const PROVIDER_STREAM_SCHEMA_ID: &str = "agent.semantic-protocols.provider-stream";
/// Stable schema version for Provider stream envelopes.
pub const PROVIDER_STREAM_SCHEMA_VERSION: &str = "1";

/// Validate the identity fields required on every Provider stream envelope.
///
/// This positional primitive boundary is the raw wire DTO adapter; callers
/// validate it before constructing typed resident state.
pub fn validate_provider_stream_envelope(
    schema_id: &str,
    schema_version: &str,
    session_id: &str,
    request_id: &str,
    workspace_identity: &str,
    generation_digest: &str,
    provider_id: &str,
    language_id: &str,
    kind: &str,
    payload_schema_id: &str,
) -> Result<(), String> {
    if schema_id != PROVIDER_STREAM_SCHEMA_ID || schema_version != PROVIDER_STREAM_SCHEMA_VERSION {
        return Err("invalid provider stream schema identity".to_owned());
    }
    for (name, value) in [
        ("sessionId", session_id),
        ("requestId", request_id),
        ("workspaceIdentity", workspace_identity),
        ("generationDigest", generation_digest),
        ("providerId", provider_id),
        ("languageId", language_id),
        ("kind", kind),
        ("payloadSchemaId", payload_schema_id),
    ] {
        if value.is_empty() {
            return Err(format!("provider stream envelope {name} is empty"));
        }
    }
    Ok(())
}
