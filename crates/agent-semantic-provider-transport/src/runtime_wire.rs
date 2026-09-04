use serde::Deserialize;
use serde::Serialize;

pub const PROVIDER_RUNTIME_REQUEST_FRAME_SCHEMA_ID: &str =
    "agent.semantic-protocols.provider-runtime-request-frame";
pub const PROVIDER_RUNTIME_RESPONSE_FRAME_SCHEMA_ID: &str =
    "agent.semantic-protocols.provider-runtime-response-frame";
pub const PROVIDER_RUNTIME_FRAME_SCHEMA_VERSION: &str = "1";

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProviderRuntimeRequestFrame {
    pub schema_id: String,
    pub schema_version: String,
    pub request_id: String,
    pub operation: String,
    pub payload: serde_json::Value,
}

impl ProviderRuntimeRequestFrame {
    pub fn new(
        request_id: impl Into<String>,
        operation: impl Into<String>,
        payload: impl AsRef<[u8]>,
    ) -> Result<Self, String> {
        let payload = serde_json::from_slice(payload.as_ref()).map_err(|error| {
            format!("provider runtime request payload must be structured JSON: {error}")
        })?;
        let frame = Self {
            schema_id: PROVIDER_RUNTIME_REQUEST_FRAME_SCHEMA_ID.to_owned(),
            schema_version: PROVIDER_RUNTIME_FRAME_SCHEMA_VERSION.to_owned(),
            request_id: request_id.into(),
            operation: operation.into(),
            payload,
        };
        frame.validate()?;
        Ok(frame)
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.schema_id != PROVIDER_RUNTIME_REQUEST_FRAME_SCHEMA_ID
            || self.schema_version != PROVIDER_RUNTIME_FRAME_SCHEMA_VERSION
        {
            return Err("provider runtime request frame schema identity drift".to_owned());
        }
        if self.request_id.trim().is_empty() || self.operation.trim().is_empty() {
            return Err(
                "provider runtime request frame requires requestId and operation".to_owned(),
            );
        }
        if !self.payload.is_object() {
            return Err("provider runtime request payload must be a JSON object".to_owned());
        }
        Ok(())
    }

    pub fn payload_bytes(&self) -> Result<Vec<u8>, String> {
        serde_json::to_vec(&self.payload)
            .map_err(|error| format!("encode provider runtime request payload: {error}"))
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ProviderRuntimeResponseOutcome {
    Ready,
    Error,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProviderRuntimeResponseFrame {
    pub schema_id: String,
    pub schema_version: String,
    pub request_id: String,
    pub outcome: ProviderRuntimeResponseOutcome,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub payload: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl ProviderRuntimeResponseFrame {
    pub fn ready(request_id: impl Into<String>, payload: serde_json::Value) -> Self {
        Self {
            schema_id: PROVIDER_RUNTIME_RESPONSE_FRAME_SCHEMA_ID.to_owned(),
            schema_version: PROVIDER_RUNTIME_FRAME_SCHEMA_VERSION.to_owned(),
            request_id: request_id.into(),
            outcome: ProviderRuntimeResponseOutcome::Ready,
            payload: Some(payload),
            error: None,
        }
    }

    pub fn error(request_id: impl Into<String>, error: impl Into<String>) -> Self {
        Self {
            schema_id: PROVIDER_RUNTIME_RESPONSE_FRAME_SCHEMA_ID.to_owned(),
            schema_version: PROVIDER_RUNTIME_FRAME_SCHEMA_VERSION.to_owned(),
            request_id: request_id.into(),
            outcome: ProviderRuntimeResponseOutcome::Error,
            payload: None,
            error: Some(error.into()),
        }
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.schema_id != PROVIDER_RUNTIME_RESPONSE_FRAME_SCHEMA_ID
            || self.schema_version != PROVIDER_RUNTIME_FRAME_SCHEMA_VERSION
        {
            return Err("provider runtime response frame schema identity drift".to_owned());
        }
        if self.request_id.trim().is_empty() {
            return Err("provider runtime response frame requires requestId".to_owned());
        }
        match (&self.outcome, &self.payload, &self.error) {
            (ProviderRuntimeResponseOutcome::Ready, Some(payload), None) if payload.is_object() => {
                Ok(())
            }
            (ProviderRuntimeResponseOutcome::Error, None, Some(error))
                if !error.trim().is_empty() =>
            {
                Ok(())
            }
            _ => Err("provider runtime response frame outcome/payload mismatch".to_owned()),
        }
    }

    pub fn payload_bytes(&self) -> Result<Option<Vec<u8>>, String> {
        self.payload
            .as_ref()
            .map(|payload| {
                serde_json::to_vec(payload)
                    .map_err(|error| format!("encode provider runtime response payload: {error}"))
            })
            .transpose()
    }
}

#[cfg(test)]
#[path = "../tests/unit/runtime_wire.rs"]
mod tests;
