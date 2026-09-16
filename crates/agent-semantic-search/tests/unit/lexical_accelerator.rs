// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use agent_semantic_search::AdmittedLexicalOwner;
use agent_semantic_search::ContentSearchGenerationReceipt;
use agent_semantic_search::LexicalAcceleratorReceipt;
use agent_semantic_search::LexicalOwnerFact;
use agent_semantic_search::LexicalRecallRoute;
use agent_semantic_search::LexicalRouteEquivalenceCase;
use agent_semantic_search::SearchGenerationConstructionStage;
use agent_semantic_search::SearchGenerationIdentity;
use agent_semantic_search::SearchGenerationStageReceipt;
use agent_semantic_search::plan_lexical_generation;
use agent_semantic_search::plan_lexical_recall_route;

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
fn accelerator_publication_requires_rg_tantivy_equivalence() {
    let candidates = digest("candidates");
    LexicalAcceleratorReceipt::build(
        &content_generation(),
        &plan(),
        digest("tantivy"),
        vec![LexicalRouteEquivalenceCase {
            normalized_query_digest: digest("query"),
            resident_grep_candidate_set_digest: candidates.clone(),
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
            resident_grep_candidate_set_digest: digest("rg"),
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
        LexicalRecallRoute::ResidentGrep,
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
            resident_grep_candidate_set_digest: candidates.clone(),
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
            resident_grep_candidate_set_digest: candidates.clone(),
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
