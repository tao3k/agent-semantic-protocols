use agent_semantic_client_protocol::{
    CLIENT_FRAME_SCHEMA_ID, CLIENT_PROTOCOL_ID, CLIENT_PROTOCOL_VERSION, ClientFrame,
    ClientFrameBase, ClientInfo, ClientOutcome, ClientRequestId, ClientSessionId,
    ClientWorkspaceIdentity, SCHEMA_VERSION, WORKSPACE_GENERATION_ENSURE_READY_METHOD,
};

use super::{
    CLIENT_FRAME_PARTITION_BYTES, CLIENT_FRAME_RESPONSE_BUDGET, ResponsePartitionAssembly,
    encode_response_partitions, response_budget_for_frame,
};

fn dispatch(method: &str) -> ClientFrame {
    ClientFrame::Dispatch {
        base: ClientFrameBase {
            schema_id: CLIENT_FRAME_SCHEMA_ID.to_owned(),
            schema_version: SCHEMA_VERSION.to_owned(),
            protocol_id: CLIENT_PROTOCOL_ID.to_owned(),
            protocol_version: CLIENT_PROTOCOL_VERSION.to_owned(),
            session_id: ClientSessionId::new("transport-budget-session").expect("session id"),
            workspace_identity: ClientWorkspaceIdentity::new("transport-budget-workspace")
                .expect("workspace identity"),
            trace_context: None,
        },
        request_id: ClientRequestId::new(format!("transport-budget-{method}")).expect("request id"),
        project_root: "/workspace".to_owned(),
        client_info: ClientInfo {
            name: "transport-budget-test".to_owned(),
            version: "1".to_owned(),
        },
        method: method.to_owned(),
        params: serde_json::json!({}),
    }
}

fn large_response() -> ClientFrame {
    ClientFrame::Response {
        base: match dispatch("rust.query") {
            ClientFrame::Dispatch { base, .. } => base,
            _ => unreachable!(),
        },
        request_id: ClientRequestId::new("partitioned-response").expect("request id"),
        outcome: ClientOutcome::Ready,
        result: Some(serde_json::json!({
            "source": "x".repeat(CLIENT_FRAME_PARTITION_BYTES + 4096)
        })),
        error: None,
        catalog: None,
    }
}

#[test]
fn grpc_transport_preserves_catalog_dispatch_class() {
    assert_eq!(
        response_budget_for_frame(&dispatch(WORKSPACE_GENERATION_ENSURE_READY_METHOD)),
        None,
    );
    assert_eq!(
        response_budget_for_frame(&dispatch("rust.query")),
        Some(CLIENT_FRAME_RESPONSE_BUDGET),
    );
}

#[test]
fn large_response_is_content_bound_and_reassembled_from_bounded_partitions() {
    let envelopes = encode_response_partitions(large_response()).expect("partition response");
    assert!(envelopes.len() > 1);

    let mut assembly = None;
    let mut terminal = None;
    for envelope in envelopes {
        let partition = match envelope.frame.expect("partition frame") {
            super::super::generated::client_frame_envelope::Frame::ResponsePartition(partition) => {
                partition
            }
            _ => panic!("large response must use response partitions"),
        };
        assert!(partition.encoded_response.len() <= CLIENT_FRAME_PARTITION_BYTES);
        let current = assembly.get_or_insert_with(|| {
            ResponsePartitionAssembly::new(&partition).expect("first partition")
        });
        terminal = current.push(partition).expect("valid partition");
    }

    match terminal.expect("final partition completes response") {
        ClientFrame::Response {
            outcome,
            result: Some(result),
            ..
        } => {
            assert_eq!(outcome, ClientOutcome::Ready);
            assert!(
                result["source"].as_str().expect("source").len() > CLIENT_FRAME_PARTITION_BYTES
            );
        }
        _ => panic!("partitioned response must decode to the original terminal"),
    }
}

#[test]
fn response_partition_digest_mismatch_fails_closed() {
    let mut envelopes = encode_response_partitions(large_response()).expect("partition response");
    let first = envelopes.remove(0);
    let mut partition = match first.frame.expect("partition frame") {
        super::super::generated::client_frame_envelope::Frame::ResponsePartition(partition) => {
            partition
        }
        _ => panic!("large response must use response partitions"),
    };
    let mut assembly = ResponsePartitionAssembly::new(&partition).expect("first partition");
    assert!(assembly.push(partition).expect("first partition").is_none());

    let mut last = envelopes.pop().expect("last partition");
    for mut envelope in envelopes {
        let partition = match envelope.frame.take().expect("partition frame") {
            super::super::generated::client_frame_envelope::Frame::ResponsePartition(partition) => {
                partition
            }
            _ => panic!("large response must use response partitions"),
        };
        assert!(
            assembly
                .push(partition)
                .expect("middle partition")
                .is_none()
        );
    }
    partition = match last.frame.take().expect("partition frame") {
        super::super::generated::client_frame_envelope::Frame::ResponsePartition(partition) => {
            partition
        }
        _ => panic!("large response must use response partitions"),
    };
    partition.encoded_response[0] ^= 1;
    assert!(assembly.push(partition).is_err());
}
