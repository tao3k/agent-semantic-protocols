// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! ASP downstream policy crate for Rust project harness build ownership.

#[cfg(feature = "workspace-policy")]
pub use asp_rust;

pub mod build_gate;
mod canonical_replacement_search_scenario;
mod mapped_topology_owner_membership_scenario;
mod owner_content_mutation_search_scenario;
mod topology_index_search_scenario;
pub use build_gate::AspRustProjectHarnessMemberPolicyReceipt;
pub use build_gate::assert_asp_rust_project_harness_member_policy;
pub use build_gate::assert_asp_rust_project_harness_member_policy_from_env;
/// Reusable hook scenarios for Rust project harness policy checks.
pub mod member_policy;
pub mod search_scenarios;
#[cfg(feature = "workspace-policy")]
pub mod workspace_policy;

pub use asp_rust_build_support::AspRustScenario as AspRustProjectHarnessScenario;
pub use asp_rust_build_support::AspRustScenarioBenchmarkSpec as AspRustProjectHarnessScenarioBenchmark;
pub use asp_rust_build_support::AspRustScenarioCommand as AspRustProjectHarnessScenarioCommand;
pub use asp_rust_build_support::AspRustScenarioMeasurement as AspRustProjectHarnessScenarioMeasurement;
pub use asp_rust_build_support::AspRustScenarioMetricKind as AspRustProjectHarnessScenarioMetricKind;
pub use asp_rust_build_support::AspRustScenarioMetricSpec as AspRustProjectHarnessScenarioMetric;
pub use asp_rust_build_support::AspRustScenarioObservation as AspRustProjectHarnessScenarioObservation;
pub use asp_rust_build_support::AspRustScenarioPackage as AspRustProjectHarnessScenarioPackage;
pub use asp_rust_build_support::asp_rust_scenario as asp_rust_project_harness_scenario;
pub use asp_rust_build_support::asp_rust_scenario_package as asp_rust_project_harness_scenario_package;
pub use asp_rust_build_support::measure_asp_rust_scenario;
pub use asp_rust_build_support::render_asp_rust_scenario_benchmark_toml;
pub use asp_rust_build_support::write_asp_rust_scenario_benchmark_toml;
pub use mapped_topology_owner_membership_scenario::MAPPED_TOPOLOGY_OWNER_MEMBERSHIP_SCENARIO_ID;
pub use member_policy::AspRustProjectHarnessMemberPolicy;
pub use member_policy::AspRustProjectHarnessOwnerPolicy;
pub use member_policy::asp_workspace_member_forbidden_normal_dependencies;
pub use member_policy::asp_workspace_member_policies;
pub use search_scenarios::ASP_SEARCH_SCENARIO_PACKAGE_NAME;
pub use search_scenarios::CANDIDATE_TOPOLOGY_OWNER_SCOPE_SCENARIO_ID;
pub use search_scenarios::FIRST_CALL_SINGLE_FLIGHT_TERMINAL_SCENARIO_ID;
pub use search_scenarios::LEXICAL_SEARCH_FRAME_GRAPH_ROUTER_WARM_PATH_SCENARIO_ID;
pub use search_scenarios::PARSER_ARTIFACT_CONTENT_REUSE_SCENARIO_ID;
pub use search_scenarios::SEARCH_GRAPH_ROUTER_NEXT_EXACT_ACTION_SCENARIO_ID;
pub use search_scenarios::SEARCH_PACKAGE_LINEAR_PERFORMANCE_SCENARIO_ID;
pub use search_scenarios::SEARCH_SOURCE_INDEX_OWNER_ITEM_GRAPH_CHAIN_SCENARIO_ID;
pub use search_scenarios::SEARCH_SUBAGENT_COMPACT_RECEIPT_SCENARIO_ID;
pub use search_scenarios::asp_search_scenario_package;

#[cfg(feature = "workspace-policy")]
pub use workspace_policy::assert_asp_workspace_build_identity_from_env;
#[cfg(feature = "workspace-policy")]
pub use workspace_policy::assert_asp_workspace_policy;
#[cfg(feature = "workspace-policy")]
pub use workspace_policy::assert_asp_workspace_policy_from_env;
#[cfg(feature = "workspace-policy")]
pub use workspace_policy::{
    assert_asp_rust_project_harness_member_source_policy_from_env,
    evaluate_asp_rust_project_harness_member_source_policy,
};
