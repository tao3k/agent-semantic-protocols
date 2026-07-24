//! ASP downstream policy crate for Rust project harness evidence graphs.

pub use rust_lang_project_harness::{
    RustHarnessConfig, RustOwnerResponsibility, RustProjectHarnessDownstreamPolicy,
    RustVerificationProfileHint, RustVerificationStabilityPictureConfig, RustVerificationTaskKind,
    assert_rust_project_harness_clean_with_config,
    assert_rust_project_harness_downstream_policy_from_env,
    assert_rust_project_harness_verification_from_env_with_config, default_rust_harness_config,
    rust_harness_config_for_project,
};

pub mod build_gate;
pub use build_gate::assert_asp_rust_project_harness_member_policy_from_env;
pub mod evidence;
pub mod member_policy;
pub mod package_evidence_graph;
pub mod scenario;
pub mod search_scenarios;
pub mod workspace_evidence_graph;

pub use member_policy::{
    AspRustProjectHarnessMemberPolicy, AspRustProjectHarnessOwnerPolicy,
    asp_workspace_member_policies,
};
pub use scenario::{
    AspRustProjectHarnessScenario, AspRustProjectHarnessScenarioCommand,
    AspRustProjectHarnessScenarioPackage,
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

pub use evidence::{
    AspRustProjectHarnessEvidenceGraphInput, AspRustProjectHarnessEvidenceGraphSummary,
    summarize_client_db_evidence_graph,
};
pub use package_evidence_graph::{
    AspRustProjectHarnessPackageEvidenceGraphReceipt,
    AspRustProjectHarnessPackageEvidenceGraphRequest, build_package_evidence_graph_receipt,
};
