use std::path::PathBuf;

use tokio::sync::{mpsc, oneshot};

use crate::runtime_server_workspace::WorkspaceOwnerSnapshot;

const DEFAULT_QUEUE_CAPACITY: usize = 64;

pub enum RuntimeSearchServiceRequest {
    ProviderOwner {
        workspace_identity: String,
        project_root: PathBuf,
        language_id: String,
        owner_path: String,
        response: oneshot::Sender<Result<WorkspaceOwnerSnapshot, String>>,
    },
    TreeSitterQuery {
        workspace_identity: String,
        project_root: PathBuf,
        language_id: String,
        args: Vec<String>,
        response: oneshot::Sender<Result<Option<String>, String>>,
    },
}

#[derive(Clone)]
pub struct RuntimeSearchServiceHandle {
    sender: mpsc::Sender<RuntimeSearchServiceRequest>,
}

pub fn runtime_search_service_channel() -> (
    RuntimeSearchServiceHandle,
    mpsc::Receiver<RuntimeSearchServiceRequest>,
) {
    let (sender, receiver) = mpsc::channel(DEFAULT_QUEUE_CAPACITY);
    (RuntimeSearchServiceHandle { sender }, receiver)
}

impl RuntimeSearchServiceHandle {
    pub async fn provider_owner(
        &self,
        workspace_identity: String,
        project_root: PathBuf,
        language_id: String,
        owner_path: String,
    ) -> Result<WorkspaceOwnerSnapshot, String> {
        let (response, receipt) = oneshot::channel();
        self.sender
            .send(RuntimeSearchServiceRequest::ProviderOwner {
                workspace_identity,
                project_root,
                language_id,
                owner_path,
                response,
            })
            .await
            .map_err(|_| "Runtime search service is not accepting owner requests".to_owned())?;
        receipt
            .await
            .map_err(|_| "Runtime search service dropped the owner response".to_owned())?
    }

    pub async fn tree_sitter_query(
        &self,
        workspace_identity: String,
        project_root: PathBuf,
        language_id: String,
        args: Vec<String>,
    ) -> Result<Option<String>, String> {
        let (response, receipt) = oneshot::channel();
        self.sender
            .send(RuntimeSearchServiceRequest::TreeSitterQuery {
                workspace_identity,
                project_root,
                language_id,
                args,
                response,
            })
            .await
            .map_err(|_| {
                "Runtime search service is not accepting Tree-sitter requests".to_owned()
            })?;
        receipt
            .await
            .map_err(|_| "Runtime search service dropped the Tree-sitter response".to_owned())?
    }
}
