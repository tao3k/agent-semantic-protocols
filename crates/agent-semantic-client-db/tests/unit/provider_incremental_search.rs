use std::sync::MutexGuard;

use crate::engine::{
    ProviderIncrementalOwnerWrite, ProviderIncrementalScoped, ProviderOwnerBatchProbeRequest,
    ProviderOwnerDecision, ProviderOwnerFingerprint, ProviderOwnerMetadata, ProviderOwnerProbe,
    ProviderSearchWorkspaceSession, ProviderSelectorProjection, WorkspaceDbRegistry,
};
use crate::test_support::{StateHomeGuard, environment_lock, workspace};
use tempfile::TempDir;

#[tokio::test(flavor = "current_thread")]
async fn unchanged_owner_is_metadata_only() {
    let fixture = ProviderIncrementalFixture::new("unchanged").await;
    let first = fixture
        .write_owner("src/lib.rs", fixture.fingerprint(1, "digest-a"))
        .await;
    let probe = fixture.probe("src/lib.rs", fixture.metadata(1)).await;
    assert_eq!(probe.decision, ProviderOwnerDecision::Unchanged);
    assert_eq!(probe.generation_before, Some(first.generation_after));
    assert_eq!(probe.content_digest.as_deref(), Some("digest-a"));
}

#[tokio::test(flavor = "current_thread")]
async fn changed_owner_updates_one_leaf_and_ancestors() {
    let fixture = ProviderIncrementalFixture::new("changed").await;
    let first = fixture
        .write_owner("src/nested/lib.rs", fixture.fingerprint(1, "digest-a"))
        .await;
    let changed_probe = fixture
        .probe("src/nested/lib.rs", fixture.metadata(2))
        .await;
    assert_eq!(changed_probe.decision, ProviderOwnerDecision::Changed);
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
    let fixture = ProviderIncrementalFixture::new("new").await;
    let first = fixture
        .write_owner("src/lib.rs", fixture.fingerprint(1, "digest-a"))
        .await;
    let new_probe = fixture.probe("src/new.rs", fixture.metadata(2)).await;
    assert_eq!(new_probe.decision, ProviderOwnerDecision::New);
    let added = fixture
        .write_owner("src/new.rs", fixture.fingerprint(2, "digest-b"))
        .await;
    assert_eq!(added.owner_index_writes, 1);
    assert_eq!(added.merkle_leaf_writes, 1);
    assert_eq!(added.merkle_path_node_writes, 2);
    assert_ne!(added.generation_after, first.generation_after);

    let existing_probe = fixture.probe("src/lib.rs", fixture.metadata(1)).await;
    assert_eq!(existing_probe.decision, ProviderOwnerDecision::Unchanged);
    assert_eq!(existing_probe.content_digest.as_deref(), Some("digest-a"));
}

struct ProviderIncrementalFixture {
    _environment: MutexGuard<'static, ()>,
    _state_home: StateHomeGuard,
    _temp: TempDir,
    scope: ProviderIncrementalScoped,
    session: ProviderSearchWorkspaceSession,
}

impl ProviderIncrementalFixture {
    async fn new(label: &str) -> Self {
        let environment = environment_lock();
        let temp = TempDir::new().expect("create provider incremental tempfile");
        let state_home = StateHomeGuard::install(&temp.path().join("state"));
        let (project_root, _resolved, mut scope) = workspace(temp.path(), label);
        scope.provider_id = "rust-harness".to_owned();
        let session = WorkspaceDbRegistry::default()
            .acquire(&project_root, &scope)
            .await
            .expect("acquire provider incremental workspace session");
        Self {
            _environment: environment,
            _state_home: state_home,
            _temp: temp,
            scope,
            session,
        }
    }

    fn metadata(&self, revision: i64) -> ProviderOwnerMetadata {
        ProviderOwnerMetadata {
            file_identity: "file-1".to_string(),
            size_bytes: u64::try_from(100 + revision).expect("positive fixture size"),
            modified_unix_nanos: 1_000 + revision,
            change_time_unix_nanos: 2_000 + revision,
        }
    }

    fn fingerprint(&self, revision: i64, content_digest: &str) -> ProviderOwnerFingerprint {
        ProviderOwnerFingerprint {
            metadata: self.metadata(revision),
            content_digest: content_digest.to_string(),
        }
    }

    async fn write_owner(
        &self,
        owner_path: &str,
        fingerprint: ProviderOwnerFingerprint,
    ) -> crate::engine::ProviderIncrementalWriteReceipt {
        self.session
            .write_provider_incremental_owner(&ProviderIncrementalOwnerWrite {
                scope: self.scope.clone(),
                owner_path: owner_path.to_string(),
                fingerprint,
                projection_completeness: "complete-owner".to_string(),
                projections: vec![ProviderSelectorProjection {
                    structural_selector: format!("rust://{owner_path}#item/function/example"),
                    capture_name: "declaration.name".to_string(),
                    signature: "pub fn example()".to_string(),
                    item_kind: "function".to_string(),
                    item_name: "example".to_string(),
                    source_byte_start: 0,
                    source_byte_end: 16,
                }],
            })
            .await
            .expect("write provider incremental owner")
    }

    async fn probe(&self, owner_path: &str, metadata: ProviderOwnerMetadata) -> ProviderOwnerProbe {
        let receipt = self
            .session
            .probe_provider_owners(
                &self.scope,
                &[ProviderOwnerBatchProbeRequest {
                    owner_path: owner_path.to_owned(),
                    metadata,
                }],
            )
            .await
            .expect("probe provider owner through workspace session");
        assert_eq!(receipt.results.len(), 1);
        receipt
            .results
            .into_iter()
            .next()
            .expect("one probe result")
            .probe
    }
}
