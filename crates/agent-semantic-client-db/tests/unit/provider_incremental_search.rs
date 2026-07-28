use std::{fs, path::PathBuf};

use crate::engine::{
    ProviderIncrementalOwnerWriteV1, ProviderIncrementalScopeV1, ProviderOwnerDecisionV1,
    ProviderOwnerFingerprintV1, ProviderOwnerMetadataV1, ProviderSelectorProjectionV1,
    probe_provider_owner_v1, write_provider_incremental_owner_v1,
};

#[tokio::test(flavor = "current_thread")]
async fn unchanged_owner_is_metadata_only() {
    let fixture = ProviderIncrementalFixture::new("unchanged");
    let first = fixture
        .write_owner("src/lib.rs", fixture.fingerprint(1, "digest-a"))
        .await;
    let probe = probe_provider_owner_v1(
        fixture.db_path.as_path(),
        &fixture.scope,
        "src/lib.rs",
        &fixture.metadata(1),
    )
    .await
    .expect("probe unchanged owner");
    assert_eq!(probe.decision, ProviderOwnerDecisionV1::Unchanged);
    assert_eq!(probe.generation_before, Some(first.generation_after));
    assert_eq!(probe.content_digest.as_deref(), Some("digest-a"));
}

#[tokio::test(flavor = "current_thread")]
async fn changed_owner_updates_one_leaf_and_ancestors() {
    let fixture = ProviderIncrementalFixture::new("changed");
    let first = fixture
        .write_owner("src/nested/lib.rs", fixture.fingerprint(1, "digest-a"))
        .await;
    let changed_probe = probe_provider_owner_v1(
        fixture.db_path.as_path(),
        &fixture.scope,
        "src/nested/lib.rs",
        &fixture.metadata(2),
    )
    .await
    .expect("probe changed owner");
    assert_eq!(changed_probe.decision, ProviderOwnerDecisionV1::Changed);
    let changed = fixture
        .write_owner("src/nested/lib.rs", fixture.fingerprint(2, "digest-b"))
        .await;
    assert_eq!(changed.owner_index_writes, 1);
    assert_eq!(changed.merkle_leaf_writes, 1);
    assert_eq!(changed.merkle_path_node_writes, 3);
    assert_eq!(
        changed.generation_before.as_deref(),
        Some(first.generation_after.as_str())
    );
    assert_ne!(changed.generation_after, first.generation_after);
}

#[tokio::test(flavor = "current_thread")]
async fn new_owner_adds_one_leaf_without_rewriting_existing_owner() {
    let fixture = ProviderIncrementalFixture::new("new");
    let first = fixture
        .write_owner("src/lib.rs", fixture.fingerprint(1, "digest-a"))
        .await;
    let new_probe = probe_provider_owner_v1(
        fixture.db_path.as_path(),
        &fixture.scope,
        "src/new.rs",
        &fixture.metadata(2),
    )
    .await
    .expect("probe new owner");
    assert_eq!(new_probe.decision, ProviderOwnerDecisionV1::New);
    let added = fixture
        .write_owner("src/new.rs", fixture.fingerprint(2, "digest-b"))
        .await;
    assert_eq!(added.owner_index_writes, 1);
    assert_eq!(added.merkle_leaf_writes, 1);
    assert_eq!(added.merkle_path_node_writes, 2);
    assert_ne!(added.generation_after, first.generation_after);

    let existing_probe = probe_provider_owner_v1(
        fixture.db_path.as_path(),
        &fixture.scope,
        "src/lib.rs",
        &fixture.metadata(1),
    )
    .await
    .expect("probe existing owner after sibling insert");
    assert_eq!(existing_probe.decision, ProviderOwnerDecisionV1::Unchanged);
    assert_eq!(existing_probe.content_digest.as_deref(), Some("digest-a"));
}

struct ProviderIncrementalFixture {
    root: PathBuf,
    db_path: PathBuf,
    scope: ProviderIncrementalScopeV1,
}

impl ProviderIncrementalFixture {
    fn new(label: &str) -> Self {
        let root = temp_root(label);
        let db_path = root.join("facts.turso");
        Self {
            root,
            db_path,
            scope: ProviderIncrementalScopeV1 {
                project_root: "/project".to_string(),
                workspace_identity: "workspace-1".to_string(),
                provider_workspace_identity_digest: "provider-workspace-digest".to_string(),
                language_id: "rust".to_string(),
                provider_id: "rust-harness".to_string(),
                provider_workspace_root: "/project".to_string(),
            },
        }
    }

    fn metadata(&self, revision: i64) -> ProviderOwnerMetadataV1 {
        ProviderOwnerMetadataV1 {
            file_identity: "file-1".to_string(),
            size_bytes: u64::try_from(100 + revision).expect("positive fixture size"),
            modified_unix_nanos: 1_000 + revision,
            change_time_unix_nanos: 2_000 + revision,
        }
    }

    fn fingerprint(&self, revision: i64, content_digest: &str) -> ProviderOwnerFingerprintV1 {
        ProviderOwnerFingerprintV1 {
            metadata: self.metadata(revision),
            content_digest: content_digest.to_string(),
        }
    }

    async fn write_owner(
        &self,
        owner_path: &str,
        fingerprint: ProviderOwnerFingerprintV1,
    ) -> crate::engine::ProviderIncrementalWriteReceiptV1 {
        write_provider_incremental_owner_v1(
            self.db_path.as_path(),
            &ProviderIncrementalOwnerWriteV1 {
                scope: self.scope.clone(),
                owner_path: owner_path.to_string(),
                fingerprint,
                projection_completeness: "complete-owner".to_string(),
                projections: vec![ProviderSelectorProjectionV1 {
                    structural_selector: format!("rust://{owner_path}#item/function/example"),
                    capture_name: "declaration.name".to_string(),
                    signature: "pub fn example()".to_string(),
                    item_kind: "function".to_string(),
                    item_name: "example".to_string(),
                    source_byte_start: 0,
                    source_byte_end: 16,
                }],
            },
        )
        .await
        .expect("write provider incremental owner")
    }
}

impl Drop for ProviderIncrementalFixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn temp_root(label: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!(
        "asp-provider-incremental-{label}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system time before unix epoch")
            .as_nanos()
    ));
    fs::create_dir_all(&root).expect("create provider incremental fixture root");
    root
}
