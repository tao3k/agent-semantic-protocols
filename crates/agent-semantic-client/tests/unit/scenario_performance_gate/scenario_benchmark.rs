#[test]
fn asp_unit_scenarios_have_rust_harness_benchmark_toml_gates() {
    super::scenario_benchmark_manifest::asp_unit_scenarios_have_rust_harness_benchmark_toml_gates();
}

#[test]
fn asp_unit_scenarios_cover_perf_sensitive_subcommands() {
    super::scenario_benchmark_manifest::asp_unit_scenarios_cover_perf_sensitive_subcommands();
}

#[test]
fn asp_evidence_graph_rank_cold_functional_path_stays_inside_scenario_gate() {
    super::graph_rank::asp_evidence_graph_rank_cold_functional_path_stays_inside_scenario_gate();
}

#[test]
fn asp_search_candidate_contract_cold_functional_path_stays_inside_scenario_gate() {
    super::search_candidate_contract::asp_search_candidate_contract_cold_functional_path_stays_inside_scenario_gate();
}

#[test]
fn asp_provider_candidate_annotations_cold_functional_path_stays_inside_scenario_gate() {
    super::overlay_and_provider_gates::asp_provider_candidate_annotations_cold_functional_path_stays_inside_scenario_gate();
}

#[test]
fn asp_graph_node_projection_cold_functional_path_stays_inside_scenario_gate() {
    super::graph::asp_graph_node_projection_cold_functional_path_stays_inside_scenario_gate();
}

#[test]
fn asp_graph_candidate_projection_cold_functional_path_stays_inside_scenario_gate() {
    super::graph::asp_graph_candidate_projection_cold_functional_path_stays_inside_scenario_gate();
}

#[test]
fn asp_graph_topology_projection_cold_functional_path_stays_inside_scenario_gate() {
    super::graph::asp_graph_topology_projection_cold_functional_path_stays_inside_scenario_gate();
}

#[test]
fn asp_graph_owner_rank_cold_functional_path_stays_inside_scenario_gate() {
    super::graph::asp_graph_owner_rank_cold_functional_path_stays_inside_scenario_gate();
}

#[test]
fn asp_graph_evidence_projection_cold_functional_path_stays_inside_scenario_gate() {
    super::graph::asp_graph_evidence_projection_cold_functional_path_stays_inside_scenario_gate();
}

#[test]
fn asp_runtime_timeout_policy_cold_functional_path_stays_inside_scenario_gate() {
    super::runtime_gates::asp_runtime_timeout_policy_cold_functional_path_stays_inside_scenario_gate();
}

#[cfg(unix)]
#[tokio::test]
async fn asp_provider_process_orphan_descendant_closure_stays_inside_scenario_gate() {
    super::runtime_gates::asp_provider_process_orphan_descendant_closure_stays_inside_scenario_gate(
    )
    .await;
}

#[test]
fn asp_provider_projection_batch_workspace_pressure_stays_inside_scenario_gate() {
    super::runtime_gates::asp_provider_projection_batch_workspace_pressure_stays_inside_scenario_gate();
}

#[test]
fn scenario_benchmark_duration_contract_rejects_zero_budget() {
    super::runtime_gates::scenario_benchmark_duration_contract_rejects_zero_budget();
}

#[test]
fn asp_turso_db_engine_concurrent_process_pressure_stays_inside_scenario_gate() {
    super::scenario_performance_gate_impl::asp_turso_db_engine_concurrent_process_pressure_stays_inside_scenario_gate();
}

#[test]
fn asp_turso_agent_session_registry_shared_route_pressure_stays_inside_scenario_gate() {
    super::scenario_performance_gate_impl::asp_turso_agent_session_registry_shared_route_pressure_stays_inside_scenario_gate();
}

#[test]
fn asp_codex_rollout_session_index_algorithm_pressure_stays_inside_scenario_gate() {
    super::scenario_performance_gate_impl::asp_codex_rollout_session_index_algorithm_pressure_stays_inside_scenario_gate();
}

#[test]
fn asp_turso_source_index_refresh_lookup_pressure_stays_inside_scenario_gate() {
    super::scenario_performance_gate_impl::asp_turso_source_index_refresh_lookup_pressure_stays_inside_scenario_gate();
}

#[test]
fn asp_unit_scenarios_cover_workspace_argument_guards() {
    super::scenario_policy_scan::asp_unit_scenarios_cover_workspace_argument_guards();
}

#[test]
fn language_harnesses_have_shared_scenario_benchmark_schema_coverage() {
    super::sandtable_gates::language_harnesses_have_shared_scenario_benchmark_schema_coverage();
}
