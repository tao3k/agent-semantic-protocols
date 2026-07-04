use agent_semantic_artifacts::{
    ArtifactGeneration, ArtifactJson, ArtifactRepoId, ArtifactScopeId, ArtifactWorkspaceId,
    RepairChainFrame, RepairChainFrameInput, RepairChainFrameKind, RepairChainParentRef,
    build_repair_chain_frame,
};
use agent_semantic_client_db::{
    ClientDbArtifactRepairChainFrame, ClientDbArtifactRoot, ClientDbProofReceipt,
};

#[test]
fn db_engine_artifact_graph_persists_repair_chain_and_proof_receipt() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("build Turso artifact graph test runtime");
    runtime.block_on(async {
        let project_root = temp_root("db-engine-artifact-graph-project");
        let state_home = temp_root("db-engine-artifact-graph-state-home");
        let state = ResolvedState::resolve_with_state_home(&project_root, &state_home)
            .expect("resolve state with explicit state home");
        state.ensure_minimal_layout().expect("create state layout");
        let engine = ClientDbEngine::from_resolved_state(&state);

        let how_from_frame = repair_frame(
            &state,
            RepairChainFrameKind::HowFromFrame,
            "repair-1/how-from",
            serde_json::json!({
                "intent": "locate executable evidence",
                "selectors": ["rust://crates/agent-semantic-client-db#artifact-graph"]
            }),
            Vec::new(),
        );
        let how_fix_frame = repair_frame(
            &state,
            RepairChainFrameKind::HowFixFrame,
            "repair-1/how-fix",
            serde_json::json!({
                "changeBoundary": "Turso artifact graph adapter",
                "proofPlan": ["cargo test -p agent-semantic-client-db db_engine_artifact_graph"]
            }),
            vec![RepairChainParentRef::new(
                "howFixFromHowFrom",
                how_from_frame.root.clone(),
            )],
        );
        let proof_frame = repair_frame(
            &state,
            RepairChainFrameKind::ProofReceipt,
            "repair-1/proof",
            serde_json::json!({
                "commands": ["cargo test -p agent-semantic-client-db --locked"],
                "result": "pass"
            }),
            vec![RepairChainParentRef::new(
                "proofValidatesHowFix",
                how_fix_frame.root.clone(),
            )],
        );

        let how_from: ClientDbArtifactRoot = (&how_from_frame.root).into();
        let how_fix: ClientDbArtifactRoot = (&how_fix_frame.root).into();
        let proof_root: ClientDbArtifactRoot = (&proof_frame.root).into();
        assert_eq!(
            engine
                .upsert_artifact_roots(&[how_from.clone(), how_fix.clone(), proof_root.clone()])
                .await
                .expect("upsert artifact roots"),
            3
        );

        let how_fix_db_frame: ClientDbArtifactRepairChainFrame = (&how_fix_frame).into();
        let proof_db_frame: ClientDbArtifactRepairChainFrame = (&proof_frame).into();
        let how_edge = how_fix_db_frame.parents[0].clone();
        let proof_edge = proof_db_frame.parents[0].clone();
        assert_eq!(
            engine
                .upsert_artifact_edges(&[how_edge.clone(), proof_edge.clone()])
                .await
                .expect("upsert artifact edges"),
            2
        );
        assert_eq!(
            engine
                .upsert_repair_chain_frames(std::slice::from_ref(&how_fix_db_frame))
                .await
                .expect("upsert repair-chain frame"),
            1
        );

        let proof_receipt = ClientDbProofReceipt {
            receipt_id: "proof-receipt:repair-1".to_string(),
            obligation_id: "proof-obligation:repair-1".to_string(),
            recipe_id: "proof-recipe:repair-1".to_string(),
            checker: "axle.verify_proof".to_string(),
            environment: "unit-test".to_string(),
            okay: true,
            trust_level: "verify-proof".to_string(),
            summary_for_agent: "howFixFrame is backed by executable howFrom evidence".to_string(),
            root: proof_root.clone(),
        };
        assert_eq!(
            engine
                .upsert_proof_receipts(std::slice::from_ref(&proof_receipt))
                .await
                .expect("upsert proof receipt"),
            1
        );

        let how_edges = engine
            .lookup_artifact_edges(Some(&how_from.root_hash.value), 8)
            .await
            .expect("lookup howFrom artifact edges");
        assert_eq!(how_edges, vec![how_edge]);
        let proof_edges = engine
            .lookup_artifact_edges(Some(&how_fix.root_hash.value), 8)
            .await
            .expect("lookup howFix artifact edges");
        assert_eq!(proof_edges, vec![proof_edge]);

        let frames = engine
            .lookup_repair_chain_frames(Some("howFixFrame"), 8)
            .await
            .expect("lookup repair-chain frames");
        assert_eq!(frames, vec![how_fix_db_frame]);

        let receipts = engine
            .lookup_proof_receipts(Some(&proof_root.root_hash.value), 8)
            .await
            .expect("lookup proof receipts");
        assert_eq!(receipts, vec![proof_receipt]);

        let render = engine
            .render_artifact_graph_compact(Some("howFixFrame"), 8)
            .await
            .expect("render compact artifact graph");
        let rendered = render.to_text();
        assert_eq!(render.frame_count, 1);
        assert_eq!(render.proof_receipt_count, 1);
        assert!(rendered.contains("|artifactGraph frameCount=1 proofReceiptCount=1"));
        assert!(rendered.contains("|repairFrame kind=howFixFrame"));
        assert!(rendered.contains("|artifactEdge role=howFixFromHowFrom"));
        assert!(rendered.contains("|proofReceipt id=proof-receipt:repair-1 ok=true"));
        assert!(
            !rendered.contains('{') && !rendered.contains('}'),
            "compact render must not dump JSON: {rendered}"
        );

        let _ = std::fs::remove_dir_all(project_root);
        let _ = std::fs::remove_dir_all(state_home);
    });
}

fn repair_frame(
    state: &ResolvedState,
    frame_kind: RepairChainFrameKind,
    generation: &str,
    content: serde_json::Value,
    parents: Vec<RepairChainParentRef>,
) -> RepairChainFrame {
    build_repair_chain_frame(RepairChainFrameInput::new(
        frame_kind,
        ArtifactRepoId::new(state.repo.repo_id.to_string()),
        ArtifactWorkspaceId::new(state.workspace.workspace_id.to_string()),
        ArtifactScopeId::new(state.scope_id.to_string()),
        ArtifactGeneration::new(generation),
        ArtifactJson::new(content),
        parents,
    ))
}
