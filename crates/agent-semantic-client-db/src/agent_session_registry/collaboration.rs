//! Runtime-owned diagnostic projection of Codex Collaboration Host facts.

use std::path::{Path, PathBuf};
use std::sync::Arc;

#[cfg(unix)]
use std::os::unix::fs::FileTypeExt as _;

use agent_semantic_config::CollaborationLiveAgentSnapshot;
use serde::{Deserialize, Serialize};

use super::{AgentSessionRegistry, core::connect_turso_agent_session_registry};

const DEFAULT_ARCHIVE_RETENTION_MILLIS: u64 = 7 * 24 * 60 * 60 * 1_000;

struct CollaborationInboxSocket {
    path: PathBuf,
}

impl Drop for CollaborationInboxSocket {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CollaborationSnapshotPersistenceReceipt {
    pub schema_id: String,
    pub schema_version: String,
    pub workspace_root: String,
    pub root_session_id: String,
    pub snapshot_digest: String,
    pub observed_agent_count: usize,
    pub archived_agent_count: u64,
    pub pruned_agent_count: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CollaborationAgentObservation {
    pub workspace_root: String,
    pub root_session_id: String,
    pub agent_path: String,
    pub status_kind: String,
    pub status_json: String,
    pub observed_at_unix_ms: u64,
    pub archived_at_unix_ms: Option<u64>,
}

pub(in crate::agent_session_registry) async fn bootstrap_collaboration_snapshot_schema(
    connection: &turso::Connection,
) -> Result<(), String> {
    connection
        .execute(
            "CREATE TABLE IF NOT EXISTS asp_collaboration_agent_observations (
                workspace_root TEXT NOT NULL,
                root_session_id TEXT NOT NULL,
                agent_path TEXT NOT NULL,
                status_kind TEXT NOT NULL,
                status_json TEXT NOT NULL CHECK(json_valid(status_json)),
                observed_at_unix_ms INTEGER NOT NULL,
                archived_at_unix_ms INTEGER,
                PRIMARY KEY(workspace_root, root_session_id, agent_path)
            )",
            (),
        )
        .await
        .map_err(|error| format!("initialize Collaboration observation schema: {error}"))?;
    connection
        .execute(
            "CREATE INDEX IF NOT EXISTS idx_asp_collaboration_observations_root
             ON asp_collaboration_agent_observations(workspace_root, root_session_id, archived_at_unix_ms)",
            (),
        )
        .await
        .map_err(|error| format!("initialize Collaboration observation root index: {error}"))?;
    Ok(())
}

impl AgentSessionRegistry {
    /// Persist one complete `collaboration.list_agents` Host observation.
    ///
    /// The Host JSON remains lifecycle authority. This table is a diagnostic
    /// projection used for routing guidance, expiry, and audit only.
    pub async fn persist_collaboration_snapshot_from_runtime_owner(
        &self,
        snapshot: &CollaborationLiveAgentSnapshot,
    ) -> Result<CollaborationSnapshotPersistenceReceipt, String> {
        if !Self::is_runtime_server_owner_process() {
            return Err(
                "Collaboration snapshot persistence requires the Runtime Server DB owner"
                    .to_owned(),
            );
        }
        snapshot.validate()?;
        persist_snapshot(self.db_path(), snapshot, DEFAULT_ARCHIVE_RETENTION_MILLIS).await
    }

    pub async fn query_collaboration_agents_from_runtime_owner(
        &self,
        workspace_root: &str,
        root_session_id: &str,
        include_archived: bool,
    ) -> Result<Vec<CollaborationAgentObservation>, String> {
        if !Self::is_runtime_server_owner_process() {
            return Err(
                "Collaboration observation query requires the Runtime Server DB owner".to_owned(),
            );
        }
        query_observations(
            self.db_path(),
            workspace_root,
            root_session_id,
            include_archived,
        )
        .await
    }
}

/// Run the Runtime-owned consumer for Hook PostTool snapshots.
///
/// The Hook only performs an atomic file publication plus a non-blocking local
/// datagram. Turso parsing and lifecycle updates stay in the Runtime DB owner.
pub async fn run_collaboration_snapshot_inbox(
    registry: Arc<AgentSessionRegistry>,
    state_home: PathBuf,
    mut shutdown: tokio::sync::watch::Receiver<bool>,
) -> Result<(), String> {
    if !AgentSessionRegistry::is_runtime_server_owner_process() {
        return Err("Collaboration inbox requires the Runtime Server DB owner".to_owned());
    }
    let directory = state_home.join("agents/live");
    tokio::fs::create_dir_all(&directory)
        .await
        .map_err(|error| format!("create Collaboration snapshot inbox: {error}"))?;
    let socket_path = directory.join("notify.sock");
    if socket_path.as_os_str().as_encoded_bytes().len() > 100 {
        return Err(format!(
            "Collaboration snapshot notification socket path exceeds Unix limit: {}",
            socket_path.display()
        ));
    }
    match tokio::fs::symlink_metadata(&socket_path).await {
        Ok(metadata) if metadata.file_type().is_socket() => {
            tokio::fs::remove_file(&socket_path)
                .await
                .map_err(|error| format!("remove stale Collaboration socket: {error}"))?;
        }
        Ok(_) => {
            return Err(format!(
                "Collaboration notification path is not a socket: {}",
                socket_path.display()
            ));
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(format!("inspect Collaboration socket: {error}")),
    }
    let socket = tokio::net::UnixDatagram::bind(&socket_path)
        .map_err(|error| format!("bind Collaboration snapshot notification socket: {error}"))?;
    let _socket_guard = CollaborationInboxSocket { path: socket_path };
    drain_snapshot_inbox(&registry, &directory).await?;
    let mut notification = [0_u8; 1];
    loop {
        tokio::select! {
            changed = shutdown.changed() => {
                if changed.is_err() || *shutdown.borrow() {
                    return Ok(());
                }
            }
            received = socket.recv(&mut notification) => {
                received.map_err(|error| format!("receive Collaboration snapshot notification: {error}"))?;
                drain_snapshot_inbox(&registry, &directory).await?;
            }
        }
    }
}

async fn drain_snapshot_inbox(
    registry: &AgentSessionRegistry,
    directory: &Path,
) -> Result<(), String> {
    let mut entries = tokio::fs::read_dir(directory)
        .await
        .map_err(|error| format!("read Collaboration snapshot inbox: {error}"))?;
    while let Some(entry) = entries
        .next_entry()
        .await
        .map_err(|error| format!("advance Collaboration snapshot inbox: {error}"))?
    {
        let path = entry.path();
        let is_snapshot = path
            .extension()
            .is_some_and(|extension| extension == "json")
            && !path
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with(".candidate-"));
        if !is_snapshot {
            continue;
        }
        let bytes = tokio::fs::read(&path)
            .await
            .map_err(|error| format!("read Collaboration snapshot {}: {error}", path.display()))?;
        let snapshot: CollaborationLiveAgentSnapshot =
            serde_json::from_slice(&bytes).map_err(|error| {
                format!("decode Collaboration snapshot {}: {error}", path.display())
            })?;
        registry
            .persist_collaboration_snapshot_from_runtime_owner(&snapshot)
            .await?;
        tokio::fs::remove_file(&path).await.map_err(|error| {
            format!("retire Collaboration snapshot {}: {error}", path.display())
        })?;
    }
    Ok(())
}

async fn persist_snapshot(
    db_path: &Path,
    snapshot: &CollaborationLiveAgentSnapshot,
    archive_retention_millis: u64,
) -> Result<CollaborationSnapshotPersistenceReceipt, String> {
    let connection = connect_turso_agent_session_registry(db_path).await?;
    connection
        .execute("BEGIN IMMEDIATE", ())
        .await
        .map_err(|error| format!("begin Collaboration snapshot transaction: {error}"))?;
    let result = async {
        connection
            .execute(
                "UPDATE asp_collaboration_agent_observations
                 SET archived_at_unix_ms = ?3
                 WHERE workspace_root = ?1 AND root_session_id = ?2
                   AND archived_at_unix_ms IS NULL",
                turso::params![
                    snapshot.workspace_root.as_str(),
                    snapshot.root_session_id.as_str(),
                    snapshot.observed_at_unix_ms as i64,
                ],
            )
            .await
            .map_err(|error| format!("archive previous Collaboration snapshot: {error}"))?;
        for agent in &snapshot.agents.agents {
            let status_json = serde_json::to_string(&agent.agent_status)
                .map_err(|error| format!("encode Collaboration agent status: {error}"))?;
            let status_kind = agent
                .agent_status
                .as_str()
                .map(str::to_owned)
                .or_else(|| agent.agent_status.as_object()?.keys().next().cloned())
                .ok_or_else(|| "Collaboration agent status has no typed state".to_owned())?;
            connection
                .execute(
                    "INSERT INTO asp_collaboration_agent_observations (
                        workspace_root, root_session_id, agent_path, status_kind,
                        status_json, observed_at_unix_ms, archived_at_unix_ms
                     ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, NULL)
                     ON CONFLICT(workspace_root, root_session_id, agent_path) DO UPDATE SET
                        status_kind = excluded.status_kind,
                        status_json = excluded.status_json,
                        observed_at_unix_ms = excluded.observed_at_unix_ms,
                        archived_at_unix_ms = NULL
                     WHERE excluded.observed_at_unix_ms >= asp_collaboration_agent_observations.observed_at_unix_ms",
                    turso::params![
                        snapshot.workspace_root.as_str(),
                        snapshot.root_session_id.as_str(),
                        agent.agent_name.as_str(),
                        status_kind,
                        status_json,
                        snapshot.observed_at_unix_ms as i64,
                    ],
                )
                .await
                .map_err(|error| format!("persist Collaboration agent observation: {error}"))?;
        }
        let mut archived_rows = connection
            .query(
                "SELECT COUNT(*) FROM asp_collaboration_agent_observations
                 WHERE workspace_root = ?1 AND root_session_id = ?2
                   AND archived_at_unix_ms = ?3",
                turso::params![
                    snapshot.workspace_root.as_str(),
                    snapshot.root_session_id.as_str(),
                    snapshot.observed_at_unix_ms as i64,
                ],
            )
            .await
            .map_err(|error| format!("count archived Collaboration observations: {error}"))?;
        let archived_agent_count = archived_rows
            .next()
            .await
            .map_err(|error| format!("advance archived Collaboration count: {error}"))?
            .ok_or_else(|| "archived Collaboration count is unavailable".to_owned())?
            .get::<i64>(0)
            .map_err(|error| format!("read archived Collaboration count: {error}"))?
            as u64;
        let cutoff = snapshot
            .observed_at_unix_ms
            .saturating_sub(archive_retention_millis) as i64;
        let pruned_agent_count = connection
            .execute(
                "DELETE FROM asp_collaboration_agent_observations
                 WHERE archived_at_unix_ms IS NOT NULL AND archived_at_unix_ms < ?1",
                [cutoff],
            )
            .await
            .map_err(|error| format!("prune archived Collaboration observations: {error}"))?;
        Ok::<_, String>((archived_agent_count, pruned_agent_count))
    }
    .await;
    match result {
        Ok((archived_agent_count, pruned_agent_count)) => {
            connection
                .execute("COMMIT", ())
                .await
                .map_err(|error| format!("commit Collaboration snapshot transaction: {error}"))?;
            let bytes = serde_json::to_vec(snapshot)
                .map_err(|error| format!("encode Collaboration snapshot digest: {error}"))?;
            Ok(CollaborationSnapshotPersistenceReceipt {
                schema_id: "agent.semantic-protocols.collaboration-snapshot-persistence-receipt"
                    .to_owned(),
                schema_version: "1".to_owned(),
                workspace_root: snapshot.workspace_root.clone(),
                root_session_id: snapshot.root_session_id.clone(),
                snapshot_digest: format!("blake3-256:{}", blake3::hash(&bytes).to_hex()),
                observed_agent_count: snapshot.agents.agents.len(),
                archived_agent_count,
                pruned_agent_count,
            })
        }
        Err(error) => {
            let _ = connection.execute("ROLLBACK", ()).await;
            Err(error)
        }
    }
}

async fn query_observations(
    db_path: &Path,
    workspace_root: &str,
    root_session_id: &str,
    include_archived: bool,
) -> Result<Vec<CollaborationAgentObservation>, String> {
    let connection = connect_turso_agent_session_registry(db_path).await?;
    let sql = if include_archived {
        "SELECT workspace_root, root_session_id, agent_path, status_kind, status_json,
                observed_at_unix_ms, archived_at_unix_ms
         FROM asp_collaboration_agent_observations
         WHERE workspace_root = ?1 AND root_session_id = ?2 ORDER BY agent_path"
    } else {
        "SELECT workspace_root, root_session_id, agent_path, status_kind, status_json,
                observed_at_unix_ms, archived_at_unix_ms
         FROM asp_collaboration_agent_observations
         WHERE workspace_root = ?1 AND root_session_id = ?2
           AND archived_at_unix_ms IS NULL ORDER BY agent_path"
    };
    let mut rows = connection
        .query(sql, turso::params![workspace_root, root_session_id])
        .await
        .map_err(|error| format!("query Collaboration observations: {error}"))?;
    let mut observations = Vec::new();
    while let Some(row) = rows
        .next()
        .await
        .map_err(|error| format!("advance Collaboration observations: {error}"))?
    {
        observations.push(CollaborationAgentObservation {
            workspace_root: row
                .get(0)
                .map_err(|error| format!("read workspaceRoot: {error}"))?,
            root_session_id: row
                .get(1)
                .map_err(|error| format!("read rootSessionId: {error}"))?,
            agent_path: row
                .get(2)
                .map_err(|error| format!("read agentPath: {error}"))?,
            status_kind: row
                .get(3)
                .map_err(|error| format!("read statusKind: {error}"))?,
            status_json: row
                .get(4)
                .map_err(|error| format!("read statusJson: {error}"))?,
            observed_at_unix_ms: row
                .get::<i64>(5)
                .map_err(|error| format!("read observedAt: {error}"))?
                as u64,
            archived_at_unix_ms: row
                .get::<Option<i64>>(6)
                .map_err(|error| format!("read archivedAt: {error}"))?
                .map(|value| value as u64),
        });
    }
    Ok(observations)
}
