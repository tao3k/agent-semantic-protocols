// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use agent_semantic_search::ContentSearchGenerationReceipt;
use agent_semantic_search::NativeSyntaxDiagnostic;
use agent_semantic_search::NativeSyntaxProjection;
use agent_semantic_search::NativeSyntaxRelation;
use agent_semantic_search::NativeSyntaxSelector;
use agent_semantic_search::SearchGenerationConstructionStage;
use agent_semantic_search::SearchGenerationIdentity;
use agent_semantic_search::SearchGenerationStageReceipt;
use agent_semantic_search::SourceByteOwner;
use agent_semantic_search::build_native_syntax_stage;
use agent_semantic_search::build_native_syntax_stage_with_diagnostics;
use agent_semantic_search::build_source_byte_acquisition_stage;

fn digest(byte: char) -> String {
    format!("blake3-256:{}", byte.to_string().repeat(64))
}

fn identity() -> SearchGenerationIdentity {
    SearchGenerationIdentity {
        project_id: "project-test".to_owned(),
        workspace_id: "workspace-test".to_owned(),
        source_root_digest: digest('a'),
        provider_digest: digest('b'),
        schema_digest: digest('c'),
        generation_candidate_digest: digest('d'),
    }
}

fn stage(
    stage: SearchGenerationConstructionStage,
    worker_id: &str,
    artifact: char,
) -> SearchGenerationStageReceipt {
    SearchGenerationStageReceipt {
        stage,
        identity: identity(),
        artifact_digest: digest(artifact),
        worker_id: worker_id.to_owned(),
        complete: true,
    }
}

fn syntax_stage() -> SearchGenerationStageReceipt {
    stage(
        SearchGenerationConstructionStage::NativeSyntax,
        "provider-native-syntax-playbook-v1",
        '2',
    )
}

#[test]
fn exact_content_pipeline_identity_admits_one_generation() {
    let receipt = ContentSearchGenerationReceipt::new(stage(
        SearchGenerationConstructionStage::SourceByteAcquisition,
        "source-byte-acquisition-v1",
        '1',
    ))
    .expect("exact content generation");
    receipt.validate().expect("validated content generation");
}

#[test]
fn native_syntax_source_drift_is_not_part_of_base_publication() {
    let mut syntax = syntax_stage();
    syntax.identity.source_root_digest = digest('9');
    let base = ContentSearchGenerationReceipt::new(stage(
        SearchGenerationConstructionStage::SourceByteAcquisition,
        "source-byte-acquisition-v1",
        '1',
    ))
    .expect("byte-complete base generation");
    assert_ne!(base.identity(), &syntax.identity);
}

#[test]
fn native_syntax_project_drift_is_rejected_by_attachment_identity() {
    let mut syntax = syntax_stage();
    syntax.identity.project_id = "project-other".to_owned();
    let base = ContentSearchGenerationReceipt::new(stage(
        SearchGenerationConstructionStage::SourceByteAcquisition,
        "source-byte-acquisition-v1",
        '1',
    ))
    .expect("byte-complete base generation");
    assert_ne!(base.identity(), &syntax.identity);
}

#[test]
fn native_syntax_workspace_drift_is_rejected_by_attachment_identity() {
    let mut syntax = syntax_stage();
    syntax.identity.workspace_id = "workspace-other".to_owned();
    let base = ContentSearchGenerationReceipt::new(stage(
        SearchGenerationConstructionStage::SourceByteAcquisition,
        "source-byte-acquisition-v1",
        '1',
    ))
    .expect("byte-complete base generation");
    assert_ne!(base.identity(), &syntax.identity);
}

#[test]
fn incomplete_stage_rejects_fusion_before_publication() {
    let mut acquisition = stage(
        SearchGenerationConstructionStage::SourceByteAcquisition,
        "source-byte-acquisition-v1",
        '1',
    );
    acquisition.complete = false;
    let result = ContentSearchGenerationReceipt::new(acquisition);
    assert!(result.is_err());
}

#[test]
fn rg_compatible_coverage_is_order_independent_and_rejects_duplicates() {
    let a = build_source_byte_acquisition_stage(
        identity(),
        [
            SourceByteOwner {
                owner_path: "src/z.rs",
                content_digest: &digest('2'),
                byte_len: 3,
            },
            SourceByteOwner {
                owner_path: "src/a.rs",
                content_digest: &digest('3'),
                byte_len: 5,
            },
        ],
    )
    .expect("complete byte coverage");
    let b = build_source_byte_acquisition_stage(
        identity(),
        [
            SourceByteOwner {
                owner_path: "src/a.rs",
                content_digest: &digest('3'),
                byte_len: 5,
            },
            SourceByteOwner {
                owner_path: "src/z.rs",
                content_digest: &digest('2'),
                byte_len: 3,
            },
        ],
    )
    .expect("same coverage in another input order");
    assert_eq!(a.artifact_digest, b.artifact_digest);

    assert!(
        build_source_byte_acquisition_stage(
            identity(),
            [
                SourceByteOwner {
                    owner_path: "src/a.rs",
                    content_digest: &digest('3'),
                    byte_len: 5,
                },
                SourceByteOwner {
                    owner_path: "src/a.rs",
                    content_digest: &digest('3'),
                    byte_len: 5,
                },
            ],
        )
        .is_err()
    );
}

#[test]
fn native_syntax_playbook_binds_complete_parser_facts_without_public_owner_command() {
    let owner = |path: &str, selector: &str| NativeSyntaxProjection {
        owner_path: path.to_owned(),
        content_digest: digest('4'),
        selectors: vec![NativeSyntaxSelector {
            selector: selector.to_owned(),
            byte_start: 3,
            byte_end: 17,
            query_keys: vec!["lookup-b".to_owned(), "lookup-a".to_owned()],
            derived_projection_digest: digest('5'),
        }],
    };
    let relation = |path: &str| NativeSyntaxRelation {
        owner_path: path.to_owned(),
        relation_digest: digest('6'),
    };
    let left = build_native_syntax_stage(
        identity(),
        [
            owner("src/z.rs", "item/function/z"),
            owner("src/a.rs", "item/function/a"),
        ],
        [relation("src/z.rs"), relation("src/a.rs")],
    )
    .expect("complete native syntax playbook");
    let right = build_native_syntax_stage(
        identity(),
        [
            owner("src/a.rs", "item/function/a"),
            owner("src/z.rs", "item/function/z"),
        ],
        [relation("src/a.rs"), relation("src/z.rs")],
    )
    .expect("same parser facts in another input order");
    assert_eq!(left.stage, SearchGenerationConstructionStage::NativeSyntax);
    assert_eq!(left.artifact_digest, right.artifact_digest);

    assert!(
        build_native_syntax_stage(
            identity(),
            [
                owner("src/a.rs", "item/function/a"),
                owner("src/a.rs", "item/function/b"),
            ],
            [],
        )
        .is_err()
    );

    let incomplete_selector = NativeSyntaxProjection {
        owner_path: "src/a.rs".to_owned(),
        content_digest: digest('4'),
        selectors: vec![NativeSyntaxSelector {
            selector: "item/function/a".to_owned(),
            byte_start: 3,
            byte_end: 17,
            query_keys: Vec::new(),
            derived_projection_digest: digest('5'),
        }],
    };
    assert!(
        build_native_syntax_stage(identity(), [incomplete_selector], []).is_err(),
        "an owner path without parser query keys is not a complete native-syntax playbook"
    );
    assert!(
        build_native_syntax_stage(
            identity(),
            [owner("src/a.rs", "item/function/a")],
            [relation("src/unknown.rs")],
        )
        .is_err(),
        "relations cannot escape the content-proven native-syntax owner set"
    );
}

#[test]
fn native_syntax_stage_digest_binds_canonical_unavailable_diagnostics() {
    let diagnostic = |path: &str| NativeSyntaxDiagnostic {
        owner_path: path.to_owned(),
        content_digest: digest('7'),
        reason_kind: "source-syntax-unavailable".to_owned(),
        message: "bounded parser diagnostic".to_owned(),
    };
    let left = build_native_syntax_stage_with_diagnostics(
        identity(),
        [],
        [],
        [diagnostic("src/z.rs"), diagnostic("src/a.rs")],
    )
    .expect("typed diagnostics complete their owner-local stage evidence");
    let right = build_native_syntax_stage_with_diagnostics(
        identity(),
        [],
        [],
        [diagnostic("src/a.rs"), diagnostic("src/z.rs")],
    )
    .expect("diagnostic order is canonical");
    assert_eq!(left.artifact_digest, right.artifact_digest);

    assert!(
        build_native_syntax_stage_with_diagnostics(
            identity(),
            [],
            [],
            [NativeSyntaxDiagnostic {
                owner_path: "src/a.rs".to_owned(),
                content_digest: digest('7'),
                reason_kind: "unknown".to_owned(),
                message: "not admitted".to_owned(),
            }],
        )
        .is_err()
    );
}
