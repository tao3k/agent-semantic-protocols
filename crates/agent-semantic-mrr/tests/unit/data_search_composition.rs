// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later
use super::*;
fn leaf(id: &str, owners: &[&str], truncated: bool) -> DataSearchLeaf {
    DataSearchLeaf {
        identity: id.to_owned(),
        owners: owners.iter().map(|owner| (*owner).to_owned()).collect(),
        complete: !truncated,
        truncated,
    }
}
fn input(
    mode: DataSearchCandidateComposition,
    leaves: Vec<DataSearchLeaf>,
) -> DataSearchCompositionInput<'static> {
    DataSearchCompositionInput {
        project_id: "project",
        workspace_id: "worktree",
        runtime_generation: "runtime-a",
        content_generation: "content-a",
        expected_content_generation: "content-a",
        resident_view_digest: "view-a",
        composition_abi: "abi-a",
        query_digest: "query-a",
        mode,
        leaves,
        max_observations: NonZeroUsize::new(64).unwrap(),
    }
}
#[test]
fn actual_intersection_is_data_owned_and_native_receipt_is_bound() {
    let result = compose_resident_data_search(input(
        DataSearchCandidateComposition::Intersect,
        vec![
            leaf("rg:0", &["a.rs", "b.rs"], false),
            leaf("tantivy:0", &["b.rs", "c.rs"], false),
        ],
    ))
    .unwrap();
    assert_eq!(result.owners, BTreeSet::from(["b.rs".to_owned()]));
    assert_eq!(result.receipt["schemaVersion"], "1");
    assert_eq!(result.receipt["pooFactorIds"].as_array().unwrap().len(), 3);
    assert_eq!(result.receipt["pooEdges"].as_array().unwrap().len(), 2);
    assert!(
        result.receipt["pooDagDigest"]
            .as_str()
            .unwrap()
            .starts_with("blake3-256:")
    );
    assert_eq!(result.receipt["mergedOwnerCount"], 1);
    assert_eq!(result.receipt["observationCount"], 5);
    assert!(
        result.receipt["nativeReasoningDigest"]
            .as_str()
            .unwrap()
            .len()
            > 32
    );
}
#[test]
fn rank_join_preserves_regex_truth_despite_partial_ranking() {
    let result = compose_resident_data_search(input(
        DataSearchCandidateComposition::RankJoin,
        vec![
            leaf("rg:0", &["a.rs", "b.rs"], false),
            leaf("tantivy:0", &["b.rs"], true),
        ],
    ))
    .unwrap();
    assert_eq!(result.owners.len(), 2);
}
#[test]
fn incomplete_intersection_cannot_prove_absence() {
    assert!(
        compose_resident_data_search(input(
            DataSearchCandidateComposition::Intersect,
            vec![leaf("rg:0", &["a.rs"], false), leaf("tantivy:0", &[], true)]
        ))
        .is_err()
    );
}
#[test]
fn stale_content_is_rejected_and_worktree_binding_changes_generation() {
    let mut stale = input(
        DataSearchCandidateComposition::Single,
        vec![leaf("t:0", &["a.rs"], false)],
    );
    stale.expected_content_generation = "stale";
    assert!(
        compose_resident_data_search(stale)
            .unwrap_err()
            .contains("stale-content")
    );
    let first = compose_resident_data_search(input(
        DataSearchCandidateComposition::Single,
        vec![leaf("t:0", &["a.rs"], false)],
    ))
    .unwrap();
    let mut second_input = input(
        DataSearchCandidateComposition::Single,
        vec![leaf("t:0", &["a.rs"], false)],
    );
    second_input.workspace_id = "other-worktree";
    let second = compose_resident_data_search(second_input).unwrap();
    assert_ne!(
        first.receipt["generationIdentity"],
        second.receipt["generationIdentity"]
    );
    assert_ne!(
        first.receipt["nativeReasoningDigest"],
        second.receipt["nativeReasoningDigest"]
    );
}
