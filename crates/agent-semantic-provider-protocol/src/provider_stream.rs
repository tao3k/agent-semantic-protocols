// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Validation for language-neutral Provider stream envelopes.

/// Stable schema identity for Provider stream envelopes.
pub const PROVIDER_STREAM_SCHEMA_ID: &str = "agent.semantic-protocols.provider-stream";
/// Stable schema version for Provider stream envelopes.
pub const PROVIDER_STREAM_SCHEMA_VERSION: &str = "1";

/// Borrowed wire identity fields required on every Provider stream envelope.
#[derive(Clone, Copy, Debug)]
pub struct ProviderStreamEnvelopeIdentity<'a> {
    pub schema_id: &'a str,
    pub schema_version: &'a str,
    pub session_id: &'a str,
    pub request_id: &'a str,
    pub workspace_identity: &'a str,
    pub generation_digest: &'a str,
    pub provider_id: &'a str,
    pub language_id: &'a str,
    pub kind: &'a str,
    pub payload_schema_id: &'a str,
}

/// Validate the identity fields before constructing typed resident state.
pub fn validate_provider_stream_envelope(
    identity: &ProviderStreamEnvelopeIdentity<'_>,
) -> Result<(), String> {
    if identity.schema_id != PROVIDER_STREAM_SCHEMA_ID
        || identity.schema_version != PROVIDER_STREAM_SCHEMA_VERSION
    {
        return Err("invalid provider stream schema identity".to_owned());
    }
    for (name, value) in [
        ("sessionId", identity.session_id),
        ("requestId", identity.request_id),
        ("workspaceIdentity", identity.workspace_identity),
        ("generationDigest", identity.generation_digest),
        ("providerId", identity.provider_id),
        ("languageId", identity.language_id),
        ("kind", identity.kind),
        ("payloadSchemaId", identity.payload_schema_id),
    ] {
        if value.is_empty() {
            return Err(format!("provider stream envelope {name} is empty"));
        }
    }
    Ok(())
}
