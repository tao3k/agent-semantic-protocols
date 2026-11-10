// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use agent_semantic_content_identity::SourceSnapshotEvidence;
use agent_semantic_content_identity::SourceSnapshotKind;
use agent_semantic_content_identity::WorkspaceSnapshot;
use agent_semantic_content_identity::hash_blob;

fn rust_provider_manifest() -> &'static [u8] {
    agent_semantic_provider_protocol::builtin_provider_register_json().as_bytes()
}
const FIXTURE_PATH: &str = "src/lib.rs";
const FIXTURE_SOURCE: &[u8] = b"pub fn fixture() -> &'static str { \"source-index\" }\n";

pub struct CanonicalTestSnapshot {
    pub workspace: WorkspaceSnapshot,
    pub evidence: SourceSnapshotEvidence,
    pub provider_digest: String,
    pub generation:
        agent_semantic_content_identity::workspace_generation_evidence::ValidatedWorkspaceGenerationV1,
}

pub fn canonical_test_snapshot() -> CanonicalTestSnapshot {
    let source_digest = hash_blob(FIXTURE_SOURCE).value;
    let provider_digest = hash_blob(rust_provider_manifest()).value;
    let workspace = WorkspaceSnapshot::from_file_hashes([(FIXTURE_PATH, source_digest)]);
    let evidence = workspace.evidence(SourceSnapshotKind::Filesystem, provider_digest.clone());
    let generation =
        agent_semantic_content_identity::workspace_generation_evidence::ValidatedWorkspaceGenerationV1::new(
            agent_semantic_content_identity::workspace_generation_evidence::WorkspaceGenerationEvidenceV1 {
                root_digest: evidence.root_digest.clone(),
                root_depth: 1,
                leaf_count: evidence.leaf_count as u64,
                owner_count: evidence.leaf_count as u64,
            },
        )
        .expect("canonical workspace generation");
    CanonicalTestSnapshot {
        workspace,
        evidence,
        provider_digest,
        generation,
    }
}
