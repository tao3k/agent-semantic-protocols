// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use crate::ClientDispatchClass;
use crate::ResolvedServerClientMethod;
use crate::ServerClientRoute;
use crate::classify_client_dispatch;
use crate::resolve_server_client_method;
use crate::resolve_server_client_method_owner;
use crate::server_client_methods;

#[test]
fn server_catalog_exposes_workspace_search_exact_query_and_schema_bundle_methods() {
    let methods = server_client_methods(["rust".to_owned()]).expect("Rust method catalog");
    let names = methods
        .iter()
        .map(|method| method.method.as_str())
        .collect::<Vec<_>>();
    assert_eq!(
        names,
        [
            "asp.graph.evaluate",
            "asp.graphs.timeline",
            "asp.lifecycle.cancellation",
            "asp.live-corpus.cache-state",
            "asp.schema.bundle",
            "asp.session.children",
            "asp.session.host-event",
            "asp.session.register-child",
            "asp.workspace.generation.ensure-ready",
            "asp.workspace.query.playbook",
            "asp.workspace.query.syntax",
            "asp.workspace.search.playbook",
            "rust.query",
            "rust.source-index.lookup"
        ]
    );
    assert!(!names.contains(&"rust.projection-batch"));
    assert!(
        methods
            .iter()
            .filter(|method| method.method != "asp.graph.evaluate")
            .all(|method| {
                method
                    .request_schema_id
                    .as_str()
                    .starts_with("agent.semantic-protocols.")
            })
    );
}

#[test]
fn workspace_query_playbook_is_one_server_owned_complete_generation_request() {
    assert_eq!(
        resolve_server_client_method_owner(
            crate::WORKSPACE_QUERY_PLAYBOOK_METHOD,
            ["rust".to_owned(), "org".to_owned()],
        ),
        Ok(ResolvedServerClientMethod::Server(
            ServerClientRoute::WorkspaceQueryPlaybook,
        ))
    );
    assert_eq!(
        classify_client_dispatch(crate::WORKSPACE_QUERY_PLAYBOOK_METHOD),
        ClientDispatchClass::ResidentGenerationRead,
    );
    let method = server_client_methods(["rust".to_owned()])
        .expect("server method catalog")
        .into_iter()
        .find(|method| method.method == crate::WORKSPACE_QUERY_PLAYBOOK_METHOD)
        .expect("Query Playbook method");
    assert_eq!(
        method
            .parameters
            .iter()
            .map(|parameter| parameter.name.as_str())
            .collect::<Vec<_>>(),
        ["schemaId", "schemaVersion", "selectors", "projection"]
    );
}

#[test]
fn workspace_syntax_query_is_server_owned_and_complete_generation_scoped() {
    assert_eq!(
        resolve_server_client_method_owner(
            crate::WORKSPACE_SYNTAX_QUERY_METHOD,
            ["rust".to_owned(), "python".to_owned()],
        ),
        Ok(ResolvedServerClientMethod::Server(
            ServerClientRoute::WorkspaceSyntaxQuery,
        ))
    );
    assert_eq!(
        classify_client_dispatch(crate::WORKSPACE_SYNTAX_QUERY_METHOD),
        ClientDispatchClass::ResidentGenerationRead,
    );
}

#[test]
fn workspace_search_playbook_is_server_owned_and_complete_generation_scoped() {
    let languages = ["rust".to_owned(), "gerbil-scheme".to_owned()];
    assert_eq!(
        resolve_server_client_method_owner(
            crate::WORKSPACE_SEARCH_PLAYBOOK_METHOD,
            languages.clone(),
        ),
        Ok(ResolvedServerClientMethod::Server(
            ServerClientRoute::WorkspaceSearchPlaybook,
        ))
    );
    assert_eq!(
        classify_client_dispatch(crate::WORKSPACE_SEARCH_PLAYBOOK_METHOD),
        ClientDispatchClass::ResidentGenerationRead,
    );
    let method = server_client_methods(languages)
        .expect("method catalog")
        .into_iter()
        .find(|method| method.method == crate::WORKSPACE_SEARCH_PLAYBOOK_METHOD)
        .expect("Workspace Search Playbook method");
    assert_eq!(
        method.response_schema_id.as_str(),
        "agent.semantic-protocols.search-topology-settlement"
    );
    let parameters = method
        .parameters
        .iter()
        .map(|parameter| parameter.name.as_str())
        .collect::<Vec<_>>();
    assert!(parameters.contains(&"syntax"));
    assert!(parameters.contains(&"nativeSyntax"));
    assert!(parameters.contains(&"clauseOrder"));
}

#[test]
fn language_catalog_does_not_expose_a_legacy_search_route() {
    let methods = server_client_methods(["rust".to_owned()]).expect("Rust method catalog");
    assert!(methods.iter().all(|method| method.method != "rust.search"));
}

#[test]
fn workspace_generation_preflight_is_server_owned_and_language_targeted() {
    let languages = ["rust".to_owned(), "python".to_owned()];
    assert_eq!(
        resolve_server_client_method_owner(
            crate::WORKSPACE_GENERATION_ENSURE_READY_METHOD,
            languages,
        ),
        Ok(ResolvedServerClientMethod::Server(
            ServerClientRoute::WorkspaceGenerationEnsureReady,
        ))
    );
    let method = server_client_methods(["rust".to_owned()])
        .expect("server method catalog")
        .into_iter()
        .find(|method| method.method == crate::WORKSPACE_GENERATION_ENSURE_READY_METHOD)
        .expect("workspace generation ensure-ready method");
    assert_eq!(method.parameters.len(), 1);
    assert_eq!(method.parameters[0].name, "languageId");
    assert_eq!(
        method.parameters[0].cardinality,
        crate::ClientParameterCardinality::Optional
    );
    assert!(method.cancellable);
}

#[test]
fn dispatch_class_is_catalog_owned_and_preserved_by_transports() {
    assert_eq!(
        classify_client_dispatch(crate::WORKSPACE_GENERATION_ENSURE_READY_METHOD),
        ClientDispatchClass::ColdGenerationAdmission,
    );
    assert_eq!(
        classify_client_dispatch("rust.query"),
        ClientDispatchClass::ResidentGenerationRead,
    );
    assert_eq!(
        classify_client_dispatch("python.search"),
        ClientDispatchClass::InteractiveRead,
    );
    assert_eq!(
        classify_client_dispatch("asp.graph.evaluate"),
        ClientDispatchClass::ResidentGenerationRead,
    );
}

#[test]
fn multi_agent_v2_methods_are_server_owned_and_language_independent() {
    let languages = ["rust".to_owned(), "python".to_owned()];
    assert_eq!(
        resolve_server_client_method_owner(crate::MULTI_AGENT_HOST_EVENT_METHOD, languages.clone()),
        Ok(ResolvedServerClientMethod::Server(
            ServerClientRoute::MultiAgentHostEvent
        ))
    );
    assert_eq!(
        resolve_server_client_method_owner(crate::MULTI_AGENT_CHILDREN_METHOD, languages),
        Ok(ResolvedServerClientMethod::Server(
            ServerClientRoute::MultiAgentChildren
        ))
    );
}

#[test]
fn server_catalog_exposes_the_resident_runtime_graph_evaluation_method() {
    let methods = server_client_methods(["rust".to_owned()]).expect("Rust method catalog");
    let method = methods
        .iter()
        .find(|method| method.method == "asp.graph.evaluate")
        .expect("graph evaluation method");
    assert_eq!(method.route_id.as_str(), "asp.graph.evaluate");
    assert_eq!(
        method.request_schema_id.as_str(),
        "agent.semantic-protocols.semantic-graph-resident-evaluation-request"
    );
    assert_eq!(
        method.response_schema_id.as_str(),
        "agent.semantic-protocols.semantic-graph-resident-evaluation-result"
    );
    assert_eq!(
        method
            .parameters
            .iter()
            .map(|parameter| parameter.name.as_str())
            .collect::<Vec<_>>(),
        [
            "schemaId",
            "schemaVersion",
            "protocolId",
            "protocolVersion",
            "packetKind",
            "languageId",
            "surface",
            "queryTerms",
            "profile",
            "entryNodeIds",
            "budget"
        ]
    );
    assert_eq!(
        resolve_server_client_method_owner("asp.graph.evaluate", ["rust".to_owned()]),
        Ok(ResolvedServerClientMethod::Server(
            ServerClientRoute::GraphEvaluate
        ))
    );
    assert!(
        resolve_server_client_method_owner("asp.graphs.evaluate", ["rust".to_owned()]).is_err()
    );
    assert!(method.cancellable);
    assert!(!method.streaming);
}

#[test]
fn schema_bundle_method_is_transport_neutral_and_profile_selected() {
    let methods = server_client_methods(["rust".to_owned()]).expect("Rust method catalog");
    let method = methods
        .iter()
        .find(|method| method.method == crate::SCHEMA_BUNDLE_METHOD)
        .expect("schema bundle method");
    assert_eq!(method.route_id.as_str(), "asp.schema.bundle");
    assert_eq!(
        method.request_schema_id.as_str(),
        crate::SCHEMA_BUNDLE_REQUEST_SCHEMA_ID
    );
    assert_eq!(
        method.response_schema_id.as_str(),
        crate::SCHEMA_BUNDLE_RESPONSE_SCHEMA_ID
    );
    assert_eq!(
        method
            .parameters
            .iter()
            .map(|parameter| parameter.name.as_str())
            .collect::<Vec<_>>(),
        [
            "schemaId",
            "schemaVersion",
            "languageId",
            "rootSetIds",
            "knownBundleDigest"
        ]
    );
    assert!(!method.streaming);
    assert!(!method.cancellable);
}

#[test]
fn method_resolution_is_independent_of_provider_runtime_routes() {
    let languages = ["rust".to_owned(), "python".to_owned()];
    assert_eq!(
        resolve_server_client_method("python.query", languages),
        Ok(("python".to_owned(), ServerClientRoute::ExactQuery))
    );
    assert_eq!(
        resolve_server_client_method("rust.source-index.lookup", ["rust".to_owned()]),
        Ok(("rust".to_owned(), ServerClientRoute::SourceIndexLookup))
    );
}

#[test]
fn duplicate_language_identity_fails_closed() {
    let error = server_client_methods(["rust".to_owned(), "rust".to_owned()])
        .expect_err("duplicate language route ownership must be rejected");
    assert!(error.contains("multiple-installed-client-languages"));
}
