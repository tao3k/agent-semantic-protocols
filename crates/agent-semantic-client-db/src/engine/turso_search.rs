//! Turso FTS and overlay document search adapter.

use std::path::Path;

use super::turso::connect_turso_client_db;

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
    pub selector: Option<String>,
    pub document: String,
}

/// Insert or update one stable search document in the Turso DB Engine file.
pub async fn upsert_turso_search_document(
    db_path: &Path,
    document: &TursoClientDbSearchDocument,
) -> Result<(), String> {
    let connection = connect_turso_client_db(db_path).await?;
    connection
        .execute(
            "INSERT INTO asp_search_document (namespace, document_id, entity_id, selector, document)
             VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(namespace, document_id) DO UPDATE SET
                entity_id = excluded.entity_id,
                selector = excluded.selector,
                document = excluded.document",
            (
                document.namespace.as_str(),
                document.document_id.as_str(),
                document.entity_id.as_str(),
                document.selector.as_deref(),
                document.document.as_str(),
            ),
        )
        .await
        .map_err(|error| format!("failed to upsert Turso search document: {error}"))?;
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
                "SELECT document_id, selector, document
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
                    "SELECT document_id, selector, document
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
        "SELECT document_id, selector, document
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
            "SELECT document_id, selector, document
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
        let selector = row
            .get::<Option<String>>(1)
            .map_err(|error| format!("failed to read Turso selector: {error}"))?;
        let document = row
            .get::<String>(2)
            .map_err(|error| format!("failed to read Turso document body: {error}"))?;
        hits.push(TursoClientDbSearchHit {
            source,
            document_id,
            selector,
            document,
        });
    }
    Ok(())
}
