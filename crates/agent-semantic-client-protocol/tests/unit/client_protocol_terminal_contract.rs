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

    let entry = crate::SchemaBundleEntry {
        family_id: "asp.schema-family.asp-client".to_owned(),
        schema_id: "https://schemas.agent-semantic-protocols.dev/asp-client-frame.schema.json"
            .to_owned(),
        schema_version: SCHEMA_VERSION.to_owned(),
        name: "asp-client-frame.schema.json".to_owned(),
        digest: format!("blake3-256:{}", "c".repeat(64)),
    };
    let bundle_digest =
        crate::schema_bundle_digest(std::slice::from_ref(&entry)).expect("schema bundle identity");
    let receipt = crate::SchemaBundleReceipt {
        language_id: "python".to_owned(),
        root_set_ids: vec!["client-protocol".to_owned()],
        bundle_digest,
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

    let mut corrupted = unchanged.clone();
    let crate::SchemaBundleResponse::Unchanged { entries, .. } = &mut corrupted else {
        unreachable!("fixture is Unchanged")
    };
    entries[0].digest = format!("blake3-256:{}", "d".repeat(64));
    assert!(
        corrupted
            .validate()
            .unwrap_err()
            .contains("schema bundle receipt digest mismatch")
    );

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
use crate::ClientSessionId;
use crate::ClientSessionState;
use crate::ClientTransport;
use crate::ClientWorkspaceIdentity;

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
