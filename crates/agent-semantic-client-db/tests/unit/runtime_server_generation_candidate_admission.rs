use std::fs;
use std::process::Command;
use std::sync::Arc;

use agent_semantic_client_db::runtime_server_admission::{
    WorkspaceGenerationAdmission, WorkspaceGenerationAdmissionState,
    discover_workspace_generation_candidate,
};
use tokio::sync::{Barrier, Mutex};

use super::{candidate_identity_for, completed_generation};

fn run_git(root: &std::path::Path, args: &[&str]) {
    let output = Command::new("git")
        .args(args)
        .current_dir(root)
        .output()
        .expect("run git fixture command");
    assert!(
        output.status.success(),
        "git fixture command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[tokio::test]
async fn ensure_rebuilds_when_repository_candidate_generation_advances() {
    let builds = Arc::new(Mutex::new(Vec::new()));
    let admission = WorkspaceGenerationAdmission::new(Arc::new({
        let builds = Arc::clone(&builds);
        move |_workspace_identity,
              _project_root,
              candidate,
              _build_mode,
              _changed_paths,
              _cancellation| {
            let builds = Arc::clone(&builds);
            Box::pin(async move {
                builds
                    .lock()
                    .await
                    .push(candidate.candidate_generation.digest.clone());
                completed_generation(candidate)
            })
        }
    }));
    let project_root = std::env::temp_dir().join("asp-generation-candidate-advance");

    admission
        .admit(
            "workspace-candidate-advance",
            project_root.clone(),
            candidate_identity_for(
                "blake3:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            ),
        )
        .await
        .expect("admit initial candidate");
    admission
        .wait_terminal("workspace-candidate-advance", &project_root)
        .await
        .expect("wait for initial candidate");
    let advanced = admission
        .ensure(
            "workspace-candidate-advance",
            &project_root,
            candidate_identity_for(
                "blake3:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
            ),
        )
        .await
        .expect("admit advanced candidate");
    assert_eq!(advanced.state, WorkspaceGenerationAdmissionState::Building);
    assert_eq!(advanced.attempt, 2);
    let ready = admission
        .wait_terminal("workspace-candidate-advance", &project_root)
        .await
        .expect("wait for advanced candidate");
    assert_eq!(
        ready.candidate_generation.digest,
        "blake3:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"
    );
    assert_eq!(
        builds.lock().await.as_slice(),
        [
            "blake3:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            "blake3:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"
        ]
    );
    admission.shutdown().await.expect("drain admission lane");
}

#[tokio::test]
async fn ready_receipt_uses_the_candidate_captured_by_the_builder() {
    let captured = candidate_identity_for(
        "blake3:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
    );
    let admission = WorkspaceGenerationAdmission::new(Arc::new({
        let captured = captured.clone();
        move |_workspace_identity,
              _project_root,
              _requested_candidate,
              _build_mode,
              _changed_paths,
              _cancellation| {
            let captured = captured.clone();
            Box::pin(async move { completed_generation(captured) })
        }
    }));
    let project_root = std::env::temp_dir().join("asp-generation-captured-candidate");

    admission
        .admit(
            "workspace-captured-candidate",
            project_root.clone(),
            candidate_identity_for(
                "blake3:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            ),
        )
        .await
        .expect("admit requested candidate");
    let ready = admission
        .wait_terminal("workspace-captured-candidate", &project_root)
        .await
        .expect("captured candidate reaches Ready");

    assert_eq!(ready.state, WorkspaceGenerationAdmissionState::Ready);
    assert_eq!(ready.candidate_generation, captured.candidate_generation);
    assert_eq!(ready.policy_overlay_digest, captured.policy_overlay_digest);
    admission.shutdown().await.expect("drain admission lane");
}

#[tokio::test]
async fn tracked_source_edit_discovers_and_admits_a_new_generation() {
    let fixture = tempfile::tempdir().expect("candidate admission fixture");
    let project_root = fixture.path();
    run_git(project_root, &["init", "--quiet"]);
    fs::create_dir_all(project_root.join("src")).expect("create source root");
    fs::write(
        project_root.join("src/lib.rs"),
        "pub fn value() -> u8 { 1 }\n",
    )
    .expect("write initial source");
    run_git(project_root, &["add", "src/lib.rs"]);

    let initial = discover_workspace_generation_candidate(project_root)
        .await
        .expect("discover initial candidate");
    let builds = Arc::new(Mutex::new(Vec::new()));
    let admission = WorkspaceGenerationAdmission::new(Arc::new({
        let builds = Arc::clone(&builds);
        move |_workspace_identity,
              _project_root,
              candidate,
              _build_mode,
              _changed_paths,
              _cancellation| {
            let builds = Arc::clone(&builds);
            Box::pin(async move {
                builds
                    .lock()
                    .await
                    .push(candidate.candidate_generation.digest.clone());
                completed_generation(candidate)
            })
        }
    }));

    admission
        .admit(
            "workspace-tracked-source-edit",
            project_root.to_path_buf(),
            initial.clone(),
        )
        .await
        .expect("admit initial tracked candidate");
    admission
        .wait_terminal("workspace-tracked-source-edit", project_root)
        .await
        .expect("wait for initial tracked candidate");

    fs::write(
        project_root.join("src/lib.rs"),
        "pub fn value() -> u8 { 2 }\n",
    )
    .expect("edit tracked source");
    let advanced = discover_workspace_generation_candidate(project_root)
        .await
        .expect("discover advanced candidate");
    assert_ne!(
        initial.candidate_generation.digest,
        advanced.candidate_generation.digest
    );
    let building = admission
        .ensure(
            "workspace-tracked-source-edit",
            project_root,
            advanced.clone(),
        )
        .await
        .expect("admit advanced tracked candidate");
    assert_eq!(building.state, WorkspaceGenerationAdmissionState::Building);
    assert_eq!(building.attempt, 2);
    let ready = admission
        .wait_terminal("workspace-tracked-source-edit", project_root)
        .await
        .expect("wait for advanced tracked candidate");
    assert_eq!(
        ready.candidate_generation.digest,
        advanced.candidate_generation.digest
    );
    assert_eq!(
        builds.lock().await.as_slice(),
        [
            initial.candidate_generation.digest,
            advanced.candidate_generation.digest
        ]
    );
    admission.shutdown().await.expect("drain admission lane");
}

#[tokio::test]
async fn untracked_source_owner_discovers_and_admits_a_new_generation() {
    let fixture = tempfile::tempdir().expect("candidate admission fixture");
    let project_root = fixture.path();
    run_git(project_root, &["init", "--quiet"]);
    fs::create_dir_all(project_root.join("src")).expect("create source root");
    fs::write(
        project_root.join("src/lib.rs"),
        "pub fn value() -> u8 { 1 }\n",
    )
    .expect("write initial source");
    run_git(project_root, &["add", "src/lib.rs"]);

    let initial = discover_workspace_generation_candidate(project_root)
        .await
        .expect("discover initial candidate");
    fs::write(
        project_root.join("src/new_owner.rs"),
        "pub fn newly_materialized_owner() -> u8 { 2 }\n",
    )
    .expect("write untracked source owner");
    let advanced = discover_workspace_generation_candidate(project_root)
        .await
        .expect("discover candidate with untracked owner");

    assert_ne!(
        initial.candidate_generation.digest, advanced.candidate_generation.digest,
        "a live untracked language owner must advance the candidate generation"
    );

    let builds = Arc::new(Mutex::new(Vec::new()));
    let admission = WorkspaceGenerationAdmission::new(Arc::new({
        let builds = Arc::clone(&builds);
        move |_workspace_identity,
              _project_root,
              candidate,
              _build_mode,
              changed_paths,
              _cancellation| {
            let builds = Arc::clone(&builds);
            Box::pin(async move {
                builds.lock().await.push((
                    candidate.candidate_generation.digest.clone(),
                    (*changed_paths).clone(),
                ));
                completed_generation(candidate)
            })
        }
    }));
    admission
        .admit(
            "workspace-untracked-source-owner",
            project_root.to_path_buf(),
            advanced.clone(),
        )
        .await
        .expect("admit candidate containing untracked owner");
    let ready = admission
        .wait_terminal("workspace-untracked-source-owner", project_root)
        .await
        .expect("wait for untracked owner candidate");
    assert_eq!(
        ready.candidate_generation.digest,
        advanced.candidate_generation.digest
    );
    assert_eq!(builds.lock().await.len(), 1);
    admission.shutdown().await.expect("drain admission lane");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn ensure_coalesces_an_advanced_candidate_behind_an_inflight_build() {
    let builds = Arc::new(Mutex::new(Vec::new()));
    let first_release = Arc::new(Barrier::new(2));
    let admission = WorkspaceGenerationAdmission::new(Arc::new({
        let builds = Arc::clone(&builds);
        let first_release = Arc::clone(&first_release);
        move |_workspace_identity,
              _project_root,
              candidate,
              _build_mode,
              _changed_paths,
              _cancellation| {
            let builds = Arc::clone(&builds);
            let first_release = Arc::clone(&first_release);
            Box::pin(async move {
                let digest = candidate.candidate_generation.digest.clone();
                let first = {
                    let mut builds = builds.lock().await;
                    let first = builds.is_empty();
                    builds.push(digest);
                    first
                };
                if first {
                    first_release.wait().await;
                }
                completed_generation(candidate)
            })
        }
    }));
    let project_root = std::env::temp_dir().join("asp-generation-candidate-coalesce");

    admission
        .admit(
            "workspace-candidate-coalesce",
            project_root.clone(),
            candidate_identity_for(
                "blake3:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            ),
        )
        .await
        .expect("admit initial candidate");
    let queued = admission
        .ensure(
            "workspace-candidate-coalesce",
            &project_root,
            candidate_identity_for(
                "blake3:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
            ),
        )
        .await
        .expect("queue advanced candidate");
    assert_eq!(queued.state, WorkspaceGenerationAdmissionState::Building);
    assert_eq!(queued.attempt, 2);
    first_release.wait().await;
    let ready = admission
        .wait_terminal("workspace-candidate-coalesce", &project_root)
        .await
        .expect("wait for coalesced candidate");
    assert_eq!(ready.attempt, 2);
    assert_eq!(
        ready.candidate_generation.digest,
        "blake3:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"
    );
    assert_eq!(
        builds.lock().await.as_slice(),
        [
            "blake3:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            "blake3:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"
        ]
    );
    admission.shutdown().await.expect("drain admission lane");
}
