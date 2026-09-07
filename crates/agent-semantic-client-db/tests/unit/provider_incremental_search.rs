// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use std::{sync::MutexGuard, time::Instant};

use crate::engine::{
    ProviderIncrementalOwnerWrite, ProviderIncrementalScoped, ProviderOwnerBatchProbeRequest,
    ProviderOwnerDecision, ProviderOwnerFingerprint, ProviderOwnerMetadata, ProviderOwnerProbe,
    ProviderSearchWorkspaceSession, ProviderSelectorProjection, WorkspaceDbRegistry,
};
use crate::test_support::{StateHomeGuard, TestDir, environment_lock, workspace};

#[tokio::test(flavor = "current_thread")]
async fn unchanged_owner_is_metadata_only() {
    let fixture = ProviderIncrementalFixture::new("unchanged").await;
    let first = fixture.write_owner("src/lib.rs", 1).await;
    let probe = fixture.probe("src/lib.rs", fixture.metadata(1)).await;
    assert_eq!(probe.decision, ProviderOwnerDecision::Unchanged);
    assert_eq!(probe.generation_before, Some(first.generation_after));
    assert_eq!(
        probe.content_digest,
        Some(fixture.fingerprint(1).content_digest)
    );
}

#[tokio::test(flavor = "current_thread")]
async fn changed_owner_updates_one_leaf_and_ancestors() {
    let fixture = ProviderIncrementalFixture::new("changed").await;
    let first = fixture.write_owner("src/nested/lib.rs", 1).await;
    let changed_probe = fixture
        .probe("src/nested/lib.rs", fixture.metadata(2))
        .await;
    assert_eq!(changed_probe.decision, ProviderOwnerDecision::Changed);
    let changed = fixture.write_owner("src/nested/lib.rs", 2).await;
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
    let first = fixture.write_owner("src/lib.rs", 1).await;
    let new_probe = fixture.probe("src/new.rs", fixture.metadata(2)).await;
    assert_eq!(new_probe.decision, ProviderOwnerDecision::New);
    let added = fixture.write_owner("src/new.rs", 2).await;
    assert_eq!(added.owner_index_writes, 1);
    assert_eq!(added.merkle_leaf_writes, 1);
    assert_eq!(added.merkle_path_node_writes, 2);
    assert_ne!(added.generation_after, first.generation_after);

    let existing_probe = fixture.probe("src/lib.rs", fixture.metadata(1)).await;
    assert_eq!(existing_probe.decision, ProviderOwnerDecision::Unchanged);
    assert_eq!(
        existing_probe.content_digest,
        Some(fixture.fingerprint(1).content_digest)
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn resident_provider_owner_reads_are_parallel_and_sub_millisecond_at_p99() {
    const TASK_COUNT: usize = 32;
    const READS_PER_TASK: usize = 32;
    const P99_BUDGET_NANOS: u128 = 1_000_000;

    let fixture = ProviderIncrementalFixture::new("resident-cache").await;
    fixture.write_owner("src/lib.rs", 1).await;
    let mut tasks = tokio::task::JoinSet::new();
    for _ in 0..TASK_COUNT {
        let session = fixture.session.clone();
        let scope = fixture.scope.clone();
        let metadata = fixture.metadata(1);
        tasks.spawn(async move {
            let mut samples = Vec::with_capacity(READS_PER_TASK);
            for _ in 0..READS_PER_TASK {
                let started = Instant::now();
                let probe = session
                    .probe_provider_owners(
                        &scope,
                        &[ProviderOwnerBatchProbeRequest {
                            owner_path: "src/lib.rs".to_owned(),
                            metadata: metadata.clone(),
                        }],
                    )
                    .await
                    .expect("probe resident provider owner");
                assert_eq!(probe.scope_scan_count, 0);
                assert_eq!(
                    probe.results[0].probe.decision,
                    ProviderOwnerDecision::Unchanged
                );
                let snapshot = session
                    .read_provider_owner_snapshot(&scope, "src/lib.rs")
                    .await
                    .expect("read resident provider owner")
                    .expect("resident provider owner exists");
                assert_eq!(snapshot.fingerprint.metadata, metadata);
                samples.push(started.elapsed().as_nanos());
            }
            samples
        });
    }
    let mut samples = Vec::with_capacity(TASK_COUNT * READS_PER_TASK);
    while let Some(result) = tasks.join_next().await {
        samples.extend(result.expect("resident provider owner reader task"));
    }
    samples.sort_unstable();
    let p99 = samples[(samples.len() * 99).div_ceil(100) - 1];
    let max = *samples.last().expect("at least one resident cache sample");
    eprintln!(
        "[provider-owner-resident-performance] tasks={TASK_COUNT} readsPerTask={READS_PER_TASK} samples={} p99Nanos={p99} maxNanos={max} budgetNanos={P99_BUDGET_NANOS}",
        samples.len()
    );
    assert!(
        p99 < P99_BUDGET_NANOS,
        "resident provider owner read p99 exceeded one millisecond: p99Nanos={p99}"
    );
}

struct ProviderIncrementalFixture {
    _state_home: StateHomeGuard,
    _temp: TestDir,
    _environment: MutexGuard<'static, ()>,
    scope: ProviderIncrementalScoped,
    session: ProviderSearchWorkspaceSession,
}

impl ProviderIncrementalFixture {
    async fn new(label: &str) -> Self {
        let environment = environment_lock();
        let temp = TestDir::new(label);
        let state_home = StateHomeGuard::install(&temp.path().join("state"));
        let (project_root, _resolved, mut scope) = workspace(temp.path(), label);
        scope.provider_id = "asp-rust".to_owned();
        let session = WorkspaceDbRegistry::default()
            .acquire(&project_root, &scope)
            .await
            .expect("acquire provider incremental workspace session");
        Self {
            _state_home: state_home,
            _temp: temp,
            _environment: environment,
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

    fn source_bytes(&self, revision: i64) -> Vec<u8> {
        vec![
            u8::try_from(revision).expect("fixture revision fits in one byte");
            usize::try_from(self.metadata(revision).size_bytes)
                .expect("fixture size fits in memory")
        ]
    }

    fn fingerprint(&self, revision: i64) -> ProviderOwnerFingerprint {
        let source_bytes = self.source_bytes(revision);
        ProviderOwnerFingerprint {
            metadata: self.metadata(revision),
            content_digest: agent_semantic_content_identity::ArtifactHash::blake3(
                source_bytes.as_slice(),
            )
            .value,
        }
    }

    async fn write_owner(
        &self,
        owner_path: &str,
        revision: i64,
    ) -> crate::engine::ProviderIncrementalWriteReceipt {
        let source_bytes = self.source_bytes(revision);
        self.session
            .write_provider_incremental_owner(&ProviderIncrementalOwnerWrite {
                scope: self.scope.clone(),
                owner_path: owner_path.to_string(),
                fingerprint: self.fingerprint(revision),
                source_bytes,
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
