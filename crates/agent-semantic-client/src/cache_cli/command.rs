//! Runtime Server-backed cache CLI adapter.

use std::path::{Path, PathBuf};

use agent_semantic_client_core::LanguageId;
use agent_semantic_client_db::workspace_db_ipc::{
    RuntimeCacheControlRequest, RuntimeCacheInvalidationScope, WorkspaceDbSourceIndexLookupRequest,
    cache_control_via_runtime_server, read_source_index_via_runtime_server,
};
use serde_json::json;

struct SourceIndexLookupSpec {
    query: String,
    index_root: PathBuf,
    index_owner: Option<String>,
    limit: u32,
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
        query: query.ok_or_else(|| "--query is required".to_owned())?,
        index_root,
        index_owner,
        limit: limit.unwrap_or(8),
    })
}

fn runtime_cache_mutation_id(action: &str) -> Result<String, String> {
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|error| format!("system clock is before the Unix epoch: {error}"))?
        .as_nanos();
    Ok(format!(
        "cache-v1:{action}:{}:{timestamp}",
        std::process::id()
    ))
}

fn run_runtime_cache_control(
    request: RuntimeCacheControlRequest,
    receipt_json: bool,
) -> Result<(), String> {
    let receipt = cache_control_via_runtime_server(request)?;
    println!(
        "[asp-cache] status={:?} route=runtime-server action={} authority={} generation={} databaseOpensByClient={} writerQueueOwner={}",
        receipt.generation_state,
        receipt.action,
        receipt.authority,
        receipt.generation_digest.as_deref().unwrap_or("-"),
        receipt.database_opens_by_client,
        receipt.writer_queue_owner
    );
    if let Some(mutation_id) = &receipt.mutation_id {
        println!("|cache mutationId={mutation_id}");
    }
    if let Some(failure) = &receipt.failure {
        println!("|cache failure={failure}");
    }
    if receipt_json {
        let encoded = serde_json::to_string(&receipt)
            .map_err(|error| format!("failed to serialize cache-control receipt: {error}"))?;
        eprintln!("{encoded}");
    }
    Ok(())
}

fn run_source_index_lookup(
    project_root: &Path,
    facade_language_id: Option<&LanguageId>,
    args: &[String],
    receipt_json: bool,
) -> Result<(), String> {
    let spec = parse_source_index_lookup_args(project_root, args)?;
    if let Some(index_owner) = &spec.index_owner {
        return Err(format!(
            "--index-owner `{index_owner}` is not a v1 RuntimeServer lookup field; select the provider through the language facade"
        ));
    }
    let result = read_source_index_via_runtime_server(WorkspaceDbSourceIndexLookupRequest {
        project_root: project_root.to_path_buf(),
        indexed_project_root: spec.index_root.clone(),
        query: spec.query.clone(),
        language_id: facade_language_id.cloned(),
        limit: spec.limit,
    })?;
    if result.candidates.is_empty() {
        println!(
            "noOutput reason=source-index-{} query={} indexRoot={} route=runtime-server",
            result.state.as_str(),
            spec.query,
            spec.index_root.display()
        );
    } else {
        println!(
            "[asp-cache-source-index] status={} route=runtime-server indexRoot={} query={} candidates={} snapshotRoot={} providerDigest={} indexArtifactDigest={} rawSourceStored=false",
            result.state.as_str(),
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
                "|candidate path={} language={} provider={} kind={} lines={} queryKeys={} selectorSymbol={} selectorKind={}",
                candidate.path,
                candidate
                    .language_id
                    .as_ref()
                    .map_or("-", LanguageId::as_str),
                candidate
                    .provider_id
                    .as_ref()
                    .map_or("-", |provider| provider.as_str()),
                candidate.source_kind.as_str(),
                candidate
                    .line_count
                    .map(|count| count.to_string())
                    .unwrap_or_else(|| "-".to_owned()),
                candidate
                    .query_keys
                    .iter()
                    .map(|key| key.as_str())
                    .collect::<Vec<_>>()
                    .join(","),
                candidate
                    .selector_symbol
                    .as_ref()
                    .map_or("-", |symbol| symbol.as_str()),
                candidate
                    .selector_kind
                    .as_ref()
                    .map_or("-", |kind| kind.as_str())
            );
        }
    }
    if receipt_json {
        let receipt = json!({
            "schemaId": "agent.semantic-protocols.semantic-source-index.lookup-receipt",
            "schemaVersion": "1",
            "status": result.state.as_str(),
            "route": "runtime-server",
            "indexRoot": spec.index_root,
            "query": spec.query,
            "limit": spec.limit,
            "sourceSnapshot": result.source_snapshot,
            "indexArtifactDigest": result.index_artifact_digest,
            "rawSourceStored": false,
            "databaseOpensByClient": 0,
            "candidates": result.candidates,
        });
        eprintln!("{receipt}");
    }
    Ok(())
}

pub(crate) fn run_cache(
    project_root: &Path,
    facade_language_id: Option<&LanguageId>,
    forwarded_args: &[String],
    receipt_json: bool,
) -> Result<(), String> {
    let project_root_text = project_root.display().to_string();
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
        [subcommand] if subcommand == "status" => run_runtime_cache_control(
            RuntimeCacheControlRequest::Status {
                project_root: project_root_text,
            },
            receipt_json,
        ),
        [subcommand] if subcommand == "import" => run_runtime_cache_control(
            RuntimeCacheControlRequest::RebuildSourceIndex {
                project_root: project_root_text,
                mutation_id: runtime_cache_mutation_id("rebuild-source-index")?,
            },
            receipt_json,
        ),
        [subcommand, action] if subcommand == "source-index" && action == "refresh" => {
            run_runtime_cache_control(
                RuntimeCacheControlRequest::RefreshSourceIndex {
                    project_root: project_root_text,
                    expected_generation: None,
                },
                receipt_json,
            )
        }
        [subcommand, action, rest @ ..]
            if subcommand == "source-index" && action == "lookup" =>
        {
            run_source_index_lookup(project_root, facade_language_id, rest, receipt_json)
        }
        [subcommand, scope] if subcommand == "flush" && scope == "syntax-rows" => {
            run_runtime_cache_control(
                RuntimeCacheControlRequest::Invalidate {
                    project_root: project_root_text,
                    mutation_id: runtime_cache_mutation_id("invalidate-syntax-rows")?,
                    scope: RuntimeCacheInvalidationScope::SyntaxRows,
                },
                receipt_json,
            )
        }
        [subcommand] if subcommand == "invalidate" || subcommand == "flush" => {
            run_runtime_cache_control(
                RuntimeCacheControlRequest::Invalidate {
                    project_root: project_root_text,
                    mutation_id: runtime_cache_mutation_id("invalidate-workspace-generation")?,
                    scope: RuntimeCacheInvalidationScope::WorkspaceGeneration,
                },
                receipt_json,
            )
        }
        _ => Err(
            "usage: asp cache <status|gc [--grace-days <n>] [--apply]|import|source-index refresh|source-index lookup --query <term> [--index-root <path>] [--limit <n>]|invalidate|flush [syntax-rows]>; use asp <language> cache source-index lookup ... for language-scoped lookup"
                .to_owned(),
        ),
    }
}
