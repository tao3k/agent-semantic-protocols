use crate::source_snapshot::{
    ResolutionAuthority, ResolutionEvidence, ResolutionState, SOURCE_RESOLUTION_SCHEMA_ID,
    SOURCE_SNAPSHOT_SCHEMA_ID, SourceSnapshotKind, WorkspaceSnapshot,
};

#[test]
fn snapshot_root_is_order_independent() {
    let first = WorkspaceSnapshot::from_file_hashes([
        ("src/lib.rs", "sha256:lib"),
        ("src/main.rs", "sha256:main"),
    ]);
    let second = WorkspaceSnapshot::from_file_hashes([
        ("src/main.rs", "sha256:main"),
        ("src/lib.rs", "sha256:lib"),
    ]);

    assert_eq!(first.root_digest(), second.root_digest());
}

#[test]
fn changed_blob_changes_snapshot_root() {
    let before = WorkspaceSnapshot::from_file_hashes([("src/lib.rs", "sha256:before")]);
    let after = WorkspaceSnapshot::from_file_hashes([("src/lib.rs", "sha256:after")]);

    assert_ne!(before.root_digest(), after.root_digest());
}

#[test]
fn paths_are_normalized_before_lookup() {
    let snapshot = WorkspaceSnapshot::from_file_hashes([("./src\\lib.rs", "sha256:lib")]);

    assert_eq!(snapshot.file_digest("src/lib.rs"), Some("sha256:lib"));
}

#[test]
fn evidence_uses_versioned_schema_ids() {
    let snapshot = WorkspaceSnapshot::from_file_hashes([("src/lib.rs", "sha256:lib")]);
    let evidence = snapshot.evidence(SourceSnapshotKind::Filesystem, "d".repeat(64));
    let resolution = ResolutionEvidence::new(
        snapshot.root_digest(),
        ResolutionAuthority::LiveParser,
        ResolutionState::LiveHit,
    );

    assert_eq!(evidence.schema_id, SOURCE_SNAPSHOT_SCHEMA_ID);
    assert_eq!(evidence.algorithm, "blake3-merkle-v1");
    assert_eq!(evidence.leaf_count, 1);
    assert_eq!(resolution.schema_id, SOURCE_RESOLUTION_SCHEMA_ID);
}
