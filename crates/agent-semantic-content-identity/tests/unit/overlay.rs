use crate::{SourceSnapshotKind, WorkspaceSnapshot, hash_blob};

#[test]
fn overlay_delta_upserts_and_deletes_snapshot_leaves() {
    let first_digest = hash_blob(b"struct First;").value;
    let removed_digest = hash_blob(b"struct Removed;").value;
    let replacement_digest = hash_blob(b"struct FirstChanged;").value;
    let base = WorkspaceSnapshot::from_file_hashes([
        ("src/first.rs", first_digest),
        ("src/removed.rs", removed_digest),
    ]);
    let base_root = base.root_digest().to_string();

    let derived = base.with_overlay_delta(
        [("src/first.rs", replacement_digest.clone())],
        ["src/removed.rs"],
    );
    let evidence = derived.evidence(SourceSnapshotKind::EditorBuffer, "provider-digest");

    assert_eq!(
        derived.file_digest("src/first.rs"),
        Some(replacement_digest.as_str())
    );
    assert!(!derived.contains_path("src/removed.rs"));
    assert_ne!(derived.root_digest(), base_root);
    assert_eq!(
        evidence.base_root_digest.as_deref(),
        Some(base_root.as_str())
    );
    assert!(evidence.dirty_paths_digest.is_some());
}

#[test]
fn overlay_delta_identity_is_order_independent() {
    let base = WorkspaceSnapshot::from_file_hashes([
        ("src/a.rs", hash_blob(b"a").value),
        ("src/b.rs", hash_blob(b"b").value),
    ]);
    let left = base.with_overlay_delta(
        [
            ("src/a.rs", hash_blob(b"a2").value),
            ("src/c.rs", hash_blob(b"c").value),
        ],
        ["src/b.rs"],
    );
    let right = base.with_overlay_delta(
        [
            ("src/c.rs", hash_blob(b"c").value),
            ("src/a.rs", hash_blob(b"a2").value),
        ],
        ["src/b.rs"],
    );

    assert_eq!(left.root_digest(), right.root_digest());
    assert_eq!(
        left.evidence(SourceSnapshotKind::EditorBuffer, "provider")
            .dirty_paths_digest,
        right
            .evidence(SourceSnapshotKind::EditorBuffer, "provider")
            .dirty_paths_digest
    );
}
