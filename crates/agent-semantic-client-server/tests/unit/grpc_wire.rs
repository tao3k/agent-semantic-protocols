// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-only

use agent_semantic_client_protocol::ClientCapabilities;
use agent_semantic_client_protocol::ClientFrame;
use agent_semantic_client_protocol::ClientFrameBase;
use agent_semantic_client_protocol::ClientInfo;
use agent_semantic_client_protocol::ClientMethod;
use agent_semantic_client_protocol::ClientOutcome;
use agent_semantic_client_protocol::ClientParameter;
use agent_semantic_client_protocol::ClientParameterCardinality;
use agent_semantic_client_protocol::ClientParameterSource;
use agent_semantic_client_protocol::ClientParameterType;
use agent_semantic_client_protocol::ClientProjectId;
use agent_semantic_client_protocol::ClientProtocolCatalog;
use agent_semantic_client_protocol::ClientRequestId;
use agent_semantic_client_protocol::ClientSessionId;
use agent_semantic_client_protocol::ClientTransport;
use agent_semantic_client_protocol::ClientWorkspaceIdentity;
use agent_semantic_client_protocol::TraceContext;
use agent_semantic_client_protocol::protocol_identity::CLIENT_CATALOG_SCHEMA_ID;
use agent_semantic_client_protocol::protocol_identity::CLIENT_FRAME_SCHEMA_ID;
use agent_semantic_client_protocol::protocol_identity::CLIENT_PROTOCOL_ID;
use agent_semantic_client_protocol::protocol_identity::CLIENT_PROTOCOL_VERSION;
use agent_semantic_client_protocol::protocol_identity::SCHEMA_VERSION;
use serde_json::json;

use super::decode_frame;
use super::encode_frame;

fn base() -> ClientFrameBase {
    ClientFrameBase {
        schema_id: CLIENT_FRAME_SCHEMA_ID.to_owned(),
        schema_version: SCHEMA_VERSION.to_owned(),
        protocol_id: CLIENT_PROTOCOL_ID.to_owned(),
        protocol_version: CLIENT_PROTOCOL_VERSION.to_owned(),
        session_id: ClientSessionId::new("wire-session").expect("session id"),
        project_id: ClientProjectId::new("repo-wire-project").expect("project id"),
        workspace_id: ClientWorkspaceIdentity::new("workspace-wire-workspace")
            .expect("workspace identity"),
        trace_context: None,
    }
}

fn catalog() -> ClientProtocolCatalog {
    let parameter_types = [
        ClientParameterType::String,
        ClientParameterType::StringArray,
        ClientParameterType::WorkspaceRelativePath,
        ClientParameterType::StructuralSelector,
        ClientParameterType::Presentation,
        ClientParameterType::Boolean,
        ClientParameterType::UnsignedInteger,
        ClientParameterType::Json,
    ];
    ClientProtocolCatalog {
        schema_id: CLIENT_CATALOG_SCHEMA_ID.to_owned(),
        schema_version: SCHEMA_VERSION.to_owned(),
        protocol_id: CLIENT_PROTOCOL_ID.to_owned(),
        protocol_version: CLIENT_PROTOCOL_VERSION.to_owned(),
        catalog_generation: "catalog-generation".to_owned(),
        workspace_generation: "workspace-generation".to_owned(),
        transports: vec![ClientTransport::RuntimeIpc],
        capabilities: ClientCapabilities {
            request_cancellation: true,
            events: true,
            streaming: true,
            trace_context: true,
        },
        methods: vec![ClientMethod {
            method: "rust.search".to_owned(),
            route_id: agent_semantic_client_protocol::ClientRouteId::new("rust.search")
                .expect("route id"),
            request_schema_id: agent_semantic_client_protocol::ClientSchemaId::new(
                "request-schema",
            )
            .expect("request schema id"),
            response_schema_id: agent_semantic_client_protocol::ClientSchemaId::new(
                "response-schema",
            )
            .expect("response schema id"),
            error_schema_ids: vec![
                agent_semantic_client_protocol::ClientSchemaId::new("error-schema")
                    .expect("error schema id"),
            ],
            parameters: parameter_types
                .into_iter()
                .enumerate()
                .map(|(index, value_type)| ClientParameter {
                    name: format!("parameter-{index}"),
                    value_type,
                    cardinality: match index % 3 {
                        0 => ClientParameterCardinality::Required,
                        1 => ClientParameterCardinality::Optional,
                        _ => ClientParameterCardinality::Many,
                    },
                    source: if index % 2 == 0 {
                        ClientParameterSource::Request
                    } else {
                        ClientParameterSource::RuntimeContext
                    },
                })
                .collect(),
            cancellable: true,
            streaming: true,
        }],
    }
}

fn request_id(value: &str) -> ClientRequestId {
    ClientRequestId::new(value).expect("request id")
}

#[test]
fn canonical_protobuf_round_trips_every_client_frame_variant() {
    let info = ClientInfo {
        name: "wire-test".to_owned(),
        version: "1".to_owned(),
    };
    let frames = vec![
        ClientFrame::Initialize {
            base: base(),
            request_id: request_id("initialize"),
            client_info: info.clone(),
            capabilities: json!({"streaming": true}),
        },
        ClientFrame::Request {
            base: base(),
            request_id: request_id("request"),
            catalog_generation: "catalog-generation".to_owned(),
            workspace_generation: "workspace-generation".to_owned(),
            method: "rust.query".to_owned(),
            params: json!({"selector": "rust://crate#item/function/example"}),
        },
        ClientFrame::Cancel {
            base: base(),
            request_id: request_id("cancel"),
        },
        ClientFrame::Shutdown {
            base: base(),
            request_id: request_id("shutdown"),
        },
        ClientFrame::Exit { base: base() },
        ClientFrame::Response {
            base: ClientFrameBase {
                trace_context: Some(TraceContext {
                    traceparent: "00-00000000000000000000000000000001-0000000000000001-01"
                        .to_owned(),
                    tracestate: Some("asp=wire".to_owned()),
                }),
                ..base()
            },
            request_id: request_id("response"),
            outcome: ClientOutcome::Ready,
            result: Some(json!({"state": "ready"})),
            error: None,
            catalog: Some(catalog()),
        },
        ClientFrame::Event {
            base: base(),
            event_id: "event-1".to_owned(),
            event: "workspace-generation-published".to_owned(),
            payload: json!({"generation": "workspace-generation"}),
        },
    ];

    for frame in frames {
        let decoded =
            decode_frame(encode_frame(frame.clone()).expect("encode frame")).expect("decode frame");
        assert_eq!(decoded, frame);
    }
}

#[test]
fn canonical_protobuf_rejects_missing_identity_and_discriminant() {
    let frame = ClientFrame::Exit { base: base() };
    let mut missing_base = encode_frame(frame.clone()).expect("encode frame");
    missing_base.base = None;
    assert_eq!(
        decode_frame(missing_base).expect_err("missing base must fail"),
        "protobuf ClientFrame base must be present"
    );

    let mut missing_frame = encode_frame(frame).expect("encode frame");
    missing_frame.frame = None;
    assert_eq!(
        decode_frame(missing_frame).expect_err("missing discriminant must fail"),
        "protobuf ClientFrame frame must be present"
    );
}

#[test]
fn canonical_protobuf_rejects_invalid_dynamic_json_and_unknown_outcome() {
    let request = ClientFrame::Request {
        base: base(),
        request_id: request_id("request-invalid-json"),
        catalog_generation: "catalog-generation".to_owned(),
        workspace_generation: "workspace-generation".to_owned(),
        method: "rust.search".to_owned(),
        params: json!({"query": "valid-before-wire-corruption"}),
    };
    let mut invalid_json = encode_frame(request).expect("encode request");
    let Some(super::wire::client_frame_envelope::Frame::Request(request)) =
        invalid_json.frame.as_mut()
    else {
        panic!("expected request wire frame");
    };
    request.params_json = b"{".to_vec();
    assert!(
        decode_frame(invalid_json)
            .expect_err("invalid dynamic JSON must fail")
            .starts_with("decode protobuf ClientFrame params:")
    );

    let response = ClientFrame::Response {
        base: base(),
        request_id: request_id("response-invalid-outcome"),
        outcome: ClientOutcome::Ready,
        result: None,
        error: None,
        catalog: None,
    };
    let mut invalid_outcome = encode_frame(response).expect("encode response");
    let Some(super::wire::client_frame_envelope::Frame::Response(response)) =
        invalid_outcome.frame.as_mut()
    else {
        panic!("expected response wire frame");
    };
    response.outcome = 0;
    assert_eq!(
        decode_frame(invalid_outcome).expect_err("unspecified outcome must fail"),
        "unsupported client outcome on protobuf wire"
    );
}
