use agent_semantic_content_identity::SourceSnapshotEvidence;
use agent_semantic_content_identity::SourceSnapshotKind;
use agent_semantic_content_identity::WorkspaceSnapshot;
use agent_semantic_content_identity::hash_blob;
use agent_semantic_content_identity::workspace_generation_evidence::ValidatedWorkspaceGenerationV1;
use agent_semantic_content_identity::workspace_generation_evidence::WorkspaceGenerationEvidenceV1;

fn rust_provider_manifest() -> &'static [u8] {
    agent_semantic_provider_protocol::builtin_provider_register_json().as_bytes()
}

pub(crate) struct CanonicalTestSnapshot {
    pub(crate) evidence: SourceSnapshotEvidence,
    pub(crate) generation: ValidatedWorkspaceGenerationV1,
}

pub(crate) fn canonical_test_snapshot() -> CanonicalTestSnapshot {
    let workspace = WorkspaceSnapshot::from_file_hashes([(
        "src/lib.rs",
        hash_blob(b"pub fn fixture() -> &'static str { \"source-index\" }\n").value,
    )]);
    let evidence = workspace.evidence(
        SourceSnapshotKind::Filesystem,
        hash_blob(rust_provider_manifest()).value,
    );
    let generation = ValidatedWorkspaceGenerationV1::new(WorkspaceGenerationEvidenceV1 {
        root_digest: evidence.root_digest.clone(),
        root_depth: 1,
        leaf_count: evidence.leaf_count as u64,
        owner_count: evidence.leaf_count as u64,
    })
    .expect("canonical workspace generation");
    CanonicalTestSnapshot {
        evidence,
        generation,
    }
}
