use super::BTreeMap;
use super::BTreeSet;
use super::ProtocolId;
use super::RouteEdge;
use super::RouteExecutionMode;
use super::RouteProposal;
use super::RouteValidationError;
use super::edge_set;
use super::validate_execution_mode;
use super::validate_group_dependencies;
use super::validate_proposal_execution_groups;

fn id(value: &str) -> ProtocolId {
    ProtocolId::parse(value).expect("valid protocol id")
}

fn proposal_from_fixture(source: &str) -> RouteProposal {
    let packet: serde_json::Value =
        serde_json::from_str(source).expect("valid proposal-set fixture");
    serde_json::from_value(packet["proposals"][0].clone())
        .expect("fixture proposal matches the Rust v1 protocol")
}

#[test]
fn serial_group_may_contain_multiple_ordered_actions() {
    assert_eq!(
        validate_execution_mode(RouteExecutionMode::Serial, 3, 1, None, None,),
        Ok(())
    );
}

#[test]
fn batch_group_requires_multiple_items_and_one_capability() {
    let capability = id("rust.callers.batch.v1");
    assert_eq!(
        validate_execution_mode(RouteExecutionMode::Batch, 2, 1, Some(&capability), None,),
        Ok(())
    );
    assert_eq!(
        validate_execution_mode(RouteExecutionMode::Batch, 2, 1, None, None,),
        Err(RouteValidationError::BatchCapability)
    );
}

#[test]
fn parallel_group_accepts_independent_multi_point_query() {
    let ids = vec![id("N-IMPL"), id("N-TEST"), id("N-POLICY")];
    let edges = BTreeSet::new();
    let proof = id("proof.independent.impl-test-policy");
    assert_eq!(
        validate_execution_mode(
            RouteExecutionMode::Parallel,
            ids.len(),
            3,
            None,
            Some(&proof),
        ),
        Ok(())
    );
    assert_eq!(
        validate_group_dependencies(RouteExecutionMode::Parallel, &ids, &edges,),
        Ok(())
    );
}

#[test]
fn parallel_group_rejects_transitive_dependency() {
    let ids = vec![id("N1"), id("N3")];
    let all_ids = [id("N1"), id("N2"), id("N3")];
    let route_edges = vec![
        RouteEdge {
            from: all_ids[0].clone(),
            to: all_ids[1].clone(),
        },
        RouteEdge {
            from: all_ids[1].clone(),
            to: all_ids[2].clone(),
        },
    ];
    let edges = edge_set(&route_edges).expect("valid route edges");
    assert_eq!(
        validate_group_dependencies(RouteExecutionMode::Parallel, &ids, &edges,),
        Err(RouteValidationError::ParallelDependency)
    );
}

#[test]
fn semantic_validator_rejects_dependent_parallel_fixture() {
    let proposal = proposal_from_fixture(include_str!(
        "../../../../schemas/fixtures/search-route-proposal-set/invalid-parallel-dependent-node.v1.json"
    ));
    let nodes: BTreeMap<_, _> = proposal
        .nodes
        .iter()
        .map(|node| (&node.node_id, node))
        .collect();
    let edges = edge_set(&proposal.edges).expect("valid fixture edges");
    assert_eq!(
        validate_proposal_execution_groups(&nodes, &edges, &proposal),
        Err(RouteValidationError::ParallelDependency)
    );
}

#[test]
fn semantic_validator_rejects_batch_without_capability_fixture() {
    let proposal = proposal_from_fixture(include_str!(
        "../../../../schemas/fixtures/search-route-proposal-set/invalid-batch-without-capability.v1.json"
    ));
    let nodes: BTreeMap<_, _> = proposal
        .nodes
        .iter()
        .map(|node| (&node.node_id, node))
        .collect();
    let edges = edge_set(&proposal.edges).expect("valid fixture edges");
    assert_eq!(
        validate_proposal_execution_groups(&nodes, &edges, &proposal),
        Err(RouteValidationError::BatchCapability)
    );
}
