//! Server-owned northbound method catalog.
//!
//! Provider runtime operations are southbound implementation capabilities.
//! They must never decide whether an ASP Client can discover the stable
//! language-neutral search and query surfaces.

use std::collections::BTreeSet;

use crate::{
    AGENT_SESSION_REGISTER_METHOD, AGENT_SESSION_REGISTER_REQUEST_SCHEMA_ID,
    AGENT_SESSION_REGISTER_RESPONSE_SCHEMA_ID, CLIENT_CATALOG_SCHEMA_ID, CLIENT_PROTOCOL_ID,
    CLIENT_PROTOCOL_VERSION, ClientCapabilities, ClientMethod, ClientParameter,
    ClientParameterCardinality, ClientParameterSource, ClientParameterType, ClientProtocolCatalog,
    ClientTransport, SCHEMA_BUNDLE_METHOD, SCHEMA_BUNDLE_REQUEST_SCHEMA_ID,
    SCHEMA_BUNDLE_RESPONSE_SCHEMA_ID, SCHEMA_VERSION,
};

const ROUTE_FAILURE_SCHEMA_ID: &str = "agent.semantic-protocols.route-failure";
const SEARCH_PACKET_SCHEMA_ID: &str = "agent.semantic-protocols.search-packet";
const QUERY_RESULT_SCHEMA_ID: &str = "agent.semantic-protocols.query-result";
const SEARCH_REQUEST_SCHEMA_ID: &str = "agent.semantic-protocols.asp-client-search-request";
const SOURCE_INDEX_LOOKUP_REQUEST_SCHEMA_ID: &str =
    "agent.semantic-protocols.asp-client-source-index-lookup-request";
const SOURCE_INDEX_LOOKUP_RESPONSE_SCHEMA_ID: &str =
    "agent.semantic-protocols.resident-search-result";
const EXACT_QUERY_REQUEST_SCHEMA_ID: &str =
    "agent.semantic-protocols.asp-client-exact-query-request";

pub const GRAPH_EVALUATE_METHOD: &str = "asp.graph.evaluate";
pub const GRAPH_EVALUATE_REQUEST_SCHEMA_ID: &str =
    "agent.semantic-protocols.semantic-graph-resident-evaluation-request";
pub const GRAPH_EVALUATE_RESPONSE_SCHEMA_ID: &str =
    "agent.semantic-protocols.semantic-graph-resident-evaluation-result";
pub const GRAPH_TIMELINE_METHOD: &str = "asp.graphs.timeline";
pub const GRAPH_TIMELINE_REQUEST_SCHEMA_ID: &str =
    "agent.semantic-protocols.asp-client-graphs-timeline-request";
pub const GRAPH_TIMELINE_RESPONSE_SCHEMA_ID: &str =
    "agent.semantic-protocols.graph-turbo-artifact-timeline";

pub const CANCELLATION_PROBE_METHOD: &str = "asp.lifecycle.cancellation";
pub const CANCELLATION_PROBE_REQUEST_SCHEMA_ID: &str =
    "agent.semantic-protocols.asp-client-cancellation-probe-request";
pub const CANCELLATION_PROBE_RESPONSE_SCHEMA_ID: &str =
    "agent.semantic-protocols.asp-client-cancellation-probe-response";
pub const WORKSPACE_GENERATION_ENSURE_READY_METHOD: &str = "asp.workspace.generation.ensure-ready";
pub const WORKSPACE_GENERATION_ENSURE_READY_REQUEST_SCHEMA_ID: &str =
    "agent.semantic-protocols.asp-client-workspace-generation-ensure-ready-request";
pub const WORKSPACE_GENERATION_ENSURE_READY_RESPONSE_SCHEMA_ID: &str =
    "agent.semantic-protocols.runtime-server-workspace-generation-admission";

/// Method-derived lifecycle class shared by every ClientFrame adapter.
/// Concrete interactive durations remain layer-local, but an adapter cannot
/// reinterpret cold generation admission as an interactive read.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ClientDispatchClass {
    InteractiveRead,
    ColdGenerationAdmission,
}

#[must_use]
pub fn classify_client_dispatch(method: &str) -> ClientDispatchClass {
    if method == WORKSPACE_GENERATION_ENSURE_READY_METHOD {
        ClientDispatchClass::ColdGenerationAdmission
    } else {
        ClientDispatchClass::InteractiveRead
    }
}
pub const MULTI_AGENT_HOST_EVENT_METHOD: &str = "asp.session.host-event";
pub const MULTI_AGENT_CHILDREN_METHOD: &str = "asp.session.children";
pub const LIVE_CORPUS_CACHE_STATE_METHOD: &str = "asp.live-corpus.cache-state";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ServerClientRoute {
    AgentSessionRegister,
    MultiAgentHostEvent,
    MultiAgentChildren,
    CancellationProbe,
    LiveCorpusCacheState,
    WorkspaceGenerationEnsureReady,
    GraphEvaluate,
    GraphsTimeline,
    Search,
    SourceIndexLookup,
    ExactQuery,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ResolvedServerClientMethod {
    Server(ServerClientRoute),
    Language {
        language_id: String,
        route: ServerClientRoute,
    },
}

impl ServerClientRoute {
    pub const fn operation(self) -> &'static str {
        match self {
            Self::AgentSessionRegister => "session.register-child",
            Self::MultiAgentHostEvent => "session.host-event",
            Self::MultiAgentChildren => "session.children",
            Self::CancellationProbe => "lifecycle.cancellation",
            Self::LiveCorpusCacheState => "live-corpus.cache-state",
            Self::WorkspaceGenerationEnsureReady => "workspace.generation.ensure-ready",
            Self::GraphEvaluate => "graph.evaluate",
            Self::GraphsTimeline => "graphs.timeline",
            Self::Search => "search",
            Self::SourceIndexLookup => "source-index.lookup",
            Self::ExactQuery => "query",
        }
    }
}

pub fn server_client_methods(
    language_ids: impl IntoIterator<Item = String>,
) -> Result<Vec<ClientMethod>, String> {
    let mut admitted = BTreeSet::new();
    let mut methods = Vec::new();
    for language_id in language_ids {
        if !admitted.insert(language_id.clone()) {
            return Err(format!(
                "state=route-ambiguous reasonKind=multiple-installed-client-languages languageId={language_id}"
            ));
        }
        methods.extend([
            search_method(&language_id),
            source_index_lookup_method(&language_id),
            exact_query_method(&language_id),
        ]);
    }
    methods.extend([
        agent_session_register_method(),
        multi_agent_host_event_method(),
        multi_agent_children_method(),
        cancellation_probe_method(),
        live_corpus_cache_state_method(),
        workspace_generation_ensure_ready_method(),
        schema_bundle_method(),
        graph_evaluate_method(),
        graphs_timeline_method(),
    ]);
    methods.sort_by(|left, right| left.method.cmp(&right.method));
    Ok(methods)
}

fn multi_agent_host_event_method() -> ClientMethod {
    ClientMethod {
        method: MULTI_AGENT_HOST_EVENT_METHOD.to_owned(),
        route_id: MULTI_AGENT_HOST_EVENT_METHOD.to_owned(),
        request_schema_id: "agent.semantic-protocols.codex-multi-agent-v2-host-lifecycle-event"
            .to_owned(),
        response_schema_id: "agent.semantic-protocols.agent-session-host-binding".to_owned(),
        error_schema_ids: vec![ROUTE_FAILURE_SCHEMA_ID.to_owned()],
        parameters: vec![required("event", ClientParameterType::Json)],
        cancellable: false,
        streaming: false,
    }
}

fn multi_agent_children_method() -> ClientMethod {
    ClientMethod {
        method: MULTI_AGENT_CHILDREN_METHOD.to_owned(),
        route_id: MULTI_AGENT_CHILDREN_METHOD.to_owned(),
        request_schema_id: "agent.semantic-protocols.agent-session-lifecycle-projection".to_owned(),
        response_schema_id:
            "agent.semantic-protocols.codex-multi-agent-v2-control-plane-projection".to_owned(),
        error_schema_ids: vec![ROUTE_FAILURE_SCHEMA_ID.to_owned()],
        parameters: vec![required_string("rootSessionId")],
        cancellable: false,
        streaming: false,
    }
}

fn agent_session_register_method() -> ClientMethod {
    ClientMethod {
        method: AGENT_SESSION_REGISTER_METHOD.to_owned(),
        route_id: AGENT_SESSION_REGISTER_METHOD.to_owned(),
        request_schema_id: AGENT_SESSION_REGISTER_REQUEST_SCHEMA_ID.to_owned(),
        response_schema_id: AGENT_SESSION_REGISTER_RESPONSE_SCHEMA_ID.to_owned(),
        error_schema_ids: vec![ROUTE_FAILURE_SCHEMA_ID.to_owned()],
        parameters: vec![
            required_string("schemaId"),
            required_string("schemaVersion"),
            required_string("rootSessionId"),
            required_string("parentThreadId"),
            required_string("childThreadId"),
            required_string("agentName"),
            required_string("agentPath"),
            required_string("routeKey"),
        ],
        cancellable: false,
        streaming: false,
    }
}

fn live_corpus_cache_state_method() -> ClientMethod {
    ClientMethod {
        method: LIVE_CORPUS_CACHE_STATE_METHOD.to_owned(),
        route_id: LIVE_CORPUS_CACHE_STATE_METHOD.to_owned(),
        request_schema_id: crate::LIVE_CORPUS_CACHE_STATE_REQUEST_SCHEMA_ID.to_owned(),
        response_schema_id: crate::LIVE_CORPUS_CACHE_STATE_RECEIPT_SCHEMA_ID.to_owned(),
        error_schema_ids: vec![ROUTE_FAILURE_SCHEMA_ID.to_owned()],
        parameters: vec![
            required_string("schemaId"),
            required_string("schemaVersion"),
            required_string("operationId"),
            required_string("resourceId"),
            required_string("languageId"),
            required_string("providerId"),
            required_string("artifactDigest"),
            required_string("cacheState"),
            required_string("prepareAction"),
            required_string("mutationScope"),
            optional("expectedGenerationDigest", ClientParameterType::Json),
            optional("expectedRootDigest", ClientParameterType::Json),
        ],
        cancellable: false,
        streaming: false,
    }
}

fn schema_bundle_method() -> ClientMethod {
    ClientMethod {
        method: SCHEMA_BUNDLE_METHOD.to_owned(),
        route_id: SCHEMA_BUNDLE_METHOD.to_owned(),
        request_schema_id: SCHEMA_BUNDLE_REQUEST_SCHEMA_ID.to_owned(),
        response_schema_id: SCHEMA_BUNDLE_RESPONSE_SCHEMA_ID.to_owned(),
        error_schema_ids: vec![ROUTE_FAILURE_SCHEMA_ID.to_owned()],
        parameters: vec![
            required_string("schemaId"),
            required_string("schemaVersion"),
            required_string("languageId"),
            required("rootSetIds", ClientParameterType::StringArray),
            optional("knownBundleDigest", ClientParameterType::String),
        ],
        cancellable: false,
        streaming: false,
    }
}

fn cancellation_probe_method() -> ClientMethod {
    ClientMethod {
        method: CANCELLATION_PROBE_METHOD.to_owned(),
        route_id: CANCELLATION_PROBE_METHOD.to_owned(),
        request_schema_id: CANCELLATION_PROBE_REQUEST_SCHEMA_ID.to_owned(),
        response_schema_id: CANCELLATION_PROBE_RESPONSE_SCHEMA_ID.to_owned(),
        error_schema_ids: vec![ROUTE_FAILURE_SCHEMA_ID.to_owned()],
        parameters: Vec::new(),
        cancellable: true,
        streaming: false,
    }
}

fn workspace_generation_ensure_ready_method() -> ClientMethod {
    ClientMethod {
        method: WORKSPACE_GENERATION_ENSURE_READY_METHOD.to_owned(),
        route_id: WORKSPACE_GENERATION_ENSURE_READY_METHOD.to_owned(),
        request_schema_id: WORKSPACE_GENERATION_ENSURE_READY_REQUEST_SCHEMA_ID.to_owned(),
        response_schema_id: WORKSPACE_GENERATION_ENSURE_READY_RESPONSE_SCHEMA_ID.to_owned(),
        error_schema_ids: vec![ROUTE_FAILURE_SCHEMA_ID.to_owned()],
        parameters: vec![ClientParameter {
            name: "languageId".to_owned(),
            value_type: ClientParameterType::String,
            cardinality: ClientParameterCardinality::Optional,
            source: ClientParameterSource::Request,
        }],
        cancellable: true,
        streaming: false,
    }
}

pub fn server_client_catalog(
    catalog_generation: String,
    workspace_generation: String,
    transports: Vec<ClientTransport>,
    language_ids: impl IntoIterator<Item = String>,
) -> Result<ClientProtocolCatalog, String> {
    // `server_client_methods` is the single authority for both language routes
    // and shared Runtime routes.  Do not append shared methods here: doing so
    // makes catalog construction order-dependent and turns an exact replay of
    // the cancellation route into a duplicate declaration.
    let mut methods = server_client_methods(language_ids)?;
    methods.sort_by(|left, right| left.method.cmp(&right.method));
    let catalog = ClientProtocolCatalog {
        schema_id: CLIENT_CATALOG_SCHEMA_ID.to_owned(),
        schema_version: SCHEMA_VERSION.to_owned(),
        protocol_id: CLIENT_PROTOCOL_ID.to_owned(),
        protocol_version: CLIENT_PROTOCOL_VERSION.to_owned(),
        catalog_generation,
        workspace_generation,
        transports,
        capabilities: ClientCapabilities {
            request_cancellation: methods.iter().any(|method| method.cancellable),
            events: true,
            streaming: methods.iter().any(|method| method.streaming),
            trace_context: true,
        },
        methods,
    };
    catalog
        .validate()
        .map_err(|error| format!("{}: {}", error.reason_kind, error.message))?;
    Ok(catalog)
}

pub fn resolve_server_client_method(
    method: &str,
    language_ids: impl IntoIterator<Item = String>,
) -> Result<(String, ServerClientRoute), String> {
    match resolve_server_client_method_owner(method, language_ids)? {
        ResolvedServerClientMethod::Language { language_id, route } => Ok((language_id, route)),
        ResolvedServerClientMethod::Server(_) => Err(format!(
            "state=route-owner-mismatch reasonKind=server-owned-method-requires-server-dispatch method={method}"
        )),
    }
}

pub fn resolve_server_client_method_owner(
    method: &str,
    language_ids: impl IntoIterator<Item = String>,
) -> Result<ResolvedServerClientMethod, String> {
    if method == GRAPH_EVALUATE_METHOD {
        return Ok(ResolvedServerClientMethod::Server(
            ServerClientRoute::GraphEvaluate,
        ));
    }
    if method == AGENT_SESSION_REGISTER_METHOD {
        return Ok(ResolvedServerClientMethod::Server(
            ServerClientRoute::AgentSessionRegister,
        ));
    }
    if method == MULTI_AGENT_HOST_EVENT_METHOD {
        return Ok(ResolvedServerClientMethod::Server(
            ServerClientRoute::MultiAgentHostEvent,
        ));
    }
    if method == MULTI_AGENT_CHILDREN_METHOD {
        return Ok(ResolvedServerClientMethod::Server(
            ServerClientRoute::MultiAgentChildren,
        ));
    }
    if method == LIVE_CORPUS_CACHE_STATE_METHOD {
        return Ok(ResolvedServerClientMethod::Server(
            ServerClientRoute::LiveCorpusCacheState,
        ));
    }
    if method == WORKSPACE_GENERATION_ENSURE_READY_METHOD {
        return Ok(ResolvedServerClientMethod::Server(
            ServerClientRoute::WorkspaceGenerationEnsureReady,
        ));
    }
    if method == GRAPH_TIMELINE_METHOD {
        return Ok(ResolvedServerClientMethod::Server(
            ServerClientRoute::GraphsTimeline,
        ));
    }
    let mut resolved = None;
    for language_id in language_ids {
        let Some(route) = method
            .strip_prefix(&format!("{language_id}."))
            .and_then(route_from_suffix)
        else {
            continue;
        };
        if resolved.is_some() {
            return Err(format!(
                "state=route-ambiguous reasonKind=multiple-installed-client-methods method={method}"
            ));
        }
        resolved = Some(ResolvedServerClientMethod::Language { language_id, route });
    }
    resolved.ok_or_else(|| {
        format!(
            "state=route-missing reasonKind=method-not-in-server-client-catalog method={method}"
        )
    })
}

fn route_from_suffix(suffix: &str) -> Option<ServerClientRoute> {
    match suffix {
        "search" => Some(ServerClientRoute::Search),
        "source-index.lookup" => Some(ServerClientRoute::SourceIndexLookup),
        "query" => Some(ServerClientRoute::ExactQuery),
        _ => None,
    }
}

fn graph_evaluate_method() -> ClientMethod {
    ClientMethod {
        method: GRAPH_EVALUATE_METHOD.to_owned(),
        route_id: GRAPH_EVALUATE_METHOD.to_owned(),
        request_schema_id: GRAPH_EVALUATE_REQUEST_SCHEMA_ID.to_owned(),
        response_schema_id: GRAPH_EVALUATE_RESPONSE_SCHEMA_ID.to_owned(),
        error_schema_ids: vec![ROUTE_FAILURE_SCHEMA_ID.to_owned()],
        parameters: vec![
            required_string("schemaId"),
            required_string("schemaVersion"),
            required_string("protocolId"),
            required_string("protocolVersion"),
            required_string("packetKind"),
            required_string("languageId"),
            required_string("surface"),
            required("queryTerms", ClientParameterType::StringArray),
            required_string("profile"),
            required("entryNodeIds", ClientParameterType::StringArray),
            required("budget", ClientParameterType::Json),
        ],
        cancellable: true,
        streaming: false,
    }
}

fn graphs_timeline_method() -> ClientMethod {
    ClientMethod {
        method: GRAPH_TIMELINE_METHOD.to_owned(),
        route_id: GRAPH_TIMELINE_METHOD.to_owned(),
        request_schema_id: GRAPH_TIMELINE_REQUEST_SCHEMA_ID.to_owned(),
        response_schema_id: GRAPH_TIMELINE_RESPONSE_SCHEMA_ID.to_owned(),
        error_schema_ids: vec![ROUTE_FAILURE_SCHEMA_ID.to_owned()],
        parameters: vec![
            required("eventPacket", ClientParameterType::Json),
            required("arguments", ClientParameterType::StringArray),
        ],
        cancellable: true,
        streaming: false,
    }
}

fn search_method(language_id: &str) -> ClientMethod {
    method(
        language_id,
        ServerClientRoute::Search,
        SEARCH_REQUEST_SCHEMA_ID,
        SEARCH_PACKET_SCHEMA_ID,
        vec![
            required_string("schemaId"),
            required_string("schemaVersion"),
            required_string("intent"),
            required_string("query"),
        ],
    )
}

fn source_index_lookup_method(language_id: &str) -> ClientMethod {
    method(
        language_id,
        ServerClientRoute::SourceIndexLookup,
        SOURCE_INDEX_LOOKUP_REQUEST_SCHEMA_ID,
        SOURCE_INDEX_LOOKUP_RESPONSE_SCHEMA_ID,
        vec![
            required_string("schemaId"),
            required_string("schemaVersion"),
            required_string("query"),
            required("indexRoot", ClientParameterType::String),
            required("limit", ClientParameterType::UnsignedInteger),
        ],
    )
}

fn exact_query_method(language_id: &str) -> ClientMethod {
    method(
        language_id,
        ServerClientRoute::ExactQuery,
        EXACT_QUERY_REQUEST_SCHEMA_ID,
        QUERY_RESULT_SCHEMA_ID,
        vec![
            required_string("schemaId"),
            required_string("schemaVersion"),
            required("selector", ClientParameterType::StructuralSelector),
            optional("projection", ClientParameterType::Presentation),
        ],
    )
}

fn method(
    language_id: &str,
    route: ServerClientRoute,
    request_schema_id: &str,
    response_schema_id: &str,
    parameters: Vec<ClientParameter>,
) -> ClientMethod {
    let route_id = format!("{language_id}.{}", route.operation());
    ClientMethod {
        method: route_id.clone(),
        route_id,
        request_schema_id: request_schema_id.to_owned(),
        response_schema_id: response_schema_id.to_owned(),
        error_schema_ids: vec![ROUTE_FAILURE_SCHEMA_ID.to_owned()],
        parameters,
        cancellable: true,
        streaming: false,
    }
}

fn required_string(name: &str) -> ClientParameter {
    required(name, ClientParameterType::String)
}

fn required(name: &str, value_type: ClientParameterType) -> ClientParameter {
    parameter(name, value_type, ClientParameterCardinality::Required)
}

fn optional(name: &str, value_type: ClientParameterType) -> ClientParameter {
    parameter(name, value_type, ClientParameterCardinality::Optional)
}

fn parameter(
    name: &str,
    value_type: ClientParameterType,
    cardinality: ClientParameterCardinality,
) -> ClientParameter {
    ClientParameter {
        name: name.to_owned(),
        value_type,
        cardinality,
        source: ClientParameterSource::Request,
    }
}
