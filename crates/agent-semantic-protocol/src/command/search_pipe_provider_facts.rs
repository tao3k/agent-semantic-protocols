//! Provider-owned semantic fact enrichment for ASP search pipe graph requests.

use std::path::Path;
use std::path::PathBuf;
use std::time::Duration;

use agent_semantic_hook::{ActivatedProvider, RuntimeProfiles};
use agent_semantic_provider_transport::ProviderProcessLimits;
use serde_json::Value;

use super::provider_process::{
    provider_invocation_with_profile, run_provider_command_with_stdin_limits,
};
use super::search_config::AspConfig;
use super::search_pipe_model::Candidate;

const PROVIDER_GRAPH_FACT_CANDIDATE_LIMIT: usize = 12;
const PROVIDER_GRAPH_FACT_TIMEOUT: Duration = Duration::from_millis(1_500);
const PROVIDER_GRAPH_FACT_OUTPUT_LIMIT_BYTES: usize = 256 * 1024;

#[derive(Debug, Default)]
pub(super) struct ProviderGraphFacts {
    pub(super) nodes: Vec<Value>,
    pub(super) edges: Vec<Value>,
    pub(super) input_candidates: usize,
    pub(super) fact_candidates: usize,
    pub(super) truncated_candidates: usize,
}

pub(super) struct ProviderGraphFactsContext<'a> {
    pub(super) provider: &'a ActivatedProvider,
    pub(super) profiles: &'a RuntimeProfiles,
    pub(super) provider_bin_root: &'a Path,
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
    if !context.provider.search_capabilities.semantic_facts {
        return Ok(ProviderGraphFacts::default());
    }
    if candidates.is_empty() {
        return Ok(ProviderGraphFacts::default());
    }
    let fact_candidates = provider_fact_candidates(candidates);
    let input_candidates = candidates.len();
    let truncated_candidates = input_candidates.saturating_sub(fact_candidates.len());
    let semantic_fact_limits = ProviderProcessLimits {
        timeout: Some(PROVIDER_GRAPH_FACT_TIMEOUT),
        max_stdout_bytes: Some(PROVIDER_GRAPH_FACT_OUTPUT_LIMIT_BYTES),
        max_stderr_bytes: Some(PROVIDER_GRAPH_FACT_OUTPUT_LIMIT_BYTES),
    };
    let args = vec![
        "search".to_string(),
        "semantic-facts".to_string(),
        query.to_string(),
        "--json".to_string(),
    ];
    let invocation = provider_invocation_with_profile(
        context.profiles,
        context.provider,
        &args,
        context.provider_bin_root,
        config,
    )?;
    let output = match run_provider_command_with_stdin_limits(
        language_id,
        context.provider,
        &invocation,
        project_root,
        context.cache_home,
        candidate_stdin(project_root, &fact_candidates),
        semantic_fact_limits,
    ) {
        Ok(output) => output,
        Err(_) => {
            return Ok(ProviderGraphFacts {
                input_candidates,
                fact_candidates: fact_candidates.len(),
                truncated_candidates,
                ..ProviderGraphFacts::default()
            });
        }
    };
    if !output.status.success() {
        return Ok(ProviderGraphFacts {
            input_candidates,
            fact_candidates: fact_candidates.len(),
            truncated_candidates,
            ..ProviderGraphFacts::default()
        });
    }
    let mut facts = provider_graph_facts_from_stdout(output.stdout.as_ref())?;
    facts.input_candidates = input_candidates;
    facts.fact_candidates = fact_candidates.len();
    facts.truncated_candidates = truncated_candidates;
    Ok(facts)
}

fn provider_fact_candidates(candidates: &[Candidate]) -> Vec<Candidate> {
    let mut seen = std::collections::BTreeSet::new();
    candidates
        .iter()
        .filter(|candidate| seen.insert(candidate.path.clone()))
        .take(PROVIDER_GRAPH_FACT_CANDIDATE_LIMIT)
        .cloned()
        .collect()
}

fn provider_graph_facts_from_stdout(stdout: &[u8]) -> Result<ProviderGraphFacts, String> {
    let Some(value) = provider_graph_facts_json(stdout) else {
        return Ok(ProviderGraphFacts::default());
    };
    Ok(provider_graph_facts_from_value(value))
}

fn provider_graph_facts_json(stdout: &[u8]) -> Option<Value> {
    if let Ok(value) = serde_json::from_slice::<Value>(stdout) {
        return Some(value);
    }
    let start = stdout.iter().position(|byte| *byte == b'{')?;
    let end = stdout.iter().rposition(|byte| *byte == b'}')?;
    if end <= start {
        return None;
    }
    serde_json::from_slice::<Value>(&stdout[start..=end]).ok()
}

fn provider_graph_facts_from_value(value: Value) -> ProviderGraphFacts {
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
    ProviderGraphFacts {
        nodes,
        edges,
        ..ProviderGraphFacts::default()
    }
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

pub(super) fn query_requests_semantic_facts(query: &str) -> bool {
    if query
        .split_whitespace()
        .any(term_looks_like_path_or_selector)
    {
        return false;
    }
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
                | "concurrency"
                | "concurrent"
                | "cancellation"
                | "interruption"
                | "resource"
                | "resources"
                | "leak"
                | "leaks"
                | "queue"
                | "queues"
                | "stream"
                | "streams"
                | "fiber"
                | "fibers"
        )
    })
}

fn term_looks_like_path_or_selector(term: &str) -> bool {
    term.contains('.') || term.contains('/') || term.contains('\\')
}

fn query_terms(query: &str) -> Vec<String> {
    query
        .split(|character: char| !(character == '_' || character.is_ascii_alphanumeric()))
        .map(str::trim)
        .filter(|term| !term.is_empty())
        .map(str::to_ascii_lowercase)
        .collect()
}
