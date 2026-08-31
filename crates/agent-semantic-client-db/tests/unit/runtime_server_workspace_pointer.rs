use std::sync::Arc;

use super::{WorkspaceGenerationPointerReader, WorkspaceGenerationPointerWriter};
use crate::runtime_server_workspace::model::WORKSPACE_GENERATION_SCHEMA_ID;
use crate::runtime_server_workspace::{WorkspaceGenerationSnapshot, WorkspaceGenerationState};

fn snapshot(active_epoch: u64) -> WorkspaceGenerationSnapshot {
    let digest = || format!("blake3-256:{active_epoch:064x}");
    let workspace_snapshot =
        agent_semantic_content_identity::WorkspaceSnapshot::from_file_bytes([(
            "src/lib.rs",
            format!("epoch-{active_epoch}"),
        )]);
    let source_root_digest = format!("blake3-256:{}", workspace_snapshot.root_digest());
    WorkspaceGenerationSnapshot {
        schema_id: WORKSPACE_GENERATION_SCHEMA_ID.to_owned(),
        schema_version: "2".to_owned(),
        workspace_identity: "workspace-pointer-stress".to_owned(),
        state: WorkspaceGenerationState::Ready,
        active_epoch,
        generation_digest: digest(),
        root_depth: [1, 0],
        source_kind: agent_semantic_content_identity::SourceSnapshotKind::Filesystem,
        leaf_count: 1,
        owner_count: 1,
        provider_schema_digest: digest(),
        source_root_digest,
        base_root_digest: None,
        source_provider_digest: digest(),
        dirty_paths_digest: None,
        module_graph_digest: digest(),
        selector_set_digest: digest(),
        memory_backend_digest: digest(),
        workspace_source_scope_generation: digest(),
        durable_commit_digest: digest(),
        mmap_segment_path: format!("/fixture/generation-{active_epoch}.mmap"),
        previous_epoch_readable: active_epoch > 1,
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn concurrent_readers_observe_only_complete_generation_pointer_publications() {
    const LAST_EPOCH: u64 = 128;
    const READS_PER_TASK: usize = 2_048;
    let temporary = tempfile::tempdir().expect("create pointer stress fixture");
    let writer = Arc::new(
        WorkspaceGenerationPointerWriter::open(temporary.path())
            .await
            .expect("open pointer writer"),
    );
    writer.publish(&snapshot(1)).await.expect("publish epoch 1");
    let reader = Arc::new(
        WorkspaceGenerationPointerReader::open(writer.path())
            .await
            .expect("open pointer reader"),
    );
    assert_eq!(reader.read().expect("lease epoch 1").active_epoch, 1);

    let reader_tasks = std::thread::available_parallelism()
        .map(usize::from)
        .unwrap_or(1)
        .clamp(2, 32);
    let started = tokio::time::Instant::now();
    let mut tasks = tokio::task::JoinSet::new();
    for _ in 0..reader_tasks {
        let reader = Arc::clone(&reader);
        tasks.spawn(async move {
            let mut last = 1;
            for _ in 0..READS_PER_TASK {
                let observed = reader.read().expect("read complete pointer generation");
                observed
                    .validate()
                    .expect("validate complete pointer generation");
                assert!(observed.active_epoch >= last);
                assert!(observed.active_epoch <= LAST_EPOCH);
                last = observed.active_epoch;
                tokio::task::yield_now().await;
            }
        });
    }
    let publishing = {
        let writer = Arc::clone(&writer);
        tasks.spawn(async move {
            for epoch in 2..=LAST_EPOCH {
                writer
                    .publish(&snapshot(epoch))
                    .await
                    .expect("publish complete pointer generation");
                tokio::task::yield_now().await;
            }
        });
    };
    let _ = publishing;
    while let Some(result) = tasks.join_next().await {
        result.expect("join pointer stress task");
    }
    let elapsed = started.elapsed();
    assert_eq!(
        reader
            .read()
            .expect("read terminal generation")
            .active_epoch,
        LAST_EPOCH
    );
    eprintln!(
        "[runtime-workspace-pointer-concurrency] readerTasks={reader_tasks} readsPerTask={READS_PER_TASK} publications={} elapsedMicros={} partialReads=0",
        LAST_EPOCH - 1,
        elapsed.as_micros()
    );
}
