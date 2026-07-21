//! Source-index and structural-index DB Engine facade methods.

use std::path::Path;

use crate::source_index::{ClientDbSourceIndexImport, ClientDbSourceIndexRefreshRequest};
use crate::structural_index::ClientDbStructuralIndexImport;

use crate::engine::facade::{ClientDbEngine, block_on_db_engine_async};
use crate::engine::turso_search::replace_turso_search_document_generation;
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
        let search_document_count = refresh.owner_count as usize;
        Ok(source_index_read_model_report(
            refresh.owner_count as usize + refresh.selector_count as usize,
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
    projection.validate()?;
    Ok(source_index_read_model_report(
        refresh.owner_count as usize + refresh.selector_count as usize,
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
    node_locator_count: usize,
    search_document_count: usize,
) -> ClientDbEngineSourceIndexReadModelReport {
    ClientDbEngineSourceIndexReadModelReport {
        node_locator_count,
        search_document_count,
    }
}

async fn persist_structural_index_search_documents_at_path(
    db_path: &Path,
    import: &ClientDbStructuralIndexImport,
    source_snapshot: &agent_semantic_content_identity::SourceSnapshotEvidence,
) -> Result<usize, String> {
    let mut documents = Vec::new();
    for symbol in &import.symbols {
        let entity_id = format!(
            "symbol:{}:{}:{}:{}",
            source_snapshot.root_digest,
            symbol.owner_path.as_str(),
            symbol.kind.as_str(),
            symbol.name.as_str()
        );
        let selector = symbol
            .source_locator
            .as_ref()
            .map(|locator| locator.as_str().to_string());
        let mut terms = vec![
            "symbol".to_string(),
            symbol.name.as_str().to_string(),
            symbol.kind.as_str().to_string(),
            symbol.owner_path.as_str().to_string(),
            import.language_id.as_str().to_string(),
            import.provider_id.as_str().to_string(),
        ];
        if let Some(selector) = &selector {
            terms.push(selector.clone());
        }
        terms.extend(symbol.query_keys.iter().map(|key| key.as_str().to_string()));
        let document = crate::TursoClientDbSearchDocument {
            document_id: format!("structural-index:{entity_id}"),
            entity_id,
            selector,
            document: terms.join(" "),
        };
        documents.push(document);
    }
    for dependency in &import.dependency_usages {
        let dependency_label = dependency
            .api_name
            .as_ref()
            .map(|api_name| {
                format!(
                    "{}::{}",
                    dependency.package_name.as_str(),
                    api_name.as_str()
                )
            })
            .unwrap_or_else(|| dependency.package_name.as_str().to_string());
        let entity_id = format!(
            "dependency:{}:{}:{}",
            source_snapshot.root_digest,
            dependency.owner_path.as_str(),
            dependency_label
        );
        let selector = dependency
            .source_locator
            .as_ref()
            .map(|locator| locator.as_str().to_string());
        let mut terms = vec![
            "dependency-usage".to_string(),
            dependency_label,
            dependency.owner_path.as_str().to_string(),
            import.language_id.as_str().to_string(),
            import.provider_id.as_str().to_string(),
        ];
        if let Some(selector) = &selector {
            terms.push(selector.clone());
        }
        terms.extend(
            dependency
                .query_keys
                .iter()
                .map(|key| key.as_str().to_string()),
        );
        documents.push(crate::TursoClientDbSearchDocument {
            document_id: format!("structural-index:{entity_id}"),
            entity_id,
            selector,
            document: terms.join(" "),
        });
    }
    replace_turso_search_document_generation(
        db_path,
        "structural-index",
        source_snapshot,
        &documents,
    )
    .await
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
    let search_document_count =
        persist_structural_index_search_documents_at_path(db_path, import, source_snapshot).await?;
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
