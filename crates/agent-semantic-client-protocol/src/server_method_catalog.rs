// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Server-owned northbound method catalog.
//!
//! Provider runtime operations are southbound implementation capabilities.
//! They must never decide whether an ASP Client can discover the stable
//! language-neutral search and query surfaces.

use std::collections::BTreeSet;

use crate::AGENT_SESSION_REGISTER_METHOD;
use crate::AGENT_SESSION_REGISTER_REQUEST_SCHEMA_ID;
use crate::AGENT_SESSION_REGISTER_RESPONSE_SCHEMA_ID;
use crate::ClientCapabilities;
use crate::ClientMethod;
use crate::ClientParameter;
use crate::ClientParameterCardinality;
use crate::ClientParameterSource;
use crate::ClientParameterType;
use crate::ClientProtocolCatalog;
use crate::ClientTransport;
use crate::SCHEMA_BUNDLE_METHOD;
use crate::SCHEMA_BUNDLE_REQUEST_SCHEMA_ID;
use crate::SCHEMA_BUNDLE_RESPONSE_SCHEMA_ID;
use crate::protocol_identity::CLIENT_CATALOG_SCHEMA_ID;
use crate::protocol_identity::CLIENT_PROTOCOL_ID;
use crate::protocol_identity::CLIENT_PROTOCOL_VERSION;
use crate::protocol_identity::SCHEMA_VERSION;

const ROUTE_FAILURE_SCHEMA_ID: &str = "agent.semantic-protocols.route-failure";
/// Temporary absolute first-computation measurement boundary, not a latency SLO.
/// Kept shared so the transport cannot expire before the server's typed terminal.
pub const FIRST_COMPUTATION_OBSERVATION_BUDGET: std::time::Duration =
    std::time::Duration::from_secs(60);
const QUERY_RESULT_SCHEMA_ID: &str = "agent.semantic-protocols.query-result";
const SOURCE_INDEX_LOOKUP_REQUEST_SCHEMA_ID: &str =
    "agent.semantic-protocols.asp-client-source-index-lookup-request";
const SOURCE_INDEX_LOOKUP_RESPONSE_SCHEMA_ID: &str =
    "agent.semantic-protocols.resident-search-result";
const EXACT_QUERY_REQUEST_SCHEMA_ID: &str =
    "agent.semantic-protocols.asp-client-exact-query-request";
const WORKSPACE_SEARCH_PLAYBOOK_REQUEST_SCHEMA_ID: &str =
    "agent.semantic-protocols.asp-client-workspace-search-playbook-request";
const WORKSPACE_SEARCH_PLAYBOOK_RESPONSE_SCHEMA_ID: &str =
    "agent.semantic-protocols.search-topology-settlement";
const WORKSPACE_QUERY_PLAYBOOK_REQUEST_SCHEMA_ID: &str =
    "agent.semantic-protocols.asp-client-workspace-query-playbook-request";
const WORKSPACE_QUERY_PLAYBOOK_RESPONSE_SCHEMA_ID: &str =
    "agent.semantic-protocols.query-playbook-materialization-receipt";
const WORKSPACE_SYNTAX_QUERY_REQUEST_SCHEMA_ID: &str =
    "agent.semantic-protocols.asp-client-workspace-syntax-query-request";
const WORKSPACE_SYNTAX_QUERY_RESPONSE_SCHEMA_ID: &str =
    "agent.semantic-protocols.asp-client-workspace-syntax-query-response";
const WORKSPACE_SYNTAX_PLAN_CONTEXT_REQUEST_SCHEMA_ID: &str =
    "agent.semantic-protocols.asp-client-workspace-syntax-plan-context-request";
const WORKSPACE_SYNTAX_PLAN_CONTEXT_RESPONSE_SCHEMA_ID: &str =
    "agent.semantic-protocols.asp-client-workspace-syntax-plan-context-response";

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
pub const WORKSPACE_SEARCH_PLAYBOOK_METHOD: &str = "asp.workspace.search.playbook";
pub const WORKSPACE_QUERY_PLAYBOOK_METHOD: &str = "asp.workspace.query.playbook";
pub const WORKSPACE_SYNTAX_QUERY_METHOD: &str = "asp.workspace.query.syntax";
pub const WORKSPACE_SYNTAX_PLAN_CONTEXT_METHOD: &str = "asp.workspace.query.syntax-plan-context";
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
    ResidentGenerationRead,
    ColdGenerationAdmission,
}

#[must_use]
pub fn classify_client_dispatch(method: &str) -> ClientDispatchClass {
    if method == WORKSPACE_GENERATION_ENSURE_READY_METHOD {
        ClientDispatchClass::ColdGenerationAdmission
    } else if method == WORKSPACE_SEARCH_PLAYBOOK_METHOD
        || method == WORKSPACE_QUERY_PLAYBOOK_METHOD
        || method == WORKSPACE_SYNTAX_QUERY_METHOD
        || method == WORKSPACE_SYNTAX_PLAN_CONTEXT_METHOD
        || method == GRAPH_EVALUATE_METHOD
        || method.ends_with(".query")
    {
        // Ready reads are strictly bounded. First-computation requests may
        // join server-owned publication under a separate absolute deadline;
        // the request never becomes a CompleteGeneration admission authority.
        ClientDispatchClass::ResidentGenerationRead
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
    WorkspaceSearchPlaybook,
    WorkspaceQueryPlaybook,
    WorkspaceSyntaxQuery,
    WorkspaceSyntaxPlanContext,
    GraphEvaluate,
    GraphsTimeline,
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
            Self::WorkspaceSearchPlaybook => "workspace.search.playbook",
            Self::WorkspaceQueryPlaybook => "workspace.query.playbook",
            Self::WorkspaceSyntaxQuery => "workspace.query.syntax",
            Self::WorkspaceSyntaxPlanContext => "workspace.query.syntax-plan-context",
            Self::GraphEvaluate => "graph.evaluate",
            Self::GraphsTimeline => "graphs.timeline",
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
        workspace_search_playbook_method(),
        workspace_query_playbook_method(),
        workspace_syntax_query_method(),
        workspace_syntax_plan_context_method(),
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
        route_id: client_route_id(MULTI_AGENT_HOST_EVENT_METHOD),
        request_schema_id: client_schema_id(
            "agent.semantic-protocols.codex-multi-agent-v2-host-lifecycle-event",
        ),
        response_schema_id: client_schema_id("agent.semantic-protocols.agent-session-host-binding"),
        error_schema_ids: vec![client_schema_id(ROUTE_FAILURE_SCHEMA_ID)],
        parameters: vec![required("event", ClientParameterType::Json)],
        cancellable: false,
        streaming: false,
    }
}

fn multi_agent_children_method() -> ClientMethod {
    ClientMethod {
        method: MULTI_AGENT_CHILDREN_METHOD.to_owned(),
        route_id: client_route_id(MULTI_AGENT_CHILDREN_METHOD),
        request_schema_id: client_schema_id(
            "agent.semantic-protocols.agent-session-lifecycle-projection",
        ),
        response_schema_id: client_schema_id(
            "agent.semantic-protocols.codex-multi-agent-v2-control-plane-projection",
        ),
        error_schema_ids: vec![client_schema_id(ROUTE_FAILURE_SCHEMA_ID)],
        parameters: vec![required_string("rootSessionId")],
        cancellable: false,
        streaming: false,
    }
}

fn agent_session_register_method() -> ClientMethod {
    ClientMethod {
        method: AGENT_SESSION_REGISTER_METHOD.to_owned(),
        route_id: client_route_id(AGENT_SESSION_REGISTER_METHOD),
        request_schema_id: client_schema_id(AGENT_SESSION_REGISTER_REQUEST_SCHEMA_ID),
        response_schema_id: client_schema_id(AGENT_SESSION_REGISTER_RESPONSE_SCHEMA_ID),
        error_schema_ids: vec![client_schema_id(ROUTE_FAILURE_SCHEMA_ID)],
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
        route_id: client_route_id(LIVE_CORPUS_CACHE_STATE_METHOD),
        request_schema_id: client_schema_id(crate::LIVE_CORPUS_CACHE_STATE_REQUEST_SCHEMA_ID),
        response_schema_id: client_schema_id(crate::LIVE_CORPUS_CACHE_STATE_RECEIPT_SCHEMA_ID),
        error_schema_ids: vec![client_schema_id(ROUTE_FAILURE_SCHEMA_ID)],
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
        route_id: client_route_id(SCHEMA_BUNDLE_METHOD),
        request_schema_id: client_schema_id(SCHEMA_BUNDLE_REQUEST_SCHEMA_ID),
        response_schema_id: client_schema_id(SCHEMA_BUNDLE_RESPONSE_SCHEMA_ID),
        error_schema_ids: vec![client_schema_id(ROUTE_FAILURE_SCHEMA_ID)],
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
        route_id: client_route_id(CANCELLATION_PROBE_METHOD),
        request_schema_id: client_schema_id(CANCELLATION_PROBE_REQUEST_SCHEMA_ID),
        response_schema_id: client_schema_id(CANCELLATION_PROBE_RESPONSE_SCHEMA_ID),
        error_schema_ids: vec![client_schema_id(ROUTE_FAILURE_SCHEMA_ID)],
        parameters: Vec::new(),
        cancellable: true,
        streaming: false,
    }
}

fn client_route_id(value: &str) -> crate::ClientRouteId {
    crate::ClientRouteId::new(value).expect("static client route id")
}

fn client_schema_id(value: &str) -> crate::ClientSchemaId {
    crate::ClientSchemaId::new(value).expect("static client schema id")
}

fn workspace_generation_ensure_ready_method() -> ClientMethod {
    ClientMethod {
        method: WORKSPACE_GENERATION_ENSURE_READY_METHOD.to_owned(),
        route_id: client_route_id(WORKSPACE_GENERATION_ENSURE_READY_METHOD),
        request_schema_id: client_schema_id(WORKSPACE_GENERATION_ENSURE_READY_REQUEST_SCHEMA_ID),
        response_schema_id: client_schema_id(WORKSPACE_GENERATION_ENSURE_READY_RESPONSE_SCHEMA_ID),
        error_schema_ids: vec![client_schema_id(ROUTE_FAILURE_SCHEMA_ID)],
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
    if method == WORKSPACE_SEARCH_PLAYBOOK_METHOD {
        return Ok(ResolvedServerClientMethod::Server(
            ServerClientRoute::WorkspaceSearchPlaybook,
        ));
    }
    if method == WORKSPACE_QUERY_PLAYBOOK_METHOD {
        return Ok(ResolvedServerClientMethod::Server(
            ServerClientRoute::WorkspaceQueryPlaybook,
        ));
    }
    if method == WORKSPACE_SYNTAX_QUERY_METHOD {
        return Ok(ResolvedServerClientMethod::Server(
            ServerClientRoute::WorkspaceSyntaxQuery,
        ));
    }
    if method == WORKSPACE_SYNTAX_PLAN_CONTEXT_METHOD {
        return Ok(ResolvedServerClientMethod::Server(
            ServerClientRoute::WorkspaceSyntaxPlanContext,
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
        "source-index.lookup" => Some(ServerClientRoute::SourceIndexLookup),
        "query" => Some(ServerClientRoute::ExactQuery),
        _ => None,
    }
}

fn graph_evaluate_method() -> ClientMethod {
    ClientMethod {
        method: GRAPH_EVALUATE_METHOD.to_owned(),
        route_id: client_route_id(GRAPH_EVALUATE_METHOD),
        request_schema_id: client_schema_id(GRAPH_EVALUATE_REQUEST_SCHEMA_ID),
        response_schema_id: client_schema_id(GRAPH_EVALUATE_RESPONSE_SCHEMA_ID),
        error_schema_ids: vec![client_schema_id(ROUTE_FAILURE_SCHEMA_ID)],
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
        route_id: client_route_id(GRAPH_TIMELINE_METHOD),
        request_schema_id: client_schema_id(GRAPH_TIMELINE_REQUEST_SCHEMA_ID),
        response_schema_id: client_schema_id(GRAPH_TIMELINE_RESPONSE_SCHEMA_ID),
        error_schema_ids: vec![client_schema_id(ROUTE_FAILURE_SCHEMA_ID)],
        parameters: vec![
            required("eventPacket", ClientParameterType::Json),
            required("arguments", ClientParameterType::StringArray),
        ],
        cancellable: true,
        streaming: false,
    }
}

fn workspace_search_playbook_method() -> ClientMethod {
    ClientMethod {
        method: WORKSPACE_SEARCH_PLAYBOOK_METHOD.to_owned(),
        route_id: client_route_id(WORKSPACE_SEARCH_PLAYBOOK_METHOD),
        request_schema_id: client_schema_id(WORKSPACE_SEARCH_PLAYBOOK_REQUEST_SCHEMA_ID),
        response_schema_id: client_schema_id(WORKSPACE_SEARCH_PLAYBOOK_RESPONSE_SCHEMA_ID),
        error_schema_ids: vec![client_schema_id(ROUTE_FAILURE_SCHEMA_ID)],
        parameters: vec![
            required_string("schemaId"),
            required_string("schemaVersion"),
            optional("languages", ClientParameterType::String),
            optional("documents", ClientParameterType::String),
            optional("workspace", ClientParameterType::String),
            optional("fd", ClientParameterType::Json),
            optional("rg", ClientParameterType::Json),
            optional("tantivy", ClientParameterType::Json),
            optional("syntax", ClientParameterType::Json),
            optional("nativeSyntax", ClientParameterType::StringArray),
            optional("graph", ClientParameterType::Json),
            required("clauseOrder", ClientParameterType::Json),
        ],
        cancellable: true,
        streaming: false,
    }
}

fn workspace_query_playbook_method() -> ClientMethod {
    ClientMethod {
        method: WORKSPACE_QUERY_PLAYBOOK_METHOD.to_owned(),
        route_id: client_route_id(WORKSPACE_QUERY_PLAYBOOK_METHOD),
        request_schema_id: client_schema_id(WORKSPACE_QUERY_PLAYBOOK_REQUEST_SCHEMA_ID),
        response_schema_id: client_schema_id(WORKSPACE_QUERY_PLAYBOOK_RESPONSE_SCHEMA_ID),
        error_schema_ids: vec![client_schema_id(ROUTE_FAILURE_SCHEMA_ID)],
        parameters: vec![
            required_string("schemaId"),
            required_string("schemaVersion"),
            required("selectors", ClientParameterType::StringArray),
            required_string("projection"),
        ],
        cancellable: true,
        streaming: false,
    }
}

fn workspace_syntax_query_method() -> ClientMethod {
    ClientMethod {
        method: WORKSPACE_SYNTAX_QUERY_METHOD.to_owned(),
        route_id: client_route_id(WORKSPACE_SYNTAX_QUERY_METHOD),
        request_schema_id: client_schema_id(WORKSPACE_SYNTAX_QUERY_REQUEST_SCHEMA_ID),
        response_schema_id: client_schema_id(WORKSPACE_SYNTAX_QUERY_RESPONSE_SCHEMA_ID),
        error_schema_ids: vec![client_schema_id(ROUTE_FAILURE_SCHEMA_ID)],
        parameters: vec![
            required_string("schemaId"),
            required_string("schemaVersion"),
            optional("languages", ClientParameterType::String),
            optional("documents", ClientParameterType::String),
            optional("workspace", ClientParameterType::String),
            required("syntax", ClientParameterType::Json),
            required_string("projection"),
        ],
        cancellable: true,
        streaming: false,
    }
}

fn workspace_syntax_plan_context_method() -> ClientMethod {
    ClientMethod {
        method: WORKSPACE_SYNTAX_PLAN_CONTEXT_METHOD.to_owned(),
        route_id: client_route_id(WORKSPACE_SYNTAX_PLAN_CONTEXT_METHOD),
        request_schema_id: client_schema_id(WORKSPACE_SYNTAX_PLAN_CONTEXT_REQUEST_SCHEMA_ID),
        response_schema_id: client_schema_id(WORKSPACE_SYNTAX_PLAN_CONTEXT_RESPONSE_SCHEMA_ID),
        error_schema_ids: vec![client_schema_id(ROUTE_FAILURE_SCHEMA_ID)],
        parameters: vec![
            required_string("schemaId"),
            required_string("schemaVersion"),
            required_string("producer"),
        ],
        cancellable: true,
        streaming: false,
    }
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
        route_id: crate::ClientRouteId::new(route_id).expect("derived client route id"),
        request_schema_id: client_schema_id(request_schema_id),
        response_schema_id: client_schema_id(response_schema_id),
        error_schema_ids: vec![client_schema_id(ROUTE_FAILURE_SCHEMA_ID)],
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
