use agent_semantic_content_identity::{
    SourceSnapshotEvidence, SourceSnapshotKind, WorkspaceSnapshot, hash_blob,
};

const RUST_PROVIDER_MANIFEST: &[u8] = include_bytes!(
    "../../../../languages/rust-lang-project-harness/provider/asp-provider-manifest.json"
);
const FIXTURE_PATH: &str = "src/lib.rs";
const FIXTURE_SOURCE: &[u8] = b"pub fn fixture() -> &'static str { \"source-index\" }\n";

pub(crate) struct CanonicalTestSnapshot {
    pub(crate) workspace: WorkspaceSnapshot,
    pub(crate) evidence: SourceSnapshotEvidence,
    pub(crate) provider_digest: String,
}

pub(crate) fn canonical_test_snapshot() -> CanonicalTestSnapshot {
    let source_digest = hash_blob(FIXTURE_SOURCE).value;
    let provider_digest = hash_blob(RUST_PROVIDER_MANIFEST).value;
    let workspace = WorkspaceSnapshot::from_file_hashes([(FIXTURE_PATH, source_digest)]);
    let evidence = workspace.evidence(SourceSnapshotKind::Filesystem, provider_digest.clone());
    CanonicalTestSnapshot {
        workspace,
        evidence,
        provider_digest,
    }
}
