use agent_semantic_content_identity::{
    SourceSnapshotEvidence, SourceSnapshotKind, WorkspaceSnapshot, hash_blob,
};

const RUST_PROVIDER_MANIFEST: &[u8] =
    include_bytes!("../../../../languages/rust-lang-project-harness/schemas/asp-provider.json");

pub(crate) struct CanonicalTestSnapshot {
    pub(crate) evidence: SourceSnapshotEvidence,
}

pub(crate) fn canonical_test_snapshot() -> CanonicalTestSnapshot {
    let workspace = WorkspaceSnapshot::from_file_hashes([(
        "src/lib.rs",
        hash_blob(b"pub fn fixture() -> &'static str { \"source-index\" }\n").value,
    )]);
    CanonicalTestSnapshot {
        evidence: workspace.evidence(
            SourceSnapshotKind::Filesystem,
            hash_blob(RUST_PROVIDER_MANIFEST).value,
        ),
    }
}
