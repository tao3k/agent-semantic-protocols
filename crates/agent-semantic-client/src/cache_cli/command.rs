//! `asp cache` maintenance command implementation.

use std::{
    fs,
    path::{Path, PathBuf},
};

use agent_semantic_client_core::{
    AGENT_SEMANTIC_CLIENT_CACHE_MANIFEST_PROTOCOL_ID,
    AGENT_SEMANTIC_CLIENT_CACHE_MANIFEST_PROTOCOL_VERSION,
    AGENT_SEMANTIC_CLIENT_CACHE_MANIFEST_SCHEMA_ID,
    AGENT_SEMANTIC_CLIENT_CACHE_MANIFEST_SCHEMA_VERSION, CacheManifestReport, CacheManifestStatus,
    ClientCacheManifest, ClientCachePath, ClientDbBackend, ClientDbEngineDurability,
    ClientDbEngineFeaturesReceipt, ClientDbEngineReceipt, ClientDbFileName, ClientDbStatus,
    ClientMethod, ClientReceipt, ClientRepoId, ClientScopeId, ClientStateLayoutVersion,
    ClientWorkspaceId, LanguageId, ProjectContext, ProviderId, ProviderRegistrySnapshot,
    StateLayout,
};
use agent_semantic_client_db::{ClientDbEngine, ClientDbEngineReport, ClientDbReport};
use serde_json::json;

use super::source_index_evidence::{
    source_index_lookup_artifact_evidence,
};
use super::structural_index_import::import_structural_index_artifacts;
use crate::source_index::{
    SourceIndexLookupRequest, lookup_source_index_in_cache,
};


pub(crate) fn run_cache(
    project_root: &Path,
    facade_language_id: Option<&LanguageId>,
    forwarded_args: &[String],
    receipt_json: bool,
) -> Result<(), String> {
    match forwarded_args {
        [subcommand, rest @ ..] if subcommand == "migrate" => {
            super::run_cache_migration(project_root, rest, receipt_json)
        }
        [subcommand, rest @ ..] if subcommand == "gc" => {
            super::project_registry_gc_command::run_project_registry_gc(
                project_root,
                rest,
                receipt_json,
            )
        }
        [subcommand] if subcommand == "status" => {
            let snapshot = ProviderRegistrySnapshot::load(project_root);
            let provenance = snapshot
                .as_ref()
                .map_or_else(|_| Vec::new(), ProviderRegistrySnapshot::native_provenance);
            let cache_report = ClientCacheManifest::inspect_project(project_root);
            let db_engine_report = ClientDbEngine::resolve(project_root)
                .ok()
                .map(|engine| engine.inspect());
            let db_engine_available = db_engine_report.is_some();
            let active_db_report = db_engine_report
                .as_ref()
                .map(|report| ClientDbEngine::inspect_client_dir(report.client_dir()));
            let mut receipt = ClientReceipt::cache_status(provenance, &cache_report);
            if let Some(db_engine_report) = &db_engine_report {
                apply_db_engine_report_to_receipt(&mut receipt, db_engine_report);
            }
            receipt.db_read_count = Some(u64::from(db_engine_available));
            receipt.db_write_count = Some(0);
            let (activation, provider_count) = match &snapshot {
                Ok(snapshot) => (
                    snapshot.activation_path.display().to_string(),
                    snapshot.providers.len(),
                ),
                Err(error) => {
                    if !receipt_json {
                        eprintln!("[asp-cache] activation unavailable: {error}");
                    }
                    ("missing".to_string(), 0)
                }
            };
            println!(
                "[asp-cache] status={} route=local-cache activation={} providers={} cacheRoot={} manifest={} generations={} rawSourceStored={}",
                cache_status_line(&cache_report, active_db_report.as_ref()),
                activation,
                provider_count,
                display_optional_path(cache_report.cache_root.as_deref()),
                cache_report.status.as_str(),
                cache_report.generation_count,
                cache_report.raw_source_stored
            );
            println!(
                "|cache manifestPath={} cacheManifestStatus={}",
                display_optional_path(cache_report.manifest_path.as_deref()),
                cache_report.status.as_str()
            );
            print_db_engine_status(db_engine_report.as_ref());
            print_db_status(active_db_report.as_ref());
            print_cache_reason(&cache_report);
            println!("|reason phase=db-engine-turso arrow=false providerCommands=0");
            if snapshot.is_err() {
                println!("|cmd install=asp install plugin --codex .");
                println!("|cmd guide=asp guide");
            }
            if receipt_json {
                let receipt = serde_json::to_string(&receipt)
                    .map_err(|error| format!("failed to serialize receipt: {error}"))?;
                eprintln!("{receipt}");
            }
            Ok(())
        }
        [subcommand] if subcommand == "import" => {
            let state_layout = cache_state_layout(project_root)?;
            let snapshot = ProviderRegistrySnapshot::load(project_root);
            let provenance = snapshot
                .as_ref()
                .map_or_else(|_| Vec::new(), ProviderRegistrySnapshot::native_provenance);
            let cache_report = ClientCacheManifest::inspect_project(project_root);
            let manifest = ClientCacheManifest::load_from_path(state_layout.cache_manifest_path())?;
            let cache_root = state_layout.client_cache_dir();
            ClientDbEngine::import_manifest_from_client_dir(cache_root, &manifest)?;
            let source_snapshot =
                crate::source_index::current_source_index_snapshot(project_root)?;
            let structural_index_imported_count =
                import_structural_index_artifacts(
                    cache_root,
                    &manifest,
                    &source_snapshot.source_snapshot,
                )?;
            let db_report = ClientDbEngine::inspect_client_dir(cache_root);
            let mut receipt =
                ClientReceipt::cache_report(ClientMethod::CacheImport, provenance, &cache_report);
            apply_project_db_report_to_receipt(&mut receipt, project_root, &db_report);
            receipt.db_read_count = Some(0);
            receipt.db_write_count = Some(0);
            println!(
                "[asp-cache] status=imported route=local-cache cacheRoot={} manifest={} generations={} rawSourceStored={} structuralIndexImported={}",
                display_optional_path(cache_report.cache_root.as_deref()),
                cache_report.status.as_str(),
                cache_report.generation_count,
                cache_report.raw_source_stored,
                structural_index_imported_count
            );
            println!(
                "|cache manifestPath={} cacheManifestStatus={}",
                display_optional_path(cache_report.manifest_path.as_deref()),
                cache_report.status.as_str()
            );
            print_db_status(Some(&db_report));
            println!("|reason phase=db-engine-turso action=import arrow=false providerCommands=0");
            if receipt_json {
                let receipt = serde_json::to_string(&receipt)
                    .map_err(|error| format!("failed to serialize receipt: {error}"))?;
                eprintln!("{receipt}");
            }
            Ok(())
        }
        [subcommand, action, rest @ ..] if subcommand == "source-index" && action == "lookup" => {
            let spec = parse_source_index_lookup_args(project_root, rest)?;
            let source_snapshot = if let Some(index_owner) = spec.index_owner.as_deref() {
                let language_id = facade_language_id.ok_or_else(|| {
                    "--index-owner requires a language-scoped `asp <language> cache source-index lookup` request"
                        .to_string()
                })?;
                crate::source_index::current_runtime_source_index_snapshot(
                    project_root,
                    &spec.index_root,
                    language_id,
                    &ProviderId::from(index_owner),
                )?
            } else {
                crate::source_index::current_source_index_snapshot(&spec.index_root)?
            };
            let result = lookup_source_index_in_cache(SourceIndexLookupRequest {
                cache_project_root: project_root,
                indexed_project_root: &spec.index_root,
                language_id: facade_language_id,
    query: &spec.query,
    limit: spec.limit,
    source_snapshot: &source_snapshot.source_snapshot,
})?;
            if result.candidates.is_empty() {
                println!(
                    "noOutput reason=source-index-{} query={} indexRoot={} snapshotRoot={} providerDigest={} indexArtifactDigest={}",
                    result.state.as_str(),
                    spec.query,
                    spec.index_root.display(),
                    result
                        .source_snapshot
                        .as_ref()
                        .map_or("-", |snapshot| snapshot.root_digest.as_str()),
                    result
                        .source_snapshot
                        .as_ref()
                        .map_or("-", |snapshot| snapshot.provider_digest.as_str()),
                    result.index_artifact_digest.as_deref().unwrap_or("-")
                );
            } else {
                println!(
                    "[asp-cache-source-index] status={} route=local-cache db={} indexRoot={} query={} candidates={} snapshotRoot={} providerDigest={} indexArtifactDigest={} rawSourceStored=false",
                    result.state.as_str(),
                    result.db_path.display(),
                    spec.index_root.display(),
                    spec.query,
                    result.candidates.len(),
                    result
                        .source_snapshot
                        .as_ref()
                        .map_or("-", |snapshot| snapshot.root_digest.as_str()),
                    result
                        .source_snapshot
                        .as_ref()
                        .map_or("-", |snapshot| snapshot.provider_digest.as_str()),
                    result.index_artifact_digest.as_deref().unwrap_or("-")
                );
                for candidate in &result.candidates {
                    println!(
                        "|candidate path={} language={} provider={} kind={} lines={} queryKeys={}",
                        candidate.path,
                        candidate
                            .language_id
                            .as_ref()
                            .map_or("-", LanguageId::as_str),
                        candidate
                            .provider_id
                            .as_ref()
                            .map_or("-", ProviderId::as_str),
                        candidate.source_kind.as_str(),
                        candidate
                            .line_count
                            .map(|count| count.to_string())
                            .unwrap_or_else(|| "-".to_string()),
                        candidate
                            .query_keys
                            .iter()
                            .map(|key| key.as_str())
                            .collect::<Vec<_>>()
                            .join(",")
                    );
                }
            }
            if receipt_json {
                let receipt = json!({
                    "schemaId": "agent.semantic-protocols.semantic-source-index.lookup-receipt",
                    "schemaVersion": "1",
                    "status": result.state.as_str(),
                    "route": "local-cache",
                    "dbPath": result.db_path.display().to_string(),
                    "indexRoot": spec.index_root.display().to_string(),
                    "indexOwner": spec.index_owner,
                    "query": spec.query,
                    "limit": spec.limit,
                    "sourceSnapshot": result.source_snapshot.as_ref(),
                    "indexArtifactDigest": result.index_artifact_digest.as_deref(),
                    "artifactEvidence": source_index_lookup_artifact_evidence(&result),
                    "rawSourceStored": false,
                    "candidates": result.candidates.iter().map(|candidate| {
                        json!({
                            "path": *candidate.path,
                            "languageId": candidate.language_id.as_ref().map(LanguageId::as_str),
                            "providerId": candidate.provider_id.as_ref().map(ProviderId::as_str),
                            "sourceKind": candidate.source_kind.as_str(),
                            "lineCount": candidate.line_count,
                            "queryKeys": candidate
                                .query_keys
                                .iter()
                                .map(|key| key.as_str())
                                .collect::<Vec<_>>()
                        })
                    }).collect::<Vec<_>>()
                });
                eprintln!("{receipt}");
            }
            Ok(())
        }
        [subcommand, scope] if subcommand == "flush" && scope == "syntax-rows" => {
            let state_layout = cache_state_layout(project_root)?;
            let snapshot = ProviderRegistrySnapshot::load(project_root);
            let provenance = snapshot
                .as_ref()
                .map_or_else(|_| Vec::new(), ProviderRegistrySnapshot::native_provenance);
            let cache_root = state_layout.client_cache_dir();
            let flushed_syntax_rows =
                ClientDbEngine::flush_syntax_query_rows_from_client_dir(cache_root)?;
            let updated_cache_report = ClientCacheManifest::inspect_project(project_root);
            let db_report = ClientDbEngine::inspect_client_dir(cache_root);
            let mut receipt = ClientReceipt::cache_report(
                ClientMethod::CacheFlush,
                provenance,
                &updated_cache_report,
            );
            receipt.cache_status = agent_semantic_client_core::CacheStatus::Invalidated;
            apply_project_db_report_to_receipt(&mut receipt, project_root, &db_report);
            receipt.db_read_count = Some(1);
            receipt.db_write_count = Some(1);
            println!(
                "[asp-cache] status=flushed route=local-cache cacheRoot={} manifest={} generations={} rawSourceStored={} flushedSyntaxRows={}",
                display_optional_path(updated_cache_report.cache_root.as_deref()),
                updated_cache_report.status.as_str(),
                updated_cache_report.generation_count,
                updated_cache_report.raw_source_stored,
                flushed_syntax_rows
            );
            println!(
                "|cache manifestPath={} cacheManifestStatus={}",
                display_optional_path(updated_cache_report.manifest_path.as_deref()),
                updated_cache_report.status.as_str()
            );
            print_db_status(Some(&db_report));
            println!(
                "|reason phase=db-engine-turso action=flush-syntax-rows manifestArtifactsDeleted=false providerCommands=0"
            );
            if receipt_json {
                let receipt = serde_json::to_string(&receipt)
                    .map_err(|error| format!("failed to serialize receipt: {error}"))?;
                eprintln!("{receipt}");
            }
            Ok(())
        }
        [subcommand] if subcommand == "invalidate" || subcommand == "flush" => {
            let state_layout = cache_state_layout(project_root)?;
            let is_flush = subcommand == "flush";
            let action = if is_flush { "flush" } else { "invalidate" };
            let status = if is_flush { "flushed" } else { "invalidated" };
            let count_label = if is_flush {
                "flushedGenerations"
            } else {
                "invalidatedGenerations"
            };
            let snapshot = ProviderRegistrySnapshot::load(project_root);
            let provenance = snapshot
                .as_ref()
                .map_or_else(|_| Vec::new(), ProviderRegistrySnapshot::native_provenance);
            let cache_report = ClientCacheManifest::inspect_project(project_root);
            let cache_root = state_layout.client_cache_dir();
            let db_invalidated_generation_count =
                ClientDbEngine::invalidate_generations_for_project_from_client_dir(
                    cache_root,
                    project_root,
                )?;
            let manifest_invalidated_generation_count =
                clear_manifest_generations(&cache_report, &state_layout, project_root)?;
            let invalidated_generation_count =
                db_invalidated_generation_count.max(manifest_invalidated_generation_count);
            let updated_cache_report = ClientCacheManifest::inspect_project(project_root);
            let db_report = ClientDbEngine::inspect_client_dir(cache_root);
            let receipt_method = if is_flush {
                ClientMethod::CacheFlush
            } else {
                ClientMethod::CacheInvalidate
            };
            let mut receipt =
                ClientReceipt::cache_report(receipt_method, provenance, &updated_cache_report);
            receipt.cache_status = agent_semantic_client_core::CacheStatus::Invalidated;
            apply_project_db_report_to_receipt(&mut receipt, project_root, &db_report);
            receipt.db_read_count = Some(1);
            receipt.db_write_count = Some(1);
            println!(
                "[asp-cache] status={} route=local-cache cacheRoot={} manifest={} generations={} rawSourceStored={} {}={}",
                status,
                display_optional_path(updated_cache_report.cache_root.as_deref()),
                updated_cache_report.status.as_str(),
                updated_cache_report.generation_count,
                updated_cache_report.raw_source_stored,
                count_label,
                invalidated_generation_count
            );
            println!(
                "|cache manifestPath={} cacheManifestStatus={}",
                display_optional_path(updated_cache_report.manifest_path.as_deref()),
                updated_cache_report.status.as_str()
            );
            print_db_status(Some(&db_report));
            println!(
                "|reason phase=db-engine-turso action={} manifestArtifactsDeleted=false providerCommands=0",
                action
            );
            if receipt_json {
                let receipt = serde_json::to_string(&receipt)
                    .map_err(|error| format!("failed to serialize receipt: {error}"))?;
                eprintln!("{receipt}");
            }
            Ok(())
        }
        _ => Err(
            "usage: asp cache <status|gc [--grace-days <n>] [--apply]|import|source-index lookup --query <term> [--index-root <path>] [--index-owner <provider>] [--limit <n>]|invalidate|flush [syntax-rows]>; use asp <language> cache source-index lookup ... for language-scoped lookup"
                .to_string(),
        ),
    }
}


struct SourceIndexLookupSpec {
    query: String,
    index_root: PathBuf,
    index_owner: Option<String>,
    limit: u32,
}

fn parse_source_index_lookup_args(
    project_root: &Path,
    args: &[String],
) -> Result<SourceIndexLookupSpec, String> {
    let mut query = None;
    let mut index_root = None;
    let mut index_owner = None;
    let mut limit = None;
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--query" => query = Some(next_flag_value("--query", &mut iter)?),
            "--index-root" => index_root = Some(next_flag_value("--index-root", &mut iter)?),
            "--index-owner" => index_owner = Some(next_flag_value("--index-owner", &mut iter)?),
            "--limit" => {
                let value = next_flag_value("--limit", &mut iter)?;
                limit = Some(
                    value
                        .parse::<u32>()
                        .map_err(|error| format!("invalid --limit `{value}`: {error}"))?,
                );
            }
            other => return Err(format!("unexpected source-index lookup argument: {other}")),
        }
    }
    let index_root = index_root
        .map(PathBuf::from)
        .map(|path| {
            if path.is_absolute() {
                path
            } else {
                project_root.join(path)
            }
        })
        .unwrap_or_else(|| project_root.to_path_buf());
    Ok(SourceIndexLookupSpec {
        query: query.ok_or_else(|| "--query is required".to_string())?,
        index_root,
        index_owner,
        limit: limit.unwrap_or(8),
    })
}


fn next_flag_value<'a>(
    flag: &str,
    iter: &mut impl Iterator<Item = &'a String>,
) -> Result<String, String> {
    let value = iter
        .next()
        .ok_or_else(|| format!("{flag} requires a value"))?;
    if value.starts_with('-') {
        Err(format!("{flag} requires a value"))
    } else {
        Ok(value.clone())
    }
}


fn clear_manifest_generations(
    cache_report: &CacheManifestReport,
    state_layout: &StateLayout,
    project_root: &Path,
) -> Result<u32, String> {
    let manifest_path = state_layout.cache_manifest_path();
    if cache_report.status == CacheManifestStatus::Invalid {
        write_cache_manifest(
            manifest_path,
            &empty_cache_manifest(state_layout.client_cache_dir()),
        )?;
        return Ok(0);
    }
    if cache_report.status != CacheManifestStatus::Present {
        return Ok(0);
    }
    let mut manifest = ClientCacheManifest::load_from_path(manifest_path)?;
    let project_root = normalized_project_root(project_root);
    let before = manifest.generations.len();
    manifest.generations.retain(|generation| {
        !manifest_project_root_matches(&generation.project_root, &project_root)
    });
    let invalidated = before
        .saturating_sub(manifest.generations.len())
        .min(u32::MAX as usize) as u32;
    if invalidated == 0 {
        return Ok(0);
    }
    write_cache_manifest(manifest_path, &manifest)?;
    Ok(invalidated)
}

fn normalized_project_root(project_root: &Path) -> String {
    project_root
        .canonicalize()
        .unwrap_or_else(|_| project_root.to_path_buf())
        .display()
        .to_string()
}

fn manifest_project_root_matches(candidate: &str, project_root: &str) -> bool {
    candidate == project_root || normalized_project_root(Path::new(candidate)) == project_root
}

fn empty_cache_manifest(cache_root: &Path) -> ClientCacheManifest {
    ClientCacheManifest {
        schema_id: AGENT_SEMANTIC_CLIENT_CACHE_MANIFEST_SCHEMA_ID.into(),
        schema_version: AGENT_SEMANTIC_CLIENT_CACHE_MANIFEST_SCHEMA_VERSION.into(),
        protocol_id: AGENT_SEMANTIC_CLIENT_CACHE_MANIFEST_PROTOCOL_ID.into(),
        protocol_version: AGENT_SEMANTIC_CLIENT_CACHE_MANIFEST_PROTOCOL_VERSION.into(),
        cache_root: ClientCachePath::from_path(cache_root),
        generations: Vec::new(),
    }
}

fn write_cache_manifest(
    manifest_path: &Path,
    manifest: &ClientCacheManifest,
) -> Result<(), String> {
    if let Some(parent) = manifest_path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("failed to create cache manifest dir: {error}"))?;
    }
    let text = serde_json::to_string_pretty(manifest)
        .map_err(|error| format!("failed to serialize cache manifest: {error}"))?;
    fs::write(manifest_path, text)
        .map_err(|error| format!("failed to write cache manifest: {error}"))
}

fn cache_state_layout(project_root: &Path) -> Result<StateLayout, String> {
    Ok(ProjectContext::resolve(project_root)?
        .state_layout()
        .clone())
}

fn cache_status_line(
    cache_report: &CacheManifestReport,
    db_report: Option<&ClientDbReport>,
) -> &'static str {
    match cache_report.status {
        CacheManifestStatus::Unavailable => "unavailable",
        CacheManifestStatus::Missing => match db_report {
            Some(report) if db_report_has_indexed_content(report) => "available",
            _ => "missing",
        },
        CacheManifestStatus::Invalid => "invalid",
        CacheManifestStatus::Present => match db_report {
            Some(report)
                if report.status == ClientDbStatus::Present && report.generation_count > 0 =>
            {
                "available"
            }
            Some(report) if report.status == ClientDbStatus::Invalid => "invalid",
            Some(_) | None => "unimported",
        },
    }
}

fn db_report_has_indexed_content(report: &ClientDbReport) -> bool {
    report.status == ClientDbStatus::Present
        && (report.generation_count > 0
            || report.source_index_generation_count > 0
            || report.source_index_owner_count > 0
            || report.source_index_selector_count > 0)
}

fn print_db_engine_status(engine_report: Option<&ClientDbEngineReport>) {
    if let Some(engine_report) = engine_report {
        println!(
            "|dbEngine backend={} layoutVersion={} repoId={} workspaceId={} scopeId={} clientDir={} manifestPath={} dbPath={} artifactPath={}",
            engine_report.backend(),
            engine_report.layout_version(),
            engine_report.repo_id(),
            engine_report.workspace_id(),
            engine_report.scope_id(),
            engine_report.client_dir().display(),
            engine_report.manifest_path().display(),
            engine_report.db_path().display(),
            engine_report.artifact_path().display()
        );
    } else {
        println!(
            "|dbEngine backend=unavailable layoutVersion=unavailable repoId=unavailable workspaceId=unavailable scopeId=unavailable clientDir=unavailable manifestPath=unavailable dbPath=unavailable artifactPath=unavailable"
        );
    }
}

fn print_db_status(db_report: Option<&ClientDbReport>) {
    if let Some(db_report) = db_report {
        let runtime_pragmas = db_report
            .runtime_pragmas
            .as_ref()
            .map(|pragmas| {
                format!(
                    " journalMode={} synchronous={} busyTimeoutMs={} foreignKeys={}",
                    pragmas.journal_mode(),
                    pragmas.synchronous(),
                    pragmas.busy_timeout_ms(),
                    pragmas.foreign_keys()
                )
            })
            .unwrap_or_default();
        println!(
            "|db path={} status={} generations={} syntaxRows={}/{}/{} sourceIndex={}/{}/{} artifactEvents={} rawSourceStored={}{}",
            db_report.db_path.display(),
            db_report.status.as_str(),
            db_report.generation_count,
            db_report.syntax_row_generation_count,
            db_report.syntax_row_match_count,
            db_report.syntax_row_capture_count,
            db_report.source_index_generation_count,
            db_report.source_index_owner_count,
            db_report.source_index_selector_count,
            db_report.artifact_event_count,
            db_report.raw_source_stored,
            runtime_pragmas
        );
        if let Some(reason) = &db_report.reason {
            println!(
                "|reason clientDb={} detail={}",
                db_report.status.as_str(),
                compact_detail(reason)
            );
        }
    } else {
        println!(
            "|db path=unavailable status=unavailable generations=0 syntaxRows=0/0/0 sourceIndex=0/0/0 artifactEvents=0 rawSourceStored=false journalMode=unknown synchronous=unknown busyTimeoutMs=unknown foreignKeys=false"
        );
    }
}

fn apply_db_report_to_receipt(receipt: &mut ClientReceipt, db_report: &ClientDbReport) {
    receipt.client_db_path = Some(ClientCachePath::from_path(&db_report.db_path));
    receipt.client_db_status = Some(db_report.status.clone());
    receipt.client_db_generation_count = Some(db_report.generation_count);
    receipt.client_db_syntax_row_generation_count = Some(db_report.syntax_row_generation_count);
    receipt.client_db_syntax_row_match_count = Some(db_report.syntax_row_match_count);
    receipt.client_db_syntax_row_capture_count = Some(db_report.syntax_row_capture_count);
    receipt.client_db_source_index_generation_count = Some(db_report.source_index_generation_count);
    receipt.client_db_source_index_owner_count = Some(db_report.source_index_owner_count);
    receipt.client_db_source_index_selector_count = Some(db_report.source_index_selector_count);
    receipt.client_db_artifact_event_count = Some(db_report.artifact_event_count);
    receipt.client_db_raw_source_stored = Some(db_report.raw_source_stored);
    if let Some(pragmas) = &db_report.runtime_pragmas {
        receipt.client_db_journal_mode = Some(pragmas.journal_mode().into());
        receipt.client_db_synchronous = Some(pragmas.synchronous());
        receipt.client_db_busy_timeout_ms = u64::try_from(pragmas.busy_timeout_ms()).ok();
        receipt.client_db_foreign_keys = Some(pragmas.foreign_keys());
    }
}

fn apply_db_engine_report_to_receipt(
    receipt: &mut ClientReceipt,
    engine_report: &ClientDbEngineReport,
) {
    receipt.db_engine = Some(db_engine_receipt(engine_report));
    let active_report = ClientDbEngine::inspect_client_dir(engine_report.client_dir());
    apply_db_report_to_receipt(receipt, &active_report);
}

fn apply_project_db_report_to_receipt(
    receipt: &mut ClientReceipt,
    project_root: &Path,
    fallback_db_report: &ClientDbReport,
) {
    if let Some(engine_report) = ClientDbEngine::resolve(project_root)
        .ok()
        .map(|engine| engine.inspect())
    {
        apply_db_engine_report_to_receipt(receipt, &engine_report);
    } else {
        apply_db_report_to_receipt(receipt, fallback_db_report);
    }
}

fn db_engine_receipt(engine_report: &ClientDbEngineReport) -> ClientDbEngineReceipt {
    ClientDbEngineReceipt {
        backend: ClientDbBackend::from(engine_report.backend()),
        layout_version: ClientStateLayoutVersion::from(engine_report.layout_version()),
        db_file_name: ClientDbFileName::from(engine_report.db_file_name()),
        schema_version: engine_report.schema_version(),
        durability: ClientDbEngineDurability::from(engine_report.durability()),
        features: ClientDbEngineFeaturesReceipt {
            async_io: engine_report.features().async_io(),
            concurrent_writes: engine_report.features().concurrent_writes(),
            fts: engine_report.features().fts(),
            vector: engine_report.features().vector(),
            overlay_search: engine_report.features().overlay_search(),
            sync: engine_report.features().sync(),
            encryption: engine_report.features().encryption(),
        },
        client_dir: ClientCachePath::from_path(engine_report.client_dir()),
        db_path: ClientCachePath::from_path(engine_report.db_path()),
        manifest_path: ClientCachePath::from_path(engine_report.manifest_path()),
        artifact_path: ClientCachePath::from_path(engine_report.artifact_path()),
        repo_id: ClientRepoId::from(engine_report.repo_id().to_string()),
        workspace_id: ClientWorkspaceId::from(engine_report.workspace_id().to_string()),
        scope_id: ClientScopeId::from(engine_report.scope_id().to_string()),
    }
}

fn print_cache_reason(cache_report: &CacheManifestReport) {
    if let Some(reason) = &cache_report.reason {
        println!(
            "|reason cacheManifest={} detail={}",
            cache_report.status.as_str(),
            compact_detail(reason)
        );
    }
}

fn display_optional_path(path: Option<&Path>) -> String {
    path.map_or_else(
        || "unavailable".to_string(),
        |path| path.display().to_string(),
    )
}

fn compact_detail(detail: &str) -> String {
    detail.split_whitespace().collect::<Vec<_>>().join("_")
}
