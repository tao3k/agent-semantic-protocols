use agent_semantic_content_identity::SourceSnapshotEvidence;
use agent_semantic_content_identity::SourceSnapshotKind;
use agent_semantic_content_identity::provider_projection_relation::ProviderProjectedRelation;
use agent_semantic_content_identity::provider_projection_relation::ProviderProjectedRelationEndpoint;
use agent_semantic_content_identity::workspace_generation_evidence::WorkspaceGenerationEvidenceV1;
use agent_semantic_search_projection::ResidentSearchHit;
use agent_semantic_search_projection::ResidentSearchProjectionTier;

use crate::ContentSearchGenerationReceipt;
use crate::ResidentGraphEvaluationBudget;
use crate::ResidentGraphEvaluationRequest;
use crate::ResidentGraphSearchBudget;
use crate::ResidentGraphSearchRequest;
use crate::SearchGenerationConstructionStage;
use crate::SearchGenerationGraphRequest;
use crate::SearchGenerationIdentity;
use crate::SearchGenerationStageReceipt;
use crate::build_resident_graph_generation;
use crate::evaluate_resident_graph_generation;
use crate::rank_resident_graph_generation;
use crate::resident_graph_search::materialize_resident_graph_generation;
use crate::stable_graph_node_id;

fn content_generation(identity: &SearchGenerationIdentity) -> ContentSearchGenerationReceipt {
    let stage = |stage, byte: char| SearchGenerationStageReceipt {
        stage,
        identity: identity.clone(),
        artifact_digest: format!("blake3-256:{}", byte.to_string().repeat(64)),
        worker_id: byte.to_string(),
        complete: true,
    };
    ContentSearchGenerationReceipt::new(stage(
        SearchGenerationConstructionStage::SourceByteAcquisition,
        'e',
    ))
    .expect("content generation")
}

fn hit(path: &str) -> ResidentSearchHit {
    ResidentSearchHit {
        owner_path: path.to_owned(),
        owner_content_digest: format!("blake3-256:{}", "b".repeat(64)),
        language_id: Some("rust".to_owned()),
        projection_tier: ResidentSearchProjectionTier::ShallowNavigation,
        line_count: 10,
        query_keys: vec!["compare".to_owned()],
        selector: None,
        score: Some(1),
    }
}

fn relation(owner: &str) -> ProviderProjectedRelation {
    ProviderProjectedRelation {
        from: ProviderProjectedRelationEndpoint {
            kind: agent_semantic_content_identity::ProviderRelationEndpointKindV1::Owner,
            id: owner.to_owned(),
        },
        kind: "contains".into(),
        to: ProviderProjectedRelationEndpoint {
            kind: agent_semantic_content_identity::ProviderRelationEndpointKindV1::Item,
            id: "WorkspaceGenerationAdmission::compare".to_owned(),
        },
    }
}

fn owner_relation(from: &str, to: &str) -> ProviderProjectedRelation {
    ProviderProjectedRelation {
        from: ProviderProjectedRelationEndpoint {
            kind: agent_semantic_content_identity::ProviderRelationEndpointKindV1::Owner,
            id: from.to_owned(),
        },
        kind: "imports".into(),
        to: ProviderProjectedRelationEndpoint {
            kind: agent_semantic_content_identity::ProviderRelationEndpointKindV1::Owner,
            id: to.to_owned(),
        },
    }
}

#[test]
fn lexical_frontier_ranks_inside_one_generation_bound_graph() {
    let root = "a".repeat(64);
    let snapshot =
        SourceSnapshotEvidence::new(root.clone(), SourceSnapshotKind::Filesystem, 1, "provider");
    let generation = WorkspaceGenerationEvidenceV1 {
        root_digest: root,
        root_depth: 1,
        leaf_count: 1,
        owner_count: 1,
    };
    let hits = vec![hit("src/lib.rs")];
    let relations = vec![relation("src/lib.rs")];
    let graph = materialize_resident_graph_generation(
        &snapshot,
        &generation,
        ["src/lib.rs".to_owned()],
        relations,
    )
    .expect("generation graph");
    let generation_digest = format!("blake3-256:{}", "c".repeat(64));
    let stage = rank_resident_graph_generation(
        ResidentGraphSearchRequest {
            operation_id: "dispatch-1",
            operation: "conceptual",
            query: "WorkspaceGenerationAdmission compare",
            language_id: "rust",
            provider_id: "asp-rust",
            generation_digest: &generation_digest,
            source_snapshot: &snapshot,
            workspace_generation: &generation,
            lexical_hits: &hits,
            generation_graph: &graph,
        },
        ResidentGraphSearchBudget {
            max_nodes: 8,
            max_edges: 8,
            max_frontier: 8,
            max_results: 8,
        },
    )
    .expect("resident graph rank succeeds");

    assert_eq!(stage.generation_digest, generation_digest);
    assert_eq!(stage.ranked_owner_paths, ["src/lib.rs"]);
    assert_eq!(stage.work.provider_rpc_count, 0);
    assert_eq!(graph.edge_count(), 1);
}

#[test]
fn graph_projection_rejects_a_lexical_owner_outside_the_generation() {
    let root = "a".repeat(64);
    let snapshot =
        SourceSnapshotEvidence::new(root.clone(), SourceSnapshotKind::Filesystem, 1, "provider");
    let generation = WorkspaceGenerationEvidenceV1 {
        root_digest: root,
        root_depth: 1,
        leaf_count: 1,
        owner_count: 1,
    };
    let hits = vec![hit("src/lib.rs")];
    let graph = materialize_resident_graph_generation(
        &snapshot,
        &generation,
        ["src/other.rs".to_owned()],
        [relation("src/other.rs")],
    )
    .expect("generation graph");
    let generation_digest = format!("blake3-256:{}", "c".repeat(64));
    let error = rank_resident_graph_generation(
        ResidentGraphSearchRequest {
            operation_id: "dispatch-2",
            operation: "conceptual",
            query: "compare",
            language_id: "rust",
            provider_id: "asp-rust",
            generation_digest: &generation_digest,
            source_snapshot: &snapshot,
            workspace_generation: &generation,
            lexical_hits: &hits,
            generation_graph: &graph,
        },
        ResidentGraphSearchBudget {
            max_nodes: 8,
            max_edges: 8,
            max_frontier: 8,
            max_results: 8,
        },
    )
    .expect_err("lexical owner outside the generation must fail closed");

    assert!(error.contains("absent from generation graph"), "{error}");
}

#[test]
fn generation_graph_is_query_independent_and_deterministic() {
    let root = "a".repeat(64);
    let snapshot =
        SourceSnapshotEvidence::new(root.clone(), SourceSnapshotKind::Filesystem, 2, "provider");
    let generation = WorkspaceGenerationEvidenceV1 {
        root_digest: root,
        root_depth: 1,
        leaf_count: 2,
        owner_count: 2,
    };
    let forward = materialize_resident_graph_generation(
        &snapshot,
        &generation,
        ["src/z.rs".to_owned(), "src/a.rs".to_owned()],
        [relation("src/a.rs")],
    )
    .expect("forward graph");
    let reverse = materialize_resident_graph_generation(
        &snapshot,
        &generation,
        ["src/a.rs".to_owned(), "src/z.rs".to_owned()],
        [relation("src/a.rs")],
    )
    .expect("reverse graph");

    assert_eq!(forward.digest(), reverse.digest());
    assert_eq!(forward.node_count(), reverse.node_count());
    assert_eq!(forward.edge_count(), reverse.edge_count());
}

#[test]
fn generation_graph_rejects_a_relation_to_an_unadmitted_owner() {
    let root = "a".repeat(64);
    let snapshot =
        SourceSnapshotEvidence::new(root.clone(), SourceSnapshotKind::Filesystem, 1, "provider");
    let generation = WorkspaceGenerationEvidenceV1 {
        root_digest: root,
        root_depth: 1,
        leaf_count: 1,
        owner_count: 1,
    };
    let relation = ProviderProjectedRelation {
        from: ProviderProjectedRelationEndpoint {
            kind: agent_semantic_content_identity::ProviderRelationEndpointKindV1::Owner,
            id: "src/lib.rs".to_owned(),
        },
        kind: "imports".into(),
        to: ProviderProjectedRelationEndpoint {
            kind: agent_semantic_content_identity::ProviderRelationEndpointKindV1::Owner,
            id: "src/missing.rs".to_owned(),
        },
    };

    let error = materialize_resident_graph_generation(
        &snapshot,
        &generation,
        ["src/lib.rs".to_owned()],
        [relation],
    )
    .expect_err("phantom owner relation must fail closed");
    assert!(error.contains("unadmitted owner"), "{error}");
}

#[test]
fn generation_graph_cannot_be_reused_under_a_different_source_root() {
    let snapshot = SourceSnapshotEvidence::new(
        "a".repeat(64),
        SourceSnapshotKind::Filesystem,
        1,
        "provider",
    );
    let generation = WorkspaceGenerationEvidenceV1 {
        root_digest: "a".repeat(64),
        root_depth: 1,
        leaf_count: 1,
        owner_count: 1,
    };
    let graph = materialize_resident_graph_generation(
        &snapshot,
        &generation,
        ["src/lib.rs".to_owned()],
        [relation("src/lib.rs")],
    )
    .expect("generation graph");
    let stale_snapshot = SourceSnapshotEvidence::new(
        "b".repeat(64),
        SourceSnapshotKind::Filesystem,
        1,
        "provider",
    );
    let stale_generation = WorkspaceGenerationEvidenceV1 {
        root_digest: "b".repeat(64),
        root_depth: 1,
        leaf_count: 1,
        owner_count: 1,
    };

    let error = rank_resident_graph_generation(
        ResidentGraphSearchRequest {
            operation_id: "dispatch-stale",
            operation: "conceptual",
            query: "compare",
            language_id: "rust",
            provider_id: "asp-rust",
            generation_digest: &format!("blake3-256:{}", "c".repeat(64)),
            source_snapshot: &stale_snapshot,
            workspace_generation: &stale_generation,
            lexical_hits: &[hit("src/lib.rs")],
            generation_graph: &graph,
        },
        ResidentGraphSearchBudget {
            max_nodes: 8,
            max_edges: 8,
            max_frontier: 8,
            max_results: 8,
        },
    )
    .expect_err("a graph admitted under a different source root must fail closed");

    assert!(error.contains("generation binding mismatch"), "{error}");
}

#[test]
fn warm_graph_rank_executes_inside_the_resident_generation_without_provider_rpc() {
    let root = "a".repeat(64);
    let snapshot = SourceSnapshotEvidence::new(
        root.clone(),
        SourceSnapshotKind::Filesystem,
        2,
        format!("blake3-256:{}", "b".repeat(64)),
    );
    let generation = WorkspaceGenerationEvidenceV1 {
        root_digest: root,
        root_depth: 1,
        leaf_count: 2,
        owner_count: 2,
    };
    let identity = SearchGenerationIdentity {
        project_id: "project-cache".to_owned(),
        workspace_id: "workspace-cache".to_owned(),
        source_root_digest: format!("blake3-256:{}", "a".repeat(64)),
        provider_digest: format!("blake3-256:{}", "b".repeat(64)),
        schema_digest: format!("blake3-256:{}", "c".repeat(64)),
        generation_candidate_digest: format!("blake3-256:{}", "d".repeat(64)),
    };
    let graph_request = SearchGenerationGraphRequest::new(
        &content_generation(&identity),
        snapshot.clone(),
        generation.clone(),
        ["src/a.rs".to_owned(), "src/b.rs".to_owned()],
        [owner_relation("src/a.rs", "src/b.rs")],
    )
    .expect("generation graph request");
    let graph = build_resident_graph_generation(std::sync::Arc::new(graph_request))
        .expect("generation graph");
    let generation_digest = format!("blake3-256:{}", "c".repeat(64));
    let hits = [hit("src/a.rs")];
    let budget = ResidentGraphSearchBudget {
        max_nodes: 8,
        max_edges: 8,
        max_frontier: 8,
        max_results: 8,
    };
    let stage = rank_resident_graph_generation(
        ResidentGraphSearchRequest {
            operation_id: "dispatch-resident",
            operation: "conceptual",
            query: "compare",
            language_id: "rust",
            provider_id: "asp-rust",
            generation_digest: &generation_digest,
            source_snapshot: &snapshot,
            workspace_generation: &generation,
            lexical_hits: &hits,
            generation_graph: &graph,
        },
        budget,
    )
    .expect("resident graph rank");
    let cached = rank_resident_graph_generation(
        ResidentGraphSearchRequest {
            operation_id: "dispatch-resident-replay",
            operation: "conceptual",
            query: "COMPARE",
            language_id: "rust",
            provider_id: "asp-rust",
            generation_digest: &generation_digest,
            source_snapshot: &snapshot,
            workspace_generation: &generation,
            lexical_hits: &hits,
            generation_graph: &graph,
        },
        budget,
    )
    .expect("cached resident graph rank");

    assert_eq!(stage.ranked_owner_paths, ["src/a.rs", "src/b.rs"]);
    assert_eq!(stage.work.provider_rpc_count, 0);
    assert!(stage.work.visited_nodes <= 8);
    assert!(stage.work.visited_edges <= 8);
    assert!(stage.work.frontier_peak <= 8);
    assert_eq!(stage.work.cache_miss_count, 1);
    assert_eq!(stage.work.cache_hit_count, 0);
    assert_eq!(cached.work.cache_hit_count, 1);
    assert_eq!(cached.work.cache_miss_count, 0);
    assert_eq!(cached.work.cache_entry_count, 1);
    assert_eq!(cached.work.cache_capacity, 1_024);
    assert_eq!(cached.work.cache_shard_count, 64);
    assert!(cached.work.cache_value_bytes > 0);
    assert_eq!(cached.result_digest, stage.result_digest);
    assert_eq!(cached.ranked_owner_paths, stage.ranked_owner_paths);
}

#[test]
fn resident_graph_rank_fails_closed_when_the_explicit_budget_is_exhausted() {
    let root = "a".repeat(64);
    let snapshot =
        SourceSnapshotEvidence::new(root.clone(), SourceSnapshotKind::Filesystem, 2, "provider");
    let generation = WorkspaceGenerationEvidenceV1 {
        root_digest: root,
        root_depth: 1,
        leaf_count: 2,
        owner_count: 2,
    };
    let graph = materialize_resident_graph_generation(
        &snapshot,
        &generation,
        ["src/a.rs".to_owned(), "src/b.rs".to_owned()],
        [owner_relation("src/a.rs", "src/b.rs")],
    )
    .expect("generation graph");
    let generation_digest = format!("blake3-256:{}", "c".repeat(64));
    let error = rank_resident_graph_generation(
        ResidentGraphSearchRequest {
            operation_id: "dispatch-budget",
            operation: "conceptual",
            query: "compare",
            language_id: "rust",
            provider_id: "asp-rust",
            generation_digest: &generation_digest,
            source_snapshot: &snapshot,
            workspace_generation: &generation,
            lexical_hits: &[hit("src/a.rs")],
            generation_graph: &graph,
        },
        ResidentGraphSearchBudget {
            max_nodes: 1,
            max_edges: 0,
            max_frontier: 1,
            max_results: 1,
        },
    )
    .expect_err("resident graph must not exceed an explicit work budget");

    assert!(error.contains("graph-edge-budget-exhausted"), "{error}");
}

#[test]
fn intent_only_evaluation_uses_the_resident_generation_and_zero_provider_rpc() {
    let root = "a".repeat(64);
    let snapshot =
        SourceSnapshotEvidence::new(root.clone(), SourceSnapshotKind::Filesystem, 2, "provider");
    let generation = WorkspaceGenerationEvidenceV1 {
        root_digest: root,
        root_depth: 1,
        leaf_count: 2,
        owner_count: 2,
    };
    let graph = materialize_resident_graph_generation(
        &snapshot,
        &generation,
        ["src/a.rs".to_owned(), "src/b.rs".to_owned()],
        [owner_relation("src/a.rs", "src/b.rs")],
    )
    .expect("generation graph");
    let generation_digest = format!("blake3-256:{}", "c".repeat(64));
    let result = evaluate_resident_graph_generation(
        ResidentGraphEvaluationRequest {
            operation_id: "dispatch-evaluate",
            generation_digest: &generation_digest,
            source_snapshot: &snapshot,
            workspace_generation: &generation,
            entry_node_ids: &[stable_graph_node_id("owner", "src/a.rs")],
            generation_graph: &graph,
        },
        ResidentGraphEvaluationBudget {
            max_depth: 4,
            max_nodes: 8,
            max_edges: 8,
            max_results: 8,
        },
    )
    .expect("resident graph evaluation");

    assert_eq!(
        result
            .ranked_nodes
            .iter()
            .filter_map(|node| node.owner_path.as_deref())
            .collect::<Vec<_>>(),
        ["src/a.rs", "src/b.rs"]
    );
    assert_eq!(result.work.provider_rpc_count, 0);
    assert_eq!(result.edges.len(), 1);
}
