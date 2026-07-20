//! Graph-turbo request packets for ASP-owned fast search candidates.

use std::{collections::HashSet, env, path::Path};

use serde_json::{Value, json};

use super::{
    search_config::AspConfig,
    search_pipe_dependency_facts::{
        DependencyFact, candidate_usage_dependency_matches_query, dependency_matches_query,
    },
    search_pipe_dependency_seed_cache::collect_cached_dependency_facts,
    search_pipe_graph_nodes::{
        append_candidate_nodes, append_hot_nodes, append_project_topology_nodes,
        append_submodule_owner_edges, candidate_node_id, hot_node_id, stable_node_id,
    },
    search_pipe_graph_turbo_owner_rank::graph_owner_rank_report_with_topology,
    search_pipe_graph_turbo_seed::{has_package_path_candidate, query_owner_seed_paths},
    search_pipe_model::{Candidate, SearchPipeSourceTrace},
    search_pipe_provider_facts::{ProviderGraphFacts, ProviderGraphFactsContext},
    search_pipe_seed_decision::SeedPhaseDecision,
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
    pub(super) source_snapshot: &'a agent_semantic_content_identity::SourceSnapshotEvidence,
    pub(super) cache_home: &'a Path,
    pub(super) query: Option<&'a str>,
    pub(super) query_clauses: &'a [String],
    pub(super) candidates: &'a [Candidate],
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
    let source_snapshot = request.source_snapshot;
    let cache_home = request.cache_home;
    let surface = request.surface;
    let query = request.query;
    let query_clauses = request.query_clauses;
    let candidates = request.candidates;
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
        for term in query_terms(query) {
            let query_id = stable_node_id("query", &term);
            if !seed_ids.contains(&query_id) {
                seed_ids.push(query_id.clone());
                query_seed_present = true;
                nodes.push(json!({
                    "id": query_id,
                    "kind": "query",
                    "role": "term",
                    "value": term,
                    "action": "lexical"
                }));
            }
        }
    }

    let augmented_candidates = candidates.to_vec();
    let query_terms = query.map(query_terms).unwrap_or_default();
    let graph_candidates = sparse_graph_candidates(&augmented_candidates, query);
    let query_adjustment_policy = query_adjustment_policy_from_env();
    let topology_membership_enabled = query_adjustment_policy
        .as_ref()
        .and_then(|policy| policy.get("topologyMembership"))
        .and_then(Value::as_bool)
        .unwrap_or(true);
    let owner_rank_report = graph_owner_rank_report_with_topology(
        &graph_candidates,
        &query_terms,
        topology_membership_enabled.then_some(dependency_root),
        source_snapshot,
    );
    let owners = owner_rank_report
        .ranked_owners
        .iter()
        .map(|owner| owner.path.clone())
        .collect::<Vec<_>>();
    let topology_submodule_count =
        agent_semantic_search::graph_project_submodule_paths(dependency_root).len();
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
    let seed_plan = agent_semantic_search::graph_turbo_seed_plan(
        agent_semantic_search::GraphTurboSeedPlanInput {
            query_present,
            query_seed_present,
            candidate_count: graph_candidates.len(),
            candidate_owner_count: owners.len(),
            query_owner_seed_count,
            fallback_owner_seed_count,
            seed_ids: &seed_ids,
            seed_decision: &seed_decision,
        },
    );
    if include_owner_context {
        nodes.extend(agent_semantic_search::owner_path_graph_nodes(&owners));
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

    let route = (surface == "search-lexical")
        .then(|| {
            graph_route_value(
                language_id,
                query,
                &query_terms,
                &owner_rank_report.query_axes,
                &owner_rank_report.ranked_owners,
                &surfaces,
            )
        })
        .flatten();
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
    if let Some(route) = route {
        packet["route"] = route;
    }
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

pub(crate) fn graph_route_next_action(language_id: &str, owner_path: &str, query: &str) -> Value {
    json!({
        "kind": "search-owner-items",
        "languageId": language_id,
        "ownerPath": owner_path,
        "query": query,
        "requiredActor": "verified-resident-search-agent",
        "requiredCapability": "owner-items",
        "executable": true,
    })
}

fn graph_route_value(
    language_id: &str,
    query: Option<&str>,
    query_terms: &[String],
    query_axes: &[String],
    ranked_owners: &[agent_semantic_search::GraphOwnerRankedOwner],
    surfaces: &[String],
) -> Option<Value> {
    let owner = ranked_owners.first()?;
    let query = graph_route_query(query, query_terms);
    if query.is_empty() {
        return None;
    }
    Some(json!({
        "kind": "graph-route",
        "version": "lexical-route-v1",
        "algorithm": "graph-owner-rank-v1",
        "relation": graph_route_relation(query_terms),
        "routeKind": graph_route_kind(surfaces),
        "queryCount": graph_route_query_count(query_terms, query_axes),
        "coveredQueryCount": owner.matched_query_axes.len(),
        "owner": {
            "path": owner.path.clone(),
            "matchedQueryAxes": owner.matched_query_axes.clone(),
            "score": graph_route_score(&owner.score),
            "localHits": owner.score.local_hits,
            "symbols": owner.symbols.iter().take(6).cloned().collect::<Vec<_>>(),
            "reachability": "unknown",
        },
        "nextAction": graph_route_next_action(language_id, &owner.path, &query),
        "avoid": ["frontier-dump", "source-body", "line-selector"],
    }))
}

fn graph_route_query_count(query_terms: &[String], query_axes: &[String]) -> usize {
    if query_axes.is_empty() {
        query_terms.len()
    } else {
        query_axes.len()
    }
}

fn graph_route_query(query: Option<&str>, query_terms: &[String]) -> String {
    let joined_terms = query_terms
        .iter()
        .map(|term| term.trim())
        .filter(|term| !term.is_empty())
        .collect::<Vec<_>>()
        .join("|");
    if !joined_terms.is_empty() {
        joined_terms
    } else {
        query.unwrap_or_default().trim().to_string()
    }
}

fn graph_route_relation(query_terms: &[String]) -> &'static str {
    if query_terms.len() > 1 {
        "cohesive"
    } else {
        "query-bundle-required"
    }
}

fn graph_route_kind(surfaces: &[String]) -> &'static str {
    if include_deps(surfaces) {
        "owner-deps"
    } else if include_items(surfaces) {
        "owner-item"
    } else if include_tests(surfaces) {
        "owner-test"
    } else {
        "owner"
    }
}

fn graph_route_score(score: &agent_semantic_search::GraphOwnerRankScore) -> Value {
    json!({
        "total": score.total,
        "queryAxisCount": score.query_axis_count,
        "packageQueryAxisCount": score.package_query_axis_count,
        "topologyQueryAxisCount": score.topology_query_axis_count,
        "localHits": score.local_hits,
        "symbolCount": score.symbol_count,
    })
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
    if include_deps(surfaces) && surface != "search-lexical" {
        return Some(context);
    }
    let search_pipe_dependency_query = surface == "search-pipe"
        && query.is_some_and(|query| {
            candidate_usage_dependency_matches_query(language_id, candidates, query)
        });
    search_pipe_dependency_query.then_some(context)
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

fn sparse_graph_candidates(candidates: &[Candidate], _query: Option<&str>) -> Vec<Candidate> {
    let sparsity_inputs = candidates
        .iter()
        .map(|candidate| {
            agent_semantic_search::GraphCandidateSparsityInput::new(
                candidate.path.clone(),
                candidate.symbol.clone(),
            )
        })
        .collect::<Vec<_>>();
    agent_semantic_search::select_sparse_graph_candidate_indices(
        &sparsity_inputs,
        GRAPH_TURBO_CANDIDATE_NODE_LIMIT,
    )
    .into_iter()
    .filter_map(|index| candidates.get(index).cloned())
    .collect()
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
    nodes.extend(agent_semantic_search::compact_provider_fact_nodes(
        &provider_facts.nodes,
    ));
    nodes.extend(agent_semantic_search::provider_candidate_annotation_nodes(
        &provider_facts.candidate_annotations,
    ));
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
    for term in query_terms(query) {
        let query_id = stable_node_id("query", &term);
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
}

fn append_query_dependency_edges(edges: &mut Vec<Value>, query: &str, facts: &[DependencyFact]) {
    for term in query_terms(query) {
        let query_id = stable_node_id("query", &term);
        for fact in facts
            .iter()
            .filter(|fact| dependency_matches_query(&fact.dependency, &term))
        {
            edges.push(edge(
                &query_id,
                &stable_node_id("dependency", &fact.dependency),
                "matches",
            ));
        }
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
