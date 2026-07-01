//! Turso FTS and overlay document search adapter.

use std::path::Path;

use super::turso::connect_turso_client_db;

const SEARCH_DOCUMENT_FTS_INDEX: &str = "asp_search_document_fts_idx";
const SEARCH_DOCUMENT_FTS_REBUILD_THRESHOLD: usize = 128;

/// Feature-gated stable search document row written through the Turso adapter.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TursoClientDbSearchDocument {
    pub namespace: String,
    pub document_id: String,
    pub entity_id: String,
    pub selector: Option<String>,
    pub document: String,
}

/// Feature-gated dirty overlay document row written through the Turso adapter.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TursoClientDbOverlayDocument {
    pub repo_id: String,
    pub workspace_id: String,
    pub session_id: String,
    pub base_generation: String,
    pub document_id: String,
    pub selector: Option<String>,
    pub document: String,
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

/// Insert or update stable search documents using one Turso connection.
pub async fn upsert_turso_search_documents(
    db_path: &Path,
    documents: &[TursoClientDbSearchDocument],
) -> Result<usize, String> {
    if documents.is_empty() {
        return Ok(0);
    }
    let connection = connect_turso_client_db(db_path).await?;
    let rebuild_fts_index = documents.len() >= SEARCH_DOCUMENT_FTS_REBUILD_THRESHOLD;
    if rebuild_fts_index {
        drop_search_document_fts_index(&connection).await?;
    }
    connection
        .execute("BEGIN TRANSACTION", ())
        .await
        .map_err(|error| format!("failed to begin Turso search document transaction: {error}"))?;
    let mut statement = match connection
        .prepare_cached(
            "INSERT INTO asp_search_document (namespace, document_id, entity_id, selector, document)
             VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(namespace, document_id) DO UPDATE SET
                entity_id = excluded.entity_id,
                selector = excluded.selector,
                document = excluded.document",
        )
        .await
    {
        Ok(statement) => statement,
        Err(error) => {
            let _ = connection.execute("ROLLBACK", ()).await;
            if rebuild_fts_index {
                let _ = ensure_search_document_fts_index(&connection).await;
            }
            return Err(format!(
                "failed to prepare Turso search document upsert: {error}"
            ));
        }
    };
    for document in documents {
        if let Err(error) = statement
            .execute((
                document.namespace.as_str(),
                document.document_id.as_str(),
                document.entity_id.as_str(),
                document.selector.as_deref(),
                document.document.as_str(),
            ))
            .await
            .map_err(|error| format!("failed to upsert Turso search document: {error}"))
        {
            let _ = connection.execute("ROLLBACK", ()).await;
            if rebuild_fts_index {
                let _ = ensure_search_document_fts_index(&connection).await;
            }
            return Err(error);
        }
    }
    connection
        .execute("COMMIT", ())
        .await
        .map_err(|error| format!("failed to commit Turso search document transaction: {error}"))?;
    if rebuild_fts_index {
        ensure_search_document_fts_index(&connection).await?;
    }
    Ok(documents.len())
}

async fn drop_search_document_fts_index(connection: &turso::Connection) -> Result<(), String> {
    connection
        .execute(
            format!("DROP INDEX IF EXISTS {SEARCH_DOCUMENT_FTS_INDEX}"),
            (),
        )
        .await
        .map_err(|error| format!("failed to drop Turso search document FTS index: {error}"))?;
    Ok(())
}

async fn ensure_search_document_fts_index(connection: &turso::Connection) -> Result<(), String> {
    connection
        .execute(
            format!(
                "CREATE INDEX IF NOT EXISTS {SEARCH_DOCUMENT_FTS_INDEX} ON asp_search_document USING fts (document, selector)"
            ),
            (),
        )
        .await
        .map_err(|error| format!("failed to recreate Turso search document FTS index: {error}"))?;
    Ok(())
}

/// Insert or update one dirty overlay document in the Turso DB Engine file.
pub async fn upsert_turso_overlay_document(
    db_path: &Path,
    document: &TursoClientDbOverlayDocument,
) -> Result<(), String> {
    let connection = connect_turso_client_db(db_path).await?;
    connection
        .execute(
            "INSERT INTO asp_overlay_document
             (repo_id, workspace_id, session_id, base_generation, document_id, selector, document)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
             ON CONFLICT(repo_id, workspace_id, session_id, base_generation, document_id)
             DO UPDATE SET
                selector = excluded.selector,
                document = excluded.document",
            (
                document.repo_id.as_str(),
                document.workspace_id.as_str(),
                document.session_id.as_str(),
                document.base_generation.as_str(),
                document.document_id.as_str(),
                document.selector.as_deref(),
                document.document.as_str(),
            ),
        )
        .await
        .map_err(|error| format!("failed to upsert Turso overlay document: {error}"))?;
    Ok(())
}

/// Search active overlay and stable Turso documents with FTS-first routing.
pub async fn search_turso_documents(
    db_path: &Path,
    query: &str,
    limit: u32,
) -> Result<Vec<TursoClientDbSearchHit>, String> {
    if limit == 0 || query.trim().is_empty() {
        return Ok(Vec::new());
    }
    let connection = connect_turso_client_db(db_path).await?;
    let mut hits = Vec::new();
    if let Some(fts_query) = turso_fts_query(query) {
        let fts_result = async {
            collect_turso_search_hits(
                &connection,
                "overlay",
                "SELECT document_id, NULL as entity_id, selector, document
                 FROM asp_overlay_document
                 WHERE document MATCH ?1 OR selector MATCH ?1
                 LIMIT ?2",
                &fts_query,
                limit,
                &mut hits,
            )
            .await?;
            if hits.len() < limit as usize {
                collect_turso_search_hits(
                    &connection,
                    "stable",
                    "SELECT document_id, entity_id, selector, document
                     FROM asp_search_document
                     WHERE document MATCH ?1 OR selector MATCH ?1
                     LIMIT ?2",
                    &fts_query,
                    limit.saturating_sub(hits.len() as u32),
                    &mut hits,
                )
                .await?;
            }
            Ok::<(), String>(())
        }
        .await;
        if fts_result.is_ok() && !hits.is_empty() {
            return Ok(hits);
        }
        hits.clear();
    }
    let like_query = format!("%{}%", query.trim());
    collect_turso_search_hits(
        &connection,
        "overlay",
        "SELECT document_id, NULL as entity_id, selector, document
         FROM asp_overlay_document
         WHERE document LIKE ?1 OR selector LIKE ?1
         ORDER BY document_id
         LIMIT ?2",
        &like_query,
        limit,
        &mut hits,
    )
    .await?;
    if hits.len() < limit as usize {
        collect_turso_search_hits(
            &connection,
            "stable",
            "SELECT document_id, entity_id, selector, document
             FROM asp_search_document
             WHERE document LIKE ?1 OR selector LIKE ?1
             ORDER BY document_id
             LIMIT ?2",
            &like_query,
            limit.saturating_sub(hits.len() as u32),
            &mut hits,
        )
        .await?;
    }
    Ok(hits)
}

/// Search stable Turso documents inside one namespace using an existing connection.
pub(super) async fn search_turso_stable_documents_with_connection(
    connection: &turso::Connection,
    namespace: &str,
    query: &str,
    limit: u32,
) -> Result<Vec<TursoClientDbSearchHit>, String> {
    if limit == 0 || query.trim().is_empty() {
        return Ok(Vec::new());
    }
    let mut hits = Vec::new();
    if let Some(fts_query) = turso_fts_query(query) {
        let fts_result = collect_turso_search_hits_with_namespace(
            connection,
            "SELECT document_id, entity_id, selector, document
             FROM asp_search_document
             WHERE namespace = ?1 AND (document MATCH ?2 OR selector MATCH ?2)
             LIMIT ?3",
            namespace,
            &fts_query,
            limit,
            &mut hits,
        )
        .await;
        if fts_result.is_ok() && !hits.is_empty() {
            return Ok(hits);
        }
        hits.clear();
    }
    let like_query = format!("%{}%", query.trim());
    collect_turso_search_hits_with_namespace(
        connection,
        "SELECT document_id, entity_id, selector, document
         FROM asp_search_document
         WHERE namespace = ?1 AND (document LIKE ?2 OR selector LIKE ?2)
         ORDER BY document_id
         LIMIT ?3",
        namespace,
        &like_query,
        limit,
        &mut hits,
    )
    .await?;
    Ok(hits)
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
    query: &str,
    limit: u32,
    hits: &mut Vec<TursoClientDbSearchHit>,
) -> Result<(), String> {
    let mut rows = connection
        .query(sql, (query, limit))
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

async fn collect_turso_search_hits_with_namespace(
    connection: &turso::Connection,
    sql: &str,
    namespace: &str,
    query: &str,
    limit: u32,
    hits: &mut Vec<TursoClientDbSearchHit>,
) -> Result<(), String> {
    let mut rows = connection
        .query(sql, (namespace, query, limit))
        .await
        .map_err(|error| format!("failed to query Turso stable search documents: {error}"))?;
    while let Some(row) = rows
        .next()
        .await
        .map_err(|error| format!("failed to read Turso stable search row: {error}"))?
    {
        let document_id = row
            .get::<String>(0)
            .map_err(|error| format!("failed to read Turso stable document id: {error}"))?;
        let entity_id = row
            .get::<String>(1)
            .map_err(|error| format!("failed to read Turso stable entity id: {error}"))?;
        let selector = row
            .get::<Option<String>>(2)
            .map_err(|error| format!("failed to read Turso stable selector: {error}"))?;
        let document = row
            .get::<String>(3)
            .map_err(|error| format!("failed to read Turso stable document body: {error}"))?;
        hits.push(TursoClientDbSearchHit {
            source: "stable",
            document_id,
            entity_id: Some(entity_id),
            selector,
            document,
        });
    }
    Ok(())
}
