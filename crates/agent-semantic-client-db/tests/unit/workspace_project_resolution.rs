use std::{
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::{Duration, Instant},
};

use agent_semantic_client_db::workspace_project_resolution::{
    PackageManagerInput, ProjectEntryInput, ProjectResolutionFuture, ProjectResolutionInputs,
    WORKSPACE_PROJECT_RESOLUTION_CONTROL_SCHEMA_ID, WORKSPACE_PROJECT_RESOLUTION_SCHEMA_VERSION,
    WorkspaceProjectResolutionControl, WorkspaceProjectResolutionFailure,
    WorkspaceProjectResolutionFailureKind, WorkspaceProjectResolutionOperation,
    WorkspaceProjectResolutionState, WorkspaceProjectResolver, WorkspaceResolutionGeneration,
    WorkspaceResolutionIdentity, spawn_workspace_project_resolution_actor,
};

struct CountingResolver {
    calls: AtomicUsize,
}

impl CountingResolver {
    fn new() -> Self {
        Self {
            calls: AtomicUsize::new(0),
        }
    }
}

impl WorkspaceProjectResolver for CountingResolver {
    fn resolve(
        &self,
        inputs: ProjectResolutionInputs,
        next_generation_id: u64,
    ) -> ProjectResolutionFuture {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Box::pin(async move {
            if inputs
                .project_entries
                .iter()
                .any(|entry| entry.content_digest == "invalid")
            {
                return Err(WorkspaceProjectResolutionFailure {
                    reason_kind: WorkspaceProjectResolutionFailureKind::ProjectEntryParseFailed,
                    next: "repair the package-manager entry".to_owned(),
                });
            }
            Ok(WorkspaceResolutionGeneration {
                generation_id: next_generation_id,
                candidate_generation: inputs.candidate_generation,
                package_graph_digest: format!("package-graph:{next_generation_id}"),
                resolved_source_scope_digest: format!("scope:{next_generation_id}"),
                project_resolution_artifact: format!(
                    "project-resolution/generation-{next_generation_id}.json"
                ),
                resolved_source_scope_artifact: format!(
                    "resolved-source-scope/generation-{next_generation_id}.json"
                ),
            })
        })
    }
}

fn identity() -> WorkspaceResolutionIdentity {
    WorkspaceResolutionIdentity {
        repository_identity: "repo-1".to_owned(),
        worktree_identity: "worktree-1".to_owned(),
        workspace_root_digest: "workspace-root".to_owned(),
    }
}

fn inputs(candidate_generation: &str, manifest_digest: &str) -> ProjectResolutionInputs {
    ProjectResolutionInputs {
        candidate_generation: candidate_generation.to_owned(),
        project_entries: vec![ProjectEntryInput {
            provider_id: "rust".to_owned(),
            path: "Cargo.toml".to_owned(),
            content_digest: manifest_digest.to_owned(),
        }],
        package_manager_inputs: vec![PackageManagerInput {
            path: "Cargo.lock".to_owned(),
            content_digest: "lock".to_owned(),
        }],
    }
}

#[tokio::test]
async fn many_sessions_share_one_serving_generation() {
    let resolver = Arc::new(CountingResolver::new());
    let daemon = spawn_workspace_project_resolution_actor(identity(), resolver);
    daemon.attach_session("session-a").await;
    daemon.attach_session("session-b").await;

    let published = daemon
        .refresh_inputs(inputs("git-index:1", "manifest:1"))
        .await;
    assert_eq!(
        published.state,
        WorkspaceProjectResolutionState::GenerationServing
    );
    assert_eq!(published.attached_session_count, 2);

    let first = daemon.current_scope("session-a").await;
    let second = daemon.current_scope("session-b").await;
    assert_eq!(first.generation, second.generation);
}

#[tokio::test]
async fn unchanged_package_inputs_reuse_memory_generation() {
    let resolver = Arc::new(CountingResolver::new());
    let daemon = spawn_workspace_project_resolution_actor(identity(), resolver.clone());
    daemon.attach_session("session-a").await;

    let first = daemon
        .refresh_inputs(inputs("git-index:1", "manifest:1"))
        .await;
    let second = daemon
        .refresh_inputs(inputs("git-index:1", "manifest:1"))
        .await;
    assert_eq!(first.generation, second.generation);
    assert_eq!(resolver.calls.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn failed_manifest_delta_never_replaces_last_good_generation() {
    let resolver = Arc::new(CountingResolver::new());
    let daemon = spawn_workspace_project_resolution_actor(identity(), resolver);
    daemon.attach_session("session-a").await;

    let good = daemon
        .refresh_inputs(inputs("git-index:1", "manifest:1"))
        .await;
    let failed = daemon
        .refresh_inputs(inputs("git-index:2", "invalid"))
        .await;
    assert_eq!(failed.state, WorkspaceProjectResolutionState::Failed);
    assert_eq!(
        failed
            .failure
            .expect("typed package parser failure")
            .reason_kind,
        WorkspaceProjectResolutionFailureKind::ProjectEntryParseFailed
    );

    let still_serving = daemon.current_scope("session-a").await;
    assert_eq!(still_serving.generation, good.generation);
}

#[tokio::test]
async fn warm_generation_receipt_is_sub_millisecond_per_request() {
    let resolver = Arc::new(CountingResolver::new());
    let daemon = spawn_workspace_project_resolution_actor(identity(), resolver);
    daemon.attach_session("session-a").await;
    daemon
        .refresh_inputs(inputs("git-index:1", "manifest:1"))
        .await;

    let request_count = 1_000_u32;
    let started = Instant::now();
    for _ in 0..request_count {
        let receipt = daemon.observe_generation().await;
        assert_eq!(
            receipt.state,
            WorkspaceProjectResolutionState::GenerationServing
        );
    }
    let elapsed = started.elapsed();
    let average = elapsed / request_count;
    eprintln!(
        "workspace-project-resolution warm requests={request_count} wall={elapsed:?} average={average:?}"
    );
    assert!(
        average < Duration::from_millis(1),
        "warm workspace daemon receipt must remain sub-millisecond; average={average:?}"
    );
}

#[tokio::test]
async fn typed_control_rejects_cross_workspace_requests() {
    let resolver = Arc::new(CountingResolver::new());
    let daemon = spawn_workspace_project_resolution_actor(identity(), resolver);
    let receipt = daemon
        .execute_control(WorkspaceProjectResolutionControl {
            schema_id: WORKSPACE_PROJECT_RESOLUTION_CONTROL_SCHEMA_ID.to_owned(),
            schema_version: WORKSPACE_PROJECT_RESOLUTION_SCHEMA_VERSION.to_owned(),
            workspace_identity: WorkspaceResolutionIdentity {
                repository_identity: "repo-other".to_owned(),
                ..identity()
            },
            operation: WorkspaceProjectResolutionOperation::ObserveGeneration,
        })
        .await;
    assert_eq!(receipt.state, WorkspaceProjectResolutionState::Failed);
    assert_eq!(
        receipt.failure.expect("identity failure").reason_kind,
        WorkspaceProjectResolutionFailureKind::WorkspaceIdentityMismatch
    );
}

#[test]
fn typed_refresh_control_serializes_to_shared_contract_shape() {
    let value = serde_json::to_value(WorkspaceProjectResolutionControl {
        schema_id: WORKSPACE_PROJECT_RESOLUTION_CONTROL_SCHEMA_ID.to_owned(),
        schema_version: WORKSPACE_PROJECT_RESOLUTION_SCHEMA_VERSION.to_owned(),
        workspace_identity: identity(),
        operation: WorkspaceProjectResolutionOperation::RefreshInputs {
            candidate_generation: "git-index:1".to_owned(),
            project_entries: inputs("git-index:1", "manifest:1").project_entries,
            package_manager_inputs: inputs("git-index:1", "manifest:1").package_manager_inputs,
        },
    })
    .expect("serialize typed workspace project-resolution control");
    assert_eq!(value["operation"]["kind"], "refresh-inputs");
    assert_eq!(
        value["operation"]["candidateGeneration"],
        serde_json::json!("git-index:1")
    );
    assert!(value["operation"].get("defaultSourceRoots").is_none());
    assert!(
        value["operation"]
            .get("defaultIgnoredPathPrefixes")
            .is_none()
    );
}
