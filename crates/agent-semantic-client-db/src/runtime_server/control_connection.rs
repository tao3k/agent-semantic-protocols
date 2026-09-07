// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use std::sync::Arc;

use tokio::{io::AsyncWriteExt, net::TcpStream, sync::watch};

use crate::{
    WorkspaceDbRegistry,
    runtime_server_control::{
        RuntimeServerControlReceipt, RuntimeServerControlRequest, RuntimeServerEndpoint,
        RuntimeServerRequestReadError, RuntimeServerState, read_runtime_server_requests,
        write_runtime_server_receipts,
    },
};

use super::AspPythonGraphsStatusHandle;

const CONTROL_REPLAY_EPOCH_CAPACITY: usize = 262_144;

#[derive(Default)]
pub(super) struct RuntimeServerControlReplayGuard {
    admitted: std::collections::HashSet<[u8; 32]>,
}

impl RuntimeServerControlReplayGuard {
    fn admit(&mut self, request_id: &str) -> Result<(), String> {
        if request_id.is_empty() {
            return Err("Runtime Server control request nonce is empty".to_owned());
        }
        let nonce_digest = *blake3::hash(request_id.as_bytes()).as_bytes();
        if self.admitted.contains(&nonce_digest) {
            return Err("Runtime Server control request replay rejected".to_owned());
        }
        if self.admitted.len() >= CONTROL_REPLAY_EPOCH_CAPACITY {
            return Err(
                "Runtime Server control nonce capacity exhausted for the current owner epoch"
                    .to_owned(),
            );
        }
        self.admitted.insert(nonce_digest);
        Ok(())
    }
}

async fn ensure_control_workspace(
    project_root: Option<&str>,
    generation_admission: Option<
        &Arc<crate::runtime_server_admission::WorkspaceGenerationAdmission>,
    >,
) -> Result<Option<crate::runtime_server_control::WorkspaceGenerationControlReceipt>, String> {
    let project_root = project_root
        .map(std::path::Path::new)
        .ok_or_else(|| "Runtime Server ensure-workspace request omitted project root".to_owned())?;
    let admission = generation_admission.ok_or_else(|| {
        "Runtime Server ensure-workspace requires the workspace admission authority".to_owned()
    })?;
    admission
        .admit_project_workspace_identity(project_root.to_path_buf())
        .await?;
    Ok(None)
}

async fn build_control_receipt(
    request: RuntimeServerControlRequest,
    endpoint: &RuntimeServerEndpoint,
    registry: &Arc<WorkspaceDbRegistry>,
    generation_admission: Option<
        &Arc<crate::runtime_server_admission::WorkspaceGenerationAdmission>,
    >,
    lifecycle: &watch::Receiver<RuntimeServerState>,
    asp_python_graphs_status: Option<&AspPythonGraphsStatusHandle>,
    replay_guard: &Arc<tokio::sync::Mutex<RuntimeServerControlReplayGuard>>,
) -> Result<(RuntimeServerControlReceipt, bool), String> {
    replay_guard.lock().await.admit(&request.request_id)?;
    let restart = request.requires_restart(endpoint)?;
    let generation = if request.operation
        == crate::runtime_server_control::RuntimeServerOperation::EnsureWorkspace
    {
        ensure_control_workspace(request.project_root.as_deref(), generation_admission).await
    } else {
        Ok(None)
    };
    let entry_counts = registry.workspace_entry_counts();
    let workspace_entry_count = entry_counts
        .slot_count
        .max(entry_counts.loaded_entry_count)
        .max(
            generation_admission
                .map(|admission| admission.admitted_project_workspace_count())
                .unwrap_or(0),
        );
    let mut receipt = control_receipt_for_state(
        request.request_id,
        endpoint,
        *lifecycle.borrow(),
        workspace_entry_count,
        restart,
        generation.as_ref().err(),
    );
    receipt.workspace_generation = generation.ok().flatten();
    receipt.asp_python_graphs = asp_python_graphs_status.map(|status| status.snapshot());
    Ok((receipt, restart))
}

fn control_receipt_for_state(
    request_id: String,
    endpoint: &RuntimeServerEndpoint,
    lifecycle: RuntimeServerState,
    workspace_entry_count: usize,
    restart: bool,
    ensure_failure: Option<&String>,
) -> RuntimeServerControlReceipt {
    if let Some(reason) = ensure_failure {
        let mut receipt =
            RuntimeServerControlReceipt::healthy(request_id, endpoint, workspace_entry_count);
        receipt.state = RuntimeServerState::Degraded;
        receipt.reason = Some(reason.clone());
        return receipt;
    }
    if restart {
        return RuntimeServerControlReceipt::draining(request_id, endpoint, workspace_entry_count);
    }
    if lifecycle == RuntimeServerState::Starting {
        let mut receipt = RuntimeServerControlReceipt::starting(
            request_id,
            endpoint.runtime_binary_identity.clone(),
            endpoint.artifact_mode.clone(),
            endpoint.artifact_catalog_digest.clone(),
            "workspace-generation-admission".to_owned(),
        );
        receipt.workspace_entry_count = workspace_entry_count;
        return receipt;
    }
    RuntimeServerControlReceipt::healthy(request_id, endpoint, workspace_entry_count)
}

async fn process_control_requests(
    requests: Vec<RuntimeServerControlRequest>,
    endpoint: &RuntimeServerEndpoint,
    registry: &Arc<WorkspaceDbRegistry>,
    generation_admission: Option<
        &Arc<crate::runtime_server_admission::WorkspaceGenerationAdmission>,
    >,
    lifecycle: &watch::Receiver<RuntimeServerState>,
    asp_python_graphs_status: Option<&AspPythonGraphsStatusHandle>,
    replay_guard: &Arc<tokio::sync::Mutex<RuntimeServerControlReplayGuard>>,
) -> Result<(Vec<RuntimeServerControlReceipt>, bool), String> {
    use tokio_stream::StreamExt;

    tokio_stream::iter(requests)
        .then(|request| {
            build_control_receipt(
                request,
                endpoint,
                registry,
                generation_admission,
                lifecycle,
                asp_python_graphs_status,
                replay_guard,
            )
        })
        .collect::<Vec<_>>()
        .await
        .into_iter()
        .try_fold((Vec::new(), false), |mut batch, outcome| {
            let (receipt, restart) = outcome?;
            batch.0.push(receipt);
            batch.1 |= restart;
            Ok(batch)
        })
}

pub(super) async fn serve_connection(
    mut stream: TcpStream,
    endpoint: RuntimeServerEndpoint,
    registry: Arc<WorkspaceDbRegistry>,
    generation_admission: Option<
        Arc<crate::runtime_server_admission::WorkspaceGenerationAdmission>,
    >,
    lifecycle: watch::Receiver<RuntimeServerState>,
    asp_python_graphs_status: Option<AspPythonGraphsStatusHandle>,
    mut drain: watch::Receiver<bool>,
    replay_guard: Arc<tokio::sync::Mutex<RuntimeServerControlReplayGuard>>,
) -> Result<bool, String> {
    let mut authenticated_first_frame = false;
    loop {
        let requests = tokio::select! {
            requests = read_control_requests(&mut stream, authenticated_first_frame) => match requests? {
                Some(requests) => requests,
                None => return Ok(false),
            },
            changed = drain.changed() => {
                let _ = changed;
                return Ok(false);
            }
        };
        let (receipts, restart) = process_control_requests(
            requests,
            &endpoint,
            &registry,
            generation_admission.as_ref(),
            &lifecycle,
            asp_python_graphs_status.as_ref(),
            &replay_guard,
        )
        .await?;
        crate::runtime_server_runtime::within_connection_io_budget(
            "runtime-server-control-write",
            write_runtime_server_receipts(&mut stream, &receipts),
        )
        .await?;
        authenticated_first_frame = true;
        if restart {
            // Linearize the draining receipt before this connection publishes
            // the process-level restart signal.  A write-complete task alone
            // is not a peer-visible boundary: explicitly flush and half-close
            // the response side so the client observes the typed receipt
            // before the supervisor drains sibling connections.
            stream.flush().await.map_err(|error| {
                format!("failed to flush Runtime Server restart receipt: {error}")
            })?;
            stream.shutdown().await.map_err(|error| {
                format!("failed to finalize Runtime Server restart receipt: {error}")
            })?;
            return Ok(true);
        }
    }
}

async fn read_control_requests(
    stream: &mut TcpStream,
    authenticated_first_frame: bool,
) -> Result<Option<Vec<RuntimeServerControlRequest>>, String> {
    let read = async {
        match read_runtime_server_requests(stream).await {
            Ok(requests) => Ok(Some(requests)),
            Err(RuntimeServerRequestReadError::Closed) => Ok(None),
            Err(RuntimeServerRequestReadError::Invalid(error)) => Err(error),
        }
    };
    if authenticated_first_frame {
        read.await
    } else {
        crate::runtime_server_runtime::within_connection_io_budget(
            "runtime-server-control-first-frame",
            read,
        )
        .await
    }
}
