// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Versioned request, response, event, and lifecycle frames exchanged with Runtime.

use serde::Deserialize;
use serde::Serialize;
use serde_json::Value;

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
        result: Option<Value>,
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
