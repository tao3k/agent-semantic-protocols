use agent_semantic_artifacts::{
    ARTIFACT_EDGE_SCHEMA_ID, ARTIFACT_EDGE_SCHEMA_VERSION, ArtifactGeneration, ArtifactHash,
    ArtifactKind, ArtifactRepoId, ArtifactRootEdgeInput, ArtifactRootInput, ArtifactRootRef,
    ArtifactScopeId, ArtifactWorkspaceId, build_artifact_root_edge, hash_artifact_root_edge,
};

fn root(root_kind: &str, generation: &str, seed: &[u8]) -> ArtifactRootRef {
    ArtifactRootRef::from_input(
        ArtifactRootInput {
            repo_id: ArtifactRepoId::new("repo"),
            workspace_id: ArtifactWorkspaceId::new("workspace"),
            scope_id: ArtifactScopeId::new("default"),
            generation: ArtifactGeneration::new(generation),
            root_kind: ArtifactKind::new(root_kind),
            node_hash: ArtifactHash::blake3(seed),
        },
        None,
        None,
        None,
    )
}

#[test]
fn artifact_root_edge_hash_is_deterministic() {
    let input = ArtifactRootEdgeInput::new(
        "howFrom",
        root("howFixFrame", "how-fix-1", b"how-fix"),
        root("howFromFrame", "how-from-1", b"how-from"),
    );

    assert_eq!(
        hash_artifact_root_edge(&input),
        hash_artifact_root_edge(&input)
    );
}

#[test]
fn artifact_root_edge_hash_changes_when_role_changes() {
    let parent = root("proofReceipt", "proof-1", b"proof");
    let child = root("changeSet", "change-set-1", b"change-set");
    let left = ArtifactRootEdgeInput::new("changeSet", parent.clone(), child.clone());
    let right = ArtifactRootEdgeInput::new("graphDiff", parent, child);

    assert_ne!(
        hash_artifact_root_edge(&left),
        hash_artifact_root_edge(&right)
    );
}

#[test]
fn artifact_root_edge_hash_changes_when_direction_changes() {
    let parent = root("proofReceipt", "proof-1", b"proof");
    let child = root("changeSet", "change-set-1", b"change-set");
    let forward = ArtifactRootEdgeInput::new("proof", parent.clone(), child.clone());
    let reverse = ArtifactRootEdgeInput::new("proof", child, parent);

    assert_ne!(
        hash_artifact_root_edge(&forward),
        hash_artifact_root_edge(&reverse)
    );
}

#[test]
fn artifact_root_edge_serializes_schema_identity() {
    let edge = build_artifact_root_edge(
        ArtifactRootEdgeInput::new(
            "graphDiff",
            root("proofReceipt", "proof-1", b"proof"),
            root("graphDiff", "graph-diff-1", b"graph-diff"),
        )
        .with_ordinal(1),
    );
    let json = serde_json::to_value(edge).expect("serialize artifact root edge");

    assert_eq!(json["schemaId"], ARTIFACT_EDGE_SCHEMA_ID);
    assert_eq!(json["schemaVersion"], ARTIFACT_EDGE_SCHEMA_VERSION);
    assert_eq!(json["role"], "graphDiff");
    assert_eq!(json["ordinal"], 1);
    assert_eq!(json["parent"]["rootKind"], "proofReceipt");
    assert_eq!(json["child"]["rootKind"], "graphDiff");
    assert_eq!(json["edgeHash"]["algorithm"], "blake3");
}
