use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

use super::{RuntimeSchedulerObservation, observe_process_memory};
use crate::runtime_server_admission::{
    WorkspaceGenerationAdmission, WorkspaceGenerationBuildCompletion,
    WorkspaceGenerationBuildFailure, WorkspaceGenerationCandidateIdentity,
    WorkspaceGenerationCommitReceipt, WorkspaceGenerationFailureStage,
};

const GENERATION_ADMISSION_DISK_READ_GATE_BYTES: u64 = 16 * 1024 * 1024;
const GENERATION_ADMISSION_DISK_WRITE_GATE_BYTES: u64 = 4 * 1024 * 1024;
const GENERATION_ADMISSION_RSS_GROWTH_GATE_BYTES: u64 = 32 * 1024 * 1024;

fn candidate_identity(digest_byte: char) -> WorkspaceGenerationCandidateIdentity {
    WorkspaceGenerationCandidateIdentity {
        candidate_generation: agent_semantic_runtime::git::RepositoryCandidateGeneration {
            algorithm: "blake3-worktree-state-v1".to_owned(),
            digest: format!("blake3:{}", digest_byte.to_string().repeat(64)),
            authorities: vec![agent_semantic_runtime::git::RepositoryCandidateAuthority::GitIndex],
        },
        policy_overlay_digest:
            "blake3:dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd".to_owned(),
    }
}

fn completed_generation(
    candidate: WorkspaceGenerationCandidateIdentity,
) -> Result<WorkspaceGenerationBuildCompletion, WorkspaceGenerationBuildFailure> {
    WorkspaceGenerationBuildCompletion::new(
        candidate,
        WorkspaceGenerationCommitReceipt {
            projection_capability: crate::fixture::ready_projection_capability_fixture(
                "workspace-test",
                "blake3-256:1111111111111111111111111111111111111111111111111111111111111111",
                "blake3-256:2222222222222222222222222222222222222222222222222222222222222222",
                1,
            ),
            active_epoch: 1,
            generation_digest:
                "blake3-256:1111111111111111111111111111111111111111111111111111111111111111"
                    .to_owned(),
            source_root_digest:
                "blake3-256:2222222222222222222222222222222222222222222222222222222222222222"
                    .to_owned(),
        },
    )
    .map_err(|error| {
        WorkspaceGenerationBuildFailure::new(
            WorkspaceGenerationFailureStage::CanonicalGenerationPublication,
            error,
        )
    })
}

fn resource_observation() -> super::ProcessMemoryObservation {
    observe_process_memory(
        0,
        RuntimeSchedulerObservation {
            worker_threads: 1,
            alive_tasks: 0,
            global_queue_depth: 0,
        },
    )
    .expect("process resource observation must be available")
}

fn monotonic_delta(before: Option<u64>, after: Option<u64>) -> Option<u64> {
    before
        .zip(after)
        .map(|(before, after)| after.saturating_sub(before))
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
#[tokio::test(flavor = "multi_thread")]
async fn generation_admission_disk_bytes_and_rss_stay_within_gate() {
    const CONCURRENT_ADMISSIONS: usize = 256;
    let root = tempfile::tempdir().expect("generation resource gate root");
    let build_count = Arc::new(AtomicUsize::new(0));
    let admission = Arc::new(WorkspaceGenerationAdmission::new(Arc::new({
        let build_count = Arc::clone(&build_count);
        move |_workspace, _root, candidate, _mode, _changed_paths, _cancellation| {
            build_count.fetch_add(1, Ordering::Relaxed);
            Box::pin(async move { completed_generation(candidate) })
        }
    })));

    let warm_root = root.path().join("warm");
    admission
        .admit(
            "workspace-resource-warm",
            warm_root.clone(),
            candidate_identity('a'),
        )
        .await
        .expect("warm resource sampler and generation builder");
    admission
        .wait_terminal("workspace-resource-warm", &warm_root)
        .await
        .expect("warm generation terminal");
    build_count.store(0, Ordering::Relaxed);

    let before = resource_observation();
    let project_root = root.path().join("measured");
    let candidate = candidate_identity('b');
    let mut tasks = Vec::with_capacity(CONCURRENT_ADMISSIONS);
    for _ in 0..CONCURRENT_ADMISSIONS {
        let admission = Arc::clone(&admission);
        let project_root = project_root.clone();
        let candidate = candidate.clone();
        tasks.push(tokio::spawn(async move {
            admission
                .admit("workspace-resource-measured", project_root, candidate)
                .await
                .expect("concurrent generation admission")
        }));
    }
    for task in tasks {
        task.await.expect("generation admission task joins");
    }
    admission
        .wait_terminal("workspace-resource-measured", &project_root)
        .await
        .expect("measured generation terminal");
    let after = resource_observation();

    assert_eq!(
        build_count.load(Ordering::Relaxed),
        1,
        "256 concurrent admissions must share one generation build"
    );
    if let Some(disk_read_delta) = monotonic_delta(before.disk_read_bytes, after.disk_read_bytes) {
        assert!(
            disk_read_delta <= GENERATION_ADMISSION_DISK_READ_GATE_BYTES,
            "generation admission disk reads exceeded gate: delta={disk_read_delta} gate={GENERATION_ADMISSION_DISK_READ_GATE_BYTES}"
        );
    }
    if let Some(disk_write_delta) = monotonic_delta(before.disk_write_bytes, after.disk_write_bytes)
    {
        assert!(
            disk_write_delta <= GENERATION_ADMISSION_DISK_WRITE_GATE_BYTES,
            "generation admission disk writes exceeded gate: delta={disk_write_delta} gate={GENERATION_ADMISSION_DISK_WRITE_GATE_BYTES}"
        );
    }
    if let Some(rss_growth) = monotonic_delta(before.resident_bytes, after.resident_bytes) {
        assert!(
            rss_growth <= GENERATION_ADMISSION_RSS_GROWTH_GATE_BYTES,
            "generation admission RSS growth exceeded gate: delta={rss_growth} gate={GENERATION_ADMISSION_RSS_GROWTH_GATE_BYTES}"
        );
    }

    admission
        .shutdown()
        .await
        .expect("generation resource gate drains all tasks");
}
