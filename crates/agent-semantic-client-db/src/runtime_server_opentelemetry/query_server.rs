use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

use super::{
    exporter::TursoOpenTelemetrySpanExporter,
    query::{RuntimePerformanceQuery, RuntimePerformanceQueryReceipt},
};

pub(super) async fn run_query_server(
    listener: tokio::net::UnixListener,
    exporter: TursoOpenTelemetrySpanExporter,
    provider: opentelemetry_sdk::trace::SdkTracerProvider,
    mut shutdown: tokio::sync::watch::Receiver<bool>,
) -> Result<(), String> {
    let mut connections = tokio::task::JoinSet::new();
    loop {
        tokio::select! {
            accepted = listener.accept() => {
                let (stream, _) = accepted.map_err(|error| {
                    format!("failed to accept Runtime Server telemetry query: {error}")
                })?;
                let exporter = exporter.clone();
                let provider = provider.clone();
                connections.spawn(async move { serve_query(stream, exporter, provider).await });
            }
            completed = connections.join_next(), if !connections.is_empty() => {
                if let Some(result) = completed {
                    match result.map_err(|error| {
                        format!("Runtime Server telemetry query task failed: {error}")
                    })? {
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
        match result
            .map_err(|error| format!("Runtime Server telemetry query task failed: {error}"))?
        {
            Ok(()) | Err(_) => {}
        }
    }
    Ok(())
}

async fn serve_query(
    stream: tokio::net::UnixStream,
    exporter: TursoOpenTelemetrySpanExporter,
    provider: opentelemetry_sdk::trace::SdkTracerProvider,
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
    tokio::task::spawn_blocking(move || provider.force_flush())
        .await
        .map_err(|error| format!("Runtime Server telemetry flush task failed: {error}"))?
        .map_err(|error| format!("Runtime Server telemetry flush failed: {error}"))?;
    let count = exporter
        .budget_failure_count_for_workspace(&query.workspace_identity, &query.surface, &query.stage)
        .await?;
    let latest_attributes_json = exporter
        .latest_attributes_json_for_workspace(
            &query.workspace_identity,
            &query.surface,
            &query.stage,
        )
        .await?;
    let receipt = RuntimePerformanceQueryReceipt::new(query, count, latest_attributes_json);
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
