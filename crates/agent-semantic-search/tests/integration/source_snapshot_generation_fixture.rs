use agent_semantic_content_identity::workspace_generation_evidence::{
    ValidatedWorkspaceGenerationV1, WorkspaceGenerationEvidenceV1,
};
use agent_semantic_content_identity::{
    SourceSnapshotEvidence, SourceSnapshotKind, WorkspaceSnapshot, hash_blob,
};

const RUST_PROVIDER_MANIFEST: &[u8] =
    include_bytes!("../../../../languages/rust-lang-project-harness/schemas/asp-provider.json");

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
        hash_blob(RUST_PROVIDER_MANIFEST).value,
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
