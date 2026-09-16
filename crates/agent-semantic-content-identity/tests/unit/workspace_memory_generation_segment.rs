// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use agent_semantic_content_identity::workspace_memory_generation_segment::WORKSPACE_MEMORY_GENERATION_SEGMENT_SCHEMA_ID;
use agent_semantic_content_identity::workspace_memory_generation_segment::WorkspaceMemoryGenerationDirectoryV1;
use agent_semantic_content_identity::workspace_memory_generation_segment::WorkspaceMemoryGenerationSectionKindV1;
use agent_semantic_content_identity::workspace_memory_generation_segment::WorkspaceMemoryGenerationSectionRepresentationV1;
use agent_semantic_content_identity::workspace_memory_generation_segment::WorkspaceMemoryGenerationSectionV1;

fn digest() -> String {
    format!("blake3-256:{}", "0".repeat(64))
}

fn directory() -> WorkspaceMemoryGenerationDirectoryV1 {
    let kinds = [
        WorkspaceMemoryGenerationSectionKindV1::GenerationEvidence,
        WorkspaceMemoryGenerationSectionKindV1::ProjectResolutions,
        WorkspaceMemoryGenerationSectionKindV1::OwnerDirectory,
        WorkspaceMemoryGenerationSectionKindV1::OwnerBytes,
        WorkspaceMemoryGenerationSectionKindV1::LexicalIndex,
        WorkspaceMemoryGenerationSectionKindV1::SelectorIndex,
        WorkspaceMemoryGenerationSectionKindV1::GraphRelations,
    ];
    WorkspaceMemoryGenerationDirectoryV1 {
        schema_id: WORKSPACE_MEMORY_GENERATION_SEGMENT_SCHEMA_ID.to_owned(),
        schema_version: "1".to_owned(),
        workspace_identity: "workspace-fixture".to_owned(),
        generation_digest: digest(),
        active_epoch: 1,
        root_depth: [1, 0],
        byte_length: 700,
        sections: kinds
            .into_iter()
            .enumerate()
            .map(|(position, kind)| WorkspaceMemoryGenerationSectionV1 {
                kind,
                offset: (position * 100) as u64,
                length: 100,
                record_count: 1,
                representation: WorkspaceMemoryGenerationSectionRepresentationV1::SortedOffsetTable,
                digest: digest(),
            })
            .collect(),
    }
}

#[test]
fn complete_ordered_v1_directory_is_valid() {
    directory().validate().expect("valid v1 directory");
}

#[test]
fn duplicate_or_overlapping_sections_are_rejected() {
    let mut duplicate = directory();
    duplicate.sections[1].kind = duplicate.sections[0].kind;
    assert!(duplicate.validate().is_err());

    let mut overlap = directory();
    overlap.sections[1].offset = 99;
    assert!(overlap.validate().is_err());
}

#[test]
fn out_of_bounds_or_unqualified_sections_are_rejected() {
    let mut out_of_bounds = directory();
    out_of_bounds.sections[6].length = 101;
    assert!(out_of_bounds.validate().is_err());

    let mut unqualified = directory();
    unqualified.sections[0].digest = "missing-algorithm".to_owned();
    assert!(unqualified.validate().is_err());
}
