// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Runtime DB-owned transactional State Home catalog.

use std::{path::Path, time::Duration};

use agent_semantic_artifacts::{
    CatalogBatchReceipt, CatalogGeneration, CatalogObservation, CatalogObservationReceipt,
    CleanupPlan, CleanupSelection, ProjectBinding, RetainedObject, RetentionLease,
    RetentionObjectKind, RetentionPlanner, STATE_HOME_CATALOG_SCHEMA_ID,
    STATE_HOME_CATALOG_SCHEMA_VERSION, admit_state_home_catalog_batch,
    validate_state_home_catalog_observations,
};

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
            return admit_state_home_catalog_batch(self.generation().await?, observations);
        }
        validate_state_home_catalog_observations(observations)?;
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
        let expected = admit_state_home_catalog_batch(
            CatalogGeneration::new(generation.get().saturating_sub(1)),
            observations,
        )?;
        if expected.generation != generation {
            return Err("State Home catalog generation transaction diverged".to_string());
        }
        Ok(CatalogBatchReceipt {
            generation,
            ..expected
        })
    }

    pub async fn generation(&self) -> Result<CatalogGeneration, String> {
        let connection = self.connection.lock().await;
        read_connection_generation(&connection).await
    }

    /// CAS-delete exact catalog objects after their physical workspaces have
    /// been atomically staged out of the live namespace.
    pub async fn delete_objects(
        &self,
        expected_generation: CatalogGeneration,
        object_ids: &std::collections::BTreeSet<String>,
    ) -> Result<CatalogGeneration, String> {
        if object_ids.is_empty() {
            return Ok(expected_generation);
        }
        let mut connection = self.connection.lock().await;
        let transaction = connection
            .transaction_with_behavior(turso::transaction::TransactionBehavior::Immediate)
            .await
            .map_err(|error| format!("begin State Home deletion transaction: {error}"))?;
        let observed = read_generation(&transaction).await?;
        if observed != expected_generation {
            return Err(format!(
                "reasonKind=state-home-cleanup-catalog-generation-changed expected={} actual={}",
                expected_generation.get(),
                observed.get()
            ));
        }
        for object_id in object_ids {
            transaction
                .execute(
                    "DELETE FROM asp_retention_lease WHERE object_id = ?1",
                    [object_id.as_str()],
                )
                .await
                .map_err(|error| format!("delete State Home object leases: {error}"))?;
            let removed = transaction
                .execute(
                    "DELETE FROM asp_retained_object WHERE object_id = ?1",
                    [object_id.as_str()],
                )
                .await
                .map_err(|error| format!("delete State Home object: {error}"))?;
            if removed != 1 {
                return Err(format!(
                    "reasonKind=state-home-cleanup-object-not-current objectId={object_id}"
                ));
            }
        }
        transaction
            .execute(
                "DELETE FROM asp_project_binding WHERE workspace_digest NOT IN (SELECT workspace_digest FROM asp_retained_object)",
                (),
            )
            .await
            .map_err(|error| format!("delete unreferenced State Home bindings: {error}"))?;
        transaction
            .execute(
                "UPDATE asp_state_home_catalog SET generation = generation + 1 WHERE singleton = 1",
                (),
            )
            .await
            .map_err(|error| format!("advance State Home deletion generation: {error}"))?;
        let generation = read_generation(&transaction).await?;
        transaction
            .commit()
            .await
            .map_err(|error| format!("commit State Home deletion: {error}"))?;
        Ok(generation)
    }

    pub async fn plan_cleanup(
        &self,
        evaluated_at_ms: u64,
        retain_for_ms: u64,
    ) -> Result<CleanupPlan, String> {
        self.plan_cleanup_selected(evaluated_at_ms, retain_for_ms, CleanupSelection::All)
            .await
    }

    pub async fn plan_cleanup_selected(
        &self,
        evaluated_at_ms: u64,
        retain_for_ms: u64,
        selection: CleanupSelection,
    ) -> Result<CleanupPlan, String> {
        selection.validate()?;
        let connection = self.connection.lock().await;
        let mut rows = connection
            .query(
                "SELECT object_id, workspace_digest, kind, last_observed_at_ms, byte_count
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
            let object_id = row.get::<String>(0).map_err(row_error)?;
            let workspace_digest = row.get::<String>(1).map_err(row_error)?;
            let selected = match &selection {
                CleanupSelection::All => true,
                CleanupSelection::WorkspaceDigest {
                    workspace_digest: expected,
                } => &workspace_digest == expected,
                CleanupSelection::ObjectId {
                    object_id: expected,
                } => &object_id == expected,
            };
            if !selected {
                continue;
            }
            objects.push(RetainedObject {
                object_id,
                kind: parse_object_kind(&row.get::<String>(2).map_err(row_error)?)?,
                last_observed_at_ms: from_i64(
                    "lastObservedAtMs",
                    row.get::<i64>(3).map_err(row_error)?,
                )?,
                byte_count: from_i64("byteCount", row.get::<i64>(4).map_err(row_error)?)?,
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
        RetentionPlanner::new(evaluated_at_ms, retain_for_ms)
            .plan_selected(objects, &leases, selection)
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
    Ok(CatalogGeneration::new(from_i64(
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
    Ok(CatalogGeneration::new(from_i64(
        "catalogGeneration",
        row.get::<i64>(0).map_err(row_error)?,
    )?))
}

fn object_kind(object: &RetainedObject) -> &'static str {
    match object.kind {
        RetentionObjectKind::Project => "project",
        RetentionObjectKind::Workspace => "workspace",
        RetentionObjectKind::Artifact => "artifact",
        RetentionObjectKind::ProviderBuild => "provider-build",
        RetentionObjectKind::SourceSnapshot => "source-snapshot",
        RetentionObjectKind::Quarantine => "quarantine",
        RetentionObjectKind::Receipt => "receipt",
    }
}

fn parse_object_kind(value: &str) -> Result<RetentionObjectKind, String> {
    match value {
        "project" => Ok(RetentionObjectKind::Project),
        "workspace" => Ok(RetentionObjectKind::Workspace),
        "artifact" => Ok(RetentionObjectKind::Artifact),
        "provider-build" => Ok(RetentionObjectKind::ProviderBuild),
        "source-snapshot" => Ok(RetentionObjectKind::SourceSnapshot),
        "quarantine" => Ok(RetentionObjectKind::Quarantine),
        "receipt" => Ok(RetentionObjectKind::Receipt),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn cleanup_selection_targets_one_workspace_and_missing_identity_fails_closed() {
        let temporary = tempfile::tempdir().expect("catalog selection fixture");
        let first_root = temporary.path().join("first");
        let second_root = temporary.path().join("second");
        std::fs::create_dir_all(&first_root).expect("first workspace root");
        std::fs::create_dir_all(&second_root).expect("second workspace root");
        let catalog = StateHomeCatalog::open(temporary.path().join("catalog/state.turso"))
            .await
            .expect("open catalog");
        let first = ProjectBinding::resolve(None, "git-common-dir:first", &first_root)
            .expect("first binding");
        let second = ProjectBinding::resolve(None, "git-common-dir:second", &second_root)
            .expect("second binding");
        for (binding, object_id) in [(&first, "cache:first"), (&second, "cache:second")] {
            catalog
                .observe(
                    binding,
                    &RetainedObject {
                        object_id: object_id.to_string(),
                        kind: RetentionObjectKind::Workspace,
                        last_observed_at_ms: 1,
                        byte_count: 10,
                    },
                    &[],
                    1,
                )
                .await
                .expect("observe retained workspace");
        }

        let plan = catalog
            .plan_cleanup_selected(
                100,
                10,
                CleanupSelection::WorkspaceDigest {
                    workspace_digest: first.workspace.digest.to_string(),
                },
            )
            .await
            .expect("plan exact workspace cleanup");
        assert_eq!(plan.entries.len(), 1);
        assert_eq!(plan.entries[0].object.object_id, "cache:first");

        let error = catalog
            .plan_cleanup_selected(
                100,
                10,
                CleanupSelection::ObjectId {
                    object_id: "cache:missing".to_string(),
                },
            )
            .await
            .expect_err("missing exact cleanup object must fail closed");
        assert!(error.contains("state-home-cleanup-selection-no-match"));
    }

    #[tokio::test]
    async fn deletion_generation_mismatch_preserves_the_catalog_object() {
        let temporary = tempfile::tempdir().expect("catalog deletion fixture");
        let workspace_root = temporary.path().join("workspace");
        std::fs::create_dir_all(&workspace_root).expect("workspace root");
        let catalog = StateHomeCatalog::open(temporary.path().join("catalog/state.turso"))
            .await
            .expect("open catalog");
        let binding =
            ProjectBinding::resolve(None, "git-common-dir:repo", &workspace_root).expect("binding");
        let object = RetainedObject {
            object_id: "workspace:delete".to_string(),
            kind: RetentionObjectKind::Workspace,
            last_observed_at_ms: 1,
            byte_count: 10,
        };
        let observed = catalog
            .observe(&binding, &object, &[], 1)
            .await
            .expect("observe workspace");
        let selected = std::collections::BTreeSet::from([object.object_id.clone()]);

        let error = catalog
            .delete_objects(
                CatalogGeneration::new(observed.generation.get() - 1),
                &selected,
            )
            .await
            .expect_err("stale cleanup generation must fail closed");
        assert!(error.contains("state-home-cleanup-catalog-generation-changed"));

        let plan = catalog
            .plan_cleanup_selected(
                100,
                10,
                CleanupSelection::ObjectId {
                    object_id: object.object_id.clone(),
                },
            )
            .await
            .expect("stale CAS must preserve the object");
        assert_eq!(plan.entries.len(), 1);

        let committed = catalog
            .delete_objects(observed.generation, &selected)
            .await
            .expect("delete current object");
        assert_eq!(committed.get(), observed.generation.get() + 1);
        let error = catalog
            .plan_cleanup_selected(
                100,
                10,
                CleanupSelection::ObjectId {
                    object_id: object.object_id,
                },
            )
            .await
            .expect_err("committed deletion must remove the object");
        assert!(error.contains("state-home-cleanup-selection-no-match"));
    }
}
