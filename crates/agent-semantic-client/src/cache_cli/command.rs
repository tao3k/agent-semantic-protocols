//! `asp cache` maintenance command implementation.

use std::{fs, path::Path};

use agent_semantic_client_core::{
    CacheManifestReport, CacheManifestStatus, ClientCacheManifest, ClientCachePath, ClientDbStatus,
    ClientMethod, ClientReceipt, ProviderRegistrySnapshot,
};
use agent_semantic_client_db::{ClientDb, ClientDbReport};

pub(crate) fn run_cache(
    project_root: &Path,
    forwarded_args: &[String],
    receipt_json: bool,
) -> Result<(), String> {
    match forwarded_args {
        [subcommand] if subcommand == "status" => {
            let snapshot = ProviderRegistrySnapshot::load(project_root);
            let provenance = snapshot
                .as_ref()
                .map_or_else(|_| Vec::new(), ProviderRegistrySnapshot::native_provenance);
            let cache_report = ClientCacheManifest::inspect_project(project_root);
            let db_report = cache_report
                .cache_root
                .as_ref()
                .map(|cache_root| ClientDb::inspect(ClientDb::default_path(cache_root)));
            let mut receipt = ClientReceipt::cache_status(provenance, &cache_report);
            if let Some(db_report) = &db_report {
                receipt.client_db_path = Some(ClientCachePath::from_path(&db_report.db_path));
                receipt.client_db_status = Some(db_report.status.clone());
                receipt.client_db_generation_count = Some(db_report.generation_count);
                receipt.client_db_raw_source_stored = Some(db_report.raw_source_stored);
            }
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
                println!("|cmd install=asp hook install --client codex .");
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
            let snapshot = ProviderRegistrySnapshot::load(project_root);
            let provenance = snapshot
                .as_ref()
                .map_or_else(|_| Vec::new(), ProviderRegistrySnapshot::native_provenance);
            let cache_report = ClientCacheManifest::inspect_project(project_root);
            let manifest = ClientCacheManifest::load_from_path(
                cache_report
                    .manifest_path
                    .as_ref()
                    .ok_or_else(|| "cache manifest path unavailable".to_string())?,
            )?;
            let cache_root = cache_report
                .cache_root
                .as_ref()
                .ok_or_else(|| "cache root unavailable".to_string())?;
            let db_path = ClientDb::default_path(cache_root);
            let mut db = ClientDb::open_or_create(db_path.clone())?;
            db.import_manifest(&manifest)?;
            let db_report = ClientDb::inspect(db_path);
            let mut receipt =
                ClientReceipt::cache_report(ClientMethod::CacheImport, provenance, &cache_report);
            receipt.client_db_path = Some(ClientCachePath::from_path(&db_report.db_path));
            receipt.client_db_status = Some(db_report.status.clone());
            receipt.client_db_generation_count = Some(db_report.generation_count);
            receipt.client_db_raw_source_stored = Some(db_report.raw_source_stored);
            println!(
                "[asp-cache] status=imported route=local-cache cacheRoot={} manifest={} generations={} rawSourceStored={}",
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
        [subcommand] if subcommand == "invalidate" || subcommand == "flush" => {
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
            let cache_root = cache_report
                .cache_root
                .as_ref()
                .ok_or_else(|| "cache root unavailable".to_string())?;
            let db_path = ClientDb::default_path(cache_root);
            let db_invalidated_generation_count = ClientDb::invalidate_generations(&db_path)?;
            let manifest_invalidated_generation_count = clear_manifest_generations(&cache_report)?;
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
            receipt.client_db_path = Some(ClientCachePath::from_path(&db_report.db_path));
            receipt.client_db_status = Some(db_report.status.clone());
            receipt.client_db_generation_count = Some(db_report.generation_count);
            receipt.client_db_raw_source_stored = Some(db_report.raw_source_stored);
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
        _ => Err("usage: asp cache <status|import|invalidate|flush> [--root <path>]".to_string()),
    }
}

fn clear_manifest_generations(cache_report: &CacheManifestReport) -> Result<u32, String> {
    if cache_report.status != CacheManifestStatus::Present {
        return Ok(0);
    }
    let manifest_path = cache_report
        .manifest_path
        .as_ref()
        .ok_or_else(|| "cache manifest path unavailable".to_string())?;
    let mut manifest = ClientCacheManifest::load_from_path(manifest_path)?;
    let invalidated = manifest.generations.len().min(u32::MAX as usize) as u32;
    if invalidated == 0 {
        return Ok(0);
    }
    manifest.generations.clear();
    write_cache_manifest(manifest_path, &manifest)?;
    Ok(invalidated)
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
        println!(
            "|db path={} status={} generations={} rawSourceStored={}",
            db_report.db_path.display(),
            db_report.status.as_str(),
            db_report.generation_count,
            db_report.raw_source_stored
        );
        if let Some(reason) = &db_report.reason {
            println!(
                "|reason clientDb={} detail={}",
                db_report.status.as_str(),
                compact_detail(reason)
            );
        }
    } else {
        println!("|db path=unavailable status=unavailable generations=0 rawSourceStored=false");
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
