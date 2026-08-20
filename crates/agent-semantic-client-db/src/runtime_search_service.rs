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
    pub owner_paths: Vec<String>,
    pub root_digest: Option<String>,
    pub provider_digest: Option<String>,
    pub index_artifact_digest: Option<String>,
    pub resident_read_elapsed_micros: u64,
    pub service_elapsed_micros: u64,
    pub elapsed_micros: u64,
    pub work_counters: crate::workspace_db_ipc::RuntimeResidentReadWorkCounters,
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
    ProviderRuntimeRelease {
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
    request_deadline: std::time::Duration,
}

const DEFAULT_RUNTIME_SEARCH_REQUEST_DEADLINE: std::time::Duration =
    std::time::Duration::from_secs(30);

pub fn runtime_search_service_channel() -> (
    RuntimeSearchServiceHandle,
    mpsc::Receiver<RuntimeSearchServiceRequest>,
) {
    runtime_search_service_channel_with_deadline(DEFAULT_RUNTIME_SEARCH_REQUEST_DEADLINE)
}

fn runtime_search_service_channel_with_deadline(
    request_deadline: std::time::Duration,
) -> (
    RuntimeSearchServiceHandle,
    mpsc::Receiver<RuntimeSearchServiceRequest>,
) {
    let (sender, receiver) = mpsc::channel(DEFAULT_QUEUE_CAPACITY);
    (
        RuntimeSearchServiceHandle {
            sender,
            request_deadline,
        },
        receiver,
    )
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
            return Err("runtime-provider-search-source-index-resident-index-missing".to_owned());
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
    let owner_paths = lookup
        .candidates
        .iter()
        .filter_map(|candidate| candidate.selector_projection.as_ref())
        .map(|projection| projection.proof.owner_path().to_owned())
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
        owner_paths,
        root_digest,
        provider_digest,
        index_artifact_digest: lookup.index_artifact_digest,
        resident_read_elapsed_micros,
        service_elapsed_micros,
        elapsed_micros,
        work_counters: crate::workspace_db_ipc::RuntimeResidentReadWorkCounters {
            database_opens: 0,
            filesystem_reads: 0,
            provider_spawns: 0,
            control_socket_roundtrips: 0,
        },
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
        self.await_receipt(
            receipt,
            "provider-runtime",
            "Runtime search service dropped the provider runtime response",
        )
        .await
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
        self.await_receipt(
            receipt,
            "provider-runtime-ready",
            "Runtime search service dropped the provider readiness response",
        )
        .await
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
        self.await_receipt(
            receipt,
            "provider-runtime-await-ready",
            "Runtime search service dropped the provider readiness wait response",
        )
        .await
    }

    pub async fn provider_runtime_release(
        &self,
        project_root: PathBuf,
        language_id: String,
    ) -> Result<serde_json::Value, String> {
        let (response, receipt) = oneshot::channel();
        self.sender
            .send(RuntimeSearchServiceRequest::ProviderRuntimeRelease {
                project_root,
                language_id,
                response,
            })
            .await
            .map_err(|_| {
                "Runtime search service is not accepting provider release requests".to_owned()
            })?;
        self.await_receipt(
            receipt,
            "provider-runtime-release",
            "Runtime search service dropped the provider release response",
        )
        .await
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
        self.await_receipt(
            receipt,
            "provider-operation",
            "Runtime search service dropped the provider operation response",
        )
        .await
    }
    async fn await_receipt<T>(
        &self,
        receipt: oneshot::Receiver<Result<T, String>>,
        operation: &str,
        dropped_message: &str,
    ) -> Result<T, String> {
        match tokio::time::timeout(self.request_deadline, receipt).await {
            Ok(Ok(result)) => result,
            Ok(Err(_)) => Err(dropped_message.to_owned()),
            Err(_) => Err(format!(
                "runtime search service request deadline exceeded: operation={operation} deadlineMillis={}",
                self.request_deadline.as_millis()
            )),
        }
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
        self.await_receipt(
            receipt,
            "provider-owner",
            "Runtime search service dropped the owner response",
        )
        .await
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
        self.await_receipt(
            receipt,
            "tree-sitter-query",
            "Runtime search service dropped the Tree-sitter response",
        )
        .await
    }
}
