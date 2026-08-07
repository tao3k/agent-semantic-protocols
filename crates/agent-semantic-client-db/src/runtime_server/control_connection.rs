use std::sync::Arc;

use tokio::{net::UnixStream, sync::watch};

use crate::{
    WorkspaceDbRegistry,
    runtime_server_control::{
        RuntimeServerControlReceipt, RuntimeServerControlRequest, RuntimeServerEndpoint,
        RuntimeServerRequestReadError, RuntimeServerState, read_runtime_server_requests,
        write_runtime_server_receipts,
    },
};

use super::GraphTurboResidentStatusHandle;

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

pub(super) async fn serve_connection(
    mut stream: UnixStream,
    endpoint: RuntimeServerEndpoint,
    registry: Arc<WorkspaceDbRegistry>,
    lifecycle: watch::Receiver<RuntimeServerState>,
    graph_turbo_resident_status: Option<GraphTurboResidentStatusHandle>,
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
        let entry_counts = registry.workspace_entry_counts();
        let slot_count = entry_counts.slot_count;
        let loaded_entry_count = entry_counts.loaded_entry_count;
        let workspace_entry_count = slot_count.max(loaded_entry_count);
        let mut restart = false;
        let mut receipts = Vec::with_capacity(requests.len());
        for request in requests {
            replay_guard.lock().await.admit(&request.request_id)?;
            let request_restart = request.requires_restart(&endpoint)?;
            restart |= request_restart;
            let mut receipt = if request_restart {
                RuntimeServerControlReceipt::draining(
                    request.request_id,
                    &endpoint,
                    workspace_entry_count,
                )
            } else if *lifecycle.borrow() == RuntimeServerState::Starting {
                let mut receipt = RuntimeServerControlReceipt::starting(
                    request.request_id,
                    endpoint.runtime_artifact_digest.clone(),
                    endpoint.artifact_mode.clone(),
                    endpoint.artifact_catalog_digest.clone(),
                    "workspace-generation-restore".to_owned(),
                );
                receipt.workspace_entry_count = workspace_entry_count;
                receipt
            } else {
                RuntimeServerControlReceipt::healthy(
                    request.request_id,
                    &endpoint,
                    workspace_entry_count,
                )
            };
            receipt.graph_turbo_resident = graph_turbo_resident_status
                .as_ref()
                .map(|status| status.snapshot());
            receipts.push(receipt);
        }
        crate::runtime_server_runtime::within_connection_io_budget(
            "runtime-server-control-write",
            write_runtime_server_receipts(&mut stream, &receipts),
        )
        .await?;
        authenticated_first_frame = true;
        if restart {
            return Ok(true);
        }
    }
}

async fn read_control_requests(
    stream: &mut UnixStream,
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
