//! Typed Graph Turbo intent generation.  Graph content is resident-only.

use serde_json::{Value, json};

use super::{
    search_pipe_model::{Candidate, SearchPipeSourceTrace},
    search_pipe_surfaces::normalized_search_surfaces,
};

const GRAPH_TURBO_REQUEST_SCHEMA_ID: &str = "agent.semantic-protocols.semantic-graph-turbo-request";
const GRAPH_TURBO_SOURCE_LIMIT: usize = 64;

pub(super) struct GraphTurboSearchPipeRequest<'a> {
    pub(super) surface: &'a str,
    pub(super) generation:
        &'a agent_semantic_search::graph_generation_authority::AdmittedGraphGenerationV1<'a>,
    pub(super) query: Option<&'a str>,
    pub(super) query_clauses: &'a [String],
    pub(super) candidates: &'a [Candidate],
    pub(super) pipes: &'a [String],
    pub(super) source: &'a str,
    pub(super) candidate_sources: &'a [String],
    pub(super) source_trace: &'a [SearchPipeSourceTrace],
    pub(super) read_memory_selectors: &'a [String],
    pub(super) action_frontier: &'a [Value],
}

pub(super) async fn render_graph_turbo_request(
    request: GraphTurboSearchPipeRequest<'_>,
) -> Result<String, String> {
    let packet = resident_graph_turbo_intent(&request)?;
    serde_json::to_string(&packet)
        .map(|mut text| {
            text.push('\n');
            text
        })
        .map_err(|error| format!("failed to serialize Graph Turbo intent: {error}"))
}

/// The only client-side Graph Turbo packet.  It has ranking controls and
/// typed roots, never a materialized graph or provider facts.
pub(super) fn resident_graph_turbo_intent(
    request: &GraphTurboSearchPipeRequest<'_>,
) -> Result<Value, String> {
    let graph_sources = graph_fact_sources(request.candidates);
    if graph_sources.is_empty() {
        return Err("resident Graph Turbo requires at least one typed candidate source".to_owned());
    }
    let terms = request.query.map(query_terms).unwrap_or_default();
    let surfaces = normalized_search_surfaces(request.pipes);
    let mut packet = json!({
        "schemaId": GRAPH_TURBO_REQUEST_SCHEMA_ID,
        "schemaVersion": "1",
        "protocolId": "agent.semantic-protocols.semantic-language",
        "protocolVersion": "1",
        "packetKind": "graph-turbo-request",
        "surface": request.surface,
        "queryTerms": terms,
        "profile": profile_for_surfaces(&surfaces),
        "algorithm": "typed-ppr-diverse",
        "surfaces": surfaces,
        "source": request.source,
        "sourceSnapshot": request.generation.source_snapshot(),
        "candidateSources": request.candidate_sources,
        "graphSources": graph_sources,
        "sourceTrace": graph_turbo_source_trace(request.source_trace),
        "budget": 10,
        "kindBudgets": {"owner": 4, "workspace": 1, "provider-root": 2, "submodule": 4, "dependency": 2, "test": 3, "item": 6, "field": 4, "type": 3, "collection": 2, "hot": 3},
        "windowMerge": {"enabled": true, "maxGapLines": 8},
        "pathBudget": 5,
        "pathMaxHops": 4,
        "cache": {"enabled": true, "generationResident": true},
    });
    if !request.query_clauses.is_empty() {
        packet["queryClauses"] = json!(request.query_clauses);
    }
    if !request.read_memory_selectors.is_empty() {
        packet["readMemory"] = json!({"seenSelectors": request.read_memory_selectors});
    }
    if !request.action_frontier.is_empty() {
        packet["actionFrontier"] = Value::Array(request.action_frontier.to_vec());
    }
    Ok(packet)
}

fn graph_fact_sources(candidates: &[Candidate]) -> Vec<Value> {
    let mut sources = std::collections::BTreeSet::new();
    for candidate in candidates.iter().take(GRAPH_TURBO_SOURCE_LIMIT) {
        sources.insert(("owner".to_owned(), candidate.path.clone()));
        if let Some(selector) = candidate.selector.as_deref() {
            sources.insert(("item".to_owned(), selector.to_owned()));
        }
    }
    sources
        .into_iter()
        .map(|(kind, id)| json!({"kind": kind, "id": id}))
        .collect()
}

fn query_terms(query: &str) -> Vec<String> {
    query
        .split_whitespace()
        .map(str::to_owned)
        .filter(|term| !term.is_empty())
        .collect()
}

fn profile_for_surfaces(surfaces: &[String]) -> &'static str {
    if surfaces.iter().any(|surface| surface == "deps") {
        "owner-deps"
    } else if surfaces.iter().any(|surface| surface == "tests") {
        "owner-tests"
    } else {
        "owner-query"
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
                    "fields": trace.fields,
                })
            })
            .collect(),
    )
}
