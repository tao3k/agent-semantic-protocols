use agent_semantic_artifacts::{
    ArtifactGeneration, ArtifactHash, ArtifactJson, ArtifactKind, ArtifactRepoId,
    ArtifactRootInput, ArtifactRootRef, ArtifactScopeId, ArtifactWorkspaceId,
    RepairChainFrameInput, RepairChainFrameKind, RepairChainParentRef, build_repair_chain_frame,
    hash_normalized_json,
};
use serde_json::json;

fn test_root(root_kind: &str, generation: &str, seed: &[u8]) -> ArtifactRootRef {
    let node_hash = ArtifactHash::blake3(seed);
    ArtifactRootRef::from_input(
        ArtifactRootInput {
            repo_id: ArtifactRepoId::new("repo"),
            workspace_id: ArtifactWorkspaceId::new("workspace"),
            scope_id: ArtifactScopeId::new("default"),
            generation: ArtifactGeneration::new(generation),
            root_kind: ArtifactKind::new(root_kind),
            node_hash,
        },
        None,
        None,
        None,
    )
}

fn frame_input<T: serde::Serialize>(
    frame_kind: RepairChainFrameKind,
    generation: &str,
    content: T,
    parents: Vec<RepairChainParentRef>,
) -> RepairChainFrameInput {
    RepairChainFrameInput::new(
        frame_kind,
        ArtifactRepoId::new("repo"),
        ArtifactWorkspaceId::new("workspace"),
        ArtifactScopeId::new("default"),
        ArtifactGeneration::new(generation),
        ArtifactJson::from_serializable(&content).expect("repair frame content must serialize"),
        parents,
    )
}

#[test]
fn how_from_frame_links_search_sources_and_graph_roots() {
    let search_receipt = test_root("searchReceipt", "g1", b"search");
    let provider_output = test_root("providerOutput", "g1", b"provider");
    let dynamic_overlay = test_root("dynamicOverlay", "session-1", b"overlay");
    let compact_graph = test_root("compactGraph", "g1", b"compact-graph");
    let content = json!({
        "intent": "find owner evidence",
        "selectors": ["crates/agent-semantic-artifacts#repair-chain"],
    });
    let expected_content_hash = hash_normalized_json(
        &ArtifactJson::from_serializable(&content).expect("test artifact JSON should serialize"),
    );
    let frame = build_repair_chain_frame(frame_input(
        RepairChainFrameKind::HowFromFrame,
        "how-from-1",
        content,
        vec![
            RepairChainParentRef::new("searchReceipt", search_receipt),
            RepairChainParentRef::new("providerOutput", provider_output),
            RepairChainParentRef::new("dynamicOverlay", dynamic_overlay),
            RepairChainParentRef::new("compactGraph", compact_graph),
        ],
    ));

    assert_eq!(frame.frame_kind, RepairChainFrameKind::HowFromFrame);
    assert_eq!(frame.root.root_kind.as_str(), "howFromFrame");
    assert_eq!(frame.content_hash, expected_content_hash);
    assert_eq!(frame.parents.len(), 4);
}

#[test]
fn repair_chain_parent_order_does_not_change_root_hash() {
    let search_receipt =
        RepairChainParentRef::new("searchReceipt", test_root("searchReceipt", "g1", b"search"));
    let source_index = RepairChainParentRef::new(
        "sourceIndexBundle",
        test_root("sourceIndexBundle", "g1", b"source-index"),
    );
    let content = json!({"intent": "stable evidence"});
    let left = build_repair_chain_frame(frame_input(
        RepairChainFrameKind::HowFromFrame,
        "how-from-1",
        content.clone(),
        vec![source_index.clone(), search_receipt.clone()],
    ));
    let right = build_repair_chain_frame(frame_input(
        RepairChainFrameKind::HowFromFrame,
        "how-from-1",
        content,
        vec![search_receipt, source_index],
    ));

    assert_eq!(left.root.root_hash, right.root.root_hash);
}

#[test]
fn how_fix_frame_depends_on_how_from_root() {
    let how_from = build_repair_chain_frame(frame_input(
        RepairChainFrameKind::HowFromFrame,
        "how-from-1",
        json!({"intent": "repair boundary"}),
        vec![RepairChainParentRef::new(
            "providerOutput",
            test_root("providerOutput", "g1", b"provider"),
        )],
    ));
    let how_fix = build_repair_chain_frame(frame_input(
        RepairChainFrameKind::HowFixFrame,
        "how-fix-1",
        json!({
            "owners": ["agent-semantic-artifacts"],
            "proof": ["cargo test -p agent-semantic-artifacts"],
        }),
        vec![RepairChainParentRef::new("howFrom", how_from.root.clone())],
    ));
    let changed_parent = build_repair_chain_frame(frame_input(
        RepairChainFrameKind::HowFromFrame,
        "how-from-2",
        json!({"intent": "repair boundary"}),
        vec![RepairChainParentRef::new(
            "providerOutput",
            test_root("providerOutput", "g2", b"provider-changed"),
        )],
    ));
    let changed_how_fix = build_repair_chain_frame(frame_input(
        RepairChainFrameKind::HowFixFrame,
        "how-fix-1",
        json!({
            "owners": ["agent-semantic-artifacts"],
            "proof": ["cargo test -p agent-semantic-artifacts"],
        }),
        vec![RepairChainParentRef::new(
            "howFrom",
            changed_parent.root.clone(),
        )],
    ));

    assert_eq!(how_fix.root.root_kind.as_str(), "howFixFrame");
    assert_ne!(how_fix.root.root_hash, changed_how_fix.root.root_hash);
}

#[test]
fn proof_receipt_links_change_set_and_graph_diff() {
    let how_fix = build_repair_chain_frame(frame_input(
        RepairChainFrameKind::HowFixFrame,
        "how-fix-1",
        json!({"owners": ["agent-semantic-artifacts"]}),
        vec![RepairChainParentRef::new(
            "howFrom",
            test_root("howFromFrame", "how-from-1", b"how-from"),
        )],
    ));
    let change_set = build_repair_chain_frame(frame_input(
        RepairChainFrameKind::ChangeSet,
        "change-set-1",
        json!({"changedSelectors": ["schemas/semantic-artifact-identity"]}),
        vec![RepairChainParentRef::new("howFix", how_fix.root.clone())],
    ));
    let graph_diff = build_repair_chain_frame(frame_input(
        RepairChainFrameKind::GraphDiff,
        "graph-diff-1",
        json!({"addedRootKinds": ["howFromFrame", "proofReceipt"]}),
        vec![RepairChainParentRef::new(
            "changeSet",
            change_set.root.clone(),
        )],
    ));
    let proof = build_repair_chain_frame(frame_input(
        RepairChainFrameKind::ProofReceipt,
        "proof-1",
        json!({"commands": ["cargo test -p agent-semantic-artifacts"]}),
        vec![
            RepairChainParentRef::new("changeSet", change_set.root),
            RepairChainParentRef::new("graphDiff", graph_diff.root),
        ],
    ));

    assert_eq!(proof.root.root_kind.as_str(), "proofReceipt");
    assert_eq!(proof.parents.len(), 2);
    assert!(
        proof
            .parents
            .iter()
            .any(|parent| parent.role == "changeSet")
    );
    assert!(
        proof
            .parents
            .iter()
            .any(|parent| parent.role == "graphDiff")
    );
}
