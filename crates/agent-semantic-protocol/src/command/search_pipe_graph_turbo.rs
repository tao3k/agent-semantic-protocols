//! Graph-turbo request packets for ASP-owned fast search candidates.

use std::{
    collections::{HashMap, HashSet},
    env,
    path::Path,
};

use serde_json::{Value, json};

use super::{
    search_config::AspConfig,
    search_pipe_dependency_facts::{
        DependencyFact, candidate_usage_dependency_matches_query, dependency_matches_query,
    },
    search_pipe_dependency_seed_cache::collect_cached_dependency_facts,
    search_pipe_graph_nodes::{
        append_candidate_nodes, append_hot_nodes, append_project_topology_nodes,
        append_submodule_owner_edges, candidate_node_id, hot_node_id, project_submodule_paths,
        stable_node_id,
    },
    search_pipe_graph_turbo_owner_rank::ranked_candidate_paths_with_topology,
    search_pipe_graph_turbo_seed::{has_package_path_candidate, query_owner_seed_paths},
    search_pipe_model::{Candidate, SearchPipeSourceTrace},
    search_pipe_provider_facts::{ProviderGraphFacts, ProviderGraphFactsContext},
    search_pipe_quality::{
        analyze_search_pipe_quality, compact_fact_value, is_generated_path, query_allows_generated,
    },
    search_pipe_quality_model::SearchPipeQuality,
    search_pipe_query_evidence::{is_high_value_term, strong_match},
    search_pipe_query_pack::{query_clauses, unique_query_terms},
    search_pipe_seed_decision::{
        SearchActionSelection, SearchEvidenceState, SeedPhaseDecision,
        recommended_action_for_seed_risk,
    },
    search_pipe_surfaces::{
        include_deps, include_items, include_owner_context, include_tests, include_topology,
        normalized_search_surfaces,
    },
};

const GRAPH_TURBO_REQUEST_SCHEMA_ID: &str = "agent.semantic-protocols.semantic-graph-turbo-request";
const GRAPH_TURBO_CANDIDATE_NODE_LIMIT: usize = 64;

pub(super) struct GraphTurboSearchPipeRequest<'a> {
    pub(super) surface: &'a str,
    pub(super) language_id: &'a str,
    pub(super) dependency_root: &'a Path,
    pub(super) cache_home: &'a Path,
    pub(super) query: Option<&'a str>,
    pub(super) query_clauses: &'a [String],
    pub(super) candidates: &'a [Candidate],
    pub(super) precomputed_quality: Option<&'a SearchPipeQuality>,
    pub(super) pipes: &'a [String],
    pub(super) source: &'a str,
    pub(super) candidate_sources: &'a [String],
    pub(super) source_trace: &'a [SearchPipeSourceTrace],
    pub(super) provider_facts: &'a ProviderGraphFacts,
    pub(super) provider_context: Option<&'a ProviderGraphFactsContext<'a>>,
    pub(super) config: &'a AspConfig,
    pub(super) read_memory_selectors: &'a [String],
    pub(super) action_frontier: &'a [Value],
}

pub(super) fn render_graph_turbo_request(
    request: GraphTurboSearchPipeRequest<'_>,
) -> Result<String, String> {
    let packet = graph_turbo_request(&request);
    serde_json::to_string(&packet)
        .map(|mut text| {
            text.push('\n');
            text
        })
        .map_err(|error| format!("failed to serialize graph turbo request: {error}"))
}

pub(super) fn graph_turbo_request(request: &GraphTurboSearchPipeRequest<'_>) -> Value {
    let language_id = request.language_id;
    let dependency_root = request.dependency_root;
    let cache_home = request.cache_home;
    let surface = request.surface;
    let query = request.query;
    let query_clauses = request.query_clauses;
    let candidates = request.candidates;
    let precomputed_quality = request.precomputed_quality;
    let pipes = request.pipes;
    let source = request.source;
    let candidate_sources = request.candidate_sources;
    let source_trace = request.source_trace;
    let provider_facts = request.provider_facts;
    let provider_context = request.provider_context;
    let config = request.config;
    let read_memory_selectors = request.read_memory_selectors;
    let external_action_frontier = request.action_frontier;
    let mut surfaces = normalized_search_surfaces(pipes);
    let mut nodes = Vec::new();
    let mut edges = Vec::new();
    let mut seed_ids = Vec::new();
    let query_present = query.map(|query| !query.trim().is_empty()).unwrap_or(false);
    let mut query_seed_present = false;
    if let Some(query) = query.filter(|query| !query.trim().is_empty()) {
        let query_id = stable_node_id("query", query);
        seed_ids.push(query_id.clone());
        query_seed_present = true;
        nodes.push(json!({
            "id": query_id,
            "kind": "query",
            "role": "term",
            "value": query,
            "action": "fzf"
        }));
    }

    let computed_quality = if precomputed_quality.is_none() {
        query
            .filter(|query| !query.trim().is_empty())
            .map(|query| analyze_search_pipe_quality(language_id, query, candidates))
    } else {
        None
    };
    let quality_for_query = precomputed_quality.or(computed_quality.as_ref());
    let augmented_candidates = if quality_for_query.is_some_and(allow_query_anchor_candidates) {
        query_anchor_candidates(language_id, query, candidates)
    } else {
        candidates.to_vec()
    };
    let query_terms = query.map(query_terms).unwrap_or_default();
    let graph_candidates = sparse_graph_candidates(&augmented_candidates, query);
    let query_adjustment_policy = query_adjustment_policy_from_env();
    let topology_membership_enabled = query_adjustment_policy
        .as_ref()
        .and_then(|policy| policy.get("topologyMembership"))
        .and_then(Value::as_bool)
        .unwrap_or(true);
    let owners = ranked_candidate_paths_with_topology(
        &graph_candidates,
        &query_terms,
        topology_membership_enabled.then_some(dependency_root),
    );
    let topology_submodule_count = project_submodule_paths(dependency_root).len();
    let dependency_seed = if include_deps(&surfaces) {
        collect_cached_dependency_facts(
            language_id,
            dependency_root,
            cache_home,
            config,
            provider_context_for_dependency_seed(
                surface,
                language_id,
                provider_context,
                &surfaces,
                query,
                &graph_candidates,
            ),
            query,
            &graph_candidates,
        )
    } else {
        let manifest_facts = super::search_pipe_dependency_facts::collect_manifest_dependency_facts(
            language_id,
            dependency_root,
        );
        if should_auto_include_dependency_surface(query, &surfaces, &manifest_facts) {
            surfaces.push("deps".to_string());
            collect_cached_dependency_facts(
                language_id,
                dependency_root,
                cache_home,
                config,
                provider_context_for_dependency_seed(
                    surface,
                    language_id,
                    provider_context,
                    &surfaces,
                    query,
                    &graph_candidates,
                ),
                query,
                &graph_candidates,
            )
        } else {
            super::search_pipe_dependency_seed_cache::CachedDependencyFacts {
                cache_status: "skipped",
                topology_source: "not-requested",
                facts: Vec::new(),
            }
        }
    };
    let dependency_seed_cache_status = dependency_seed.cache_status;
    let dependency_facts = dependency_seed.facts;
    let profile = profile_for_surfaces(&surfaces);
    let include_owner_context = include_owner_context(&surfaces);
    let include_items = include_items(&surfaces);
    let include_tests = include_tests(&surfaces);
    let include_deps = include_deps(&surfaces);
    let include_topology = include_topology(&surfaces);
    let seed_decision =
        SeedPhaseDecision::from_query_shape(query_seed_present, query_terms.len(), owners.len());
    let query_owner_anchor_budget = if has_package_path_candidate(&graph_candidates, &query_terms) {
        seed_decision.query_owner_anchor_budget.max(1)
    } else {
        seed_decision.query_owner_anchor_budget
    };
    let mut query_owner_seed_count = 0usize;
    if query_seed_present {
        for owner in query_owner_seed_paths(
            &graph_candidates,
            &owners,
            query_owner_anchor_budget,
            &query_terms,
        ) {
            let owner_seed_id = stable_node_id("owner", &owner);
            if !seed_ids.contains(&owner_seed_id) {
                seed_ids.push(owner_seed_id);
                query_owner_seed_count += 1;
            }
        }
    }
    let mut fallback_owner_seed_count = 0usize;
    if seed_ids.is_empty() {
        let fallback_owner_seed_ids = owners
            .iter()
            .take(2)
            .map(|owner| stable_node_id("owner", owner))
            .collect::<Vec<_>>();
        fallback_owner_seed_count = fallback_owner_seed_ids.len();
        seed_ids.extend(fallback_owner_seed_ids);
    }
    let seed_plan = graph_turbo_seed_plan(GraphTurboSeedPlanInput {
        query_present,
        query_seed_present,
        candidate_count: graph_candidates.len(),
        candidate_owner_count: owners.len(),
        query_owner_seed_count,
        fallback_owner_seed_count,
        seed_ids: &seed_ids,
        seed_decision: &seed_decision,
    });
    if include_owner_context {
        append_owner_nodes(&mut nodes, &owners);
    }
    if include_topology {
        append_project_topology_nodes(
            &mut nodes,
            &mut edges,
            language_id,
            dependency_root,
            &graph_candidates,
        );
    }
    if include_items {
        append_candidate_nodes(
            &mut nodes,
            language_id,
            &graph_candidates,
            GRAPH_TURBO_CANDIDATE_NODE_LIMIT,
        );
        append_hot_nodes(
            &mut nodes,
            language_id,
            &graph_candidates,
            GRAPH_TURBO_CANDIDATE_NODE_LIMIT,
        );
        append_provider_fact_nodes(&mut nodes, provider_facts);
    }
    if include_deps {
        append_dependency_nodes(&mut nodes, &dependency_facts);
    }
    if include_tests {
        append_test_nodes(&mut nodes, &owners);
    }
    append_graph_edges(
        &mut edges,
        GraphEdgeInputs {
            query,
            candidates: &graph_candidates,
            owners: &owners,
            workspace_root: dependency_root,
            dependency_facts: &dependency_facts,
            provider_facts,
            surfaces: &surfaces,
        },
    );

    let mut packet = json!({
        "schemaId": GRAPH_TURBO_REQUEST_SCHEMA_ID,
        "schemaVersion": "1",
        "protocolId": "agent.semantic-protocols.semantic-language",
        "protocolVersion": "1",
        "packetKind": "graph-turbo-request",
        "surface": surface,
        "queryTerms": query_terms,
        "profile": profile,
        "algorithm": "typed-ppr-diverse",
        "surfaces": surfaces,
        "source": source,
        "candidateSources": candidate_sources,
        "sourceTrace": graph_turbo_source_trace(source_trace),
        "seedIds": seed_ids,
        "seedPlan": seed_plan,
        "budget": 10,
        "kindBudgets": {"owner": 4, "workspace": 1, "provider-root": 2, "submodule": 4, "dependency": 2, "test": 3, "item": 6, "field": 4, "type": 3, "collection": 2, "hot": 3},
        "windowMerge": {"enabled": true, "maxGapLines": 8},
        "pathBudget": 5,
        "pathMaxHops": 4,
        "cache": {
            "enabled": true,
            "dependencySeed": {
                "enabled": true,
                "status": dependency_seed_cache_status,
                "topology": dependency_seed.topology_source,
                "facts": dependency_facts.len(),
            },
        },
        "graph": {
            "nodes": nodes,
            "edges": edges,
        },
    });
    if topology_membership_enabled && topology_submodule_count > 0 {
        packet["fields"] = json!({"topologyRank": "submodule-membership"});
    }
    if topology_submodule_count > 0 {
        packet["summary"] = json!({"topologyRankSubmodules": topology_submodule_count});
    }
    if !query_clauses.is_empty() {
        packet["queryClauses"] = json!(query_clauses);
    }
    if !read_memory_selectors.is_empty() {
        packet["readMemory"] = json!({
            "seenSelectors": read_memory_selectors,
        });
    }
    if let Some(policy) = query_adjustment_policy {
        packet["queryAdjustmentPolicy"] = policy;
    }
    if !external_action_frontier.is_empty() {
        packet["actionFrontier"] = Value::Array(external_action_frontier.to_vec());
    }
    packet
}

fn allow_query_anchor_candidates(quality: &SearchPipeQuality) -> bool {
    quality.query_pack_quality != "low"
        && quality.package_cohesion != "low"
        && quality.weak_terms.is_empty()
}

fn provider_context_for_dependency_seed<'a>(
    surface: &str,
    language_id: &str,
    provider_context: Option<&'a ProviderGraphFactsContext<'a>>,
    surfaces: &[String],
    query: Option<&str>,
    candidates: &[Candidate],
) -> Option<&'a ProviderGraphFactsContext<'a>> {
    let context = provider_context?;
    if include_deps(surfaces) && surface != "search-fzf" {
        return Some(context);
    }
    let search_pipe_dependency_query = surface == "search-pipe"
        && query.is_some_and(|query| {
            candidate_usage_dependency_matches_query(language_id, candidates, query)
        });
    search_pipe_dependency_query.then_some(context)
}

fn query_anchor_candidates(
    language_id: &str,
    query: Option<&str>,
    candidates: &[Candidate],
) -> Vec<Candidate> {
    let Some(query) = query.filter(|query| !query.trim().is_empty()) else {
        return candidates.to_vec();
    };
    let terms = unique_query_terms(&query_clauses(language_id, query));
    let mut augmented = candidates.to_vec();
    let mut seen = augmented
        .iter()
        .map(|candidate| {
            (
                candidate.path.clone(),
                candidate.line,
                candidate.symbol.to_ascii_lowercase(),
            )
        })
        .collect::<HashSet<_>>();
    for term in terms.iter().filter(|term| is_high_value_term(term)) {
        if augmented
            .iter()
            .any(|candidate| candidate.symbol == term.raw || candidate.symbol == term.lower)
        {
            continue;
        }
        let Some(source_candidate) = candidates
            .iter()
            .find(|candidate| strong_match(language_id, candidate, term))
        else {
            continue;
        };
        let key = (
            source_candidate.path.clone(),
            source_candidate.line,
            term.lower.clone(),
        );
        if !seen.insert(key) {
            continue;
        }
        augmented.push(Candidate {
            path: source_candidate.path.clone(),
            line: source_candidate.line,
            end_line: source_candidate.end_line,
            symbol: term.raw.clone(),
            text: source_candidate.text.clone(),
            source: "query-anchor".to_string(),
            confidence: "query-anchor".to_string(),
        });
    }
    augmented
}

fn query_adjustment_policy_from_env() -> Option<Value> {
    let variant = env::var("ASP_GRAPH_TURBO_ABLATION_VARIANT").ok()?;
    match variant.trim() {
        "no-query-seed-prior" => Some(json!({"seedPrior": false})),
        "no-package-cohesion" => Some(json!({"packageCohesion": false})),
        "no-query-clause-coverage" => Some(json!({"queryClauseCoverage": false})),
        "no-local-evidence" => Some(json!({"localEvidence": false})),
        "no-topology-membership" => Some(json!({"topologyMembership": false})),
        _ => None,
    }
}

struct GraphTurboSeedPlanInput<'a> {
    query_present: bool,
    query_seed_present: bool,
    candidate_count: usize,
    candidate_owner_count: usize,
    query_owner_seed_count: usize,
    fallback_owner_seed_count: usize,
    seed_ids: &'a [String],
    seed_decision: &'a SeedPhaseDecision,
}

fn graph_turbo_seed_plan(input: GraphTurboSeedPlanInput<'_>) -> Value {
    let reason = if input.query_seed_present {
        "query"
    } else if input.fallback_owner_seed_count > 0 {
        "fallback-owner"
    } else {
        "empty"
    };
    let mut risk_factors = Vec::new();
    if input.seed_ids.is_empty() {
        risk_factors.push("empty-seed-frontier");
    }
    if input.fallback_owner_seed_count > 0 {
        risk_factors.push("fallback-owner");
    }
    if input.query_present && !input.query_seed_present {
        risk_factors.push("query-seed-missing");
    }
    risk_factors.extend(input.seed_decision.risk_factors.iter().copied());
    let seed_quality = if input.seed_ids.is_empty() {
        "fail"
    } else if risk_factors.is_empty() {
        "good"
    } else {
        "review"
    };
    let recommended_actions = if risk_factors.is_empty() {
        vec!["keep-query-seed"]
    } else {
        risk_factors
            .iter()
            .filter_map(|risk| recommended_action_for_seed_risk(risk))
            .collect::<Vec<_>>()
    };
    let selection = SearchActionSelection::for_first_action(SearchEvidenceState::Unknown, "seed");
    let evidence_states = SearchEvidenceState::all()
        .iter()
        .map(|state| state.as_str())
        .collect::<Vec<_>>();
    json!({
        "phase": "seed-query",
        "algorithm": "asp-search-pipe-v1",
        "reason": reason,
        "seedQuality": seed_quality,
        "queryPresent": input.query_present,
        "querySeedPresent": input.query_seed_present,
        "candidateCount": input.candidate_count,
        "candidateOwnerCount": input.candidate_owner_count,
        "queryOwnerSeedCount": input.query_owner_seed_count,
        "fallbackOwnerSeedCount": input.fallback_owner_seed_count,
        "selectedSeedCount": input.seed_ids.len(),
        "seedIds": input.seed_ids,
        "riskFactors": risk_factors,
        "recommendedActions": recommended_actions,
        "selectionPolicy": {
            "flow": "evidence-state-reasoning-tree",
            "evidenceState": selection.evidence_state.as_str(),
            "knownEvidenceStates": evidence_states,
            "firstActionStage": selection.first_action_stage,
            "allowedFirstStages": selection.allowed_first_stages,
            "disallowedFirstStages": selection.disallowed_first_stages,
            "firstActionMatchesEvidenceState": selection.first_action_matches_evidence_state,
            "reasoningTreeRouteShown": selection.reasoning_tree_route_shown,
            "chosenRoutePreconditionsMet": selection.chosen_route_preconditions_met,
            "unnecessarySeedCount": selection.unnecessary_seed_count,
            "seedWhenKnownOwnerCount": selection.seed_when_known_owner_count,
            "seedWhenKnownSymbolCount": selection.seed_when_known_symbol_count,
            "seedWhenKnownSelectorCount": selection.seed_when_known_selector_count,
        },
    })
}

fn sparse_graph_candidates(candidates: &[Candidate], query: Option<&str>) -> Vec<Candidate> {
    let allow_generated = query_allows_generated(query);
    let filtered = candidates
        .iter()
        .filter(|candidate| allow_generated || !is_generated_path(&candidate.path))
        .cloned()
        .collect::<Vec<_>>();
    let candidates = if filtered.is_empty() {
        candidates.to_vec()
    } else {
        filtered
    };
    let mut selected = Vec::new();
    let mut selected_indices = HashSet::new();
    let mut symbol_counts: HashMap<String, usize> = HashMap::new();
    let mut per_symbol_limit = 1usize;
    while selected.len() < GRAPH_TURBO_CANDIDATE_NODE_LIMIT
        && selected_indices.len() < candidates.len()
    {
        let mut added = false;
        for (index, candidate) in candidates.iter().enumerate() {
            if selected.len() >= GRAPH_TURBO_CANDIDATE_NODE_LIMIT {
                break;
            }
            if selected_indices.contains(&index) {
                continue;
            }
            let symbol_count = symbol_counts
                .get(candidate.symbol.as_str())
                .copied()
                .unwrap_or(0);
            if symbol_count >= per_symbol_limit {
                continue;
            }
            selected_indices.insert(index);
            symbol_counts.insert(candidate.symbol.clone(), symbol_count + 1);
            selected.push(candidate.clone());
            added = true;
        }
        if !added {
            per_symbol_limit += 1;
        }
    }
    selected
}

fn append_owner_nodes(nodes: &mut Vec<Value>, owners: &[String]) {
    for owner in owners {
        nodes.push(json!({
            "id": stable_node_id("owner", owner),
            "kind": "owner",
            "role": "path",
            "value": owner,
            "action": "owner",
            "path": owner
        }));
    }
}

fn graph_turbo_source_trace(source_trace: &[SearchPipeSourceTrace]) -> Value {
    Value::Array(
        source_trace
            .iter()
            .map(|trace| {
                let mut entry = json!({
                    "source": trace.source,
                    "status": trace.status,
                    "matched": trace.matched,
                    "missing": trace.missing,
                    "normalized": trace.normalized,
                });
                if !trace.fields.is_empty() {
                    entry["fields"] = json!(trace.fields);
                }
                entry
            })
            .collect(),
    )
}

fn append_provider_fact_nodes(nodes: &mut Vec<Value>, provider_facts: &ProviderGraphFacts) {
    nodes.extend(
        provider_facts
            .nodes
            .iter()
            .cloned()
            .map(compact_provider_fact_node),
    );
}

fn compact_provider_fact_node(mut node: Value) -> Value {
    if let Some(value) = node.get("value").and_then(Value::as_str) {
        node["value"] = json!(compact_fact_value(value));
    }
    if let Some(value) = node.get("matchText").and_then(Value::as_str) {
        node["matchText"] = json!(compact_fact_value(value));
    }
    node
}

fn append_dependency_nodes(nodes: &mut Vec<Value>, dependency_facts: &[DependencyFact]) {
    let mut seen = HashSet::new();
    let mut seen_versions = HashSet::new();
    for fact in dependency_facts {
        if seen.insert(fact.dependency.clone()) {
            nodes.push(json!({
                "id": stable_node_id("dependency", &fact.dependency),
                "kind": "dependency",
                "role": "pkg",
                "value": fact.dependency,
                "action": "deps",
                "source": "finder",
                "confidence": dependency_confidence(fact),
            }));
        }
        if let Some(version) = fact.version.as_deref()
            && seen_versions.insert(format!("{}@{version}", fact.dependency))
        {
            nodes.push(json!({
                "id": stable_node_id("dependency-version", &format!("{}@{version}", fact.dependency)),
                "kind": "dependency-version",
                "role": "version",
                "value": format!("{}@{version}", fact.dependency),
                "action": "evidence",
                "source": "finder",
                "confidence": dependency_confidence(fact),
            }));
        }
    }
}

fn dependency_confidence(fact: &DependencyFact) -> &'static str {
    if fact.source == "manifest" {
        "exact"
    } else {
        "likely"
    }
}

fn append_test_nodes(nodes: &mut Vec<Value>, owners: &[String]) {
    for owner in owners {
        nodes.push(json!({
            "id": stable_node_id("test", owner),
            "kind": "test",
            "role": "path",
            "value": owner,
            "action": "tests",
            "path": owner
        }));
    }
}

struct GraphEdgeInputs<'a> {
    query: Option<&'a str>,
    candidates: &'a [Candidate],
    owners: &'a [String],
    workspace_root: &'a std::path::Path,
    dependency_facts: &'a [DependencyFact],
    provider_facts: &'a ProviderGraphFacts,
    surfaces: &'a [String],
}

fn append_graph_edges(edges: &mut Vec<Value>, input: GraphEdgeInputs<'_>) {
    if let Some(query) = input.query.filter(|query| !query.trim().is_empty()) {
        append_query_match_edges(edges, query, input.candidates, input.owners, input.surfaces);
        if include_deps(input.surfaces) {
            append_query_dependency_edges(edges, query, input.dependency_facts);
        }
    }
    if include_items(input.surfaces) {
        append_owner_candidate_edges(edges, input.candidates);
        append_candidate_hot_edges(edges, input.candidates);
        append_provider_fact_edges(edges, input.provider_facts);
    }
    if include_topology(input.surfaces) && include_owner_context(input.surfaces) {
        append_submodule_owner_edges(edges, input.workspace_root, input.owners);
    }
    if include_deps(input.surfaces) {
        append_owner_dependency_edges(edges, input.dependency_facts);
        append_dependency_version_edges(edges, input.dependency_facts);
    }
    if include_tests(input.surfaces) {
        append_test_cover_edges(edges, input.owners);
    }
}

fn append_query_match_edges(
    edges: &mut Vec<Value>,
    query: &str,
    candidates: &[Candidate],
    owners: &[String],
    surfaces: &[String],
) {
    let query_id = stable_node_id("query", query);
    if include_owner_context(surfaces) {
        for owner in owners {
            edges.push(edge(&query_id, &stable_node_id("owner", owner), "matches"));
        }
    }
    if include_items(surfaces) {
        for candidate in candidates.iter().take(GRAPH_TURBO_CANDIDATE_NODE_LIMIT) {
            edges.push(edge(&query_id, &candidate_node_id(candidate), "matches"));
        }
    }
}

fn append_query_dependency_edges(edges: &mut Vec<Value>, query: &str, facts: &[DependencyFact]) {
    let query_id = stable_node_id("query", query);
    for fact in facts
        .iter()
        .filter(|fact| dependency_matches_query(&fact.dependency, query))
    {
        edges.push(edge(
            &query_id,
            &stable_node_id("dependency", &fact.dependency),
            "matches",
        ));
    }
}

fn append_owner_candidate_edges(edges: &mut Vec<Value>, candidates: &[Candidate]) {
    for candidate in candidates.iter().take(GRAPH_TURBO_CANDIDATE_NODE_LIMIT) {
        edges.push(edge(
            &stable_node_id("owner", &candidate.path),
            &candidate_node_id(candidate),
            "contains",
        ));
    }
}

fn append_candidate_hot_edges(edges: &mut Vec<Value>, candidates: &[Candidate]) {
    for candidate in candidates.iter().take(GRAPH_TURBO_CANDIDATE_NODE_LIMIT) {
        edges.push(edge(
            &candidate_node_id(candidate),
            &hot_node_id(candidate),
            "contains",
        ));
    }
}

fn append_provider_fact_edges(edges: &mut Vec<Value>, provider_facts: &ProviderGraphFacts) {
    edges.extend(provider_facts.edges.iter().cloned());
}

fn append_owner_dependency_edges(edges: &mut Vec<Value>, dependency_facts: &[DependencyFact]) {
    let mut seen = HashSet::new();
    for fact in dependency_facts {
        if fact.source == "manifest" {
            continue;
        }
        let key = format!("{}:{}", fact.owner_path, fact.dependency);
        if seen.insert(key) {
            edges.push(edge(
                &stable_node_id("owner", &fact.owner_path),
                &stable_node_id("dependency", &fact.dependency),
                "imports",
            ));
        }
    }
}

fn append_dependency_version_edges(edges: &mut Vec<Value>, dependency_facts: &[DependencyFact]) {
    let mut seen = HashSet::new();
    for fact in dependency_facts {
        let Some(version) = fact.version.as_deref() else {
            continue;
        };
        let key = format!("{}@{version}", fact.dependency);
        if seen.insert(key.clone()) {
            edges.push(edge(
                &stable_node_id("dependency", &fact.dependency),
                &stable_node_id("dependency-version", &key),
                "version_locked",
            ));
        }
    }
}

fn append_test_cover_edges(edges: &mut Vec<Value>, owners: &[String]) {
    for owner in owners {
        edges.push(edge(
            &stable_node_id("owner", owner),
            &stable_node_id("test", owner),
            "covers",
        ));
    }
}

fn edge(source: &str, target: &str, relation: &str) -> Value {
    json!({
        "source": source,
        "target": target,
        "relation": relation,
    })
}

fn should_auto_include_dependency_surface(
    query: Option<&str>,
    surfaces: &[String],
    dependency_facts: &[DependencyFact],
) -> bool {
    let Some(query) = query else {
        return false;
    };
    !include_deps(surfaces)
        && dependency_facts
            .iter()
            .any(|fact| dependency_matches_query(&fact.dependency, query))
}

fn profile_for_surfaces(surfaces: &[String]) -> &'static str {
    if include_deps(surfaces) {
        "query-deps"
    } else if include_tests(surfaces) && !include_items(surfaces) {
        "owner-tests"
    } else {
        "owner-query"
    }
}

fn query_terms(query: &str) -> Vec<String> {
    query
        .split(|character: char| character == ',' || character == '|' || character.is_whitespace())
        .map(str::trim)
        .filter(|term| {
            term.chars()
                .any(|character| character.is_ascii_alphanumeric())
        })
        .map(ToOwned::to_owned)
        .fold(Vec::new(), |mut terms, term| {
            if !terms.iter().any(|seen| seen == &term) {
                terms.push(term);
            }
            terms
        })
}
