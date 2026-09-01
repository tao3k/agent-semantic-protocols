use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};
use serde_json::Value;

mod agent_session;
mod routes;
mod schema_bundle;
mod server_method_catalog;
#[cfg(test)]
#[path = "../tests/unit/server_method_catalog.rs"]
mod server_method_catalog_tests;
pub mod workspace_source_mutation;
#[cfg(test)]
#[path = "../tests/unit/workspace_source_mutation.rs"]
mod workspace_source_mutation_tests;
pub use routes::{
    AspClientExactQueryFailure, AspClientExactQueryRequest, AspClientExactQueryResponse,
    AspClientGraphsTimelineRequest, AspClientOwnerSearchRequest, AspClientOwnerSearchResponse,
    AspClientOwnerSearchSeed, AspClientRuntimeWorkCounters, AspClientSearchRequest,
    AspClientSourceIndexLookupRequest, LIVE_CORPUS_CACHE_STATE_RECEIPT_SCHEMA_ID,
    LIVE_CORPUS_CACHE_STATE_REQUEST_SCHEMA_ID, LiveCorpusCacheStateReceipt,
    LiveCorpusCacheStateRequest, ProviderNativeExactProjection, ProviderNativeExactRequest,
    ProviderNativeOwnerSearchRequest, ProviderNativeOwnerSearchResponse,
    RuntimeProviderSearchRequest,
};
pub use schema_bundle::{
    SCHEMA_BUNDLE_METHOD, SCHEMA_BUNDLE_REQUEST_SCHEMA_ID, SCHEMA_BUNDLE_RESPONSE_SCHEMA_ID,
    SchemaBundleDocument, SchemaBundleEntry, SchemaBundleReceipt, SchemaBundleRequest,
    SchemaBundleResponse,
};
pub use server_method_catalog::{
    CANCELLATION_PROBE_METHOD, CANCELLATION_PROBE_REQUEST_SCHEMA_ID,
    CANCELLATION_PROBE_RESPONSE_SCHEMA_ID, ClientDispatchClass, GRAPH_EVALUATE_METHOD,
    GRAPH_EVALUATE_REQUEST_SCHEMA_ID, GRAPH_EVALUATE_RESPONSE_SCHEMA_ID, GRAPH_TIMELINE_METHOD,
    GRAPH_TIMELINE_REQUEST_SCHEMA_ID, GRAPH_TIMELINE_RESPONSE_SCHEMA_ID,
    LIVE_CORPUS_CACHE_STATE_METHOD, MULTI_AGENT_CHILDREN_METHOD, MULTI_AGENT_HOST_EVENT_METHOD,
    ResolvedServerClientMethod, ServerClientRoute, WORKSPACE_GENERATION_ENSURE_READY_METHOD,
    WORKSPACE_GENERATION_ENSURE_READY_REQUEST_SCHEMA_ID,
    WORKSPACE_GENERATION_ENSURE_READY_RESPONSE_SCHEMA_ID, classify_client_dispatch,
    resolve_server_client_method, resolve_server_client_method_owner, server_client_catalog,
    server_client_methods,
};

pub const CLIENT_PROTOCOL_ID: &str = "agent.semantic-protocols.client";
pub const CLIENT_PROTOCOL_VERSION: &str = "1";
pub const CLIENT_FRAME_SCHEMA_ID: &str = "agent.semantic-protocols.client.frame";
pub const CLIENT_CATALOG_SCHEMA_ID: &str = "agent.semantic-protocols.client.protocol-catalog";
pub const SCHEMA_VERSION: &str = "1";

macro_rules! client_identifier {
    ($name:ident) => {
        #[derive(Clone, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
        #[serde(transparent)]
        pub struct $name(String);

        impl $name {
            pub fn new(value: impl Into<String>) -> Result<Self, String> {
                let value = value.into();
                if value.trim().is_empty() {
                    return Err(concat!(stringify!($name), " must be non-empty").to_owned());
                }
                Ok(Self(value))
            }

            pub fn as_str(&self) -> &str {
                &self.0
            }

            pub fn into_inner(self) -> String {
                self.0
            }
        }

        impl TryFrom<String> for $name {
            type Error = String;
            fn try_from(value: String) -> Result<Self, Self::Error> {
                Self::new(value)
            }
        }
    };
}

client_identifier!(ClientWorkspaceIdentity);
client_identifier!(ClientSessionId);
client_identifier!(ClientRequestId);

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ClientProtocolCatalog {
    pub schema_id: String,
    pub schema_version: String,
    pub protocol_id: String,
    pub protocol_version: String,
    pub catalog_generation: String,
    pub workspace_generation: String,
    pub transports: Vec<ClientTransport>,
    pub capabilities: ClientCapabilities,
    pub methods: Vec<ClientMethod>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ClientTransport {
    RuntimeIpc,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ClientCapabilities {
    pub request_cancellation: bool,
    pub events: bool,
    pub streaming: bool,
    pub trace_context: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ClientMethod {
    pub method: String,
    pub route_id: String,
    pub request_schema_id: String,
    pub response_schema_id: String,
    pub error_schema_ids: Vec<String>,
    pub parameters: Vec<ClientParameter>,
    pub cancellable: bool,
    pub streaming: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ClientParameter {
    pub name: String,
    pub value_type: ClientParameterType,
    pub cardinality: ClientParameterCardinality,
    pub source: ClientParameterSource,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ClientParameterType {
    String,
    StringArray,
    WorkspaceRelativePath,
    StructuralSelector,
    Presentation,
    Boolean,
    UnsignedInteger,
    Json,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ClientParameterCardinality {
    Required,
    Optional,
    Many,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ClientParameterSource {
    Request,
    RuntimeContext,
}

pub mod runtime_generation;

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(
    tag = "kind",
    rename_all = "kebab-case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum ClientFrame {
    Initialize {
        #[serde(flatten)]
        base: ClientFrameBase,
        request_id: ClientRequestId,
        project_root: String,
        client_info: ClientInfo,
        capabilities: Value,
    },
    Dispatch {
        #[serde(flatten)]
        base: ClientFrameBase,
        request_id: ClientRequestId,
        project_root: String,
        client_info: ClientInfo,
        method: String,
        params: Value,
    },
    Request {
        #[serde(flatten)]
        base: ClientFrameBase,
        request_id: ClientRequestId,
        catalog_generation: String,
        workspace_generation: String,
        method: String,
        params: Value,
    },
    Cancel {
        #[serde(flatten)]
        base: ClientFrameBase,
        request_id: ClientRequestId,
    },
    Shutdown {
        #[serde(flatten)]
        base: ClientFrameBase,
        request_id: ClientRequestId,
    },
    Exit {
        #[serde(flatten)]
        base: ClientFrameBase,
    },
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
    Event {
        #[serde(flatten)]
        base: ClientFrameBase,
        event_id: String,
        event: String,
        payload: Value,
    },
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ClientFrameBase {
    pub schema_id: String,
    pub schema_version: String,
    pub protocol_id: String,
    pub protocol_version: String,
    pub session_id: ClientSessionId,
    pub workspace_identity: ClientWorkspaceIdentity,
    #[serde(default)]
    pub trace_context: Option<TraceContext>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ClientInfo {
    pub name: String,
    pub version: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TraceContext {
    pub traceparent: String,
    #[serde(default)]
    pub tracestate: Option<String>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ClientOutcome {
    Ready,
    Error,
    Cancelled,
    StaleGeneration,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ClientConformanceSuite {
    #[serde(rename = "$schema", default)]
    pub schema: Option<String>,
    pub schema_id: String,
    pub schema_version: String,
    pub protocol_id: String,
    pub protocol_version: String,
    pub cases: Vec<ClientConformanceCase>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ClientConformanceCase {
    pub case_id: String,
    pub request: ClientFrame,
    pub expected_outcome: ClientOutcome,
    #[serde(default)]
    pub expected_reason_kind: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClientConformanceReceipt {
    pub case_id: String,
    pub outcome: ClientOutcome,
    pub reason_kind: Option<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ClientSessionState {
    Created,
    Initialized,
    Shutdown,
    Exited,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClientSession {
    state: ClientSessionState,
    session_id: Option<ClientSessionId>,
    workspace_identity: Option<ClientWorkspaceIdentity>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClientAdmissionError {
    pub reason_kind: &'static str,
    pub message: String,
}

impl ClientProtocolCatalog {
    pub fn validate(&self) -> Result<(), ClientAdmissionError> {
        validate_protocol_identity(
            &self.schema_id,
            &self.schema_version,
            &self.protocol_id,
            &self.protocol_version,
            CLIENT_CATALOG_SCHEMA_ID,
        )?;
        validate_digest("catalogGeneration", &self.catalog_generation)?;
        validate_digest("workspaceGeneration", &self.workspace_generation)?;
        let mut transports = BTreeSet::new();
        for transport in &self.transports {
            if !transports.insert(*transport) {
                return Err(error(
                    "duplicate-client-transport",
                    "client transport is duplicated",
                ));
            }
        }
        if transports.is_empty() {
            return Err(error(
                "client-transport-required",
                "client transport is required",
            ));
        }
        let mut methods = BTreeSet::new();
        for method in &self.methods {
            validate_dotted_id("method", &method.method)?;
            validate_dotted_id("routeId", &method.route_id)?;
            for (field, value) in [
                ("requestSchemaId", &method.request_schema_id),
                ("responseSchemaId", &method.response_schema_id),
            ] {
                if value.is_empty() {
                    return Err(error(
                        "client-method-schema-required",
                        format!("{field} is required"),
                    ));
                }
            }
            if !methods.insert(method.method.as_str()) {
                return Err(error(
                    "duplicate-client-method",
                    format!("duplicate method: {}", method.method),
                ));
            }
            let mut parameters = BTreeSet::new();
            for parameter in &method.parameters {
                if !parameter
                    .name
                    .as_bytes()
                    .first()
                    .is_some_and(u8::is_ascii_lowercase)
                    || !parameter
                        .name
                        .bytes()
                        .all(|byte| byte.is_ascii_alphanumeric())
                {
                    return Err(error(
                        "client-parameter-name-invalid",
                        format!("invalid parameter name: {}", parameter.name),
                    ));
                }
                if !parameters.insert(parameter.name.as_str()) {
                    return Err(error(
                        "duplicate-client-parameter",
                        format!("duplicate parameter: {}", parameter.name),
                    ));
                }
            }
        }
        Ok(())
    }

    pub fn admits_method(&self, method: &str) -> bool {
        self.methods
            .iter()
            .any(|candidate| candidate.method == method)
    }
}

impl ClientFrame {
    pub fn base(&self) -> &ClientFrameBase {
        match self {
            Self::Initialize { base, .. }
            | Self::Dispatch { base, .. }
            | Self::Request { base, .. }
            | Self::Cancel { base, .. }
            | Self::Shutdown { base, .. }
            | Self::Exit { base }
            | Self::Response { base, .. }
            | Self::Event { base, .. } => base,
        }
    }

    pub fn validate_identity(&self) -> Result<(), ClientAdmissionError> {
        let base = self.base();
        validate_protocol_identity(
            &base.schema_id,
            &base.schema_version,
            &base.protocol_id,
            &base.protocol_version,
            CLIENT_FRAME_SCHEMA_ID,
        )?;
        if base.session_id.as_str().is_empty() {
            return Err(error(
                "client-session-required",
                "client sessionId is required",
            ));
        }
        if base.workspace_identity.as_str().is_empty() {
            return Err(error(
                "client-workspace-required",
                "client workspaceIdentity is required",
            ));
        }
        let correlated_request_id = match self {
            Self::Initialize { request_id, .. }
            | Self::Dispatch { request_id, .. }
            | Self::Request { request_id, .. }
            | Self::Cancel { request_id, .. }
            | Self::Shutdown { request_id, .. }
            | Self::Response { request_id, .. } => Some(request_id),
            Self::Exit { .. } | Self::Event { .. } => None,
        };
        if correlated_request_id.is_some_and(|request_id| request_id.as_str().is_empty()) {
            return Err(error(
                "client-request-id-required",
                "correlated client frame requires requestId",
            ));
        }
        Ok(())
    }
}

impl ClientSessionState {
    pub fn admit(
        self,
        frame: &ClientFrame,
        catalog: &ClientProtocolCatalog,
    ) -> Result<Self, ClientAdmissionError> {
        frame.validate_identity()?;
        match (self, frame) {
            (Self::Created, ClientFrame::Initialize { .. }) => Ok(Self::Initialized),
            (
                Self::Initialized,
                ClientFrame::Request {
                    catalog_generation,
                    workspace_generation,
                    method,
                    params,
                    ..
                },
            ) => {
                if catalog_generation != &catalog.catalog_generation
                    || workspace_generation != &catalog.workspace_generation
                {
                    return Err(error(
                        "client-catalog-generation-mismatch",
                        "client request generation is stale",
                    ));
                }
                if !catalog.admits_method(method) {
                    return Err(error(
                        "method-not-in-client-catalog",
                        format!("client method is not admitted: {method}"),
                    ));
                }
                let method = catalog
                    .methods
                    .iter()
                    .find(|candidate| candidate.method == *method)
                    .expect("admitted method must be present");
                validate_request_params(method, params)?;
                Ok(Self::Initialized)
            }
            (Self::Initialized, ClientFrame::Cancel { .. }) => Ok(Self::Initialized),
            (Self::Initialized, ClientFrame::Shutdown { .. }) => Ok(Self::Shutdown),
            (Self::Shutdown, ClientFrame::Exit { .. }) => Ok(Self::Exited),
            _ => Err(error(
                "client-lifecycle-transition-denied",
                "client frame is not admitted in the current lifecycle state",
            )),
        }
    }
}

impl Default for ClientSession {
    fn default() -> Self {
        Self {
            state: ClientSessionState::Created,
            session_id: None,
            workspace_identity: None,
        }
    }
}

impl ClientSession {
    #[must_use]
    pub fn state(&self) -> ClientSessionState {
        self.state
    }

    pub fn admit(
        &mut self,
        frame: &ClientFrame,
        catalog: &ClientProtocolCatalog,
    ) -> Result<ClientSessionState, ClientAdmissionError> {
        frame.validate_identity()?;
        let base = frame.base();
        if self.state == ClientSessionState::Created {
            if !matches!(frame, ClientFrame::Initialize { .. }) {
                return Err(error(
                    "client-lifecycle-transition-denied",
                    "client session must initialize before other frames",
                ));
            }
            self.session_id = Some(base.session_id.clone());
            self.workspace_identity = Some(base.workspace_identity.clone());
        } else if self.session_id.as_ref() != Some(&base.session_id)
            || self.workspace_identity.as_ref() != Some(&base.workspace_identity)
        {
            return Err(error(
                "client-session-isolation-mismatch",
                "client sessionId or workspaceIdentity crossed an isolation boundary",
            ));
        }
        let next = self.state.admit(frame, catalog)?;
        self.state = next;
        Ok(next)
    }
}

pub fn run_conformance_suite(
    suite: &ClientConformanceSuite,
    catalog: &ClientProtocolCatalog,
) -> Result<Vec<ClientConformanceReceipt>, ClientAdmissionError> {
    validate_protocol_identity(
        &suite.schema_id,
        &suite.schema_version,
        &suite.protocol_id,
        &suite.protocol_version,
        "agent.semantic-protocols.client.conformance",
    )?;
    catalog.validate()?;
    let mut session = ClientSession::default();
    let mut receipts = Vec::with_capacity(suite.cases.len());
    for case in &suite.cases {
        let (outcome, reason_kind) = match session.admit(&case.request, catalog) {
            Ok(_) => {
                let outcome = if matches!(case.request, ClientFrame::Cancel { .. }) {
                    ClientOutcome::Cancelled
                } else {
                    ClientOutcome::Ready
                };
                (outcome, None)
            }
            Err(error) => {
                let outcome = if error.reason_kind == "client-catalog-generation-mismatch" {
                    ClientOutcome::StaleGeneration
                } else {
                    ClientOutcome::Error
                };
                (outcome, Some(error.reason_kind.to_owned()))
            }
        };
        if outcome != case.expected_outcome
            || case
                .expected_reason_kind
                .as_deref()
                .is_some_and(|expected| reason_kind.as_deref() != Some(expected))
        {
            return Err(error(
                "client-conformance-case-failed",
                format!(
                    "case={} expected={:?}/{:?} actual={outcome:?}/{reason_kind:?}",
                    case.case_id, case.expected_outcome, case.expected_reason_kind
                ),
            ));
        }
        receipts.push(ClientConformanceReceipt {
            case_id: case.case_id.clone(),
            outcome,
            reason_kind,
        });
    }
    Ok(receipts)
}

fn validate_request_params(
    method: &ClientMethod,
    params: &Value,
) -> Result<(), ClientAdmissionError> {
    let params = params.as_object().ok_or_else(|| {
        error(
            "client-request-params-invalid",
            "client request params must be an object",
        )
    })?;
    for name in params.keys() {
        let Some(parameter) = method
            .parameters
            .iter()
            .find(|parameter| parameter.name == *name)
        else {
            return Err(error(
                "client-request-parameter-unknown",
                format!("client parameter is not declared by the catalog: {name}"),
            ));
        };
        if parameter.source == ClientParameterSource::RuntimeContext {
            return Err(error(
                "client-runtime-context-injection-denied",
                format!("client cannot supply RuntimeContext parameter: {name}"),
            ));
        }
    }
    for parameter in &method.parameters {
        if parameter.source == ClientParameterSource::RuntimeContext {
            continue;
        }
        let value = params.get(&parameter.name);
        if value.is_none() && parameter.cardinality == ClientParameterCardinality::Required {
            return Err(error(
                "client-request-parameter-required",
                format!("required client parameter is missing: {}", parameter.name),
            ));
        }
        let Some(value) = value else {
            continue;
        };
        let valid = match parameter.cardinality {
            ClientParameterCardinality::Many => value.as_array().is_some_and(|values| {
                values
                    .iter()
                    .all(|value| parameter_value_matches(parameter.value_type, value))
            }),
            ClientParameterCardinality::Required | ClientParameterCardinality::Optional => {
                parameter_value_matches(parameter.value_type, value)
            }
        };
        if !valid {
            return Err(error(
                "client-request-parameter-type-mismatch",
                format!("client parameter has the wrong type: {}", parameter.name),
            ));
        }
    }
    Ok(())
}

fn parameter_value_matches(value_type: ClientParameterType, value: &Value) -> bool {
    match value_type {
        ClientParameterType::String
        | ClientParameterType::WorkspaceRelativePath
        | ClientParameterType::StructuralSelector
        | ClientParameterType::Presentation => value.is_string(),
        ClientParameterType::StringArray => value
            .as_array()
            .is_some_and(|values| !values.is_empty() && values.iter().all(Value::is_string)),
        ClientParameterType::Boolean => value.is_boolean(),
        ClientParameterType::UnsignedInteger => value.as_u64().is_some(),
        ClientParameterType::Json => true,
    }
}

fn validate_protocol_identity(
    schema_id: &str,
    schema_version: &str,
    protocol_id: &str,
    protocol_version: &str,
    expected_schema_id: &str,
) -> Result<(), ClientAdmissionError> {
    if schema_id != expected_schema_id
        || schema_version != SCHEMA_VERSION
        || protocol_id != CLIENT_PROTOCOL_ID
        || protocol_version != CLIENT_PROTOCOL_VERSION
    {
        return Err(error(
            "client-protocol-identity-mismatch",
            "client protocol identity is unsupported",
        ));
    }
    Ok(())
}

fn validate_digest(field: &str, value: &str) -> Result<(), ClientAdmissionError> {
    let Some(hex) = value
        .strip_prefix("blake3-256:")
        .or_else(|| value.strip_prefix("sha256:"))
    else {
        return Err(error(
            "client-generation-digest-invalid",
            format!("{field} uses an unsupported digest"),
        ));
    };
    if hex.len() != 64
        || !hex
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    {
        return Err(error(
            "client-generation-digest-invalid",
            format!("{field} digest is invalid"),
        ));
    }
    Ok(())
}

fn validate_dotted_id(field: &str, value: &str) -> Result<(), ClientAdmissionError> {
    let valid = value.split('.').count() >= 2
        && value.split('.').all(|segment| {
            !segment.is_empty()
                && segment.as_bytes()[0].is_ascii_lowercase()
                && segment
                    .bytes()
                    .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
        });
    if !valid {
        return Err(error(
            "client-semantic-id-invalid",
            format!("{field} is not a dotted semantic id"),
        ));
    }
    Ok(())
}

fn error(reason_kind: &'static str, message: impl Into<String>) -> ClientAdmissionError {
    ClientAdmissionError {
        reason_kind,
        message: message.into(),
    }
}

#[cfg(test)]
mod tests;
pub use agent_session::{
    AGENT_SESSION_REGISTER_METHOD, AGENT_SESSION_REGISTER_REQUEST_SCHEMA_ID,
    AGENT_SESSION_REGISTER_RESPONSE_SCHEMA_ID, AgentSessionRegisterReceipt,
    AgentSessionRegisterRequest,
};
