// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Atomic V1 publication of the Codex child-session instance registry.

use std::path::{Path, PathBuf};

use fs2::FileExt as _;
use serde::{Deserialize, Serialize};

use super::schema::bootstrap_turso_agent_session_schema;
use super::types::AGENT_SESSION_REGISTRY_DB_NAME;

const RECEIPT_FILE: &str = "session-registry.current.v1.json";
const LOCK_FILE: &str = ".session-registry.publication.lock";
const ARCHIVED_V1_DB_FILE: &str = "session-registry.turso";
const SCHEMA_ID: &str = "agent.semantic-protocols.agent-session-registry-publication";

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct AgentSessionRegistryPublicationReceipt {
    schema_id: String,
    schema_version: String,
    db_schema_version: u64,
    current_db_file: String,
    archived_db_file: String,
}

impl AgentSessionRegistryPublicationReceipt {
    fn current() -> Self {
        Self {
            schema_id: SCHEMA_ID.to_owned(),
            schema_version: "1".to_owned(),
            db_schema_version: 1,
            current_db_file: AGENT_SESSION_REGISTRY_DB_NAME.to_owned(),
            archived_db_file: ARCHIVED_V1_DB_FILE.to_owned(),
        }
    }

    fn validate(&self) -> Result<(), String> {
        if self.schema_id != SCHEMA_ID
            || self.schema_version != "1"
            || self.db_schema_version != 1
            || self.current_db_file != AGENT_SESSION_REGISTRY_DB_NAME
            || self.archived_db_file != ARCHIVED_V1_DB_FILE
        {
            return Err(format!(
                "agent-session-registry-publication-not-current: schemaId={} schemaVersion={} dbSchemaVersion={} currentDbFile={} archivedDbFile={}",
                self.schema_id,
                self.schema_version,
                self.db_schema_version,
                self.current_db_file,
                self.archived_db_file,
            ));
        }
        Ok(())
    }
}

pub(super) fn physical_current_db_path(state_root: &Path) -> PathBuf {
    state_root.join(AGENT_SESSION_REGISTRY_DB_NAME)
}

pub(super) fn read_current_registry_path(state_root: &Path) -> Result<Option<PathBuf>, String> {
    let receipt_path = state_root.join(RECEIPT_FILE);
    let bytes = match std::fs::read(&receipt_path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => {
            return Err(format!(
                "failed to read agent-session registry publication {}: {error}",
                receipt_path.display()
            ));
        }
    };
    current_registry_path_from_receipt(state_root, &receipt_path, &bytes).map(Some)
}

fn current_registry_path_from_receipt(
    state_root: &Path,
    receipt_path: &Path,
    bytes: &[u8],
) -> Result<PathBuf, String> {
    let receipt: AgentSessionRegistryPublicationReceipt =
        serde_json::from_slice(bytes).map_err(|error| {
            format!(
                "agent-session-registry-publication-malformed: path={} error={error}",
                receipt_path.display()
            )
        })?;
    receipt.validate()?;
    let physical_path = physical_current_db_path(state_root);
    if !physical_path.is_file() {
        return Err(format!(
            "agent-session-registry-publication-target-missing: {}",
            physical_path.display()
        ));
    }
    Ok(physical_path)
}

pub(super) async fn ensure_current_registry_published(
    state_root: &Path,
) -> Result<PathBuf, String> {
    tokio::fs::create_dir_all(state_root)
        .await
        .map_err(|error| {
            format!(
                "failed to create agent-session registry publication root {}: {error}",
                state_root.display()
            )
        })?;
    let lock_path = state_root.join(LOCK_FILE);
    let lock = std::fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(&lock_path)
        .map_err(|error| {
            format!(
                "failed to open agent-session registry publication lock {}: {error}",
                lock_path.display()
            )
        })?;
    let lock = tokio::task::spawn_blocking(move || {
        lock.lock_exclusive().map_err(|error| {
            format!(
                "failed to acquire agent-session registry publication lock {}: {error}",
                lock_path.display()
            )
        })?;
        Ok::<_, String>(lock)
    })
    .await
    .map_err(|error| format!("agent-session registry publication lock task failed: {error}"))??;

    let result = ensure_current_registry_published_locked(state_root).await;
    lock.unlock().map_err(|error| {
        format!("failed to release agent-session registry publication lock: {error}")
    })?;
    result
}

async fn ensure_current_registry_published_locked(state_root: &Path) -> Result<PathBuf, String> {
    let receipt_path = state_root.join(RECEIPT_FILE);
    let physical_path = physical_current_db_path(state_root);
    match tokio::fs::read(&receipt_path).await {
        Ok(bytes) => {
            current_registry_path_from_receipt(state_root, &receipt_path, &bytes)?;
            bootstrap_turso_agent_session_schema(&physical_path).await?;
            return Ok(physical_path);
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => {
            return Err(format!(
                "failed to read agent-session registry publication {}: {error}",
                receipt_path.display()
            ));
        }
    }

    if physical_path.exists() {
        bootstrap_turso_agent_session_schema(&physical_path).await?;
    } else {
        let candidate = state_root.join(format!(
            ".session-registry.candidate.{}.turso",
            std::process::id()
        ));
        bootstrap_turso_agent_session_schema(&candidate).await?;
        tokio::fs::rename(&candidate, &physical_path)
            .await
            .map_err(|error| {
                format!(
                    "failed to publish current agent-session database {}: {error}",
                    physical_path.display()
                )
            })?;
    }

    let receipt = serde_json::to_vec_pretty(&AgentSessionRegistryPublicationReceipt::current())
        .map_err(|error| format!("failed to encode agent-session registry publication: {error}"))?;
    let temporary = state_root.join(format!(".{RECEIPT_FILE}.{}.tmp", std::process::id()));
    let mut file = tokio::fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&temporary)
        .await
        .map_err(|error| {
            format!(
                "failed to create agent-session registry publication candidate {}: {error}",
                temporary.display()
            )
        })?;
    use tokio::io::AsyncWriteExt as _;
    file.write_all(&receipt).await.map_err(|error| {
        format!(
            "failed to write agent-session registry publication candidate {}: {error}",
            temporary.display()
        )
    })?;
    file.sync_all().await.map_err(|error| {
        format!(
            "failed to sync agent-session registry publication candidate {}: {error}",
            temporary.display()
        )
    })?;
    drop(file);
    tokio::fs::rename(&temporary, &receipt_path)
        .await
        .map_err(|error| {
            format!(
                "failed to commit agent-session registry publication {}: {error}",
                receipt_path.display()
            )
        })?;
    Ok(physical_path)
}
