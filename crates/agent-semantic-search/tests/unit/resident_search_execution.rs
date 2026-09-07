// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use agent_semantic_search::ContentSearchGenerationReceipt;
use agent_semantic_search::ResidentByteCoverageIndex;
use agent_semantic_search::ResidentByteCoverageInput;
use agent_semantic_search::ResidentSearchAuthority;
use agent_semantic_search::ResidentSearchFusionCapabilities;
use agent_semantic_search::ResidentSearchIntent;
use agent_semantic_search::SearchGenerationConstructionStage;
use agent_semantic_search::SearchGenerationIdentity;
use agent_semantic_search::SearchGenerationStageReceipt;
use agent_semantic_search::plan_resident_search_execution;

fn capabilities() -> ResidentSearchFusionCapabilities {
    ResidentSearchFusionCapabilities {
        lexical: true,
        resident_graph: true,
        python_graph: false,
        byte_evidence: true,
        complete_byte_coverage: true,
    }
}

fn digest(byte: char) -> String {
    format!("blake3-256:{}", byte.to_string().repeat(64))
}

fn content_generation() -> ContentSearchGenerationReceipt {
    let identity = SearchGenerationIdentity {
        project_id: "project-test".to_owned(),
        workspace_id: "workspace-test".to_owned(),
        source_root_digest: digest('1'),
        provider_digest: digest('2'),
        schema_digest: digest('3'),
        generation_candidate_digest: digest('4'),
    };
    let stage = |stage, artifact: char, worker: &str| SearchGenerationStageReceipt {
        stage,
        identity: identity.clone(),
        artifact_digest: digest(artifact),
        worker_id: worker.to_owned(),
        complete: true,
    };
    ContentSearchGenerationReceipt::new(stage(
        SearchGenerationConstructionStage::SourceByteAcquisition,
        '7',
        "rust-source-byte-acquisition-v1",
    ))
    .expect("content generation")
}

#[test]
fn ready_capabilities_are_derived_from_the_published_generation() {
    let published_capabilities =
        ResidentSearchFusionCapabilities::from_open_generation(&content_generation())
            .expect("opened generation capabilities");
    assert_eq!(
        published_capabilities,
        ResidentSearchFusionCapabilities {
            lexical: false,
            resident_graph: false,
            python_graph: false,
            byte_evidence: true,
            complete_byte_coverage: true,
        }
    );
}

#[test]
fn ready_conceptual_search_projects_the_ordered_generation() {
    let plan = plan_resident_search_execution(ResidentSearchIntent::Conceptual, capabilities())
        .expect("conceptual plan");
    assert!(plan.use_tantivy_candidates);
    assert!(!plan.use_cold_rg_candidates);
    assert!(plan.project_resident_graph);
    assert!(!plan.require_python_graph);
    assert!(!plan.verify_rg_bytes);
}

#[test]
fn relationship_intent_requires_the_exact_optional_python_graph_capability() {
    let error = plan_resident_search_execution(ResidentSearchIntent::Relationship, capabilities())
        .expect_err("relationship intent must not invent Python Graph");
    assert_eq!(
        error,
        "query-not-ready: relationship intent requires an exact generation-bound Python Graph capability"
    );
    let plan = plan_resident_search_execution(
        ResidentSearchIntent::Relationship,
        ResidentSearchFusionCapabilities {
            python_graph: true,
            ..capabilities()
        },
    )
    .expect("exact Python Graph capability");
    assert!(plan.require_python_graph);
}

#[test]
fn cold_conceptual_search_uses_rg_while_derived_attachments_build() {
    let plan = plan_resident_search_execution(
        ResidentSearchIntent::Conceptual,
        ResidentSearchFusionCapabilities {
            lexical: false,
            resident_graph: false,
            ..capabilities()
        },
    )
    .expect("content publication admits cold rg without derived attachments");
    assert!(!plan.use_tantivy_candidates);
    assert!(plan.use_cold_rg_candidates);
    assert!(!plan.project_resident_graph);
}

#[test]
fn absence_proof_requires_complete_generation_bound_byte_coverage() {
    let error = plan_resident_search_execution(
        ResidentSearchIntent::AbsenceProof,
        ResidentSearchFusionCapabilities {
            complete_byte_coverage: false,
            ..capabilities()
        },
    )
    .expect_err("absence proof must reject partial coverage");
    assert_eq!(
        error,
        "query-not-ready: absence proof requires complete generation-bound byte coverage"
    );
}

#[test]
fn exact_literal_needs_neither_lexical_nor_graph_attachment() {
    let plan = plan_resident_search_execution(
        ResidentSearchIntent::ExactLiteral,
        ResidentSearchFusionCapabilities {
            lexical: false,
            resident_graph: false,
            ..capabilities()
        },
    )
    .expect("published bytes admit exact-literal search");
    assert!(!plan.use_tantivy_candidates);
    assert!(!plan.use_cold_rg_candidates);
    assert!(!plan.project_resident_graph);
    assert!(plan.verify_rg_bytes);
}

#[test]
fn unsupported_semantic_intent_fails_closed() {
    assert!(ResidentSearchIntent::parse("unsupported-intent").is_err());
}

#[test]
fn resident_byte_coverage_returns_bounded_generation_candidates() {
    let rust = ResidentSearchAuthority {
        language_id: "rust".try_into().expect("language"),
        provider_id: "asp-rust".try_into().expect("provider"),
    };
    let index = ResidentByteCoverageIndex::new([
        ResidentByteCoverageInput {
            owner_path: "src/a.rs".to_owned(),
            authority: Some(rust.clone()),
            bytes: b"publish_workspace_search_generation_v1",
        },
        ResidentByteCoverageInput {
            owner_path: "src/b.rs".to_owned(),
            authority: Some(rust.clone()),
            bytes: b"unrelated owner",
        },
    ]);
    assert_eq!(
        index
            .candidate_owner_paths(b"workspace_search", Some(&rust), 8)
            .expect("candidates"),
        ["src/a.rs"]
    );
    assert!(
        index
            .candidate_owner_paths(b"definitely_absent", Some(&rust), 8)
            .expect("complete miss")
            .is_empty()
    );
}

#[test]
fn resident_byte_coverage_fails_closed_before_unbounded_verification() {
    let index = ResidentByteCoverageIndex::new([
        ResidentByteCoverageInput {
            owner_path: "a".to_owned(),
            authority: None,
            bytes: b"common literal a",
        },
        ResidentByteCoverageInput {
            owner_path: "b".to_owned(),
            authority: None,
            bytes: b"common literal b",
        },
    ]);
    let error = index
        .candidate_owner_paths(b"common", None, 1)
        .expect_err("candidate overflow must fail closed");
    assert!(error.contains("candidate budget exceeded"));
    assert!(index.candidate_owner_paths(b"x", None, 1).is_err());
}
