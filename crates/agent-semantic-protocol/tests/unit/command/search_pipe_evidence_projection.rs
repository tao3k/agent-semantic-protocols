use std::collections::HashMap;

#[path = "../../../src/command/search_pipe_evidence_projection.rs"]
mod search_pipe_evidence_projection;

use search_pipe_evidence_projection::rank_frontier_has_only_owner_or_topology_nodes;

#[test]
fn rank_frontier_is_redundant_for_owner_only_evidence() {
    let kinds = evidence_kinds(&[("O", "owner"), ("O2", "owner")]);

    assert!(rank_frontier_has_only_owner_or_topology_nodes(&kinds));
}

#[test]
fn rank_frontier_is_redundant_for_owner_with_topology_context() {
    let kinds = evidence_kinds(&[("O", "owner"), ("W", "workspace"), ("P", "provider-root")]);

    assert!(rank_frontier_has_only_owner_or_topology_nodes(&kinds));
}

#[test]
fn rank_frontier_is_redundant_for_owner_with_submodule_context() {
    let kinds = evidence_kinds(&[("O", "owner"), ("S", "submodule")]);

    assert!(rank_frontier_has_only_owner_or_topology_nodes(&kinds));
}

#[test]
fn rank_frontier_stays_visible_for_syntax_evidence() {
    let kinds = evidence_kinds(&[("O", "owner"), ("I", "item")]);

    assert!(!rank_frontier_has_only_owner_or_topology_nodes(&kinds));
}

fn evidence_kinds(entries: &[(&str, &str)]) -> HashMap<String, String> {
    entries
        .iter()
        .map(|(alias, kind)| ((*alias).to_string(), (*kind).to_string()))
        .collect()
}
