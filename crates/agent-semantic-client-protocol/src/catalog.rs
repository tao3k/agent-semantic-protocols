// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Declarative catalog of Runtime client transports, capabilities, and typed methods.

use serde::Deserialize;
use serde::Serialize;

use crate::ClientRouteId;
use crate::ClientSchemaId;

/// Complete client-facing protocol catalog for one workspace generation.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ClientProtocolCatalog {
    /// Catalog schema identity.
    pub schema_id: String,
    /// Catalog schema version.
    pub schema_version: String,
    /// Protocol identity implemented by the catalog.
    pub protocol_id: String,
    /// Protocol version implemented by the catalog.
    pub protocol_version: String,
    /// Immutable catalog content generation.
    pub catalog_generation: String,
    /// Workspace generation bound to the catalog.
    pub workspace_generation: String,
    /// Supported client transports.
    pub transports: Vec<ClientTransport>,
    /// Cross-method client capabilities.
    pub capabilities: ClientCapabilities,
    /// Typed methods admitted by this catalog.
    pub methods: Vec<ClientMethod>,
}

/// Transport by which a client exchanges frames with Runtime.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ClientTransport {
    /// Runtime-owned local inter-process transport.
    RuntimeIpc,
}

/// Features shared by every method in a client catalog.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ClientCapabilities {
    /// Whether in-flight requests may be cancelled.
    pub request_cancellation: bool,
    /// Whether Runtime may emit events.
    pub events: bool,
    /// Whether methods may return streamed frames.
    pub streaming: bool,
    /// Whether request frames carry trace context.
    pub trace_context: bool,
}

/// Schema-bound description of one callable Runtime method.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ClientMethod {
    /// Wire method name.
    pub method: String,
    /// Stable route identity.
    pub route_id: ClientRouteId,
    /// Request schema identity.
    pub request_schema_id: ClientSchemaId,
    /// Success response schema identity.
    pub response_schema_id: ClientSchemaId,
    /// Typed error schema identities.
    pub error_schema_ids: Vec<ClientSchemaId>,
    /// Ordered method parameters.
    pub parameters: Vec<ClientParameter>,
    /// Whether the method admits cancellation.
    pub cancellable: bool,
    /// Whether the method emits a stream.
    pub streaming: bool,
}

/// One named input consumed by a client method.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ClientParameter {
    /// Wire parameter name.
    pub name: String,
    /// Validated value representation.
    pub value_type: ClientParameterType,
    /// Required, optional, or repeated cardinality.
    pub cardinality: ClientParameterCardinality,
    /// Authority that supplies the parameter.
    pub source: ClientParameterSource,
}

/// Value representations accepted by catalog parameters.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ClientParameterType {
    /// UTF-8 string.
    String,
    /// Repeated UTF-8 strings.
    StringArray,
    /// Path resolved relative to the admitted workspace.
    WorkspaceRelativePath,
    /// Provider-owned structural selector.
    StructuralSelector,
    /// Presentation or projection mode.
    Presentation,
    /// Boolean flag.
    Boolean,
    /// Non-negative integer.
    UnsignedInteger,
    /// Schema-validated JSON value.
    Json,
}

/// Cardinality of a catalog parameter.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ClientParameterCardinality {
    /// Exactly one value is required.
    Required,
    /// Zero or one value is accepted.
    Optional,
    /// Zero or more values are accepted.
    Many,
}

/// Authority responsible for supplying a parameter value.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ClientParameterSource {
    /// The caller supplies the value in its request.
    Request,
    /// Runtime derives the value from admitted context.
    RuntimeContext,
}
