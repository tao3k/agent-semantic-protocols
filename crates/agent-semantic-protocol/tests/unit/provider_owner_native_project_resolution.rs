#[test]
fn native_owner_request_preserves_complete_repository_candidate_identity() {
    let workspace_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("workspace root");
    let snapshot =
        agent_semantic_runtime::git::discover_repository_candidate_snapshot(workspace_root)
            .expect("discover repository candidates")
            .expect("Git repository candidate snapshot");
    let request = super::encode_provider_project_resolution_request(
        "rust",
        "rs-harness",
        workspace_root,
        &snapshot,
    )
    .expect("encode typed project-resolution request");
    let value: serde_json::Value =
        serde_json::from_slice(&request).expect("decode typed project-resolution request");
    let candidates = &value["repositoryCandidates"];

    assert_eq!(candidates["schemaId"], snapshot.schema_id);
    assert_eq!(
        candidates["candidateGeneration"]["algorithm"],
        snapshot.candidate_generation.algorithm
    );
    assert_eq!(
        candidates["candidateGeneration"]["digest"],
        snapshot.candidate_generation.digest
    );
    assert_eq!(
        candidates["repositoryIdentity"]["repositoryId"],
        snapshot.repository_identity.repository_id
    );
    assert_eq!(
        candidates["worktreeIdentity"]["worktreeId"],
        snapshot.worktree_identity.worktree_id
    );
    assert!(candidates["metrics"]["candidateCount"].is_number());
}
