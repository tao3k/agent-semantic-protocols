use crate::source_snapshot::{
    ResolutionAuthority, ResolutionEvidence, ResolutionState, SOURCE_SNAPSHOT_ALGORITHM,
    SOURCE_SNAPSHOT_SCHEMA_ID, SnapshotBoundResolution, SourceSnapshotKind, WorkspaceSnapshot,
};

#[test]
fn evidence_binds_the_canonical_merkle_algorithm_and_leaf_count() {
    let snapshot = WorkspaceSnapshot::from_file_hashes([
        ("src/lib.rs", "a".repeat(64)),
        ("src/main.rs", "b".repeat(64)),
    ]);

    let evidence = snapshot.evidence(SourceSnapshotKind::Filesystem, "d".repeat(64));

    assert_eq!(evidence.schema_id, SOURCE_SNAPSHOT_SCHEMA_ID);
    assert_eq!(evidence.algorithm, SOURCE_SNAPSHOT_ALGORITHM);
    assert_eq!(evidence.root_digest, snapshot.root_digest());
    assert_eq!(evidence.leaf_count, 2);
    assert_eq!(evidence.provider_digest, "d".repeat(64));
}

#[test]
fn editor_overlay_is_a_merkle_delta_over_the_base_snapshot() {
    let base = WorkspaceSnapshot::from_file_hashes([
        ("src/lib.rs", "a".repeat(64)),
        ("src/main.rs", "b".repeat(64)),
    ]);
    let dirty_digest = "c".repeat(64);
    let overlay = base.with_overlay([("src/lib.rs", dirty_digest.clone())]);

    let evidence = overlay.evidence(SourceSnapshotKind::EditorBuffer, "d".repeat(64));

    assert_ne!(overlay.root_digest(), base.root_digest());
    assert_eq!(
        overlay.file_digest("src/lib.rs"),
        Some(dirty_digest.as_str())
    );
    assert_eq!(
        evidence.base_root_digest.as_deref(),
        Some(base.root_digest())
    );
    assert!(evidence.dirty_paths_digest.is_some());
    assert_eq!(evidence.leaf_count, 2);
}

#[test]
fn resolution_must_be_bound_to_the_same_snapshot_root() {
    let snapshot = WorkspaceSnapshot::from_file_hashes([("src/lib.rs", "a".repeat(64))]);
    let evidence = snapshot.evidence(SourceSnapshotKind::Filesystem, "d".repeat(64));
    let matching = ResolutionEvidence::new(
        snapshot.root_digest(),
        ResolutionAuthority::LiveParser,
        ResolutionState::LiveHit,
    );

    assert!(SnapshotBoundResolution::new(evidence.clone(), matching).is_ok());

    let stale = ResolutionEvidence::new(
        "f".repeat(64),
        ResolutionAuthority::DerivedIndex,
        ResolutionState::ArtifactCacheHit,
    );
    let error = SnapshotBoundResolution::new(evidence, stale).expect_err("stale root must fail");
    assert!(error.contains("does not match"));
}

#[test]
fn materialized_overlay_evidence_matches_canonical_delta_evidence() {
    let base = WorkspaceSnapshot::from_file_hashes([
        ("src/a.rs", "a".repeat(64)),
        ("src/b.rs", "b".repeat(64)),
    ]);
    let canonical_overlay = base.with_overlay_delta([("src/a.rs", "c".repeat(64))], ["src/b.rs"]);
    let materialized_current = WorkspaceSnapshot::from_file_hashes([("src/a.rs", "c".repeat(64))]);

    let expected = canonical_overlay.evidence(SourceSnapshotKind::Filesystem, "d".repeat(64));
    let actual = materialized_current
        .overlay_evidence(
            SourceSnapshotKind::Filesystem,
            "d".repeat(64),
            base.root_digest(),
            ["src/a.rs"],
            ["src/b.rs"],
        )
        .expect("materialized overlay evidence");

    assert_eq!(actual, expected);
}
