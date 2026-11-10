// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Client session isolation, conformance, and resident-plane receipt contracts.

use agent_semantic_client_protocol::{
    ClientCapabilities, ClientConformanceSuite, ClientFrame, ClientFrameBase, ClientInfo,
    ClientMethod, ClientParameter, ClientParameterCardinality, ClientParameterSource,
    ClientParameterType, ClientProjectId, ClientProtocolCatalog, ClientRequestId, ClientSession,
    ClientSessionId, ClientSessionState, ClientTransport, ClientWorkspaceIdentity,
    run_conformance_suite,
};
use serde_json::json;

fn digest(character: char) -> String {
    format!("blake3-256:{}", character.to_string().repeat(64))
}

fn request_id(value: &str) -> ClientRequestId {
    ClientRequestId::new(value).expect("request id")
}

fn catalog() -> ClientProtocolCatalog {
    ClientProtocolCatalog {
        schema_id: agent_semantic_client_protocol::CLIENT_CATALOG_SCHEMA_ID.to_owned(),
        schema_version: agent_semantic_client_protocol::SCHEMA_VERSION.to_owned(),
        protocol_id: agent_semantic_client_protocol::CLIENT_PROTOCOL_ID.to_owned(),
        protocol_version: agent_semantic_client_protocol::CLIENT_PROTOCOL_VERSION.to_owned(),
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
            route_id: agent_semantic_client_protocol::ClientRouteId::new("rust.query")
                .expect("route id"),
            request_schema_id: agent_semantic_client_protocol::ClientSchemaId::new(
                "request-schema",
            )
            .expect("request schema id"),
            response_schema_id: agent_semantic_client_protocol::ClientSchemaId::new(
                "response-schema",
            )
            .expect("response schema id"),
            error_schema_ids: vec![agent_semantic_client_protocol::ClientSchemaId::new(
                "error-schema",
            )
            .expect("error schema id")],
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
        schema_id: agent_semantic_client_protocol::CLIENT_FRAME_SCHEMA_ID.to_owned(),
        schema_version: agent_semantic_client_protocol::SCHEMA_VERSION.to_owned(),
        protocol_id: agent_semantic_client_protocol::CLIENT_PROTOCOL_ID.to_owned(),
        protocol_version: agent_semantic_client_protocol::CLIENT_PROTOCOL_VERSION.to_owned(),
        session_id: ClientSessionId::new("session").expect("session id"),
        project_id: ClientProjectId::new("repo-project").expect("project id"),
        workspace_id: ClientWorkspaceIdentity::new("workspace-checkout").expect("workspace id"),
        trace_context: None,
    }
}

#[test]
fn catalog_parameter_identity_matches_provider_route_input_slots() {
    let mut catalog = catalog();
    catalog.methods[0].parameters[0].name = "ownerPath".to_owned();
    catalog.validate().expect("camelCase provider input slot");
    catalog.methods[0].parameters[0].name = "owner-path".to_owned();
    assert_eq!(
        catalog.validate().expect_err("kebab name must fail").reason_kind,
        "client-parameter-name-invalid"
    );
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
    assert_eq!(
        ClientSessionState::Initialized
            .admit(&request, &catalog)
            .expect_err("RuntimeContext injection must fail")
            .reason_kind,
        "client-runtime-context-injection-denied"
    );
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
            .map(agent_semantic_client_protocol::ClientReasonKind::as_str),
        Some("method-not-in-client-catalog")
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
    for (request_id_value, crossed) in [
        ("workspace", {
            let mut value = base();
            value.workspace_id =
                ClientWorkspaceIdentity::new("other-workspace").expect("workspace id");
            value
        }),
        ("project", {
            let mut value = base();
            value.project_id = ClientProjectId::new("repo-other-project").expect("project id");
            value
        }),
    ] {
        let error = session
            .admit(
                &ClientFrame::Request {
                    base: crossed,
                    request_id: request_id(request_id_value),
                    catalog_generation: catalog.catalog_generation.clone(),
                    workspace_generation: catalog.workspace_generation.clone(),
                    method: "rust.query".to_owned(),
                    params: json!({"query": "owner"}),
                    client_timing_witness: None,
                },
                &catalog,
            )
            .expect_err("session identity crossing must fail");
        assert_eq!(error.reason_kind, "client-session-isolation-mismatch");
    }
}

#[test]
fn runtime_resident_request_plane_receipts_are_fail_closed() {
    let receipt = agent_semantic_client_protocol::RuntimeResidentRequestPlaneReceipt::ready(
        agent_semantic_client_protocol::RuntimeResidentRequestOperation::Query,
        agent_semantic_client_protocol::RuntimeResidentRequestTemperature::Cold,
        digest('a'),
        999,
    );
    receipt.validate().expect("strict request-plane receipt");
    let mut boundary = receipt;
    boundary.elapsed_micros = 1_000;
    assert!(boundary.validate().is_err());

    let mut not_ready =
        agent_semantic_client_protocol::RuntimeResidentRequestPlaneReceipt::query_not_ready(
            agent_semantic_client_protocol::RuntimeResidentRequestOperation::Search,
            agent_semantic_client_protocol::RuntimeResidentRequestTemperature::Warm,
            7,
        );
    not_ready.validate().expect("typed not-ready receipt");
    not_ready.generation_digest = Some(digest('b'));
    assert!(not_ready.validate().is_err());
}
