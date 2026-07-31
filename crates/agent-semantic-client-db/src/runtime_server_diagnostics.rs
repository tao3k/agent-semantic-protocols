use std::path::{Path, PathBuf};

use serde::Serialize;
use tokio::sync::mpsc;

use crate::runtime_server::RuntimeServerEvent;

const DIAGNOSTIC_SCHEMA_ID: &str = "agent.semantic-protocols.runtime-server-diagnostic";

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct RuntimeServerDiagnosticReceipt<'a> {
    schema_id: &'static str,
    schema_version: &'static str,
    event: &'a RuntimeServerEvent,
}

/// Owns the bounded latest-event diagnostic lane for one Runtime Server.
pub struct RuntimeServerDiagnostics {
    task: tokio::task::JoinHandle<Result<(), String>>,
}

impl RuntimeServerDiagnostics {
    pub async fn start(
        receipt_path: PathBuf,
    ) -> Result<(mpsc::UnboundedSender<RuntimeServerEvent>, Self), String> {
        if let Some(parent) = receipt_path.parent() {
            tokio::fs::create_dir_all(parent).await.map_err(|error| {
                format!(
                    "failed to create Runtime Server diagnostic directory {}: {error}",
                    parent.display()
                )
            })?;
        }
        let (sender, mut receiver) = mpsc::unbounded_channel();
        let task = tokio::spawn(async move {
            while let Some(event) = receiver.recv().await {
                write_latest_event(&receipt_path, &event).await?;
            }
            Ok(())
        });
        Ok((sender, Self { task }))
    }

    pub async fn join(self) -> Result<(), String> {
        self.task
            .await
            .map_err(|error| format!("Runtime Server diagnostic lane failed: {error}"))?
    }
}

async fn write_latest_event(path: &Path, event: &RuntimeServerEvent) -> Result<(), String> {
    let bytes = serde_json::to_vec(&RuntimeServerDiagnosticReceipt {
        schema_id: DIAGNOSTIC_SCHEMA_ID,
        schema_version: "1",
        event,
    })
    .map_err(|error| format!("failed to encode Runtime Server diagnostic receipt: {error}"))?;
    let temporary = path.with_extension("json.tmp");
    tokio::fs::write(&temporary, bytes).await.map_err(|error| {
        format!(
            "failed to write Runtime Server diagnostic receipt {}: {error}",
            temporary.display()
        )
    })?;
    tokio::fs::rename(&temporary, path).await.map_err(|error| {
        format!(
            "failed to publish Runtime Server diagnostic receipt {}: {error}",
            path.display()
        )
    })
}
