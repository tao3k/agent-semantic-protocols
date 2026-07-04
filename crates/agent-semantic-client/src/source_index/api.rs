//! Public refresh API for the DB Engine source index.

use std::collections::BTreeSet;
use std::path::Path;
use std::time::Instant;

use agent_semantic_client_core::{
    ClientCacheFileHash, LanguageId, ProjectContext, ProviderId, ProviderRegistryEvidence,
    ProviderRegistrySnapshot, SemanticSchemaId, SemanticSchemaVersion,
};
use agent_semantic_client_db::ClientDbEngineWriteSession;
use agent_semantic_client_db::{
    ClientDbEngine, ClientDbSourceIndexImportAssemblyRequest, ClientDbSourceIndexRefreshRequest,
    client_db_source_index_file_count, client_db_source_index_generation_id,
    source_index_file_hashes, source_index_import_with_file_hashes,
};
use agent_semantic_runtime::{collect_runtime_source_index_files, runtime_source_index_context};

use super::collect::collect_source_index_files;
use super::config::{
    SOURCE_INDEX_FILE_BYTES_LIMIT, SOURCE_INDEX_FILE_LIMIT, SOURCE_INDEX_PROVIDER_ID,
    SOURCE_INDEX_SCHEMA_ID, SOURCE_INDEX_SCHEMA_VERSION,
};
use super::model::{SourceIndexRefreshReport, SourceIndexScopeFile};

/// Refresh the DB Engine source index for a project without storing raw source.
pub fn refresh_source_index(project_root: &Path) -> Result<SourceIndexRefreshReport, String> {
    let trace_started = Instant::now();
    let mut context = SourceIndexRefreshContext::resolve(project_root)?;
    source_index_trace("context-resolved", trace_started);
    let snapshot = ProviderRegistrySnapshot::load(project_root)?;
    source_index_trace("provider-registry-loaded", trace_started);
    let registry = snapshot.evidence(project_root);
    let previous_file_hashes = context.latest_file_hashes(project_root)?;
    source_index_trace("previous-file-hashes-loaded", trace_started);
    {
        if let Some(report) = try_reuse_source_index_scope(
            &context,
            SourceIndexScopeReuse {
                index_root: project_root,
                previous_file_hashes: previous_file_hashes.as_deref(),
                registry: &registry,
            },
        )? {
            source_index_trace("scope-reused", trace_started);
            return Ok(report);
        }
    }
    let files = collect_source_index_files(project_root, &snapshot)?;
    source_index_trace("scope-files-collected", trace_started);
    context.refresh_generation(SourceIndexGenerationRefresh {
        index_root: project_root,
        files: &files,
        previous_file_hashes: previous_file_hashes.as_deref(),
        registry: &registry,
    })
}

/// Refresh source-index rows for an ASP-managed runtime source checkout.
pub fn refresh_runtime_source_index(
    project_root: &Path,
    checkout_root: &Path,
    language_id: &LanguageId,
    provider_id: &ProviderId,
) -> Result<SourceIndexRefreshReport, String> {
    let mut context = SourceIndexRefreshContext::resolve(project_root)?;
    let client_cache_dir = context.client_cache_dir();
    let runtime_context = runtime_source_index_context(
        checkout_root,
        client_cache_dir,
        language_id.as_str(),
        provider_id.as_str(),
    )?;

    let files = collect_runtime_source_index_files(
        &runtime_context.checkout_root,
        language_id.as_str(),
        provider_id.as_str(),
        SOURCE_INDEX_FILE_LIMIT,
    )?
    .into_iter()
    .map(|file| SourceIndexScopeFile {
        path: file.path,
        language_id: LanguageId::from(file.language_id),
        provider_id: ProviderId::from(file.provider_id),
    })
    .collect::<Vec<_>>();
    if files.is_empty() {
        return Err(format!(
            "runtime source index found no source files in {} for language {}",
            runtime_context.checkout_root.display(),
            language_id
        ));
    }
    let registry = ProviderRegistryEvidence {
        fingerprint: runtime_context.registry_fingerprint,
        scope_dirs: BTreeSet::new(),
    };
    let previous_file_hashes = context.latest_file_hashes(&runtime_context.checkout_root)?;
    context.refresh_generation(SourceIndexGenerationRefresh {
        index_root: &runtime_context.checkout_root,
        files: &files,
        previous_file_hashes: previous_file_hashes.as_deref(),
        registry: &registry,
    })
}

struct SourceIndexRefreshContext {
    db_path: std::path::PathBuf,
    client_cache_dir: std::path::PathBuf,
    db_session: ClientDbEngineWriteSession,
    schema_id: SemanticSchemaId,
    schema_version: SemanticSchemaVersion,
}

impl SourceIndexRefreshContext {
    fn resolve(project_root: &Path) -> Result<Self, String> {
        let project_context = ProjectContext::resolve(project_root)?;
        project_context.require_inside_workspace(project_root)?;
        let db_engine = ClientDbEngine::resolve(project_root)?;
        let db_path = db_engine.db_path().to_path_buf();
        let client_cache_dir = db_engine.client_dir().to_path_buf();
        let db_session = ClientDbEngine::open_write_session_client_dir(db_engine.client_dir())?;
        Ok(Self {
            db_path,
            client_cache_dir,
            db_session,
            schema_id: SemanticSchemaId::from(SOURCE_INDEX_SCHEMA_ID),
            schema_version: SemanticSchemaVersion::from(SOURCE_INDEX_SCHEMA_VERSION),
        })
    }

    fn client_cache_dir(&self) -> &Path {
        &self.client_cache_dir
    }

    fn latest_file_hashes(
        &self,
        index_root: &Path,
    ) -> Result<Option<Vec<ClientCacheFileHash>>, String> {
        self.db_session.latest_source_index_file_hashes(
            index_root,
            &self.schema_id,
            &self.schema_version,
        )
    }

    fn refresh_generation(
        &mut self,
        request: SourceIndexGenerationRefresh<'_>,
    ) -> Result<SourceIndexRefreshReport, String> {
        let trace_started = Instant::now();
        let file_hashes = source_index_file_hashes(
            request.index_root,
            request.files,
            request.previous_file_hashes,
            &request.registry.fingerprint,
            request.registry.scope_dirs.iter().map(String::as_str),
        )?;
        source_index_trace("generation-file-hashes-built", trace_started);
        let reusable_stats = self.db_session.reusable_source_index_generation(
            request.index_root,
            &self.schema_id,
            &self.schema_version,
            &file_hashes,
        )?;
        source_index_trace("generation-reuse-checked", trace_started);
        if let Some(stats) = reusable_stats {
            return Ok(source_index_refresh_report(
                &self.db_path,
                stats,
                request.files.len(),
                true,
            ));
        }
        let generation_id = client_db_source_index_generation_id();
        let import = source_index_import_with_file_hashes(
            ClientDbSourceIndexImportAssemblyRequest {
                generation_id,
                project_root: request.index_root.to_path_buf(),
                schema_id: self.schema_id.clone(),
                schema_version: self.schema_version.clone(),
                selector_source: SOURCE_INDEX_PROVIDER_ID.into(),
                file_text_bytes_limit: SOURCE_INDEX_FILE_BYTES_LIMIT,
                previous_file_hashes: None,
                registry_fingerprint: request.registry.fingerprint.clone(),
                extra_scope_dirs: request.registry.scope_dirs.iter().cloned().collect(),
                files: request.files.to_vec(),
            },
            file_hashes,
        )?;
        source_index_trace("generation-import-assembled", trace_started);
        let report =
            self.db_session
                .refresh_source_index_import(ClientDbSourceIndexRefreshRequest {
                    import,
                    file_count: client_db_source_index_file_count(request.files.len()),
                })?;
        source_index_trace("generation-turso-imported", trace_started);
        Ok(SourceIndexRefreshReport::from_report(
            self.db_path.clone(),
            report,
        ))
    }
}

fn source_index_trace(stage: &str, started: Instant) {
    if std::env::var_os("ASP_SOURCE_INDEX_TRACE").is_some() {
        eprintln!(
            "[source-index-trace] stage={} elapsedMs={}",
            stage,
            started.elapsed().as_millis()
        );
    }
}

struct SourceIndexGenerationRefresh<'a> {
    index_root: &'a Path,
    files: &'a [SourceIndexScopeFile],
    previous_file_hashes: Option<&'a [ClientCacheFileHash]>,
    registry: &'a ProviderRegistryEvidence,
}

struct SourceIndexScopeReuse<'a> {
    index_root: &'a Path,
    previous_file_hashes: Option<&'a [agent_semantic_client_core::ClientCacheFileHash]>,
    registry: &'a ProviderRegistryEvidence,
}

fn try_reuse_source_index_scope(
    context: &SourceIndexRefreshContext,
    scope: SourceIndexScopeReuse<'_>,
) -> Result<Option<SourceIndexRefreshReport>, String> {
    let Some(previous_file_hashes) = scope.previous_file_hashes else {
        return Ok(None);
    };
    let files = match context.db_session.latest_source_index_scope_files(
        scope.index_root,
        &context.schema_id,
        &context.schema_version,
    )? {
        Some(files) => files,
        None => return Ok(None),
    };
    let file_hashes = match source_index_file_hashes(
        scope.index_root,
        &files,
        Some(previous_file_hashes),
        &scope.registry.fingerprint,
        scope.registry.scope_dirs.iter().map(String::as_str),
    ) {
        Ok(file_hashes) => file_hashes,
        Err(_) => return Ok(None),
    };
    let Some(stats) = context.db_session.reusable_source_index_generation(
        scope.index_root,
        &context.schema_id,
        &context.schema_version,
        &file_hashes,
    )?
    else {
        return Ok(None);
    };
    Ok(Some(source_index_refresh_report(
        &context.db_path,
        stats,
        files.len(),
        true,
    )))
}

fn source_index_refresh_report(
    db_path: &Path,
    stats: agent_semantic_client_db::ClientDbSourceIndexStats,
    file_count: usize,
    reused_generation: bool,
) -> SourceIndexRefreshReport {
    SourceIndexRefreshReport::from_stats(
        db_path.to_path_buf(),
        stats,
        file_count,
        reused_generation,
    )
}
