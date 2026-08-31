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
fn northbound_client_requests_are_distinct_from_provider_runtime_requests() {
    let search = crate::AspClientSearchRequest {
        schema_id: "agent.semantic-protocols.asp-client-search-request".to_owned(),
        schema_version: "1".to_owned(),
        operation: "pipe".to_owned(),
        query: "RuntimeAspClient".to_owned(),
    };
    search.validate_schema_identity().expect("search identity");

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

    let owner = crate::AspClientOwnerSearchRequest {
        schema_id: "agent.semantic-protocols.asp-client-owner-search-request".to_owned(),
        schema_version: "1".to_owned(),
        owner_path: "src/lib.rs".to_owned(),
        query: "example".to_owned(),
        view: "seeds".to_owned(),
    };
    owner.validate_schema_identity().expect("owner identity");

    let owner_response = crate::AspClientOwnerSearchResponse {
        schema_id: "agent.semantic-protocols.asp-client-owner-search-response".to_owned(),
        schema_version: "1".to_owned(),
        state: "owner".to_owned(),
        generation_digest: "generation-1".to_owned(),
        root_digest: "root-1".to_owned(),
        owner_path: "src/lib.rs".to_owned(),
        content_digest: Some("blake3-256:owner".to_owned()),
        query: "example".to_owned(),
        view: "seeds".to_owned(),
        candidate_count: 1,
        returned_count: 1,
        selectors: vec![crate::AspClientOwnerSearchSeed {
            selector: "rust://src/lib.rs#item/function/example".to_owned(),
            byte_start: 0,
            byte_end: 12,
        }],
    };
    owner_response.validate().expect("owner response identity");

    assert_ne!(
        search.schema_id,
        "agent.semantic-protocols.runtime-provider-search-request"
    );
}

#[test]
fn graph_timeline_request_is_structured_and_rejects_unknown_fields() {
    let request = crate::AspClientGraphsTimelineRequest {
        schema_id: crate::GRAPH_TIMELINE_REQUEST_SCHEMA_ID.to_owned(),
        schema_version: crate::SCHEMA_VERSION.to_owned(),
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
        language_id: "rust".to_owned(),
        provider_id: "asp-rust".to_owned(),
        requested_selector: Some("rust://src/lib.rs#item/function/missing".to_owned()),
        resolved_selector: None,
        projection_kind: Some("source".to_owned()),
        phase: "resident-selector-read".to_owned(),
        reason_kind: "projection-missing".to_owned(),
        generation_digest: Some(format!("blake3-256:{}", "a".repeat(64))),
        root_digest: Some("b".repeat(64)),
        recommended_next: serde_json::json!({"action": "query-owner-or-admitted-scope"}),
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
        schema_version: crate::SCHEMA_VERSION.to_owned(),
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
        schema_version: crate::SCHEMA_VERSION.to_owned(),
        name: "asp-client-frame.schema.json".to_owned(),
        digest: format!("blake3-256:{}", "c".repeat(64)),
    };
    let ready = crate::SchemaBundleResponse::Ready {
        schema_id: crate::SCHEMA_BUNDLE_RESPONSE_SCHEMA_ID.to_owned(),
        schema_version: crate::SCHEMA_VERSION.to_owned(),
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
        schema_version: crate::SCHEMA_VERSION.to_owned(),
        receipt,
        entries: vec![entry],
    };
    unchanged.validate().expect("unchanged schema bundle");

    let failed = crate::SchemaBundleResponse::Failed {
        schema_id: crate::SCHEMA_BUNDLE_RESPONSE_SCHEMA_ID.to_owned(),
        schema_version: crate::SCHEMA_VERSION.to_owned(),
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
        workspace_identity: "workspace".to_owned(),
        language_id: "rust".to_owned(),
        scope: "production".to_owned(),
        query_plan: json!({"method":"lexical","terms":["owner"],"view":"seeds"}),
    };
    let encoded = serde_json::to_vec(&request).expect("encode");
    let decoded: crate::RuntimeProviderSearchRequest =
        serde_json::from_slice(&encoded).expect("decode");
    assert!(decoded.validate_schema_identity().is_ok());
    let mut invalid = decoded;
    invalid.schema_id.push_str(".v1");
    assert!(invalid.validate_schema_identity().is_err());
}

use super::*;

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
            method: "rust.search".to_owned(),
            route_id: "rust.search".to_owned(),
            request_schema_id: "request-schema".to_owned(),
            response_schema_id: "response-schema".to_owned(),
            error_schema_ids: vec!["error-schema".to_owned()],
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
        workspace_identity: ClientWorkspaceIdentity::new("workspace").expect("workspace id"),
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
        project_root: "/workspace".to_owned(),
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
        method: "rust.search".to_owned(),
        params: json!({"query": 42}),
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
        method: "rust.search".to_owned(),
        params: json!({"query": "owner", "workspace": "../other"}),
    };
    let error = ClientSessionState::Initialized
        .admit(&request, &catalog)
        .expect_err("RuntimeContext injection must fail");
    assert_eq!(error.reason_kind, "client-runtime-context-injection-denied");
}

#[test]
fn shared_conformance_fixture_runs_in_the_reference_implementation() {
    let suite: ClientConformanceSuite = serde_json::from_str(include_str!(
        "../../../schemas/fixtures/asp-client-conformance/base.json"
    ))
    .expect("parse shared conformance suite");
    let receipts = run_conformance_suite(&suite, &catalog()).expect("conformance suite");
    assert_eq!(receipts.len(), 5);
    assert_eq!(
        receipts[2].reason_kind.as_deref(),
        Some("method-not-in-client-catalog")
    );
    assert_eq!(
        receipts[3].reason_kind.as_deref(),
        Some("client-catalog-generation-mismatch")
    );
}

#[test]
fn session_and_workspace_identity_are_bound_at_initialize() {
    let catalog = catalog();
    let mut session = ClientSession::default();
    session
        .admit(
            &ClientFrame::Initialize {
                base: base(),
                request_id: request_id("initialize"),
                project_root: "/workspace".to_owned(),
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
    crossed.workspace_identity =
        ClientWorkspaceIdentity::new("other-workspace").expect("workspace id");
    let error = session
        .admit(
            &ClientFrame::Request {
                base: crossed,
                request_id: request_id("request"),
                catalog_generation: catalog.catalog_generation.clone(),
                workspace_generation: catalog.workspace_generation.clone(),
                method: "rust.search".to_owned(),
                params: json!({"query": "owner"}),
            },
            &catalog,
        )
        .expect_err("workspace crossing must fail");
    assert_eq!(error.reason_kind, "client-session-isolation-mismatch");
}
