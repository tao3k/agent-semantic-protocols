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

#[test]
fn content_identity_ignores_snapshot_provenance_but_not_provider_binding() {
    let snapshot = WorkspaceSnapshot::from_file_hashes([("src/lib.rs", "sha256:lib")]);
    let filesystem = snapshot.evidence(SourceSnapshotKind::Filesystem, "provider-a");
    let mut overlay = snapshot.evidence(SourceSnapshotKind::DerivedOverlay, "provider-a");
    overlay.base_root_digest = Some("base-root".to_owned());
    overlay.dirty_paths_digest = Some("dirty-paths".to_owned());

    assert!(filesystem.has_same_content_identity(&overlay));

    overlay.provider_digest = "provider-b".to_owned();
    assert!(!filesystem.has_same_content_identity(&overlay));
}

#[test]
fn chained_live_overlays_fold_into_one_canonical_root_depth() {
    let base =
        WorkspaceSnapshot::from_file_hashes([("src/lib.rs", "lib-v1"), ("src/main.rs", "main-v1")]);
    let first = base.with_overlay([("src/lib.rs", "lib-v2")]);
    let chained = first.with_overlay([("src/main.rs", "main-v2")]);
    let combined = base.with_overlay([("src/lib.rs", "lib-v2"), ("src/main.rs", "main-v2")]);

    assert_eq!(chained, combined);
    let evidence = chained.evidence(
        agent_semantic_content_identity::SourceSnapshotKind::DerivedOverlay,
        "provider-digest",
    );
    assert_eq!(
        evidence.base_root_digest.as_deref(),
        Some(base.root_digest())
    );
    assert!(evidence.dirty_paths_digest.is_some());
}

#[test]
fn an_untracked_path_added_then_removed_collapses_to_the_canonical_snapshot() {
    let base = WorkspaceSnapshot::from_file_hashes([("src/lib.rs", "lib-v1")]);
    let added = base.with_overlay([("src/generated.rs", "generated-v1")]);
    let restored =
        added.with_overlay_delta(std::iter::empty::<(&str, &str)>(), ["src/generated.rs"]);

    assert_eq!(restored, base);
}

#[test]
fn a_modified_path_reverted_to_its_base_digest_clears_overlay_evidence() {
    let base = WorkspaceSnapshot::from_file_hashes([("src/lib.rs", "lib-v1")]);
    let modified = base.with_overlay([("src/lib.rs", "lib-v2")]);
    let restored = modified.with_overlay([("src/lib.rs", "lib-v1")]);

    assert_eq!(restored, base);
}
