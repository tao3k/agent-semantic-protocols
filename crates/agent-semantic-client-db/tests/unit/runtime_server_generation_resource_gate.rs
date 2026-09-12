// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

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
    workspace: &str,
    candidate: WorkspaceGenerationCandidateIdentity,
) -> Result<WorkspaceGenerationBuildCompletion, WorkspaceGenerationBuildFailure> {
    WorkspaceGenerationBuildCompletion::new(
        candidate,
        WorkspaceGenerationCommitReceipt {
            projection_capability: crate::fixture::ready_projection_capability_fixture(
                workspace,
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
        move |workspace,
              _root,
              candidate,
              _mode,
              _changed_paths,
              _provider_target,
              _cancellation| {
            build_count.fetch_add(1, Ordering::Relaxed);
            Box::pin(async move { completed_generation(&workspace, candidate) })
        }
    })));

    let warm_root = root.path().join("warm");
    std::fs::create_dir_all(&warm_root).expect("warm project root");
    let warm_workspace = agent_semantic_client_core::state_core::ResolvedState::resolve(&warm_root)
        .expect("resolve warm workspace")
        .workspace
        .workspace_id
        .to_string();
    admission
        .admit(&warm_workspace, warm_root.clone(), candidate_identity('a'))
        .await
        .expect("warm resource sampler and generation builder");
    let warm = admission
        .wait_terminal(&warm_workspace, &warm_root)
        .await
        .expect("warm generation terminal");
    assert_eq!(
        warm.state,
        crate::runtime_server_admission::WorkspaceGenerationAdmissionState::Ready,
        "{warm:?}"
    );
    build_count.store(0, Ordering::Relaxed);

    let project_root = root.path().join("measured");
    std::fs::create_dir_all(&project_root).expect("measured project root");
    let workspace = agent_semantic_client_core::state_core::ResolvedState::resolve(&project_root)
        .expect("resolve measured workspace")
        .workspace
        .workspace_id
        .to_string();
    let before = resource_observation();
    let candidate = candidate_identity('b');
    let mut tasks = Vec::with_capacity(CONCURRENT_ADMISSIONS);
    for _ in 0..CONCURRENT_ADMISSIONS {
        let admission = Arc::clone(&admission);
        let project_root = project_root.clone();
        let candidate = candidate.clone();
        let workspace = workspace.clone();
        tasks.push(tokio::spawn(async move {
            admission
                .admit(&workspace, project_root, candidate)
                .await
                .expect("concurrent generation admission")
        }));
    }
    for task in tasks {
        task.await.expect("generation admission task joins");
    }
    let measured = admission
        .wait_terminal(&workspace, &project_root)
        .await
        .expect("measured generation terminal");
    assert_eq!(
        measured.state,
        crate::runtime_server_admission::WorkspaceGenerationAdmissionState::Ready,
        "{measured:?}"
    );
    assert!(measured.commit.is_some());
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
