use std::collections::BTreeMap;
use std::future::Future;
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;

pub const WORKSPACE_GENERATION_ADMISSION_RECEIPT_SCHEMA_ID: &str =
    "agent.semantic-protocols.runtime-server-workspace-generation-admission.v1";

pub type WorkspaceGenerationBuildFuture =
    Pin<Box<dyn Future<Output = Result<(), String>> + Send + 'static>>;
pub type WorkspaceGenerationBuilder =
    Arc<dyn Fn(String, PathBuf) -> WorkspaceGenerationBuildFuture + Send + Sync + 'static>;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum WorkspaceGenerationAdmissionState {
    Building,
    Ready,
    Failed,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceGenerationAdmissionReceipt {
    pub schema_id: String,
    pub schema_version: String,
    pub workspace_identity: String,
    pub state: WorkspaceGenerationAdmissionState,
    pub accepted: bool,
    pub attempt: u64,
    pub error: Option<String>,
}

impl WorkspaceGenerationAdmissionReceipt {
    pub fn validate(&self) -> Result<(), String> {
        if self.schema_id != WORKSPACE_GENERATION_ADMISSION_RECEIPT_SCHEMA_ID
            || self.schema_version != "1"
        {
            return Err("workspace generation admission receipt schema mismatch".to_owned());
        }
        if self.workspace_identity.trim().is_empty() || self.attempt == 0 {
            return Err("workspace generation admission receipt identity is incomplete".to_owned());
        }
        match (&self.state, &self.error) {
            (WorkspaceGenerationAdmissionState::Building, None)
            | (WorkspaceGenerationAdmissionState::Ready, None)
            | (WorkspaceGenerationAdmissionState::Failed, Some(_)) => Ok(()),
            _ => Err("workspace generation admission receipt state is inconsistent".to_owned()),
        }
    }
}

#[derive(Clone)]
pub struct WorkspaceGenerationAdmission {
    builder: WorkspaceGenerationBuilder,
    states: Arc<Mutex<BTreeMap<String, WorkspaceGenerationAdmissionReceipt>>>,
    tasks: Arc<Mutex<tokio::task::JoinSet<()>>>,
    changes: Arc<tokio::sync::Notify>,
}

impl WorkspaceGenerationAdmission {
    pub fn new(builder: WorkspaceGenerationBuilder) -> Self {
        Self {
            builder,
            states: Arc::new(Mutex::new(BTreeMap::new())),
            tasks: Arc::new(Mutex::new(tokio::task::JoinSet::new())),
            changes: Arc::new(tokio::sync::Notify::new()),
        }
    }

    pub async fn admit(
        &self,
        workspace_identity: impl Into<String>,
        project_root: PathBuf,
    ) -> Result<WorkspaceGenerationAdmissionReceipt, String> {
        let workspace_identity = workspace_identity.into();
        if workspace_identity.trim().is_empty() {
            return Err("workspace generation admission identity must be non-empty".to_owned());
        }
        if !project_root.is_absolute() {
            return Err("workspace generation admission root must be absolute".to_owned());
        }

        let mut states = self.states.lock().await;
        let attempt = match states.get(&workspace_identity) {
            Some(existing) if existing.state != WorkspaceGenerationAdmissionState::Failed => {
                let mut observed = existing.clone();
                observed.accepted = false;
                return Ok(observed);
            }
            Some(existing) => existing.attempt.saturating_add(1),
            None => 1,
        };
        let receipt = WorkspaceGenerationAdmissionReceipt {
            schema_id: WORKSPACE_GENERATION_ADMISSION_RECEIPT_SCHEMA_ID.to_owned(),
            schema_version: "1".to_owned(),
            workspace_identity: workspace_identity.clone(),
            state: WorkspaceGenerationAdmissionState::Building,
            accepted: true,
            attempt,
            error: None,
        };
        receipt.validate()?;
        states.insert(workspace_identity.clone(), receipt.clone());
        drop(states);

        let builder = Arc::clone(&self.builder);
        let states = Arc::clone(&self.states);
        let changes = Arc::clone(&self.changes);
        let mut tasks = self.tasks.lock().await;
        while tasks.try_join_next().is_some() {}
        tasks.spawn(async move {
            let completed = match builder(workspace_identity.clone(), project_root).await {
                Ok(()) => WorkspaceGenerationAdmissionReceipt {
                    schema_id: WORKSPACE_GENERATION_ADMISSION_RECEIPT_SCHEMA_ID.to_owned(),
                    schema_version: "1".to_owned(),
                    workspace_identity: workspace_identity.clone(),
                    state: WorkspaceGenerationAdmissionState::Ready,
                    accepted: false,
                    attempt,
                    error: None,
                },
                Err(error) => WorkspaceGenerationAdmissionReceipt {
                    schema_id: WORKSPACE_GENERATION_ADMISSION_RECEIPT_SCHEMA_ID.to_owned(),
                    schema_version: "1".to_owned(),
                    workspace_identity: workspace_identity.clone(),
                    state: WorkspaceGenerationAdmissionState::Failed,
                    accepted: false,
                    attempt,
                    error: Some(error),
                },
            };
            states.lock().await.insert(workspace_identity, completed);
            changes.notify_waiters();
        });
        Ok(receipt)
    }

    pub async fn status(
        &self,
        workspace_identity: &str,
    ) -> Option<WorkspaceGenerationAdmissionReceipt> {
        self.states.lock().await.get(workspace_identity).cloned()
    }

    pub async fn wait_terminal(
        &self,
        workspace_identity: &str,
    ) -> Result<WorkspaceGenerationAdmissionReceipt, String> {
        loop {
            let changed = self.changes.notified();
            tokio::pin!(changed);
            changed.as_mut().enable();
            let receipt = self.status(workspace_identity).await.ok_or_else(|| {
                format!(
                    "workspace generation admission is unknown: workspaceIdentity={workspace_identity}"
                )
            })?;
            if receipt.state != WorkspaceGenerationAdmissionState::Building {
                return Ok(receipt);
            }
            changed.as_mut().await;
        }
    }

    pub async fn shutdown(&self) -> Result<usize, String> {
        let mut tasks = self.tasks.lock().await;
        let task_count = tasks.len();
        tasks.abort_all();
        while let Some(result) = tasks.join_next().await {
            if let Err(error) = result
                && !error.is_cancelled()
            {
                return Err(format!(
                    "workspace generation admission task failed during shutdown: {error}"
                ));
            }
        }
        Ok(task_count)
    }
}
