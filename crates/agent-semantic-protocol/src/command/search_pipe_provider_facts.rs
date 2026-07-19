//! Provider-owned semantic fact enrichment for ASP search pipe graph requests.

use std::env;
use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::time::Duration;

use agent_semantic_hook::{ActivatedProvider, RuntimeProfiles};
use agent_semantic_provider_transport::ProviderProcessLimits;
use serde_json::Value;
use sha2::{Digest, Sha256};

use super::provider_process::{
    provider_invocation_with_profile, run_provider_command_with_stdin_limits,
};
use super::search_config::AspConfig;
use super::search_pipe_model::Candidate;

const PROVIDER_GRAPH_FACT_CANDIDATE_LIMIT: usize = 12;
const PROVIDER_GRAPH_FACT_TIMEOUT_MS: u64 = 100;
const PROVIDER_GRAPH_FACT_OUTPUT_LIMIT_BYTES: usize = 256 * 1024;
const PROVIDER_WORKSPACE_SCOPE_COLD_TIMEOUT_MS: u64 = 500;

#[derive(Debug, Default)]
pub(super) struct ProviderGraphFacts {
    pub(super) nodes: Vec<Value>,
    pub(super) edges: Vec<Value>,
    pub(super) candidate_annotations: Vec<Value>,
    pub(super) input_candidates: usize,
    pub(super) fact_candidates: usize,
    pub(super) truncated_candidates: usize,
    pub(super) descriptor_id: Option<String>,
    pub(super) descriptor_version: Option<String>,
    pub(super) matched_axes: Vec<String>,
    pub(super) matched_terms: Vec<String>,
}

fn provider_graph_facts_intent_receipt(
    intent: &agent_semantic_search::SearchPipeSemanticFactsIntentDecision,
) -> ProviderGraphFacts {
    ProviderGraphFacts {
        descriptor_id: Some(intent.descriptor_id.clone()),
        descriptor_version: Some(intent.descriptor_version.clone()),
        matched_axes: intent.matched_axes.clone(),
        matched_terms: intent.matched_terms.clone(),
        ..ProviderGraphFacts::default()
    }
}

pub(super) struct ProviderGraphFactsContext<'a> {
    pub(super) provider: &'a ActivatedProvider,
    pub(super) profiles: &'a RuntimeProfiles,
    pub(super) cache_home: &'a Path,
}

pub(super) fn collect_provider_workspace_scope(
    language_id: &str,
    project_root: &Path,
    config: &AspConfig,
    context: Option<&ProviderGraphFactsContext<'_>>,
) -> Result<Option<agent_semantic_search::SemanticWorkspaceScope>, String> {
    let Some(context) = context else {
        return Ok(None);
    };
    if !context.provider.search_capabilities.workspace_scope {
        return Ok(None);
    }
    if let Some(scope) = load_cached_provider_workspace_scope(project_root, context) {
        return Ok(Some(scope));
    }
    let args = vec![
        "search".to_string(),
        "workspace-scope".to_string(),
        "--json".to_string(),
    ];
    let invocation = provider_invocation_with_profile(
        context.profiles,
        context.provider,
        &args,
        project_root,
        config,
    )?;
    let limits = ProviderProcessLimits {
        timeout: Some(Duration::from_millis(
            PROVIDER_WORKSPACE_SCOPE_COLD_TIMEOUT_MS,
        )),
        max_stdout_bytes: Some(PROVIDER_GRAPH_FACT_OUTPUT_LIMIT_BYTES),
        max_stderr_bytes: Some(PROVIDER_GRAPH_FACT_OUTPUT_LIMIT_BYTES),
        memory_limit_bytes: Some(1024 * 1024 * 1024),
    };
    let output = run_provider_command_with_stdin_limits(
        language_id,
        context.provider,
        &invocation,
        project_root,
        context.cache_home,
        Vec::new(),
        limits,
    )
    .map_err(|error| format!("provider-unavailable: workspace-scope failed: {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "provider-unavailable: workspace-scope exited with status {:?}: {}",
            output.status.code(),
            String::from_utf8_lossy(output.stderr.as_ref()).trim()
        ));
    }
    let packet: Value = serde_json::from_slice(output.stdout.as_ref())
        .map_err(|error| format!("provider workspace-scope returned invalid JSON: {error}"))?;
    let scope = agent_semantic_search::SemanticWorkspaceScope::from_packet(&packet)?;
    let expected_root = fs::canonicalize(project_root).map_err(|error| {
        format!(
            "provider workspace-scope could not canonicalize project root {}: {error}",
            project_root.display()
        )
    })?;
    if !provider_workspace_scope_matches_context(&scope, &expected_root, context) {
        return Err(format!(
            "provider workspace-scope identity mismatch: expected providerId={} languageId={} discoveryRoot={}",
            context.provider.provider_id,
            context.provider.language_id,
            expected_root.display()
        ));
    }
    persist_provider_workspace_scope(project_root, context, &packet);
    Ok(Some(scope))
}

fn provider_workspace_scope_matches_context(
    scope: &agent_semantic_search::SemanticWorkspaceScope,
    expected_root: &Path,
    context: &ProviderGraphFactsContext<'_>,
) -> bool {
    scope.provider_id == context.provider.provider_id
        && scope.language_id == context.provider.language_id
        && scope.discovery_root == expected_root
}

fn load_cached_provider_workspace_scope(
    project_root: &Path,
    context: &ProviderGraphFactsContext<'_>,
) -> Option<agent_semantic_search::SemanticWorkspaceScope> {
    let cache_path = provider_workspace_scope_cache_path(project_root, context);
    let cached: Value = serde_json::from_slice(&fs::read(cache_path).ok()?).ok()?;
    if cached.get("providerBinaryStamp").and_then(Value::as_str)
        != Some(provider_binary_stamp(context.provider).as_str())
    {
        return None;
    }
    let packet = cached.get("packet")?;
    let scope = agent_semantic_search::SemanticWorkspaceScope::from_packet(packet).ok()?;
    let expected_root = fs::canonicalize(project_root).ok()?;
    if !provider_workspace_scope_matches_context(&scope, &expected_root, context)
        || !scope.anchors.iter().all(|anchor| {
            fs::read(&anchor.path)
                .ok()
                .is_some_and(|bytes| sha256_bytes(&bytes) == anchor.sha256)
        })
    {
        return None;
    }
    Some(scope)
}

fn persist_provider_workspace_scope(
    project_root: &Path,
    context: &ProviderGraphFactsContext<'_>,
    packet: &Value,
) {
    let cache_path = provider_workspace_scope_cache_path(project_root, context);
    let Some(parent) = cache_path.parent() else {
        return;
    };
    if fs::create_dir_all(parent).is_err() {
        return;
    }
    let payload = serde_json::json!({
        "schemaId": "agent.semantic-protocols.provider-workspace-scope-cache",
        "schemaVersion": "1",
        "providerBinaryStamp": provider_binary_stamp(context.provider),
        "packet": packet,
    });
    let Ok(bytes) = serde_json::to_vec(&payload) else {
        return;
    };
    let temporary = cache_path.with_extension(format!("tmp-{}", std::process::id()));
    if fs::write(&temporary, bytes).is_ok() {
        let _ = fs::rename(&temporary, &cache_path);
    }
}

fn provider_workspace_scope_cache_path(
    project_root: &Path,
    context: &ProviderGraphFactsContext<'_>,
) -> PathBuf {
    let canonical_root = fs::canonicalize(project_root).unwrap_or_else(|_| project_root.to_owned());
    let identity = format!(
        "workspace-scope-v1\n{}\n{}\n{}\n{}",
        context.provider.provider_id,
        context.provider.language_id,
        canonical_root.display(),
        provider_binary_stamp(context.provider),
    );
    context
        .cache_home
        .join("workspace-scope")
        .join(format!("{}.json", sha256_bytes(identity.as_bytes())))
}

fn provider_binary_stamp(provider: &ActivatedProvider) -> String {
    let metadata = fs::metadata(&provider.binary).ok();
    let modified = metadata
        .as_ref()
        .and_then(|metadata| metadata.modified().ok())
        .and_then(|modified| modified.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|duration| format!("{}:{}", duration.as_secs(), duration.subsec_nanos()))
        .unwrap_or_else(|| "unknown".to_string());
    format!(
        "{}:{}:{modified}",
        provider.binary,
        metadata.as_ref().map_or(0, fs::Metadata::len),
    )
}

fn sha256_bytes(bytes: &[u8]) -> String {
    format!("sha256:{:x}", Sha256::digest(bytes))
}

pub(super) fn collect_provider_graph_facts(
    language_id: &str,
    project_root: &Path,
    query: Option<&str>,
    candidates: &[Candidate],
    config: &AspConfig,
    context: Option<&ProviderGraphFactsContext<'_>>,
) -> Result<ProviderGraphFacts, String> {
    let Some(context) = context else {
        return Ok(ProviderGraphFacts::default());
    };
    if !context.provider.search_capabilities.semantic_facts {
        return Ok(ProviderGraphFacts::default());
    }
    let Some(query) = query else {
        return Ok(ProviderGraphFacts::default());
    };
    let Some(intent) = query_requests_semantic_facts(context.provider, query) else {
        return Ok(ProviderGraphFacts::default());
    };
    if !intent.requested {
        return Ok(ProviderGraphFacts::default());
    }
    if candidates.is_empty() {
        return Ok(provider_graph_facts_intent_receipt(&intent));
    }
    let fact_candidates = provider_fact_candidates(candidates);
    let input_candidates = candidates.len();
    let truncated_candidates = input_candidates.saturating_sub(fact_candidates.len());
    let semantic_fact_limits = ProviderProcessLimits {
        timeout: Some(provider_graph_fact_timeout()),
        max_stdout_bytes: Some(PROVIDER_GRAPH_FACT_OUTPUT_LIMIT_BYTES),
        max_stderr_bytes: Some(PROVIDER_GRAPH_FACT_OUTPUT_LIMIT_BYTES),
        memory_limit_bytes: Some(1024 * 1024 * 1024),
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
        project_root,
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
                ..provider_graph_facts_intent_receipt(&intent)
            });
        }
    };
    if !output.status.success() {
        return Ok(ProviderGraphFacts {
            input_candidates,
            fact_candidates: fact_candidates.len(),
            truncated_candidates,
            ..provider_graph_facts_intent_receipt(&intent)
        });
    }
    let mut facts = provider_graph_facts_from_stdout(output.stdout.as_ref())?;
    facts.input_candidates = input_candidates;
    facts.fact_candidates = fact_candidates.len();
    facts.truncated_candidates = truncated_candidates;
    facts.descriptor_id = Some(intent.descriptor_id);
    facts.descriptor_version = Some(intent.descriptor_version);
    facts.matched_axes = intent.matched_axes;
    facts.matched_terms = intent.matched_terms;
    Ok(facts)
}

fn provider_graph_fact_timeout() -> Duration {
    env::var("ASP_PROVIDER_GRAPH_FACT_TIMEOUT_MS")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .filter(|value| *value > 0)
        .map(Duration::from_millis)
        .unwrap_or_else(|| Duration::from_millis(PROVIDER_GRAPH_FACT_TIMEOUT_MS))
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
    let Some(envelope) = agent_semantic_search::provider_facts_envelope_from_stdout(stdout) else {
        return Ok(ProviderGraphFacts::default());
    };
    Ok(ProviderGraphFacts {
        nodes: envelope.nodes,
        edges: envelope.edges,
        candidate_annotations: envelope.candidate_annotations,
        ..ProviderGraphFacts::default()
    })
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

pub(super) fn query_requests_semantic_facts(
    provider: &agent_semantic_hook::ActivatedProvider,
    query: &str,
) -> Option<agent_semantic_search::SearchPipeSemanticFactsIntentDecision> {
    let descriptor = provider.semantic_facts_descriptor.as_ref()?;
    let intent_axes = descriptor
        .intent_axes
        .iter()
        .map(
            |intent_axis| agent_semantic_search::SearchPipeSemanticFactsIntentAxis {
                axis: &intent_axis.axis,
                terms: &intent_axis.terms,
                roles: &intent_axis.roles,
            },
        )
        .collect::<Vec<_>>();
    Some(agent_semantic_search::search_pipe_semantic_facts_intent(
        &provider.language_id,
        query,
        agent_semantic_search::SearchPipeSemanticFactsDescriptor {
            descriptor_id: &descriptor.descriptor_id,
            descriptor_version: &descriptor.descriptor_version,
            intent_axes: &intent_axes,
        },
    ))
}

pub(super) fn with_query_pack_descriptor<R>(
    context: Option<&ProviderGraphFactsContext<'_>>,
    f: impl FnOnce(Option<agent_semantic_search::SearchPipeQueryPackDescriptor<'_>>) -> R,
) -> R {
    let Some(descriptor) =
        context.and_then(|context| context.provider.query_pack_descriptor.as_ref())
    else {
        return f(None);
    };
    let term_role_overrides = descriptor
        .term_role_overrides
        .iter()
        .map(
            |override_| agent_semantic_search::SearchPipeQueryPackTermRoleOverride {
                term: &override_.term,
                role: &override_.role,
                case_sensitive: override_.case_sensitive,
            },
        )
        .collect::<Vec<_>>();
    let clause_sets = descriptor
        .recipes
        .iter()
        .map(|recipe| {
            recipe
                .clauses
                .iter()
                .map(|clause| agent_semantic_search::SearchPipeQueryPackClause {
                    terms: &clause.terms,
                    roles: &clause.roles,
                    intent_axes: &clause.intent_axes,
                })
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    let recipes = descriptor
        .recipes
        .iter()
        .zip(&clause_sets)
        .map(
            |(recipe, clauses)| agent_semantic_search::SearchPipeQueryPackRecipe {
                recipe_id: &recipe.recipe_id,
                trigger_terms: &recipe.trigger.terms,
                trigger_match: &recipe.trigger.r#match,
                clauses,
            },
        )
        .collect::<Vec<_>>();
    f(Some(agent_semantic_search::SearchPipeQueryPackDescriptor {
        descriptor_id: &descriptor.descriptor_id,
        descriptor_version: &descriptor.descriptor_version,
        language_id: &descriptor.language_id,
        term_role_overrides: &term_role_overrides,
        recipes: &recipes,
    }))
}
