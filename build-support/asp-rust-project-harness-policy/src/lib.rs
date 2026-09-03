//! ASP downstream policy crate for Rust project harness evidence graphs.

#[cfg(feature = "workspace-policy")]
pub use asp_rust;

pub mod build_gate;
pub use build_gate::{
    AspRustProjectHarnessMemberPolicyReceipt, validate_asp_rust_project_harness_member_manifest,
};
pub mod evidence;
/// Reusable hook scenarios for Rust project harness policy checks.
pub mod member_policy;
pub mod package_evidence_graph;
pub mod search_scenarios;
pub mod workspace_evidence_graph;
#[cfg(feature = "workspace-policy")]
pub mod workspace_policy;

pub use asp_rust_build_support::{
    AspRustScenario as AspRustProjectHarnessScenario,
    AspRustScenarioBenchmarkSpec as AspRustProjectHarnessScenarioBenchmark,
    AspRustScenarioCommand as AspRustProjectHarnessScenarioCommand,
    AspRustScenarioMeasurement as AspRustProjectHarnessScenarioMeasurement,
    AspRustScenarioMetricKind as AspRustProjectHarnessScenarioMetricKind,
    AspRustScenarioMetricSpec as AspRustProjectHarnessScenarioMetric,
    AspRustScenarioObservation as AspRustProjectHarnessScenarioObservation,
    AspRustScenarioPackage as AspRustProjectHarnessScenarioPackage,
    asp_rust_scenario as asp_rust_project_harness_scenario,
    asp_rust_scenario_package as asp_rust_project_harness_scenario_package,
    measure_asp_rust_scenario, render_asp_rust_scenario_benchmark_toml,
    write_asp_rust_scenario_benchmark_toml,
};
pub use member_policy::{
    AspRustProjectHarnessMemberPolicy, AspRustProjectHarnessOwnerPolicy,
    asp_workspace_member_forbidden_normal_dependencies, asp_workspace_member_policies,
};
pub use search_scenarios::{
    ASP_SEARCH_SCENARIO_PACKAGE_NAME, LEXICAL_SEARCH_FRAME_GRAPH_ROUTER_WARM_PATH_SCENARIO_ID,
    SEARCH_GRAPH_ROUTER_NEXT_EXACT_ACTION_SCENARIO_ID,
    SEARCH_PACKAGE_LINEAR_PERFORMANCE_SCENARIO_ID,
    SEARCH_SOURCE_INDEX_OWNER_ITEM_GRAPH_CHAIN_SCENARIO_ID,
    SEARCH_SUBAGENT_COMPACT_RECEIPT_SCENARIO_ID, asp_search_scenario_package,
};

pub use workspace_evidence_graph::{
    AspRustProjectHarnessWorkspaceEvidenceGraphEdgeKind,
    AspRustProjectHarnessWorkspaceEvidenceGraphEdgeReceipt,
    AspRustProjectHarnessWorkspaceEvidenceGraphNodeKind,
    AspRustProjectHarnessWorkspaceEvidenceGraphNodeReceipt,
    AspRustProjectHarnessWorkspaceEvidenceGraphReceipt,
    AspRustProjectHarnessWorkspaceEvidenceGraphRequest,
    AspRustProjectHarnessWorkspaceEvidenceGraphSummaryReceipt,
    build_asp_workspace_evidence_graph_receipt, build_workspace_evidence_graph_receipt,
};
#[cfg(feature = "workspace-policy")]
pub use workspace_policy::{
    assert_asp_workspace_build_identity_from_env, assert_asp_workspace_policy,
    assert_asp_workspace_policy_from_env,
};

pub use evidence::{
    AspRustProjectHarnessEvidenceGraphInput, AspRustProjectHarnessEvidenceGraphSummary,
    summarize_client_db_evidence_graph,
};
pub use package_evidence_graph::{
    AspRustProjectHarnessPackageEvidenceGraphReceipt,
    AspRustProjectHarnessPackageEvidenceGraphRequest, build_package_evidence_graph_receipt,
};
