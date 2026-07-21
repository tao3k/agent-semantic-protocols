//! Durable resident dispatch claim, rebind, replay, and terminal-receipt state.

use std::path::Path;

use crate::engine::turso_statement::{
    execute_turso_operation, execute_turso_statement, run_turso_operation,
};

use super::core::{connect_turso_agent_session_registry, turso_session_by_name};
use super::types::{
    AgentSessionDispatchClaimResult, AgentSessionDispatchLeaseRecord,
    agent_session_message_target_is_live_bound,
};

impl super::core::AgentSessionRegistry {
    /// Read one exact dispatch lease for capability validation.
    pub fn dispatch_lease(
        &self,
        project_id: &str,
        root_session_id: &str,
        name: &str,
        dispatch_identity: &str,
    ) -> Result<Option<AgentSessionDispatchLeaseRecord>, String> {
        super::core::block_on_agent_session_registry_async(async {
            let connection = connect_turso_agent_session_registry(self.db_path()).await?;
            turso_dispatch_lease_by_identity(
                &connection,
                project_id,
                root_session_id,
                name,
                dispatch_identity,
            )
            .await
        })
    }
}

pub(super) async fn bootstrap_turso_agent_dispatch_schema(
    connection: &turso::Connection,
) -> Result<(), String> {
    execute_turso_statement(
        connection,
        "CREATE TABLE IF NOT EXISTS asp_agent_dispatch_leases (
            project_id TEXT NOT NULL,
            root_session_id TEXT NOT NULL,
            name TEXT NOT NULL,
            dispatch_identity TEXT NOT NULL,
            command_digest TEXT NOT NULL,
            delivery_target_id TEXT,
            delivery_generation_id TEXT,
            status TEXT NOT NULL,
            attempt_count INTEGER NOT NULL,
            created_at INTEGER NOT NULL,
            updated_at INTEGER NOT NULL,
            completed_at INTEGER,
            evidence_ref TEXT,
            PRIMARY KEY(project_id, root_session_id, name, dispatch_identity)
        )",
        "failed to initialize Turso agent dispatch lease schema",
    )
    .await?;
    ensure_turso_agent_dispatch_generation_column(connection).await?;
    execute_turso_statement(
        connection,
        "CREATE INDEX IF NOT EXISTS idx_asp_agent_dispatch_leases_target
            ON asp_agent_dispatch_leases(project_id, root_session_id, name, delivery_target_id)",
        "failed to initialize Turso agent dispatch lease index",
    )
    .await
}

async fn ensure_turso_agent_dispatch_generation_column(
    connection: &turso::Connection,
) -> Result<(), String> {
    let mut rows = run_turso_operation(
        || async {
            connection
                .query("PRAGMA table_info(asp_agent_dispatch_leases)", ())
                .await
                .map_err(|error| error.to_string())
        },
        "failed to inspect Turso agent dispatch schema",
    )
    .await?;
    while let Some(row) = rows
        .next()
        .await
        .map_err(|error| format!("failed to inspect Turso agent dispatch column: {error}"))?
    {
        let column_name = row
            .get::<String>(1)
            .map_err(|error| format!("failed to read Turso agent dispatch column: {error}"))?;
        if column_name == "delivery_generation_id" {
            return Ok(());
        }
    }
    execute_turso_statement(
        connection,
        "ALTER TABLE asp_agent_dispatch_leases ADD COLUMN delivery_generation_id TEXT",
        "failed to migrate Turso agent dispatch generation",
    )
    .await
}

const AGENT_DISPATCH_LEASE_SELECT: &str = "SELECT
    project_id,
    root_session_id,
    name,
    dispatch_identity,
    command_digest,
    delivery_target_id,
    delivery_generation_id,
    status,
    attempt_count,
    created_at,
    updated_at,
    completed_at,
    evidence_ref
 FROM asp_agent_dispatch_leases";

fn agent_dispatch_lease_from_turso_row(
    row: &turso::Row,
) -> Result<AgentSessionDispatchLeaseRecord, String> {
    Ok(AgentSessionDispatchLeaseRecord {
        project_id: row
            .get::<String>(0)
            .map_err(|error| format!("failed to read dispatch project id: {error}"))?,
        root_session_id: row
            .get::<String>(1)
            .map_err(|error| format!("failed to read dispatch root session id: {error}"))?,
        name: row
            .get::<String>(2)
            .map_err(|error| format!("failed to read dispatch resident name: {error}"))?,
        dispatch_identity: row
            .get::<String>(3)
            .map_err(|error| format!("failed to read dispatch identity: {error}"))?,
        command_digest: row
            .get::<String>(4)
            .map_err(|error| format!("failed to read dispatch command digest: {error}"))?,
        delivery_target_id: row
            .get::<Option<String>>(5)
            .map_err(|error| format!("failed to read dispatch delivery target: {error}"))?,
        delivery_generation_id: row
            .get::<Option<String>>(6)
            .map_err(|error| format!("failed to read dispatch delivery generation: {error}"))?,
        status: row
            .get::<String>(7)
            .map_err(|error| format!("failed to read dispatch status: {error}"))?,
        attempt_count: row
            .get::<i64>(8)
            .map_err(|error| format!("failed to read dispatch attempt count: {error}"))?
            .max(0)
            .min(i64::from(u32::MAX)) as u32,
        created_at: row
            .get::<i64>(9)
            .map_err(|error| format!("failed to read dispatch created time: {error}"))?,
        updated_at: row
            .get::<i64>(10)
            .map_err(|error| format!("failed to read dispatch updated time: {error}"))?,
        completed_at: row
            .get::<Option<i64>>(11)
            .map_err(|error| format!("failed to read dispatch completed time: {error}"))?,
        evidence_ref: row
            .get::<Option<String>>(12)
            .map_err(|error| format!("failed to read dispatch evidence ref: {error}"))?,
    })
}

pub(super) async fn turso_dispatch_lease_by_identity(
    connection: &turso::Connection,
    project_id: &str,
    root_session_id: &str,
    name: &str,
    dispatch_identity: &str,
) -> Result<Option<AgentSessionDispatchLeaseRecord>, String> {
    let sql = format!(
        "{AGENT_DISPATCH_LEASE_SELECT}
         WHERE project_id = ?1
           AND root_session_id = ?2
           AND name = ?3
           AND dispatch_identity = ?4"
    );
    let mut rows = run_turso_operation(
        || async {
            connection
                .query(&sql, (project_id, root_session_id, name, dispatch_identity))
                .await
                .map_err(|error| error.to_string())
        },
        "failed to read Turso agent dispatch lease",
    )
    .await?;
    let Some(row) = rows
        .next()
        .await
        .map_err(|error| format!("failed to read Turso agent dispatch lease row: {error}"))?
    else {
        return Ok(None);
    };
    agent_dispatch_lease_from_turso_row(&row).map(Some)
}

async fn rollback_agent_dispatch_transaction(
    connection: &turso::Connection,
    error: String,
) -> Result<AgentSessionDispatchClaimResult, String> {
    let _ = execute_turso_statement(
        connection,
        "ROLLBACK",
        "failed to roll back Turso agent dispatch transaction",
    )
    .await;
    Err(error)
}

pub(super) async fn turso_claim_dispatch(
    db_path: &Path,
    request: super::types::AgentSessionDispatchClaimRequest<'_>,
) -> Result<super::types::AgentSessionDispatchClaimResult, String> {
    let native_delivery_target;
    let native_delivery_generation;
    let (delivery_target_id, delivery_generation_id) =
        if let Some(target) = request.delivery_target_override {
            (target, target)
        } else {
            let session = turso_session_by_name(
                db_path,
                request.project_id,
                request.root_session_id,
                request.name,
            )
            .await?
            .ok_or_else(|| "resident dispatch requires a registered session".to_string())?;
            if !agent_session_message_target_is_live_bound(&session, request.root_session_id) {
                return Err("resident dispatch requires a verified live message target".to_string());
            }
            native_delivery_target = session
                .message_target_id
                .ok_or_else(|| "live-bound resident has no message target".to_string())?;
            native_delivery_generation = session.session_id;
            (
                native_delivery_target.as_str(),
                native_delivery_generation.as_str(),
            )
        };
    let connection = connect_turso_agent_session_registry(db_path).await?;
    execute_turso_statement(
        &connection,
        "BEGIN IMMEDIATE",
        "failed to begin Turso agent dispatch claim",
    )
    .await?;

    let claim = async {
        let existing = turso_dispatch_lease_by_identity(
            &connection,
            request.project_id,
            request.root_session_id,
            request.name,
            request.dispatch_identity,
        )
        .await?;
        let action = dispatch_claim_action(
            existing.as_ref(),
            request.dispatch_identity,
            request.command_digest,
            delivery_target_id,
            delivery_generation_id,
        )?;
        match (action, existing.is_some()) {
            ("send", true) => {
                execute_turso_operation(
                    || async {
                        connection
                            .execute(
                                "UPDATE asp_agent_dispatch_leases
                                 SET delivery_target_id = ?1,
                                     delivery_generation_id = ?2,
                                     status = 'in-flight',
                                     attempt_count = attempt_count + 1,
                                     updated_at = ?3,
                                     completed_at = NULL,
                                     evidence_ref = NULL
                                 WHERE project_id = ?4
                                   AND root_session_id = ?5
                                   AND name = ?6
                                   AND dispatch_identity = ?7
                                   AND status = 'orphaned-awaiting-rebind'",
                                (
                                    delivery_target_id,
                                    delivery_generation_id,
                                    request.now,
                                    request.project_id,
                                    request.root_session_id,
                                    request.name,
                                    request.dispatch_identity,
                                ),
                            )
                            .await
                            .map_err(|error| error.to_string())
                    },
                    "failed to rebind Turso agent dispatch lease",
                )
                .await?;
            }
            ("send", false) => {
                execute_turso_operation(
                    || async {
                        connection
                            .execute(
                                "INSERT INTO asp_agent_dispatch_leases (
                                    project_id,
                                    root_session_id,
                                    name,
                                    dispatch_identity,
                                    command_digest,
                                    delivery_target_id,
                                    delivery_generation_id,
                                    status,
                                    attempt_count,
                                    created_at,
                                    updated_at
                                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 'in-flight', 1, ?8, ?8)",
                                (
                                    request.project_id,
                                    request.root_session_id,
                                    request.name,
                                    request.dispatch_identity,
                                    request.command_digest,
                                    delivery_target_id,
                                    delivery_generation_id,
                                    request.now,
                                ),
                            )
                            .await
                            .map_err(|error| error.to_string())
                    },
                    "failed to create Turso agent dispatch lease",
                )
                .await?;
            }
            _ => {}
        }
        let lease = turso_dispatch_lease_by_identity(
            &connection,
            request.project_id,
            request.root_session_id,
            request.name,
            request.dispatch_identity,
        )
        .await?
        .ok_or_else(|| "claimed agent dispatch lease was not readable".to_string())?;
        Ok(AgentSessionDispatchClaimResult {
            action: action.to_string(),
            lease,
        })
    }
    .await;

    let result = match claim {
        Ok(result) => result,
        Err(error) => return rollback_agent_dispatch_transaction(&connection, error).await,
    };
    execute_turso_statement(
        &connection,
        "COMMIT",
        "failed to commit Turso agent dispatch claim",
    )
    .await?;
    Ok(result)
}

fn dispatch_claim_action(
    existing: Option<&AgentSessionDispatchLeaseRecord>,
    dispatch_identity: &str,
    command_digest: &str,
    delivery_target_id: &str,
    delivery_generation_id: &str,
) -> Result<&'static str, String> {
    match existing {
        Some(lease) if lease.command_digest != command_digest => Err(format!(
            "dispatch identity digest mismatch: identity={dispatch_identity} stored={} requested={command_digest}",
            lease.command_digest
        )),
        Some(lease) if lease.status == "terminal" => Ok("complete"),
        Some(lease)
            if lease.status == "in-flight"
                && lease.delivery_generation_id.as_deref() == Some(delivery_generation_id)
                && lease.delivery_target_id.as_deref() == Some(delivery_target_id) =>
        {
            Ok("wait")
        }
        Some(lease)
            if lease.status == "orphaned-awaiting-rebind"
                && lease.delivery_generation_id.as_deref() != Some(delivery_generation_id) =>
        {
            Ok("send")
        }
        Some(lease) => Err(format!(
            "dispatch is not deliverable in the current generation: identity={dispatch_identity} status={} storedGeneration={} requestedGeneration={delivery_generation_id}",
            lease.status,
            lease.delivery_generation_id.as_deref().unwrap_or("none"),
        )),
        None => Ok("send"),
    }
}

#[cfg(test)]
#[path = "../../tests/unit/agent_session_dispatch_claim.rs"]
mod dispatch_claim_tests;

pub(super) async fn turso_complete_dispatch(
    db_path: &Path,
    request: super::types::AgentSessionDispatchCompleteRequest<'_>,
) -> Result<super::types::AgentSessionDispatchLeaseRecord, String> {
    let connection = connect_turso_agent_session_registry(db_path).await?;
    let lease = turso_dispatch_lease_by_identity(
        &connection,
        request.project_id,
        request.root_session_id,
        request.name,
        request.dispatch_identity,
    )
    .await?
    .ok_or_else(|| "cannot complete an unknown agent dispatch identity".to_string())?;
    if lease.command_digest != request.command_digest {
        return Err(format!(
            "dispatch identity digest mismatch: identity={} stored={} requested={}",
            request.dispatch_identity, lease.command_digest, request.command_digest
        ));
    }
    if lease.status != "terminal" {
        execute_turso_operation(
            || async {
                connection
                    .execute(
                        "UPDATE asp_agent_dispatch_leases
                         SET status = 'terminal',
                             updated_at = ?1,
                             completed_at = ?1,
                             evidence_ref = ?2
                         WHERE project_id = ?3
                           AND root_session_id = ?4
                           AND name = ?5
                           AND dispatch_identity = ?6",
                        (
                            request.now,
                            request.evidence_ref,
                            request.project_id,
                            request.root_session_id,
                            request.name,
                            request.dispatch_identity,
                        ),
                    )
                    .await
                    .map_err(|error| error.to_string())
            },
            "failed to complete Turso agent dispatch lease",
        )
        .await?;
    }
    turso_dispatch_lease_by_identity(
        &connection,
        request.project_id,
        request.root_session_id,
        request.name,
        request.dispatch_identity,
    )
    .await?
    .ok_or_else(|| "completed agent dispatch lease was not readable".to_string())
}

pub(super) async fn turso_mark_dispatch_orphaned(
    db_path: &Path,
    request: super::types::AgentSessionDispatchMarkOrphanedRequest<'_>,
) -> Result<super::types::AgentSessionDispatchLeaseRecord, String> {
    let connection = connect_turso_agent_session_registry(db_path).await?;
    let lease = turso_dispatch_lease_by_identity(
        &connection,
        request.project_id,
        request.root_session_id,
        request.name,
        request.dispatch_identity,
    )
    .await?
    .ok_or_else(|| "cannot mark an unknown agent dispatch identity orphaned".to_string())?;
    if lease.command_digest != request.command_digest {
        return Err(format!(
            "dispatch identity digest mismatch: identity={} stored={} requested={}",
            request.dispatch_identity, lease.command_digest, request.command_digest
        ));
    }
    match lease.status.as_str() {
        "in-flight" => {
            execute_turso_operation(
                || async {
                    connection
                        .execute(
                            "UPDATE asp_agent_dispatch_leases
                             SET status = 'orphaned-awaiting-rebind',
                                 updated_at = ?1
                             WHERE project_id = ?2
                               AND root_session_id = ?3
                               AND name = ?4
                               AND dispatch_identity = ?5
                               AND status = 'in-flight'",
                            (
                                request.now,
                                request.project_id,
                                request.root_session_id,
                                request.name,
                                request.dispatch_identity,
                            ),
                        )
                        .await
                        .map_err(|error| error.to_string())
                },
                "failed to mark Turso agent dispatch lease orphaned",
            )
            .await?;
        }
        "orphaned-awaiting-rebind" | "terminal" => {}
        other => {
            return Err(format!(
                "dispatch is not orphanable: identity={} status={other}",
                request.dispatch_identity
            ));
        }
    }
    turso_dispatch_lease_by_identity(
        &connection,
        request.project_id,
        request.root_session_id,
        request.name,
        request.dispatch_identity,
    )
    .await?
    .ok_or_else(|| "orphaned agent dispatch lease was not readable".to_string())
}
