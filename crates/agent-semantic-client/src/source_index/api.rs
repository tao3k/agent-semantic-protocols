//! Public refresh API for the DB Engine source index.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Instant;

use agent_semantic_client_core::{
    ClientCacheFileHash, LanguageId, ProjectContext, ProviderId, ProviderRegistryEvidence,
    ProviderRegistrySnapshot, SemanticSchemaId, SemanticSchemaVersion,
};
use agent_semantic_client_db::ClientDbEngineWriteSession;
use agent_semantic_client_db::{
    ClientDbEngine, ClientDbSourceIndexImportAssemblyRequest, ClientDbSourceIndexRefreshRequest,
    ClientDbSourceIndexScopeFile, client_db_source_index_file_count,
    client_db_source_index_generation_id, source_index_file_hashes,
    source_index_import_with_file_hashes,
};
use agent_semantic_runtime::{collect_runtime_source_index_files, runtime_source_index_context};

use super::collect::collect_source_index_files;
use super::config::{
    SOURCE_INDEX_FILE_BYTES_LIMIT, SOURCE_INDEX_FILE_LIMIT, SOURCE_INDEX_PROVIDER_ID,
    SOURCE_INDEX_SCHEMA_ID, SOURCE_INDEX_SCHEMA_VERSION,
};
use super::model::{SourceIndexRefreshReport, SourceIndexScopeFile};

/// Reuse the DB Engine source index for a project without scanning source files.
pub fn refresh_source_index(
    project_root: &Path,
) -> Result<Option<SourceIndexRefreshReport>, String> {
    let trace_started = Instant::now();
    let mut context = SourceIndexRefreshContext::resolve(project_root)?;
    source_index_trace("context-resolved", trace_started);
    let dirty_paths = source_index_tracked_worktree_dirty_paths(project_root);
    let tracked_worktree_dirty = dirty_paths.as_ref().map_or(true, |paths| !paths.is_empty());
    if tracked_worktree_dirty {
        source_index_trace("tracked-worktree-dirty", trace_started);
    }
    let snapshot = ProviderRegistrySnapshot::load(project_root)?;
    source_index_trace("provider-registry-loaded", trace_started);
    let registry = snapshot.evidence(project_root);
    let previous_file_hashes = context.latest_file_hashes(project_root)?;
    source_index_trace("previous-file-hashes-loaded", trace_started);
    if !tracked_worktree_dirty {
        if let Some(report) = try_reuse_source_index_scope(
            &context,
            SourceIndexScopeReuse {
                index_root: project_root,
                previous_file_hashes: previous_file_hashes.as_deref(),
                registry: &registry,
            },
        )? {
            source_index_trace("scope-reused", trace_started);
            return Ok(Some(report));
        }
    } else if let Some(dirty_paths) = dirty_paths.as_ref() {
        if let Some(report) = try_reuse_dirty_source_index_scope(
            &context,
            SourceIndexScopeReuse {
                index_root: project_root,
                previous_file_hashes: previous_file_hashes.as_deref(),
                registry: &registry,
            },
            dirty_paths,
        )? {
            source_index_trace("dirty-scope-reused", trace_started);
            return Ok(Some(report));
        }
    }
    source_index_trace("scope-reuse-missed", trace_started);
    let files = collect_source_index_files(project_root, &snapshot)?;
    source_index_trace("dirty-scope-files-collected", trace_started);
    let report = context.refresh_generation(SourceIndexGenerationRefresh {
        index_root: project_root,
        files: &files,
        previous_file_hashes: previous_file_hashes.as_deref(),
        registry: &registry,
    })?;
    source_index_trace("dirty-generation-refreshed", trace_started);
    Ok(Some(report))
}

/// Rebuild the DB Engine source index for a project without storing raw source.
pub fn rebuild_source_index(project_root: &Path) -> Result<SourceIndexRefreshReport, String> {
    let trace_started = Instant::now();
    let mut context = SourceIndexRefreshContext::resolve(project_root)?;
    source_index_trace("context-resolved", trace_started);
    let snapshot = ProviderRegistrySnapshot::load(project_root)?;
    source_index_trace("provider-registry-loaded", trace_started);
    let registry = snapshot.evidence(project_root);
    let previous_file_hashes = context.latest_file_hashes(project_root)?;
    source_index_trace("previous-file-hashes-loaded", trace_started);
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
        (
            checkout_root,
            client_cache_dir,
            language_id.as_str(),
            provider_id.as_str(),
        )
            .into(),
    )?;

    let previous_file_hashes = context.latest_file_hashes(&runtime_context.checkout_root)?;
    let files = if let Some(previous_file_hashes) = previous_file_hashes.as_deref() {
        let files = runtime_source_index_files_from_previous_hashes(
            &runtime_context.checkout_root,
            previous_file_hashes,
            language_id,
            provider_id,
        );
        if files.is_empty() {
            collect_runtime_source_index_files(
                (
                    runtime_context.checkout_root.as_path(),
                    language_id.as_str(),
                    provider_id.as_str(),
                    SOURCE_INDEX_FILE_LIMIT,
                )
                    .into(),
            )?
            .into_iter()
            .map(|file| SourceIndexScopeFile {
                path: file.path,
                language_id: LanguageId::from(file.language_id),
                provider_id: ProviderId::from(file.provider_id),
                selector_receipts: Vec::new(),
            })
            .collect::<Vec<_>>()
        } else {
            files
        }
    } else {
        collect_runtime_source_index_files(
            (
                runtime_context.checkout_root.as_path(),
                language_id.as_str(),
                provider_id.as_str(),
                SOURCE_INDEX_FILE_LIMIT,
            )
                .into(),
        )?
        .into_iter()
        .map(|file| SourceIndexScopeFile {
            path: file.path,
            language_id: LanguageId::from(file.language_id),
            provider_id: ProviderId::from(file.provider_id),
            selector_receipts: Vec::new(),
        })
        .collect::<Vec<_>>()
    };
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
    context.refresh_generation(SourceIndexGenerationRefresh {
        index_root: &runtime_context.checkout_root,
        files: &files,
        previous_file_hashes: previous_file_hashes.as_deref(),
        registry: &registry,
    })
}

fn runtime_source_index_files_from_previous_hashes(
    index_root: &Path,
    previous_file_hashes: &[ClientCacheFileHash],
    language_id: &LanguageId,
    provider_id: &ProviderId,
) -> Vec<SourceIndexScopeFile> {
    previous_file_hashes
        .iter()
        .filter(|file_hash| !file_hash.path.as_str().starts_with("@scope/"))
        .filter_map(|file_hash| {
            let path = index_root.join(file_hash.path.as_str());
            path.is_file().then(|| SourceIndexScopeFile {
                path,
                language_id: language_id.clone(),
                provider_id: provider_id.clone(),
                selector_receipts: Vec::new(),
            })
        })
        .collect()
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

    fn latest_scope_files(
        &self,
        index_root: &Path,
    ) -> Result<Option<Vec<ClientDbSourceIndexScopeFile>>, String> {
        self.db_session.latest_source_index_scope_files(
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
    let file_count = source_index_previous_file_hash_count(previous_file_hashes);
    if file_count == 0
        || !source_index_registry_evidence_matches(
            previous_file_hashes,
            &scope.registry.fingerprint,
        )
    {
        return Ok(None);
    }
    Ok(context
        .db_session
        .latest_source_index_stats(
            scope.index_root,
            &context.schema_id,
            &context.schema_version,
        )?
        .map(|stats| source_index_refresh_report(&context.db_path, stats, file_count, true)))
}

fn try_reuse_dirty_source_index_scope(
    context: &SourceIndexRefreshContext,
    scope: SourceIndexScopeReuse<'_>,
    dirty_paths: &BTreeSet<PathBuf>,
) -> Result<Option<SourceIndexRefreshReport>, String> {
    let Some(previous_file_hashes) = scope.previous_file_hashes else {
        return Ok(None);
    };
    if dirty_paths.is_empty() {
        return Ok(None);
    }
    let Some(scope_files) = context.latest_scope_files(scope.index_root)? else {
        return Ok(None);
    };
    let scope_files_by_relative_path = scope_files
        .into_iter()
        .filter_map(|file| {
            let relative_path = file
                .path
                .strip_prefix(scope.index_root)
                .ok()
                .map(Path::to_path_buf)?;
            Some((relative_path, file))
        })
        .collect::<BTreeMap<_, _>>();
    let indexed_extensions = scope_files_by_relative_path
        .keys()
        .filter_map(|path| path.extension()?.to_str().map(str::to_owned))
        .collect::<BTreeSet<_>>();
    let mut dirty_scope_files = Vec::with_capacity(dirty_paths.len());
    for dirty_path in dirty_paths {
        let source_path = scope.index_root.join(dirty_path);
        match scope_files_by_relative_path.get(dirty_path) {
            Some(scope_file) if source_path.is_file() => dirty_scope_files.push(scope_file.clone()),
            Some(_) => return Ok(None),
            None if source_path.is_file()
                && source_index_path_matches_indexed_extension(dirty_path, &indexed_extensions) =>
            {
                return Ok(None);
            }
            None => {}
        }
    }
    let dirty_hashes = source_index_file_hashes(
        scope.index_root,
        &dirty_scope_files,
        Some(previous_file_hashes),
        &scope.registry.fingerprint,
        scope.registry.scope_dirs.iter().map(String::as_str),
    )?;
    let previous_by_path = previous_file_hashes
        .iter()
        .map(|file_hash| (file_hash.path.as_str(), file_hash))
        .collect::<BTreeMap<_, _>>();
    let dirty_paths_match_previous = dirty_hashes
        .iter()
        .filter(|file_hash| !file_hash.path.as_str().starts_with("@scope/"))
        .all(|file_hash| {
            previous_by_path
                .get(file_hash.path.as_str())
                .is_some_and(|previous| previous.sha256 == file_hash.sha256)
        });
    if !dirty_paths_match_previous {
        return Ok(None);
    }
    try_reuse_source_index_scope(context, scope)
}

fn source_index_tracked_worktree_dirty_paths(project_root: &Path) -> Option<BTreeSet<PathBuf>> {
    let git_root = source_index_git_root(project_root)?;
    let project_root = project_root.canonicalize().ok()?;
    let project_relative_path = project_root.strip_prefix(&git_root).ok()?.to_path_buf();
    let output = Command::new("git")
        .arg("-C")
        .arg(&git_root)
        .args(["diff", "--name-only", "-z", "HEAD", "--"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    Some(
        output
            .stdout
            .split(|byte| *byte == b'\0')
            .filter(|path| !path.is_empty())
            .filter_map(|path| {
                let path = PathBuf::from(String::from_utf8(path.to_vec()).ok()?);
                if project_relative_path.as_os_str().is_empty() {
                    Some(path)
                } else {
                    path.strip_prefix(&project_relative_path)
                        .ok()
                        .map(Path::to_path_buf)
                }
            })
            .collect(),
    )
}

fn source_index_git_root(project_root: &Path) -> Option<PathBuf> {
    let output = Command::new("git")
        .arg("-C")
        .arg(project_root)
        .args(["rev-parse", "--show-toplevel"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let git_root = String::from_utf8(output.stdout).ok()?;
    PathBuf::from(git_root.trim()).canonicalize().ok()
}

fn source_index_path_matches_indexed_extension(
    path: &Path,
    indexed_extensions: &BTreeSet<String>,
) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| indexed_extensions.contains(extension))
}

fn source_index_previous_file_hash_count(previous_file_hashes: &[ClientCacheFileHash]) -> usize {
    previous_file_hashes
        .iter()
        .filter(|hash| !hash.path.as_str().starts_with("@scope/"))
        .count()
}

fn source_index_registry_evidence_matches(
    previous_file_hashes: &[ClientCacheFileHash],
    registry_fingerprint: &str,
) -> bool {
    let expected_sha256 = format!("{:x}", Sha256::digest(registry_fingerprint.as_bytes()));
    let expected_byte_len = registry_fingerprint.len().min(u64::MAX as usize) as u64;
    previous_file_hashes.iter().any(|hash| {
        hash.path == "@scope/registry"
            && hash.sha256 == expected_sha256
            && hash.byte_len == expected_byte_len
            && hash.mtime_ms == 0
    })
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

#[cfg(test)]
#[path = "../../tests/unit/source_index_api.rs"]
mod source_index_api_tests;
use sha2::{Digest, Sha256};
