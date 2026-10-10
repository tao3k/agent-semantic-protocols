// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Versioned request, response, event, and lifecycle frames exchanged with Runtime.

use serde::Deserialize;
use serde::Serialize;
use serde::ser::SerializeMap;
use serde_json::Value;
use std::collections::BTreeMap;
use std::sync::Arc;

use crate::ClientProjectId;
use crate::ClientProtocolCatalog;
use crate::ClientRequestId;
use crate::ClientSessionId;
use crate::ClientWorkspaceIdentity;
use crate::RuntimeSearchClientTimingWitness;

/// A typed frame on the Runtime client protocol.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(
    tag = "kind",
    rename_all = "kebab-case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum ClientFrame {
    /// Establishes a session and receives the current method catalog.
    Initialize {
        #[serde(flatten)]
        base: ClientFrameBase,
        request_id: ClientRequestId,
        client_info: ClientInfo,
        capabilities: Value,
    },
    /// Invokes one catalog-admitted method.
    Request {
        #[serde(flatten)]
        base: ClientFrameBase,
        request_id: ClientRequestId,
        catalog_generation: String,
        workspace_generation: String,
        method: String,
        params: Value,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        client_timing_witness: Option<RuntimeSearchClientTimingWitness>,
    },
    /// Cancels an in-flight request.
    Cancel {
        #[serde(flatten)]
        base: ClientFrameBase,
        request_id: ClientRequestId,
    },
    /// Requests an orderly session shutdown.
    Shutdown {
        #[serde(flatten)]
        base: ClientFrameBase,
        request_id: ClientRequestId,
    },
    /// Confirms that the client has left the session.
    Exit {
        #[serde(flatten)]
        base: ClientFrameBase,
    },
    /// Returns a terminal outcome and optional payload for a request.
    Response {
        #[serde(flatten)]
        base: ClientFrameBase,
        request_id: ClientRequestId,
        outcome: ClientOutcome,
        #[serde(default)]
        result: Option<ClientResponsePayload>,
        #[serde(default)]
        error: Option<Value>,
        #[serde(default)]
        catalog: Option<ClientProtocolCatalog>,
    },
    /// Delivers an asynchronous Runtime event.
    Event {
        #[serde(flatten)]
        base: ClientFrameBase,
        event_id: String,
        event: String,
        payload: Value,
    },
}

/// Wire-transparent shared ownership for an immutable response JSON payload.
///
/// Runtime can retain a generation-owned materialization through gRPC
/// serialization without cloning its full JSON tree. Deserialization creates a
/// new shared payload from the received wire value.
#[derive(Clone, Debug)]
pub struct ClientResponsePayload(ClientResponsePayloadInner);

#[derive(Clone, Debug)]
enum ClientResponsePayloadInner {
    Shared {
        value: Arc<Value>,
        encoded_json: Arc<std::sync::OnceLock<Arc<[u8]>>>,
    },
    ObjectOverlay {
        template: Arc<Value>,
        fields: Arc<BTreeMap<String, Value>>,
    },
}

impl ClientResponsePayload {
    #[must_use]
    pub fn from_shared(value: Arc<Value>) -> Self {
        Self(ClientResponsePayloadInner::Shared {
            value,
            encoded_json: Arc::new(std::sync::OnceLock::new()),
        })
    }

    /// Binds request-local fields to a shared immutable object without cloning
    /// the generation-owned JSON tree. The overlay is applied while the frame
    /// is serialized and materialized only after a client receives the wire
    /// value.
    pub fn from_shared_object_overlay(
        template: Arc<Value>,
        fields: BTreeMap<String, Value>,
    ) -> Result<Self, &'static str> {
        if !template.is_object() {
            return Err("response overlay template is not an object");
        }
        Ok(Self(ClientResponsePayloadInner::ObjectOverlay {
            template,
            fields: Arc::new(fields),
        }))
    }

    #[must_use]
    pub fn as_value(&self) -> &Value {
        match &self.0 {
            ClientResponsePayloadInner::Shared { value, .. } => value.as_ref(),
            ClientResponsePayloadInner::ObjectOverlay { template, .. } => template.as_ref(),
        }
    }

    /// Return the canonical V1 JSON bytes, memoizing immutable shared payloads.
    pub fn encoded_json(&self) -> Result<Arc<[u8]>, serde_json::Error> {
        if let ClientResponsePayloadInner::Shared {
            value,
            encoded_json,
        } = &self.0
        {
            if let Some(encoded) = encoded_json.get() {
                return Ok(Arc::clone(encoded));
            }
            let encoded = Arc::<[u8]>::from(serde_json::to_vec(value.as_ref())?);
            let _ = encoded_json.set(Arc::clone(&encoded));
            return Ok(encoded_json.get().map_or(encoded, Arc::clone));
        }
        serde_json::to_vec(self).map(Arc::<[u8]>::from)
    }

    #[must_use]
    pub fn into_value(self) -> Value {
        match self.0 {
            ClientResponsePayloadInner::Shared { value, .. } => Arc::unwrap_or_clone(value),
            ClientResponsePayloadInner::ObjectOverlay { template, fields } => {
                let mut value = Arc::unwrap_or_clone(template);
                let object = value
                    .as_object_mut()
                    .expect("object response overlay validated at construction");
                for (key, value) in fields.iter() {
                    object.insert(key.clone(), value.clone());
                }
                value
            }
        }
    }
}

impl From<Value> for ClientResponsePayload {
    fn from(value: Value) -> Self {
        Self::from_shared(Arc::new(value))
    }
}

impl PartialEq for ClientResponsePayload {
    fn eq(&self, other: &Self) -> bool {
        serde_json::to_value(self).ok() == serde_json::to_value(other).ok()
    }
}

impl PartialEq<Value> for ClientResponsePayload {
    fn eq(&self, other: &Value) -> bool {
        self.as_value() == other
    }
}

impl std::ops::Deref for ClientResponsePayload {
    type Target = Value;

    fn deref(&self) -> &Self::Target {
        self.as_value()
    }
}

impl Serialize for ClientResponsePayload {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        match &self.0 {
            ClientResponsePayloadInner::Shared { value, .. } => value.serialize(serializer),
            ClientResponsePayloadInner::ObjectOverlay { template, fields } => {
                let object = template
                    .as_object()
                    .expect("object response overlay validated at construction");
                let appended = fields
                    .keys()
                    .filter(|key| !object.contains_key(key.as_str()))
                    .count();
                let mut map = serializer.serialize_map(Some(object.len() + appended))?;
                for (key, value) in object {
                    map.serialize_entry(key, fields.get(key).unwrap_or(value))?;
                }
                for (key, value) in fields.iter().filter(|(key, _)| !object.contains_key(*key)) {
                    map.serialize_entry(key, value)?;
                }
                map.end()
            }
        }
    }
}

impl<'de> Deserialize<'de> for ClientResponsePayload {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        Value::deserialize(deserializer).map(Self::from)
    }
}

#[cfg(test)]
#[path = "../tests/unit/frame.rs"]
mod tests;

/// Identity and tracing fields shared by every client frame.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ClientFrameBase {
    /// Frame schema identity.
    pub schema_id: String,
    /// Frame schema version.
    pub schema_version: String,
    /// Protocol identity.
    pub protocol_id: String,
    /// Protocol version.
    pub protocol_version: String,
    /// Runtime client session identity.
    pub session_id: ClientSessionId,
    /// Project identity admitted for the session.
    pub project_id: ClientProjectId,
    /// Workspace identity admitted for the session.
    pub workspace_id: ClientWorkspaceIdentity,
    /// Optional distributed trace context.
    #[serde(default)]
    pub trace_context: Option<TraceContext>,
}

/// Name and version of the connecting client implementation.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ClientInfo {
    /// Client implementation name.
    pub name: String,
    /// Client implementation version.
    pub version: String,
}

/// W3C-compatible trace propagation fields.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TraceContext {
    /// Required trace-parent value.
    pub traceparent: String,
    /// Optional vendor trace state.
    #[serde(default)]
    pub tracestate: Option<String>,
}

/// Terminal classification of a response frame.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ClientOutcome {
    /// The request completed successfully.
    Ready,
    /// The request failed with a typed error payload.
    Error,
    /// The request was cancelled.
    Cancelled,
    /// The request named a generation that is no longer current.
    StaleGeneration,
}
