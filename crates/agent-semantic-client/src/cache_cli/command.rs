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
    ClientCacheManifest, ClientCachePath, ClientDbStatus, ClientMethod, ClientReceipt, LanguageId,
    ProjectContext, ProviderId, ProviderRegistrySnapshot, StateLayout,
};
use agent_semantic_client_db::{ClientDb, ClientDbEngine, ClientDbReport};
use agent_semantic_runtime::{RuntimeSourceSpec, ensure_runtime_source_checkout_in_client_cache};
use serde_json::json;

use super::structural_index_import::import_structural_index_artifacts;
use crate::source_index::{
    lookup_source_index_in_cache, refresh_runtime_source_index, refresh_source_index,
};

pub(crate) fn run_cache(
    project_root: &Path,
    facade_language_id: Option<&LanguageId>,
    forwarded_args: &[String],
    receipt_json: bool,
) -> Result<(), String> {
    match forwarded_args {
        [subcommand, action, rest @ ..]
            if subcommand == "runtime-source" && action == "acquire" =>
        {
            let spec = parse_runtime_source_acquire_args(rest)?;
            let state_layout = cache_state_layout(project_root)?;
            let checkout =
                ensure_runtime_source_checkout_in_client_cache(state_layout.client_cache_dir(), &spec)?;
            let source_index_report = refresh_runtime_source_index(
                project_root,
                &checkout.checkout_dir,
                &LanguageId::from(checkout.language_id.as_str()),
                &ProviderId::from(checkout.index_owner.as_str()),
            )?;
            println!(
                "[asp-cache-runtime-source] status=ready language={} stateNamespace={} checkout={} statePathPolicy=asp-state-managed indexOwner={} sourceIndex=refreshed indexGeneration={} reused={} files={} owners={} selectors={} rawSourceStored=false",
                checkout.language_id,
                checkout.state_namespace,
                checkout.checkout,
                checkout.index_owner,
                source_index_report.generation_id,
                source_index_report.reused_generation,
                source_index_report.file_count,
                source_index_report.owner_count,
                source_index_report.selector_count
            );
            println!(
                "|sourceRef manager=git repository={} checkout={}",
                checkout.repository, checkout.checkout
            );
            println!(
                "|acquisition owner=asp operation=clone-or-fetch-checkout-index checkoutDir={} indexOwner={}",
                checkout.checkout_dir.display(),
                checkout.index_owner
            );
            println!(
                "|sourceIndex db={} generation={} reused={} projectRoot={} rawSourceStored=false",
                source_index_report.db_path.display(),
                source_index_report.generation_id,
                source_index_report.reused_generation,
                checkout.checkout_dir.display()
            );
            println!("next=asp cache import");
            if receipt_json {
                let receipt = json!({
                    "schemaId": "agent.semantic-protocols.semantic-runtime-source-acquisition.receipt",
                    "schemaVersion": "1",
                    "status": "ready",
                    "languageId": checkout.language_id,
                    "repository": checkout.repository,
                    "checkout": checkout.checkout,
                    "stateNamespace": checkout.state_namespace,
                    "statePathPolicy": "asp-state-managed",
                    "indexOwner": checkout.index_owner,
                    "checkoutDir": checkout.checkout_dir.display().to_string(),
                    "sourceIndex": {
                        "status": "refreshed",
                        "dbPath": source_index_report.db_path.display().to_string(),
                        "generationId": source_index_report.generation_id.to_string(),
                        "reused": source_index_report.reused_generation,
                        "fileCount": source_index_report.file_count,
                        "ownerCount": source_index_report.owner_count,
                        "selectorCount": source_index_report.selector_count,
                        "rawSourceStored": false,
                        "projectRoot": checkout.checkout_dir.display().to_string()
                    },
                    "next": "asp cache import"
                });
                eprintln!("{receipt}");
            }
            Ok(())
        }
        [subcommand] if subcommand == "status" => {
            let snapshot = ProviderRegistrySnapshot::load(project_root);
            let provenance = snapshot
                .as_ref()
                .map_or_else(|_| Vec::new(), ProviderRegistrySnapshot::native_provenance);
            let cache_report = ClientCacheManifest::inspect_project(project_root);
            let db_report = cache_report
                .cache_root
                .as_ref()
                .map(|cache_root| {
                    ClientDb::inspect(ClientDbEngine::sqlite_path_for_client_dir(cache_root))
                });
            let mut receipt = ClientReceipt::cache_status(provenance, &cache_report);
            if let Some(db_report) = &db_report {
                apply_db_report_to_receipt(&mut receipt, db_report);
            }
            receipt.sqlite_read_count = Some(u64::from(db_report.is_some()));
            receipt.sqlite_write_count = Some(0);
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
                cache_status_line(&cache_report, db_report.as_ref()),
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
            print_db_status(db_report.as_ref());
            print_cache_reason(&cache_report);
            println!("|reason phase=phase-1-client-db-sql arrow=false providerCommands=0");
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
            let db_path = ClientDbEngine::sqlite_path_for_client_dir(cache_root);
            let mut db = ClientDb::open_or_create(db_path.clone())?;
            db.import_manifest(&manifest)?;
            let structural_index_imported_count =
                import_structural_index_artifacts(cache_root, &mut db, &manifest)?;
            let db_report = db
                .inspect_open()
                .unwrap_or_else(|_| ClientDb::inspect(db_path));
            let mut receipt =
                ClientReceipt::cache_report(ClientMethod::CacheImport, provenance, &cache_report);
            apply_db_report_to_receipt(&mut receipt, &db_report);
            receipt.sqlite_read_count = Some(1);
            receipt.sqlite_write_count = Some(1 + structural_index_imported_count);
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
            println!(
                "|reason phase=phase-1-client-db-sql action=import arrow=false providerCommands=0"
            );
            if receipt_json {
                let receipt = serde_json::to_string(&receipt)
                    .map_err(|error| format!("failed to serialize receipt: {error}"))?;
                eprintln!("{receipt}");
            }
            Ok(())
        }
        [subcommand, action] if subcommand == "source-index" && action == "refresh" => {
            let report = refresh_source_index(project_root)?;
            println!(
                "[asp-cache-source-index] status=refreshed route=local-cache db={} generation={} reused={} files={} owners={} selectors={} rawSourceStored=false indexOwner=rust-sql",
                report.db_path.display(),
                report.generation_id,
                report.reused_generation,
                report.file_count,
                report.owner_count,
                report.selector_count
            );
            println!(
                "|reason phase=source-index-rust-sql action=refresh providerCommands=0"
            );
            if receipt_json {
                let receipt = json!({
                    "schemaId": "agent.semantic-protocols.semantic-source-index.refresh-receipt",
                    "schemaVersion": "1",
                    "status": "refreshed",
                    "route": "local-cache",
                    "dbPath": report.db_path,
                    "generationId": report.generation_id,
                    "reused": report.reused_generation,
                    "fileCount": report.file_count,
                    "ownerCount": report.owner_count,
                    "selectorCount": report.selector_count,
                    "rawSourceStored": false,
                    "indexOwner": "rust-sql"
                });
                eprintln!("{receipt}");
            }
            Ok(())
        }
        [subcommand, action, rest @ ..] if subcommand == "source-index" && action == "lookup" => {
            let spec = parse_source_index_lookup_args(project_root, rest)?;
            let result = lookup_source_index_in_cache(
                project_root,
                &spec.index_root,
                facade_language_id,
                &spec.query,
                spec.limit,
            )?;
            if result.candidates.is_empty() {
                println!(
                    "noOutput reason=source-index-{} query={} indexRoot={}",
                    result.state.as_str(),
                    spec.query,
                    spec.index_root.display()
                );
            } else {
                println!(
                    "[asp-cache-source-index] status={} route=local-cache db={} indexRoot={} query={} candidates={} rawSourceStored=false",
                    result.state.as_str(),
                    result.db_path.display(),
                    spec.index_root.display(),
                    spec.query,
                    result.candidates.len()
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
                        candidate.query_keys.join(",")
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
                    "query": spec.query,
                    "limit": spec.limit,
                    "rawSourceStored": false,
                    "candidates": result.candidates.iter().map(|candidate| {
                        json!({
                            "path": candidate.path,
                            "languageId": candidate.language_id.as_ref().map(LanguageId::as_str),
                            "providerId": candidate.provider_id.as_ref().map(ProviderId::as_str),
                            "sourceKind": candidate.source_kind.as_str(),
                            "lineCount": candidate.line_count,
                            "queryKeys": candidate.query_keys
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
            let db_path = ClientDbEngine::sqlite_path_for_client_dir(cache_root);
            let flushed_syntax_rows = ClientDb::flush_syntax_query_rows(&db_path)?;
            let updated_cache_report = ClientCacheManifest::inspect_project(project_root);
            let db_report = ClientDb::inspect(db_path);
            let mut receipt = ClientReceipt::cache_report(
                ClientMethod::CacheFlush,
                provenance,
                &updated_cache_report,
            );
            receipt.cache_status = agent_semantic_client_core::CacheStatus::Invalidated;
            apply_db_report_to_receipt(&mut receipt, &db_report);
            receipt.sqlite_read_count = Some(1);
            receipt.sqlite_write_count = Some(1);
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
                "|reason phase=phase-1-client-db-sql action=flush-syntax-rows manifestArtifactsDeleted=false providerCommands=0"
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
            let db_path = ClientDbEngine::sqlite_path_for_client_dir(cache_root);
            let db_invalidated_generation_count =
                ClientDb::invalidate_generations_for_project(&db_path, project_root)?;
            let manifest_invalidated_generation_count =
                clear_manifest_generations(&cache_report, &state_layout, project_root)?;
            let invalidated_generation_count =
                db_invalidated_generation_count.max(manifest_invalidated_generation_count);
            let updated_cache_report = ClientCacheManifest::inspect_project(project_root);
            let db_report = ClientDb::inspect(db_path);
            let receipt_method = if is_flush {
                ClientMethod::CacheFlush
            } else {
                ClientMethod::CacheInvalidate
            };
            let mut receipt =
                ClientReceipt::cache_report(receipt_method, provenance, &updated_cache_report);
            receipt.cache_status = agent_semantic_client_core::CacheStatus::Invalidated;
            apply_db_report_to_receipt(&mut receipt, &db_report);
            receipt.sqlite_read_count = Some(1);
            receipt.sqlite_write_count = Some(1);
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
                "|reason phase=phase-1-client-db-sql action={} manifestArtifactsDeleted=false providerCommands=0",
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
            "usage: asp cache <status|import|source-index refresh|source-index lookup --query <term> [--index-root <path>] [--limit <n>]|invalidate|flush [syntax-rows]|runtime-source acquire --language-id <id> --repository <url> --checkout <ref> --state-namespace <namespace> --index-owner <owner>> [--root <path>]; use asp <language> cache source-index lookup ... for language-scoped lookup"
                .to_string(),
        ),
    }
}

struct SourceIndexLookupSpec {
    query: String,
    index_root: PathBuf,
    limit: u32,
}

fn parse_source_index_lookup_args(
    project_root: &Path,
    args: &[String],
) -> Result<SourceIndexLookupSpec, String> {
    let mut query = None;
    let mut index_root = None;
    let mut limit = None;
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--query" => query = Some(next_flag_value("--query", &mut iter)?),
            "--index-root" => index_root = Some(next_flag_value("--index-root", &mut iter)?),
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
        limit: limit.unwrap_or(8),
    })
}

fn parse_runtime_source_acquire_args(args: &[String]) -> Result<RuntimeSourceSpec, String> {
    let mut language_id = None;
    let mut repository = None;
    let mut checkout = None;
    let mut state_namespace = None;
    let mut index_owner = None;
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--language-id" => language_id = Some(next_flag_value("--language-id", &mut iter)?),
            "--repository" => repository = Some(next_flag_value("--repository", &mut iter)?),
            "--checkout" => checkout = Some(next_flag_value("--checkout", &mut iter)?),
            "--state-namespace" => {
                state_namespace = Some(next_flag_value("--state-namespace", &mut iter)?);
            }
            "--index-owner" => index_owner = Some(next_flag_value("--index-owner", &mut iter)?),
            other => {
                return Err(format!(
                    "unexpected runtime-source acquire argument: {other}"
                ));
            }
        }
    }
    Ok(RuntimeSourceSpec {
        language_id: language_id.ok_or_else(|| "--language-id is required".to_string())?,
        repository: repository.ok_or_else(|| "--repository is required".to_string())?,
        checkout: checkout.ok_or_else(|| "--checkout is required".to_string())?,
        state_namespace: state_namespace
            .ok_or_else(|| "--state-namespace is required".to_string())?,
        index_owner: index_owner.ok_or_else(|| "--index-owner is required".to_string())?,
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
        CacheManifestStatus::Missing => "missing",
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

fn print_db_status(db_report: Option<&ClientDbReport>) {
    if let Some(db_report) = db_report {
        let runtime_pragmas = db_report
            .runtime_pragmas
            .as_ref()
            .map(|pragmas| {
                format!(
                    " journalMode={} synchronous={} busyTimeoutMs={} foreignKeys={}",
                    pragmas.journal_mode.as_str(),
                    pragmas.synchronous,
                    pragmas.busy_timeout_ms,
                    pragmas.foreign_keys
                )
            })
            .unwrap_or_default();
        println!(
            "|db path={} status={} generations={} syntaxRows={}/{}/{} structuralIndex={}/{}/{}/{} sourceIndex={}/{}/{} artifactEvents={} rawSourceStored={}{}",
            db_report.db_path.display(),
            db_report.status.as_str(),
            db_report.generation_count,
            db_report.syntax_row_generation_count,
            db_report.syntax_row_match_count,
            db_report.syntax_row_capture_count,
            db_report.structural_index_generation_count,
            db_report.structural_index_owner_count,
            db_report.structural_index_symbol_count,
            db_report.structural_index_dependency_usage_count,
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
            "|db path=unavailable status=unavailable generations=0 syntaxRows=0/0/0 structuralIndex=0/0/0/0 sourceIndex=0/0/0 artifactEvents=0 rawSourceStored=false journalMode=unknown synchronous=unknown busyTimeoutMs=unknown foreignKeys=false"
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
    receipt.client_db_structural_index_generation_count =
        Some(db_report.structural_index_generation_count);
    receipt.client_db_structural_index_owner_count = Some(db_report.structural_index_owner_count);
    receipt.client_db_structural_index_symbol_count = Some(db_report.structural_index_symbol_count);
    receipt.client_db_structural_index_dependency_usage_count =
        Some(db_report.structural_index_dependency_usage_count);
    receipt.client_db_source_index_generation_count = Some(db_report.source_index_generation_count);
    receipt.client_db_source_index_owner_count = Some(db_report.source_index_owner_count);
    receipt.client_db_source_index_selector_count = Some(db_report.source_index_selector_count);
    receipt.client_db_artifact_event_count = Some(db_report.artifact_event_count);
    receipt.client_db_raw_source_stored = Some(db_report.raw_source_stored);
    if let Some(pragmas) = &db_report.runtime_pragmas {
        receipt.client_db_journal_mode = Some(pragmas.journal_mode.as_str().into());
        receipt.client_db_synchronous = Some(pragmas.synchronous);
        receipt.client_db_busy_timeout_ms = u64::try_from(pragmas.busy_timeout_ms).ok();
        receipt.client_db_foreign_keys = Some(pragmas.foreign_keys);
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
