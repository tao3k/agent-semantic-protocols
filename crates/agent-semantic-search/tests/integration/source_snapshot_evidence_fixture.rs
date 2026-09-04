use agent_semantic_content_identity::SourceSnapshotEvidence;
use agent_semantic_content_identity::SourceSnapshotKind;
use agent_semantic_content_identity::WorkspaceSnapshot;
use agent_semantic_content_identity::hash_blob;

fn rust_provider_manifest() -> &'static [u8] {
    agent_semantic_provider_protocol::builtin_provider_register_json().as_bytes()
}

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
            hash_blob(rust_provider_manifest()).value,
        ),
    }
}
