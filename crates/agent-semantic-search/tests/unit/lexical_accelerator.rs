use agent_semantic_search::{
    AdmittedLexicalOwner, COLD_RG_QUERY_RECEIPT_SCHEMA_ID, ColdRgQueryReceipt,
    ContentSearchGenerationReceipt, LexicalAcceleratorReceipt, LexicalOwnerFact,
    LexicalRecallRoute, LexicalRouteEquivalenceCase, SearchGenerationConstructionStage,
    SearchGenerationIdentity, SearchGenerationStageReceipt, plan_lexical_generation,
    plan_lexical_recall_route,
};

fn digest(value: &str) -> String {
    format!("blake3-256:{}", blake3::hash(value.as_bytes()).to_hex())
}

fn identity() -> SearchGenerationIdentity {
    SearchGenerationIdentity {
        project_id: "project".to_owned(),
        workspace_id: "workspace".to_owned(),
        source_root_digest: digest("root"),
        provider_digest: digest("provider"),
        schema_digest: digest("schema"),
        generation_candidate_digest: digest("candidate"),
    }
}

fn plan() -> agent_semantic_search::LexicalGenerationPlan {
    let content_digest = digest("owner");
    let query_keys = vec!["owner".to_owned()];
    plan_lexical_generation(
        &digest("analyzer"),
        ["src/lib.rs"],
        [AdmittedLexicalOwner {
            owner_path: "src/lib.rs",
            content_digest: &content_digest,
        }],
        [LexicalOwnerFact {
            owner_path: "src/lib.rs",
            content_digest: &content_digest,
            query_keys: &query_keys,
        }],
        [],
    )
    .expect("lexical plan")
}

fn content_generation_with_bytes(label: &str) -> ContentSearchGenerationReceipt {
    let identity = identity();
    let stage = |stage, label: &str| SearchGenerationStageReceipt {
        stage,
        identity: identity.clone(),
        artifact_digest: digest(label),
        worker_id: label.to_owned(),
        complete: true,
    };
    ContentSearchGenerationReceipt::new(stage(
        SearchGenerationConstructionStage::SourceByteAcquisition,
        label,
    ))
    .expect("content generation")
}

fn content_generation() -> ContentSearchGenerationReceipt {
    content_generation_with_bytes("bytes")
}

#[test]
fn cold_rg_receipt_has_zero_processes_and_no_tantivy_build() {
    ColdRgQueryReceipt {
        schema_id: COLD_RG_QUERY_RECEIPT_SCHEMA_ID.to_owned(),
        schema_version: "1".to_owned(),
        identity: identity(),
        inventory_digest: digest("inventory"),
        normalized_query_digest: digest("query"),
        candidate_set_digest: digest("candidates"),
        fd_process_count: 0,
        rg_process_count: 0,
        tantivy_build_count: 0,
        complete: true,
    }
    .validate()
    .expect("cold rg route");
}

#[test]
fn accelerator_publication_requires_rg_tantivy_equivalence() {
    let candidates = digest("candidates");
    LexicalAcceleratorReceipt::build(
        &content_generation(),
        &plan(),
        digest("tantivy"),
        vec![LexicalRouteEquivalenceCase {
            normalized_query_digest: digest("query"),
            cold_rg_candidate_set_digest: candidates.clone(),
            tantivy_candidate_set_digest: candidates,
        }],
    )
    .expect("equivalent accelerator");
}

#[test]
fn accelerator_candidate_drift_fails_closed() {
    let error = LexicalAcceleratorReceipt::build(
        &content_generation(),
        &plan(),
        digest("tantivy"),
        vec![LexicalRouteEquivalenceCase {
            normalized_query_digest: digest("query"),
            cold_rg_candidate_set_digest: digest("rg"),
            tantivy_candidate_set_digest: digest("tantivy-results"),
        }],
    )
    .expect_err("route drift must fail");
    assert!(error.contains("not equivalent"));
}

#[test]
fn content_generation_routes_to_rg_before_accelerator_publication() {
    assert_eq!(
        plan_lexical_recall_route(&content_generation(), None).expect("cold route"),
        LexicalRecallRoute::ColdRg,
    );
}

#[test]
fn exact_accelerator_switches_the_same_generation_to_tantivy() {
    let candidates = digest("candidates");
    let accelerator = LexicalAcceleratorReceipt::build(
        &content_generation(),
        &plan(),
        digest("tantivy"),
        vec![LexicalRouteEquivalenceCase {
            normalized_query_digest: digest("query"),
            cold_rg_candidate_set_digest: candidates.clone(),
            tantivy_candidate_set_digest: candidates,
        }],
    )
    .expect("accelerator");
    assert_eq!(
        plan_lexical_recall_route(&content_generation(), Some(&accelerator))
            .expect("Tantivy route"),
        LexicalRecallRoute::Tantivy,
    );
}

#[test]
fn accelerator_from_another_content_generation_fails_closed() {
    let candidates = digest("candidates");
    let foreign = content_generation_with_bytes("foreign-bytes");
    let accelerator = LexicalAcceleratorReceipt::build(
        &foreign,
        &plan(),
        digest("tantivy"),
        vec![LexicalRouteEquivalenceCase {
            normalized_query_digest: digest("query"),
            cold_rg_candidate_set_digest: candidates.clone(),
            tantivy_candidate_set_digest: candidates,
        }],
    )
    .expect("foreign accelerator");
    assert!(
        plan_lexical_recall_route(&content_generation(), Some(&accelerator))
            .expect_err("foreign generation must fail")
            .contains("content-generation identity drift")
    );
}
