use std::path::PathBuf;

use tokio::sync::{mpsc, oneshot};

use crate::runtime_server_workspace::WorkspaceOwnerSnapshot;

const DEFAULT_QUEUE_CAPACITY: usize = 64;

#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeProviderSearchReceipt {
    pub schema_id: String,
    pub schema_version: String,
    pub operation_id: String,
    pub status: String,
    pub language_id: String,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
    pub status_code: i32,
    pub read_state: crate::source_index::ClientDbSourceIndexLookupState,
    pub candidate_count: usize,
    pub selectors: Vec<String>,
    pub root_digest: Option<String>,
    pub provider_digest: Option<String>,
    pub index_artifact_digest: Option<String>,
    pub resident_read_elapsed_micros: u64,
    pub service_elapsed_micros: u64,
    pub elapsed_micros: u64,
}

pub enum RuntimeSearchServiceRequest {
    ProviderRuntime {
        project_root: PathBuf,
        language_id: String,
        response: oneshot::Sender<Result<serde_json::Value, String>>,
    },
    ProviderRuntimeReady {
        project_root: PathBuf,
        language_id: String,
        response: oneshot::Sender<Result<serde_json::Value, String>>,
    },
    ProviderRuntimeAwaitReady {
        project_root: PathBuf,
        language_id: String,
        response: oneshot::Sender<Result<serde_json::Value, String>>,
    },
    ProviderOperation {
        project_root: PathBuf,
        language_id: String,
        operation: String,
        payload: Vec<u8>,
        response: oneshot::Sender<Result<Vec<u8>, String>>,
    },
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

pub fn build_runtime_provider_search_receipt(
    operation_id: String,
    language_id: agent_semantic_client_core::LanguageId,
    lookup: crate::source_index::ClientDbSourceIndexLookupResult,
    resident_read_elapsed_micros: u64,
) -> Result<RuntimeProviderSearchReceipt, String> {
    let started = std::time::Instant::now();
    let read_state = lookup.state.clone();
    match read_state {
        crate::source_index::ClientDbSourceIndexLookupState::MissingDb => {
            return Err("runtime-provider-search-source-index-missing".to_owned());
        }
        crate::source_index::ClientDbSourceIndexLookupState::ColdRequired => {
            return Err("runtime-provider-search-source-index-cold-required".to_owned());
        }
        crate::source_index::ClientDbSourceIndexLookupState::Busy => {
            return Err("runtime-provider-search-source-index-busy".to_owned());
        }
        _ => {}
    }

    let stdout = if lookup.candidates.is_empty() {
        format!(
            "[search-frontier] state={:?} languageId={} candidates=0\n",
            read_state, language_id
        )
        .into_bytes()
    } else {
        let mut rendered = lookup
            .candidates
            .iter()
            .map(|candidate| candidate.path.to_string())
            .collect::<Vec<_>>()
            .join("\n")
            .into_bytes();
        rendered.push(b'\n');
        rendered
    };
    let candidate_count = lookup.candidates.len();
    let selectors = lookup
        .candidates
        .iter()
        .filter_map(|candidate| candidate.selector_projection.as_ref())
        .map(|projection| projection.proof.structural_selector().to_owned())
        .collect::<Vec<_>>();
    let (root_digest, provider_digest) = lookup
        .source_snapshot
        .as_ref()
        .map(|snapshot| {
            (
                Some(snapshot.root_digest.clone()),
                Some(snapshot.provider_digest.clone()),
            )
        })
        .unwrap_or((None, None));
    let service_elapsed_micros = u64::try_from(started.elapsed().as_micros()).unwrap_or(u64::MAX);
    let elapsed_micros = resident_read_elapsed_micros.saturating_add(service_elapsed_micros);
    if elapsed_micros >= 1_000 {
        return Err(format!(
            "runtime-provider-search exceeded the synchronous mmap receipt budget: elapsedMicros={elapsed_micros} budgetExclusiveMicros=1000"
        ));
    }

    Ok(RuntimeProviderSearchReceipt {
        schema_id: "agent.semantic-protocols.runtime-provider-search-receipt.v1".to_owned(),
        schema_version: "1".to_owned(),
        operation_id,
        status: if candidate_count == 0 {
            "no-matches".to_owned()
        } else {
            "matches".to_owned()
        },
        language_id: language_id.to_string(),
        stdout,
        stderr: Vec::new(),
        status_code: 0,
        read_state,
        candidate_count,
        selectors,
        root_digest,
        provider_digest,
        index_artifact_digest: lookup.index_artifact_digest,
        resident_read_elapsed_micros,
        service_elapsed_micros,
        elapsed_micros,
    })
}

#[cfg(test)]
#[path = "../tests/unit/runtime_search_service.rs"]
mod tests;

impl RuntimeSearchServiceHandle {
    pub async fn provider_runtime(
        &self,
        project_root: PathBuf,
        language_id: String,
    ) -> Result<serde_json::Value, String> {
        let (response, receipt) = oneshot::channel();
        self.sender
            .send(RuntimeSearchServiceRequest::ProviderRuntime {
                project_root,
                language_id,
                response,
            })
            .await
            .map_err(|_| {
                "Runtime search service is not accepting provider runtime requests".to_owned()
            })?;
        receipt.await.map_err(|_| {
            "Runtime search service dropped the provider runtime response".to_owned()
        })?
    }

    pub async fn provider_runtime_ready(
        &self,
        project_root: PathBuf,
        language_id: String,
    ) -> Result<serde_json::Value, String> {
        let (response, receipt) = oneshot::channel();
        self.sender
            .send(RuntimeSearchServiceRequest::ProviderRuntimeReady {
                project_root,
                language_id,
                response,
            })
            .await
            .map_err(|_| {
                "Runtime search service is not accepting provider readiness requests".to_owned()
            })?;
        receipt.await.map_err(|_| {
            "Runtime search service dropped the provider readiness response".to_owned()
        })?
    }

    pub async fn provider_runtime_await_ready(
        &self,
        project_root: PathBuf,
        language_id: String,
    ) -> Result<serde_json::Value, String> {
        let (response, receipt) = oneshot::channel();
        self.sender
            .send(RuntimeSearchServiceRequest::ProviderRuntimeAwaitReady {
                project_root,
                language_id,
                response,
            })
            .await
            .map_err(|_| {
                "Runtime search service is not accepting provider readiness waits".to_owned()
            })?;
        receipt.await.map_err(|_| {
            "Runtime search service dropped the provider readiness wait response".to_owned()
        })?
    }

    pub async fn provider_operation(
        &self,
        project_root: PathBuf,
        language_id: String,
        operation: String,
        payload: Vec<u8>,
    ) -> Result<Vec<u8>, String> {
        let (response, receipt) = oneshot::channel();
        self.sender
            .send(RuntimeSearchServiceRequest::ProviderOperation {
                project_root,
                language_id,
                operation,
                payload,
                response,
            })
            .await
            .map_err(|_| {
                "Runtime search service is not accepting provider operation requests".to_owned()
            })?;
        receipt.await.map_err(|_| {
            "Runtime search service dropped the provider operation response".to_owned()
        })?
    }
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
