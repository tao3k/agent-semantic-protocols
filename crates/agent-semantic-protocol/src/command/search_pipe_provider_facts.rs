//! Provider-owned semantic fact enrichment for ASP search pipe graph requests.

use std::path::Path;

use agent_semantic_hook::{ActivatedProvider, RuntimeProfiles};
use serde_json::Value;

use super::search_pipe_model::Candidate;

const PROVIDER_GRAPH_FACT_CANDIDATE_LIMIT: usize = 12;

#[derive(Debug, Default)]
pub(super) struct ProviderGraphFacts {
    pub(super) nodes: Vec<Value>,
    pub(super) edges: Vec<Value>,
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
}

pub(super) async fn collect_provider_graph_facts(
    language_id: &str,
    project_root: &Path,
    query: Option<&str>,
    candidates: &[Candidate],
    context: Option<&ProviderGraphFactsContext<'_>>,
    search_data_plane: Option<&crate::server::runtime_server::RuntimeServerSearchDataPlane>,
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
    let Some(intent) = query_requests_semantic_facts(context.provider, query)? else {
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
    let search_data_plane = search_data_plane.ok_or_else(|| {
        format!(
            "resident provider relation generation is required: language={language_id} workspace={}",
            project_root.display()
        )
    })?;
    let mut sources = Vec::new();
    for candidate in &fact_candidates {
        if let Some(selector) = candidate.selector.as_deref() {
            sources.push(
                agent_semantic_client_db::workspace_db_ipc::RuntimeGraphFactSource {
                    kind: "item".to_owned(),
                    id: selector.to_owned(),
                },
            );
        }
        sources.push(
            agent_semantic_client_db::workspace_db_ipc::RuntimeGraphFactSource {
                kind: "owner".to_owned(),
                id: candidate.path.clone(),
            },
        );
    }
    let mut relations = search_data_plane.read_graph_facts(sources).await?.relations;
    relations.sort();
    relations.dedup();
    let mut nodes = std::collections::BTreeMap::new();
    let mut edges = Vec::with_capacity(relations.len());
    for relation in relations {
        nodes.insert(
            (relation.from.kind.clone(), relation.from.id.clone()),
            serde_json::to_value(&relation.from)
                .map_err(|error| format!("encode provider relation source node: {error}"))?,
        );
        nodes.insert(
            (relation.to.kind.clone(), relation.to.id.clone()),
            serde_json::to_value(&relation.to)
                .map_err(|error| format!("encode provider relation target node: {error}"))?,
        );
        edges.push(
            serde_json::to_value(&relation)
                .map_err(|error| format!("encode provider relation edge: {error}"))?,
        );
    }
    let mut facts = ProviderGraphFacts {
        nodes: nodes.into_values().collect(),
        edges,
        ..ProviderGraphFacts::default()
    };
    facts.input_candidates = input_candidates;
    facts.fact_candidates = fact_candidates.len();
    facts.truncated_candidates = truncated_candidates;
    facts.descriptor_id = Some(intent.descriptor_id);
    facts.descriptor_version = Some(intent.descriptor_version);
    facts.matched_axes = intent.matched_axes;
    facts.matched_terms = intent.matched_terms;
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

pub(super) fn query_requests_semantic_facts(
    provider: &agent_semantic_hook::ActivatedProvider,
    query: &str,
) -> Result<Option<agent_semantic_search::SearchPipeSemanticFactsIntentDecision>, String> {
    let Some(descriptor) = provider.semantic_facts_descriptor.as_ref() else {
        return Ok(None);
    };
    let intent_axis_roles = descriptor
        .intent_axes
        .iter()
        .map(|intent_axis| {
            intent_axis
                .roles()
                .iter()
                .copied()
                .map(provider_query_pack_role)
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    let intent_axis_terms = descriptor
        .intent_axes
        .iter()
        .map(|intent_axis| intent_axis.terms().map(str::to_owned).collect::<Vec<_>>())
        .collect::<Vec<_>>();
    let intent_axes = descriptor
        .intent_axes
        .iter()
        .zip(&intent_axis_terms)
        .zip(&intent_axis_roles)
        .map(|((intent_axis, terms), roles)| {
            agent_semantic_search::SearchPipeSemanticFactsIntentAxis {
                axis: intent_axis.axis(),
                terms,
                roles,
            }
        })
        .collect::<Vec<_>>();
    with_activated_provider_query_pack_descriptor(provider, |query_pack_descriptor| {
        Some(agent_semantic_search::search_pipe_semantic_facts_intent(
            agent_semantic_search::SearchPipeLanguageId::new(provider.language_id.as_str()),
            agent_semantic_search::SearchPipeQueryText::new(query),
            query_pack_descriptor,
            agent_semantic_search::SearchPipeSemanticFactsDescriptor {
                descriptor_id: &descriptor.descriptor_id,
                descriptor_version: &descriptor.descriptor_version,
                intent_axes: &intent_axes,
            },
        ))
    })
}

pub(super) fn with_query_pack_descriptor<R>(
    context: Option<&ProviderGraphFactsContext<'_>>,
    f: impl FnOnce(agent_semantic_search::SearchPipeQueryPackDescriptor<'_>) -> R,
) -> Result<R, String> {
    let provider = context.map(|context| context.provider).ok_or_else(|| {
        "provider query-pack descriptor is required for parser-owned search".to_string()
    })?;
    with_activated_provider_query_pack_descriptor(provider, f)
}

fn provider_query_pack_role(
    role: agent_semantic_hook::ProviderQueryPackTermRole,
) -> agent_semantic_search::SearchPipeTermRole {
    match role {
        agent_semantic_hook::ProviderQueryPackTermRole::Context => {
            agent_semantic_search::SearchPipeTermRole::Context
        }
        agent_semantic_hook::ProviderQueryPackTermRole::Concept => {
            agent_semantic_search::SearchPipeTermRole::Concept
        }
        agent_semantic_hook::ProviderQueryPackTermRole::Symbol => {
            agent_semantic_search::SearchPipeTermRole::Symbol
        }
        agent_semantic_hook::ProviderQueryPackTermRole::Literal => {
            agent_semantic_search::SearchPipeTermRole::Literal
        }
        agent_semantic_hook::ProviderQueryPackTermRole::DiagnosticCode => {
            agent_semantic_search::SearchPipeTermRole::DiagnosticCode
        }
    }
}

fn with_activated_provider_query_pack_descriptor<R>(
    provider: &agent_semantic_hook::ActivatedProvider,
    f: impl FnOnce(agent_semantic_search::SearchPipeQueryPackDescriptor<'_>) -> R,
) -> Result<R, String> {
    let descriptor = &provider.query_pack_descriptor;
    let term_role_overrides = descriptor
        .term_role_overrides()
        .iter()
        .map(
            |override_| agent_semantic_search::SearchPipeQueryPackTermRoleOverride {
                term: &override_.term,
                role: provider_query_pack_role(override_.role),
                case_sensitive: override_.case_sensitive,
            },
        )
        .collect::<Vec<_>>();
    let clause_role_sets = descriptor
        .recipes()
        .iter()
        .map(|recipe| {
            recipe
                .clauses
                .iter()
                .map(|clause| {
                    clause
                        .roles
                        .iter()
                        .copied()
                        .map(provider_query_pack_role)
                        .collect::<Vec<_>>()
                })
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    let clause_sets = descriptor
        .recipes()
        .iter()
        .zip(&clause_role_sets)
        .map(|(recipe, role_sets)| {
            recipe
                .clauses
                .iter()
                .zip(role_sets)
                .map(
                    |(clause, roles)| agent_semantic_search::SearchPipeQueryPackClause {
                        terms: &clause.terms,
                        roles,
                        intent_axes: &clause.intent_axes,
                    },
                )
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    let recipes = descriptor
        .recipes()
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
    Ok(f(agent_semantic_search::SearchPipeQueryPackDescriptor {
        descriptor_id: descriptor.descriptor_id(),
        descriptor_version: descriptor.descriptor_version(),
        language_id: descriptor.language_id(),
        term_role_overrides: &term_role_overrides,
        recipes: &recipes,
    }))
}
