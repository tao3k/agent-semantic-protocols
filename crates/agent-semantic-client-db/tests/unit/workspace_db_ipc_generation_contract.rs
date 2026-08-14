use agent_semantic_client_db::workspace_db_ipc::WorkspaceDbIpcOperation;

fn candidate_identity()
-> agent_semantic_client_db::runtime_server_admission::WorkspaceGenerationCandidateIdentity {
    agent_semantic_client_db::runtime_server_admission::WorkspaceGenerationCandidateIdentity {
        candidate_generation: agent_semantic_runtime::git::RepositoryCandidateGeneration {
            algorithm: "blake3-worktree-state-v1".to_owned(),
            digest: "blake3:cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc"
                .to_owned(),
            authorities: vec![agent_semantic_runtime::git::RepositoryCandidateAuthority::GitIndex],
        },
        policy_overlay_digest:
            "blake3:dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd".to_owned(),
    }
}

fn candidate_json() -> serde_json::Value {
    serde_json::json!({
        "candidateGeneration": {
            "algorithm": "blake3-worktree-state-v1",
            "digest": "blake3:cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc",
            "authorities": ["git-index"]
        },
        "policyOverlayDigest": "blake3:dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd"
    })
}

#[test]
fn runtime_generation_admission_wire_shape_requires_lifecycle_identity() {
    let admit = serde_json::to_value(WorkspaceDbIpcOperation::AdmitRuntimeGeneration {
        mutation_id: "session-root/tool-use-1".to_owned(),
        project_root: "/workspace".to_owned(),
        changed_paths: vec![
            "/workspace/src/lib.rs".to_owned(),
            "/workspace/languages/rust/src/lib.rs".to_owned(),
        ],
        candidate: candidate_identity(),
    })
    .expect("encode runtime generation admission");
    let submit = serde_json::to_value(WorkspaceDbIpcOperation::SubmitRuntimeGenerationMutation {
        mutation_id: "session-root/tool-use-1".to_owned(),
        project_root: "/workspace".to_owned(),
        changed_paths: vec!["/workspace/src/lib.rs".to_owned()],
    })
    .expect("encode daemon-owned runtime generation submission");
    assert_eq!(
        admit,
        serde_json::json!({
            "kind": "admit-runtime-generation",
            "mutationId": "session-root/tool-use-1",
            "projectRoot": "/workspace",
            "changedPaths": [
                "/workspace/src/lib.rs",
                "/workspace/languages/rust/src/lib.rs"
            ],
            "candidate": candidate_json()
        })
    );
    assert_eq!(
        submit,
        serde_json::json!({
            "kind": "submit-runtime-generation-mutation",
            "mutationId": "session-root/tool-use-1",
            "projectRoot": "/workspace",
            "changedPaths": ["/workspace/src/lib.rs"]
        })
    );
}

#[test]
fn query_driven_generation_control_is_absent_from_the_wire_contract() {
    for kind in [
        "ensure-runtime-generation",
        "ensure-runtime-generation-ready",
        "repair-runtime-generation-locator",
    ] {
        let error = serde_json::from_value::<WorkspaceDbIpcOperation>(serde_json::json!({
            "requestId": "legacy-query-generation-control",
            "kind": kind,
            "projectRoot": "/workspace"
        }))
        .expect_err("legacy query-driven generation control must not deserialize");
        assert!(error.to_string().contains("unknown variant"), "{error}");
    }
}

fn mutation_json(mutation_id: Option<&str>, changed_paths: serde_json::Value) -> serde_json::Value {
    let mut value = serde_json::json!({
        "kind": "admit-runtime-generation",
        "projectRoot": "/workspace",
        "changedPaths": changed_paths,
        "candidate": candidate_json()
    });
    if let Some(mutation_id) = mutation_id {
        value["mutationId"] = serde_json::Value::String(mutation_id.to_owned());
    }
    value
}

#[test]
fn runtime_generation_mutation_admission_rejects_a_pathless_wire_request() {
    let error = serde_json::from_value::<WorkspaceDbIpcOperation>(mutation_json(
        Some("session-root/tool-use-1"),
        serde_json::json!([]),
    ))
    .expect_err("mutation admission must carry changed paths");
    assert!(error.to_string().contains("changedPaths"));
}

#[test]
fn runtime_generation_mutation_admission_rejects_duplicate_wire_paths() {
    let error = serde_json::from_value::<WorkspaceDbIpcOperation>(mutation_json(
        Some("session-root/tool-use-1"),
        serde_json::json!(["/workspace/src/lib.rs", "/workspace/src/lib.rs"]),
    ))
    .expect_err("mutation admission paths must be unique");
    assert!(error.to_string().contains("duplicate paths"));
}

#[test]
fn runtime_generation_mutation_admission_rejects_a_missing_mutation_identity() {
    let error = serde_json::from_value::<WorkspaceDbIpcOperation>(mutation_json(
        None,
        serde_json::json!(["/workspace/src/lib.rs"]),
    ))
    .expect_err("mutation admission must carry an event identity");
    assert!(error.to_string().contains("mutationId"));
}

#[test]
fn runtime_generation_mutation_admission_rejects_an_empty_mutation_identity() {
    let error = serde_json::from_value::<WorkspaceDbIpcOperation>(mutation_json(
        Some(" "),
        serde_json::json!(["/workspace/src/lib.rs"]),
    ))
    .expect_err("mutation admission identity must be non-empty");
    assert!(error.to_string().contains("mutationId"));
}
