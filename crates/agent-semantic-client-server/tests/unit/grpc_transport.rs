// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use agent_semantic_client_protocol::ClientFrame;
use agent_semantic_client_protocol::ClientFrameBase;
use agent_semantic_client_protocol::ClientOutcome;
use agent_semantic_client_protocol::ClientProjectId;
use agent_semantic_client_protocol::ClientRequestId;
use agent_semantic_client_protocol::ClientSessionId;
use agent_semantic_client_protocol::ClientWorkspaceIdentity;
use agent_semantic_client_protocol::WORKSPACE_GENERATION_ENSURE_READY_METHOD;
use agent_semantic_client_protocol::protocol_identity::CLIENT_FRAME_SCHEMA_ID;
use agent_semantic_client_protocol::protocol_identity::CLIENT_PROTOCOL_ID;
use agent_semantic_client_protocol::protocol_identity::CLIENT_PROTOCOL_VERSION;
use agent_semantic_client_protocol::protocol_identity::SCHEMA_VERSION;

use super::CLIENT_FRAME_PARTITION_BYTES;
use super::CLIENT_FRAME_RESPONSE_BUDGET;
use super::ResponsePartitionAssembly;
use super::encode_response_partitions;
use super::response_budget_for_frame;

fn request(method: &str) -> ClientFrame {
    ClientFrame::Request {
        base: ClientFrameBase {
            schema_id: CLIENT_FRAME_SCHEMA_ID.to_owned(),
            schema_version: SCHEMA_VERSION.to_owned(),
            protocol_id: CLIENT_PROTOCOL_ID.to_owned(),
            protocol_version: CLIENT_PROTOCOL_VERSION.to_owned(),
            session_id: ClientSessionId::new("transport-budget-session").expect("session id"),
            project_id: ClientProjectId::new("repo-transport-budget").expect("project id"),
            workspace_id: ClientWorkspaceIdentity::new("workspace-transport-budget")
                .expect("workspace identity"),
            trace_context: None,
        },
        request_id: ClientRequestId::new(format!("transport-budget-{method}")).expect("request id"),
        catalog_generation: "catalog-generation".to_owned(),
        workspace_generation: "workspace-generation".to_owned(),
        method: method.to_owned(),
        params: serde_json::json!({}),
        client_timing_witness: None,
    }
}

fn large_response() -> ClientFrame {
    ClientFrame::Response {
        base: match request("rust.query") {
            ClientFrame::Request { base, .. } => base,
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
fn grpc_transport_preserves_catalog_request_class() {
    assert_eq!(
        response_budget_for_frame(&request(WORKSPACE_GENERATION_ENSURE_READY_METHOD)),
        None,
    );
    for method in [
        "rust.query",
        agent_semantic_client_protocol::WORKSPACE_SEARCH_PLAYBOOK_METHOD,
        agent_semantic_client_protocol::WORKSPACE_QUERY_PLAYBOOK_METHOD,
        agent_semantic_client_protocol::WORKSPACE_SYNTAX_PLAN_CONTEXT_METHOD,
    ] {
        assert_eq!(
            response_budget_for_frame(&request(method)),
            Some(
                agent_semantic_client_protocol::FIRST_COMPUTATION_OBSERVATION_BUDGET
                    + std::time::Duration::from_secs(1)
            )
        );
    }
    assert_eq!(
        response_budget_for_frame(&request("rust.search")),
        Some(CLIENT_FRAME_RESPONSE_BUDGET)
    );
    assert_eq!(
        response_budget_for_frame(&request("asp.graph.evaluate")),
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
