use serde::{Deserialize, Serialize};

pub const SEMANTIC_PROJECTION_SCHEMA_ID: &str = "agent.semantic-protocols.semantic-projection";
pub const SEMANTIC_PROJECTION_SCHEMA_VERSION: &str = "1";

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SemanticProjection<Payload> {
    pub schema_id: String,
    pub schema_version: String,
    pub projection_kind: String,
    pub language_id: String,
    pub provider_id: String,
    pub root_selector: String,
    pub evidence_context_ref: String,
    pub payload_schema_id: String,
    pub payload_digest: String,
    pub payload: Payload,
}

impl<Payload> SemanticProjection<Payload>
where
    Payload: Serialize,
{
    pub fn new(
        projection_kind: impl Into<String>,
        language_id: impl Into<String>,
        provider_id: impl Into<String>,
        root_selector: impl Into<String>,
        evidence_context_ref: impl Into<String>,
        payload_schema_id: impl Into<String>,
        payload: Payload,
    ) -> Result<Self, String> {
        let encoded = serde_json::to_vec(&payload)
            .map_err(|error| format!("encode semantic projection payload: {error}"))?;
        let payload_digest = format!("blake3-256:{}", blake3::hash(&encoded).to_hex());
        let projection = Self {
            schema_id: SEMANTIC_PROJECTION_SCHEMA_ID.to_owned(),
            schema_version: SEMANTIC_PROJECTION_SCHEMA_VERSION.to_owned(),
            projection_kind: projection_kind.into(),
            language_id: language_id.into(),
            provider_id: provider_id.into(),
            root_selector: root_selector.into(),
            evidence_context_ref: evidence_context_ref.into(),
            payload_schema_id: payload_schema_id.into(),
            payload_digest,
            payload,
        };
        projection.validate()?;
        Ok(projection)
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.schema_id != SEMANTIC_PROJECTION_SCHEMA_ID
            || self.schema_version != SEMANTIC_PROJECTION_SCHEMA_VERSION
        {
            return Err("semantic projection schema is unsupported".to_owned());
        }
        for (field, value) in [
            ("projectionKind", self.projection_kind.as_str()),
            ("languageId", self.language_id.as_str()),
            ("providerId", self.provider_id.as_str()),
            ("rootSelector", self.root_selector.as_str()),
            ("evidenceContextRef", self.evidence_context_ref.as_str()),
            ("payloadSchemaId", self.payload_schema_id.as_str()),
        ] {
            if value.trim().is_empty() {
                return Err(format!("semantic projection {field} is required"));
            }
        }
        if !self.provider_id.starts_with("asp-") {
            return Err("semantic projection providerId must use asp-<language>".to_owned());
        }
        validate_digest("evidenceContextRef", &self.evidence_context_ref)?;
        validate_digest("payloadDigest", &self.payload_digest)?;
        let encoded = serde_json::to_vec(&self.payload)
            .map_err(|error| format!("encode semantic projection payload: {error}"))?;
        let expected = format!("blake3-256:{}", blake3::hash(&encoded).to_hex());
        if self.payload_digest != expected {
            return Err(format!(
                "semantic projection payload digest mismatch: expected={expected} actual={}",
                self.payload_digest
            ));
        }
        Ok(())
    }
}

fn validate_digest(field: &str, value: &str) -> Result<(), String> {
    let Some(hex) = value.strip_prefix("blake3-256:") else {
        return Err(format!("semantic projection {field} must use blake3-256"));
    };
    if hex.len() != 64 || !hex.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(format!("semantic projection {field} digest is invalid"));
    }
    Ok(())
}
