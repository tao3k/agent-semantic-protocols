// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;

use crate::runtime_server::RuntimeServerEvent;

const DIAGNOSTIC_SCHEMA_ID: &str = "agent.semantic-protocols.runtime-server-diagnostic";
const DIAGNOSTIC_JOURNAL_SCHEMA_ID: &str =
    "agent.semantic-protocols.runtime-server-diagnostic-journal";
const DIAGNOSTIC_JOURNAL_CAPACITY: usize = 64;
const DIAGNOSTIC_PERSIST_INTERVAL: std::time::Duration = std::time::Duration::from_secs(5);

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct RuntimeServerDiagnosticReceipt<'a> {
    schema_id: &'static str,
    schema_version: &'static str,
    sequence: u64,
    event: &'a serde_json::Value,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct RuntimeServerDiagnosticJournalReceipt<'a> {
    schema_id: &'static str,
    schema_version: &'static str,
    events: &'a std::collections::VecDeque<RuntimeServerDiagnosticJournalEntry>,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct RuntimeServerDiagnosticJournalEntry {
    sequence: u64,
    observed_unix_millis: u64,
    event: serde_json::Value,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RuntimeServerDiagnosticJournalSnapshot {
    schema_id: String,
    schema_version: String,
    events: std::collections::VecDeque<RuntimeServerDiagnosticJournalEntry>,
}

/// Owns the bounded latest-event diagnostic lane for one Runtime Server.
pub struct RuntimeServerDiagnostics {
    task: tokio::task::JoinHandle<Result<(), String>>,
}

impl RuntimeServerDiagnostics {
    pub async fn start(
        receipt_path: PathBuf,
    ) -> Result<
        (
            crate::runtime_server_observability::RuntimeServerEventPublisher,
            Self,
        ),
        String,
    > {
        if let Some(parent) = receipt_path.parent() {
            tokio::fs::create_dir_all(parent).await.map_err(|error| {
                format!(
                    "failed to create Runtime Server diagnostic directory {}: {error}",
                    parent.display()
                )
            })?;
        }
        let journal_path = receipt_path.with_file_name("runtime-server-diagnostic-journal.v1.json");
        let (initial_sequence, initial_journal) = load_journal(&journal_path).await?;
        let worker_count = tokio::runtime::Handle::current().metrics().num_workers();
        let capacity = worker_count.saturating_mul(32).clamp(256, 4_096);
        let (sender, mut receiver) = mpsc::channel(capacity);
        let publisher =
            crate::runtime_server_observability::RuntimeServerEventPublisher::new(sender, capacity);
        let task = tokio::spawn(async move {
            let mut sequence = initial_sequence;
            let mut journal = initial_journal;
            let mut persist = tokio::time::interval(DIAGNOSTIC_PERSIST_INTERVAL);
            persist.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
            // Consume the immediate first tick: persistence is driven by a
            // real interval or channel close, never by actor startup.
            persist.tick().await;
            let mut dirty = false;

            loop {
                tokio::select! {
                    event = receiver.recv() => match event {
                        Some(event) => {
                            sequence = sequence.saturating_add(1);
                            append_journal_event(&mut journal, sequence, event)?;
                            dirty = true;
                        }
                        None => {
                            if dirty {
                                persist_diagnostics(&receipt_path, &journal).await?;
                            }
                            break;
                        }
                    },
                    _ = persist.tick(), if dirty => {
                        persist_diagnostics(&receipt_path, &journal).await?;
                        dirty = false;
                    }
                }
            }
            Ok(())
        });
        Ok((publisher, Self { task }))
    }

    pub async fn join(self) -> Result<(), String> {
        self.task
            .await
            .map_err(|error| format!("Runtime Server diagnostic lane failed: {error}"))?
    }
}

async fn persist_diagnostics(
    receipt_path: &Path,
    journal: &std::collections::VecDeque<RuntimeServerDiagnosticJournalEntry>,
) -> Result<(), String> {
    let latest = journal.back().ok_or_else(|| {
        "Runtime Server diagnostic persistence requested without an event".to_owned()
    })?;
    let journal_path = receipt_path.with_file_name("runtime-server-diagnostic-journal.v1.json");
    write_latest_journal_event(receipt_path, latest).await?;
    write_journal(&journal_path, journal).await
}

fn append_journal_event(
    journal: &mut std::collections::VecDeque<RuntimeServerDiagnosticJournalEntry>,
    sequence: u64,
    event: RuntimeServerEvent,
) -> Result<(), String> {
    if journal.len() == DIAGNOSTIC_JOURNAL_CAPACITY {
        journal.pop_front();
    }
    journal.push_back(RuntimeServerDiagnosticJournalEntry {
        sequence,
        observed_unix_millis: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis()
            .try_into()
            .unwrap_or(u64::MAX),
        event: serde_json::to_value(event).map_err(|error| {
            format!("failed to encode Runtime Server diagnostic event: {error}")
        })?,
    });
    Ok(())
}

async fn load_journal(
    path: &Path,
) -> Result<
    (
        u64,
        std::collections::VecDeque<RuntimeServerDiagnosticJournalEntry>,
    ),
    String,
> {
    let bytes = match tokio::fs::read(path).await {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok((
                0,
                std::collections::VecDeque::with_capacity(DIAGNOSTIC_JOURNAL_CAPACITY),
            ));
        }
        Err(error) => {
            return Err(format!(
                "failed to read Runtime Server diagnostic journal {}: {error}",
                path.display()
            ));
        }
    };
    let snapshot: RuntimeServerDiagnosticJournalSnapshot =
        serde_json::from_slice(&bytes).map_err(|error| {
            format!(
                "failed to decode Runtime Server diagnostic journal {}: {error}",
                path.display()
            )
        })?;
    if snapshot.schema_id != DIAGNOSTIC_JOURNAL_SCHEMA_ID || snapshot.schema_version != "1" {
        return Err(format!(
            "unsupported Runtime Server diagnostic journal contract at {}: schemaId={} schemaVersion={}",
            path.display(),
            snapshot.schema_id,
            snapshot.schema_version
        ));
    }
    if snapshot.events.len() > DIAGNOSTIC_JOURNAL_CAPACITY {
        return Err(format!(
            "Runtime Server diagnostic journal at {} exceeds capacity {}",
            path.display(),
            DIAGNOSTIC_JOURNAL_CAPACITY
        ));
    }
    let mut previous_sequence = 0_u64;
    for entry in &snapshot.events {
        if entry.sequence <= previous_sequence {
            return Err(format!(
                "Runtime Server diagnostic journal at {} has non-increasing sequence {} after {}",
                path.display(),
                entry.sequence,
                previous_sequence
            ));
        }
        previous_sequence = entry.sequence;
    }
    Ok((previous_sequence, snapshot.events))
}

async fn write_journal(
    path: &Path,
    events: &std::collections::VecDeque<RuntimeServerDiagnosticJournalEntry>,
) -> Result<(), String> {
    let bytes = serde_json::to_vec(&RuntimeServerDiagnosticJournalReceipt {
        schema_id: DIAGNOSTIC_JOURNAL_SCHEMA_ID,
        schema_version: "1",
        events,
    })
    .map_err(|error| format!("failed to encode Runtime Server diagnostic journal: {error}"))?;
    let temporary = path.with_extension("json.tmp");
    tokio::fs::write(&temporary, bytes).await.map_err(|error| {
        format!(
            "failed to write Runtime Server diagnostic journal {}: {error}",
            temporary.display()
        )
    })?;
    tokio::fs::rename(&temporary, path).await.map_err(|error| {
        format!(
            "failed to publish Runtime Server diagnostic journal {}: {error}",
            path.display()
        )
    })
}

async fn write_latest_journal_event(
    path: &Path,
    latest: &RuntimeServerDiagnosticJournalEntry,
) -> Result<(), String> {
    let bytes = serde_json::to_vec(&RuntimeServerDiagnosticReceipt {
        schema_id: DIAGNOSTIC_SCHEMA_ID,
        schema_version: "1",
        sequence: latest.sequence,
        event: &latest.event,
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
