pub(crate) fn asp_query_wrapper_source_index_bridge_cold_functional_path_stays_inside_scenario_gate()
{
    super::contracts::assert_query_wrapper_source_index_bridge_benchmark_contract();
    let _ = agent_semantic_search::QueryWrapperSearchSourceIndexTrace {
        lookup: agent_semantic_search::QueryWrapperSourceIndexLookup::new(Vec::new()),
    };
}

pub(crate) fn asp_query_wrapper_render_hint_projection_cold_functional_path_stays_inside_scenario_gate()
{
    super::contracts::assert_query_wrapper_render_hint_projection_benchmark_contract();
    let trace = agent_semantic_search::QueryWrapperSearchSourceIndexTrace {
        lookup: agent_semantic_search::QueryWrapperSourceIndexLookup::new(Vec::new()),
    };
    let _ = agent_semantic_search::query_wrapper_source_index_trace_projection(&trace);
}

pub(crate) fn asp_query_wrapper_clause_normalization_cold_functional_path_stays_inside_scenario_gate()
{
    super::contracts::assert_query_wrapper_clause_normalization_benchmark_contract();
    let raw_queries = vec![String::from("source access")];
    let clauses = agent_semantic_search::query_wrapper_clauses(&raw_queries);
    let _ = agent_semantic_search::query_wrapper_unique_clause_terms(&clauses);
}

pub(crate) fn asp_search_query_budget_cold_functional_path_stays_inside_scenario_gate() {
    super::contracts::assert_search_query_budget_benchmark_contract();
    super::contracts::assert_search_query_budget_benchmark_contract();
    let _ = agent_semantic_search::search_query_budget_block(["source", "access"]);
}
