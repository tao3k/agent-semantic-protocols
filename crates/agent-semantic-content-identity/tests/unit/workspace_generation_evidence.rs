// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Workspace generation evidence contract tests.

use agent_semantic_content_identity::workspace_generation_evidence::WorkspaceGenerationAuthority;
use agent_semantic_content_identity::workspace_generation_evidence::WorkspaceGenerationEvidenceError;
use agent_semantic_content_identity::workspace_generation_evidence::WorkspaceGenerationEvidenceV1;

fn complete(root: &str) -> WorkspaceGenerationEvidenceV1 {
    WorkspaceGenerationEvidenceV1 {
        root_digest: root.to_owned(),
        root_depth: 1,
        leaf_count: 85,
        owner_count: 84,
    }
}

#[test]
fn graph_authority_fails_closed_without_an_active_generation() {
    assert_eq!(
        WorkspaceGenerationAuthority::unavailable().admit_graph(&complete("root-a")),
        Err(WorkspaceGenerationEvidenceError::GenerationAuthorityMissing)
    );
}

#[test]
fn graph_authority_supports_resident_memory_and_rejects_incomplete_generations() {
    let mut evidence = complete("root-a");
    evidence.root_depth = 0;
    assert_eq!(evidence.validate_complete(), Ok(()));

    evidence = complete("root-a");
    evidence.root_depth = 2;
    assert_eq!(
        evidence.validate_complete(),
        Err(WorkspaceGenerationEvidenceError::UnsupportedRootDepth { root_depth: 2 })
    );

    evidence = complete("root-a");
    evidence.owner_count = 86;
    assert_eq!(
        evidence.validate_complete(),
        Err(WorkspaceGenerationEvidenceError::InvalidOwnerCoverage {
            owner_count: 86,
            leaf_count: 85,
        })
    );
}

#[test]
fn empty_source_scope_is_a_complete_generation() {
    let mut evidence = complete("empty-root");
    evidence.leaf_count = 0;
    evidence.owner_count = 0;
    assert_eq!(evidence.validate_complete(), Ok(()));
}

#[test]
fn graph_authority_requires_the_active_generation_identity() {
    let candidate = complete("root-a");
    let active = complete("root-b");
    assert_eq!(
        candidate.validate_same_generation(&active),
        Err(WorkspaceGenerationEvidenceError::GenerationMismatch {
            candidate_root: "root-a".to_owned(),
            active_root: "root-b".to_owned(),
        })
    );
    assert!(active.validate_same_generation(&active).is_ok());
}
