use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

use super::query::{RuntimePerformanceQuery, RuntimePerformanceQueryReceipt};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum QueryConnectionOutcome {
    LivenessProbe,
    ReceiptWritten,
}

fn record_query_connection_terminal(
    permit: crate::runtime_server_runtime::RuntimeServerTaskPermit,
    result: &Result<QueryConnectionOutcome, String>,
) {
    if result.is_ok() {
        permit.complete();
    } else {
        permit.fail();
    }
}

pub(super) async fn run_query_server(
    listener: tokio::net::UnixListener,
    live_store: std::sync::Arc<super::live_store::RuntimePerformanceLiveStore>,
    mut shutdown: tokio::sync::watch::Receiver<bool>,
    task_scope: crate::runtime_server_runtime::RuntimeServerTaskScope,
) -> Result<(), String> {
    let mut connections = tokio::task::JoinSet::new();
    let connection_supervisor =
        crate::runtime_server_runtime::RuntimeServerConnectionSupervisor::for_current_runtime(
            "runtime-server-telemetry-query",
        );
    loop {
        tokio::select! {
            accepted = listener.accept(), if connection_supervisor.has_capacity() => {
                let (stream, _) = accepted.map_err(|error| {
                    format!("failed to accept Runtime Server telemetry query: {error}")
                })?;
                let live_store = std::sync::Arc::clone(&live_store);
                let permit = task_scope.permit("runtime-server-telemetry-query-connection")?;
                let connection_lease = connection_supervisor
                    .try_admit()
                    .expect("capacity guard must admit one telemetry query connection");
                let mut connection_shutdown = shutdown.clone();
                connections.spawn(async move {
                    let result = tokio::select! {
                        biased;
                        changed = connection_shutdown.changed() => {
                            let _ = changed;
                            Err("telemetry query connection cancelled by shutdown".to_owned())
                        }
                        result = crate::runtime_server_runtime::within_connection_io_budget(
                            "telemetry query",
                            serve_query(stream, live_store),
                        ) => result,
                    };
                    record_query_connection_terminal(permit, &result);
                    (connection_lease, result)
                });
            }
            completed = connections.join_next(), if !connections.is_empty() => {
                if let Some(result) = completed {
                    let (_connection_lease, result) = result.map_err(|error| {
                        format!("Runtime Server telemetry query task failed: {error}")
                    })?;
                    match result {
                        Ok(_) | Err(_) => {}
                    }
                }
            }
            changed = shutdown.changed() => {
                let _ = changed;
                break;
            }
        }
    }
    while let Some(result) = connections.join_next().await {
        let (_connection_lease, result) = result
            .map_err(|error| format!("Runtime Server telemetry query task failed: {error}"))?;
        match result {
            Ok(_) | Err(_) => {}
        }
    }
    Ok(())
}

async fn serve_query(
    stream: tokio::net::UnixStream,
    live_store: std::sync::Arc<super::live_store::RuntimePerformanceLiveStore>,
) -> Result<QueryConnectionOutcome, String> {
    let (reader, mut writer) = stream.into_split();
    let mut request_line = String::new();
    let bytes_read = BufReader::new(reader)
        .read_line(&mut request_line)
        .await
        .map_err(|error| format!("failed to read Runtime Server telemetry query: {error}"))?;
    if bytes_read == 0 {
        return Ok(QueryConnectionOutcome::LivenessProbe);
    }
    if !request_line.ends_with('\n') {
        return Err(serde_json::json!({
            "schemaId": "agent.semantic-protocols.client.frame",
            "schemaVersion": "1",
            "state": "failed",
            "reasonKind": "frame-eof",
            "message": "Runtime Server telemetry query ended before its frame delimiter"
        })
        .to_string());
    }
    let query: RuntimePerformanceQuery = serde_json::from_str(&request_line)
        .map_err(|error| format!("failed to decode Runtime Server telemetry query: {error}"))?;
    query.validate()?;
    let summary = live_store.summary(&query.workspace_identity, &query.surface, &query.stage);
    let receipt = RuntimePerformanceQueryReceipt::new(query, summary);
    let mut packet = serde_json::to_vec(&receipt)
        .map_err(|error| format!("failed to encode Runtime Server telemetry receipt: {error}"))?;
    packet.push(b'\n');
    writer
        .write_all(&packet)
        .await
        .map_err(|error| format!("failed to write Runtime Server telemetry receipt: {error}"))?;
    writer
        .shutdown()
        .await
        .map_err(|error| format!("failed to finish Runtime Server telemetry receipt: {error}"))?;
    Ok(QueryConnectionOutcome::ReceiptWritten)
}

#[cfg(test)]
#[path = "../../tests/unit/runtime_server_opentelemetry_query_connection.rs"]
mod tests;
