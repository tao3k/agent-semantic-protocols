//! Transactional State Home catalog owned by Artifacts.

use std::{path::Path, time::Duration};

use serde::{Deserialize, Serialize};

use crate::{CleanupPlan, ProjectBinding, RetainedObject, RetentionLease, RetentionPlanner};

pub const STATE_HOME_CATALOG_SCHEMA_ID: &str = "agent.semantic-protocols.state-home-catalog";
pub const STATE_HOME_CATALOG_SCHEMA_VERSION: u32 = 1;

const BOOTSTRAP_SQL: &str = "
CREATE TABLE IF NOT EXISTS asp_state_home_catalog (
    singleton INTEGER PRIMARY KEY CHECK(singleton = 1),
    schema_id TEXT NOT NULL,
    schema_version INTEGER NOT NULL,
    generation INTEGER NOT NULL
);
INSERT INTO asp_state_home_catalog(singleton, schema_id, schema_version, generation)
VALUES (1, 'agent.semantic-protocols.state-home-catalog', 1, 0)
ON CONFLICT(singleton) DO NOTHING;
CREATE TABLE IF NOT EXISTS asp_project_binding (
    workspace_digest TEXT PRIMARY KEY,
    repo_digest TEXT NOT NULL,
    binding_digest TEXT NOT NULL,
    binding_json TEXT NOT NULL,
    observed_at_ms INTEGER NOT NULL
);
CREATE TABLE IF NOT EXISTS asp_retained_object (
    object_id TEXT PRIMARY KEY,
    workspace_digest TEXT NOT NULL,
    kind TEXT NOT NULL,
    last_observed_at_ms INTEGER NOT NULL,
    byte_count INTEGER NOT NULL,
    FOREIGN KEY(workspace_digest) REFERENCES asp_project_binding(workspace_digest)
);
CREATE TABLE IF NOT EXISTS asp_retention_lease (
    lease_id TEXT PRIMARY KEY,
    object_id TEXT NOT NULL,
    owner TEXT NOT NULL,
    expires_at_ms INTEGER,
    FOREIGN KEY(object_id) REFERENCES asp_retained_object(object_id) ON DELETE CASCADE
);
CREATE INDEX IF NOT EXISTS idx_asp_retention_lease_object
ON asp_retention_lease(object_id);
";

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct CatalogGeneration(u64);

impl CatalogGeneration {
    pub const fn get(self) -> u64 {
        self.0
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogObservationReceipt {
    pub schema_id: String,
    pub schema_version: u32,
    pub generation: CatalogGeneration,
    pub workspace_digest: String,
    pub object_id: String,
    pub lease_count: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CatalogObservation {
    pub binding: ProjectBinding,
    pub object: RetainedObject,
    pub leases: Vec<RetentionLease>,
    pub observed_at_ms: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogBatchReceipt {
    pub schema_id: String,
    pub schema_version: u32,
    pub generation: CatalogGeneration,
    pub observation_count: usize,
}

pub struct StateHomeCatalog {
    _database: turso::Database,
    connection: tokio::sync::Mutex<turso::Connection>,
}

impl StateHomeCatalog {
    pub async fn open(path: impl AsRef<Path>) -> Result<Self, String> {
        let path = path.as_ref();
        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent).await.map_err(|error| {
                format!(
                    "create State Home catalog directory {}: {error}",
                    parent.display()
                )
            })?;
        }
        let path = path
            .to_str()
            .ok_or_else(|| "State Home catalog path must be valid UTF-8".to_string())?;
        let database = turso::Builder::new_local(path)
            .experimental_multiprocess_wal(true)
            .build()
            .await
            .map_err(|error| format!("open State Home catalog: {error}"))?;
        let connection = database
            .connect()
            .map_err(|error| format!("connect State Home catalog: {error}"))?;
        connection
            .busy_timeout(Duration::from_secs(5))
            .map_err(|error| format!("configure State Home catalog busy timeout: {error}"))?;
        connection
            .execute_batch(BOOTSTRAP_SQL)
            .await
            .map_err(|error| format!("bootstrap State Home catalog: {error}"))?;
        validate_catalog_identity(&connection).await?;
        Ok(Self {
            _database: database,
            connection: tokio::sync::Mutex::new(connection),
        })
    }

    pub async fn observe(
        &self,
        binding: &ProjectBinding,
        object: &RetainedObject,
        leases: &[RetentionLease],
        observed_at_ms: u64,
    ) -> Result<CatalogObservationReceipt, String> {
        let observation = CatalogObservation {
            binding: binding.clone(),
            object: object.clone(),
            leases: leases.to_vec(),
            observed_at_ms,
        };
        let receipt = self
            .observe_batch(std::slice::from_ref(&observation))
            .await?;
        Ok(CatalogObservationReceipt {
            schema_id: receipt.schema_id,
            schema_version: receipt.schema_version,
            generation: receipt.generation,
            workspace_digest: binding.workspace.digest.to_string(),
            object_id: object.object_id.clone(),
            lease_count: leases.len(),
        })
    }

    pub async fn observe_batch(
        &self,
        observations: &[CatalogObservation],
    ) -> Result<CatalogBatchReceipt, String> {
        if observations.is_empty() {
            return Ok(CatalogBatchReceipt {
                schema_id: STATE_HOME_CATALOG_SCHEMA_ID.to_string(),
                schema_version: STATE_HOME_CATALOG_SCHEMA_VERSION,
                generation: self.generation().await?,
                observation_count: 0,
            });
        }
        for observation in observations {
            observation.binding.validate()?;
            validate_observation(&observation.object, &observation.leases)?;
        }
        let mut connection = self.connection.lock().await;
        let transaction = connection
            .transaction_with_behavior(turso::transaction::TransactionBehavior::Immediate)
            .await
            .map_err(|error| format!("begin State Home catalog transaction: {error}"))?;
        for observation in observations {
            let binding_json = serde_json::to_string(&observation.binding)
                .map_err(|error| format!("encode State Home project binding: {error}"))?;
            let observed_at_ms = as_i64("observedAtMs", observation.observed_at_ms)?;
            let last_observed_at_ms =
                as_i64("lastObservedAtMs", observation.object.last_observed_at_ms)?;
            let byte_count = as_i64("byteCount", observation.object.byte_count)?;
            transaction
                .execute(
                    "INSERT INTO asp_project_binding(
                    workspace_digest, repo_digest, binding_digest, binding_json, observed_at_ms
                 ) VALUES (?1, ?2, ?3, ?4, ?5)
                 ON CONFLICT(workspace_digest) DO UPDATE SET
                    repo_digest = excluded.repo_digest,
                    binding_digest = excluded.binding_digest,
                    binding_json = excluded.binding_json,
                    observed_at_ms = excluded.observed_at_ms",
                    turso::params![
                        observation.binding.workspace.digest.as_str(),
                        observation.binding.repo.digest.as_str(),
                        observation.binding.binding_digest.as_str(),
                        binding_json,
                        observed_at_ms
                    ],
                )
                .await
                .map_err(|error| format!("publish State Home project binding: {error}"))?;
            transaction
                .execute(
                    "INSERT INTO asp_retained_object(
                    object_id, workspace_digest, kind, last_observed_at_ms, byte_count
                 ) VALUES (?1, ?2, ?3, ?4, ?5)
                 ON CONFLICT(object_id) DO UPDATE SET
                    workspace_digest = excluded.workspace_digest,
                    kind = excluded.kind,
                    last_observed_at_ms = excluded.last_observed_at_ms,
                    byte_count = excluded.byte_count",
                    turso::params![
                        observation.object.object_id.as_str(),
                        observation.binding.workspace.digest.as_str(),
                        object_kind(&observation.object),
                        last_observed_at_ms,
                        byte_count
                    ],
                )
                .await
                .map_err(|error| format!("publish State Home retained object: {error}"))?;
            transaction
                .execute(
                    "DELETE FROM asp_retention_lease WHERE object_id = ?1",
                    [observation.object.object_id.as_str()],
                )
                .await
                .map_err(|error| format!("replace State Home retention leases: {error}"))?;
            for lease in &observation.leases {
                let expires_at_ms = lease
                    .expires_at_ms
                    .map(|value| as_i64("expiresAtMs", value))
                    .transpose()?;
                transaction
                    .execute(
                        "INSERT INTO asp_retention_lease(
                        lease_id, object_id, owner, expires_at_ms
                     ) VALUES (?1, ?2, ?3, ?4)",
                        turso::params![
                            lease.lease_id.as_str(),
                            lease.object_id.as_str(),
                            lease.owner.as_str(),
                            expires_at_ms
                        ],
                    )
                    .await
                    .map_err(|error| format!("publish State Home retention lease: {error}"))?;
            }
        }
        transaction
            .execute(
                "UPDATE asp_state_home_catalog SET generation = generation + 1 WHERE singleton = 1",
                (),
            )
            .await
            .map_err(|error| format!("advance State Home catalog generation: {error}"))?;
        let generation = read_generation(&transaction).await?;
        transaction
            .commit()
            .await
            .map_err(|error| format!("commit State Home catalog transaction: {error}"))?;
        Ok(CatalogBatchReceipt {
            schema_id: STATE_HOME_CATALOG_SCHEMA_ID.to_string(),
            schema_version: STATE_HOME_CATALOG_SCHEMA_VERSION,
            generation,
            observation_count: observations.len(),
        })
    }

    pub async fn generation(&self) -> Result<CatalogGeneration, String> {
        let connection = self.connection.lock().await;
        read_connection_generation(&connection).await
    }

    pub async fn plan_cleanup(
        &self,
        evaluated_at_ms: u64,
        retain_for_ms: u64,
    ) -> Result<CleanupPlan, String> {
        let connection = self.connection.lock().await;
        let mut rows = connection
            .query(
                "SELECT object_id, kind, last_observed_at_ms, byte_count
                 FROM asp_retained_object ORDER BY object_id",
                (),
            )
            .await
            .map_err(|error| format!("query State Home retained objects: {error}"))?;
        let mut objects = Vec::new();
        while let Some(row) = rows
            .next()
            .await
            .map_err(|error| format!("advance State Home retained object row: {error}"))?
        {
            objects.push(RetainedObject {
                object_id: row.get::<String>(0).map_err(row_error)?,
                kind: parse_object_kind(&row.get::<String>(1).map_err(row_error)?)?,
                last_observed_at_ms: from_i64(
                    "lastObservedAtMs",
                    row.get::<i64>(2).map_err(row_error)?,
                )?,
                byte_count: from_i64("byteCount", row.get::<i64>(3).map_err(row_error)?)?,
            });
        }
        let mut rows = connection
            .query(
                "SELECT lease_id, object_id, owner, expires_at_ms
                 FROM asp_retention_lease ORDER BY lease_id",
                (),
            )
            .await
            .map_err(|error| format!("query State Home retention leases: {error}"))?;
        let mut leases = Vec::new();
        while let Some(row) = rows
            .next()
            .await
            .map_err(|error| format!("advance State Home retention lease row: {error}"))?
        {
            let expires_at_ms = row.get::<Option<i64>>(3).map_err(row_error)?;
            leases.push(RetentionLease {
                lease_id: row.get::<String>(0).map_err(row_error)?,
                object_id: row.get::<String>(1).map_err(row_error)?,
                owner: row.get::<String>(2).map_err(row_error)?,
                expires_at_ms: expires_at_ms
                    .map(|value| from_i64("expiresAtMs", value))
                    .transpose()?,
            });
        }
        RetentionPlanner::new(evaluated_at_ms, retain_for_ms).plan(objects, &leases)
    }
}

async fn validate_catalog_identity(connection: &turso::Connection) -> Result<(), String> {
    let mut rows = connection
        .query(
            "SELECT schema_id, schema_version FROM asp_state_home_catalog WHERE singleton = 1",
            (),
        )
        .await
        .map_err(|error| format!("query State Home catalog identity: {error}"))?;
    let row = rows
        .next()
        .await
        .map_err(|error| format!("advance State Home catalog identity: {error}"))?
        .ok_or_else(|| "State Home catalog identity is missing".to_string())?;
    let schema_id = row.get::<String>(0).map_err(row_error)?;
    let schema_version = row.get::<i64>(1).map_err(row_error)?;
    if schema_id != STATE_HOME_CATALOG_SCHEMA_ID
        || schema_version != i64::from(STATE_HOME_CATALOG_SCHEMA_VERSION)
    {
        return Err("State Home catalog schema identity mismatch".to_string());
    }
    Ok(())
}

async fn read_generation(
    connection: &turso::transaction::Transaction<'_>,
) -> Result<CatalogGeneration, String> {
    let mut rows = connection
        .query(
            "SELECT generation FROM asp_state_home_catalog WHERE singleton = 1",
            (),
        )
        .await
        .map_err(|error| format!("query State Home catalog generation: {error}"))?;
    let row = rows
        .next()
        .await
        .map_err(|error| format!("advance State Home catalog generation: {error}"))?
        .ok_or_else(|| "State Home catalog generation is missing".to_string())?;
    Ok(CatalogGeneration(from_i64(
        "catalogGeneration",
        row.get::<i64>(0).map_err(row_error)?,
    )?))
}

async fn read_connection_generation(
    connection: &turso::Connection,
) -> Result<CatalogGeneration, String> {
    let mut rows = connection
        .query(
            "SELECT generation FROM asp_state_home_catalog WHERE singleton = 1",
            (),
        )
        .await
        .map_err(|error| format!("query State Home catalog generation: {error}"))?;
    let row = rows
        .next()
        .await
        .map_err(|error| format!("advance State Home catalog generation: {error}"))?
        .ok_or_else(|| "State Home catalog generation is missing".to_string())?;
    Ok(CatalogGeneration(from_i64(
        "catalogGeneration",
        row.get::<i64>(0).map_err(row_error)?,
    )?))
}

fn validate_observation(object: &RetainedObject, leases: &[RetentionLease]) -> Result<(), String> {
    if object.object_id.is_empty() {
        return Err("retained object id must not be empty".to_string());
    }
    for lease in leases {
        if lease.object_id != object.object_id {
            return Err(format!(
                "retention lease {} targets a different object",
                lease.lease_id
            ));
        }
    }
    RetentionPlanner::new(object.last_observed_at_ms, 0)
        .plan(vec![object.clone()], leases)
        .map(|_| ())
}

fn object_kind(object: &RetainedObject) -> &'static str {
    match object.kind {
        crate::RetentionObjectKind::Project => "project",
        crate::RetentionObjectKind::Workspace => "workspace",
        crate::RetentionObjectKind::Artifact => "artifact",
        crate::RetentionObjectKind::ProviderBuild => "provider-build",
        crate::RetentionObjectKind::SourceSnapshot => "source-snapshot",
        crate::RetentionObjectKind::Quarantine => "quarantine",
        crate::RetentionObjectKind::Receipt => "receipt",
    }
}

fn parse_object_kind(value: &str) -> Result<crate::RetentionObjectKind, String> {
    match value {
        "project" => Ok(crate::RetentionObjectKind::Project),
        "workspace" => Ok(crate::RetentionObjectKind::Workspace),
        "artifact" => Ok(crate::RetentionObjectKind::Artifact),
        "provider-build" => Ok(crate::RetentionObjectKind::ProviderBuild),
        "source-snapshot" => Ok(crate::RetentionObjectKind::SourceSnapshot),
        "quarantine" => Ok(crate::RetentionObjectKind::Quarantine),
        "receipt" => Ok(crate::RetentionObjectKind::Receipt),
        _ => Err(format!("unknown State Home retention object kind: {value}")),
    }
}

fn as_i64(label: &str, value: u64) -> Result<i64, String> {
    i64::try_from(value).map_err(|_| format!("{label} exceeds signed storage range"))
}

fn from_i64(label: &str, value: i64) -> Result<u64, String> {
    u64::try_from(value).map_err(|_| format!("{label} must not be negative"))
}

fn row_error(error: turso::Error) -> String {
    format!("decode State Home catalog row: {error}")
}
