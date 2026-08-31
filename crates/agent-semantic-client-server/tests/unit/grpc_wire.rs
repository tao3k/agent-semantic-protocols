use agent_semantic_client_protocol::{
    CLIENT_CATALOG_SCHEMA_ID, CLIENT_FRAME_SCHEMA_ID, CLIENT_PROTOCOL_ID, CLIENT_PROTOCOL_VERSION,
    ClientCapabilities, ClientFrame, ClientFrameBase, ClientInfo, ClientMethod, ClientOutcome,
    ClientParameter, ClientParameterCardinality, ClientParameterSource, ClientParameterType,
    ClientProtocolCatalog, ClientRequestId, ClientSessionId, ClientTransport,
    ClientWorkspaceIdentity, SCHEMA_VERSION, TraceContext,
};
use serde_json::json;

use super::{decode_frame, encode_frame};

fn base() -> ClientFrameBase {
    ClientFrameBase {
        schema_id: CLIENT_FRAME_SCHEMA_ID.to_owned(),
        schema_version: SCHEMA_VERSION.to_owned(),
        protocol_id: CLIENT_PROTOCOL_ID.to_owned(),
        protocol_version: CLIENT_PROTOCOL_VERSION.to_owned(),
        session_id: ClientSessionId::new("wire-session").expect("session id"),
        workspace_identity: ClientWorkspaceIdentity::new("wire-workspace")
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
            route_id: "rust.search".to_owned(),
            request_schema_id: "request-schema".to_owned(),
            response_schema_id: "response-schema".to_owned(),
            error_schema_ids: vec!["error-schema".to_owned()],
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
            project_root: "/workspace".to_owned(),
            client_info: info.clone(),
            capabilities: json!({"streaming": true}),
        },
        ClientFrame::Dispatch {
            base: base(),
            request_id: request_id("dispatch"),
            project_root: "/workspace".to_owned(),
            client_info: info,
            method: "rust.search".to_owned(),
            params: json!({"query": "WorkspaceGenerationAdmission"}),
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
    let dispatch = ClientFrame::Dispatch {
        base: base(),
        request_id: request_id("dispatch-invalid-json"),
        project_root: "/workspace".to_owned(),
        client_info: ClientInfo {
            name: "wire-test".to_owned(),
            version: "1".to_owned(),
        },
        method: "rust.search".to_owned(),
        params: json!({"query": "valid-before-wire-corruption"}),
    };
    let mut invalid_json = encode_frame(dispatch).expect("encode dispatch");
    let Some(super::wire::client_frame_envelope::Frame::Dispatch(dispatch)) =
        invalid_json.frame.as_mut()
    else {
        panic!("expected dispatch wire frame");
    };
    dispatch.params_json = b"{".to_vec();
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
