//! Turso FTS and overlay document search adapter.

use std::path::Path;

use super::turso::connect_turso_search_projection_db;
use super::turso_statement::{
    execute_turso_operation, execute_turso_prepared_statement_with_lock_retry,
    execute_turso_statement,
};

/// Feature-gated stable search document row written through the Turso adapter.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TursoClientDbSearchDocument {
    pub document_id: String,
    pub entity_id: String,
    pub selector: Option<String>,
    pub document: String,
}

/// Snapshot-bound search projection lookup state.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TursoClientDbSearchState {
    EmptyIndex,
    ColdRequired,
    Hit,
    Miss,
}

/// Search projection result bound to one expected Merkle root.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TursoClientDbSearchResult {
    pub state: TursoClientDbSearchState,
    pub hits: Vec<TursoClientDbSearchHit>,
}

/// Search hit returned from the Turso stable or overlay document tables.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TursoClientDbSearchHit {
    pub source: &'static str,
    pub document_id: String,
    pub entity_id: Option<String>,
    pub selector: Option<String>,
    pub document: String,
}

/// Atomically replace one namespace's active depth-zero projection generation.
pub async fn replace_turso_search_document_generation(
    db_path: &Path,
    namespace: &str,
    source_snapshot: &agent_semantic_content_identity::SourceSnapshotEvidence,
    documents: &[TursoClientDbSearchDocument],
) -> Result<usize, String> {
    let connection = connect_turso_search_projection_db(db_path).await?;
    replace_turso_search_document_generation_with_connection(
        &connection,
        namespace,
        source_snapshot,
        documents,
    )
    .await
}

/// Atomically replace one namespace's active generation on an existing connection.
pub(super) async fn replace_turso_search_document_generation_with_connection(
    connection: &turso::Connection,
    namespace: &str,
    source_snapshot: &agent_semantic_content_identity::SourceSnapshotEvidence,
    documents: &[TursoClientDbSearchDocument],
) -> Result<usize, String> {
    execute_turso_statement(
        connection,
        "BEGIN IMMEDIATE",
        "failed to begin Turso search projection transaction",
    )
    .await?;
    if let Err(error) = execute_turso_operation(
        || async {
            connection
                .execute(
                    "DELETE FROM asp_search_projection_document
                     WHERE namespace = ?1 AND snapshot_root = ?2",
                    (namespace, source_snapshot.root_digest.as_str()),
                )
                .await
                .map_err(|error| error.to_string())
        },
        "failed to clear the replaced Turso search projection generation",
    )
    .await
    {
        let _ = execute_turso_statement(
            connection,
            "ROLLBACK",
            "failed to rollback Turso search projection transaction after generation clear",
        )
        .await;
        return Err(error);
    }
    let mut statement = match connection
        .prepare_cached(
            "INSERT INTO asp_search_projection_document
             (namespace, snapshot_root, document_id, entity_id, selector, document)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)
             ON CONFLICT(namespace, snapshot_root, document_id) DO UPDATE SET
                entity_id = excluded.entity_id,
                selector = excluded.selector,
                document = excluded.document",
        )
        .await
    {
        Ok(statement) => statement,
        Err(error) => {
            let _ = execute_turso_statement(
                connection,
                "ROLLBACK",
                "failed to rollback Turso search projection transaction after prepare",
            )
            .await;
            return Err(format!(
                "failed to prepare Turso search projection replace: {error}"
            ));
        }
    };
    for document in documents {
        if let Err(error) = execute_turso_prepared_statement_with_lock_retry!(
            statement,
            (
                namespace,
                source_snapshot.root_digest.as_str(),
                document.document_id.as_str(),
                document.entity_id.as_str(),
                document.selector.as_deref(),
                document.document.as_str(),
            ),
            "failed to replace Turso search projection document",
        ) {
            let _ = execute_turso_statement(
                connection,
                "ROLLBACK",
                "failed to rollback Turso search projection transaction after document write",
            )
            .await;
            return Err(error);
        }
    }
    drop(statement);
    if let Err(error) = execute_turso_operation(
        || async {
            connection
                .execute(
                    "INSERT INTO asp_search_projection_generation
                     (namespace, snapshot_root, provider_digest)
                     VALUES (?1, ?2, ?3)
                     ON CONFLICT(namespace) DO UPDATE SET
                        snapshot_root = excluded.snapshot_root,
                        provider_digest = excluded.provider_digest",
                    (
                        namespace,
                        source_snapshot.root_digest.as_str(),
                        source_snapshot.provider_digest.as_str(),
                    ),
                )
                .await
                .map_err(|error| error.to_string())
        },
        "failed to publish Turso search projection generation",
    )
    .await
    {
        let _ = execute_turso_statement(
            connection,
            "ROLLBACK",
            "failed to rollback Turso search projection transaction after generation publish",
        )
        .await;
        return Err(error);
    }
    if let Err(error) = execute_turso_operation(
        || async {
            connection
                .execute(
                    "DELETE FROM asp_search_projection_document
                     WHERE namespace = ?1 AND snapshot_root <> ?2",
                    (namespace, source_snapshot.root_digest.as_str()),
                )
                .await
                .map_err(|error| error.to_string())
        },
        "failed to prune stale Turso search projection generations",
    )
    .await
    {
        let _ = execute_turso_statement(
            connection,
            "ROLLBACK",
            "failed to rollback Turso search projection transaction after stale prune",
        )
        .await;
        return Err(error);
    }
    if let Err(error) = execute_turso_statement(
        connection,
        "COMMIT",
        "failed to commit Turso search projection transaction",
    )
    .await
    {
        let _ = execute_turso_statement(
            connection,
            "ROLLBACK",
            "failed to rollback Turso search projection transaction after commit",
        )
        .await;
        return Err(error);
    }
    Ok(documents.len())
}

/// Search one active root-bound projection generation with FTS-first routing.
pub async fn search_turso_documents(
    db_path: &Path,
    namespace: &str,
    source_snapshot: &agent_semantic_content_identity::SourceSnapshotEvidence,
    query: &str,
    limit: u32,
) -> Result<TursoClientDbSearchResult, String> {
    if limit == 0 || query.trim().is_empty() {
        return Ok(TursoClientDbSearchResult {
            state: TursoClientDbSearchState::Miss,
            hits: Vec::new(),
        });
    }
    let connection = connect_turso_search_projection_db(db_path).await?;
    let mut generation_rows = connection
        .query(
            "SELECT snapshot_root, provider_digest
             FROM asp_search_projection_generation
             WHERE namespace = ?1",
            (namespace,),
        )
        .await
        .map_err(|error| format!("failed to query Turso search projection generation: {error}"))?;
    let Some(generation) = generation_rows
        .next()
        .await
        .map_err(|error| format!("failed to read Turso search projection generation: {error}"))?
    else {
        return Ok(TursoClientDbSearchResult {
            state: TursoClientDbSearchState::EmptyIndex,
            hits: Vec::new(),
        });
    };
    let active_root = generation
        .get::<String>(0)
        .map_err(|error| format!("failed to read Turso search projection root: {error}"))?;
    let active_provider_digest = generation
        .get::<String>(1)
        .map_err(|error| format!("failed to read Turso search projection provider: {error}"))?;
    if active_root != source_snapshot.root_digest
        || active_provider_digest != source_snapshot.provider_digest
    {
        return Ok(TursoClientDbSearchResult {
            state: TursoClientDbSearchState::ColdRequired,
            hits: Vec::new(),
        });
    }
    let mut hits = Vec::new();
    if let Some(fts_query) = turso_fts_query(query) {
        let fts_result = collect_turso_search_hits(
            &connection,
            "projection",
            "SELECT document_id, entity_id, selector, document
             FROM asp_search_projection_document
             WHERE namespace = ?1 AND snapshot_root = ?2
               AND (document MATCH ?3 OR selector MATCH ?3)
             LIMIT ?4",
            namespace,
            source_snapshot.root_digest.as_str(),
            &fts_query,
            limit,
            &mut hits,
        )
        .await;
        if fts_result.is_ok() && !hits.is_empty() {
            return Ok(TursoClientDbSearchResult {
                state: TursoClientDbSearchState::Hit,
                hits,
            });
        }
        hits.clear();
    }
    let like_query = format!("%{}%", query.trim());
    collect_turso_search_hits(
        &connection,
        "projection",
        "SELECT document_id, entity_id, selector, document
         FROM asp_search_projection_document
         WHERE namespace = ?1 AND snapshot_root = ?2
           AND (document LIKE ?3 OR selector LIKE ?3)
         ORDER BY document_id
         LIMIT ?4",
        namespace,
        source_snapshot.root_digest.as_str(),
        &like_query,
        limit,
        &mut hits,
    )
    .await?;
    let state = if hits.is_empty() {
        TursoClientDbSearchState::Miss
    } else {
        TursoClientDbSearchState::Hit
    };
    Ok(TursoClientDbSearchResult { state, hits })
}

fn turso_fts_query(query: &str) -> Option<String> {
    let terms = query
        .split(|character: char| !(character == '_' || character.is_ascii_alphanumeric()))
        .filter(|term| !term.is_empty())
        .take(8)
        .collect::<Vec<_>>();
    (!terms.is_empty()).then(|| terms.join(" "))
}

async fn collect_turso_search_hits(
    connection: &turso::Connection,
    source: &'static str,
    sql: &str,
    namespace: &str,
    snapshot_root: &str,
    query: &str,
    limit: u32,
    hits: &mut Vec<TursoClientDbSearchHit>,
) -> Result<(), String> {
    let mut rows = connection
        .query(sql, (namespace, snapshot_root, query, limit))
        .await
        .map_err(|error| format!("failed to query Turso search documents: {error}"))?;
    while let Some(row) = rows
        .next()
        .await
        .map_err(|error| format!("failed to read Turso search row: {error}"))?
    {
        let document_id = row
            .get::<String>(0)
            .map_err(|error| format!("failed to read Turso document id: {error}"))?;
        let entity_id = row
            .get::<Option<String>>(1)
            .map_err(|error| format!("failed to read Turso entity id: {error}"))?;
        let selector = row
            .get::<Option<String>>(2)
            .map_err(|error| format!("failed to read Turso selector: {error}"))?;
        let document = row
            .get::<String>(3)
            .map_err(|error| format!("failed to read Turso document body: {error}"))?;
        hits.push(TursoClientDbSearchHit {
            source,
            document_id,
            entity_id,
            selector,
            document,
        });
    }
    Ok(())
}

impl TursoClientDbSearchHit {
    #[must_use]
    pub fn source(&self) -> &str {
        self.source
    }

    #[must_use]
    pub fn document_id(&self) -> &str {
        &self.document_id
    }

    #[must_use]
    pub fn entity_id(&self) -> Option<&str> {
        self.entity_id.as_deref()
    }

    #[must_use]
    pub fn selector(&self) -> Option<&str> {
        self.selector.as_deref()
    }

    #[must_use]
    pub fn document(&self) -> &str {
        &self.document
    }
}
