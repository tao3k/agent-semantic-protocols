use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

use super::query::{RuntimePerformanceQuery, RuntimePerformanceQueryReceipt};

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
                connections.spawn(async move {
                    let result = crate::runtime_server_runtime::within_connection_io_budget(
                        "telemetry query",
                        serve_query(stream, live_store),
                    )
                    .await;
                    if result.is_ok() {
                        permit.complete();
                    } else {
                        permit.fail();
                    }
                    (connection_lease, result)
                });
            }
            completed = connections.join_next(), if !connections.is_empty() => {
                if let Some(result) = completed {
                    let (_connection_lease, result) = result.map_err(|error| {
                        format!("Runtime Server telemetry query task failed: {error}")
                    })?;
                    match result {
                        Ok(()) | Err(_) => {}
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
            Ok(()) | Err(_) => {}
        }
    }
    Ok(())
}

async fn serve_query(
    stream: tokio::net::UnixStream,
    live_store: std::sync::Arc<super::live_store::RuntimePerformanceLiveStore>,
) -> Result<(), String> {
    let (reader, mut writer) = stream.into_split();
    let mut request_line = String::new();
    BufReader::new(reader)
        .read_line(&mut request_line)
        .await
        .map_err(|error| format!("failed to read Runtime Server telemetry query: {error}"))?;
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
        .map_err(|error| format!("failed to finish Runtime Server telemetry receipt: {error}"))
}
