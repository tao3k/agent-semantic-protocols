// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use super::SparseProviderOwnerCache;
use crate::runtime_server_workspace::WorkspaceOwnerSnapshot;

fn owner(path: &str, bytes: &[u8]) -> WorkspaceOwnerSnapshot {
    WorkspaceOwnerSnapshot {
        authority: None,
        owner_path: path.to_owned(),
        content_digest: format!("blake3-256:{}", blake3::hash(bytes).to_hex()),
        native_syntax_diagnostic: None,
        bytes: bytes.to_vec(),
        selectors: Vec::new(),
    }
}

#[test]
fn sparse_provider_owner_cache_is_bounded_and_owner_scoped() {
    let root = tempfile::tempdir().expect("create bounded cache root");
    let cache = SparseProviderOwnerCache::new(1);
    let first = owner("src/first.rs", b"fn first() {}");
    let second = owner("src/second.rs", b"fn second() {}");
    cache
        .publish("workspace", root.path(), first.clone())
        .expect("publish first owner");
    cache
        .publish("workspace", root.path(), second.clone())
        .expect("publish second owner");
    assert!(
        cache
            .read_validated(
                "workspace",
                root.path(),
                &first.owner_path,
                &first.content_digest,
            )
            .is_none()
    );
    assert!(
        cache
            .read_validated(
                "workspace",
                root.path(),
                &second.owner_path,
                &second.content_digest,
            )
            .is_some()
    );
}

#[test]
fn changed_owner_digest_evicts_the_stale_sparse_projection() {
    let root = tempfile::tempdir().expect("create stale cache root");
    let cache = SparseProviderOwnerCache::new(4);
    let stale = owner("src/lib.rs", b"fn value() -> u64 { 1 }");
    let fresh = owner("src/lib.rs", b"fn value() -> u64 { 2 }");
    cache
        .publish("workspace", root.path(), stale.clone())
        .expect("publish stale owner");
    assert!(
        cache
            .read_validated(
                "workspace",
                root.path(),
                &stale.owner_path,
                &fresh.content_digest,
            )
            .is_none()
    );
    assert!(
        cache
            .read_validated(
                "workspace",
                root.path(),
                &stale.owner_path,
                &stale.content_digest,
            )
            .is_none()
    );
}

#[test]
fn canonical_scope_promotion_evicts_only_the_superseded_sparse_owners() {
    let first_root = tempfile::tempdir().expect("create first scope root");
    let second_root = tempfile::tempdir().expect("create second scope root");
    let cache = SparseProviderOwnerCache::new(4);
    let first = owner("src/lib.rs", b"fn first() {}");
    let second = owner("src/lib.rs", b"fn second() {}");
    cache
        .publish("workspace-first", first_root.path(), first.clone())
        .expect("publish first scope");
    cache
        .publish("workspace-second", second_root.path(), second.clone())
        .expect("publish second scope");
    cache.evict_scope("workspace-first", first_root.path());
    assert!(
        cache
            .read_validated(
                "workspace-first",
                first_root.path(),
                &first.owner_path,
                &first.content_digest,
            )
            .is_none()
    );
    assert!(
        cache
            .read_validated(
                "workspace-second",
                second_root.path(),
                &second.owner_path,
                &second.content_digest,
            )
            .is_some()
    );
}

#[tokio::test]
async fn fresh_provider_owner_publication_is_readable_without_a_canonical_generation() {
    let root = tempfile::tempdir().expect("create cold-owner registry root");
    std::fs::create_dir_all(root.path().join("src")).expect("create cold-owner source root");
    let bytes = b"pub fn value() -> u64 { 1 }";
    std::fs::write(root.path().join("src/lib.rs"), bytes).expect("write owner source");
    let registry = crate::runtime_server_workspace::RuntimeServerWorkspaceRegistry::new(
        root.path().to_path_buf(),
    )
    .expect("create workspace registry");
    let projected = owner("src/lib.rs", bytes);
    registry
        .publish_provider_owner(
            "cold-owner-request",
            "workspace-cold-owner",
            root.path(),
            projected.clone(),
        )
        .await
        .expect("publish fresh provider owner");
    let read = registry
        .read_projection_owner("workspace-cold-owner", root.path(), &projected.owner_path)
        .await
        .expect("read fresh provider owner");
    assert!(matches!(
        read,
        crate::runtime_server_workspace::WorkspaceRuntimeOwnerRead::SparseProviderOwner {
            owner,
            ..
        } if owner == projected
    ));
}

#[tokio::test]
async fn changed_owner_bytes_cannot_hit_the_runtime_sparse_cache() {
    let root = tempfile::tempdir().expect("create changed-owner registry root");
    std::fs::create_dir_all(root.path().join("src")).expect("create changed-owner source root");
    std::fs::write(
        root.path().join("src/lib.rs"),
        b"pub fn value() -> u64 { 1 }",
    )
    .expect("write initial owner source");
    let registry = crate::runtime_server_workspace::RuntimeServerWorkspaceRegistry::new(
        root.path().to_path_buf(),
    )
    .expect("create workspace registry");
    let projected = owner("src/lib.rs", b"pub fn value() -> u64 { 1 }");
    registry
        .publish_provider_owner(
            "changed-owner-request",
            "workspace-changed-owner",
            root.path(),
            projected.clone(),
        )
        .await
        .expect("publish initial provider owner");
    std::fs::write(
        root.path().join("src/lib.rs"),
        b"pub fn value() -> u64 { 2 }",
    )
    .expect("write changed owner source");
    assert!(matches!(
        registry
            .read_projection_owner(
                "workspace-changed-owner",
                root.path(),
                &projected.owner_path,
            )
            .await
            .expect("read changed sparse owner"),
        crate::runtime_server_workspace::WorkspaceRuntimeOwnerRead::GenerationMissing
    ));
}
