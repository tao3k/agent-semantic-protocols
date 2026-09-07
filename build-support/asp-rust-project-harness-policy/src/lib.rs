// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! ASP downstream policy crate for Rust project harness evidence graphs.

#[cfg(feature = "workspace-policy")]
pub use asp_rust;

pub mod build_gate;
pub use build_gate::AspRustProjectHarnessMemberPolicyReceipt;
pub use build_gate::assert_asp_rust_project_harness_member_policy;
pub use build_gate::assert_asp_rust_project_harness_member_policy_from_env;
pub mod evidence;
/// Reusable hook scenarios for Rust project harness policy checks.
pub mod member_policy;
pub mod package_evidence_graph;
pub mod search_scenarios;
pub mod workspace_evidence_graph;
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
pub use member_policy::AspRustProjectHarnessMemberPolicy;
pub use member_policy::AspRustProjectHarnessOwnerPolicy;
pub use member_policy::asp_workspace_member_forbidden_normal_dependencies;
pub use member_policy::asp_workspace_member_policies;
pub use search_scenarios::ASP_SEARCH_SCENARIO_PACKAGE_NAME;
pub use search_scenarios::LEXICAL_SEARCH_FRAME_GRAPH_ROUTER_WARM_PATH_SCENARIO_ID;
pub use search_scenarios::SEARCH_GRAPH_ROUTER_NEXT_EXACT_ACTION_SCENARIO_ID;
pub use search_scenarios::SEARCH_PACKAGE_LINEAR_PERFORMANCE_SCENARIO_ID;
pub use search_scenarios::SEARCH_SOURCE_INDEX_OWNER_ITEM_GRAPH_CHAIN_SCENARIO_ID;
pub use search_scenarios::SEARCH_SUBAGENT_COMPACT_RECEIPT_SCENARIO_ID;
pub use search_scenarios::asp_search_scenario_package;

pub use workspace_evidence_graph::AspRustProjectHarnessWorkspaceEvidenceGraphEdgeKind;
pub use workspace_evidence_graph::AspRustProjectHarnessWorkspaceEvidenceGraphEdgeReceipt;
pub use workspace_evidence_graph::AspRustProjectHarnessWorkspaceEvidenceGraphNodeKind;
pub use workspace_evidence_graph::AspRustProjectHarnessWorkspaceEvidenceGraphNodeReceipt;
pub use workspace_evidence_graph::AspRustProjectHarnessWorkspaceEvidenceGraphReceipt;
pub use workspace_evidence_graph::AspRustProjectHarnessWorkspaceEvidenceGraphRequest;
pub use workspace_evidence_graph::AspRustProjectHarnessWorkspaceEvidenceGraphSummaryReceipt;
pub use workspace_evidence_graph::build_asp_workspace_evidence_graph_receipt;
pub use workspace_evidence_graph::build_workspace_evidence_graph_receipt;
#[cfg(feature = "workspace-policy")]
pub use workspace_policy::assert_asp_workspace_build_identity_from_env;
#[cfg(feature = "workspace-policy")]
pub use workspace_policy::assert_asp_workspace_policy;
#[cfg(feature = "workspace-policy")]
pub use workspace_policy::assert_asp_workspace_policy_from_env;

pub use evidence::AspRustProjectHarnessEvidenceGraphInput;
pub use evidence::AspRustProjectHarnessEvidenceGraphSummary;
pub use evidence::summarize_client_db_evidence_graph;
pub use package_evidence_graph::AspRustProjectHarnessPackageEvidenceGraphReceipt;
pub use package_evidence_graph::AspRustProjectHarnessPackageEvidenceGraphRequest;
pub use package_evidence_graph::build_package_evidence_graph_receipt;
