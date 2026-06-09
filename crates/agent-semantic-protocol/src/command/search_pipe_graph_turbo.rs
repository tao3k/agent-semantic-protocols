//! Graph-turbo request packets for ASP-owned fast search candidates.

use std::collections::{HashMap, HashSet};

use serde_json::{Value, json};

use super::{
    search_pipe_dependency_facts::{
        DependencyFact, collect_dependency_facts, dependency_matches_query,
    },
    search_pipe_model::{Candidate, SearchPipeSourceTrace},
    search_pipe_provider_facts::ProviderGraphFacts,
    search_pipe_quality::{compact_fact_value, is_generated_path, query_allows_generated},
    search_pipe_surfaces::{
        include_deps, include_items, include_owner_context, include_tests,
        normalized_search_surfaces,
    },
};

const GRAPH_TURBO_REQUEST_SCHEMA_ID: &str = "agent.semantic-protocols.semantic-graph-turbo-request";
const GRAPH_TURBO_CANDIDATE_NODE_LIMIT: usize = 64;
const HOT_CONTEXT_BEFORE_LINES: usize = 8;
const HOT_CONTEXT_AFTER_LINES: usize = 12;

pub(super) struct GraphTurboSearchPipeRequest<'a> {
    pub(super) surface: &'a str,
    pub(super) language_id: &'a str,
    pub(super) query: Option<&'a str>,
    pub(super) candidates: &'a [Candidate],
    pub(super) pipes: &'a [String],
    pub(super) source: &'a str,
    pub(super) candidate_sources: &'a [String],
    pub(super) source_trace: &'a [SearchPipeSourceTrace],
    pub(super) provider_facts: &'a ProviderGraphFacts,
    pub(super) read_memory_selectors: &'a [String],
}

pub(super) fn render_graph_turbo_request(
    request: GraphTurboSearchPipeRequest<'_>,
) -> Result<String, String> {
    let packet = graph_turbo_request(&request);
    serde_json::to_string_pretty(&packet)
        .map(|mut text| {
            text.push('\n');
            text
        })
        .map_err(|error| format!("failed to serialize graph turbo request: {error}"))
}

fn graph_turbo_request(request: &GraphTurboSearchPipeRequest<'_>) -> Value {
    let language_id = request.language_id;
    let surface = request.surface;
    let query = request.query;
    let candidates = request.candidates;
    let pipes = request.pipes;
    let source = request.source;
    let candidate_sources = request.candidate_sources;
    let source_trace = request.source_trace;
    let provider_facts = request.provider_facts;
    let read_memory_selectors = request.read_memory_selectors;
    let profile = profile_for_pipes(pipes);
    let surfaces = normalized_search_surfaces(pipes);
    let include_owner_context = include_owner_context(&surfaces);
    let include_items = include_items(&surfaces);
    let include_tests = include_tests(&surfaces);
    let include_deps = include_deps(&surfaces);
    let mut nodes = Vec::new();
    let mut edges = Vec::new();
    let mut seed_ids = Vec::new();
    if let Some(query) = query.filter(|query| !query.trim().is_empty()) {
        let query_id = stable_node_id("query", query);
        seed_ids.push(query_id.clone());
        nodes.push(json!({
            "id": query_id,
            "kind": "query",
            "role": "term",
            "value": query,
            "action": "fzf"
        }));
    }

    let graph_candidates = sparse_graph_candidates(candidates, query);
    let owners = unique_candidate_paths(&graph_candidates);
    if seed_ids.is_empty() {
        seed_ids.extend(
            owners
                .iter()
                .take(2)
                .map(|owner| stable_node_id("owner", owner)),
        );
    }
    if include_owner_context {
        append_owner_nodes(&mut nodes, &owners);
    }
    if include_items {
        append_candidate_nodes(&mut nodes, language_id, &graph_candidates);
        append_hot_nodes(&mut nodes, &graph_candidates);
        append_provider_fact_nodes(&mut nodes, provider_facts);
    }
    let dependency_facts = collect_dependency_facts(language_id, query, &graph_candidates);
    if include_deps {
        append_dependency_nodes(&mut nodes, &dependency_facts);
    }
    if include_tests {
        append_test_nodes(&mut nodes, &owners);
    }
    append_graph_edges(
        &mut edges,
        query,
        &graph_candidates,
        &owners,
        &dependency_facts,
        provider_facts,
        &surfaces,
    );

    let mut packet = json!({
        "schemaId": GRAPH_TURBO_REQUEST_SCHEMA_ID,
        "schemaVersion": "1",
        "protocolId": "agent.semantic-protocols.semantic-language",
        "protocolVersion": "1",
        "packetKind": "graph-turbo-request",
        "surface": surface,
        "queryTerms": query.map(query_terms).unwrap_or_default(),
        "profile": profile,
        "algorithm": "typed-ppr-diverse",
        "surfaces": surfaces,
        "source": source,
        "candidateSources": candidate_sources,
        "sourceTrace": graph_turbo_source_trace(source_trace),
        "seedIds": seed_ids,
        "budget": 10,
        "kindBudgets": {"owner": 4, "dependency": 2, "test": 3, "item": 6, "field": 4, "type": 3, "collection": 2, "hot": 3},
        "windowMerge": {"enabled": true, "maxGapLines": 8},
        "pathBudget": 5,
        "pathMaxHops": 4,
        "cache": {"enabled": true},
        "graph": {
            "nodes": nodes,
            "edges": edges,
        },
    });
    if !read_memory_selectors.is_empty() {
        packet["readMemory"] = json!({
            "seenSelectors": read_memory_selectors,
        });
    }
    packet
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

fn append_candidate_nodes(nodes: &mut Vec<Value>, language_id: &str, candidates: &[Candidate]) {
    for candidate in candidates.iter().take(GRAPH_TURBO_CANDIDATE_NODE_LIMIT) {
        nodes.push(json!({
            "id": candidate_node_id(candidate),
            "kind": "item",
            "role": "symbol",
            "value": candidate.symbol,
            "action": "syntax",
            "path": candidate.path,
            "ownerPath": candidate.path,
            "symbol": candidate.symbol,
            "startLine": candidate.line,
            "endLine": candidate.line,
            "locator": format!("{}:{}:{}", candidate.path, candidate.line, candidate.line),
            "matchText": candidate.text,
            "syntaxQuery": candidate_tree_sitter_pattern(language_id, &candidate.symbol),
            "source": candidate.source,
            "confidence": candidate.confidence,
        }));
    }
}

fn append_hot_nodes(nodes: &mut Vec<Value>, candidates: &[Candidate]) {
    for candidate in candidates.iter().take(GRAPH_TURBO_CANDIDATE_NODE_LIMIT) {
        let (start_line, end_line) = hot_context_range(candidate.line);
        let locator = format!("{}:{}:{}", candidate.path, start_line, end_line);
        nodes.push(json!({
            "id": hot_node_id(candidate),
            "kind": "hot",
            "role": "range",
            "value": candidate.symbol,
            "action": "code",
            "path": candidate.path,
            "ownerPath": candidate.path,
            "symbol": candidate.symbol,
            "startLine": start_line,
            "endLine": end_line,
            "locator": locator,
            "matchText": candidate.text,
            "source": candidate.source,
            "confidence": candidate.confidence,
        }));
    }
}

fn graph_turbo_source_trace(source_trace: &[SearchPipeSourceTrace]) -> Value {
    Value::Array(
        source_trace
            .iter()
            .map(|trace| {
                json!({
                    "source": trace.source,
                    "status": trace.status,
                    "matched": trace.matched,
                    "missing": trace.missing,
                    "normalized": trace.normalized,
                })
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

fn hot_context_range(line: usize) -> (usize, usize) {
    (
        line.saturating_sub(HOT_CONTEXT_BEFORE_LINES).max(1),
        line + HOT_CONTEXT_AFTER_LINES,
    )
}

fn append_dependency_nodes(nodes: &mut Vec<Value>, dependency_facts: &[DependencyFact]) {
    let mut seen = HashSet::new();
    for fact in dependency_facts {
        if seen.insert(fact.dependency.clone()) {
            nodes.push(json!({
                "id": stable_node_id("dependency", &fact.dependency),
                "kind": "dependency",
                "role": "pkg",
                "value": fact.dependency,
                "action": "deps",
            }));
        }
    }
}

fn candidate_tree_sitter_pattern(language_id: &str, symbol: &str) -> Option<String> {
    let escaped_symbol = symbol.replace('\\', "\\\\").replace('"', "\\\"");
    match language_id {
        "rust" => Some(format!(
            "((function_item name: (_) @function.name) (#eq? @function.name \"{escaped_symbol}\"))"
        )),
        "python" => Some(format!(
            "((function_definition name: (identifier) @function.name) (#eq? @function.name \"{escaped_symbol}\"))"
        )),
        _ => None,
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

fn append_graph_edges(
    edges: &mut Vec<Value>,
    query: Option<&str>,
    candidates: &[Candidate],
    owners: &[String],
    dependency_facts: &[DependencyFact],
    provider_facts: &ProviderGraphFacts,
    surfaces: &[String],
) {
    if let Some(query) = query.filter(|query| !query.trim().is_empty()) {
        append_query_match_edges(edges, query, candidates, owners, surfaces);
        if include_deps(surfaces) {
            append_query_dependency_edges(edges, query, dependency_facts);
        }
    }
    if include_items(surfaces) {
        append_owner_candidate_edges(edges, candidates);
        append_candidate_hot_edges(edges, candidates);
        append_provider_fact_edges(edges, provider_facts);
    }
    if include_deps(surfaces) {
        append_owner_dependency_edges(edges, dependency_facts);
    }
    if include_tests(surfaces) {
        append_test_cover_edges(edges, owners);
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

fn unique_candidate_paths(candidates: &[Candidate]) -> Vec<String> {
    let mut seen = HashSet::new();
    candidates
        .iter()
        .filter_map(|candidate| {
            let path = candidate.path.clone();
            seen.insert(path.clone()).then_some(path)
        })
        .collect()
}

fn profile_for_pipes(pipes: &[String]) -> &'static str {
    let surfaces = normalized_search_surfaces(pipes);
    if include_deps(&surfaces) {
        "query-deps"
    } else if include_tests(&surfaces) && !include_items(&surfaces) {
        "owner-tests"
    } else {
        "owner-query"
    }
}

fn candidate_node_id(candidate: &Candidate) -> String {
    stable_node_id(
        "item",
        &format!("{}:{}:{}", candidate.path, candidate.symbol, candidate.line),
    )
}

fn hot_node_id(candidate: &Candidate) -> String {
    stable_node_id(
        "hot",
        &format!("{}:{}:{}", candidate.path, candidate.symbol, candidate.line),
    )
}

fn stable_node_id(kind: &str, value: &str) -> String {
    let mut rendered = String::with_capacity(kind.len() + value.len() + 1);
    rendered.push_str(kind);
    rendered.push(':');
    for character in value.chars() {
        if character == '_' || character == '-' || character == '/' || character == '.' {
            rendered.push(character);
        } else if character.is_ascii_alphanumeric() {
            rendered.push(character.to_ascii_lowercase());
        } else {
            rendered.push('-');
        }
    }
    while rendered.ends_with('-') {
        rendered.pop();
    }
    if rendered.len() == kind.len() + 1 {
        rendered.push_str("node");
    }
    rendered
}

fn query_terms(query: &str) -> Vec<String> {
    query
        .split(|character: char| character == ',' || character == '|' || character.is_whitespace())
        .map(str::trim)
        .filter(|term| !term.is_empty())
        .map(ToOwned::to_owned)
        .fold(Vec::new(), |mut terms, term| {
            if !terms.iter().any(|seen| seen == &term) {
                terms.push(term);
            }
            terms
        })
}
