//! Provider-owned semantic fact enrichment for ASP search pipe graph requests.

use std::path::Path;
use std::path::PathBuf;

use agent_semantic_hook::{ActivatedProvider, RuntimeProfiles};
use serde_json::Value;

use super::provider_process::{provider_invocation_with_profile, run_provider_command_with_stdin};
use super::search_config::AspConfig;
use super::search_pipe_render::Candidate;

#[derive(Debug, Default)]
pub(super) struct ProviderGraphFacts {
    pub(super) nodes: Vec<Value>,
    pub(super) edges: Vec<Value>,
}

pub(super) struct ProviderGraphFactsContext<'a> {
    pub(super) provider: &'a ActivatedProvider,
    pub(super) profiles: &'a RuntimeProfiles,
    pub(super) cache_home: &'a Path,
}

pub(super) fn collect_provider_graph_facts(
    language_id: &str,
    project_root: &Path,
    query: Option<&str>,
    candidates: &[Candidate],
    config: &AspConfig,
    context: Option<&ProviderGraphFactsContext<'_>>,
) -> Result<ProviderGraphFacts, String> {
    let Some(query) = query.filter(|query| query_requests_semantic_facts(query)) else {
        return Ok(ProviderGraphFacts::default());
    };
    let Some(context) = context else {
        return Ok(ProviderGraphFacts::default());
    };
    if candidates.is_empty() {
        return Ok(ProviderGraphFacts::default());
    }
    let args = vec![
        "search".to_string(),
        "semantic-facts".to_string(),
        query.to_string(),
        "--json".to_string(),
    ];
    let invocation =
        provider_invocation_with_profile(context.profiles, context.provider, &args, config)?;
    let output = run_provider_command_with_stdin(
        language_id,
        context.provider,
        &invocation,
        project_root,
        context.cache_home,
        candidate_stdin(project_root, candidates),
    )?;
    if !output.status.success() {
        return Ok(ProviderGraphFacts::default());
    }
    provider_graph_facts_from_stdout(output.stdout.as_ref())
}

fn provider_graph_facts_from_stdout(stdout: &[u8]) -> Result<ProviderGraphFacts, String> {
    let Ok(value) = serde_json::from_slice::<Value>(stdout) else {
        return Ok(ProviderGraphFacts::default());
    };
    let nodes = value
        .get("nodes")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let edges = value
        .get("edges")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    Ok(ProviderGraphFacts { nodes, edges })
}

fn candidate_stdin(project_root: &Path, candidates: &[Candidate]) -> Vec<u8> {
    let mut stdin = String::new();
    for candidate in candidates {
        stdin.push_str(&candidate_path_for_provider(project_root, &candidate.path));
        stdin.push(':');
        stdin.push_str(&candidate.line.to_string());
        stdin.push_str(":1:");
        stdin.push_str(&candidate.text.replace('\n', " "));
        stdin.push('\n');
    }
    stdin.into_bytes()
}

fn candidate_path_for_provider(project_root: &Path, path: &str) -> String {
    let path = Path::new(path);
    if path.is_absolute() {
        return path
            .strip_prefix(project_root)
            .unwrap_or(path)
            .to_string_lossy()
            .to_string();
    }
    if project_root.join(path).exists() {
        return path.to_string_lossy().to_string();
    }
    let Ok(cwd) = std::env::current_dir() else {
        return path.to_string_lossy().to_string();
    };
    let cwd_relative = cwd.join(path);
    if cwd_relative.exists()
        && let Ok(provider_relative) = cwd_relative.strip_prefix(project_root)
    {
        return provider_relative.to_string_lossy().to_string();
    }
    PathBuf::from(path).to_string_lossy().to_string()
}

fn query_requests_semantic_facts(query: &str) -> bool {
    query_terms(query).into_iter().any(|term| {
        matches!(
            term.as_str(),
            "field"
                | "fields"
                | "type"
                | "types"
                | "scalar"
                | "scalars"
                | "collection"
                | "collections"
                | "list"
                | "lists"
                | "map"
                | "maps"
                | "set"
                | "sets"
                | "vec"
                | "vecdeque"
                | "hashmap"
                | "hashset"
                | "btreemap"
                | "btreeset"
        )
    })
}

fn query_terms(query: &str) -> Vec<String> {
    query
        .split(|character: char| !(character == '_' || character.is_ascii_alphanumeric()))
        .map(str::trim)
        .filter(|term| !term.is_empty())
        .map(str::to_ascii_lowercase)
        .collect()
}
