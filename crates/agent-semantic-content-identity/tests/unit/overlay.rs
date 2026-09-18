// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use crate::SourceSnapshotKind;
use crate::WorkspaceSnapshot;
use crate::hash_blob;

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

#[test]
fn canonical_delta_reuses_digest_leaves_and_matches_full_rebuild() {
    let unchanged_digest = hash_blob(b"unchanged").value;
    let replacement_digest = hash_blob(b"replacement").value;
    let added_digest = hash_blob(b"added").value;
    let base = WorkspaceSnapshot::from_file_hashes([
        ("src/unchanged.rs", unchanged_digest.clone()),
        ("src/replaced.rs", hash_blob(b"old").value),
        ("src/removed.rs", hash_blob(b"removed").value),
    ]);

    let successor = base.canonical_with_overlay_delta(
        [
            ("src/replaced.rs", replacement_digest.clone()),
            ("src/added.rs", added_digest.clone()),
        ],
        ["src/removed.rs"],
    );
    let rebuilt = WorkspaceSnapshot::from_file_hashes([
        ("src/added.rs", added_digest),
        ("src/replaced.rs", replacement_digest),
        ("src/unchanged.rs", unchanged_digest),
    ]);

    assert_eq!(successor, rebuilt);
    assert_eq!(successor.file_digests().count(), 3);
    assert!(
        successor
            .evidence(SourceSnapshotKind::DerivedOverlay, "provider")
            .base_root_digest
            .is_none(),
        "canonical successor folds acquisition lineage into a complete leaf proof"
    );
}

#[test]
fn deserialized_snapshot_revalidates_root_instead_of_trusting_process_cache() {
    let snapshot = WorkspaceSnapshot::from_file_hashes([
        ("src/a.rs", hash_blob(b"a").value),
        ("src/b.rs", hash_blob(b"b").value),
    ]);
    let encoded = serde_json::to_value(&snapshot).expect("snapshot serializes");
    let restored: WorkspaceSnapshot =
        serde_json::from_value(encoded.clone()).expect("snapshot deserializes");

    restored
        .validate()
        .expect("an untampered persisted snapshot revalidates");

    let mut tampered = encoded;
    tampered["root_digest"] = serde_json::Value::String("0".repeat(64));
    let tampered: WorkspaceSnapshot =
        serde_json::from_value(tampered).expect("tampered shape still deserializes");

    assert_eq!(
        tampered.validate(),
        Err("workspace snapshot root digest does not match its leaves".to_owned())
    );
}
