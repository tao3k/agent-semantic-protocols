use super::{RouteExecutionMode, RouteProposal};

fn proposal_from_fixture(source: &str) -> RouteProposal {
    let packet: serde_json::Value =
        serde_json::from_str(source).expect("valid proposal-set fixture");
    serde_json::from_value(packet["proposals"][0].clone())
        .expect("fixture proposal matches the Rust v1 protocol")
}

#[test]
fn rust_protocol_accepts_serial_proposal_fixture() {
    let proposal = proposal_from_fixture(include_str!(
        "../../../../schemas/fixtures/search-route-proposal-set/ready-serial.v1.json"
    ));
    assert_eq!(
        proposal.execution_groups[0].mode,
        RouteExecutionMode::Serial
    );
}

#[test]
fn rust_protocol_accepts_batch_proposal_fixture() {
    let proposal = proposal_from_fixture(include_str!(
        "../../../../schemas/fixtures/search-route-proposal-set/ready-batch.v1.json"
    ));
    assert_eq!(proposal.execution_groups[0].mode, RouteExecutionMode::Batch);
    assert_eq!(proposal.execution_groups[0].node_ids.len(), 2);
}

#[test]
fn rust_protocol_accepts_parallel_proposal_fixture() {
    let proposal = proposal_from_fixture(include_str!(
        "../../../../schemas/fixtures/search-route-proposal-set/ready-parallel.v1.json"
    ));
    assert_eq!(
        proposal.execution_groups[0].mode,
        RouteExecutionMode::Parallel
    );
    assert_eq!(proposal.execution_groups[0].node_ids.len(), 3);
}
