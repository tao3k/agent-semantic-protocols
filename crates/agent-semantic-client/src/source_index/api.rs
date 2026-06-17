//! Public refresh API for the Rust SQL source index.

use std::collections::BTreeSet;
use std::fs;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use agent_semantic_client_core::{
    CacheGenerationId, LanguageId, ProjectContext, ProviderId, ProviderRegistrySnapshot,
    ResolvedProvider, SemanticSchemaId, SemanticSchemaVersion,
};
use agent_semantic_client_db::{ClientDb, ClientDbSourceIndexStats};

use super::collect::collect_source_index_files;
use super::config::{SOURCE_INDEX_FILE_LIMIT, SOURCE_INDEX_SCHEMA_ID, SOURCE_INDEX_SCHEMA_VERSION};
use super::import::{source_index_file_hashes, source_index_import_with_file_hashes};
use super::model::{SourceIndexRefreshReport, SourceIndexScopeFile};

/// Refresh the Rust SQL source index for a project without storing raw source.
pub fn refresh_source_index(project_root: &Path) -> Result<SourceIndexRefreshReport, String> {
    let project_context = ProjectContext::resolve(project_root)?;
    project_context.require_inside_workspace(project_root)?;
    let db_path = ClientDb::default_path(project_context.state_layout().client_cache_dir());
    let snapshot = ProviderRegistrySnapshot::load(project_root)?;
    let mut db = ClientDb::open_or_create(&db_path)?;
    let schema_id = SemanticSchemaId::from(SOURCE_INDEX_SCHEMA_ID);
    let schema_version = SemanticSchemaVersion::from(SOURCE_INDEX_SCHEMA_VERSION);
    let registry_fingerprint = provider_registry_fingerprint(&snapshot);
    let registry_scope_dirs = provider_registry_scope_dirs(project_root, &snapshot);
    let previous_file_hashes =
        db.latest_source_index_file_hashes(project_root, &schema_id, &schema_version)?;
    if let Some(report) = try_reuse_source_index_scope(
        &db,
        SourceIndexScopeReuse {
            db_path: &db_path,
            index_root: project_root,
            schema_id: &schema_id,
            schema_version: &schema_version,
            previous_file_hashes: previous_file_hashes.as_deref(),
            registry_fingerprint: &registry_fingerprint,
            registry_scope_dirs: &registry_scope_dirs,
        },
    )? {
        return Ok(report);
    }
    let files = collect_source_index_files(project_root, &snapshot)?;
    let file_hashes = source_index_file_hashes(
        project_root,
        &files,
        previous_file_hashes.as_deref(),
        &registry_fingerprint,
        &registry_scope_dirs,
    )?;
    if let Some(stats) = db.reusable_source_index_generation(
        project_root,
        &schema_id,
        &schema_version,
        &file_hashes,
    )? {
        return Ok(SourceIndexRefreshReport {
            db_path,
            generation_id: stats.generation_id,
            reused_generation: true,
            file_count: files.len().min(u32::MAX as usize) as u32,
            owner_count: stats.owner_count,
            selector_count: stats.selector_count,
        });
    }
    let generation_id = source_index_generation_id();
    let import = source_index_import_with_file_hashes(
        project_root,
        generation_id.clone(),
        &files,
        file_hashes,
    )?;
    let stats = db.replace_source_index(&import)?;
    let reused_generation = stats.generation_id != generation_id;
    Ok(SourceIndexRefreshReport {
        db_path,
        generation_id: stats.generation_id,
        reused_generation,
        file_count: files.len().min(u32::MAX as usize) as u32,
        owner_count: stats.owner_count,
        selector_count: stats.selector_count,
    })
}

/// Refresh source-index rows for an ASP-managed runtime source checkout.
pub fn refresh_runtime_source_index(
    project_root: &Path,
    checkout_root: &Path,
    language_id: &LanguageId,
    provider_id: &ProviderId,
) -> Result<SourceIndexRefreshReport, String> {
    let project_context = ProjectContext::resolve(project_root)?;
    project_context.require_inside_workspace(project_root)?;
    let client_cache_dir = project_context.state_layout().client_cache_dir();
    let checkout_root = fs::canonicalize(checkout_root).map_err(|error| {
        format!(
            "failed to resolve runtime source checkout {}: {error}",
            checkout_root.display()
        )
    })?;
    let canonical_cache_dir = fs::canonicalize(client_cache_dir).map_err(|error| {
        format!(
            "failed to resolve ASP client cache dir {}: {error}",
            client_cache_dir.display()
        )
    })?;
    if !checkout_root.starts_with(&canonical_cache_dir) {
        return Err(format!(
            "runtime source checkout {} is outside ASP client cache {}",
            checkout_root.display(),
            canonical_cache_dir.display()
        ));
    }

    let files = collect_runtime_source_index_files(&checkout_root, language_id, provider_id)?;
    if files.is_empty() {
        return Err(format!(
            "runtime source index found no source files in {} for language {}",
            checkout_root.display(),
            language_id
        ));
    }
    let db_path = ClientDb::default_path(client_cache_dir);
    let mut db = ClientDb::open_or_create(&db_path)?;
    let schema_id = SemanticSchemaId::from(SOURCE_INDEX_SCHEMA_ID);
    let schema_version = SemanticSchemaVersion::from(SOURCE_INDEX_SCHEMA_VERSION);
    let registry_fingerprint =
        runtime_source_registry_fingerprint(&checkout_root, language_id, provider_id);
    let registry_scope_dirs = BTreeSet::new();
    let previous_file_hashes =
        db.latest_source_index_file_hashes(&checkout_root, &schema_id, &schema_version)?;
    let file_hashes = source_index_file_hashes(
        &checkout_root,
        &files,
        previous_file_hashes.as_deref(),
        &registry_fingerprint,
        &registry_scope_dirs,
    )?;
    if let Some(stats) = db.reusable_source_index_generation(
        &checkout_root,
        &schema_id,
        &schema_version,
        &file_hashes,
    )? {
        return Ok(SourceIndexRefreshReport {
            db_path,
            generation_id: stats.generation_id,
            reused_generation: true,
            file_count: files.len().min(u32::MAX as usize) as u32,
            owner_count: stats.owner_count,
            selector_count: stats.selector_count,
        });
    }
    let generation_id = source_index_generation_id();
    let import = source_index_import_with_file_hashes(
        &checkout_root,
        generation_id.clone(),
        &files,
        file_hashes,
    )?;
    let stats = db.replace_source_index(&import)?;
    let reused_generation = stats.generation_id != generation_id;
    Ok(SourceIndexRefreshReport {
        db_path,
        generation_id: stats.generation_id,
        reused_generation,
        file_count: files.len().min(u32::MAX as usize) as u32,
        owner_count: stats.owner_count,
        selector_count: stats.selector_count,
    })
}

struct SourceIndexScopeReuse<'a> {
    db_path: &'a Path,
    index_root: &'a Path,
    schema_id: &'a SemanticSchemaId,
    schema_version: &'a SemanticSchemaVersion,
    previous_file_hashes: Option<&'a [agent_semantic_client_core::ClientCacheFileHash]>,
    registry_fingerprint: &'a str,
    registry_scope_dirs: &'a BTreeSet<String>,
}

fn try_reuse_source_index_scope(
    db: &ClientDb,
    scope: SourceIndexScopeReuse<'_>,
) -> Result<Option<SourceIndexRefreshReport>, String> {
    let Some(previous_file_hashes) = scope.previous_file_hashes else {
        return Ok(None);
    };
    let files = match latest_scope_files_from_owners(
        db,
        scope.index_root,
        scope.schema_id,
        scope.schema_version,
    )? {
        Some(files) => files,
        None => return Ok(None),
    };
    let file_hashes = match source_index_file_hashes(
        scope.index_root,
        &files,
        Some(previous_file_hashes),
        scope.registry_fingerprint,
        scope.registry_scope_dirs,
    ) {
        Ok(file_hashes) => file_hashes,
        Err(_) => return Ok(None),
    };
    let Some(stats) = db.reusable_source_index_generation(
        scope.index_root,
        scope.schema_id,
        scope.schema_version,
        &file_hashes,
    )?
    else {
        return Ok(None);
    };
    Ok(Some(source_index_refresh_report(
        scope.db_path,
        stats,
        files.len(),
        true,
    )))
}

fn latest_scope_files_from_owners(
    db: &ClientDb,
    index_root: &Path,
    schema_id: &SemanticSchemaId,
    schema_version: &SemanticSchemaVersion,
) -> Result<Option<Vec<SourceIndexScopeFile>>, String> {
    let owners = db.latest_source_index_generation_owners(index_root, schema_id, schema_version)?;
    if owners.is_empty() {
        return Ok(None);
    }
    let mut files = Vec::new();
    for owner in owners {
        if owner.source_kind.as_str() != "file" {
            continue;
        }
        let (Some(language_id), Some(provider_id)) = (owner.language_id, owner.provider_id) else {
            return Ok(None);
        };
        files.push(SourceIndexScopeFile {
            path: index_root.join(owner.owner_path.as_str()),
            language_id,
            provider_id,
        });
    }
    if files.is_empty() {
        Ok(None)
    } else {
        Ok(Some(files))
    }
}

fn source_index_refresh_report(
    db_path: &Path,
    stats: ClientDbSourceIndexStats,
    file_count: usize,
    reused_generation: bool,
) -> SourceIndexRefreshReport {
    SourceIndexRefreshReport {
        db_path: db_path.to_path_buf(),
        generation_id: stats.generation_id,
        reused_generation,
        file_count: file_count.min(u32::MAX as usize) as u32,
        owner_count: stats.owner_count,
        selector_count: stats.selector_count,
    }
}

fn provider_registry_fingerprint(snapshot: &ProviderRegistrySnapshot) -> String {
    let mut rows = vec![format!("activation={}", snapshot.activation_path.display())];
    for provider in &snapshot.providers {
        rows.push(provider_fingerprint(provider));
    }
    rows.join("\n")
}

fn provider_registry_scope_dirs(
    project_root: &Path,
    snapshot: &ProviderRegistrySnapshot,
) -> BTreeSet<String> {
    let mut dirs = BTreeSet::new();
    dirs.insert(".".to_string());
    for provider in &snapshot.providers {
        let package_roots = if provider.package_roots.is_empty() {
            vec![".".to_string()]
        } else {
            provider.package_roots.clone()
        };
        for package_root in package_roots {
            insert_existing_scope_dir(project_root, &project_root.join(&package_root), &mut dirs);
            for source_root in &provider.source_roots {
                insert_existing_scope_dir(
                    project_root,
                    &project_root.join(&package_root).join(source_root),
                    &mut dirs,
                );
            }
            for config_file in &provider.config_files {
                if let Some(parent) = project_root.join(&package_root).join(config_file).parent() {
                    insert_existing_scope_dir(project_root, parent, &mut dirs);
                }
            }
        }
    }
    dirs
}

fn insert_existing_scope_dir(project_root: &Path, dir: &Path, dirs: &mut BTreeSet<String>) {
    if !dir.is_dir() {
        return;
    }
    let relative = dir
        .strip_prefix(project_root)
        .ok()
        .and_then(|path| path.to_str())
        .filter(|value| !value.is_empty())
        .unwrap_or(".");
    dirs.insert(relative.replace(std::path::MAIN_SEPARATOR, "/"));
}

fn provider_fingerprint(provider: &ResolvedProvider) -> String {
    [
        format!("language={}", provider.language_id),
        format!("provider={}", provider.provider_id),
        format!("binary={}", provider.binary),
        format!("execution={:?}", provider.execution),
        format!("prefix={}", provider.provider_command_prefix.join("\u{1f}")),
        format!(
            "runtime={}",
            provider
                .runtime_command_argv
                .as_ref()
                .map(|argv| argv.join("\u{1f}"))
                .unwrap_or_default()
        ),
        format!(
            "runtimeStatus={}",
            provider
                .runtime_profile_status
                .map(|status| status.as_str())
                .unwrap_or_default()
        ),
        format!("packageRoots={}", provider.package_roots.join("\u{1f}")),
        format!("sourceRoots={}", provider.source_roots.join("\u{1f}")),
        format!("configFiles={}", provider.config_files.join("\u{1f}")),
        format!(
            "sourceExtensions={}",
            provider.source_extensions.join("\u{1f}")
        ),
        format!(
            "ignoredPathPrefixes={}",
            provider.ignored_path_prefixes.join("\u{1f}")
        ),
    ]
    .join("\u{1e}")
}

fn runtime_source_registry_fingerprint(
    checkout_root: &Path,
    language_id: &LanguageId,
    provider_id: &ProviderId,
) -> String {
    format!(
        "runtimeSource\ngenerationRoot={}\nlanguage={}\nprovider={}",
        checkout_root.display(),
        language_id,
        provider_id
    )
}

fn source_index_generation_id() -> CacheGenerationId {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0);
    CacheGenerationId::from(format!("source-index-{nanos}"))
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
