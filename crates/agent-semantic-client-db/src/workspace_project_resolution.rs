// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-only

//! Runtime-owned, generation-pinned workspace project-resolution actor.

use std::{
    collections::HashSet,
    future::Future,
    pin::Pin,
    sync::{Arc, Mutex},
};

use serde::{Deserialize, Serialize};
use tokio::sync::{mpsc, oneshot};

/// Schema identity for project-resolution control requests.
pub const WORKSPACE_PROJECT_RESOLUTION_CONTROL_SCHEMA_ID: &str =
    "agent.semantic-protocols.workspace-project-resolution-control";
/// Schema identity for project-resolution terminal receipts.
pub const WORKSPACE_PROJECT_RESOLUTION_RECEIPT_SCHEMA_ID: &str =
    "agent.semantic-protocols.workspace-project-resolution-receipt";
/// Version carried inside project-resolution schemas and receipts.
pub const WORKSPACE_PROJECT_RESOLUTION_SCHEMA_VERSION: &str = "1";

/// Stable repository, worktree, and root identity for one workspace actor.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceResolutionIdentity {
    pub repository_identity: String,
    pub worktree_identity: String,
    pub workspace_root_digest: String,
}

/// Provider-owned project entry that participates in scope resolution.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectEntryInput {
    pub provider_id: String,
    pub path: String,
    pub content_digest: String,
}

/// Package-manager input that participates in dependency resolution.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PackageManagerInput {
    pub path: String,
    pub content_digest: String,
}

/// Immutable inputs for one project-resolution generation.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectResolutionInputs {
    pub candidate_generation: String,
    pub project_entries: Vec<ProjectEntryInput>,
    pub package_manager_inputs: Vec<PackageManagerInput>,
}

/// Published package graph and source-scope generation.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceResolutionGeneration {
    pub generation_id: u64,
    pub candidate_generation: String,
    pub package_graph_digest: String,
    pub resolved_source_scope_digest: String,
    pub project_resolution_artifact: String,
    pub resolved_source_scope_artifact: String,
}

/// Explicit lifecycle state of the workspace project-resolution actor.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum WorkspaceProjectResolutionState {
    WorkspaceAttached,
    CandidateGenerationReady,
    ProjectEntryDeltaClassified,
    PackageResolverRunning,
    PackageGraphReady,
    ResolvedSourceScopeReady,
    WorkspaceArtifactPublished,
    GenerationServing,
    GenerationRetired,
    Failed,
}

/// Typed terminal reason for project-resolution failure.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum WorkspaceProjectResolutionFailureKind {
    WorkspaceIdentityMismatch,
    SessionNotAttached,
    CandidateGenerationStale,
    ProjectEntryRequired,
    ProjectEntryParseFailed,
    PackageGraphUnavailable,
    ResolvedSourceScopeUnavailable,
    ArtifactPublicationFailed,
    DaemonUnavailable,
}

/// Failure evidence and safe next action emitted by project resolution.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceProjectResolutionFailure {
    pub reason_kind: WorkspaceProjectResolutionFailureKind,
    pub next: String,
}

/// Terminal observation of one workspace project-resolution operation.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceProjectResolutionReceipt {
    pub schema_id: String,
    pub schema_version: String,
    pub workspace_identity: WorkspaceResolutionIdentity,
    pub state: WorkspaceProjectResolutionState,
    pub attached_session_count: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub generation: Option<WorkspaceResolutionGeneration>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub failure: Option<WorkspaceProjectResolutionFailure>,
}

/// Typed control envelope admitted by the workspace actor.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceProjectResolutionControl {
    pub schema_id: String,
    pub schema_version: String,
    pub workspace_identity: WorkspaceResolutionIdentity,
    pub operation: WorkspaceProjectResolutionOperation,
}

/// Closed operation set accepted by workspace project resolution.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(
    tag = "kind",
    rename_all = "kebab-case",
    rename_all_fields = "camelCase"
)]
pub enum WorkspaceProjectResolutionOperation {
    AttachSession {
        session_identity: String,
    },
    DetachSession {
        session_identity: String,
    },
    RefreshInputs {
        candidate_generation: String,
        project_entries: Vec<ProjectEntryInput>,
        package_manager_inputs: Vec<PackageManagerInput>,
    },
    GetCurrentScope {
        session_identity: String,
    },
    ObserveGeneration,
}

/// Owned asynchronous resolution result returned by a provider resolver.
pub type ProjectResolutionFuture = Pin<
    Box<
        dyn Future<
                Output = Result<WorkspaceResolutionGeneration, WorkspaceProjectResolutionFailure>,
            > + Send,
    >,
>;

/// Provider-neutral resolver used by the Runtime-owned project-resolution actor.
pub trait WorkspaceProjectResolver: Send + Sync + 'static {
    /// Resolve one immutable input set into the next generation.
    fn resolve(
        &self,
        inputs: ProjectResolutionInputs,
        next_generation_id: u64,
    ) -> ProjectResolutionFuture;
}

#[derive(Clone)]
/// Bounded request handle for one Runtime-owned workspace resolution actor.
pub struct WorkspaceProjectResolutionHandle {
    sender: mpsc::Sender<ActorRequest>,
    workspace_identity: WorkspaceResolutionIdentity,
    lifecycle: Arc<WorkspaceProjectResolutionLifecycle>,
}

struct WorkspaceProjectResolutionLifecycle {
    task: Mutex<Option<crate::runtime_server_runtime::RuntimeServerOwnedTask<()>>>,
    task_scope: crate::runtime_server_runtime::RuntimeServerTaskScope,
}

impl Drop for WorkspaceProjectResolutionLifecycle {
    fn drop(&mut self) {
        if let Ok(task) = self.task.get_mut()
            && let Some(task) = task.take()
        {
            task.abort();
        }
    }
}

impl WorkspaceProjectResolutionHandle {
    /// Submit one typed control operation to the owned actor.
    pub async fn execute_control(
        &self,
        control: WorkspaceProjectResolutionControl,
    ) -> WorkspaceProjectResolutionReceipt {
        if control.schema_id != WORKSPACE_PROJECT_RESOLUTION_CONTROL_SCHEMA_ID
            || control.schema_version != WORKSPACE_PROJECT_RESOLUTION_SCHEMA_VERSION
            || control.workspace_identity != self.workspace_identity
        {
            return WorkspaceProjectResolutionReceipt {
                schema_id: WORKSPACE_PROJECT_RESOLUTION_RECEIPT_SCHEMA_ID.to_owned(),
                schema_version: WORKSPACE_PROJECT_RESOLUTION_SCHEMA_VERSION.to_owned(),
                workspace_identity: self.workspace_identity.clone(),
                state: WorkspaceProjectResolutionState::Failed,
                attached_session_count: 0,
                generation: None,
                failure: Some(WorkspaceProjectResolutionFailure {
                    reason_kind: WorkspaceProjectResolutionFailureKind::WorkspaceIdentityMismatch,
                    next: "resolve and reconnect to the canonical workspace resident endpoint"
                        .to_owned(),
                }),
            };
        }
        match control.operation {
            WorkspaceProjectResolutionOperation::AttachSession { session_identity } => {
                self.attach_session(session_identity).await
            }
            WorkspaceProjectResolutionOperation::DetachSession { session_identity } => {
                self.detach_session(session_identity).await
            }
            WorkspaceProjectResolutionOperation::RefreshInputs {
                candidate_generation,
                project_entries,
                package_manager_inputs,
            } => {
                self.refresh_inputs(ProjectResolutionInputs {
                    candidate_generation,
                    project_entries,
                    package_manager_inputs,
                })
                .await
            }
            WorkspaceProjectResolutionOperation::GetCurrentScope { session_identity } => {
                self.current_scope(session_identity).await
            }
            WorkspaceProjectResolutionOperation::ObserveGeneration => {
                self.observe_generation().await
            }
        }
    }

    /// Attach a session to the shared workspace generation.
    pub async fn attach_session(
        &self,
        session_identity: impl Into<String>,
    ) -> WorkspaceProjectResolutionReceipt {
        self.request(ActorOperation::AttachSession(session_identity.into()))
            .await
    }

    /// Detach a session without retiring generations used by other sessions.
    pub async fn detach_session(
        &self,
        session_identity: impl Into<String>,
    ) -> WorkspaceProjectResolutionReceipt {
        self.request(ActorOperation::DetachSession(session_identity.into()))
            .await
    }

    /// Publish changed project inputs and resolve only when identity changed.
    pub async fn refresh_inputs(
        &self,
        inputs: ProjectResolutionInputs,
    ) -> WorkspaceProjectResolutionReceipt {
        self.request(ActorOperation::RefreshInputs(inputs)).await
    }

    /// Read the currently served source scope for an attached session.
    pub async fn current_scope(
        &self,
        session_identity: impl Into<String>,
    ) -> WorkspaceProjectResolutionReceipt {
        self.request(ActorOperation::GetCurrentScope(session_identity.into()))
            .await
    }

    /// Observe the current generation without changing actor state.
    pub async fn observe_generation(&self) -> WorkspaceProjectResolutionReceipt {
        self.request(ActorOperation::ObserveGeneration).await
    }

    /// Stop and join the actor, then prove its Runtime task scope drained.
    pub async fn shutdown(&self) -> Result<(), String> {
        let _ = self.request(ActorOperation::Shutdown).await;
        self.lifecycle.task_scope.begin_drain();
        let task = self
            .lifecycle
            .task
            .lock()
            .map_err(|_| "workspace project-resolution lifecycle lock poisoned".to_owned())?
            .take();
        if let Some(task) = task {
            task.join().await?;
        }
        self.lifecycle.task_scope.finish(0)?;
        Ok(())
    }

    async fn request(&self, operation: ActorOperation) -> WorkspaceProjectResolutionReceipt {
        let (reply, receiver) = oneshot::channel();
        if self
            .sender
            .send(ActorRequest { operation, reply })
            .await
            .is_err()
        {
            return unavailable_receipt(&self.workspace_identity);
        }
        receiver
            .await
            .unwrap_or_else(|_| unavailable_receipt(&self.workspace_identity))
    }
}

/// Admit one workspace project-resolution actor into an owned Runtime task scope.
pub fn spawn_workspace_project_resolution_actor(
    workspace_identity: WorkspaceResolutionIdentity,
    resolver: Arc<dyn WorkspaceProjectResolver>,
) -> Result<WorkspaceProjectResolutionHandle, String> {
    let queue_capacity = crate::runtime_concurrency::RuntimeConcurrencyPlan::current()
        .writer_queue_capacity()
        .clamp(16, 256);
    let (sender, receiver) = mpsc::channel(queue_capacity);
    let task_scope =
        crate::runtime_server_runtime::RuntimeServerTaskScope::new("workspace-project-resolution");
    let task = task_scope.spawn(
        "workspace-project-resolution-actor",
        run_actor(
            WorkspaceProjectResolutionActor::new(workspace_identity.clone(), resolver),
            receiver,
        ),
    )?;
    Ok(WorkspaceProjectResolutionHandle {
        sender,
        workspace_identity,
        lifecycle: Arc::new(WorkspaceProjectResolutionLifecycle {
            task: Mutex::new(Some(task)),
            task_scope,
        }),
    })
}

struct WorkspaceProjectResolutionActor {
    workspace_identity: WorkspaceResolutionIdentity,
    resolver: Arc<dyn WorkspaceProjectResolver>,
    attached_sessions: HashSet<String>,
    current_inputs: Option<ProjectResolutionInputs>,
    current_generation: Option<WorkspaceResolutionGeneration>,
    next_generation_id: u64,
    state: WorkspaceProjectResolutionState,
}

impl WorkspaceProjectResolutionActor {
    fn new(
        workspace_identity: WorkspaceResolutionIdentity,
        resolver: Arc<dyn WorkspaceProjectResolver>,
    ) -> Self {
        Self {
            workspace_identity,
            resolver,
            attached_sessions: HashSet::new(),
            current_inputs: None,
            current_generation: None,
            next_generation_id: 1,
            state: WorkspaceProjectResolutionState::WorkspaceAttached,
        }
    }

    fn receipt(&self) -> WorkspaceProjectResolutionReceipt {
        WorkspaceProjectResolutionReceipt {
            schema_id: WORKSPACE_PROJECT_RESOLUTION_RECEIPT_SCHEMA_ID.to_owned(),
            schema_version: WORKSPACE_PROJECT_RESOLUTION_SCHEMA_VERSION.to_owned(),
            workspace_identity: self.workspace_identity.clone(),
            state: self.state,
            attached_session_count: self.attached_sessions.len(),
            generation: self.current_generation.clone(),
            failure: None,
        }
    }

    fn failure(
        &self,
        reason_kind: WorkspaceProjectResolutionFailureKind,
        next: impl Into<String>,
    ) -> WorkspaceProjectResolutionReceipt {
        WorkspaceProjectResolutionReceipt {
            state: WorkspaceProjectResolutionState::Failed,
            failure: Some(WorkspaceProjectResolutionFailure {
                reason_kind,
                next: next.into(),
            }),
            ..self.receipt()
        }
    }

    async fn apply(&mut self, operation: ActorOperation) -> WorkspaceProjectResolutionReceipt {
        match operation {
            ActorOperation::AttachSession(session_identity) => {
                self.attached_sessions.insert(session_identity);
                self.state = self
                    .current_generation
                    .as_ref()
                    .map_or(WorkspaceProjectResolutionState::WorkspaceAttached, |_| {
                        WorkspaceProjectResolutionState::GenerationServing
                    });
                self.receipt()
            }
            ActorOperation::DetachSession(session_identity) => {
                self.attached_sessions.remove(&session_identity);
                self.receipt()
            }
            ActorOperation::GetCurrentScope(session_identity) => {
                if !self.attached_sessions.contains(&session_identity) {
                    return self.failure(
                        WorkspaceProjectResolutionFailureKind::SessionNotAttached,
                        "attach-session before requesting the current resolved source scope",
                    );
                }
                if self.current_generation.is_none() {
                    return self.failure(
                        WorkspaceProjectResolutionFailureKind::ResolvedSourceScopeUnavailable,
                        "publish a package-manager-derived workspace generation",
                    );
                }
                self.state = WorkspaceProjectResolutionState::GenerationServing;
                self.receipt()
            }
            ActorOperation::ObserveGeneration => self.receipt(),
            ActorOperation::RefreshInputs(inputs) => self.refresh(inputs).await,
            ActorOperation::Shutdown => self.receipt(),
        }
    }

    async fn refresh(
        &mut self,
        mut inputs: ProjectResolutionInputs,
    ) -> WorkspaceProjectResolutionReceipt {
        canonicalize_inputs(&mut inputs);
        if self.current_inputs.as_ref() == Some(&inputs) {
            self.state = WorkspaceProjectResolutionState::GenerationServing;
            return self.receipt();
        }
        self.state = WorkspaceProjectResolutionState::ProjectEntryDeltaClassified;
        let generation_id = self.next_generation_id;
        self.state = WorkspaceProjectResolutionState::PackageResolverRunning;
        let resolved = self.resolver.resolve(inputs.clone(), generation_id).await;
        let generation = match resolved {
            Ok(generation) => generation,
            Err(failure) => {
                self.state = WorkspaceProjectResolutionState::Failed;
                return WorkspaceProjectResolutionReceipt {
                    failure: Some(failure),
                    ..self.receipt()
                };
            }
        };
        if generation.generation_id != generation_id
            || generation.candidate_generation != inputs.candidate_generation
        {
            return self.failure(
                WorkspaceProjectResolutionFailureKind::CandidateGenerationStale,
                "resolver must publish the requested candidate generation and generation id",
            );
        }
        self.state = WorkspaceProjectResolutionState::WorkspaceArtifactPublished;
        self.current_inputs = Some(inputs);
        self.current_generation = Some(generation);
        self.next_generation_id += 1;
        self.state = WorkspaceProjectResolutionState::GenerationServing;
        self.receipt()
    }
}

enum ActorOperation {
    AttachSession(String),
    DetachSession(String),
    RefreshInputs(ProjectResolutionInputs),
    GetCurrentScope(String),
    ObserveGeneration,
    Shutdown,
}

struct ActorRequest {
    operation: ActorOperation,
    reply: oneshot::Sender<WorkspaceProjectResolutionReceipt>,
}

async fn run_actor(
    mut actor: WorkspaceProjectResolutionActor,
    mut receiver: mpsc::Receiver<ActorRequest>,
) {
    while let Some(request) = receiver.recv().await {
        let shutdown = matches!(&request.operation, ActorOperation::Shutdown);
        let receipt = if shutdown {
            actor.receipt()
        } else {
            actor.apply(request.operation).await
        };
        let _ = request.reply.send(receipt);
        if shutdown {
            receiver.close();
            break;
        }
    }
}

fn canonicalize_inputs(inputs: &mut ProjectResolutionInputs) {
    inputs.project_entries.sort_by(|left, right| {
        left.path
            .cmp(&right.path)
            .then(left.provider_id.cmp(&right.provider_id))
    });
    inputs
        .package_manager_inputs
        .sort_by(|left, right| left.path.cmp(&right.path));
}

fn unavailable_receipt(
    workspace_identity: &WorkspaceResolutionIdentity,
) -> WorkspaceProjectResolutionReceipt {
    WorkspaceProjectResolutionReceipt {
        schema_id: WORKSPACE_PROJECT_RESOLUTION_RECEIPT_SCHEMA_ID.to_owned(),
        schema_version: WORKSPACE_PROJECT_RESOLUTION_SCHEMA_VERSION.to_owned(),
        workspace_identity: workspace_identity.clone(),
        state: WorkspaceProjectResolutionState::Failed,
        attached_session_count: 0,
        generation: None,
        failure: Some(WorkspaceProjectResolutionFailure {
            reason_kind: WorkspaceProjectResolutionFailureKind::DaemonUnavailable,
            next: "start or reconnect to the workspace resident daemon".to_owned(),
        }),
    }
}
