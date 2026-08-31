//! Runtime Server-backed cache CLI adapter.

use std::path::{Path, PathBuf};

use agent_semantic_client_core::LanguageId;
use agent_semantic_client_protocol::{
    AspClientSourceIndexLookupRequest, ClientFrame, ClientOutcome,
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

async fn run_source_index_lookup(
    project_root: &Path,
    facade_language_id: Option<&LanguageId>,
    args: &[String],
    receipt_json: bool,
) -> Result<(), String> {
    let language_id = facade_language_id.ok_or_else(|| {
        "source-index lookup requires a language facade: use `asp <language> cache source-index lookup ...`"
            .to_owned()
    })?;
    let spec = parse_source_index_lookup_args(project_root, args)?;
    if let Some(index_owner) = &spec.index_owner {
        return Err(format!(
            "--index-owner `{index_owner}` is not a v1 RuntimeServer lookup field; select the provider through the language facade"
        ));
    }
    let request = AspClientSourceIndexLookupRequest {
        schema_id: "agent.semantic-protocols.asp-client-source-index-lookup-request".to_owned(),
        schema_version: "1".to_owned(),
        query: spec.query.clone(),
        index_root: spec.index_root.display().to_string(),
        limit: spec.limit,
    };
    request.validate_schema_identity()?;
    let params = serde_json::to_value(request)
        .map_err(|error| format!("encode source-index lookup request: {error}"))?;
    let state_home = agent_semantic_runtime::state_core::resolve_state_home()?;
    let frame = crate::AspClient::new(state_home, project_root)
        .dispatch(language_id.as_str(), "source-index.lookup", params)
        .await?;
    let payload = match frame {
        ClientFrame::Response {
            outcome: ClientOutcome::Ready,
            result: Some(result),
            error: None,
            ..
        } => result,
        ClientFrame::Response { outcome, error, .. } => {
            return Err(format!(
                "source-index lookup failed: outcome={outcome:?} error={error:?}"
            ));
        }
        frame => {
            return Err(format!(
                "source-index lookup returned a non-response frame: {frame:?}"
            ));
        }
    };
    let result: agent_semantic_search_projection::ResidentSearchReadyResult =
        serde_json::from_value(payload)
            .map_err(|error| format!("decode source-index lookup response: {error}"))?;
    result.validate()?;
    if result.hits.is_empty() {
        println!(
            "noOutput reason=source-index-{} query={} indexRoot={} route=runtime-server",
            result.state.as_str(),
            spec.query,
            spec.index_root.display()
        );
    } else {
        println!(
            "[asp-cache-source-index] status={} route=runtime-server indexRoot={} query={} hits={} generationDigest={} rootDigest={} providerDigest={} indexArtifactDigest={} rawSourceStored=false",
            result.state.as_str(),
            spec.index_root.display(),
            spec.query,
            result.hits.len(),
            result.generation_digest,
            result.root_digest,
            result.provider_digest,
            result.index_artifact_digest,
        );
        for hit in &result.hits {
            println!(
                "|hit ownerPath={} language={} tier={} lines={} queryKeys={} selector={} score={}",
                hit.owner_path,
                hit.language_id.as_ref().map_or("-", String::as_str),
                hit.projection_tier.as_str(),
                hit.line_count,
                hit.query_keys
                    .iter()
                    .map(String::as_str)
                    .collect::<Vec<_>>()
                    .join(","),
                hit.selector.as_deref().unwrap_or("-"),
                hit.score
                    .map(|score| score.to_string())
                    .unwrap_or_else(|| "-".to_owned()),
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
            "generationDigest": result.generation_digest,
            "rootDigest": result.root_digest,
            "providerDigest": result.provider_digest,
            "indexArtifactDigest": result.index_artifact_digest,
            "rawSourceStored": false,
            "databaseOpensByClient": 0,
            "workCounters": result.work_counters,
            "hits": result.hits,
        });
        eprintln!("{receipt}");
    }
    Ok(())
}

pub(crate) async fn run_cache(
    project_root: &Path,
    facade_language_id: Option<&LanguageId>,
    forwarded_args: &[String],
    receipt_json: bool,
) -> Result<(), String> {
    match forwarded_args {
        [subcommand, rest @ ..] if subcommand == "gc" => {
            super::project_registry_gc_command::run_project_registry_gc(
                project_root,
                rest,
                receipt_json,
            )
        }
        [subcommand, action, rest @ ..]
            if subcommand == "source-index" && action == "lookup" =>
        {
            run_source_index_lookup(project_root, facade_language_id, rest, receipt_json).await
        }
        _ => Err(
            "usage: asp cache gc [--grace-days <n>] [--apply] or asp <language> cache source-index lookup --query <term> [--index-root <workspace>] [--limit <n>]; use `asp clean --day[=<days>]` for State Home retention"
                .to_owned(),
        ),
    }
}
