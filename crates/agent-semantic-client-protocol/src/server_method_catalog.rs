//! Server-owned northbound method catalog.
//!
//! Provider runtime operations are southbound implementation capabilities.
//! They must never decide whether an ASP Client can discover the stable
//! language-neutral search and query surfaces.

use std::collections::BTreeSet;

use crate::{
    CLIENT_CATALOG_SCHEMA_ID, CLIENT_PROTOCOL_ID, CLIENT_PROTOCOL_VERSION, ClientCapabilities,
    ClientMethod, ClientParameter, ClientParameterCardinality, ClientParameterSource,
    ClientParameterType, ClientProtocolCatalog, ClientTransport, SCHEMA_VERSION,
};

const ROUTE_FAILURE_SCHEMA_ID: &str = "agent.semantic-protocols.route-failure";
const SEARCH_PACKET_SCHEMA_ID: &str = "agent.semantic-protocols.search-packet";
const QUERY_RESULT_SCHEMA_ID: &str = "agent.semantic-protocols.query-result";
const SEARCH_REQUEST_SCHEMA_ID: &str = "agent.semantic-protocols.asp-client-search-request";
const EXACT_QUERY_REQUEST_SCHEMA_ID: &str =
    "agent.semantic-protocols.asp-client-exact-query-request";
const OWNER_SEARCH_REQUEST_SCHEMA_ID: &str =
    "agent.semantic-protocols.asp-client-owner-search-request";

pub const CANCELLATION_PROBE_METHOD: &str = "asp.lifecycle.cancellation";
pub const CANCELLATION_PROBE_REQUEST_SCHEMA_ID: &str =
    "agent.semantic-protocols.asp-client-cancellation-probe-request";
pub const CANCELLATION_PROBE_RESPONSE_SCHEMA_ID: &str =
    "agent.semantic-protocols.asp-client-cancellation-probe-response";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ServerClientRoute {
    CancellationProbe,
    Search,
    ExactQuery,
    OwnerSearch,
}

impl ServerClientRoute {
    pub const fn operation(self) -> &'static str {
        match self {
            Self::CancellationProbe => "lifecycle.cancellation",
            Self::Search => "search",
            Self::ExactQuery => "query",
            Self::OwnerSearch => "search.owner",
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
            exact_query_method(&language_id),
            owner_search_method(&language_id),
        ]);
    }
    methods.sort_by(|left, right| left.method.cmp(&right.method));
    Ok(methods)
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

pub fn server_client_catalog(
    catalog_generation: String,
    workspace_generation: String,
    transports: Vec<ClientTransport>,
    language_ids: impl IntoIterator<Item = String>,
) -> Result<ClientProtocolCatalog, String> {
    let mut methods = server_client_methods(language_ids)?;
    methods.push(cancellation_probe_method());
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
        resolved = Some((language_id, route));
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
        "query" => Some(ServerClientRoute::ExactQuery),
        "search.owner" => Some(ServerClientRoute::OwnerSearch),
        _ => None,
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
            required_string("operation"),
            optional("query", ClientParameterType::String),
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

fn owner_search_method(language_id: &str) -> ClientMethod {
    method(
        language_id,
        ServerClientRoute::OwnerSearch,
        OWNER_SEARCH_REQUEST_SCHEMA_ID,
        SEARCH_PACKET_SCHEMA_ID,
        vec![
            required_string("schemaId"),
            required_string("schemaVersion"),
            required("ownerPath", ClientParameterType::WorkspaceRelativePath),
            optional("query", ClientParameterType::String),
            optional("view", ClientParameterType::Presentation),
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
