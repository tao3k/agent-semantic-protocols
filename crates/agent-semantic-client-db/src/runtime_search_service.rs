// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use std::path::PathBuf;

use serde_json::Value;
use tokio::sync::{mpsc, oneshot};
use tokio_stream::wrappers::ReceiverStream;

use crate::runtime_server_workspace::WorkspaceOwnerProjection;

const DEFAULT_QUEUE_CAPACITY: usize = 64;

pub enum RuntimeSearchServiceRequest {
    ProviderRuntime {
        project_root: PathBuf,
        language_id: String,
        response: oneshot::Sender<
            Result<agent_semantic_provider_transport::AspClientServerLifecycleReceipt, String>,
        >,
    },
    ProviderRuntimeReady {
        project_root: PathBuf,
        language_id: String,
        response: oneshot::Sender<
            Result<agent_semantic_provider_transport::AspClientServerLifecycleReceipt, String>,
        >,
    },
    ProviderRuntimeAwaitReady {
        project_root: PathBuf,
        language_id: String,
        cancellation: crate::runtime_generation_cancellation::GenerationCancellation,
        response: oneshot::Sender<
            Result<agent_semantic_provider_transport::AspClientServerLifecycleReceipt, String>,
        >,
    },
    ProviderRuntimeRelease {
        project_root: PathBuf,
        language_id: String,
        response: oneshot::Sender<
            Result<agent_semantic_provider_transport::AspClientServerLifecycleReceipt, String>,
        >,
    },
    ProviderOperation {
        project_root: PathBuf,
        language_id: String,
        operation: String,
        payload: Vec<u8>,
        cancellation: crate::runtime_generation_cancellation::GenerationCancellation,
        response: oneshot::Sender<Result<Vec<u8>, String>>,
    },
    GraphsTimeline {
        project_root: PathBuf,
        request_id: String,
        payload: Value,
        cancellation: crate::runtime_generation_cancellation::GenerationCancellation,
        response: oneshot::Sender<Result<Value, String>>,
    },
    GenerationGraph {
        request_id: String,
        payload: Value,
        cancellation: crate::runtime_generation_cancellation::GenerationCancellation,
        response: oneshot::Sender<Result<Value, String>>,
    },
    EvaluateResidentGraph {
        workspace_identity: String,
        generation_digest: String,
        request_id: String,
        payload: Value,
        cancellation: crate::runtime_generation_cancellation::GenerationCancellation,
        response: oneshot::Sender<Result<Value, String>>,
    },
    ReleaseGenerationGraph {
        workspace_identity: String,
        generation_digest: String,
        request_id: String,
        response: oneshot::Sender<Result<Value, String>>,
    },
    ProviderOwner {
        workspace_identity: String,
        project_root: PathBuf,
        language_id: String,
        owner_path: String,
        response: oneshot::Sender<Result<WorkspaceOwnerProjection, String>>,
    },
}

#[derive(Clone)]
pub struct RuntimeSearchServiceHandle {
    sender: mpsc::Sender<RuntimeSearchServiceRequest>,
}

pub fn runtime_search_service_channel() -> (
    RuntimeSearchServiceHandle,
    ReceiverStream<RuntimeSearchServiceRequest>,
) {
    let (sender, receiver) = mpsc::channel(DEFAULT_QUEUE_CAPACITY);
    (
        RuntimeSearchServiceHandle { sender },
        ReceiverStream::new(receiver),
    )
}

#[cfg(test)]
#[path = "../tests/unit/runtime_search_service.rs"]
mod tests;

impl RuntimeSearchServiceHandle {
    pub async fn provider_runtime(
        &self,
        project_root: PathBuf,
        language_id: String,
    ) -> Result<agent_semantic_provider_transport::AspClientServerLifecycleReceipt, String> {
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
    ) -> Result<agent_semantic_provider_transport::AspClientServerLifecycleReceipt, String> {
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
        cancellation: crate::runtime_generation_cancellation::GenerationCancellation,
    ) -> Result<agent_semantic_provider_transport::AspClientServerLifecycleReceipt, String> {
        let (response, receipt) = oneshot::channel();
        self.sender
            .send(RuntimeSearchServiceRequest::ProviderRuntimeAwaitReady {
                project_root,
                language_id,
                cancellation,
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
    ) -> Result<agent_semantic_provider_transport::AspClientServerLifecycleReceipt, String> {
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
        cancellation: crate::runtime_generation_cancellation::GenerationCancellation,
    ) -> Result<Vec<u8>, String> {
        let (response, receipt) = oneshot::channel();
        self.sender
            .send(RuntimeSearchServiceRequest::ProviderOperation {
                project_root,
                language_id,
                operation,
                payload,
                cancellation,
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

    /// Submit an explicitly requested offline history/timeline packet through
    /// the ASP Server-owned `asp-python-graphs` process. Ready Search/Query
    /// graph evaluation never enters this service.
    pub async fn graphs_timeline(
        &self,
        project_root: PathBuf,
        request_id: String,
        payload: Value,
        cancellation: crate::runtime_generation_cancellation::GenerationCancellation,
    ) -> Result<Value, String> {
        let (response, receipt) = oneshot::channel();
        self.sender
            .send(RuntimeSearchServiceRequest::GraphsTimeline {
                project_root,
                request_id,
                payload,
                cancellation,
                response,
            })
            .await
            .map_err(|_| {
                "Runtime search service is not accepting graph timeline requests".to_owned()
            })?;
        self.await_receipt(
            receipt,
            "graphs-timeline",
            "Runtime search service dropped the graph timeline response",
        )
        .await
    }

    pub async fn generation_graph(
        &self,
        request_id: String,
        payload: Value,
        cancellation: crate::runtime_generation_cancellation::GenerationCancellation,
    ) -> Result<Value, String> {
        let (response, receipt) = oneshot::channel();
        self.sender
            .send(RuntimeSearchServiceRequest::GenerationGraph {
                request_id,
                payload,
                cancellation,
                response,
            })
            .await
            .map_err(|_| {
                "Runtime search service is not accepting generation graph requests".to_owned()
            })?;
        self.await_receipt(
            receipt,
            "generation-graph",
            "Runtime search service dropped the generation graph response",
        )
        .await
    }

    pub async fn evaluate_resident_graph(
        &self,
        workspace_identity: String,
        generation_digest: String,
        request_id: String,
        payload: Value,
        cancellation: crate::runtime_generation_cancellation::GenerationCancellation,
    ) -> Result<Value, String> {
        let (response, receipt) = oneshot::channel();
        self.sender
            .send(RuntimeSearchServiceRequest::EvaluateResidentGraph {
                workspace_identity,
                generation_digest,
                request_id,
                payload,
                cancellation,
                response,
            })
            .await
            .map_err(|_| {
                "Runtime search service is not accepting resident graph evaluation requests"
                    .to_owned()
            })?;
        self.await_receipt(
            receipt,
            "evaluate-resident-graph",
            "Runtime search service dropped the resident graph evaluation response",
        )
        .await
    }

    pub async fn release_generation_graph(
        &self,
        workspace_identity: String,
        generation_digest: String,
        request_id: String,
    ) -> Result<Value, String> {
        let (response, receipt) = oneshot::channel();
        self.sender
            .send(RuntimeSearchServiceRequest::ReleaseGenerationGraph {
                workspace_identity,
                generation_digest,
                request_id,
                response,
            })
            .await
            .map_err(|_| {
                "Runtime search service is not accepting graph generation release requests"
                    .to_owned()
            })?;
        self.await_receipt(
            receipt,
            "release-generation-graph",
            "Runtime search service dropped the graph generation release response",
        )
        .await
    }

    async fn await_receipt<T>(
        &self,
        receipt: oneshot::Receiver<Result<T, String>>,
        operation: &str,
        dropped_message: &str,
    ) -> Result<T, String> {
        match receipt.await {
            Ok(result) => result,
            Err(_) => Err(format!("{dropped_message}: operation={operation}")),
        }
    }

    pub async fn provider_owner(
        &self,
        workspace_identity: String,
        project_root: PathBuf,
        language_id: String,
        owner_path: String,
    ) -> Result<WorkspaceOwnerProjection, String> {
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
}
