//! Source-index and structural-index DB Engine facade methods.

use std::path::Path;

use crate::evidence_graph::{source_index_evidence_graph, structural_index_evidence_graph};
use crate::source_index::{ClientDbSourceIndexImport, ClientDbSourceIndexRefreshRequest};
use crate::structural_index::ClientDbStructuralIndexImport;

use crate::engine::facade::{ClientDbEngine, block_on_db_engine_async};
use crate::engine::turso_evidence_graph::TursoClientDbEvidenceGraphPersistReport;
use crate::engine::turso_search::upsert_turso_search_documents;
use crate::engine::turso_source_index::refresh_turso_source_index_import;
use crate::engine::{
    ClientDbEngineSourceIndexReadModelReport, ClientDbEngineStructuralIndexReadModelReport,
};

impl ClientDbEngine {
    /// Persist stable source-index graph and search documents through the active DB Engine backend.
    pub async fn persist_source_index_read_model(
        &self,
        import: &ClientDbSourceIndexImport,
        source_snapshot: &agent_semantic_content_identity::SourceSnapshotEvidence,
    ) -> Result<ClientDbEngineSourceIndexReadModelReport, String> {
        let trace_started = std::time::Instant::now();
        let refresh = refresh_turso_source_index_import(
            self.db_path(),
            ClientDbSourceIndexRefreshRequest {
                import: import.clone(),
                file_count: import.file_hashes.len().min(u32::MAX as usize) as u32,
                source_snapshot: source_snapshot.clone(),
            },
        )
        .await?;
        db_engine_trace("source-index-refresh-read-model", trace_started);
        let graph = source_index_evidence_graph(import);
        db_engine_trace("source-index-graph-built", trace_started);
        let graph_report =
            crate::engine::persist_turso_evidence_graph(self.db_path(), &graph, source_snapshot)
                .await?;
        let search_document_count = refresh.owner_count as usize;
        Ok(source_index_read_model_report(
            graph_report,
            search_document_count,
        ))
    }

    /// Persist one parser-owned language projection through the Turso read model.
    pub async fn persist_language_projection_read_model(
        &self,
        import: &ClientDbSourceIndexImport,
        projection: &crate::ClientDbLanguageProjection,
        source_snapshot: &agent_semantic_content_identity::SourceSnapshotEvidence,
    ) -> Result<ClientDbEngineSourceIndexReadModelReport, String> {
        persist_language_projection_read_model_at_path(
            self.db_path(),
            import,
            projection,
            source_snapshot,
        )
        .await
    }

    /// Persist one parser-owned language projection through an isolated client directory.
    pub fn persist_language_projection_read_model_from_client_dir(
        client_dir: impl AsRef<Path>,
        import: &ClientDbSourceIndexImport,
        projection: &crate::ClientDbLanguageProjection,
        source_snapshot: &agent_semantic_content_identity::SourceSnapshotEvidence,
    ) -> Result<ClientDbEngineSourceIndexReadModelReport, String> {
        let db_path = Self::turso_path_for_client_dir(client_dir);
        let import = import.clone();
        let projection = projection.clone();
        let source_snapshot = source_snapshot.clone();
        block_on_db_engine_async(async move {
            persist_language_projection_read_model_at_path(
                &db_path,
                &import,
                &projection,
                &source_snapshot,
            )
            .await
        })
    }

    /// Persist stable structural-index graph facts through the active DB Engine backend.
    pub async fn persist_structural_index_read_model(
        &self,
        import: &ClientDbStructuralIndexImport,
        source_snapshot: &agent_semantic_content_identity::SourceSnapshotEvidence,
    ) -> Result<ClientDbEngineStructuralIndexReadModelReport, String> {
        persist_structural_index_read_model_at_path(self.db_path(), import, source_snapshot).await
    }
}

async fn persist_language_projection_read_model_at_path(
    db_path: &Path,
    import: &ClientDbSourceIndexImport,
    projection: &crate::ClientDbLanguageProjection,
    source_snapshot: &agent_semantic_content_identity::SourceSnapshotEvidence,
) -> Result<ClientDbEngineSourceIndexReadModelReport, String> {
    let trace_started = std::time::Instant::now();
    crate::engine::turso_bootstrap::bootstrap_turso_client_db(db_path).await?;
    let refresh = refresh_turso_source_index_import(
        db_path,
        ClientDbSourceIndexRefreshRequest {
            import: import.clone(),
            file_count: import.file_hashes.len().min(u32::MAX as usize) as u32,
            source_snapshot: source_snapshot.clone(),
        },
    )
    .await?;
    db_engine_trace("language-projection-source-index-refreshed", trace_started);
    let graph = crate::source_index::language_projection::language_projection_evidence_graph(
        import, projection,
    )?;
    let graph_report =
        crate::engine::persist_turso_evidence_graph(db_path, &graph, source_snapshot).await?;
    db_engine_trace(
        "language-projection-evidence-graph-persisted",
        trace_started,
    );
    Ok(source_index_read_model_report(
        graph_report,
        refresh.owner_count as usize,
    ))
}

fn db_engine_trace(stage: &str, started: std::time::Instant) {
    if std::env::var_os("ASP_SOURCE_INDEX_TRACE").is_some() {
        eprintln!(
            "[db-engine-trace] stage={} elapsedMs={}",
            stage,
            started.elapsed().as_millis()
        );
    }
}

fn source_index_read_model_report(
    graph_report: TursoClientDbEvidenceGraphPersistReport,
    search_document_count: usize,
) -> ClientDbEngineSourceIndexReadModelReport {
    ClientDbEngineSourceIndexReadModelReport {
        graph_entity_count: graph_report.entity_count,
        graph_edge_count: graph_report.edge_count,
        graph_artifact_digest: graph_report.graph_artifact_digest,
        search_document_count,
    }
}

async fn persist_structural_index_search_documents_at_path(
    db_path: &Path,
    generation_id: &str,
    graph: &crate::ClientDbEvidenceGraph,
) -> Result<usize, String> {
    let mut documents = Vec::new();
    for node in graph
        .nodes
        .iter()
        .filter(|node| matches!(node.kind, "symbol" | "dependency-usage"))
    {
        let mut terms = vec![node.kind.to_string(), node.label.clone()];
        if let Some(path) = &node.path {
            terms.push(path.clone());
        }
        if let Some(selector) = &node.selector {
            terms.push(selector.clone());
        }
        if let Some(language_id) = &node.language_id {
            terms.push(language_id.clone());
        }
        if let Some(provider_id) = &node.provider_id {
            terms.push(provider_id.clone());
        }
        terms.extend(node.query_keys.iter().cloned());
        let document = crate::TursoClientDbSearchDocument {
            namespace: "structural-index".to_string(),
            document_id: format!("structural-index:{generation_id}:{}", node.id),
            entity_id: node.id.clone(),
            selector: node.selector.clone(),
            document: terms.join(" "),
        };
        documents.push(document);
    }
    upsert_turso_search_documents(db_path, &documents).await
}

pub(in crate::engine) async fn persist_structural_index_read_model_at_path(
    db_path: &Path,
    import: &ClientDbStructuralIndexImport,
    source_snapshot: &agent_semantic_content_identity::SourceSnapshotEvidence,
) -> Result<ClientDbEngineStructuralIndexReadModelReport, String> {
    let _refresh_write_guard = structural_index_refresh_write_lock()
        .clone()
        .acquire_owned()
        .await
        .map_err(|error| format!("failed to acquire structural index refresh lock: {error}"))?;
    let trace_started = std::time::Instant::now();
    crate::engine::turso_bootstrap::bootstrap_turso_client_db(db_path).await?;
    db_engine_trace("structural-index-bootstrap", trace_started);
    let graph = structural_index_evidence_graph(import);
    db_engine_trace("structural-index-graph-built", trace_started);
    crate::persist_turso_evidence_graph(db_path, &graph, source_snapshot).await?;
    db_engine_trace("structural-index-graph-persisted", trace_started);
    let search_document_count = persist_structural_index_search_documents_at_path(
        db_path,
        import.generation_id.as_str(),
        &graph,
    )
    .await?;
    db_engine_trace("structural-index-search-documents-persisted", trace_started);
    Ok(structural_index_read_model_report(search_document_count))
}

fn structural_index_refresh_write_lock() -> &'static std::sync::Arc<tokio::sync::Semaphore> {
    static LOCK: std::sync::OnceLock<std::sync::Arc<tokio::sync::Semaphore>> =
        std::sync::OnceLock::new();
    LOCK.get_or_init(|| std::sync::Arc::new(tokio::sync::Semaphore::new(1)))
}

fn structural_index_read_model_report(
    search_document_count: usize,
) -> ClientDbEngineStructuralIndexReadModelReport {
    ClientDbEngineStructuralIndexReadModelReport {
        search_document_count,
    }
}
