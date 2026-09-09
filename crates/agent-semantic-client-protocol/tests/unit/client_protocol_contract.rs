// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use crate::protocol_identity::CLIENT_CATALOG_SCHEMA_ID;
use crate::protocol_identity::CLIENT_FRAME_SCHEMA_ID;
use crate::protocol_identity::CLIENT_PROTOCOL_ID;
use crate::protocol_identity::CLIENT_PROTOCOL_VERSION;
use crate::protocol_identity::SCHEMA_VERSION;
use serde_json::json;

#[test]
fn cancellation_probe_has_a_language_neutral_route_operation() {
    assert_eq!(
        crate::ServerClientRoute::CancellationProbe.operation(),
        "lifecycle.cancellation"
    );
    assert_eq!(
        crate::CANCELLATION_PROBE_METHOD,
        "asp.lifecycle.cancellation"
    );
}

#[test]
fn server_catalog_declares_each_shared_method_exactly_once() {
    let digest = format!("blake3-256:{}", "0".repeat(64));
    let catalog = crate::server_method_catalog::server_client_catalog(
        digest.clone(),
        digest,
        vec![crate::ClientTransport::RuntimeIpc],
        Vec::new(),
    )
    .expect("shared Runtime methods form a valid catalog");

    assert_eq!(
        catalog
            .methods
            .iter()
            .filter(|method| method.method == crate::CANCELLATION_PROBE_METHOD)
            .count(),
        1,
        "the shared cancellation route must have one catalog authority"
    );
}

#[test]
fn server_catalog_publishes_the_cancellation_probe() {
    let methods = crate::server_method_catalog::server_client_methods(Vec::new())
        .expect("server method catalog");
    let method = methods
        .iter()
        .find(|method| method.method == crate::server_method_catalog::CANCELLATION_PROBE_METHOD)
        .expect("cancellation probe method");
    assert_eq!(
        method.route_id.as_str(),
        crate::server_method_catalog::CANCELLATION_PROBE_METHOD
    );
    assert!(method.cancellable);
    assert!(!method.streaming);
}

#[test]
fn server_catalog_publishes_the_live_corpus_cache_state_authority() {
    let methods = crate::server_method_catalog::server_client_methods(Vec::new())
        .expect("server method catalog");
    let method = methods
        .iter()
        .find(|method| method.method == crate::LIVE_CORPUS_CACHE_STATE_METHOD)
        .expect("Live Corpus cache-state method");
    assert_eq!(
        method.route_id.as_str(),
        crate::LIVE_CORPUS_CACHE_STATE_METHOD
    );
    assert!(!method.cancellable);
    assert!(!method.streaming);
}

#[test]
fn live_corpus_cache_state_matrix_is_content_bound_and_fail_closed() {
    let warm = crate::LiveCorpusCacheStateRequest {
        schema_id: crate::LIVE_CORPUS_CACHE_STATE_REQUEST_SCHEMA_ID.to_owned(),
        schema_version: "1".to_owned(),
        operation_id: "warm-rust-tokio".to_owned(),
        resource_id: "rust.tokio".to_owned(),
        language_id: "rust".to_owned(),
        provider_id: "asp-rust".to_owned(),
        artifact_digest: "a".repeat(64),
        cache_state: "warm-read".to_owned(),
        prepare_action: "reuse-exact-resident-generation".to_owned(),
        mutation_scope: "none".to_owned(),
        expected_generation_digest: Some(format!("blake3-256:{}", "b".repeat(64))),
        expected_root_digest: Some("c".repeat(64)),
    };
    warm.validate().expect("exact warm cache identity");
    let mut stale = warm.clone();
    stale.expected_root_digest = None;
    assert!(stale.validate().is_err());
    let mut released = warm.clone();
    released.cache_state = "released".to_owned();
    released.prepare_action = "release-exact-benchmark-generation".to_owned();
    released.mutation_scope = "benchmark-workspace-generation".to_owned();
    released
        .validate()
        .expect("exact benchmark generation release");
    let mut global = warm;
    global.mutation_scope = "global".to_owned();
    assert!(global.validate().is_err());
}

#[test]
fn live_corpus_cache_receipt_is_bound_to_project_workspace_without_global_authority() {
    let receipt = crate::LiveCorpusCacheStateReceipt {
        schema_id: crate::LIVE_CORPUS_CACHE_STATE_RECEIPT_SCHEMA_ID.to_owned(),
        schema_version: "1".to_owned(),
        operation_id: "warm-rust-tokio".to_owned(),
        state: "ready".to_owned(),
        cache_state: "warm-read".to_owned(),
        project_id: "repo-project".to_owned(),
        workspace_id: "workspace-checkout".to_owned(),
        generation_digest: Some(format!("blake3-256:{}", "b".repeat(64))),
        root_digest: Some("c".repeat(64)),
        resident_generation_evicted: false,
        client_session_evicted: false,
        source_workspace_mutation_count: 0,
        filesystem_delete_count: 0,
        elapsed_micros: 17,
    };
    receipt.validate().expect("project/workspace-bound receipt");
    let value = serde_json::to_value(receipt).expect("serialize receipt");
    assert_eq!(value["projectId"], "repo-project");
    assert_eq!(value["workspaceId"], "workspace-checkout");
    assert!(value.get("workspaceIdentity").is_none());
    assert!(value.get("globalCacheMutationCount").is_none());
}

#[test]
fn northbound_client_requests_are_distinct_from_provider_runtime_requests() {
    let workspace_playbook = crate::AspClientWorkspaceSearchPlaybookRequest {
        schema_id: "agent.semantic-protocols.asp-client-workspace-search-playbook-request"
            .to_owned(),
        schema_version: "1".to_owned(),
        language: Some("rust".to_owned()),
        documents: None,
        workspace: None,
        rg: Some(vec![vec!["RuntimeAspClient".to_owned(), ".".to_owned()]]),
        tantivy: Some(vec![vec![
            "title:\"Runtime ASP client\"^2 OR body:transport".to_owned(),
        ]]),
        syntax: None,
        native_syntax: None,
        graph: None,
        clause_order: vec![
            crate::AspClientSearchPlaybookClauseRef {
                axis: crate::AspClientSearchPlaybookClauseAxis::Rg,
                block_index: 0,
            },
            crate::AspClientSearchPlaybookClauseRef {
                axis: crate::AspClientSearchPlaybookClauseAxis::Tantivy,
                block_index: 0,
            },
        ],
    };
    workspace_playbook
        .validate_schema_identity()
        .expect("workspace playbook identity");

    let query_playbook = crate::AspClientWorkspaceQueryPlaybookRequest {
        schema_id: "agent.semantic-protocols.asp-client-workspace-query-playbook-request"
            .to_owned(),
        schema_version: "1".to_owned(),
        language: Some("rust".to_owned()),
        documents: Some("org".to_owned()),
        selectors: vec![
            "org://docs/publication.org#item/heading/Publication".to_owned(),
            "rust://src/lib.rs#item/function/example".to_owned(),
        ],
        projection: "source".to_owned(),
    };
    query_playbook
        .validate_schema_identity()
        .expect("workspace Query Playbook identity");

    let syntax_query = crate::AspClientWorkspaceSyntaxQueryRequest {
        schema_id: "agent.semantic-protocols.asp-client-workspace-syntax-query-request".to_owned(),
        schema_version: "1".to_owned(),
        languages: None,
        documents: None,
        workspace: None,
        syntax: vec![crate::AspClientSearchPlaybookSyntaxBlock {
            producer: "rust".to_owned(),
            argv: vec![
                "--treesitter-query".to_owned(),
                "((identifier) @symbol)".to_owned(),
            ],
        }],
        projection: "matches".to_owned(),
    };
    syntax_query
        .validate_schema_identity()
        .expect("workspace syntax Query identity");

    let source_index = crate::AspClientSourceIndexLookupRequest {
        schema_id: "agent.semantic-protocols.asp-client-source-index-lookup-request".to_owned(),
        schema_version: "1".to_owned(),
        query: "RuntimeAspClient".to_owned(),
        index_root: "/workspace".to_owned(),
        limit: 8,
    };
    source_index
        .validate_schema_identity()
        .expect("source-index identity");

    let exact = crate::AspClientExactQueryRequest {
        schema_id: "agent.semantic-protocols.asp-client-exact-query-request".to_owned(),
        schema_version: "1".to_owned(),
        selector: "rust://src/lib.rs#item/function/example".to_owned(),
        projection: "callable-skeleton".to_owned(),
    };
    exact.validate_schema_identity().expect("query identity");

    assert_ne!(
        workspace_playbook.schema_id,
        "agent.semantic-protocols.runtime-provider-search-request"
    );
}

#[test]
fn workspace_playbook_clause_order_is_priority_and_graph_barrier() {
    use crate::{
        AspClientSearchPlaybookClauseAxis as Axis, AspClientSearchPlaybookClauseRef as Clause,
        AspClientSearchPlaybookGraphBlock,
    };

    let request = crate::AspClientWorkspaceSearchPlaybookRequest {
        schema_id: "agent.semantic-protocols.asp-client-workspace-search-playbook-request"
            .to_owned(),
        schema_version: "1".to_owned(),
        language: Some("rust".to_owned()),
        documents: None,
        workspace: None,
        rg: Some(vec![vec![
            "-e".to_owned(),
            "ClientFrame|Endpoint".to_owned(),
        ]]),
        tantivy: Some(vec![vec![
            "title:\"Client frame\"^2 OR body:Endpoint".to_owned(),
        ]]),
        syntax: None,
        native_syntax: None,
        graph: Some(vec![AspClientSearchPlaybookGraphBlock {
            language: "gql".to_owned(),
            argv: vec!["MATCH (a:Owner)-[:CALLS]->(b:Item) RETURN a, b".to_owned()],
        }]),
        clause_order: vec![
            Clause {
                axis: Axis::Rg,
                block_index: 0,
            },
            Clause {
                axis: Axis::Tantivy,
                block_index: 0,
            },
            Clause {
                axis: Axis::Graph,
                block_index: 0,
            },
        ],
    };
    request
        .validate_schema_identity()
        .expect("written acquisition priority followed by Graph is valid");

    let mut acquisition_after_graph = request.clone();
    acquisition_after_graph.clause_order = vec![
        Clause {
            axis: Axis::Graph,
            block_index: 0,
        },
        Clause {
            axis: Axis::Rg,
            block_index: 0,
        },
        Clause {
            axis: Axis::Tantivy,
            block_index: 0,
        },
    ];
    assert!(
        acquisition_after_graph
            .validate_schema_identity()
            .unwrap_err()
            .contains("precede Graph")
    );

    let mut missing_clause = request;
    missing_clause.clause_order.remove(1);
    assert!(
        missing_clause
            .validate_schema_identity()
            .unwrap_err()
            .contains("coverage is invalid")
    );
}

#[test]
fn graph_timeline_request_is_structured_and_rejects_unknown_fields() {
    let request = crate::AspClientGraphsTimelineRequest {
        schema_id: crate::GRAPH_TIMELINE_REQUEST_SCHEMA_ID.to_owned(),
        schema_version: SCHEMA_VERSION.to_owned(),
        event_packet: serde_json::json!({"schemaId": "agent.semantic-protocols.graph-turbo-artifact-events"}),
        arguments: vec!["--recent-sessions".to_owned()],
    };
    request
        .validate_schema_identity()
        .expect("timeline identity");
    let mut encoded = serde_json::to_value(&request).expect("encode timeline request");
    encoded["graphTurboResident"] = serde_json::json!(true);
    assert!(serde_json::from_value::<crate::AspClientGraphsTimelineRequest>(encoded).is_err());
}

#[test]
fn exact_query_response_carries_falsifiable_resident_performance() {
    let response = crate::AspClientExactQueryResponse {
        schema_id: "agent.semantic-protocols.asp-client-exact-query-response".to_owned(),
        schema_version: "1".to_owned(),
        operation_id: "query-1".to_owned(),
        project_id: "repo-project".to_owned(),
        workspace_id: "workspace-checkout".to_owned(),
        language_id: "rust".to_owned(),
        provider_id: "asp-rust".to_owned(),
        generation_digest: format!("blake3-256:{}", "a".repeat(64)),
        root_digest: "b".repeat(64),
        result: serde_json::json!({
            "state": "projection",
            "generationDigest": format!("blake3-256:{}", "a".repeat(64)),
            "rootDigest": "b".repeat(64),
            "resolvedSelector": "rust://example.rs#item/function/main",
            "bytes": [102, 110, 32, 109, 97, 105, 110]
        }),
        resident_read_elapsed_micros: 7,
        service_elapsed_micros: 3,
        elapsed_micros: 10,
        work_counters: crate::AspClientRuntimeWorkCounters::default(),
    };

    response.validate().expect("valid exact-query response");
    let mut derived_owner_root_alias = response.clone();
    derived_owner_root_alias.root_digest = format!("blake3-256:{}", "b".repeat(64));
    assert!(
        derived_owner_root_alias
            .validate()
            .expect_err("derived owner-index root must not replace source root")
            .contains("rootDigest is invalid")
    );
    let encoded = serde_json::to_value(response).expect("encode exact-query response");
    assert_eq!(encoded["residentReadElapsedMicros"], 7);
    assert_eq!(encoded["workCounters"]["filesystemReadCount"], 0);
}

#[test]
fn exact_query_response_rejects_empty_ready_and_failure_is_a_distinct_terminal() {
    let response = crate::AspClientExactQueryResponse {
        schema_id: "agent.semantic-protocols.asp-client-exact-query-response".to_owned(),
        schema_version: "1".to_owned(),
        operation_id: "query-terminal".to_owned(),
        project_id: "repo-project".to_owned(),
        workspace_id: "workspace-checkout".to_owned(),
        language_id: "rust".to_owned(),
        provider_id: "asp-rust".to_owned(),
        generation_digest: format!("blake3-256:{}", "a".repeat(64)),
        root_digest: "b".repeat(64),
        result: serde_json::json!({"state": "projection", "bytes": []}),
        resident_read_elapsed_micros: 7,
        service_elapsed_micros: 3,
        elapsed_micros: 10,
        work_counters: crate::AspClientRuntimeWorkCounters::default(),
    };
    assert!(
        response
            .validate()
            .unwrap_err()
            .contains("empty byte payload")
    );

    let failure = crate::AspClientExactQueryFailure {
        schema_id: "agent.semantic-protocols.asp-client-exact-query-failure".to_owned(),
        schema_version: "1".to_owned(),
        state: "failed".to_owned(),
        operation_id: "query-terminal".to_owned(),
        project_id: "repo-project".to_owned(),
        workspace_id: "workspace-checkout".to_owned(),
        language_id: "rust".to_owned(),
        provider_id: "asp-rust".to_owned(),
        requested_selector: Some("rust://src/lib.rs#item/function/missing".to_owned()),
        resolved_selector: None,
        projection_kind: Some("source".to_owned()),
        phase: "resident-selector-read".to_owned(),
        reason_kind: "projection-missing".to_owned(),
        generation_digest: Some(format!("blake3-256:{}", "a".repeat(64))),
        root_digest: Some("b".repeat(64)),
        resident_read_elapsed_micros: 7,
        service_elapsed_micros: 3,
        elapsed_micros: 10,
        work_counters: crate::AspClientRuntimeWorkCounters::default(),
        details: serde_json::json!({"selectorState": "projection-missing"}),
    };
    failure.validate().expect("typed exact-query failure");
}

#[test]
fn schema_bundle_terminals_keep_receipt_authority_out_of_language_clients() {
    let request = crate::SchemaBundleRequest {
        schema_id: crate::SCHEMA_BUNDLE_REQUEST_SCHEMA_ID.to_owned(),
        schema_version: SCHEMA_VERSION.to_owned(),
        language_id: "python".to_owned(),
        root_set_ids: vec!["client-protocol".to_owned()],
        known_bundle_digest: Some(format!("blake3-256:{}", "b".repeat(64))),
    };
    request.validate().expect("registered profile selector");

    let receipt = crate::SchemaBundleReceipt {
        language_id: "python".to_owned(),
        root_set_ids: vec!["client-protocol".to_owned()],
        bundle_digest: format!("blake3-256:{}", "b".repeat(64)),
    };
    let entry = crate::SchemaBundleEntry {
        family_id: "asp.schema-family.asp-client".to_owned(),
        schema_id: "https://schemas.agent-semantic-protocols.dev/asp-client-frame.schema.json"
            .to_owned(),
        schema_version: SCHEMA_VERSION.to_owned(),
        name: "asp-client-frame.schema.json".to_owned(),
        digest: format!("blake3-256:{}", "c".repeat(64)),
    };
    let ready = crate::SchemaBundleResponse::Ready {
        schema_id: crate::SCHEMA_BUNDLE_RESPONSE_SCHEMA_ID.to_owned(),
        schema_version: SCHEMA_VERSION.to_owned(),
        receipt: receipt.clone(),
        entries: vec![entry.clone()],
        documents: vec![crate::SchemaBundleDocument {
            entry: entry.clone(),
            document: serde_json::json!({"type": "object"}),
        }],
    };
    ready.validate().expect("ready schema bundle");
    let encoded = serde_json::to_value(&ready).expect("encode ready schema bundle");
    assert_eq!(encoded["receipt"]["rootSetIds"][0], "client-protocol");
    assert!(encoded["receipt"].get("profileId").is_none());
    assert_eq!(
        encoded["documents"][0]["entry"]["digest"],
        encoded["entries"][0]["digest"]
    );
    let decoded_ready: crate::SchemaBundleResponse =
        serde_json::from_value(encoded).expect("decode strict Ready schema bundle");
    assert_eq!(decoded_ready, ready);

    let unchanged = crate::SchemaBundleResponse::Unchanged {
        schema_id: crate::SCHEMA_BUNDLE_RESPONSE_SCHEMA_ID.to_owned(),
        schema_version: SCHEMA_VERSION.to_owned(),
        receipt,
        entries: vec![entry],
    };
    unchanged.validate().expect("unchanged schema bundle");

    let failed = crate::SchemaBundleResponse::Failed {
        schema_id: crate::SCHEMA_BUNDLE_RESPONSE_SCHEMA_ID.to_owned(),
        schema_version: SCHEMA_VERSION.to_owned(),
        language_id: "missing-language".to_owned(),
        reason_kind: "schema-profile-not-registered".to_owned(),
        recommended_next: serde_json::json!({"action": "inspect-schema-profile-registry"}),
        details: serde_json::json!({"registered": false}),
    };
    failed.validate().expect("failed schema bundle");
    let encoded_failed = serde_json::to_value(&failed).expect("encode Failed schema bundle");
    let decoded_failed: crate::SchemaBundleResponse =
        serde_json::from_value(encoded_failed).expect("decode strict Failed schema bundle");
    assert_eq!(decoded_failed, failed);

    let mut unknown = serde_json::to_value(&failed).expect("encode Failed schema bundle");
    unknown["unexpected"] = serde_json::json!(true);
    assert!(serde_json::from_value::<crate::SchemaBundleResponse>(unknown).is_err());
}

#[test]
fn provider_route_bindings_roundtrip_and_reject_identity_drift() {
    let request = crate::RuntimeProviderSearchRequest {
        schema_id: "agent.semantic-protocols.runtime-provider-search-request".to_owned(),
        schema_version: "1".to_owned(),
        operation_id: "op".to_owned(),
        project_id: "repo-project".to_owned(),
        workspace_id: "workspace".to_owned(),
        language_id: "rust".to_owned(),
        scope: "production".to_owned(),
        query_plan: json!({"method":"lexical","terms":["owner"],"view":"seeds"}),
    };
    let encoded = serde_json::to_vec(&request).expect("encode");
    let decoded: crate::RuntimeProviderSearchRequest =
        serde_json::from_slice(&encoded).expect("decode");
    assert!(decoded.validate_schema_identity().is_ok());
    let mut invalid = decoded;
    invalid.project_id.clear();
    assert!(invalid.validate_schema_identity().is_err());
}

use crate::ClientCapabilities;
use crate::ClientConformanceSuite;
use crate::ClientFrame;
use crate::ClientFrameBase;
use crate::ClientInfo;
use crate::ClientMethod;
use crate::ClientParameter;
use crate::ClientParameterCardinality;
use crate::ClientParameterSource;
use crate::ClientParameterType;
use crate::ClientProjectId;
use crate::ClientProtocolCatalog;
use crate::ClientRequestId;
use crate::ClientSession;
use crate::ClientSessionId;
use crate::ClientSessionState;
use crate::ClientTransport;
use crate::ClientWorkspaceIdentity;
use crate::run_conformance_suite;

fn digest(character: char) -> String {
    format!("blake3-256:{}", character.to_string().repeat(64))
}

fn request_id(value: &str) -> ClientRequestId {
    ClientRequestId::new(value).expect("request id")
}

fn catalog() -> ClientProtocolCatalog {
    ClientProtocolCatalog {
        schema_id: CLIENT_CATALOG_SCHEMA_ID.to_owned(),
        schema_version: SCHEMA_VERSION.to_owned(),
        protocol_id: CLIENT_PROTOCOL_ID.to_owned(),
        protocol_version: CLIENT_PROTOCOL_VERSION.to_owned(),
        catalog_generation: digest('a'),
        workspace_generation: digest('b'),
        transports: vec![ClientTransport::RuntimeIpc],
        capabilities: ClientCapabilities {
            request_cancellation: true,
            events: true,
            streaming: false,
            trace_context: true,
        },
        methods: vec![ClientMethod {
            method: "rust.query".to_owned(),
            route_id: crate::ClientRouteId::new("rust.query").expect("route id"),
            request_schema_id: crate::ClientSchemaId::new("request-schema")
                .expect("request schema id"),
            response_schema_id: crate::ClientSchemaId::new("response-schema")
                .expect("response schema id"),
            error_schema_ids: vec![
                crate::ClientSchemaId::new("error-schema").expect("error schema id"),
            ],
            parameters: vec![ClientParameter {
                name: "query".to_owned(),
                value_type: ClientParameterType::String,
                cardinality: ClientParameterCardinality::Required,
                source: ClientParameterSource::Request,
            }],
            cancellable: true,
            streaming: false,
        }],
    }
}

fn base() -> ClientFrameBase {
    ClientFrameBase {
        schema_id: CLIENT_FRAME_SCHEMA_ID.to_owned(),
        schema_version: SCHEMA_VERSION.to_owned(),
        protocol_id: CLIENT_PROTOCOL_ID.to_owned(),
        protocol_version: CLIENT_PROTOCOL_VERSION.to_owned(),
        session_id: ClientSessionId::new("session").expect("session id"),
        project_id: ClientProjectId::new("repo-project").expect("project id"),
        workspace_id: ClientWorkspaceIdentity::new("workspace-checkout").expect("workspace id"),
        trace_context: None,
    }
}

#[test]
fn catalog_is_closed_and_generation_pinned() {
    let catalog = catalog();
    catalog.validate().expect("valid catalog");
    let unknown = ClientFrame::Request {
        base: base(),
        request_id: request_id("request"),
        catalog_generation: catalog.catalog_generation.clone(),
        workspace_generation: catalog.workspace_generation.clone(),
        method: "python.query".to_owned(),
        params: json!({}),
        client_timing_witness: None,
    };
    let error = ClientSessionState::Initialized
        .admit(&unknown, &catalog)
        .expect_err("unknown method must fail");
    assert_eq!(error.reason_kind, "method-not-in-client-catalog");
}

#[test]
fn client_lifecycle_has_no_provider_control_transition() {
    let catalog = catalog();
    let initialize = ClientFrame::Initialize {
        base: base(),
        request_id: request_id("initialize"),
        client_info: ClientInfo {
            name: "test".to_owned(),
            version: "1".to_owned(),
        },
        capabilities: json!({}),
    };
    let state = ClientSessionState::Created
        .admit(&initialize, &catalog)
        .expect("initialize");
    let shutdown = ClientFrame::Shutdown {
        base: base(),
        request_id: request_id("shutdown"),
    };
    let state = state.admit(&shutdown, &catalog).expect("shutdown");
    let exit = ClientFrame::Exit { base: base() };
    assert_eq!(
        state.admit(&exit, &catalog).expect("exit"),
        ClientSessionState::Exited
    );
}

#[test]
fn cancellation_preserves_initialized_session() {
    let catalog = catalog();
    let cancel = ClientFrame::Cancel {
        base: base(),
        request_id: request_id("request"),
    };
    assert_eq!(
        ClientSessionState::Initialized
            .admit(&cancel, &catalog)
            .expect("cancel"),
        ClientSessionState::Initialized
    );
}

#[test]
fn request_parameters_are_executable_and_fail_closed() {
    let catalog = catalog();
    let request = ClientFrame::Request {
        base: base(),
        request_id: request_id("request"),
        catalog_generation: catalog.catalog_generation.clone(),
        workspace_generation: catalog.workspace_generation.clone(),
        method: "rust.query".to_owned(),
        params: json!({"query": 42}),
        client_timing_witness: None,
    };
    let error = ClientSessionState::Initialized
        .admit(&request, &catalog)
        .expect_err("wrong parameter type must fail");
    assert_eq!(error.reason_kind, "client-request-parameter-type-mismatch");
}

#[test]
fn catalog_parameter_identity_matches_provider_route_input_slots() {
    let mut catalog = catalog();
    catalog.methods[0].parameters[0].name = "ownerPath".to_owned();
    catalog.validate().expect("camelCase provider input slot");

    catalog.methods[0].parameters[0].name = "owner-path".to_owned();
    let error = catalog
        .validate()
        .expect_err("kebab name must drift closed");
    assert_eq!(error.reason_kind, "client-parameter-name-invalid");
}

#[test]
fn runtime_context_cannot_be_injected_by_a_client() {
    let mut catalog = catalog();
    catalog.methods[0].parameters.push(ClientParameter {
        name: "workspace".to_owned(),
        value_type: ClientParameterType::WorkspaceRelativePath,
        cardinality: ClientParameterCardinality::Required,
        source: ClientParameterSource::RuntimeContext,
    });
    let request = ClientFrame::Request {
        base: base(),
        request_id: request_id("request"),
        catalog_generation: catalog.catalog_generation.clone(),
        workspace_generation: catalog.workspace_generation.clone(),
        method: "rust.query".to_owned(),
        params: json!({"query": "owner", "workspace": "../other"}),
        client_timing_witness: None,
    };
    let error = ClientSessionState::Initialized
        .admit(&request, &catalog)
        .expect_err("RuntimeContext injection must fail");
    assert_eq!(error.reason_kind, "client-runtime-context-injection-denied");
}

#[test]
fn shared_conformance_fixture_runs_in_the_reference_implementation() {
    let suite: ClientConformanceSuite = serde_json::from_str(include_str!(
        "../../../../schemas/fixtures/asp-client-conformance/base.json"
    ))
    .expect("parse shared conformance suite");
    let receipts = run_conformance_suite(&suite, &catalog()).expect("conformance suite");
    assert_eq!(receipts.len(), 5);
    assert_eq!(
        receipts[2]
            .reason_kind
            .as_ref()
            .map(crate::ClientReasonKind::as_str),
        Some("method-not-in-client-catalog")
    );
    assert_eq!(
        receipts[3]
            .reason_kind
            .as_ref()
            .map(crate::ClientReasonKind::as_str),
        Some("client-catalog-generation-mismatch")
    );
}

#[test]
fn session_project_and_workspace_are_bound_at_initialize() {
    let catalog = catalog();
    let mut session = ClientSession::default();
    session
        .admit(
            &ClientFrame::Initialize {
                base: base(),
                request_id: request_id("initialize"),
                client_info: ClientInfo {
                    name: "test".to_owned(),
                    version: "1".to_owned(),
                },
                capabilities: json!({}),
            },
            &catalog,
        )
        .expect("initialize");
    let mut crossed = base();
    crossed.workspace_id = ClientWorkspaceIdentity::new("other-workspace").expect("workspace id");
    let error = session
        .admit(
            &ClientFrame::Request {
                base: crossed,
                request_id: request_id("request"),
                catalog_generation: catalog.catalog_generation.clone(),
                workspace_generation: catalog.workspace_generation.clone(),
                method: "rust.query".to_owned(),
                params: json!({"query": "owner"}),
                client_timing_witness: None,
            },
            &catalog,
        )
        .expect_err("workspace crossing must fail");
    assert_eq!(error.reason_kind, "client-session-isolation-mismatch");

    let mut crossed = base();
    crossed.project_id = ClientProjectId::new("repo-other-project").expect("project id");
    let error = session
        .admit(
            &ClientFrame::Request {
                base: crossed,
                request_id: request_id("request-project-crossing"),
                catalog_generation: catalog.catalog_generation.clone(),
                workspace_generation: catalog.workspace_generation.clone(),
                method: "rust.query".to_owned(),
                params: json!({"query": "owner"}),
                client_timing_witness: None,
            },
            &catalog,
        )
        .expect_err("project crossing must fail");
    assert_eq!(error.reason_kind, "client-session-isolation-mismatch");
}

#[test]
fn runtime_resident_request_plane_receipt_is_field_complete_and_strict() {
    let receipt = crate::RuntimeResidentRequestPlaneReceipt::ready(
        crate::RuntimeResidentRequestOperation::Query,
        crate::RuntimeResidentRequestTemperature::Cold,
        format!("blake3-256:{}", "a".repeat(64)),
        999,
    );
    receipt.validate().expect("strict request-plane receipt");
    let packet = serde_json::to_value(&receipt).expect("serialize request-plane receipt");
    assert_eq!(packet["generationLookupCount"], 1);
    assert_eq!(packet["secondaryRuntimeRpcCount"], 0);
    assert_eq!(packet["socketDiscoveryCount"], 0);
    assert_eq!(packet["terminalWaitCount"], 0);

    let mut boundary = receipt;
    boundary.elapsed_micros = 1_000;
    assert!(boundary.validate().is_err());
}

#[test]
fn query_not_ready_request_plane_receipt_cannot_claim_a_generation() {
    let mut receipt = crate::RuntimeResidentRequestPlaneReceipt::query_not_ready(
        crate::RuntimeResidentRequestOperation::Search,
        crate::RuntimeResidentRequestTemperature::Warm,
        7,
    );
    receipt.validate().expect("typed query-not-ready receipt");
    receipt.generation_digest = Some(format!("blake3-256:{}", "b".repeat(64)));
    assert!(receipt.validate().is_err());
}
