//! Public refresh API for the Rust SQL source index.

use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

use agent_semantic_client_core::{
    ClientCacheFileHash, LanguageId, ProjectContext, ProviderId, ProviderRegistryEvidence,
    ProviderRegistrySnapshot, SemanticSchemaId, SemanticSchemaVersion,
};
use agent_semantic_client_db::{
    ClientDb, ClientDbEngine, ClientDbSourceIndexRefreshRequest, client_db_source_index_file_count,
    client_db_source_index_generation_id,
};
use agent_semantic_runtime::runtime_source_index_context;

use super::collect::collect_source_index_files;
use super::config::{SOURCE_INDEX_FILE_LIMIT, SOURCE_INDEX_SCHEMA_ID, SOURCE_INDEX_SCHEMA_VERSION};
use super::import::{source_index_file_hashes, source_index_import_with_file_hashes};
use super::model::{SourceIndexRefreshReport, SourceIndexScopeFile};

/// Refresh the Rust SQL source index for a project without storing raw source.
pub fn refresh_source_index(project_root: &Path) -> Result<SourceIndexRefreshReport, String> {
    let mut context = SourceIndexRefreshContext::resolve(project_root)?;
    let snapshot = ProviderRegistrySnapshot::load(project_root)?;
    let registry = snapshot.evidence(project_root);
    let previous_file_hashes = context.latest_file_hashes(project_root)?;
    if let Some(report) = try_reuse_source_index_scope(
        &context,
        SourceIndexScopeReuse {
            index_root: project_root,
            previous_file_hashes: previous_file_hashes.as_deref(),
            registry: &registry,
        },
    )? {
        return Ok(report);
    }
    let files = collect_source_index_files(project_root, &snapshot)?;
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
        language_id,
        provider_id,
    )?;
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
    db: ClientDb,
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
        let db = db_engine.open_or_create()?;
        Ok(Self {
            db_path,
            client_cache_dir,
            db,
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
        self.db
            .latest_source_index_file_hashes(index_root, &self.schema_id, &self.schema_version)
    }

    fn refresh_generation(
        &mut self,
        request: SourceIndexGenerationRefresh<'_>,
    ) -> Result<SourceIndexRefreshReport, String> {
        let file_hashes = source_index_file_hashes(
            request.index_root,
            request.files,
            request.previous_file_hashes,
            &request.registry.fingerprint,
            &request.registry.scope_dirs,
        )?;
        if let Some(stats) = self.db.reusable_source_index_generation(
            request.index_root,
            &self.schema_id,
            &self.schema_version,
            &file_hashes,
        )? {
            return Ok(source_index_refresh_report(
                &self.db_path,
                stats,
                request.files.len(),
                true,
            ));
        }
        let generation_id = client_db_source_index_generation_id();
        let import = source_index_import_with_file_hashes(
            request.index_root,
            generation_id,
            request.files,
            file_hashes,
        )?;
        let report = self
            .db
            .refresh_source_index_import(ClientDbSourceIndexRefreshRequest {
                import,
                file_count: client_db_source_index_file_count(request.files.len()),
            })?;
        Ok(SourceIndexRefreshReport::from_report(
            self.db_path.clone(),
            report,
        ))
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
    let files = match context.db.latest_source_index_scope_files(
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
        &scope.registry.scope_dirs,
    ) {
        Ok(file_hashes) => file_hashes,
        Err(_) => return Ok(None),
    };
    let Some(stats) = context.db.reusable_source_index_generation(
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

fn collect_runtime_source_index_files(
    checkout_root: &Path,
    language_id: &LanguageId,
    provider_id: &ProviderId,
) -> Result<Vec<SourceIndexScopeFile>, String> {
    let mut files = Vec::new();
    collect_runtime_source_index_files_from_dir(
        checkout_root,
        language_id,
        provider_id,
        &mut files,
    )?;
    files.sort_by(|left, right| left.path.cmp(&right.path));
    files.truncate(SOURCE_INDEX_FILE_LIMIT);
    Ok(files)
}

fn collect_runtime_source_index_files_from_dir(
    dir: &Path,
    language_id: &LanguageId,
    provider_id: &ProviderId,
    files: &mut Vec<SourceIndexScopeFile>,
) -> Result<(), String> {
    if files.len() >= SOURCE_INDEX_FILE_LIMIT {
        return Ok(());
    }
    let mut entries = fs::read_dir(dir)
        .map_err(|error| {
            format!(
                "failed to read runtime source dir {}: {error}",
                dir.display()
            )
        })?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("failed to read runtime source dir entry: {error}"))?;
    entries.sort_by_key(std::fs::DirEntry::path);
    for entry in entries {
        if files.len() >= SOURCE_INDEX_FILE_LIMIT {
            break;
        }
        let path = entry.path();
        let file_type = entry.file_type().map_err(|error| {
            format!(
                "failed to inspect runtime source file type {}: {error}",
                path.display()
            )
        })?;
        if file_type.is_dir() {
            if !runtime_source_dir_is_skipped(&path) {
                collect_runtime_source_index_files_from_dir(
                    &path,
                    language_id,
                    provider_id,
                    files,
                )?;
            }
        } else if file_type.is_file() && runtime_source_file_matches(language_id, &path) {
            files.push(SourceIndexScopeFile {
                path,
                language_id: language_id.clone(),
                provider_id: provider_id.clone(),
            });
        }
    }
    Ok(())
}

fn runtime_source_dir_is_skipped(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| matches!(name, ".git" | ".hg" | ".svn"))
}

fn runtime_source_file_matches(language_id: &LanguageId, path: &Path) -> bool {
    let Some(extension) = path.extension().and_then(|extension| extension.to_str()) else {
        return false;
    };
    let extension = extension.to_ascii_lowercase();
    match language_id.as_str() {
        "gerbil-scheme" => matches!(extension.as_str(), "ss" | "scm" | "sld" | "sch" | "scheme"),
        "julia" => extension == "jl",
        "python" => extension == "py",
        "rust" => extension == "rs",
        "typescript" => matches!(
            extension.as_str(),
            "ts" | "tsx" | "js" | "jsx" | "mjs" | "cjs"
        ),
        _ => false,
    }
}
