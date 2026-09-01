use agent_semantic_content_identity::{
    SourceSnapshotEvidence, SourceSnapshotKind,
    provider_projection_relation::{ProviderProjectedRelation, ProviderProjectedRelationEndpoint},
    workspace_generation_evidence::WorkspaceGenerationEvidenceV1,
};
use agent_semantic_search_projection::{
    GraphTurboEvaluationRequest, ResidentSearchHit, ResidentSearchProjectionTier,
};

use crate::{
    ResidentGraphSearchRequest, build_resident_graph_generation,
    build_resident_graph_search_request,
};

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
            kind: "owner".to_owned(),
            id: owner.to_owned(),
        },
        kind: "contains".to_owned(),
        to: ProviderProjectedRelationEndpoint {
            kind: "item".to_owned(),
            id: "WorkspaceGenerationAdmission::compare".to_owned(),
        },
    }
}

#[test]
fn lexical_frontier_projects_one_generation_bound_graph_request() {
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
    let graph = build_resident_graph_generation(
        &snapshot,
        &generation,
        ["src/lib.rs".to_owned()],
        relations,
    )
    .expect("generation graph");
    let generation_digest = format!("blake3-256:{}", "c".repeat(64));
    let value = build_resident_graph_search_request(ResidentGraphSearchRequest {
        operation_id: "dispatch-1",
        operation: "pipe",
        query: "WorkspaceGenerationAdmission compare",
        language_id: "rust",
        provider_id: "asp-rust",
        generation_digest: &generation_digest,
        source_snapshot: &snapshot,
        workspace_generation: &generation,
        lexical_hits: &hits,
        generation_graph: &graph,
    })
    .expect("request projection succeeds")
    .expect("non-empty lexical frontier produces graph work");

    GraphTurboEvaluationRequest::from_value(value.clone())
        .expect("projected request validates through the shared v1 schema");
    assert_eq!(value["surface"], "search-pipe");
    assert_eq!(value["workspaceGeneration"]["rootDigest"], "a".repeat(64));
    assert!(value.get("graph").is_none());
    assert_eq!(graph.graph()["edges"].as_array().unwrap().len(), 1);
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
    let graph = build_resident_graph_generation(
        &snapshot,
        &generation,
        ["src/other.rs".to_owned()],
        [relation("src/other.rs")],
    )
    .expect("generation graph");
    let generation_digest = format!("blake3-256:{}", "c".repeat(64));
    let error = build_resident_graph_search_request(ResidentGraphSearchRequest {
        operation_id: "dispatch-2",
        operation: "pipe",
        query: "compare",
        language_id: "rust",
        provider_id: "asp-rust",
        generation_digest: &generation_digest,
        source_snapshot: &snapshot,
        workspace_generation: &generation,
        lexical_hits: &hits,
        generation_graph: &graph,
    })
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
    let forward = build_resident_graph_generation(
        &snapshot,
        &generation,
        ["src/z.rs".to_owned(), "src/a.rs".to_owned()],
        [relation("src/a.rs")],
    )
    .expect("forward graph");
    let reverse = build_resident_graph_generation(
        &snapshot,
        &generation,
        ["src/a.rs".to_owned(), "src/z.rs".to_owned()],
        [relation("src/a.rs")],
    )
    .expect("reverse graph");

    assert_eq!(forward.digest(), reverse.digest());
    assert_eq!(forward.graph(), reverse.graph());
    assert!(std::sync::Arc::ptr_eq(
        &forward.shared_open_payload(),
        &forward.clone().shared_open_payload(),
    ));
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
            kind: "owner".to_owned(),
            id: "src/lib.rs".to_owned(),
        },
        kind: "imports".to_owned(),
        to: ProviderProjectedRelationEndpoint {
            kind: "owner".to_owned(),
            id: "src/missing.rs".to_owned(),
        },
    };

    let error = build_resident_graph_generation(
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
    let graph = build_resident_graph_generation(
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

    let error = build_resident_graph_search_request(ResidentGraphSearchRequest {
        operation_id: "dispatch-stale",
        operation: "pipe",
        query: "compare",
        language_id: "rust",
        provider_id: "asp-rust",
        generation_digest: &format!("blake3-256:{}", "c".repeat(64)),
        source_snapshot: &stale_snapshot,
        workspace_generation: &stale_generation,
        lexical_hits: &[hit("src/lib.rs")],
        generation_graph: &graph,
    })
    .expect_err("a graph admitted under a different source root must fail closed");

    assert!(error.contains("generation binding mismatch"), "{error}");
}
