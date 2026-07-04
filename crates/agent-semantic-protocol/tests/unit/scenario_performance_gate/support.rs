use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::env;
use std::fs;
use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Instant;

use crate::provider_command::support::{
    asp_command, prepend_path, provider, provider_with_owner_items, temp_project_root,
    write_activation, write_marker_provider, write_provider_bin_config,
};
use agent_semantic_client_db::{
    ClientDbSourceIndexCandidate, ClientDbSourceIndexLookupResult, ClientDbSourceIndexLookupState,
    ClientDbSourceIndexSourceKind,
};
use agent_semantic_protocol::render_selector_seeded_search_pipe;
use agent_semantic_search::render_owner_items_source_index_lookup_trace;
use serde::Deserialize;
use serde_json::Value;

mod owner_items;

pub(super) use owner_items::{
    asp_org_owner_items_cold_functional_path_stays_inside_scenario_gate,
    asp_python_owner_items_cache_hot_path_stays_inside_scenario_gate,
    asp_rust_owner_items_cache_hot_path_stays_inside_scenario_gate,
    asp_typescript_owner_items_cache_hot_path_stays_inside_scenario_gate,
};

const AGENT_POLICY_ID_GRAMMAR: &str = "<LANGUAGE>-AGENT-<TAGS>-<NUMBER>";
const LARGE_LIBRARY_STEP_MAX_ELAPSED_MS: u64 = 300;
const JULIA_LARGE_LIBRARY_STEP_MAX_ELAPSED_MS: u64 = 5_000;
const JULIA_DATAFRAMES_BATCH_STEP_MAX_ELAPSED_MS: u64 = 75;
const JULIA_DATAFRAMES_BATCH_SAMPLE_COUNT: usize = 3;
const REQUIRED_PERFORMANCE_SUBCOMMAND_POLICY_IDS: &[&str] = &[
    "RUST-AGENT-ASP-PERF-SUBCOMMAND-QUERY-SELECTOR-001",
    "RUST-AGENT-ASP-PERF-SUBCOMMAND-QUERY-TREESITTER-001",
    "RUST-AGENT-ASP-PERF-SUBCOMMAND-SEARCH-DEPS-001",
    "RUST-AGENT-ASP-PERF-SUBCOMMAND-SEARCH-FD-001",
    "RUST-AGENT-ASP-PERF-SUBCOMMAND-SEARCH-FD-SOURCE-INDEX-001",
    "RUST-AGENT-ASP-PERF-SUBCOMMAND-SEARCH-FZF-001",
    "RUST-AGENT-ASP-PERF-SUBCOMMAND-SEARCH-FZF-SOURCE-INDEX-001",
    "RUST-AGENT-ASP-PERF-SUBCOMMAND-SEARCH-OWNER-001",
    "RUST-AGENT-ASP-FUNCTIONAL-SUBCOMMAND-SEARCH-OWNER-SOURCE-INDEX-MISSING-DB-COLD-001",
    "RUST-AGENT-ASP-FUNCTIONAL-SUBCOMMAND-SEARCH-OWNER-SOURCE-INDEX-HIT-COLD-001",
    "RUST-AGENT-ASP-PERF-SUBCOMMAND-SEARCH-PIPE-001",
    "RUST-AGENT-ASP-PERF-SUBCOMMAND-SEARCH-PIPE-DYNAMIC-OVERLAY-001",
    "RUST-AGENT-ASP-PERF-SUBCOMMAND-SEARCH-PIPE-SELECTOR-SEED-001",
    "RUST-AGENT-ASP-PERF-SUBCOMMAND-SEARCH-PIPE-SOURCE-INDEX-001",
    "RUST-AGENT-ASP-PERF-SUBCOMMAND-SEARCH-RG-001",
    "RUST-AGENT-ASP-PERF-SUBCOMMAND-SEARCH-RG-SOURCE-INDEX-001",
    "RUST-AGENT-ASP-PERF-SUBCOMMAND-SEARCH-RG-SOURCE-INDEX-MISS-001",
    "RUST-AGENT-ASP-PERF-SUBCOMMAND-SOURCE-INDEX-001",
    "RUST-AGENT-ASP-PERF-SUBCOMMAND-PROVIDER-FACTS-001",
    "RUST-AGENT-ASP-FUNCTIONAL-SUBCOMMAND-SOURCE-INDEX-LOOKUP-COLD-001",
    "RUST-AGENT-ASP-FUNCTIONAL-GRAPH-ROUTE-EVIDENCE-RANK-COLD-001",
    "GERBIL-SCHEME-AGENT-ASP-FUNCTIONAL-SUBCOMMAND-SEARCH-OWNER-COLD-001",
    "JULIA-AGENT-ASP-FUNCTIONAL-SUBCOMMAND-SEARCH-OWNER-COLD-001",
    "ORG-AGENT-ASP-FUNCTIONAL-SUBCOMMAND-SEARCH-OWNER-COLD-001",
    "PYTHON-AGENT-ASP-FUNCTIONAL-SUBCOMMAND-SEARCH-OWNER-COLD-001",
    "PYTHON-AGENT-ASP-PERF-SUBCOMMAND-SEARCH-OWNER-001",
    "RUST-AGENT-ASP-FUNCTIONAL-SUBCOMMAND-PROVIDER-CANDIDATE-ANNOTATIONS-COLD-001",
    "RUST-AGENT-ASP-FUNCTIONAL-SUBCOMMAND-GRAPH-CANDIDATE-PROJECTION-COLD-001",
    "RUST-AGENT-ASP-FUNCTIONAL-SUBCOMMAND-GRAPH-EVIDENCE-PROJECTION-COLD-001",
    "RUST-AGENT-ASP-FUNCTIONAL-SUBCOMMAND-GRAPH-NODE-PROJECTION-COLD-001",
    "RUST-AGENT-ASP-FUNCTIONAL-SUBCOMMAND-GRAPH-OWNER-RANK-COLD-001",
    "RUST-AGENT-ASP-FUNCTIONAL-SUBCOMMAND-GRAPH-QUERY-OWNER-SEED-COLD-001",
    "RUST-AGENT-ASP-FUNCTIONAL-SUBCOMMAND-GRAPH-SEED-DECISION-COLD-001",
    "RUST-AGENT-ASP-FUNCTIONAL-SUBCOMMAND-GRAPH-TOPOLOGY-PROJECTION-COLD-001",
    "RUST-AGENT-ASP-FUNCTIONAL-SUBCOMMAND-SEARCH-OWNER-COLD-001",
    "RUST-AGENT-ASP-FUNCTIONAL-SUBCOMMAND-SEARCH-PIPE-PACKAGE-COHESION-COLD-001",
    "RUST-AGENT-ASP-FUNCTIONAL-SUBCOMMAND-SEARCH-PIPE-QUERY-PACK-COLD-001",
    "RUST-AGENT-ASP-FUNCTIONAL-SUBCOMMAND-SEARCH-PIPE-QUALITY-DECISION-COLD-001",
    "RUST-AGENT-ASP-FUNCTIONAL-SUBCOMMAND-SEARCH-PIPE-EVIDENCE-CLASSIFIER-COLD-001",
    "RUST-AGENT-ASP-FUNCTIONAL-SUBCOMMAND-SEARCH-PIPE-GENERATED-CANDIDATE-COLD-001",
    "TYPESCRIPT-AGENT-ASP-FUNCTIONAL-SUBCOMMAND-SEARCH-OWNER-COLD-001",
    "TYPESCRIPT-AGENT-ASP-PERF-SUBCOMMAND-SEARCH-OWNER-001",
];
const REQUIRED_WORKSPACE_ARGUMENT_POLICY_IDS: &[&str] = &["RUST-AGENT-ASP-WORKSPACE-FILE-001"];
const SHARED_SCENARIO_BENCHMARK_SCHEMA: &str = "schemas/semantic-scenario-benchmark.v1.schema.json";
const SHARED_AGENT_POLICY_ID_SCHEMA: &str = "schemas/semantic-agent-policy-id.v1.schema.json";
const LANGUAGE_SCENARIO_BENCHMARK_REQUIREMENTS: &[LanguageScenarioBenchmarkRequirement] = &[
    LanguageScenarioBenchmarkRequirement {
        language: "rust",
        root: "languages/rust-lang-project-harness/tests/unit/scenarios",
        syntax: ScenarioBenchmarkSyntax::TomlPair,
    },
    LanguageScenarioBenchmarkRequirement {
        language: "typescript",
        root: "languages/typescript-lang-project-harness/tests/unit/scenarios/software_criteria",
        syntax: ScenarioBenchmarkSyntax::TomlPair,
    },
    LanguageScenarioBenchmarkRequirement {
        language: "python",
        root: "languages/python-lang-project-harness/tests/unit/harness/scenarios/software_criteria",
        syntax: ScenarioBenchmarkSyntax::TomlPair,
    },
    LanguageScenarioBenchmarkRequirement {
        language: "julia",
        root: "languages/JuliaLangProjectHarness.jl/test/unit/scenarios/software_criteria",
        syntax: ScenarioBenchmarkSyntax::TomlPair,
    },
    LanguageScenarioBenchmarkRequirement {
        language: "gerbil-scheme",
        root: "languages/gerbil-scheme-language-project-harness/t/scenarios/policy",
        syntax: ScenarioBenchmarkSyntax::GerbilBenchmarkSs,
    },
    LanguageScenarioBenchmarkRequirement {
        language: "orgize",
        root: "languages/orgize/tests/unit/scenarios",
        syntax: ScenarioBenchmarkSyntax::TomlPair,
    },
];

const COLD_FIRST_SEARCH_LANGUAGE_IDS: &[&str] =
    &["rust", "typescript", "python", "julia", "orgize"];

fn refresh_source_index(root: &Path) {
    let output = asp_command(root)
        .args(["cache", "source-index", "refresh"])
        .output()
        .expect("run asp cache source-index refresh");
    assert!(
        output.status.success(),
        "source-index refresh failed\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn assert_source_index_query_benchmark_contract(benchmark: &SharedBenchmarkToml) {
    assert_eq!(benchmark.route_source.as_deref(), Some("source-index"));
    assert_eq!(benchmark.max_provider_process_count, Some(0));
    assert_eq!(benchmark.max_stdout_bytes, Some(8192));
    assert_eq!(benchmark.fallback_reason.as_deref(), Some("none"));
}

fn assert_codebase_memory_mcp_reference_benchmark_contract(
    benchmark: &SearchFrameReferenceBenchmarkToml,
) {
    let codebase_rounds = benchmark
        .codebase_memory_mcp_query_rounds
        .expect("benchmark must record codebase-memory-mcp reference rounds");
    let asp_rounds = benchmark
        .asp_search_query_rounds
        .expect("benchmark must record ASP search/query rounds");
    let round_delta = benchmark
        .round_delta
        .expect("benchmark must record roundDelta");
    let selector_evidence = benchmark
        .selector_evidence
        .as_deref()
        .expect("benchmark must record selectorEvidence");
    let evidence_advantage = benchmark
        .evidence_advantage
        .as_deref()
        .expect("benchmark must record evidenceAdvantage");

    assert!(codebase_rounds > 0, "reference rounds must be non-zero");
    assert!(asp_rounds > 0, "ASP rounds must be non-zero");
    assert_eq!(
        round_delta,
        asp_rounds as i32 - codebase_rounds as i32,
        "roundDelta must equal aspSearchQueryRounds - codebaseMemoryMcpQueryRounds"
    );
    assert!(
        !selector_evidence.trim().is_empty(),
        "selectorEvidence must name the parser-owned or selector-bound evidence"
    );
    assert!(
        !evidence_advantage.trim().is_empty(),
        "evidenceAdvantage must explain the comparison value"
    );
}

fn assert_dynamic_overlay_benchmark_contract(benchmark: &SharedBenchmarkToml) {
    assert_eq!(benchmark.route_source.as_deref(), Some("dynamic-overlay"));
    assert_eq!(benchmark.max_provider_process_count, Some(0));
    assert_eq!(benchmark.max_stdout_bytes, Some(8192));
    assert_eq!(benchmark.fallback_reason.as_deref(), Some("none"));
}

fn assert_turso_overlay_benchmark_contract(benchmark: &SharedBenchmarkToml) {
    assert_eq!(benchmark.route_source.as_deref(), Some("turso-overlay"));
    assert_eq!(benchmark.max_provider_process_count, Some(0));
    assert_eq!(benchmark.max_stdout_bytes, Some(4096));
    assert_eq!(benchmark.fallback_reason.as_deref(), Some("none"));
}

fn assert_selector_seed_benchmark_contract(benchmark: &SharedBenchmarkToml) {
    assert_eq!(benchmark.route_source.as_deref(), Some("selector-seed"));
    assert_eq!(benchmark.max_provider_process_count, Some(0));
    assert_eq!(benchmark.max_stdout_bytes, Some(4096));
    assert_eq!(benchmark.fallback_reason.as_deref(), Some("none"));
}

fn assert_evidence_graph_rank_benchmark_contract(benchmark: &SharedBenchmarkToml) {
    assert_eq!(
        benchmark.route_source.as_deref(),
        Some("evidence-graph-rank")
    );
    assert_eq!(benchmark.max_provider_process_count, Some(0));
    assert_eq!(benchmark.max_stdout_bytes, Some(4096));
    assert_eq!(benchmark.fallback_reason.as_deref(), Some("none"));
}

fn assert_search_candidate_contract_benchmark_contract(benchmark: &SharedBenchmarkToml) {
    assert_eq!(
        benchmark.route_source.as_deref(),
        Some("search-candidate-contract")
    );
    assert_eq!(benchmark.max_provider_process_count, Some(0));
    assert_eq!(benchmark.max_stdout_bytes, Some(4096));
    assert_eq!(benchmark.fallback_reason.as_deref(), Some("none"));
}

fn assert_query_wrapper_source_index_bridge_benchmark_contract(benchmark: &SharedBenchmarkToml) {
    assert_eq!(
        benchmark.route_source.as_deref(),
        Some("query-wrapper-source-index-bridge")
    );
    assert_eq!(benchmark.max_provider_process_count, Some(0));
    assert_eq!(benchmark.max_stdout_bytes, Some(4096));
    assert_eq!(benchmark.fallback_reason.as_deref(), Some("none"));
}

fn assert_query_wrapper_render_hint_projection_benchmark_contract(benchmark: &SharedBenchmarkToml) {
    assert_eq!(
        benchmark.route_source.as_deref(),
        Some("query-wrapper-render-hint-projection")
    );
    assert_eq!(benchmark.max_provider_process_count, Some(0));
    assert_eq!(benchmark.max_stdout_bytes, Some(4096));
    assert_eq!(benchmark.fallback_reason.as_deref(), Some("none"));
}

fn assert_query_wrapper_source_index_trace_projection_benchmark_contract(
    benchmark: &SharedBenchmarkToml,
) {
    assert_eq!(
        benchmark.route_source.as_deref(),
        Some("query-wrapper-source-index-trace-projection")
    );
    assert_eq!(benchmark.max_provider_process_count, Some(0));
    assert_eq!(benchmark.max_stdout_bytes, Some(4096));
    assert_eq!(benchmark.fallback_reason.as_deref(), Some("none"));
}

fn assert_search_owner_source_index_trace_benchmark_contract(
    benchmark: &SharedBenchmarkToml,
    expected_fallback_reason: &str,
) {
    assert_eq!(
        benchmark.route_source.as_deref(),
        Some("search-owner-source-index-trace")
    );
    assert_eq!(benchmark.max_provider_process_count, Some(0));
    assert_eq!(benchmark.max_stdout_bytes, Some(1024));
    assert_eq!(
        benchmark.fallback_reason.as_deref(),
        Some(expected_fallback_reason)
    );
}

mod turso_pressure;

pub(super) fn asp_turso_db_engine_concurrent_process_pressure_stays_inside_scenario_gate() {
    turso_pressure::asp_turso_db_engine_concurrent_process_pressure_stays_inside_scenario_gate();

    let crate_root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let scenario_root = crate_root
        .join("tests")
        .join("unit")
        .join("scenarios")
        .join("asp_turso_db_engine_concurrent_process_pressure");
    let benchmark: SharedBenchmarkToml = read_toml(&scenario_root.join("benchmark.toml"));
    assert!(
        duration_millis_from_manifest(&benchmark.max_total) <= 300,
        "DB Engine concurrent pressure must stay below the 300ms search pressure budget: max_total={}",
        benchmark.max_total
    );
    assert!(
        duration_millis_from_manifest(&benchmark.observed_total) <= 300,
        "DB Engine concurrent pressure observed_total must not sit at the generic hard ceiling: observed_total={}",
        benchmark.observed_total
    );
    assert_observed_timing_inside_budget(
        &benchmark,
        "process_db_operation",
        300,
        "DB Engine concurrent pressure",
    );
}

pub(super) fn asp_turso_agent_session_registry_shared_route_pressure_stays_inside_scenario_gate() {
    turso_pressure::asp_turso_agent_session_registry_shared_route_pressure_stays_inside_scenario_gate();

    let crate_root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let scenario_root = crate_root
        .join("tests")
        .join("unit")
        .join("scenarios")
        .join("asp_turso_agent_session_registry_shared_route_pressure");
    let benchmark: SharedBenchmarkToml = read_toml(&scenario_root.join("benchmark.toml"));
    assert!(
        duration_millis_from_manifest(&benchmark.max_total) <= 300,
        "agent-session registry pressure must stay below the 300ms search pressure budget: max_total={}",
        benchmark.max_total
    );
    assert!(
        duration_millis_from_manifest(&benchmark.observed_total) <= 300,
        "agent-session registry pressure observed_total must not sit at the generic hard ceiling: observed_total={}",
        benchmark.observed_total
    );
    assert_observed_timing_inside_budget(
        &benchmark,
        "registry_db_operation",
        300,
        "agent-session registry pressure",
    );
}

pub(super) fn asp_turso_source_index_refresh_lookup_pressure_stays_inside_scenario_gate() {
    turso_pressure::asp_turso_source_index_refresh_lookup_pressure_stays_inside_scenario_gate();

    let crate_root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let scenario_root = crate_root
        .join("tests")
        .join("unit")
        .join("scenarios")
        .join("asp_turso_source_index_refresh_lookup_pressure");
    let benchmark: SharedBenchmarkToml = read_toml(&scenario_root.join("benchmark.toml"));
    assert!(
        duration_millis_from_manifest(&benchmark.max_total) <= 250,
        "source-index concurrent pressure must stay below the 250ms search budget: max_total={}",
        benchmark.max_total
    );
    assert!(
        duration_millis_from_manifest(&benchmark.observed_total) <= 250,
        "source-index concurrent pressure observed_total must not sit at the generic hard ceiling: observed_total={}",
        benchmark.observed_total
    );
    assert_observed_timing_inside_budget(
        &benchmark,
        "source_index_pressure",
        250,
        "source-index concurrent pressure",
    );
}

fn assert_query_wrapper_clause_normalization_benchmark_contract(benchmark: &SharedBenchmarkToml) {
    assert_eq!(
        benchmark.route_source.as_deref(),
        Some("query-wrapper-clause-normalization")
    );
    assert_eq!(benchmark.max_provider_process_count, Some(0));
    assert_eq!(benchmark.max_stdout_bytes, Some(4096));
    assert_eq!(benchmark.fallback_reason.as_deref(), Some("none"));
}

fn assert_search_query_budget_benchmark_contract(benchmark: &SharedBenchmarkToml) {
    assert_eq!(
        benchmark.route_source.as_deref(),
        Some("search-query-budget")
    );
    assert_eq!(benchmark.max_provider_process_count, Some(0));
    assert_eq!(benchmark.max_stdout_bytes, Some(4096));
    assert_eq!(benchmark.fallback_reason.as_deref(), Some("none"));
}

fn assert_generated_candidate_benchmark_contract(benchmark: &SharedBenchmarkToml) {
    assert_eq!(
        benchmark.route_source.as_deref(),
        Some("search-pipe-generated-candidate")
    );
    assert_eq!(benchmark.max_provider_process_count, Some(0));
    assert_eq!(benchmark.max_stdout_bytes, Some(8192));
    assert_eq!(benchmark.fallback_reason.as_deref(), Some("none"));
}

fn assert_provider_candidate_annotations_benchmark_contract(benchmark: &SharedBenchmarkToml) {
    assert_eq!(
        benchmark.route_source.as_deref(),
        Some("provider-candidate-annotations")
    );
    assert_eq!(benchmark.max_provider_process_count, Some(0));
    assert_eq!(benchmark.max_stdout_bytes, Some(4096));
    assert_eq!(benchmark.fallback_reason.as_deref(), Some("none"));
}

fn assert_graph_node_projection_benchmark_contract(benchmark: &SharedBenchmarkToml) {
    assert_eq!(
        benchmark.route_source.as_deref(),
        Some("graph-node-projection")
    );
    assert_eq!(benchmark.max_provider_process_count, Some(0));
    assert_eq!(benchmark.max_stdout_bytes, Some(4096));
    assert_eq!(benchmark.fallback_reason.as_deref(), Some("none"));
}

fn assert_graph_candidate_projection_benchmark_contract(benchmark: &SharedBenchmarkToml) {
    assert_eq!(
        benchmark.route_source.as_deref(),
        Some("graph-candidate-projection")
    );
    assert_eq!(benchmark.max_provider_process_count, Some(0));
    assert_eq!(benchmark.max_stdout_bytes, Some(4096));
    assert_eq!(benchmark.fallback_reason.as_deref(), Some("none"));
}

fn assert_graph_topology_projection_benchmark_contract(benchmark: &SharedBenchmarkToml) {
    assert_eq!(
        benchmark.route_source.as_deref(),
        Some("graph-topology-projection")
    );
    assert_eq!(benchmark.max_provider_process_count, Some(0));
    assert_eq!(benchmark.max_stdout_bytes, Some(4096));
    assert_eq!(benchmark.fallback_reason.as_deref(), Some("none"));
}

fn assert_graph_owner_rank_benchmark_contract(benchmark: &SharedBenchmarkToml) {
    assert_eq!(benchmark.route_source.as_deref(), Some("graph-owner-rank"));
    assert_eq!(benchmark.max_provider_process_count, Some(0));
    assert_eq!(benchmark.max_stdout_bytes, Some(4096));
    assert_eq!(benchmark.fallback_reason.as_deref(), Some("none"));
}

fn assert_graph_query_owner_seed_benchmark_contract(benchmark: &SharedBenchmarkToml) {
    assert_eq!(
        benchmark.route_source.as_deref(),
        Some("graph-query-owner-seed")
    );
    assert_eq!(benchmark.max_provider_process_count, Some(0));
    assert_eq!(benchmark.max_stdout_bytes, Some(4096));
    assert_eq!(benchmark.fallback_reason.as_deref(), Some("none"));
}

fn assert_graph_seed_decision_benchmark_contract(benchmark: &SharedBenchmarkToml) {
    assert_eq!(
        benchmark.route_source.as_deref(),
        Some("graph-seed-decision")
    );
    assert_eq!(benchmark.max_provider_process_count, Some(0));
    assert_eq!(benchmark.max_stdout_bytes, Some(4096));
    assert_eq!(benchmark.fallback_reason.as_deref(), Some("none"));
}

fn assert_graph_evidence_projection_benchmark_contract(benchmark: &SharedBenchmarkToml) {
    assert_eq!(
        benchmark.route_source.as_deref(),
        Some("graph-evidence-projection")
    );
    assert_eq!(benchmark.max_provider_process_count, Some(0));
    assert_eq!(benchmark.max_stdout_bytes, Some(4096));
    assert_eq!(benchmark.fallback_reason.as_deref(), Some("none"));
}

fn assert_search_pipe_package_cohesion_benchmark_contract(benchmark: &SharedBenchmarkToml) {
    assert_eq!(
        benchmark.route_source.as_deref(),
        Some("search-pipe-package-cohesion")
    );
    assert_eq!(benchmark.max_provider_process_count, Some(0));
    assert_eq!(benchmark.max_stdout_bytes, Some(4096));
    assert_eq!(benchmark.fallback_reason.as_deref(), Some("none"));
}

fn assert_search_pipe_query_pack_benchmark_contract(benchmark: &SharedBenchmarkToml) {
    assert_eq!(
        benchmark.route_source.as_deref(),
        Some("search-pipe-query-pack")
    );
    assert_eq!(benchmark.max_provider_process_count, Some(0));
    assert_eq!(benchmark.max_stdout_bytes, Some(4096));
    assert_eq!(benchmark.fallback_reason.as_deref(), Some("none"));
}

fn assert_search_pipe_quality_decision_benchmark_contract(benchmark: &SharedBenchmarkToml) {
    assert_eq!(
        benchmark.route_source.as_deref(),
        Some("search-pipe-quality-decision")
    );
    assert_eq!(benchmark.max_provider_process_count, Some(0));
    assert_eq!(benchmark.max_stdout_bytes, Some(4096));
    assert_eq!(benchmark.fallback_reason.as_deref(), Some("none"));
}

fn assert_search_pipe_evidence_classifier_benchmark_contract(benchmark: &SharedBenchmarkToml) {
    assert_eq!(
        benchmark.route_source.as_deref(),
        Some("search-pipe-evidence-classifier")
    );
    assert_eq!(benchmark.max_provider_process_count, Some(0));
    assert_eq!(benchmark.max_stdout_bytes, Some(4096));
    assert_eq!(benchmark.fallback_reason.as_deref(), Some("none"));
}

fn assert_runtime_owner_items_receipt_benchmark_contract(benchmark: &SharedBenchmarkToml) {
    assert_eq!(
        benchmark.route_source.as_deref(),
        Some("owner-items-runtime")
    );
    assert_eq!(benchmark.max_provider_process_count, Some(1));
    assert_eq!(benchmark.max_stdout_bytes, Some(4096));
    assert_eq!(benchmark.fallback_reason.as_deref(), Some("none"));
}

fn assert_runtime_timeout_policy_benchmark_contract(benchmark: &SharedBenchmarkToml) {
    assert_eq!(
        benchmark.route_source.as_deref(),
        Some("runtime-timeout-policy")
    );
    assert_eq!(benchmark.max_provider_process_count, Some(0));
    assert_eq!(benchmark.max_stdout_bytes, Some(1024));
    assert_eq!(benchmark.fallback_reason.as_deref(), Some("none"));
}

#[derive(Clone, Copy, Debug)]
struct LanguageScenarioBenchmarkRequirement {
    language: &'static str,
    root: &'static str,
    syntax: ScenarioBenchmarkSyntax,
}

#[derive(Clone, Copy, Debug)]
enum ScenarioBenchmarkSyntax {
    TomlPair,
    GerbilBenchmarkSs,
}

#[derive(Debug, Deserialize)]
struct ScenarioPolicyIds {
    #[serde(default)]
    policy_ids: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct SharedScenarioToml {
    id: String,
    title: String,
    #[serde(default)]
    policy_ids: Vec<String>,
    agent_goal: String,
    inputs: String,
    expected: String,
}

#[derive(Debug, Deserialize)]
struct SharedBenchmarkToml {
    harness: String,
    #[serde(default)]
    test: Option<String>,
    #[serde(default)]
    bench: Option<String>,
    #[serde(default)]
    phase: Option<String>,
    target_total: String,
    max_total: String,
    observed_total: String,
    regression_budget: String,
    memory_budget_bytes: u64,
    observed_memory_bytes: u64,
    target_rationale: String,
    observed_timings: BTreeMap<String, toml::Value>,
    #[serde(default)]
    route_source: Option<String>,
    #[serde(default)]
    max_provider_process_count: Option<u32>,
    #[serde(default)]
    max_stdout_bytes: Option<u64>,
    #[serde(default)]
    fallback_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
struct SearchFrameReferenceBenchmarkToml {
    #[serde(default, rename = "codebaseMemoryMcpQueryRounds")]
    codebase_memory_mcp_query_rounds: Option<u32>,
    #[serde(default, rename = "aspSearchQueryRounds")]
    asp_search_query_rounds: Option<u32>,
    #[serde(default, rename = "roundDelta")]
    round_delta: Option<i32>,
    #[serde(default, rename = "selectorEvidence")]
    selector_evidence: Option<String>,
    #[serde(default, rename = "evidenceAdvantage")]
    evidence_advantage: Option<String>,
}

pub(super) fn asp_unit_scenarios_have_rust_harness_benchmark_toml_gates() {
    let crate_root = Path::new(env!("CARGO_MANIFEST_DIR"));
    rust_lang_project_harness::assert_rule_fixture_scenario_benchmarks(crate_root);
    let receipt = rust_lang_project_harness::validate_required_rust_scenario_benchmarks(crate_root)
        .expect("validate ASP unit scenario benchmark gates");

    assert!(
        !receipt.requirements.is_empty(),
        "ASP tests must define at least one tests/unit/scenarios/*/scenario.toml fixture with benchmark.toml"
    );
    assert_eq!(
        receipt.status,
        rust_lang_project_harness::RustScenarioBenchmarkStatus::Pass,
        "{receipt:#?}"
    );
    assert!(receipt.receipts.iter().all(|receipt| {
        receipt.benchmark.observed_total <= receipt.benchmark.max_total
            && receipt.benchmark.observed_memory_bytes <= receipt.benchmark.memory_budget_bytes
    }));
}

pub(super) fn asp_unit_scenarios_cover_perf_sensitive_query_search_subcommands() {
    let scenario_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("unit")
        .join("scenarios");
    let policy_ids = discover_scenario_policy_ids(&scenario_root);
    let missing = REQUIRED_PERFORMANCE_SUBCOMMAND_POLICY_IDS
        .iter()
        .copied()
        .filter(|policy_id| !policy_ids.contains(*policy_id))
        .collect::<Vec<_>>();

    assert!(
        missing.is_empty(),
        "ASP unit scenarios must cover performance-sensitive query/search subcommands; missing={missing:?}; observed={policy_ids:?}"
    );
}

pub(super) fn asp_language_scenarios_define_cold_first_performance_gates() {
    let missing = COLD_FIRST_SEARCH_LANGUAGE_IDS
        .iter()
        .copied()
        .filter(|language| !language_has_cold_first_benchmark(language))
        .collect::<Vec<_>>();

    assert!(
        missing.is_empty(),
        "language scenario benchmark matrix must define at least one cold functional gate per target language; missing={missing:?}"
    );
}

fn language_has_cold_first_benchmark(language: &str) -> bool {
    LANGUAGE_SCENARIO_BENCHMARK_REQUIREMENTS
        .iter()
        .find(|requirement| requirement.language == language)
        .is_some_and(|requirement| {
            let root = Path::new(env!("CARGO_MANIFEST_DIR"))
                .parent()
                .and_then(Path::parent)
                .expect("workspace root")
                .join(requirement.root);
            discover_benchmark_toml_paths(&root)
                .into_iter()
                .any(|path| {
                    let benchmark: SharedBenchmarkToml = read_toml(&path);
                    benchmark.phase.as_deref() == Some("cold")
                        && benchmark.route_source.is_some()
                        && benchmark.max_provider_process_count.is_some()
                        && benchmark.max_stdout_bytes.is_some()
                        && benchmark.fallback_reason.as_deref() == Some("none")
                        && benchmark.target_total != "0ms"
                        && benchmark.max_total != "0ms"
                })
        })
}

fn discover_benchmark_toml_paths(root: &Path) -> Vec<PathBuf> {
    let mut paths = Vec::new();
    discover_benchmark_toml_paths_into(root, &mut paths);
    paths
}

fn discover_benchmark_toml_paths_into(root: &Path, paths: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(root) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            discover_benchmark_toml_paths_into(&path, paths);
        } else if path.file_name().and_then(|name| name.to_str()) == Some("benchmark.toml") {
            paths.push(path);
        }
    }
}

pub(super) fn asp_selector_seeded_search_pipe_frontier_stays_inside_scenario_gate() {
    let crate_root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let scenario_root = crate_root
        .join("tests")
        .join("unit")
        .join("scenarios")
        .join("asp_selector_seeded_search_pipe_frontier");
    let benchmark: SharedBenchmarkToml = read_toml(&scenario_root.join("benchmark.toml"));
    assert_selector_seed_benchmark_contract(&benchmark);
    let max_total_ms = duration_millis_from_manifest(&benchmark.max_total);

    let selector = "rust://crates/agent-semantic-protocol/src/command/provider_process.rs#item/fn/provider_invocation_with_profile";
    let query = "runtime_profile_invocation RuntimeProfiles provider_command_prefix";
    let started_at = Instant::now();
    let stdout = render_selector_seeded_search_pipe("rust", selector, query, ".");
    let elapsed = started_at.elapsed();
    let elapsed_ms = elapsed.as_millis();
    let render_duration = duration_literal(elapsed);

    for expected in [
        "source=selector",
        "ranker=selector-seed",
        "ownerSeed=crates/agent-semantic-protocol/src/command/provider_process.rs",
        "symbolSeed=provider_invocation_with_profile",
        "actionFrontier=A1.query-code,A2.owner-items,A3.rg-query",
        "recommendedNext=A1.query-code",
    ] {
        assert!(
            stdout.contains(expected),
            "selector-seeded search pipe scenario missing {expected:?}; stdout={stdout}"
        );
    }
    assert!(
        stdout.contains(&format!("selectorSeed={selector}")),
        "stdout={stdout}"
    );
    assert!(
        stdout.contains(&format!(
            "nextCommand=asp rust query --selector '{selector}' --workspace . --code"
        )),
        "stdout={stdout}"
    );
    assert!(
        !stdout.contains("&&"),
        "selector-seeded search pipe must not return shell-chained next commands; stdout={stdout}"
    );
    assert!(
        elapsed_ms <= max_total_ms,
        "selector-seeded search pipe scenario exceeded benchmark max_total={} observed={}ms stdout={stdout}",
        benchmark.max_total,
        elapsed_ms
    );

    let performance_gate = serde_json::json!({
        "schemaId": "agent.semantic-protocols.semantic-hot-path-performance-gate",
        "schemaVersion": "1",
        "scenarioId": "asp-selector-seeded-search-pipe-frontier",
        "languageId": "rust",
        "workspace": ".",
        "command": [
            "asp",
            "rust",
            "search",
            "pipe",
            "--selector",
            selector,
            "--query",
            query,
            "--workspace",
            ".",
            "--view",
            "seeds"
        ],
        "phase": "hot",
        "expected": {
            "targetTotal": benchmark.target_total,
            "maxTotal": benchmark.max_total,
            "regressionBudget": benchmark.regression_budget,
            "maxProviderProcessCount": 0,
            "maxSearchOverlayProcessCount": 0,
            "maxRenderDuration": benchmark.max_total,
            "maxStdoutBytes": 4096,
            "allowedFirstRoutes": ["query-code"],
            "forbiddenRoutes": ["prime", "broad-rg", "native-finder", "provider-process"],
            "requireExactCodeIdentity": true,
            "requireNoExecutableLineRange": true
        },
        "observed": {
            "observedTotal": render_duration,
            "providerProcessCount": 0,
            "providerElapsed": "0us",
            "nativeFinderProcessCount": 0,
            "nativeFinderElapsed": "0us",
            "firstRoute": "query-code",
            "executedRoutes": ["query-code"],
            "executableLineRangeSelectorCount": 0,
            "packetOutMode": "not-applicable",
            "renderDuration": render_duration,
            "stdoutBytes": stdout.len()
        },
        "verdict": "pass",
        "evidenceRefs": ["scenario:asp-selector-seeded-search-pipe-frontier"]
    });
    assert_eq!(performance_gate["observed"]["providerProcessCount"], 0);
    assert_eq!(performance_gate["observed"]["nativeFinderProcessCount"], 0);
    assert_eq!(performance_gate["observed"]["stdoutBytes"], stdout.len());
}

pub(super) fn search_frame_codebase_memory_mcp_reference_benchmark_records_round_delta() {
    let crate_root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let scenario_root = crate_root
        .join("tests")
        .join("unit")
        .join("scenarios")
        .join("search_frame_codebase_memory_mcp_reference_benchmark");
    let scenario: SharedScenarioToml = read_toml(&scenario_root.join("scenario.toml"));
    let benchmark: SharedBenchmarkToml = read_toml(&scenario_root.join("benchmark.toml"));
    let reference_benchmark: SearchFrameReferenceBenchmarkToml =
        read_toml(&scenario_root.join("reference_benchmark.toml"));

    assert_eq!(
        scenario.id,
        "search-frame-codebase-memory-mcp-reference-benchmark"
    );
    assert_eq!(benchmark.harness, "libtest");
    assert_eq!(
        benchmark.test.as_deref(),
        Some("search_frame_codebase_memory_mcp_reference_benchmark_records_round_delta")
    );
    assert_eq!(
        benchmark.route_source.as_deref(),
        Some("search-frame-reference-benchmark")
    );
    assert_eq!(benchmark.max_provider_process_count, Some(0));
    assert_eq!(benchmark.max_stdout_bytes, Some(8192));
    assert_eq!(benchmark.fallback_reason.as_deref(), Some("none"));
    assert_codebase_memory_mcp_reference_benchmark_contract(&reference_benchmark);
}

pub(super) fn asp_source_index_search_pipe_warm_path_stays_inside_scenario_gate() {
    let crate_root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let scenario_root = crate_root
        .join("tests")
        .join("unit")
        .join("scenarios")
        .join("asp_source_index_search_pipe_warm_path");
    let benchmark: SharedBenchmarkToml = read_toml(&scenario_root.join("benchmark.toml"));
    assert_source_index_query_benchmark_contract(&benchmark);
    let max_total_ms = duration_millis_from_manifest(&benchmark.max_total);

    let root = temp_project_root("scenario-source-index-search-pipe");
    let bin_dir = root.join(".tmp").join("provider-bin");
    let marker = root.join("provider-called");
    fs::create_dir_all(root.join("src")).expect("create source root");
    fs::write(
        root.join("Cargo.toml"),
        "[package]\nname = \"scenario-source-index-search-pipe\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
    )
    .expect("write package anchor");
    fs::write(
        root.join("src/lib.rs"),
        "pub fn source_index_fixture() {}\npub fn unrelated() {}\n",
    )
    .expect("write source");
    write_marker_provider(&bin_dir, "rs-harness", &marker);
    write_activation(&root, &[provider("rust", Vec::new())]);
    refresh_source_index(&root);
    let _ = fs::remove_file(&marker);

    let language = agent_semantic_client::LanguageId::from("rust");
    let cache_root = crate::provider_command::support::cache_root(&root);
    let warmup_lookup = agent_semantic_client::lookup_source_index_in_client_cache_dir(
        agent_semantic_client::SourceIndexClientCacheLookupRequest {
            cache_root: &cache_root,
            indexed_project_root: &root,
            language_id: Some(&language),
            query: "source_index_fixture",
            limit: 256,
        },
    )
    .expect("warm source index lookup");
    assert_eq!(
        warmup_lookup.state,
        agent_semantic_client::SourceIndexLookupState::Hit
    );

    let lookup_started_at = Instant::now();
    let lookup = agent_semantic_client::lookup_source_index_in_client_cache_dir(
        agent_semantic_client::SourceIndexClientCacheLookupRequest {
            cache_root: &cache_root,
            indexed_project_root: &root,
            language_id: Some(&language),
            query: "source_index_fixture",
            limit: 256,
        },
    )
    .expect("lookup source index");
    let lookup_elapsed = lookup_started_at.elapsed();
    let lookup_duration = duration_literal(lookup_elapsed);
    assert_eq!(
        lookup.state,
        agent_semantic_client::SourceIndexLookupState::Hit
    );
    assert!(
        lookup
            .candidates
            .iter()
            .any(|candidate| candidate.path == "src/lib.rs"),
        "lookup candidates={:?}",
        lookup.candidates
    );
    assert!(
        lookup_elapsed.as_millis() <= max_total_ms,
        "source-index warm lookup exceeded benchmark max_total={} observed={} candidates={:?}",
        benchmark.max_total,
        lookup_elapsed.as_millis(),
        lookup.candidates
    );

    let output = asp_command(&root)
        .env("PATH", prepend_path(&bin_dir))
        .env("PRJ_CACHE_HOME", root.join(".cache"))
        .args([
            "rust",
            "search",
            "pipe",
            "source_index_fixture",
            "--workspace",
            ".",
            "--view",
            "seeds",
        ])
        .output()
        .expect("run asp rust search pipe");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("stdout");
    for expected in [
        "[search-pipe]",
        "sourceTrace=sourceIndex:used",
        "search-overlay:skipped",
        "ownerCoverage=bestOwner=src/lib.rs",
        "nextCommand=asp rust search owner src/lib.rs items --query source_index_fixture --workspace . --view seeds",
    ] {
        assert!(
            stdout.contains(expected),
            "source-index search pipe scenario missing {expected:?}; stdout={stdout}"
        );
    }
    assert!(
        !marker.exists(),
        "source-index warm search pipe should not spawn provider"
    );

    let performance_gate = serde_json::json!({
        "schemaId": "agent.semantic-protocols.semantic-hot-path-performance-gate",
        "schemaVersion": "1",
        "scenarioId": "asp-source-index-search-pipe-warm-path",
        "languageId": "rust",
        "workspace": ".",
        "command": [
            "asp",
            "rust",
            "search",
            "pipe",
            "source_index_fixture",
            "--workspace",
            ".",
            "--view",
            "seeds"
        ],
        "phase": "hot",
        "expected": {
            "targetTotal": benchmark.target_total,
            "maxTotal": benchmark.max_total,
            "regressionBudget": benchmark.regression_budget,
            "maxProviderProcessCount": 0,
            "maxSearchOverlayProcessCount": 0,
            "maxRenderDuration": benchmark.max_total,
            "maxStdoutBytes": 8192,
            "requireSourceIndexHit": true,
            "allowedFirstRoutes": ["source-index"],
            "forbiddenRoutes": ["prime", "native-finder", "provider-process"],
            "requireExactCodeIdentity": true,
            "requireNoExecutableLineRange": true
        },
        "observed": {
            "observedTotal": lookup_duration,
            "providerProcessCount": 0,
            "providerElapsed": "0us",
            "nativeFinderProcessCount": 0,
            "nativeFinderElapsed": "0us",
            "sourceIndexHit": true,
            "sourceIndexDuration": lookup_duration,
            "firstRoute": "source-index",
            "executedRoutes": ["source-index", "query-code"],
            "executableLineRangeSelectorCount": 0,
            "packetOutMode": "not-applicable",
            "renderDuration": lookup_duration,
            "stdoutBytes": stdout.len()
        },
        "verdict": "pass",
        "evidenceRefs": ["scenario:asp-source-index-search-pipe-warm-path"]
    });
    assert_eq!(performance_gate["observed"]["providerProcessCount"], 0);
    assert_eq!(performance_gate["observed"]["nativeFinderProcessCount"], 0);
    assert_eq!(performance_gate["observed"]["sourceIndexHit"], true);
    assert_eq!(performance_gate["observed"]["stdoutBytes"], stdout.len());
    let _ = fs::remove_dir_all(root);
}

pub(super) fn asp_source_index_lookup_adapter_cold_functional_path_stays_inside_scenario_gate() {
    let crate_root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let scenario_root = crate_root
        .join("tests")
        .join("unit")
        .join("scenarios")
        .join("asp_source_index_lookup_adapter_cold_functional_path");
    let benchmark: SharedBenchmarkToml = read_toml(&scenario_root.join("benchmark.toml"));
    assert_source_index_query_benchmark_contract(&benchmark);
    let max_total_ms = duration_millis_from_manifest(&benchmark.max_total);

    let root = temp_project_root("scenario-source-index-lookup-adapter-cold");
    let bin_dir = root.join(".tmp").join("provider-bin");
    let marker = root.join("provider-called");
    fs::create_dir_all(root.join("src")).expect("create source root");
    fs::write(
        root.join("Cargo.toml"),
        "[package]\nname = \"scenario-source-index-lookup-adapter-cold\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
    )
    .expect("write package anchor");
    fs::write(
        root.join("src/lib.rs"),
        "pub fn source_index_lookup_adapter_fixture() {}\npub fn unrelated() {}\n",
    )
    .expect("write source");
    write_marker_provider(&bin_dir, "rs-harness", &marker);
    write_activation(&root, &[provider("rust", Vec::new())]);
    refresh_source_index(&root);
    let _ = fs::remove_file(&marker);

    let language = agent_semantic_client::LanguageId::from("rust");
    let cache_root = crate::provider_command::support::cache_root(&root);
    let lookup_started_at = Instant::now();
    let lookup = agent_semantic_search::lookup_source_index_in_client_cache_dir(
        agent_semantic_search::SourceIndexClientCacheLookupRequest {
            cache_root: &cache_root,
            indexed_project_root: &root,
            language_id: Some(&language),
            query: "source_index_lookup_adapter_fixture",
            limit: 256,
        },
    )
    .expect("lookup source index through search adapter");
    let lookup_elapsed = lookup_started_at.elapsed();
    let lookup_ms = lookup_elapsed.as_millis();
    assert_eq!(
        lookup.state,
        agent_semantic_client::SourceIndexLookupState::Hit
    );
    assert!(
        lookup
            .candidates
            .iter()
            .any(|candidate| candidate.path == "src/lib.rs"),
        "lookup candidates={:?}",
        lookup.candidates
    );
    assert!(
        lookup
            .candidates
            .iter()
            .all(|candidate| candidate.query_keys.iter().all(|key| !key.contains(":1:"))),
        "source-index lookup adapter must not expose line-range identity in query keys; candidates={:?}",
        lookup.candidates
    );
    assert!(
        !marker.exists(),
        "source-index lookup adapter cold functional gate must not spawn provider during lookup"
    );
    assert!(
        lookup_ms <= max_total_ms,
        "source-index lookup adapter cold functional path exceeded benchmark max_total={} observed={}ms candidates={:?}",
        benchmark.max_total,
        lookup_ms,
        lookup.candidates
    );

    let lookup_duration = duration_literal(lookup_elapsed);
    let performance_gate = serde_json::json!({
        "schemaId": "agent.semantic-protocols.semantic-hot-path-performance-gate",
        "schemaVersion": "1",
        "scenarioId": "asp-source-index-lookup-adapter-cold-functional-path",
        "languageId": "rust",
        "workspace": ".",
        "command": [
            "agent_semantic_search::lookup_source_index_in_client_cache_dir"
        ],
        "phase": "cold",
        "expected": {
            "targetTotal": benchmark.target_total,
            "maxTotal": benchmark.max_total,
            "regressionBudget": benchmark.regression_budget,
            "maxProviderProcessCount": 0,
            "maxSearchOverlayProcessCount": 0,
            "maxStdoutBytes": benchmark.max_stdout_bytes,
            "requireSourceIndexHit": true,
            "allowedFirstRoutes": ["source-index"],
            "forbiddenRoutes": ["prime", "native-finder", "provider-process"],
            "requireExactCodeIdentity": true,
            "requireNoExecutableLineRange": true
        },
        "observed": {
            "observedTotal": lookup_duration,
            "providerProcessCount": 0,
            "providerElapsed": "0us",
            "nativeFinderProcessCount": 0,
            "nativeFinderElapsed": "0us",
            "sourceIndexHit": true,
            "sourceIndexDuration": lookup_duration,
            "firstRoute": "source-index",
            "executedRoutes": ["source-index"],
            "executableLineRangeSelectorCount": 0,
            "packetOutMode": "not-applicable",
            "renderDuration": lookup_duration,
            "stdoutBytes": 0
        },
        "verdict": "pass",
        "evidenceRefs": ["scenario:asp-source-index-lookup-adapter-cold-functional-path"]
    });
    assert_eq!(performance_gate["observed"]["providerProcessCount"], 0);
    assert_eq!(performance_gate["observed"]["nativeFinderProcessCount"], 0);
    assert_eq!(performance_gate["observed"]["sourceIndexHit"], true);
    let _ = fs::remove_dir_all(root);
}

pub(super) fn asp_search_owner_source_index_trace_missing_db_cold_functional_path_stays_inside_scenario_gate()
 {
    let crate_root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let scenario_root = crate_root
        .join("tests")
        .join("unit")
        .join("scenarios")
        .join("asp_search_owner_source_index_trace_missing_db_cold_functional_path");
    let scenario: SharedScenarioToml = read_toml(&scenario_root.join("scenario.toml"));
    let benchmark: SharedBenchmarkToml = read_toml(&scenario_root.join("benchmark.toml"));
    assert_eq!(
        scenario.id,
        "asp-search-owner-source-index-trace-missing-db-cold-functional-path"
    );
    assert_search_owner_source_index_trace_benchmark_contract(&benchmark, "sourceIndex:missing-db");
    let started_at = Instant::now();
    let stdout = render_owner_items_source_index_lookup_trace(
        "crates/agent-semantic-hook/build.rs",
        &ClientDbSourceIndexLookupResult {
            db_path: PathBuf::from("live/client/client.turso"),
            state: ClientDbSourceIndexLookupState::MissingDb,
            candidates: Vec::new(),
        },
    );
    let elapsed = started_at.elapsed();
    let elapsed_ms = elapsed.as_millis();
    let max_total_ms = duration_millis_from_manifest(&benchmark.max_total);

    for expected in [
        "|sourceIndex status=missing-db",
        "source=source-index",
        "reason=sourceIndex:missing-db",
        "next=asp_cache_source-index_refresh",
    ] {
        assert!(
            stdout.contains(expected),
            "search-owner source-index missing-db scenario missing {expected:?}; stdout={stdout}"
        );
    }
    assert!(
        elapsed_ms <= max_total_ms,
        "search-owner source-index trace scenario exceeded benchmark max_total={} observed={}ms stdout={stdout}",
        benchmark.max_total,
        elapsed_ms
    );
    assert!(
        stdout.len() <= benchmark.max_stdout_bytes.unwrap_or(u64::MAX) as usize,
        "stdout exceeded max_stdout_bytes; stdout={stdout}"
    );
    let performance_gate = serde_json::json!({
        "schemaId": "agent.semantic-protocols.semantic-hot-path-performance-gate",
        "schemaVersion": "1",
        "scenarioId": scenario.id,
        "languageId": "rust",
        "phase": "cold-functional",
        "expected": {
            "routeSource": "search-owner-source-index-trace",
            "fallbackReason": "sourceIndex:missing-db",
            "maxProviderProcessCount": 0,
            "maxStdoutBytes": benchmark.max_stdout_bytes,
            "maxTotal": benchmark.max_total
        },
        "observed": {
            "routeSource": "source-index",
            "fallbackReason": "sourceIndex:missing-db",
            "providerProcessCount": 0,
            "stdoutBytes": stdout.len(),
            "elapsed": duration_literal(elapsed)
        },
        "verdict": "pass",
        "evidenceRefs": ["scenario:asp-search-owner-source-index-trace-missing-db-cold-functional-path"]
    });
    assert_eq!(performance_gate["observed"]["providerProcessCount"], 0);

    let command_root = temp_project_root("scenario-search-owner-source-index-hit-command");
    let source_dir = command_root.join("src");
    fs::create_dir_all(&source_dir).expect("create source-index hit command src dir");
    fs::write(
        source_dir.join("lib.rs"),
        "pub fn source_index_owner_command_fixture() {}\n",
    )
    .expect("write source-index hit command fixture");
    refresh_source_index(&command_root);
    let output = asp_command(&command_root)
        .args([
            "rust",
            "search",
            "owner",
            "src/lib.rs",
            "items",
            "--workspace",
            ".",
            "--view",
            "seeds",
        ])
        .output()
        .expect("run asp rust search owner source-index hit command");
    assert!(
        output.status.success(),
        "search owner source-index hit command failed\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let command_stdout = String::from_utf8_lossy(&output.stdout);
    let command_stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        command_stdout.contains("|sourceIndex status=hit"),
        "{command_stdout}"
    );
    assert!(
        command_stdout.contains("source=source-index"),
        "{command_stdout}"
    );
    assert!(
        command_stdout.contains("path=src/lib.rs"),
        "{command_stdout}"
    );
    assert!(
        !command_stdout.contains("owner-not-found") && !command_stderr.contains("owner-not-found"),
        "stdout={command_stdout}\nstderr={command_stderr}"
    );
    assert!(
        !command_stdout.contains("path-only") && !command_stderr.contains("path-only"),
        "stdout={command_stdout}\nstderr={command_stderr}"
    );
    let _ = fs::remove_dir_all(command_root);
}

pub(super) fn asp_search_owner_source_index_trace_hit_cold_functional_path_stays_inside_scenario_gate()
 {
    let crate_root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let scenario_root = crate_root
        .join("tests")
        .join("unit")
        .join("scenarios")
        .join("asp_search_owner_source_index_trace_hit_cold_functional_path");
    let scenario: SharedScenarioToml = read_toml(&scenario_root.join("scenario.toml"));
    let benchmark: SharedBenchmarkToml = read_toml(&scenario_root.join("benchmark.toml"));
    assert_eq!(
        scenario.id,
        "asp-search-owner-source-index-trace-hit-cold-functional-path"
    );
    assert_search_owner_source_index_trace_benchmark_contract(&benchmark, "none");
    let started_at = Instant::now();
    let stdout = render_owner_items_source_index_lookup_trace(
        "crates/agent-semantic-hook/build.rs",
        &ClientDbSourceIndexLookupResult {
            db_path: PathBuf::from("live/client/client.turso"),
            state: ClientDbSourceIndexLookupState::Hit,
            candidates: vec![ClientDbSourceIndexCandidate {
                path: "crates/agent-semantic-hook/build.rs".to_string(),
                language_id: None,
                provider_id: None,
                source_kind: ClientDbSourceIndexSourceKind::Other("turso-source-index".to_string()),
                line_count: Some(8),
                query_keys: vec!["build".to_string(), "build.rs".to_string()],
            }],
        },
    );
    let elapsed = started_at.elapsed();
    let elapsed_ms = elapsed.as_millis();
    let max_total_ms = duration_millis_from_manifest(&benchmark.max_total);

    for expected in [
        "|sourceIndex status=hit",
        "source=source-index",
        "path=crates/agent-semantic-hook/build.rs",
    ] {
        assert!(
            stdout.contains(expected),
            "search-owner source-index hit scenario missing {expected:?}; stdout={stdout}"
        );
    }
    for forbidden in ["owner-not-found", "path-only"] {
        assert!(
            !stdout.contains(forbidden),
            "search-owner source-index hit scenario must not expose {forbidden:?}; stdout={stdout}"
        );
    }
    assert!(
        elapsed_ms <= max_total_ms,
        "search-owner source-index trace hit scenario exceeded benchmark max_total={} observed={}ms stdout={stdout}",
        benchmark.max_total,
        elapsed_ms
    );
    assert!(
        stdout.len() <= benchmark.max_stdout_bytes.unwrap_or(u64::MAX) as usize,
        "stdout exceeded max_stdout_bytes; stdout={stdout}"
    );
    let performance_gate = serde_json::json!({
        "schemaId": "agent.semantic-protocols.semantic-hot-path-performance-gate",
        "schemaVersion": "1",
        "scenarioId": scenario.id,
        "languageId": "rust",
        "phase": "cold-functional",
        "expected": {
            "routeSource": "search-owner-source-index-trace",
            "fallbackReason": "none",
            "maxProviderProcessCount": 0,
            "maxStdoutBytes": benchmark.max_stdout_bytes,
            "maxTotal": benchmark.max_total
        },
        "observed": {
            "routeSource": "source-index",
            "fallbackReason": "none",
            "providerProcessCount": 0,
            "stdoutBytes": stdout.len(),
            "elapsed": duration_literal(elapsed)
        },
        "verdict": "pass",
        "evidenceRefs": ["scenario:asp-search-owner-source-index-trace-hit-cold-functional-path"]
    });
    assert_eq!(performance_gate["observed"]["providerProcessCount"], 0);
}

pub(super) fn asp_evidence_graph_rank_cold_functional_path_stays_inside_scenario_gate() {
    let crate_root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let scenario_root = crate_root
        .join("tests")
        .join("unit")
        .join("scenarios")
        .join("asp_evidence_graph_rank_cold_functional_path");
    let benchmark: SharedBenchmarkToml = read_toml(&scenario_root.join("benchmark.toml"));
    assert_evidence_graph_rank_benchmark_contract(&benchmark);
    let max_total_ms = duration_millis_from_manifest(&benchmark.max_total);

    let nodes = vec![
        agent_semantic_search::EvidenceGraphRankNode {
            ordinal: 0,
            id: "structural-owner:generation:src/lib.rs".to_string(),
            kind: "structural-owner".to_string(),
            label: "src/lib.rs".to_string(),
            path: Some("src/lib.rs".to_string()),
            selector: None,
            query_keys: vec!["lib".to_string()],
            outgoing_edge_count: 8,
        },
        agent_semantic_search::EvidenceGraphRankNode {
            ordinal: 1,
            id: "selector:rust://src/lib.rs#item/struct/EvidenceFixture".to_string(),
            kind: "selector".to_string(),
            label: "EvidenceFixture".to_string(),
            path: Some("src/lib.rs".to_string()),
            selector: Some("rust://src/lib.rs#item/struct/EvidenceFixture".to_string()),
            query_keys: vec!["EvidenceFixture".to_string(), "serde".to_string()],
            outgoing_edge_count: 0,
        },
        agent_semantic_search::EvidenceGraphRankNode {
            ordinal: 2,
            id: "symbol:rust://src/lib.rs#item/impl/Serialize".to_string(),
            kind: "symbol".to_string(),
            label: "Serialize impl".to_string(),
            path: Some("src/lib.rs".to_string()),
            selector: Some("rust://src/lib.rs#item/impl/Serialize".to_string()),
            query_keys: vec!["serde".to_string()],
            outgoing_edge_count: 2,
        },
    ];
    assert!(
        nodes.iter().all(|node| !node.id.contains(":1:")
            && !node.selector.as_deref().unwrap_or("").contains(":1:")),
        "EvidenceGraph rank nodes must not encode executable line ranges"
    );

    let started_at = Instant::now();
    let ranked = agent_semantic_search::rank_evidence_graph_nodes(nodes, "serde EvidenceFixture");
    let elapsed = started_at.elapsed();
    let elapsed_ms = elapsed.as_millis();
    assert_eq!(
        ranked[0].node.selector.as_deref(),
        Some("rust://src/lib.rs#item/struct/EvidenceFixture")
    );
    assert_eq!(ranked[0].score.term_hits, 2);
    assert_eq!(ranked[0].score.selector_bonus, 1);
    assert!(
        ranked.iter().all(|ranked| ranked.score.topology_bonus <= 8),
        "topology bonus must stay bounded; ranked={ranked:?}"
    );
    assert!(
        elapsed_ms <= max_total_ms,
        "EvidenceGraph rank cold functional path exceeded benchmark max_total={} observed={}ms ranked={ranked:?}",
        benchmark.max_total,
        elapsed_ms
    );

    let observed_total = duration_literal(elapsed);
    let performance_gate = serde_json::json!({
        "schemaId": "agent.semantic-protocols.semantic-hot-path-performance-gate",
        "schemaVersion": "1",
        "scenarioId": "asp-evidence-graph-rank-cold-functional-path",
        "languageId": "rust",
        "workspace": ".",
        "command": [
            "agent_semantic_search::rank_evidence_graph_nodes"
        ],
        "phase": "cold",
        "expected": {
            "targetTotal": benchmark.target_total,
            "maxTotal": benchmark.max_total,
            "regressionBudget": benchmark.regression_budget,
            "maxProviderProcessCount": 0,
            "maxSearchOverlayProcessCount": 0,
            "maxStdoutBytes": benchmark.max_stdout_bytes,
            "requireSelectorFirst": true,
            "requireBoundedTopologyBonus": true,
            "requireNoExecutableLineRange": true
        },
        "observed": {
            "observedTotal": observed_total,
            "providerProcessCount": 0,
            "nativeFinderProcessCount": 0,
            "firstRoute": benchmark.route_source,
            "executedRoutes": [benchmark.route_source],
            "selectorFirst": true,
            "maxTopologyBonus": ranked.iter().map(|node| node.score.topology_bonus).max().unwrap_or(0),
            "executableLineRangeSelectorCount": 0,
            "stdoutBytes": 0,
            "fallbackReason": benchmark.fallback_reason
        },
        "verdict": "pass",
        "evidenceRefs": ["scenario:asp-evidence-graph-rank-cold-functional-path"]
    });
    assert_eq!(performance_gate["observed"]["providerProcessCount"], 0);
    assert_eq!(performance_gate["observed"]["nativeFinderProcessCount"], 0);
    assert_eq!(performance_gate["observed"]["selectorFirst"], true);
}

pub(super) fn asp_search_candidate_contract_cold_functional_path_stays_inside_scenario_gate() {
    let crate_root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let scenario_root = crate_root
        .join("tests")
        .join("unit")
        .join("scenarios")
        .join("asp_search_candidate_contract_cold_functional_path");
    let benchmark: SharedBenchmarkToml = read_toml(&scenario_root.join("benchmark.toml"));
    assert_search_candidate_contract_benchmark_contract(&benchmark);
    let max_total_ms = duration_millis_from_manifest(&benchmark.max_total);

    let started_at = Instant::now();
    let terms = agent_semantic_search::source_index_lookup_terms("source index fixture");
    let source_index_candidate = agent_semantic_search::source_index_candidate_to_search_candidate(
        agent_semantic_search::SourceIndexRankCandidate {
            ordinal: 0,
            path: "src/lib.rs".to_string(),
            query_keys: vec!["source_index_fixture".to_string(), "lib".to_string()],
        },
        &terms,
    );
    let overlay_hits = agent_semantic_search::search_lexical_overlay(
        agent_semantic_search::LexicalOverlaySearchRequest::new("overlay fixture").document(
            agent_semantic_search::LexicalOverlayDocument::new(
                "src/lib.rs",
                "rust://src/lib.rs#item/function/overlay_fixture",
                "overlay_fixture",
            )
            .search_text("dynamic overlay fixture owner"),
        ),
    );
    let overlay_candidate = agent_semantic_search::lexical_overlay_hit_to_search_candidate(
        &overlay_hits[0],
        "session-1/base-1",
    );
    let ranked_candidates = agent_semantic_search::merge_search_candidates(vec![
        source_index_candidate.clone(),
        overlay_candidate.clone(),
    ]);
    let elapsed = started_at.elapsed();
    let elapsed_ms = elapsed.as_millis();

    assert_eq!(source_index_candidate.route_source, "source-index");
    assert_eq!(overlay_candidate.route_source, "search-overlay");
    assert_eq!(overlay_candidate.identity_kind, "selector");
    assert_eq!(
        overlay_candidate.selector.as_deref(),
        Some("rust://src/lib.rs#item/function/overlay_fixture")
    );
    assert!(
        source_index_candidate
            .field_hits
            .iter()
            .any(|field| field.field == "query_keys"),
        "source-index candidate must carry field hit evidence: {source_index_candidate:?}"
    );
    assert!(
        overlay_candidate
            .rank_features
            .iter()
            .any(|feature| feature.name == "search-overlay-score"),
        "overlay candidate must carry rank features: {overlay_candidate:?}"
    );
    assert!(
        !agent_semantic_search::search_candidate_has_executable_line_identity(
            &source_index_candidate
        ) && !agent_semantic_search::search_candidate_has_executable_line_identity(
            &overlay_candidate
        ),
        "shared search candidate contract must not use executable line-range identity"
    );
    assert_eq!(
        ranked_candidates[0].candidate.route_source, "search-overlay",
        "active overlay candidates must outrank stable source-index candidates before graph fusion: {ranked_candidates:?}"
    );
    assert_eq!(ranked_candidates[0].selector_bonus, 1);
    assert!(
        elapsed_ms <= max_total_ms,
        "search candidate contract cold functional path exceeded benchmark max_total={} observed={}ms",
        benchmark.max_total,
        elapsed_ms
    );

    let observed_total = duration_literal(elapsed);
    let performance_gate = serde_json::json!({
        "schemaId": "agent.semantic-protocols.semantic-hot-path-performance-gate",
        "schemaVersion": "1",
        "scenarioId": "asp-search-candidate-contract-cold-functional-path",
        "languageId": "rust",
        "workspace": ".",
        "command": [
            "agent_semantic_search::source_index_candidate_to_search_candidate",
            "agent_semantic_search::lexical_overlay_hit_to_search_candidate",
            "agent_semantic_search::merge_search_candidates"
        ],
        "phase": "cold",
        "expected": {
            "targetTotal": benchmark.target_total,
            "maxTotal": benchmark.max_total,
            "regressionBudget": benchmark.regression_budget,
            "maxProviderProcessCount": 0,
            "maxSearchOverlayProcessCount": 0,
            "maxStdoutBytes": benchmark.max_stdout_bytes,
            "requireFieldHits": true,
            "requireRankFeatures": true,
            "requireOverlayBeforeStable": true,
            "allowedFirstRoutes": ["search-candidate-contract"],
            "forbiddenRoutes": ["client", "command", "native-finder", "provider-process"],
            "requireExactCodeIdentity": true,
            "requireNoExecutableLineRange": true
        },
        "observed": {
            "observedTotal": observed_total,
            "providerProcessCount": 0,
            "providerElapsed": "0us",
            "nativeFinderProcessCount": 0,
            "nativeFinderElapsed": "0us",
            "firstRoute": "search-candidate-contract",
            "executedRoutes": ["source-index", "dynamic-overlay"],
            "sourceIndexFieldHitCount": source_index_candidate.field_hits.len(),
            "overlayRankFeatureCount": overlay_candidate.rank_features.len(),
            "mergedCandidateCount": ranked_candidates.len(),
            "firstMergedRoute": ranked_candidates[0].candidate.route_source,
            "executableLineRangeSelectorCount": 0,
            "packetOutMode": "not-applicable",
            "renderDuration": observed_total,
            "stdoutBytes": 0
        },
        "verdict": "pass",
        "evidenceRefs": ["scenario:asp-search-candidate-contract-cold-functional-path"]
    });
    assert_eq!(performance_gate["observed"]["providerProcessCount"], 0);
    assert_eq!(performance_gate["observed"]["nativeFinderProcessCount"], 0);
    assert_eq!(
        performance_gate["observed"]["executableLineRangeSelectorCount"],
        0
    );
    assert_eq!(
        performance_gate["observed"]["firstMergedRoute"],
        "search-overlay"
    );
}

pub(super) fn asp_query_wrapper_source_index_bridge_cold_functional_path_stays_inside_scenario_gate()
 {
    let crate_root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let scenario_root = crate_root
        .join("tests")
        .join("unit")
        .join("scenarios")
        .join("asp_query_wrapper_source_index_bridge_cold_functional_path");
    let benchmark: SharedBenchmarkToml = read_toml(&scenario_root.join("benchmark.toml"));
    assert_query_wrapper_source_index_bridge_benchmark_contract(&benchmark);
    let max_total_ms = duration_millis_from_manifest(&benchmark.max_total);

    let root = temp_project_root("scenario-query-wrapper-source-index-bridge-cold");
    fs::create_dir_all(root.join("src")).expect("create source root");
    fs::write(
        root.join("src/lib.rs"),
        "pub fn query_wrapper_source_index_bridge() {}\n",
    )
    .expect("write source");

    let terms = vec!["query_wrapper_source_index_bridge".to_string()];
    let started_at = Instant::now();
    let lookup = agent_semantic_search::QueryWrapperSourceIndexLookup::new(
        root.join("live/client/client.turso"),
        "hit",
        vec![
            agent_semantic_search::QueryWrapperSourceIndexCandidate::new(
                "src/lib.rs",
                Some("rust".to_string()),
                Some("rs-harness".to_string()),
                "source",
                Some(1),
                terms.clone(),
            ),
        ],
    );
    let collection = agent_semantic_search::collect_query_wrapper_source_index_candidates(
        agent_semantic_search::QueryWrapperSourceIndexRequest {
            surface: agent_semantic_search::QueryWrapperCandidateSurface::Rg,
            project_root: &root,
            roots: std::slice::from_ref(&root),
            terms: &terms,
            axis_terms: &terms,
            lookup: &lookup,
        },
    )
    .expect("collect query-wrapper source-index bridge candidates")
    .expect("source-index hit should produce candidates");
    let elapsed = started_at.elapsed();
    let elapsed_ms = elapsed.as_millis();

    assert_eq!(collection.candidates.len(), 1);
    assert_eq!(collection.candidates[0].path, "src/lib.rs");
    assert_eq!(collection.candidates[0].source, "source-index");
    assert!(
        collection
            .candidates
            .iter()
            .all(|candidate| !candidate.path.contains(":1:")),
        "query-wrapper source-index bridge must not expose executable line-range identity: {:?}",
        collection.candidates
    );
    assert!(
        !root.join(".cache").join("agent-semantic-protocol").exists(),
        "query-wrapper source-index bridge must not create project-local cache"
    );
    assert!(
        elapsed_ms <= max_total_ms,
        "query-wrapper source-index bridge cold functional path exceeded benchmark max_total={} observed={}ms candidates={:?}",
        benchmark.max_total,
        elapsed_ms,
        collection.candidates
    );

    let observed_total = duration_literal(elapsed);
    let performance_gate = serde_json::json!({
        "schemaId": "agent.semantic-protocols.semantic-hot-path-performance-gate",
        "schemaVersion": "1",
        "scenarioId": "asp-query-wrapper-source-index-bridge-cold-functional-path",
        "languageId": "rust",
        "workspace": ".",
        "command": [
            "agent_semantic_search::QueryWrapperSourceIndexLookup::new",
            "agent_semantic_search::QueryWrapperSourceIndexCandidate::new",
            "agent_semantic_search::collect_query_wrapper_source_index_candidates"
        ],
        "phase": "cold",
        "expected": {
            "targetTotal": benchmark.target_total,
            "maxTotal": benchmark.max_total,
            "regressionBudget": benchmark.regression_budget,
            "maxProviderProcessCount": 0,
            "maxSearchOverlayProcessCount": 0,
            "maxStdoutBytes": benchmark.max_stdout_bytes,
            "requireSearchOwnedBridge": true,
            "allowedFirstRoutes": ["query-wrapper-source-index-bridge"],
            "forbiddenRoutes": ["command-dto-construction", "native-finder", "provider-process"],
            "requireNoExecutableLineRange": true
        },
        "observed": {
            "observedTotal": observed_total,
            "providerProcessCount": 0,
            "providerElapsed": "0us",
            "nativeFinderProcessCount": 0,
            "nativeFinderElapsed": "0us",
            "candidateCount": collection.candidates.len(),
            "firstRoute": "query-wrapper-source-index-bridge",
            "executedRoutes": ["query-wrapper-source-index-bridge"],
            "executableLineRangeSelectorCount": 0,
            "stdoutBytes": 0,
            "fallbackReason": "none"
        },
        "verdict": "pass",
        "evidenceRefs": ["scenario:asp-query-wrapper-source-index-bridge-cold-functional-path"]
    });
    assert_eq!(performance_gate["observed"]["providerProcessCount"], 0);
    assert_eq!(performance_gate["observed"]["nativeFinderProcessCount"], 0);
    assert_eq!(performance_gate["observed"]["candidateCount"], 1);
    let _ = fs::remove_dir_all(root);
}

pub(super) fn asp_query_wrapper_render_hint_projection_cold_functional_path_stays_inside_scenario_gate()
 {
    let crate_root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let scenario_root = crate_root
        .join("tests")
        .join("unit")
        .join("scenarios")
        .join("asp_query_wrapper_render_hint_projection_cold_functional_path");
    let benchmark: SharedBenchmarkToml = read_toml(&scenario_root.join("benchmark.toml"));
    assert_query_wrapper_render_hint_projection_benchmark_contract(&benchmark);
    let max_total_ms = duration_millis_from_manifest(&benchmark.max_total);

    let paths = vec![
        "packages/client/runtime/search.rs".to_string(),
        "packages/client/runtime/search.rs".to_string(),
        "packages/protocol/command/search.rs".to_string(),
        "docs/search/provider.org".to_string(),
        "crates/agent-semantic-search/src/query_wrapper_candidates.rs".to_string(),
    ];
    let started_at = Instant::now();
    let owner_candidates =
        agent_semantic_search::query_wrapper_owner_candidates(paths.clone().into_iter());
    let package_clusters =
        agent_semantic_search::query_wrapper_package_clusters_from_paths(paths.clone().into_iter());
    let rg_scope_next = agent_semantic_search::query_wrapper_rg_scope_next(paths.into_iter());
    let elapsed = started_at.elapsed();
    let elapsed_ms = elapsed.as_millis();

    assert_eq!(
        owner_candidates,
        vec![
            "packages/client/runtime/search.rs".to_string(),
            "packages/protocol/command/search.rs".to_string(),
            "docs/search/provider.org".to_string(),
            "crates/agent-semantic-search/src/query_wrapper_candidates.rs".to_string(),
        ]
    );
    assert_eq!(
        package_clusters,
        vec![
            "packages/client/runtime".to_string(),
            "packages/protocol/command".to_string(),
            "docs/search".to_string(),
            "crates/agent-semantic-search".to_string(),
        ]
    );
    assert_eq!(
        rg_scope_next,
        vec![
            "packages/client/runtime".to_string(),
            "packages/protocol/command".to_string(),
            "docs/search".to_string(),
        ]
    );
    assert!(
        elapsed_ms <= max_total_ms,
        "query-wrapper render hint projection cold functional path exceeded benchmark max_total={} observed={}ms",
        benchmark.max_total,
        elapsed_ms
    );

    let observed_total = duration_literal(elapsed);
    let performance_gate = serde_json::json!({
        "schemaId": "agent.semantic-protocols.semantic-hot-path-performance-gate",
        "schemaVersion": "1",
        "scenarioId": "asp-query-wrapper-render-hint-projection-cold-functional-path",
        "languageId": "rust",
        "workspace": ".",
        "command": [
            "agent_semantic_search::query_wrapper_owner_candidates",
            "agent_semantic_search::query_wrapper_package_clusters_from_paths",
            "agent_semantic_search::query_wrapper_rg_scope_next"
        ],
        "phase": "cold",
        "expected": {
            "targetTotal": benchmark.target_total,
            "maxTotal": benchmark.max_total,
            "regressionBudget": benchmark.regression_budget,
            "maxProviderProcessCount": 0,
            "maxSearchOverlayProcessCount": 0,
            "maxStdoutBytes": benchmark.max_stdout_bytes,
            "requireSearchOwnedProjection": true,
            "allowedFirstRoutes": ["query-wrapper-render-hint-projection"],
            "forbiddenRoutes": ["command-local-package-key", "native-finder", "provider-process"],
            "requireNoExecutableLineRange": true
        },
        "observed": {
            "observedTotal": observed_total,
            "providerProcessCount": 0,
            "providerElapsed": "0us",
            "nativeFinderProcessCount": 0,
            "nativeFinderElapsed": "0us",
            "ownerCandidateCount": owner_candidates.len(),
            "packageClusterCount": package_clusters.len(),
            "rgScopeNextCount": rg_scope_next.len(),
            "firstRoute": "query-wrapper-render-hint-projection",
            "executedRoutes": ["query-wrapper-render-hint-projection"],
            "executableLineRangeSelectorCount": 0,
            "stdoutBytes": 0,
            "fallbackReason": "none"
        },
        "verdict": "pass",
        "evidenceRefs": ["scenario:asp-query-wrapper-render-hint-projection-cold-functional-path"]
    });
    assert_eq!(performance_gate["observed"]["providerProcessCount"], 0);
    assert_eq!(performance_gate["observed"]["nativeFinderProcessCount"], 0);
    assert_eq!(performance_gate["observed"]["ownerCandidateCount"], 4);
    assert_eq!(performance_gate["observed"]["packageClusterCount"], 4);
    assert_eq!(performance_gate["observed"]["rgScopeNextCount"], 3);
}

pub(super) fn asp_query_wrapper_source_index_trace_projection_cold_functional_path_stays_inside_scenario_gate()
 {
    let crate_root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let scenario_root = crate_root
        .join("tests")
        .join("unit")
        .join("scenarios")
        .join("asp_query_wrapper_source_index_trace_projection_cold_functional_path");
    let benchmark: SharedBenchmarkToml = read_toml(&scenario_root.join("benchmark.toml"));
    assert_query_wrapper_source_index_trace_projection_benchmark_contract(&benchmark);
    let max_total_ms = duration_millis_from_manifest(&benchmark.max_total);

    let started_at = Instant::now();
    let hit = agent_semantic_search::QueryWrapperSearchSourceIndexTrace {
        lookup: agent_semantic_search::QueryWrapperSourceIndexLookup::new(
            PathBuf::from("live/client/client.turso"),
            "hit",
            Vec::new(),
        ),
        candidate_count: 2,
        elapsed: std::time::Duration::from_micros(750),
    };
    let hit_projection = agent_semantic_search::query_wrapper_source_index_trace_projection(&hit);
    let missing_db = agent_semantic_search::QueryWrapperSearchSourceIndexTrace {
        lookup: agent_semantic_search::QueryWrapperSourceIndexLookup::new(
            PathBuf::from("live/client/client.turso"),
            "missing-db",
            Vec::new(),
        ),
        candidate_count: 0,
        elapsed: std::time::Duration::from_millis(3),
    };
    let missing_projection =
        agent_semantic_search::query_wrapper_source_index_trace_projection(&missing_db);
    let elapsed = started_at.elapsed();
    let elapsed_ms = elapsed.as_millis();

    assert_eq!(hit_projection.source, "sourceIndex");
    assert_eq!(hit_projection.status, "used");
    assert_eq!(hit_projection.candidate_count, 2);
    assert_eq!(hit_projection.skipped_count, 0);
    assert!(!hit_projection.fields.contains_key("nextCommand"));
    assert_eq!(missing_projection.status, "missing-db");
    assert_eq!(missing_projection.skipped_count, 1);
    assert_eq!(
        missing_projection.fields["nextCommand"],
        serde_json::json!("asp cache source-index refresh")
    );
    assert!(
        elapsed_ms <= max_total_ms,
        "query-wrapper source-index trace projection cold functional path exceeded benchmark max_total={} observed={}ms",
        benchmark.max_total,
        elapsed_ms
    );

    let observed_total = duration_literal(elapsed);
    let performance_gate = serde_json::json!({
        "schemaId": "agent.semantic-protocols.semantic-hot-path-performance-gate",
        "schemaVersion": "1",
        "scenarioId": "asp-query-wrapper-source-index-trace-projection-cold-functional-path",
        "languageId": "rust",
        "workspace": ".",
        "command": [
            "agent_semantic_search::query_wrapper_source_index_trace_projection"
        ],
        "phase": "cold",
        "expected": {
            "targetTotal": benchmark.target_total,
            "maxTotal": benchmark.max_total,
            "regressionBudget": benchmark.regression_budget,
            "maxProviderProcessCount": 0,
            "maxSearchOverlayProcessCount": 0,
            "maxStdoutBytes": benchmark.max_stdout_bytes,
            "requireSearchOwnedProjection": true,
            "allowedFirstRoutes": ["query-wrapper-source-index-trace-projection"],
            "forbiddenRoutes": ["command-local-source-index-status", "native-finder", "provider-process"],
            "requireRefreshHintOnMiss": true
        },
        "observed": {
            "observedTotal": observed_total,
            "providerProcessCount": 0,
            "providerElapsed": "0us",
            "nativeFinderProcessCount": 0,
            "nativeFinderElapsed": "0us",
            "hitStatus": hit_projection.status,
            "missingDbStatus": missing_projection.status,
            "missingDbNextCommand": missing_projection.fields["nextCommand"],
            "firstRoute": "query-wrapper-source-index-trace-projection",
            "executedRoutes": ["query-wrapper-source-index-trace-projection"],
            "stdoutBytes": 0,
            "fallbackReason": "none"
        },
        "verdict": "pass",
        "evidenceRefs": ["scenario:asp-query-wrapper-source-index-trace-projection-cold-functional-path"]
    });
    assert_eq!(performance_gate["observed"]["providerProcessCount"], 0);
    assert_eq!(performance_gate["observed"]["nativeFinderProcessCount"], 0);
    assert_eq!(performance_gate["observed"]["hitStatus"], "used");
    assert_eq!(
        performance_gate["observed"]["missingDbNextCommand"],
        "asp cache source-index refresh"
    );
}

pub(super) fn asp_query_wrapper_clause_normalization_cold_functional_path_stays_inside_scenario_gate()
 {
    let crate_root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let scenario_root = crate_root
        .join("tests")
        .join("unit")
        .join("scenarios")
        .join("asp_query_wrapper_clause_normalization_cold_functional_path");
    let benchmark: SharedBenchmarkToml = read_toml(&scenario_root.join("benchmark.toml"));
    assert_query_wrapper_clause_normalization_benchmark_contract(&benchmark);
    let max_total_ms = duration_millis_from_manifest(&benchmark.max_total);

    let raw_queries = vec![
        "CacheStatus cache_status".to_string(),
        "HTTPServer owner".to_string(),
        "   ".to_string(),
    ];
    let started_at = Instant::now();
    let clauses = agent_semantic_search::query_wrapper_clauses(&raw_queries);
    let terms = agent_semantic_search::query_wrapper_unique_clause_terms(&clauses);
    let elapsed = started_at.elapsed();
    let elapsed_ms = elapsed.as_millis();

    assert_eq!(clauses.len(), 2);
    assert_eq!(clauses[0].id, 1);
    assert_eq!(clauses[0].raw, "CacheStatus cache_status");
    assert!(clauses[0].axis_terms.contains(&"cache".to_string()));
    assert!(clauses[0].axis_terms.contains(&"status".to_string()));
    assert_eq!(
        terms,
        vec![
            "cachestatus".to_string(),
            "cache_status".to_string(),
            "httpserver".to_string(),
            "owner".to_string(),
        ]
    );
    assert!(
        elapsed_ms <= max_total_ms,
        "query-wrapper clause normalization cold functional path exceeded benchmark max_total={} observed={}ms clauses={:?}",
        benchmark.max_total,
        elapsed_ms,
        clauses
    );

    let observed_total = duration_literal(elapsed);
    let performance_gate = serde_json::json!({
        "schemaId": "agent.semantic-protocols.semantic-hot-path-performance-gate",
        "schemaVersion": "1",
        "scenarioId": "asp-query-wrapper-clause-normalization-cold-functional-path",
        "languageId": "rust",
        "workspace": ".",
        "command": [
            "agent_semantic_search::query_wrapper_clauses",
            "agent_semantic_search::query_wrapper_unique_clause_terms"
        ],
        "phase": "cold",
        "expected": {
            "targetTotal": benchmark.target_total,
            "maxTotal": benchmark.max_total,
            "regressionBudget": benchmark.regression_budget,
            "maxProviderProcessCount": 0,
            "maxSearchOverlayProcessCount": 0,
            "maxStdoutBytes": benchmark.max_stdout_bytes,
            "requireSearchOwnedClauseNormalization": true,
            "allowedFirstRoutes": ["query-wrapper-clause-normalization"],
            "forbiddenRoutes": ["command-local-query-terms", "native-finder", "provider-process"],
            "requireIdentifierAxisExpansion": true
        },
        "observed": {
            "observedTotal": observed_total,
            "providerProcessCount": 0,
            "providerElapsed": "0us",
            "nativeFinderProcessCount": 0,
            "nativeFinderElapsed": "0us",
            "clauseCount": clauses.len(),
            "uniqueTermCount": terms.len(),
            "firstRoute": "query-wrapper-clause-normalization",
            "executedRoutes": ["query-wrapper-clause-normalization"],
            "stdoutBytes": 0,
            "fallbackReason": "none"
        },
        "verdict": "pass",
        "evidenceRefs": ["scenario:asp-query-wrapper-clause-normalization-cold-functional-path"]
    });
    assert_eq!(performance_gate["observed"]["providerProcessCount"], 0);
    assert_eq!(performance_gate["observed"]["nativeFinderProcessCount"], 0);
    assert_eq!(performance_gate["observed"]["clauseCount"], 2);
    assert_eq!(performance_gate["observed"]["uniqueTermCount"], 4);
}

pub(super) fn asp_search_query_budget_cold_functional_path_stays_inside_scenario_gate() {
    let crate_root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let scenario_root = crate_root
        .join("tests")
        .join("unit")
        .join("scenarios")
        .join("asp_search_query_budget_cold_functional_path");
    let benchmark: SharedBenchmarkToml = read_toml(&scenario_root.join("benchmark.toml"));
    assert_search_query_budget_benchmark_contract(&benchmark);
    let max_total_ms = duration_millis_from_manifest(&benchmark.max_total);

    let started_at = Instant::now();
    let block = agent_semantic_search::search_query_budget_block(
        "search query budget block generic provider",
        &[PathBuf::from(".")],
        false,
    )
    .expect("broad generic query should be blocked");
    let specific_terms =
        agent_semantic_search::search_query_terms("CacheStatus cache_status src/lib.rs");
    let specific_allowed = agent_semantic_search::search_terms_budget_block(
        &specific_terms,
        &[PathBuf::from(".")],
        false,
    )
    .is_none();
    let filtered_allowed = agent_semantic_search::search_query_budget_block(
        "search query budget block generic provider",
        &[PathBuf::from(".")],
        true,
    )
    .is_none();
    let rg_block = agent_semantic_search::search_rg_terms_budget_block(
        &[
            "alpha".to_string(),
            "beta".to_string(),
            "gamma".to_string(),
            "delta".to_string(),
            "epsilon".to_string(),
        ],
        &[],
        false,
    )
    .expect("rg broad term budget should block");
    let elapsed = started_at.elapsed();
    let elapsed_ms = elapsed.as_millis();

    assert_eq!(block.reason, "query-too-broad");
    assert_eq!(block.term_count, 6);
    assert!(specific_allowed);
    assert!(filtered_allowed);
    assert_eq!(rg_block.reason, "query-too-broad");
    assert!(
        elapsed_ms <= max_total_ms,
        "search query budget cold functional path exceeded benchmark max_total={} observed={}ms block={:?}",
        benchmark.max_total,
        elapsed_ms,
        block
    );

    let observed_total = duration_literal(elapsed);
    let performance_gate = serde_json::json!({
        "schemaId": "agent.semantic-protocols.semantic-hot-path-performance-gate",
        "schemaVersion": "1",
        "scenarioId": "asp-search-query-budget-cold-functional-path",
        "languageId": "rust",
        "workspace": ".",
        "command": [
            "agent_semantic_search::search_query_budget_block",
            "agent_semantic_search::search_terms_budget_block",
            "agent_semantic_search::search_rg_terms_budget_block"
        ],
        "phase": "cold",
        "expected": {
            "targetTotal": benchmark.target_total,
            "maxTotal": benchmark.max_total,
            "regressionBudget": benchmark.regression_budget,
            "maxProviderProcessCount": 0,
            "maxSearchOverlayProcessCount": 0,
            "maxStdoutBytes": benchmark.max_stdout_bytes,
            "requireSearchOwnedBudget": true,
            "allowedFirstRoutes": ["search-query-budget"],
            "forbiddenRoutes": ["command-local-query-budget", "native-finder", "provider-process"],
            "requireSpecificTermBypass": true
        },
        "observed": {
            "observedTotal": observed_total,
            "providerProcessCount": 0,
            "providerElapsed": "0us",
            "nativeFinderProcessCount": 0,
            "nativeFinderElapsed": "0us",
            "budgetReason": block.reason,
            "genericTermCount": block.generic_terms.len(),
            "specificAllowed": specific_allowed,
            "filteredAllowed": filtered_allowed,
            "rgBudgetReason": rg_block.reason,
            "firstRoute": "search-query-budget",
            "executedRoutes": ["search-query-budget"],
            "stdoutBytes": 0,
            "fallbackReason": "none"
        },
        "verdict": "pass",
        "evidenceRefs": ["scenario:asp-search-query-budget-cold-functional-path"]
    });
    assert_eq!(performance_gate["observed"]["providerProcessCount"], 0);
    assert_eq!(performance_gate["observed"]["nativeFinderProcessCount"], 0);
    assert_eq!(performance_gate["observed"]["specificAllowed"], true);
    assert_eq!(performance_gate["observed"]["filteredAllowed"], true);
}

pub(super) fn asp_search_pipe_generated_candidate_cold_functional_path_stays_inside_scenario_gate()
{
    let crate_root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let scenario_root = crate_root
        .join("tests")
        .join("unit")
        .join("scenarios")
        .join("asp_search_pipe_generated_candidate_cold_functional_path");
    let benchmark: SharedBenchmarkToml = read_toml(&scenario_root.join("benchmark.toml"));
    assert_generated_candidate_benchmark_contract(&benchmark);
    let max_total_ms = duration_millis_from_manifest(&benchmark.max_total);

    let started_at = Instant::now();
    let candidates = vec![
        agent_semantic_search::GraphCandidateSparsityInput::new(
            "src/generated/lib.rs",
            "HookDecision",
        ),
        agent_semantic_search::GraphCandidateSparsityInput::new(
            "src/domain/model.rs",
            "ClientReceipt",
        ),
    ];
    let selected = agent_semantic_search::select_sparse_graph_candidate_indices(&candidates, 8);
    let elapsed = started_at.elapsed();
    let elapsed_ms = elapsed.as_millis();
    assert_eq!(selected, vec![0, 1]);
    assert!(
        elapsed_ms <= max_total_ms,
        "search pipe generated candidate cold functional path exceeded benchmark max_total={} observed={}ms selected={:?}",
        benchmark.max_total,
        elapsed_ms,
        selected
    );

    let performance_gate = serde_json::json!({
        "schemaId": "agent.semantic-protocols.semantic-hot-path-performance-gate",
        "schemaVersion": "1",
        "scenarioId": "asp-search-pipe-generated-candidate-cold-functional-path",
        "languageId": "rust",
        "workspace": ".",
        "command": ["agent_semantic_search::select_sparse_graph_candidate_indices"],
        "phase": "cold",
        "expected": {
            "targetTotal": benchmark.target_total,
            "maxTotal": benchmark.max_total,
            "regressionBudget": benchmark.regression_budget,
            "maxProviderProcessCount": 0,
            "maxStdoutBytes": benchmark.max_stdout_bytes,
            "allowedFirstRoutes": ["search-pipe-generated-candidate"],
            "forbiddenRoutes": ["provider-process", "path-generated-filter"],
            "requireGeneratedCandidateRetained": true
        },
        "observed": {
            "observedTotal": duration_literal(elapsed),
            "providerProcessCount": 0,
            "providerElapsed": "0us",
            "firstRoute": "search-pipe-generated-candidate",
            "executedRoutes": ["search-pipe-generated-candidate"],
            "generatedCandidateRetained": true,
            "stdoutBytes": 0,
            "fallbackReason": "none"
        },
        "verdict": "pass",
        "evidenceRefs": ["scenario:asp-search-pipe-generated-candidate-cold-functional-path"]
    });
    assert_eq!(performance_gate["observed"]["providerProcessCount"], 0);
    assert_eq!(
        performance_gate["observed"]["generatedCandidateRetained"],
        true
    );
}

pub(super) fn asp_provider_candidate_annotations_cold_functional_path_stays_inside_scenario_gate() {
    let crate_root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let scenario_root = crate_root
        .join("tests")
        .join("unit")
        .join("scenarios")
        .join("asp_provider_candidate_annotations_cold_functional_path");
    let benchmark: SharedBenchmarkToml = read_toml(&scenario_root.join("benchmark.toml"));
    assert_provider_candidate_annotations_benchmark_contract(&benchmark);
    let max_total_ms = duration_millis_from_manifest(&benchmark.max_total);

    let annotations = vec![serde_json::json!({
        "path": "src/generated/lib.rs",
        "attributes": ["generated", "schema-generated"],
        "source": "rust-harness",
        "reason": "provider-parser-fact"
    })];
    let provider_nodes = vec![serde_json::json!({
        "id": "field:src/generated/lib.rs-items",
        "kind": "field",
        "role": "class-field",
        "value": "items: list[str]",
        "matchText": "Bag.items: list[str]\nfull provider detail"
    })];
    let stdout = br#"[agent-semantic-client] syncing generated activation
{"nodes":[{"id":"field:src/generated/lib.rs-items","kind":"field","role":"class-field","value":"items: list[str]","action":"code"}],"edges":[],"candidateAnnotations":[{"path":"src/generated/lib.rs","attributes":["generated","schema-generated"],"source":"rust-harness","reason":"provider-parser-fact"}]}
"#;
    let started_at = Instant::now();
    let envelope =
        agent_semantic_search::provider_facts_envelope_from_stdout(stdout).expect("envelope");
    let nodes = agent_semantic_search::provider_candidate_annotation_nodes(&annotations);
    let compact_nodes = agent_semantic_search::compact_provider_fact_nodes(&provider_nodes);
    let elapsed = started_at.elapsed();
    let elapsed_ms = elapsed.as_millis();

    assert_eq!(envelope.nodes.len(), 1);
    assert_eq!(envelope.candidate_annotations.len(), 1);
    assert_eq!(nodes.len(), 1);
    assert_eq!(nodes[0]["kind"], "provider-candidate-annotation");
    assert_eq!(nodes[0]["role"], "file-attributes");
    assert_eq!(nodes[0]["path"], "src/generated/lib.rs");
    assert_eq!(nodes[0]["fields"]["attributes"][0], "generated");
    assert_eq!(compact_nodes[0]["value"], "items");
    assert_eq!(compact_nodes[0]["matchText"], "Bag.items");
    assert!(
        elapsed_ms <= max_total_ms,
        "provider candidate annotations cold functional path exceeded benchmark max_total={} observed={}ms nodes={:?}",
        benchmark.max_total,
        elapsed_ms,
        nodes
    );

    let performance_gate = serde_json::json!({
        "schemaId": "agent.semantic-protocols.semantic-hot-path-performance-gate",
        "schemaVersion": "1",
        "scenarioId": "asp-provider-candidate-annotations-cold-functional-path",
        "languageId": "rust",
        "workspace": ".",
        "command": ["agent_semantic_search::provider_candidate_annotation_nodes"],
        "phase": "cold",
        "expected": {
            "targetTotal": benchmark.target_total,
            "maxTotal": benchmark.max_total,
            "regressionBudget": benchmark.regression_budget,
            "maxProviderProcessCount": 0,
            "maxStdoutBytes": benchmark.max_stdout_bytes,
            "allowedFirstRoutes": ["provider-candidate-annotations"],
            "forbiddenRoutes": ["command-local-generated-policy", "path-generated-filter"],
            "requireProviderOwnedAttributes": true,
            "requireSearchOwnedCompaction": true,
            "requireSearchOwnedStdoutExtraction": true
        },
        "observed": {
            "observedTotal": duration_literal(elapsed),
            "providerProcessCount": 0,
            "providerElapsed": "0us",
            "firstRoute": "provider-candidate-annotations",
            "executedRoutes": ["provider-candidate-annotations"],
            "providerOwnedAttributes": true,
            "searchOwnedCompaction": true,
            "searchOwnedStdoutExtraction": true,
            "stdoutBytes": 0,
            "fallbackReason": "none"
        },
        "verdict": "pass",
        "evidenceRefs": ["scenario:asp-provider-candidate-annotations-cold-functional-path"]
    });
    assert_eq!(performance_gate["observed"]["providerProcessCount"], 0);
    assert_eq!(
        performance_gate["observed"]["providerOwnedAttributes"],
        true
    );
    assert_eq!(performance_gate["observed"]["searchOwnedCompaction"], true);
    assert_eq!(
        performance_gate["observed"]["searchOwnedStdoutExtraction"],
        true
    );
}

pub(super) fn asp_graph_node_projection_cold_functional_path_stays_inside_scenario_gate() {
    let crate_root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let scenario_root = crate_root
        .join("tests")
        .join("unit")
        .join("scenarios")
        .join("asp_graph_node_projection_cold_functional_path");
    let benchmark: SharedBenchmarkToml = read_toml(&scenario_root.join("benchmark.toml"));
    assert_graph_node_projection_benchmark_contract(&benchmark);
    let max_total_ms = duration_millis_from_manifest(&benchmark.max_total);

    let owners = vec![
        "Src/Generated Lib.rs".to_string(),
        "src/domain/model.rs".to_string(),
    ];
    let started_at = Instant::now();
    let empty_id = agent_semantic_search::stable_graph_node_id("owner", "!!!");
    let nodes = agent_semantic_search::owner_path_graph_nodes(&owners);
    let elapsed = started_at.elapsed();
    let elapsed_ms = elapsed.as_millis();

    assert_eq!(empty_id, "owner:node");
    assert_eq!(nodes.len(), 2);
    assert_eq!(nodes[0]["id"], "owner:src/generated-lib.rs");
    assert_eq!(nodes[0]["kind"], "owner");
    assert_eq!(nodes[0]["role"], "path");
    assert_eq!(nodes[0]["action"], "owner");
    assert_eq!(nodes[0]["path"], "Src/Generated Lib.rs");
    assert!(
        nodes
            .iter()
            .all(|node| !node["id"].as_str().unwrap_or("").contains(":1:")),
        "graph node projection must not encode executable line ranges; nodes={nodes:?}"
    );
    assert!(
        elapsed_ms <= max_total_ms,
        "graph node projection cold functional path exceeded benchmark max_total={} observed={}ms nodes={:?}",
        benchmark.max_total,
        elapsed_ms,
        nodes
    );

    let performance_gate = serde_json::json!({
        "schemaId": "agent.semantic-protocols.semantic-hot-path-performance-gate",
        "schemaVersion": "1",
        "scenarioId": "asp-graph-node-projection-cold-functional-path",
        "languageId": "rust",
        "workspace": ".",
        "command": [
            "agent_semantic_search::stable_graph_node_id",
            "agent_semantic_search::owner_path_graph_nodes"
        ],
        "phase": "cold",
        "expected": {
            "targetTotal": benchmark.target_total,
            "maxTotal": benchmark.max_total,
            "regressionBudget": benchmark.regression_budget,
            "maxProviderProcessCount": 0,
            "maxSearchOverlayProcessCount": 0,
            "maxStdoutBytes": benchmark.max_stdout_bytes,
            "requireSearchOwnedProjection": true,
            "allowedFirstRoutes": ["graph-node-projection"],
            "forbiddenRoutes": ["command-owner-node-builder", "provider-process", "path-generated-filter"],
            "requireNoExecutableLineRange": true
        },
        "observed": {
            "observedTotal": duration_literal(elapsed),
            "providerProcessCount": 0,
            "providerElapsed": "0us",
            "nativeFinderProcessCount": 0,
            "nativeFinderElapsed": "0us",
            "ownerNodeCount": nodes.len(),
            "firstRoute": "graph-node-projection",
            "executedRoutes": ["graph-node-projection"],
            "executableLineRangeSelectorCount": 0,
            "stdoutBytes": 0,
            "fallbackReason": "none"
        },
        "verdict": "pass",
        "evidenceRefs": ["scenario:asp-graph-node-projection-cold-functional-path"]
    });
    assert_eq!(performance_gate["observed"]["providerProcessCount"], 0);
    assert_eq!(performance_gate["observed"]["nativeFinderProcessCount"], 0);
    assert_eq!(performance_gate["observed"]["ownerNodeCount"], 2);
}

pub(super) fn asp_graph_candidate_projection_cold_functional_path_stays_inside_scenario_gate() {
    let crate_root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let scenario_root = crate_root
        .join("tests")
        .join("unit")
        .join("scenarios")
        .join("asp_graph_candidate_projection_cold_functional_path");
    let benchmark: SharedBenchmarkToml = read_toml(&scenario_root.join("benchmark.toml"));
    assert_graph_candidate_projection_benchmark_contract(&benchmark);
    let max_total_ms = duration_millis_from_manifest(&benchmark.max_total);

    let candidates = vec![agent_semantic_search::GraphProjectionCandidate::new(
        "src/lib.rs",
        3,
        4,
        "SearchOwner",
        "pub fn SearchOwner() {}",
        "source-index",
        "high",
    )];
    let started_at = Instant::now();
    let item_nodes = agent_semantic_search::graph_candidate_item_nodes("rust", &candidates, 8);
    let hot_nodes = agent_semantic_search::graph_candidate_hot_nodes("rust", &candidates, 8);
    let elapsed = started_at.elapsed();
    let elapsed_ms = elapsed.as_millis();

    assert_eq!(item_nodes.len(), 1);
    assert_eq!(hot_nodes.len(), 1);
    assert_eq!(
        item_nodes[0]["structuralSelector"],
        "rust://src/lib.rs#item/symbol/SearchOwner"
    );
    assert_eq!(
        hot_nodes[0]["structuralSelector"],
        "rust://src/lib.rs#range/hot/SearchOwner"
    );
    assert_eq!(item_nodes[0]["displayLineRange"], "3:4");
    assert_eq!(hot_nodes[0]["startLine"], 1);
    assert_eq!(hot_nodes[0]["endLine"], 15);
    assert_eq!(hot_nodes[0]["codePolicy"], "requires-exact-code");
    assert!(
        item_nodes
            .iter()
            .chain(hot_nodes.iter())
            .all(|node| !node["structuralSelector"]
                .as_str()
                .unwrap_or("")
                .contains(":3:")),
        "graph candidate projection must not expose executable line ranges as structural identity; item_nodes={item_nodes:?} hot_nodes={hot_nodes:?}"
    );
    assert!(
        elapsed_ms <= max_total_ms,
        "graph candidate projection cold functional path exceeded benchmark max_total={} observed={}ms item_nodes={:?} hot_nodes={:?}",
        benchmark.max_total,
        elapsed_ms,
        item_nodes,
        hot_nodes
    );

    let performance_gate = serde_json::json!({
        "schemaId": "agent.semantic-protocols.semantic-hot-path-performance-gate",
        "schemaVersion": "1",
        "scenarioId": "asp-graph-candidate-projection-cold-functional-path",
        "languageId": "rust",
        "workspace": ".",
        "command": [
            "agent_semantic_search::graph_candidate_item_nodes",
            "agent_semantic_search::graph_candidate_hot_nodes"
        ],
        "phase": "cold",
        "expected": {
            "targetTotal": benchmark.target_total,
            "maxTotal": benchmark.max_total,
            "regressionBudget": benchmark.regression_budget,
            "maxProviderProcessCount": 0,
            "maxSearchOverlayProcessCount": 0,
            "maxStdoutBytes": benchmark.max_stdout_bytes,
            "requireSearchOwnedProjection": true,
            "allowedFirstRoutes": ["graph-candidate-projection"],
            "forbiddenRoutes": ["command-candidate-node-builder", "provider-process", "native-finder"],
            "requireNoExecutableLineRange": true
        },
        "observed": {
            "observedTotal": duration_literal(elapsed),
            "providerProcessCount": 0,
            "providerElapsed": "0us",
            "nativeFinderProcessCount": 0,
            "nativeFinderElapsed": "0us",
            "itemNodeCount": item_nodes.len(),
            "hotNodeCount": hot_nodes.len(),
            "firstRoute": "graph-candidate-projection",
            "executedRoutes": ["graph-candidate-projection"],
            "executableLineRangeSelectorCount": 0,
            "stdoutBytes": 0,
            "fallbackReason": "none"
        },
        "verdict": "pass",
        "evidenceRefs": ["scenario:asp-graph-candidate-projection-cold-functional-path"]
    });
    assert_eq!(performance_gate["observed"]["providerProcessCount"], 0);
    assert_eq!(performance_gate["observed"]["nativeFinderProcessCount"], 0);
    assert_eq!(performance_gate["observed"]["itemNodeCount"], 1);
    assert_eq!(performance_gate["observed"]["hotNodeCount"], 1);
}

pub(super) fn asp_graph_topology_projection_cold_functional_path_stays_inside_scenario_gate() {
    let crate_root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let scenario_root = crate_root
        .join("tests")
        .join("unit")
        .join("scenarios")
        .join("asp_graph_topology_projection_cold_functional_path");
    let benchmark: SharedBenchmarkToml = read_toml(&scenario_root.join("benchmark.toml"));
    assert_graph_topology_projection_benchmark_contract(&benchmark);
    let max_total_ms = duration_millis_from_manifest(&benchmark.max_total);

    let root = temp_project_root("scenario-graph-topology-projection-cold");
    fs::create_dir_all(root.join("src")).expect("create src");
    fs::create_dir_all(root.join("languages/rust/src")).expect("create submodule source");
    fs::write(
        root.join("Cargo.toml"),
        "[package]\nname = \"scenario-graph-topology-projection-cold\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
    )
    .expect("write cargo manifest");
    fs::write(root.join("Cargo.lock"), "# lock\n").expect("write cargo lock");
    fs::write(root.join("src/lib.rs"), "pub fn topology_fixture() {}\n").expect("write source");
    fs::write(
        root.join(".gitmodules"),
        "[submodule \"languages/rust\"]\n  path = languages/rust\n  url = https://example.invalid/rust.git\n",
    )
    .expect("write gitmodules");

    let candidates = vec![agent_semantic_search::GraphProjectionCandidate::new(
        "src/lib.rs",
        1,
        1,
        "topology_fixture",
        "pub fn topology_fixture() {}",
        "source-index",
        "high",
    )];
    let owners = vec!["languages/rust/src/lib.rs".to_string()];
    let started_at = Instant::now();
    let projection =
        agent_semantic_search::graph_project_topology_projection("rust", &root, &candidates);
    let owner_edges = agent_semantic_search::graph_submodule_owner_edges(&root, &owners);
    let elapsed = started_at.elapsed();
    let elapsed_ms = elapsed.as_millis();

    assert!(
        projection
            .nodes
            .iter()
            .any(|node| { node["kind"] == "language-project" && node["path"] == "." })
    );
    assert!(
        projection
            .nodes
            .iter()
            .any(|node| { node["kind"] == "project-marker" && node["path"] == "Cargo.toml" })
    );
    assert!(
        projection
            .nodes
            .iter()
            .any(|node| { node["kind"] == "dependency-marker" && node["path"] == "Cargo.lock" })
    );
    assert!(
        projection
            .nodes
            .iter()
            .any(|node| { node["kind"] == "submodule" && node["path"] == "languages/rust" })
    );
    assert_eq!(owner_edges.len(), 1);
    assert_eq!(owner_edges[0]["relation"], "contains");
    assert!(
        elapsed_ms <= max_total_ms,
        "graph topology projection cold functional path exceeded benchmark max_total={} observed={}ms projection={projection:?} owner_edges={owner_edges:?}",
        benchmark.max_total,
        elapsed_ms
    );

    let performance_gate = serde_json::json!({
        "schemaId": "agent.semantic-protocols.semantic-hot-path-performance-gate",
        "schemaVersion": "1",
        "scenarioId": "asp-graph-topology-projection-cold-functional-path",
        "languageId": "rust",
        "workspace": ".",
        "command": [
            "agent_semantic_search::graph_project_topology_projection",
            "agent_semantic_search::graph_submodule_owner_edges"
        ],
        "phase": "cold",
        "expected": {
            "targetTotal": benchmark.target_total,
            "maxTotal": benchmark.max_total,
            "regressionBudget": benchmark.regression_budget,
            "maxProviderProcessCount": 0,
            "maxSearchOverlayProcessCount": 0,
            "maxStdoutBytes": benchmark.max_stdout_bytes,
            "requireSearchOwnedProjection": true,
            "allowedFirstRoutes": ["graph-topology-projection"],
            "forbiddenRoutes": ["command-project-marker-walk", "command-gitmodules-parser", "provider-process", "native-finder"],
            "requireProviderManifestMarkers": true
        },
        "observed": {
            "observedTotal": duration_literal(elapsed),
            "providerProcessCount": 0,
            "providerElapsed": "0us",
            "nativeFinderProcessCount": 0,
            "nativeFinderElapsed": "0us",
            "nodeCount": projection.nodes.len(),
            "edgeCount": projection.edges.len() + owner_edges.len(),
            "firstRoute": "graph-topology-projection",
            "executedRoutes": ["graph-topology-projection"],
            "stdoutBytes": 0,
            "fallbackReason": "none"
        },
        "verdict": "pass",
        "evidenceRefs": ["scenario:asp-graph-topology-projection-cold-functional-path"]
    });
    assert_eq!(performance_gate["observed"]["providerProcessCount"], 0);
    assert_eq!(performance_gate["observed"]["nativeFinderProcessCount"], 0);
    assert_eq!(
        performance_gate["observed"]["firstRoute"],
        "graph-topology-projection"
    );
    let _ = fs::remove_dir_all(root);
}

pub(super) fn asp_graph_owner_rank_cold_functional_path_stays_inside_scenario_gate() {
    let crate_root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let scenario_root = crate_root
        .join("tests")
        .join("unit")
        .join("scenarios")
        .join("asp_graph_owner_rank_cold_functional_path");
    let benchmark: SharedBenchmarkToml = read_toml(&scenario_root.join("benchmark.toml"));
    assert_graph_owner_rank_benchmark_contract(&benchmark);
    let max_total_ms = duration_millis_from_manifest(&benchmark.max_total);

    let candidates = vec![
        agent_semantic_search::GraphProjectionCandidate::new(
            "src/lib.rs",
            1,
            1,
            "SearchRouter",
            "dynamic overlay graph ranking",
            "source-index",
            "high",
        ),
        agent_semantic_search::GraphProjectionCandidate::new(
            "languages/rust/src/lib.rs",
            1,
            1,
            "SearchRouter",
            "dynamic overlay graph ranking",
            "source-index",
            "high",
        ),
        agent_semantic_search::GraphProjectionCandidate::new(
            "packages/runtime/search/src/router.rs",
            1,
            1,
            "SearchRouter",
            "dynamic overlay graph ranking",
            "finder-path",
            "path",
        ),
    ];
    let query_terms = vec!["dynamicOverlay".to_string(), "SearchRouter".to_string()];
    let submodule_paths = vec!["languages/rust".to_string()];
    let started_at = Instant::now();
    let ranked = agent_semantic_search::ranked_graph_owner_paths_for_submodule_paths(
        &candidates,
        &query_terms,
        &submodule_paths,
    );
    let elapsed = started_at.elapsed();
    let elapsed_ms = elapsed.as_millis();

    assert_eq!(ranked[0], "languages/rust/src/lib.rs");
    assert!(
        ranked
            .iter()
            .any(|path| path == "packages/runtime/search/src/router.rs"),
        "ranked owners={ranked:?}"
    );
    assert!(
        elapsed_ms <= max_total_ms,
        "graph owner rank cold functional path exceeded benchmark max_total={} observed={}ms ranked={ranked:?}",
        benchmark.max_total,
        elapsed_ms
    );

    let performance_gate = serde_json::json!({
        "schemaId": "agent.semantic-protocols.semantic-hot-path-performance-gate",
        "schemaVersion": "1",
        "scenarioId": "asp-graph-owner-rank-cold-functional-path",
        "languageId": "rust",
        "workspace": ".",
        "command": [
            "agent_semantic_search::ranked_graph_owner_paths_for_submodule_paths"
        ],
        "phase": "cold",
        "expected": {
            "targetTotal": benchmark.target_total,
            "maxTotal": benchmark.max_total,
            "regressionBudget": benchmark.regression_budget,
            "maxProviderProcessCount": 0,
            "maxSearchOverlayProcessCount": 0,
            "maxStdoutBytes": benchmark.max_stdout_bytes,
            "requireSearchOwnedRank": true,
            "allowedFirstRoutes": ["graph-owner-rank"],
            "forbiddenRoutes": ["command-owner-rank", "provider-process", "native-finder"],
            "requireTopologyMembershipBoost": true
        },
        "observed": {
            "observedTotal": duration_literal(elapsed),
            "providerProcessCount": 0,
            "providerElapsed": "0us",
            "nativeFinderProcessCount": 0,
            "nativeFinderElapsed": "0us",
            "ownerCount": ranked.len(),
            "firstOwner": ranked[0],
            "firstRoute": "graph-owner-rank",
            "executedRoutes": ["graph-owner-rank"],
            "stdoutBytes": 0,
            "fallbackReason": "none"
        },
        "verdict": "pass",
        "evidenceRefs": ["scenario:asp-graph-owner-rank-cold-functional-path"]
    });
    assert_eq!(performance_gate["observed"]["providerProcessCount"], 0);
    assert_eq!(performance_gate["observed"]["nativeFinderProcessCount"], 0);
    assert_eq!(
        performance_gate["observed"]["firstOwner"],
        "languages/rust/src/lib.rs"
    );
}

pub(super) fn asp_graph_query_owner_seed_cold_functional_path_stays_inside_scenario_gate() {
    let crate_root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let scenario_root = crate_root
        .join("tests")
        .join("unit")
        .join("scenarios")
        .join("asp_graph_query_owner_seed_cold_functional_path");
    let benchmark: SharedBenchmarkToml = read_toml(&scenario_root.join("benchmark.toml"));
    assert_graph_query_owner_seed_benchmark_contract(&benchmark);
    let max_total_ms = duration_millis_from_manifest(&benchmark.max_total);

    let candidates = vec![
        agent_semantic_search::GraphProjectionCandidate::new(
            "packages/runtime/search/src/router.rs",
            1,
            1,
            "SearchRouter",
            "dynamic overlay search",
            "package-path-query",
            "package-path",
        ),
        agent_semantic_search::GraphProjectionCandidate::new(
            "src/cache.rs",
            1,
            1,
            "CacheStatus",
            "cache status receipt",
            "source-index",
            "high",
        ),
    ];
    let owners = vec![
        "src/cache.rs".to_string(),
        "packages/runtime/search/src/router.rs".to_string(),
    ];
    let package_terms = vec!["runtime_search".to_string()];
    let cache_terms = vec!["CacheStatus".to_string()];
    let started_at = Instant::now();
    let has_package_path =
        agent_semantic_search::graph_has_package_path_candidate(&candidates, &package_terms);
    let package_seed = agent_semantic_search::graph_query_owner_seed_paths(
        &candidates,
        &owners,
        1,
        &package_terms,
    );
    let evidence_seed =
        agent_semantic_search::graph_query_owner_seed_paths(&candidates, &owners, 1, &cache_terms);
    let elapsed = started_at.elapsed();
    let elapsed_ms = elapsed.as_millis();

    assert!(has_package_path);
    assert_eq!(
        package_seed,
        vec!["packages/runtime/search/src/router.rs".to_string()]
    );
    assert_eq!(evidence_seed, vec!["src/cache.rs".to_string()]);
    assert!(
        elapsed_ms <= max_total_ms,
        "graph query owner seed cold functional path exceeded benchmark max_total={} observed={}ms package_seed={package_seed:?} evidence_seed={evidence_seed:?}",
        benchmark.max_total,
        elapsed_ms
    );

    let performance_gate = serde_json::json!({
        "schemaId": "agent.semantic-protocols.semantic-hot-path-performance-gate",
        "schemaVersion": "1",
        "scenarioId": "asp-graph-query-owner-seed-cold-functional-path",
        "languageId": "rust",
        "workspace": ".",
        "command": [
            "agent_semantic_search::graph_has_package_path_candidate",
            "agent_semantic_search::graph_query_owner_seed_paths"
        ],
        "phase": "cold",
        "expected": {
            "targetTotal": benchmark.target_total,
            "maxTotal": benchmark.max_total,
            "regressionBudget": benchmark.regression_budget,
            "maxProviderProcessCount": 0,
            "maxSearchOverlayProcessCount": 0,
            "maxStdoutBytes": benchmark.max_stdout_bytes,
            "requireSearchOwnedSeed": true,
            "allowedFirstRoutes": ["graph-query-owner-seed"],
            "forbiddenRoutes": ["command-query-owner-seed", "provider-process", "native-finder"],
            "requirePackagePathSeed": true,
            "requireIdentifierAxisSeed": true
        },
        "observed": {
            "observedTotal": duration_literal(elapsed),
            "providerProcessCount": 0,
            "providerElapsed": "0us",
            "nativeFinderProcessCount": 0,
            "nativeFinderElapsed": "0us",
            "packageSeed": package_seed[0],
            "evidenceSeed": evidence_seed[0],
            "firstRoute": "graph-query-owner-seed",
            "executedRoutes": ["graph-query-owner-seed"],
            "stdoutBytes": 0,
            "fallbackReason": "none"
        },
        "verdict": "pass",
        "evidenceRefs": ["scenario:asp-graph-query-owner-seed-cold-functional-path"]
    });
    assert_eq!(performance_gate["observed"]["providerProcessCount"], 0);
    assert_eq!(performance_gate["observed"]["nativeFinderProcessCount"], 0);
    assert_eq!(
        performance_gate["observed"]["packageSeed"],
        "packages/runtime/search/src/router.rs"
    );
}

pub(super) fn asp_graph_evidence_projection_cold_functional_path_stays_inside_scenario_gate() {
    let crate_root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let scenario_root = crate_root
        .join("tests")
        .join("unit")
        .join("scenarios")
        .join("asp_graph_evidence_projection_cold_functional_path");
    let benchmark: SharedBenchmarkToml = read_toml(&scenario_root.join("benchmark.toml"));
    assert_graph_evidence_projection_benchmark_contract(&benchmark);
    let max_total_ms = duration_millis_from_manifest(&benchmark.max_total);

    let mut topology_kinds = HashMap::new();
    topology_kinds.insert("owner:src/lib.rs".to_string(), "owner".to_string());
    topology_kinds.insert("workspace:root".to_string(), "workspace".to_string());
    topology_kinds.insert("provider:rust".to_string(), "provider-root".to_string());
    topology_kinds.insert(
        "submodule:languages/rust".to_string(),
        "submodule".to_string(),
    );
    let mut mixed_kinds = topology_kinds.clone();
    mixed_kinds.insert("item:src/lib.rs-search".to_string(), "item".to_string());

    let started_at = Instant::now();
    let topology_only =
        agent_semantic_search::graph_frontier_has_only_owner_or_topology_nodes(&topology_kinds);
    let mixed =
        agent_semantic_search::graph_frontier_has_only_owner_or_topology_nodes(&mixed_kinds);
    let elapsed = started_at.elapsed();
    let elapsed_ms = elapsed.as_millis();

    assert!(topology_only);
    assert!(!mixed);
    assert!(
        elapsed_ms <= max_total_ms,
        "graph evidence projection cold functional path exceeded benchmark max_total={} observed={}ms topology_only={topology_only} mixed={mixed}",
        benchmark.max_total,
        elapsed_ms
    );

    let performance_gate = serde_json::json!({
        "schemaId": "agent.semantic-protocols.semantic-hot-path-performance-gate",
        "schemaVersion": "1",
        "scenarioId": "asp-graph-evidence-projection-cold-functional-path",
        "languageId": "rust",
        "workspace": ".",
        "command": [
            "agent_semantic_search::graph_frontier_has_only_owner_or_topology_nodes"
        ],
        "phase": "cold",
        "expected": {
            "targetTotal": benchmark.target_total,
            "maxTotal": benchmark.max_total,
            "regressionBudget": benchmark.regression_budget,
            "maxProviderProcessCount": 0,
            "maxSearchOverlayProcessCount": 0,
            "maxStdoutBytes": benchmark.max_stdout_bytes,
            "requireSearchOwnedProjectionPredicate": true,
            "allowedFirstRoutes": ["graph-evidence-projection"],
            "forbiddenRoutes": ["command-evidence-projection", "provider-process", "native-finder"]
        },
        "observed": {
            "observedTotal": duration_literal(elapsed),
            "providerProcessCount": 0,
            "providerElapsed": "0us",
            "nativeFinderProcessCount": 0,
            "nativeFinderElapsed": "0us",
            "topologyOnly": topology_only,
            "mixed": mixed,
            "firstRoute": "graph-evidence-projection",
            "executedRoutes": ["graph-evidence-projection"],
            "stdoutBytes": 0,
            "fallbackReason": "none"
        },
        "verdict": "pass",
        "evidenceRefs": ["scenario:asp-graph-evidence-projection-cold-functional-path"]
    });
    assert_eq!(performance_gate["observed"]["providerProcessCount"], 0);
    assert_eq!(performance_gate["observed"]["nativeFinderProcessCount"], 0);
    assert_eq!(performance_gate["observed"]["topologyOnly"], true);
    assert_eq!(performance_gate["observed"]["mixed"], false);
}

pub(super) fn asp_search_pipe_package_cohesion_cold_functional_path_stays_inside_scenario_gate() {
    let crate_root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let scenario_root = crate_root
        .join("tests")
        .join("unit")
        .join("scenarios")
        .join("asp_search_pipe_package_cohesion_cold_functional_path");
    let benchmark: SharedBenchmarkToml = read_toml(&scenario_root.join("benchmark.toml"));
    assert_search_pipe_package_cohesion_benchmark_contract(&benchmark);
    let max_total_ms = duration_millis_from_manifest(&benchmark.max_total);

    let candidate_paths = [
        "packages/runtime/search/src/router.rs",
        "packages/runtime/search/src/lib.rs",
        "crates/agent-semantic-search/src/lib.rs",
    ];
    let high_value_terms = vec![
        agent_semantic_search::SearchPipeCohesionTerm::new(
            "runtime-search-route",
            "runtime-search-route",
        ),
        agent_semantic_search::SearchPipeCohesionTerm::new(
            "agent-semantic-search",
            "agent-semantic-search",
        ),
    ];
    let weak_owner = vec!["runtime-search-route".to_string()];
    let strong_owner = vec![
        "runtime-search-route".to_string(),
        "agent-semantic-search".to_string(),
    ];

    let started_at = Instant::now();
    let packages = agent_semantic_search::search_pipe_candidate_packages(
        candidate_paths.into_iter().map(str::to_string),
    );
    let weak_cohesion = agent_semantic_search::search_pipe_package_cohesion(
        &packages,
        Some(&weak_owner),
        &high_value_terms,
    );
    let strong_cohesion = agent_semantic_search::search_pipe_package_cohesion(
        &packages,
        Some(&strong_owner),
        &high_value_terms,
    );
    let elapsed = started_at.elapsed();
    let elapsed_ms = elapsed.as_millis();

    assert_eq!(
        packages,
        vec![
            "crates/agent-semantic-search".to_string(),
            "packages/runtime/search".to_string()
        ]
    );
    assert_eq!(weak_cohesion, "low");
    assert_eq!(strong_cohesion, "medium");
    assert!(
        elapsed_ms <= max_total_ms,
        "search pipe package cohesion cold functional path exceeded benchmark max_total={} observed={}ms packages={packages:?} weak={weak_cohesion} strong={strong_cohesion}",
        benchmark.max_total,
        elapsed_ms
    );

    let performance_gate = serde_json::json!({
        "schemaId": "agent.semantic-protocols.semantic-hot-path-performance-gate",
        "schemaVersion": "1",
        "scenarioId": "asp-search-pipe-package-cohesion-cold-functional-path",
        "languageId": "rust",
        "workspace": ".",
        "command": [
            "agent_semantic_search::search_pipe_candidate_packages",
            "agent_semantic_search::search_pipe_package_cohesion"
        ],
        "phase": "cold",
        "expected": {
            "targetTotal": benchmark.target_total,
            "maxTotal": benchmark.max_total,
            "regressionBudget": benchmark.regression_budget,
            "maxProviderProcessCount": 0,
            "maxSearchOverlayProcessCount": 0,
            "maxStdoutBytes": benchmark.max_stdout_bytes,
            "requireSearchOwnedPackageCohesion": true,
            "allowedFirstRoutes": ["search-pipe-package-cohesion"],
            "forbiddenRoutes": ["command-package-cohesion", "provider-process", "native-finder"]
        },
        "observed": {
            "observedTotal": duration_literal(elapsed),
            "providerProcessCount": 0,
            "providerElapsed": "0us",
            "nativeFinderProcessCount": 0,
            "nativeFinderElapsed": "0us",
            "packageCount": packages.len(),
            "weakCohesion": weak_cohesion,
            "strongCohesion": strong_cohesion,
            "firstRoute": "search-pipe-package-cohesion",
            "executedRoutes": ["search-pipe-package-cohesion"],
            "stdoutBytes": 0,
            "fallbackReason": "none"
        },
        "verdict": "pass",
        "evidenceRefs": ["scenario:asp-search-pipe-package-cohesion-cold-functional-path"]
    });
    assert_eq!(performance_gate["observed"]["providerProcessCount"], 0);
    assert_eq!(performance_gate["observed"]["nativeFinderProcessCount"], 0);
    assert_eq!(performance_gate["observed"]["weakCohesion"], "low");
    assert_eq!(performance_gate["observed"]["strongCohesion"], "medium");
}

pub(super) fn asp_search_pipe_query_pack_cold_functional_path_stays_inside_scenario_gate() {
    let crate_root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let scenario_root = crate_root
        .join("tests")
        .join("unit")
        .join("scenarios")
        .join("asp_search_pipe_query_pack_cold_functional_path");
    let benchmark: SharedBenchmarkToml = read_toml(&scenario_root.join("benchmark.toml"));
    assert_search_pipe_query_pack_benchmark_contract(&benchmark);
    let max_total_ms = duration_millis_from_manifest(&benchmark.max_total);

    let query =
        "src/runtime.rs packages/runtime-search SearchRouter CacheStatus concurrency through owner";
    let candidates = vec![agent_semantic_search::SearchPipeQueryPackCandidate {
        path: "src/runtime.rs".to_string(),
        symbol: "SearchRouter".to_string(),
        text: "pub struct SearchRouter".to_string(),
    }];

    let started_at = Instant::now();
    let clauses = agent_semantic_search::search_pipe_query_clauses("rust", query);
    let clause_texts = agent_semantic_search::search_pipe_query_clause_texts("rust", query);
    let terms = agent_semantic_search::search_pipe_unique_query_terms(&clauses);
    let coverages = agent_semantic_search::search_pipe_clause_coverages(&clauses, &candidates);
    let owner_seed_terms = agent_semantic_search::search_pipe_role_terms(
        &terms,
        agent_semantic_search::SearchPipeTermRole::Symbol,
    );
    let elapsed = started_at.elapsed();
    let elapsed_ms = elapsed.as_millis();

    assert_eq!(
        clause_texts,
        vec![
            "src/runtime.rs packages/runtime-search".to_string(),
            "SearchRouter CacheStatus".to_string(),
            "concurrency".to_string()
        ]
    );
    assert_eq!(clauses.len(), 3);
    assert!(owner_seed_terms.iter().any(|term| term == "SearchRouter"));
    assert_eq!(coverages[1].matched, vec!["searchrouter".to_string()]);
    assert_eq!(coverages[1].missing, vec!["cachestatus".to_string()]);
    assert!(
        elapsed_ms <= max_total_ms,
        "search pipe query pack cold functional path exceeded benchmark max_total={} observed={}ms clauses={clause_texts:?} owner_seed_terms={owner_seed_terms:?}",
        benchmark.max_total,
        elapsed_ms
    );

    let performance_gate = serde_json::json!({
        "schemaId": "agent.semantic-protocols.semantic-hot-path-performance-gate",
        "schemaVersion": "1",
        "scenarioId": "asp-search-pipe-query-pack-cold-functional-path",
        "languageId": "rust",
        "workspace": ".",
        "command": [
            "agent_semantic_search::search_pipe_query_clauses",
            "agent_semantic_search::search_pipe_clause_coverages"
        ],
        "phase": "cold",
        "expected": {
            "targetTotal": benchmark.target_total,
            "maxTotal": benchmark.max_total,
            "regressionBudget": benchmark.regression_budget,
            "maxProviderProcessCount": 0,
            "maxSearchOverlayProcessCount": 0,
            "maxStdoutBytes": benchmark.max_stdout_bytes,
            "requireSearchOwnedQueryPack": true,
            "allowedFirstRoutes": ["search-pipe-query-pack"],
            "forbiddenRoutes": ["command-query-pack-parser", "provider-process", "native-finder"]
        },
        "observed": {
            "observedTotal": duration_literal(elapsed),
            "providerProcessCount": 0,
            "providerElapsed": "0us",
            "nativeFinderProcessCount": 0,
            "nativeFinderElapsed": "0us",
            "clauseCount": clauses.len(),
            "matched": coverages[1].matched,
            "missing": coverages[1].missing,
            "firstRoute": "search-pipe-query-pack",
            "executedRoutes": ["search-pipe-query-pack"],
            "stdoutBytes": 0,
            "fallbackReason": "none"
        },
        "verdict": "pass",
        "evidenceRefs": ["scenario:asp-search-pipe-query-pack-cold-functional-path"]
    });
    assert_eq!(performance_gate["observed"]["providerProcessCount"], 0);
    assert_eq!(performance_gate["observed"]["nativeFinderProcessCount"], 0);
    assert_eq!(performance_gate["observed"]["clauseCount"], 3);
}

pub(super) fn asp_graph_seed_decision_cold_functional_path_stays_inside_scenario_gate() {
    let crate_root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let scenario_root = crate_root
        .join("tests")
        .join("unit")
        .join("scenarios")
        .join("asp_graph_seed_decision_cold_functional_path");
    let benchmark: SharedBenchmarkToml = read_toml(&scenario_root.join("benchmark.toml"));
    assert_graph_seed_decision_benchmark_contract(&benchmark);
    let max_total_ms = duration_millis_from_manifest(&benchmark.max_total);

    let started_at = Instant::now();
    let broad_owner_drift = agent_semantic_search::SeedPhaseDecision::from_query_shape(true, 6, 4);
    let single_owner = agent_semantic_search::SeedPhaseDecision::from_query_shape(true, 6, 1);
    let split_action =
        agent_semantic_search::SeedActionIntent::from_seed_plan_action("split-query-pack");
    let narrow_action = agent_semantic_search::recommended_action_for_seed_risk("owner-drift");
    let unknown_seed = agent_semantic_search::SearchActionSelection::for_first_action(
        agent_semantic_search::SearchEvidenceState::Unknown,
        "seed",
    );
    let known_owner_seed = agent_semantic_search::SearchActionSelection::for_first_action(
        agent_semantic_search::SearchEvidenceState::KnownOwner,
        "seed",
    );
    let known_selector_seed = agent_semantic_search::SearchActionSelection::for_first_action(
        agent_semantic_search::SearchEvidenceState::KnownSelector,
        "seed",
    );
    let evidence_states = agent_semantic_search::SearchEvidenceState::all()
        .iter()
        .map(|state| state.as_str())
        .collect::<Vec<_>>();
    let seed_ids = vec![
        "query:search-router".to_string(),
        "owner:src/router.rs".to_string(),
    ];
    let seed_plan = agent_semantic_search::graph_turbo_seed_plan(
        agent_semantic_search::GraphTurboSeedPlanInput {
            query_present: true,
            query_seed_present: true,
            candidate_count: 9,
            candidate_owner_count: 4,
            query_owner_seed_count: 1,
            fallback_owner_seed_count: 0,
            seed_ids: &seed_ids,
            seed_decision: &broad_owner_drift,
        },
    );
    let elapsed = started_at.elapsed();
    let elapsed_ms = elapsed.as_millis();

    assert_eq!(broad_owner_drift.query_owner_anchor_budget, 2);
    assert_eq!(
        broad_owner_drift.risk_factors,
        ["flat-query", "owner-drift"]
    );
    assert_eq!(single_owner.query_owner_anchor_budget, 0);
    assert_eq!(
        split_action,
        Some(agent_semantic_search::SeedActionIntent::SplitQueryPack)
    );
    assert_eq!(narrow_action, Some("narrow-owner-scope"));
    assert!(unknown_seed.first_action_matches_evidence_state);
    assert!(!known_owner_seed.first_action_matches_evidence_state);
    assert_eq!(known_owner_seed.seed_when_known_owner_count, 1);
    assert!(!known_selector_seed.first_action_matches_evidence_state);
    assert_eq!(known_selector_seed.seed_when_known_selector_count, 1);
    assert_eq!(evidence_states.len(), 7);
    assert_eq!(seed_plan["reason"], "query");
    assert_eq!(seed_plan["seedQuality"], "review");
    assert_eq!(
        seed_plan["recommendedActions"],
        serde_json::json!(["split-query-pack", "narrow-owner-scope"])
    );
    assert_eq!(
        seed_plan["selectionPolicy"]["flow"],
        "evidence-state-reasoning-tree"
    );
    assert!(
        elapsed_ms <= max_total_ms,
        "graph seed decision cold functional path exceeded benchmark max_total={} observed={}ms broad_owner_drift={broad_owner_drift:?} known_owner_seed={known_owner_seed:?}",
        benchmark.max_total,
        elapsed_ms
    );

    let performance_gate = serde_json::json!({
        "schemaId": "agent.semantic-protocols.semantic-hot-path-performance-gate",
        "schemaVersion": "1",
        "scenarioId": "asp-graph-seed-decision-cold-functional-path",
        "languageId": "rust",
        "workspace": ".",
        "command": [
            "agent_semantic_search::SeedPhaseDecision::from_query_shape",
            "agent_semantic_search::SearchActionSelection::for_first_action",
            "agent_semantic_search::graph_turbo_seed_plan"
        ],
        "phase": "cold",
        "expected": {
            "targetTotal": benchmark.target_total,
            "maxTotal": benchmark.max_total,
            "regressionBudget": benchmark.regression_budget,
            "maxProviderProcessCount": 0,
            "maxSearchOverlayProcessCount": 0,
            "maxStdoutBytes": benchmark.max_stdout_bytes,
            "requireSearchOwnedSeedDecision": true,
            "allowedFirstRoutes": ["graph-seed-decision"],
            "forbiddenRoutes": ["command-seed-decision", "provider-process", "native-finder"]
        },
        "observed": {
            "observedTotal": duration_literal(elapsed),
            "providerProcessCount": 0,
            "providerElapsed": "0us",
            "nativeFinderProcessCount": 0,
            "nativeFinderElapsed": "0us",
            "queryOwnerAnchorBudget": broad_owner_drift.query_owner_anchor_budget,
            "riskFactors": broad_owner_drift.risk_factors,
            "knownOwnerSeedRejected": !known_owner_seed.first_action_matches_evidence_state,
            "knownSelectorSeedRejected": !known_selector_seed.first_action_matches_evidence_state,
            "evidenceStateCount": evidence_states.len(),
            "seedPlanReason": seed_plan["reason"],
            "seedPlanQuality": seed_plan["seedQuality"],
            "recommendedActions": seed_plan["recommendedActions"],
            "firstRoute": "graph-seed-decision",
            "executedRoutes": ["graph-seed-decision"],
            "stdoutBytes": 0,
            "fallbackReason": "none"
        },
        "verdict": "pass",
        "evidenceRefs": ["scenario:asp-graph-seed-decision-cold-functional-path"]
    });
    assert_eq!(performance_gate["observed"]["providerProcessCount"], 0);
    assert_eq!(performance_gate["observed"]["nativeFinderProcessCount"], 0);
    assert_eq!(performance_gate["observed"]["queryOwnerAnchorBudget"], 2);
    assert_eq!(performance_gate["observed"]["knownOwnerSeedRejected"], true);
    assert_eq!(performance_gate["observed"]["seedPlanQuality"], "review");
}

pub(super) fn asp_search_pipe_quality_decision_cold_functional_path_stays_inside_scenario_gate() {
    let crate_root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let scenario_root = crate_root
        .join("tests")
        .join("unit")
        .join("scenarios")
        .join("asp_search_pipe_quality_decision_cold_functional_path");
    let benchmark: SharedBenchmarkToml = read_toml(&scenario_root.join("benchmark.toml"));
    assert_search_pipe_quality_decision_benchmark_contract(&benchmark);
    let max_total_ms = duration_millis_from_manifest(&benchmark.max_total);

    let terms = vec![
        agent_semantic_search::SearchPipeQueryTerm {
            raw: "src/runtime.rs".to_string(),
            lower: "src/runtime.rs".to_string(),
            role: agent_semantic_search::SearchPipeTermRole::Symbol,
        },
        agent_semantic_search::SearchPipeQueryTerm {
            raw: "SearchRouter".to_string(),
            lower: "searchrouter".to_string(),
            role: agent_semantic_search::SearchPipeTermRole::Symbol,
        },
        agent_semantic_search::SearchPipeQueryTerm {
            raw: "concurrency".to_string(),
            lower: "concurrency".to_string(),
            role: agent_semantic_search::SearchPipeTermRole::Concept,
        },
    ];
    let global_matched = vec!["searchrouter".to_string()];
    let global_missing = Vec::new();
    let strong_matched = Vec::new();
    let weak_terms = vec!["SearchRouter".to_string()];

    let started_at = Instant::now();
    let missing_path_terms =
        agent_semantic_search::search_pipe_missing_path_terms(&terms, &global_matched);
    let owner_seed_terms =
        agent_semantic_search::search_pipe_owner_seed_terms(&terms, &missing_path_terms);
    let risks = agent_semantic_search::search_pipe_quality_risks(
        &terms,
        ["pub struct SearchRouter {\n    field: String\n}".to_string()].into_iter(),
        &global_missing,
        &strong_matched,
        &weak_terms,
        "low",
        1,
    );
    let quality = agent_semantic_search::search_pipe_query_pack_quality(
        &terms,
        &global_missing,
        &weak_terms,
        &risks,
    );
    let fd_query = agent_semantic_search::search_pipe_fd_query_terms(
        &terms,
        &weak_terms,
        &strong_matched,
        &risks,
    );
    let elapsed = started_at.elapsed();
    let elapsed_ms = elapsed.as_millis();

    assert_eq!(missing_path_terms, vec!["src/runtime.rs".to_string()]);
    assert_eq!(owner_seed_terms, vec!["SearchRouter".to_string()]);
    assert!(risks.iter().any(|risk| risk == "package-drift"));
    assert!(risks.iter().any(|risk| risk == "weak-camelcase-match"));
    assert_eq!(quality, "low");
    assert_eq!(fd_query.as_deref(), Some("SearchRouter"));
    assert!(
        elapsed_ms <= max_total_ms,
        "search pipe quality decision cold functional path exceeded benchmark max_total={} observed={}ms risks={risks:?} quality={quality} fd_query={fd_query:?}",
        benchmark.max_total,
        elapsed_ms
    );

    let performance_gate = serde_json::json!({
        "schemaId": "agent.semantic-protocols.semantic-hot-path-performance-gate",
        "schemaVersion": "1",
        "scenarioId": "asp-search-pipe-quality-decision-cold-functional-path",
        "languageId": "rust",
        "workspace": ".",
        "command": [
            "agent_semantic_search::search_pipe_quality_risks",
            "agent_semantic_search::search_pipe_query_pack_quality",
            "agent_semantic_search::search_pipe_fd_query_terms"
        ],
        "phase": "cold",
        "expected": {
            "targetTotal": benchmark.target_total,
            "maxTotal": benchmark.max_total,
            "regressionBudget": benchmark.regression_budget,
            "maxProviderProcessCount": 0,
            "maxSearchOverlayProcessCount": 0,
            "maxStdoutBytes": benchmark.max_stdout_bytes,
            "requireSearchOwnedQualityDecision": true,
            "allowedFirstRoutes": ["search-pipe-quality-decision"],
            "forbiddenRoutes": ["command-quality-decision", "provider-process", "native-finder"]
        },
        "observed": {
            "observedTotal": duration_literal(elapsed),
            "providerProcessCount": 0,
            "providerElapsed": "0us",
            "nativeFinderProcessCount": 0,
            "nativeFinderElapsed": "0us",
            "riskCount": risks.len(),
            "quality": quality,
            "fdQuery": fd_query,
            "firstRoute": "search-pipe-quality-decision",
            "executedRoutes": ["search-pipe-quality-decision"],
            "stdoutBytes": 0,
            "fallbackReason": "none"
        },
        "verdict": "pass",
        "evidenceRefs": ["scenario:asp-search-pipe-quality-decision-cold-functional-path"]
    });
    assert_eq!(performance_gate["observed"]["providerProcessCount"], 0);
    assert_eq!(performance_gate["observed"]["nativeFinderProcessCount"], 0);
    assert_eq!(performance_gate["observed"]["quality"], "low");
}

pub(super) fn asp_search_pipe_evidence_classifier_cold_functional_path_stays_inside_scenario_gate()
{
    let crate_root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let scenario_root = crate_root
        .join("tests")
        .join("unit")
        .join("scenarios")
        .join("asp_search_pipe_evidence_classifier_cold_functional_path");
    let benchmark: SharedBenchmarkToml = read_toml(&scenario_root.join("benchmark.toml"));
    assert_search_pipe_evidence_classifier_benchmark_contract(&benchmark);
    let max_total_ms = duration_millis_from_manifest(&benchmark.max_total);

    let candidates = vec![
        agent_semantic_search::SearchPipeEvidenceCandidate {
            path: "src/router.rs".to_string(),
            line: 7,
            symbol: "SearchRouter".to_string(),
            text: "pub struct SearchRouter {}".to_string(),
            source: "source-index".to_string(),
        },
        agent_semantic_search::SearchPipeEvidenceCandidate {
            path: "src/cache.rs".to_string(),
            line: 3,
            symbol: "CacheStatus".to_string(),
            text: "CacheStatus hit".to_string(),
            source: "search-overlay".to_string(),
        },
    ];
    let terms = vec![
        agent_semantic_search::SearchPipeQueryTerm {
            raw: "SearchRouter".to_string(),
            lower: "searchrouter".to_string(),
            role: agent_semantic_search::SearchPipeTermRole::Symbol,
        },
        agent_semantic_search::SearchPipeQueryTerm {
            raw: "CacheStatus".to_string(),
            lower: "cachestatus".to_string(),
            role: agent_semantic_search::SearchPipeTermRole::Symbol,
        },
        agent_semantic_search::SearchPipeQueryTerm {
            raw: "router::SearchRouter".to_string(),
            lower: "router::searchrouter".to_string(),
            role: agent_semantic_search::SearchPipeTermRole::Symbol,
        },
    ];

    let started_at = Instant::now();
    let declaration_match = agent_semantic_search::search_pipe_declaration_header_match(
        "rust",
        &candidates[0],
        &terms[0],
    );
    let compound_match =
        agent_semantic_search::search_pipe_strong_match("rust", &candidates[0], &terms[2]);
    let parser_handles =
        agent_semantic_search::search_pipe_parser_handles("rust", &candidates, &terms);
    let search_overlay_handles =
        agent_semantic_search::search_pipe_search_overlay_handles(&candidates, &terms);
    let weak_reason = agent_semantic_search::search_pipe_weak_reason(&terms[0], &candidates);
    let elapsed = started_at.elapsed();
    let elapsed_ms = elapsed.as_millis();

    assert!(declaration_match);
    assert!(compound_match);
    assert_eq!(
        parser_handles,
        vec!["SearchRouter@src/router.rs:7".to_string()]
    );
    assert_eq!(search_overlay_handles, vec!["CacheStatus".to_string()]);
    assert_eq!(weak_reason, "lexical-match");
    assert!(
        elapsed_ms <= max_total_ms,
        "search pipe evidence classifier cold functional path exceeded benchmark max_total={} observed={}ms parser_handles={parser_handles:?} search_overlay_handles={search_overlay_handles:?}",
        benchmark.max_total,
        elapsed_ms
    );

    let performance_gate = serde_json::json!({
        "schemaId": "agent.semantic-protocols.semantic-hot-path-performance-gate",
        "schemaVersion": "1",
        "scenarioId": "asp-search-pipe-evidence-classifier-cold-functional-path",
        "languageId": "rust",
        "workspace": ".",
        "command": [
            "agent_semantic_search::search_pipe_declaration_header_match",
            "agent_semantic_search::search_pipe_strong_match",
            "agent_semantic_search::search_pipe_parser_handles",
            "agent_semantic_search::search_pipe_search_overlay_handles"
        ],
        "phase": "cold",
        "expected": {
            "targetTotal": benchmark.target_total,
            "maxTotal": benchmark.max_total,
            "regressionBudget": benchmark.regression_budget,
            "maxProviderProcessCount": 0,
            "maxSearchOverlayProcessCount": 0,
            "maxStdoutBytes": benchmark.max_stdout_bytes,
            "requireSearchOwnedEvidenceClassifier": true,
            "allowedFirstRoutes": ["search-pipe-evidence-classifier"],
            "forbiddenRoutes": ["command-evidence-classifier", "provider-process", "native-finder"]
        },
        "observed": {
            "observedTotal": duration_literal(elapsed),
            "providerProcessCount": 0,
            "providerElapsed": "0us",
            "nativeFinderProcessCount": 0,
            "nativeFinderElapsed": "0us",
            "declarationMatch": declaration_match,
            "compoundMatch": compound_match,
            "parserHandleCount": parser_handles.len(),
            "searchOverlayHandleCount": search_overlay_handles.len(),
            "firstRoute": "search-pipe-evidence-classifier",
            "executedRoutes": ["search-pipe-evidence-classifier"],
            "stdoutBytes": 0,
            "fallbackReason": "none"
        },
        "verdict": "pass",
        "evidenceRefs": ["scenario:asp-search-pipe-evidence-classifier-cold-functional-path"]
    });
    assert_eq!(performance_gate["observed"]["providerProcessCount"], 0);
    assert_eq!(performance_gate["observed"]["nativeFinderProcessCount"], 0);
    assert_eq!(performance_gate["observed"]["declarationMatch"], true);
    assert_eq!(performance_gate["observed"]["compoundMatch"], true);
}

pub(super) fn asp_runtime_owner_items_receipt_cold_functional_path_stays_inside_scenario_gate() {
    let crate_root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let scenario_root = crate_root
        .join("tests")
        .join("unit")
        .join("scenarios")
        .join("asp_runtime_owner_items_receipt_cold_functional_path");
    let benchmark: SharedBenchmarkToml = read_toml(&scenario_root.join("benchmark.toml"));
    assert_runtime_owner_items_receipt_benchmark_contract(&benchmark);
    let max_total_ms = duration_millis_from_manifest(&benchmark.max_total);

    let root = temp_project_root("scenario-runtime-owner-items-receipt-cold");
    let cache_home = root.join(".cache");
    fs::create_dir_all(root.join("src")).expect("create source root");
    fs::write(root.join("src/lib.rs"), "pub fn runtime_owner_items() {}\n").expect("write owner");
    let args = vec![
        "items".to_string(),
        "--view".to_string(),
        "seeds".to_string(),
    ];
    let invocation = vec!["rs-harness".to_string(), "query".to_string()];
    let request = agent_semantic_runtime::LanguageOwnerItemsCacheRequest {
        language_id: "rust",
        args: &args,
        invocation: &invocation,
        owner: Path::new("src/lib.rs"),
        project_root: &root,
        cache_home: &cache_home,
    };

    let started_at = Instant::now();
    let outcome = agent_semantic_runtime::resolve_language_owner_items_runtime_outcome(
        &request,
        true,
        Some(agent_semantic_runtime::LanguageOwnerItemsProviderOutput {
            status_success: true,
            stdout: b"actionFrontier=internal\npublic owner item\n",
            stderr: b"provider note\n",
        }),
    )
    .expect("resolve runtime owner-items outcome");
    let elapsed = started_at.elapsed();
    let receipt = agent_semantic_runtime::language_owner_items_runtime_receipt(
        &outcome,
        1,
        elapsed.as_millis(),
    );
    let elapsed_ms = elapsed.as_millis();

    assert_eq!(receipt.outcome, "handled");
    assert_eq!(receipt.provider_process_count, 1);
    assert_eq!(receipt.stdout_bytes, b"public owner item\n".len());
    assert_eq!(receipt.stderr_bytes, b"provider note\n".len());
    assert!(!receipt.cache_hit);
    assert_eq!(receipt.fallback_reason, "none");
    assert!(
        elapsed_ms <= max_total_ms,
        "runtime owner-items receipt cold functional path exceeded benchmark max_total={} observed={}ms receipt={receipt:?}",
        benchmark.max_total,
        elapsed_ms
    );

    let observed_total = duration_literal(elapsed);
    let performance_gate = serde_json::json!({
        "schemaId": "agent.semantic-protocols.semantic-hot-path-performance-gate",
        "schemaVersion": "1",
        "scenarioId": "asp-runtime-owner-items-receipt-cold-functional-path",
        "languageId": "rust",
        "workspace": ".",
        "command": [
            "agent_semantic_runtime::resolve_language_owner_items_runtime_outcome",
            "agent_semantic_runtime::language_owner_items_runtime_receipt"
        ],
        "phase": "cold",
        "expected": {
            "targetTotal": benchmark.target_total,
            "maxTotal": benchmark.max_total,
            "regressionBudget": benchmark.regression_budget,
            "maxProviderProcessCount": benchmark.max_provider_process_count,
            "maxSearchOverlayProcessCount": 0,
            "maxStdoutBytes": benchmark.max_stdout_bytes,
            "requireRuntimeOwnedReceipt": true,
            "allowedFirstRoutes": ["owner-items-runtime"],
            "forbiddenRoutes": ["command-receipt", "native-finder", "inline-fallback"],
            "fallbackReason": "none"
        },
        "observed": {
            "observedTotal": observed_total,
            "providerProcessCount": receipt.provider_process_count,
            "providerElapsed": observed_total,
            "nativeFinderProcessCount": 0,
            "nativeFinderElapsed": "0us",
            "firstRoute": "owner-items-runtime",
            "executedRoutes": ["owner-items-runtime"],
            "stdoutBytes": receipt.stdout_bytes,
            "stderrBytes": receipt.stderr_bytes,
            "cacheHit": receipt.cache_hit,
            "fallbackReason": receipt.fallback_reason
        },
        "verdict": "pass",
        "evidenceRefs": ["scenario:asp-runtime-owner-items-receipt-cold-functional-path"]
    });
    assert_eq!(
        performance_gate["observed"]["providerProcessCount"],
        benchmark.max_provider_process_count.unwrap_or(1)
    );
    assert_eq!(performance_gate["observed"]["nativeFinderProcessCount"], 0);
    assert_eq!(performance_gate["observed"]["fallbackReason"], "none");
    let _ = fs::remove_dir_all(root);
}

pub(super) fn asp_runtime_timeout_policy_cold_functional_path_stays_inside_scenario_gate() {
    let crate_root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let scenario_root = crate_root
        .join("tests")
        .join("unit")
        .join("scenarios")
        .join("asp_runtime_timeout_policy_cold_functional_path");
    let benchmark: SharedBenchmarkToml = read_toml(&scenario_root.join("benchmark.toml"));
    assert_runtime_timeout_policy_benchmark_contract(&benchmark);
    let max_total_ms = duration_millis_from_manifest(&benchmark.max_total);

    let policy = agent_semantic_runtime::RuntimeOperationTimeoutPolicy {
        operation: "owner-items-provider".to_string(),
        max_elapsed_ms: 10,
        cancel_after_ms: 25,
    };
    let started_at = Instant::now();
    let receipt = agent_semantic_runtime::runtime_operation_timeout_receipt(&policy, 1);
    let elapsed = started_at.elapsed();
    let elapsed_ms = elapsed.as_millis();

    assert_eq!(receipt.operation, "owner-items-provider");
    assert_eq!(receipt.elapsed_ms, 1);
    assert!(!receipt.timed_out);
    assert!(!receipt.cancellation_required);
    assert!(
        elapsed_ms <= max_total_ms,
        "runtime timeout policy cold functional path exceeded benchmark max_total={} observed={}ms receipt={receipt:?}",
        benchmark.max_total,
        elapsed_ms
    );

    let observed_total = duration_literal(elapsed);
    let performance_gate = serde_json::json!({
        "schemaId": "agent.semantic-protocols.semantic-hot-path-performance-gate",
        "schemaVersion": "1",
        "scenarioId": "asp-runtime-timeout-policy-cold-functional-path",
        "languageId": "rust",
        "workspace": ".",
        "command": [
            "agent_semantic_runtime::runtime_operation_timeout_receipt"
        ],
        "phase": "cold",
        "expected": {
            "targetTotal": benchmark.target_total,
            "maxTotal": benchmark.max_total,
            "regressionBudget": benchmark.regression_budget,
            "maxProviderProcessCount": 0,
            "maxSearchOverlayProcessCount": 0,
            "maxStdoutBytes": benchmark.max_stdout_bytes,
            "requireRuntimeOwnedTimeoutPolicy": true,
            "allowedFirstRoutes": ["runtime-timeout-policy"],
            "forbiddenRoutes": ["command-timeout-policy", "provider-process"],
            "fallbackReason": "none"
        },
        "observed": {
            "observedTotal": observed_total,
            "providerProcessCount": 0,
            "providerElapsed": "0us",
            "nativeFinderProcessCount": 0,
            "nativeFinderElapsed": "0us",
            "firstRoute": "runtime-timeout-policy",
            "executedRoutes": ["runtime-timeout-policy"],
            "timedOut": receipt.timed_out,
            "cancellationRequired": receipt.cancellation_required,
            "stdoutBytes": 0,
            "fallbackReason": "none"
        },
        "verdict": "pass",
        "evidenceRefs": ["scenario:asp-runtime-timeout-policy-cold-functional-path"]
    });
    assert_eq!(performance_gate["observed"]["providerProcessCount"], 0);
    assert_eq!(performance_gate["observed"]["nativeFinderProcessCount"], 0);
    assert_eq!(performance_gate["observed"]["timedOut"], false);
    assert_eq!(performance_gate["observed"]["cancellationRequired"], false);
}

pub(super) fn scenario_benchmark_duration_contract_rejects_zero_budget() {
    let path = Path::new("scenario/benchmark.toml");
    let mut invalid = Vec::new();
    require_positive_duration_manifest_field(&mut invalid, path, "target_total", "0ms");
    require_positive_duration_manifest_field(&mut invalid, path, "target_total", "500us");
    require_observed_timing_manifest_field(
        &mut invalid,
        path,
        "provider_process_count",
        &toml::Value::String("0ms".to_string()),
    );
    require_observed_timing_manifest_field(
        &mut invalid,
        path,
        "provider_process_count",
        &toml::Value::String("0us".to_string()),
    );

    assert_eq!(
        invalid,
        vec![
            "scenario/benchmark.toml: target_total=\"0ms\" must be a positive duration such as 500us or 25ms",
            "scenario/benchmark.toml: observed_timings.provider_process_count must use 0us for zero-duration branches, not 0ms",
        ]
    );
}

pub(super) fn asp_dynamic_overlay_search_pipe_warm_path_stays_inside_scenario_gate() {
    let crate_root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let scenario_root = crate_root
        .join("tests")
        .join("unit")
        .join("scenarios")
        .join("asp_dynamic_overlay_search_pipe_warm_path");
    let benchmark: SharedBenchmarkToml = read_toml(&scenario_root.join("benchmark.toml"));
    assert_dynamic_overlay_benchmark_contract(&benchmark);
    let max_total_ms = duration_millis_from_manifest(&benchmark.max_total);

    let root = temp_project_root("scenario-dynamic-overlay-search-pipe");
    let bin_dir = root.join(".tmp").join("provider-bin");
    let marker = root.join("provider-called");
    fs::create_dir_all(root.join("src")).expect("create source root");
    fs::create_dir_all(root.join("target")).expect("create ignored target root");
    fs::write(
        root.join("Cargo.toml"),
        "[package]\nname = \"scenario-dynamic-overlay-search-pipe\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
    )
    .expect("write package anchor");
    fs::write(
        root.join("src/lib.rs"),
        "pub fn dynamic_overlay_fixture() { let dynamic_overlay_signal = true; }\n",
    )
    .expect("write source");
    fs::write(
        root.join("target").join("dynamic_overlay_ignored.rs"),
        "pub fn dynamic_overlay_ignored() {}\n",
    )
    .expect("write ignored source");
    write_marker_provider(&bin_dir, "rs-harness", &marker);
    write_activation(&root, &[provider("rust", Vec::new())]);

    let command_args = [
        "rust",
        "search",
        "pipe",
        "dynamic_overlay_fixture",
        "--source",
        "search-overlay",
        "--workspace",
        ".",
        "--view",
        "seeds",
    ];
    let output = asp_command(&root)
        .env("PATH", prepend_path(&bin_dir))
        .env("PRJ_CACHE_HOME", root.join(".cache"))
        .args(command_args)
        .output()
        .expect("run asp rust search pipe dynamic overlay");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("stdout");
    for expected in [
        "[search-pipe]",
        "source=search-overlay",
        "sourceTrace=search-overlay:used",
        "ownerCoverage=bestOwner=src/lib.rs",
        "nextCommand=asp rust search owner src/lib.rs items --query dynamic_overlay_fixture --workspace . --view seeds",
    ] {
        assert!(
            stdout.contains(expected),
            "dynamic overlay search pipe scenario missing {expected:?}; stdout={stdout}"
        );
    }
    assert!(
        !stdout.contains("target/dynamic_overlay_ignored.rs"),
        "dynamic overlay root walk must honor ignored directories; stdout={stdout}"
    );
    assert!(
        !stdout.contains("sourceTrace=sourceIndex:used"),
        "explicit dynamic overlay scenario must not use source-index; stdout={stdout}"
    );
    assert!(
        !stdout.contains("sourceTrace=finder"),
        "dynamic overlay search scenario must not expose finder as a route; stdout={stdout}"
    );
    assert!(
        !marker.exists(),
        "dynamic overlay search pipe should not spawn provider"
    );
    let collect_ms = source_trace_metric_ms(&stdout, "elapsedMs");
    assert!(
        collect_ms <= max_total_ms,
        "dynamic overlay search pipe exceeded benchmark max_total={} observed={}ms stdout={stdout}",
        benchmark.max_total,
        collect_ms
    );
    assert!(
        stdout.len() <= benchmark.max_stdout_bytes.unwrap_or(8192) as usize,
        "dynamic overlay stdout exceeded benchmark max_stdout_bytes; stdout={stdout}"
    );

    let observed_total = format!("{collect_ms}ms");
    let performance_gate = serde_json::json!({
        "schemaId": "agent.semantic-protocols.semantic-hot-path-performance-gate",
        "schemaVersion": "1",
        "scenarioId": "asp-dynamic-overlay-search-pipe-warm-path",
        "languageId": "rust",
        "workspace": ".",
        "command": [
            "asp",
            "rust",
            "search",
            "pipe",
            "dynamic_overlay_fixture",
            "--source",
            "search-overlay",
            "--workspace",
            ".",
            "--view",
            "seeds"
        ],
        "phase": "hot",
        "expected": {
            "targetTotal": benchmark.target_total,
            "maxTotal": benchmark.max_total,
            "regressionBudget": benchmark.regression_budget,
            "maxProviderProcessCount": 0,
            "maxSearchOverlayProcessCount": 0,
            "maxRenderDuration": benchmark.max_total,
            "maxStdoutBytes": benchmark.max_stdout_bytes.unwrap_or(8192),
            "allowedFirstRoutes": ["search-overlay"],
            "forbiddenRoutes": ["source-index", "native-finder", "provider-process"],
            "requireExactCodeIdentity": true,
            "requireNoExecutableLineRange": true
        },
        "observed": {
            "observedTotal": observed_total,
            "providerProcessCount": 0,
            "providerElapsed": "0us",
            "nativeFinderProcessCount": 0,
            "nativeFinderElapsed": "0us",
            "firstRoute": "search-overlay",
            "executedRoutes": ["search-overlay", "query-code"],
            "executableLineRangeSelectorCount": 0,
            "packetOutMode": "not-applicable",
            "renderDuration": observed_total,
            "stdoutBytes": stdout.len()
        },
        "verdict": "pass",
        "evidenceRefs": ["scenario:asp-dynamic-overlay-search-pipe-warm-path"]
    });
    assert_eq!(performance_gate["observed"]["providerProcessCount"], 0);
    assert_eq!(performance_gate["observed"]["nativeFinderProcessCount"], 0);
    assert_eq!(performance_gate["observed"]["firstRoute"], "search-overlay");
    assert_eq!(performance_gate["observed"]["stdoutBytes"], stdout.len());
    let _ = fs::remove_dir_all(root);
}

pub(super) fn asp_rg_query_source_index_warm_path_stays_inside_scenario_gate() {
    let crate_root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let scenario_root = crate_root
        .join("tests")
        .join("unit")
        .join("scenarios")
        .join("asp_rg_query_source_index_warm_path");
    let benchmark: SharedBenchmarkToml = read_toml(&scenario_root.join("benchmark.toml"));
    assert_source_index_query_benchmark_contract(&benchmark);
    let max_total_ms = duration_millis_from_manifest(&benchmark.max_total);

    let root = temp_project_root("scenario-rg-query-source-index");
    let bin_dir = root.join(".tmp").join("provider-bin");
    let marker = root.join("provider-called");
    fs::create_dir_all(root.join("src")).expect("create source root");
    fs::write(
        root.join("Cargo.toml"),
        "[package]\nname = \"scenario-rg-query-source-index\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
    )
    .expect("write package anchor");
    fs::write(
        root.join("src/lib.rs"),
        "pub fn source_index_fixture() {}\npub fn unrelated() {}\n",
    )
    .expect("write source");
    write_marker_provider(&bin_dir, "rs-harness", &marker);
    write_activation(&root, &[provider("rust", Vec::new())]);
    refresh_source_index(&root);
    let _ = fs::remove_file(&marker);

    let warm_output = asp_command(&root)
        .env("PATH", prepend_path(&bin_dir))
        .env("PRJ_CACHE_HOME", root.join(".cache"))
        .args(["rg", "-query", "source_index_fixture", "--workspace", "."])
        .output()
        .expect("warm asp rg -query");
    assert!(
        warm_output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&warm_output.stderr)
    );
    assert!(
        !marker.exists(),
        "source-index warm-up rg query should not spawn provider"
    );

    let output = asp_command(&root)
        .env("PATH", prepend_path(&bin_dir))
        .env("PRJ_CACHE_HOME", root.join(".cache"))
        .args(["rg", "-query", "source_index_fixture", "--workspace", "."])
        .output()
        .expect("run asp rg -query");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("stdout");
    for expected in [
        "[search-rg]",
        "source=source-index",
        "sourceTrace=sourceIndex:used",
        "query-overlay:skipped",
        "packages=src/lib.rs",
        "nextCommand=asp fd -query source_index_fixture --workspace .",
    ] {
        assert!(
            stdout.contains(expected),
            "source-index rg query scenario missing {expected:?}; stdout={stdout}"
        );
    }
    assert!(
        !stdout.contains("sourceTrace=finder:used"),
        "rg warm SourceIndex path must not collect through query overlay; stdout={stdout}"
    );
    assert!(
        !marker.exists(),
        "source-index warm rg query should not spawn provider"
    );
    let collect_ms = source_trace_metric_ms(&stdout, "collectMs");
    assert!(
        collect_ms <= max_total_ms,
        "source-index warm rg query exceeded benchmark max_total={} observed={}ms stdout={stdout}",
        benchmark.max_total,
        collect_ms
    );
    let observed_total = format!("{collect_ms}ms");
    let performance_gate = serde_json::json!({
        "schemaId": "agent.semantic-protocols.semantic-hot-path-performance-gate",
        "schemaVersion": "1",
        "scenarioId": "asp-rg-query-source-index-warm-path",
        "languageId": "query-wrapper",
        "workspace": ".",
        "command": [
            "asp",
            "rg",
            "-query",
            "source_index_fixture",
            "--workspace",
            "."
        ],
        "phase": "hot",
        "expected": {
            "targetTotal": benchmark.target_total,
            "maxTotal": benchmark.max_total,
            "regressionBudget": benchmark.regression_budget,
            "maxProviderProcessCount": 0,
            "maxSearchOverlayProcessCount": 0,
            "maxRenderDuration": benchmark.max_total,
            "maxStdoutBytes": 8192,
            "requireSourceIndexHit": true,
            "allowedFirstRoutes": ["source-index"],
            "forbiddenRoutes": ["prime", "native-finder", "provider-process"],
            "requireExactCodeIdentity": false,
            "requireNoExecutableLineRange": true
        },
        "observed": {
            "observedTotal": observed_total,
            "providerProcessCount": 0,
            "providerElapsed": "0us",
            "nativeFinderProcessCount": 0,
            "nativeFinderElapsed": "0us",
            "sourceIndexHit": true,
            "sourceIndexDuration": observed_total,
            "firstRoute": "source-index",
            "executedRoutes": ["source-index", "fd-query"],
            "executableLineRangeSelectorCount": 0,
            "packetOutMode": "not-applicable",
            "renderDuration": observed_total,
            "stdoutBytes": stdout.len()
        },
        "verdict": "pass",
        "evidenceRefs": ["scenario:asp-rg-query-source-index-warm-path"]
    });
    assert_eq!(performance_gate["observed"]["providerProcessCount"], 0);
    assert_eq!(performance_gate["observed"]["nativeFinderProcessCount"], 0);
    assert_eq!(performance_gate["observed"]["sourceIndexHit"], true);
    assert_eq!(performance_gate["observed"]["stdoutBytes"], stdout.len());
    let _ = fs::remove_dir_all(root);
}

pub(super) fn asp_fd_query_source_index_warm_path_stays_inside_scenario_gate() {
    let crate_root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let scenario_root = crate_root
        .join("tests")
        .join("unit")
        .join("scenarios")
        .join("asp_fd_query_source_index_warm_path");
    let benchmark: SharedBenchmarkToml = read_toml(&scenario_root.join("benchmark.toml"));
    let max_total_ms = duration_millis_from_manifest(&benchmark.max_total);
    assert_source_index_query_benchmark_contract(&benchmark);

    let root = temp_project_root("scenario-fd-query-source-index");
    let bin_dir = root.join(".bin");
    let marker = root.join("provider-called");
    fs::create_dir_all(root.join("src")).expect("create source root");
    fs::write(
        root.join("Cargo.toml"),
        "[package]\nname = \"scenario-fd-query-source-index\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
    )
    .expect("write package anchor");
    fs::write(
        root.join("src/lib.rs"),
        "pub fn source_index_fixture() {}\npub fn unrelated() {}\n",
    )
    .expect("write source");
    write_marker_provider(&bin_dir, "rs-harness", &marker);
    write_activation(&root, &[provider("rust", Vec::new())]);
    refresh_source_index(&root);
    let _ = fs::remove_file(&marker);

    let output = asp_command(&root)
        .env("PATH", prepend_path(&bin_dir))
        .env("PRJ_CACHE_HOME", root.join(".cache"))
        .args(["fd", "-query", "source_index_fixture", "--workspace", "."])
        .output()
        .expect("run asp fd -query");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("stdout");
    for expected in [
        "[search-fd]",
        "source=source-index",
        "sourceTrace=sourceIndex:used",
        "query-overlay:skipped",
        "src/lib.rs",
    ] {
        assert!(
            stdout.contains(expected),
            "source-index fd query scenario missing {expected:?}; stdout={stdout}"
        );
    }
    assert!(
        !stdout.contains("sourceTrace=finder:used"),
        "fd warm SourceIndex path must not collect through query overlay; stdout={stdout}"
    );
    assert!(
        !marker.exists(),
        "source-index warm fd query should not spawn provider"
    );
    let collect_ms = source_trace_metric_ms(&stdout, "collectMs");
    assert!(
        collect_ms <= max_total_ms,
        "source-index warm fd query exceeded benchmark max_total={} observed={}ms stdout={stdout}",
        benchmark.max_total,
        collect_ms
    );
    let observed_total = format!("{collect_ms}ms");
    let performance_gate = serde_json::json!({
        "schemaId": "agent.semantic-protocols.semantic-hot-path-performance-gate",
        "schemaVersion": "1",
        "scenarioId": "asp-fd-query-source-index-warm-path",
        "languageId": "query-wrapper",
        "workspace": ".",
        "command": [
            "asp",
            "fd",
            "-query",
            "source_index_fixture",
            "--workspace",
            "."
        ],
        "phase": "hot",
        "expected": {
            "targetTotal": benchmark.target_total,
            "maxTotal": benchmark.max_total,
            "regressionBudget": benchmark.regression_budget,
            "maxProviderProcessCount": 0,
            "maxSearchOverlayProcessCount": 0,
            "maxRenderDuration": benchmark.max_total,
            "maxStdoutBytes": 8192,
            "requireSourceIndexHit": true,
            "allowedFirstRoutes": ["source-index"],
            "forbiddenRoutes": ["prime", "native-finder", "provider-process"],
            "requireExactCodeIdentity": false,
            "requireNoExecutableLineRange": true
        },
        "observed": {
            "observedTotal": observed_total,
            "providerProcessCount": 0,
            "providerElapsed": "0us",
            "nativeFinderProcessCount": 0,
            "nativeFinderElapsed": "0us",
            "sourceIndexHit": true,
            "sourceIndexDuration": observed_total,
            "firstRoute": "source-index",
            "executedRoutes": ["source-index", "owner-items"],
            "executableLineRangeSelectorCount": 0,
            "packetOutMode": "not-applicable",
            "renderDuration": observed_total,
            "stdoutBytes": stdout.len()
        },
        "verdict": "pass",
        "evidenceRefs": ["scenario:asp-fd-query-source-index-warm-path"]
    });
    assert_eq!(performance_gate["observed"]["providerProcessCount"], 0);
    assert_eq!(performance_gate["observed"]["nativeFinderProcessCount"], 0);
    assert_eq!(performance_gate["observed"]["sourceIndexHit"], true);
    assert_eq!(performance_gate["observed"]["stdoutBytes"], stdout.len());
    let _ = fs::remove_dir_all(root);
}

pub(super) fn asp_rg_query_source_index_miss_skips_search_overlay_gate() {
    let crate_root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let scenario_root = crate_root
        .join("tests")
        .join("unit")
        .join("scenarios")
        .join("asp_rg_query_source_index_miss_warm_path");
    let benchmark: SharedBenchmarkToml = read_toml(&scenario_root.join("benchmark.toml"));
    let max_total_ms = duration_millis_from_manifest(&benchmark.max_total);
    assert_source_index_query_benchmark_contract(&benchmark);

    let root = temp_project_root("scenario-rg-query-source-index-miss");
    let bin_dir = root.join(".bin");
    let marker = root.join("provider-called");
    fs::create_dir_all(root.join("src")).expect("create source root");
    fs::write(
        root.join("Cargo.toml"),
        "[package]\nname = \"scenario-rg-query-source-index-miss\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
    )
    .expect("write package anchor");
    fs::write(
        root.join("src/lib.rs"),
        "pub fn source_index_fixture() {}\npub fn unrelated() {}\n",
    )
    .expect("write source");
    write_marker_provider(&bin_dir, "rs-harness", &marker);
    write_activation(&root, &[provider("rust", Vec::new())]);
    refresh_source_index(&root);
    let _ = fs::remove_file(&marker);

    let output = asp_command(&root)
        .env("PATH", prepend_path(&bin_dir))
        .env("PRJ_CACHE_HOME", root.join(".cache"))
        .args(["rg", "-query", "missing_symbol", "--workspace", "."])
        .output()
        .expect("run asp rg -query source-index miss");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("stdout");
    for expected in [
        "noOutput reason=source-index-miss",
        "sourceTrace=sourceIndex:miss",
        "query-overlay:skipped",
        "nextCommand=asp cache source-index refresh",
        "refineHint=SourceIndex miss",
    ] {
        assert!(
            stdout.contains(expected),
            "source-index miss scenario missing {expected:?}; stdout={stdout}"
        );
    }
    assert!(
        !stdout.contains("sourceTrace=finder:used"),
        "rg SourceIndex miss must not fall back to query overlay in an indexed workspace; stdout={stdout}"
    );
    assert!(
        !marker.exists(),
        "source-index miss should not spawn provider"
    );
    let collect_ms = source_trace_metric_ms(&stdout, "collectMs");
    assert!(
        collect_ms <= max_total_ms,
        "source-index warm rg miss exceeded benchmark max_total={} observed={}ms stdout={stdout}",
        benchmark.max_total,
        collect_ms
    );
    let observed_total = format!("{collect_ms}ms");
    let performance_gate = serde_json::json!({
        "schemaId": "agent.semantic-protocols.semantic-hot-path-performance-gate",
        "schemaVersion": "1",
        "scenarioId": "asp-rg-query-source-index-miss-warm-path",
        "languageId": "query-wrapper",
        "workspace": ".",
        "command": [
            "asp",
            "rg",
            "-query",
            "missing_symbol",
            "--workspace",
            "."
        ],
        "phase": "hot",
        "expected": {
            "targetTotal": benchmark.target_total,
            "maxTotal": benchmark.max_total,
            "regressionBudget": benchmark.regression_budget,
            "maxProviderProcessCount": 0,
            "maxSearchOverlayProcessCount": 0,
            "maxRenderDuration": benchmark.max_total,
            "maxStdoutBytes": 8192,
            "requireSourceIndexLookup": true,
            "allowedFirstRoutes": ["source-index"],
            "forbiddenRoutes": ["prime", "native-finder", "provider-process"],
            "requireExactCodeIdentity": false,
            "requireNoExecutableLineRange": true
        },
        "observed": {
            "observedTotal": observed_total,
            "providerProcessCount": 0,
            "providerElapsed": "0us",
            "nativeFinderProcessCount": 0,
            "nativeFinderElapsed": "0us",
            "sourceIndexState": "miss",
            "sourceIndexDuration": observed_total,
            "firstRoute": "source-index",
            "executedRoutes": ["source-index"],
            "executableLineRangeSelectorCount": 0,
            "packetOutMode": "not-applicable",
            "renderDuration": observed_total,
            "stdoutBytes": stdout.len()
        },
        "verdict": "pass",
        "evidenceRefs": ["scenario:asp-rg-query-source-index-miss-warm-path"]
    });
    assert_eq!(performance_gate["observed"]["providerProcessCount"], 0);
    assert_eq!(performance_gate["observed"]["nativeFinderProcessCount"], 0);
    assert_eq!(performance_gate["observed"]["sourceIndexState"], "miss");
    assert_eq!(performance_gate["observed"]["stdoutBytes"], stdout.len());
    let _ = fs::remove_dir_all(root);
}

pub(super) fn asp_lexical_source_index_warm_path_stays_inside_scenario_gate() {
    let crate_root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let scenario_root = crate_root
        .join("tests")
        .join("unit")
        .join("scenarios")
        .join("asp_lexical_source_index_warm_path");
    let benchmark: SharedBenchmarkToml = read_toml(&scenario_root.join("benchmark.toml"));
    assert_source_index_query_benchmark_contract(&benchmark);
    let max_total_ms = duration_millis_from_manifest(&benchmark.max_total);

    let root = temp_project_root("scenario-lexical-source-index");
    let bin_dir = root.join(".bin");
    let marker = root.join("provider-called");
    fs::create_dir_all(root.join("src")).expect("create source root");
    fs::write(
        root.join("Cargo.toml"),
        "[package]\nname = \"scenario-lexical-source-index\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
    )
    .expect("write package anchor");
    fs::write(
        root.join("src/lib.rs"),
        "pub fn source_index_fixture() {}\npub fn unrelated() {}\n",
    )
    .expect("write source");
    write_marker_provider(&bin_dir, "rs-harness", &marker);
    write_activation(&root, &[provider("rust", Vec::new())]);
    refresh_source_index(&root);
    let _ = fs::remove_file(&marker);

    let output = asp_command(&root)
        .env("PATH", prepend_path(&bin_dir))
        .env("PRJ_CACHE_HOME", root.join(".cache"))
        .args([
            "rust",
            "search",
            "lexical",
            "source_index_fixture",
            "owner",
            "items",
            "tests",
            "--workspace",
            ".",
            "--view",
            "seeds",
        ])
        .output()
        .expect("run asp rust search lexical");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("stdout");
    for expected in [
        "[search-lexical]",
        "source=source-index",
        "sourceTrace=sourceIndex:used",
        "search-overlay:skipped",
        "O=owner:path(src/lib.rs)",
    ] {
        assert!(
            stdout.contains(expected),
            "source-index lexical scenario missing {expected:?}; stdout={stdout}"
        );
    }
    assert!(
        !stdout.contains("sourceTrace=finder:used"),
        "lexical warm SourceIndex path must not collect through search overlay; stdout={stdout}"
    );
    assert!(
        !marker.exists(),
        "source-index warm lexical should not spawn provider"
    );
    let collect_ms = source_trace_metric_ms(&stdout, "collectMs");
    assert!(
        collect_ms <= max_total_ms,
        "source-index warm lexical exceeded benchmark max_total={} observed={}ms stdout={stdout}",
        benchmark.max_total,
        collect_ms
    );
    let observed_total = format!("{collect_ms}ms");
    let performance_gate = serde_json::json!({
        "schemaId": "agent.semantic-protocols.semantic-hot-path-performance-gate",
        "schemaVersion": "1",
        "scenarioId": "asp-lexical-source-index-warm-path",
        "languageId": "rust",
        "workspace": ".",
        "command": [
            "asp",
            "rust",
            "search",
            "lexical",
            "source_index_fixture",
            "owner",
            "items",
            "tests",
            "--workspace",
            ".",
            "--view",
            "seeds"
        ],
        "phase": "hot",
        "expected": {
            "targetTotal": benchmark.target_total,
            "maxTotal": benchmark.max_total,
            "regressionBudget": benchmark.regression_budget,
            "maxProviderProcessCount": 0,
            "maxSearchOverlayProcessCount": 0,
            "maxRenderDuration": benchmark.max_total,
            "maxStdoutBytes": 8192,
            "requireSourceIndexHit": true,
            "allowedFirstRoutes": ["source-index"],
            "forbiddenRoutes": ["prime", "native-finder", "provider-process"],
            "requireExactCodeIdentity": false,
            "requireNoExecutableLineRange": true
        },
        "observed": {
            "observedTotal": observed_total,
            "providerProcessCount": 0,
            "providerElapsed": "0us",
            "nativeFinderProcessCount": 0,
            "nativeFinderElapsed": "0us",
            "sourceIndexHit": true,
            "sourceIndexDuration": observed_total,
            "firstRoute": "source-index",
            "executedRoutes": ["source-index", "owner-items", "tests"],
            "executableLineRangeSelectorCount": 0,
            "packetOutMode": "not-applicable",
            "renderDuration": observed_total,
            "stdoutBytes": stdout.len()
        },
        "verdict": "pass",
        "evidenceRefs": ["scenario:asp-lexical-source-index-warm-path"]
    });
    assert_eq!(performance_gate["observed"]["providerProcessCount"], 0);
    assert_eq!(performance_gate["observed"]["nativeFinderProcessCount"], 0);
    assert_eq!(performance_gate["observed"]["sourceIndexHit"], true);
    assert_eq!(performance_gate["observed"]["stdoutBytes"], stdout.len());
    let _ = fs::remove_dir_all(root);
}

pub(super) fn asp_rust_owner_items_cold_functional_path_stays_inside_scenario_gate() {
    assert_owner_items_cold_functional_path(OwnerItemsColdFunctionalScenario {
        scenario_dir: "asp_rust_owner_items_cold_functional_path",
        scenario_id: "asp-rust-owner-items-cold-functional-path",
        language_id: "rust",
        binary: "rs-harness",
        owner_path: "crate/src/lib.rs",
        package_anchor_path: "Cargo.toml",
        package_anchor_text: "[package]\nname = \"scenario-rust-owner-items-cold-functional\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
        source_text: "pub async fn dynamic_owner_item_index() {}\n",
        query: "dynamic_owner_item_index",
        alg: "rust-harness-owner-items",
        item_symbol: "dynamic_owner_item_index",
    });
}

pub(super) fn asp_typescript_owner_items_cold_functional_path_stays_inside_scenario_gate() {
    assert_owner_items_cold_functional_path(OwnerItemsColdFunctionalScenario {
        scenario_dir: "asp_typescript_owner_items_cold_functional_path",
        scenario_id: "asp-typescript-owner-items-cold-functional-path",
        language_id: "typescript",
        binary: "ts-harness",
        owner_path: "app/src/model.ts",
        package_anchor_path: "package.json",
        package_anchor_text: "{\"name\":\"scenario-typescript-owner-items-cold-functional\",\"private\":true}\n",
        source_text: "export function dynamicOwnerItemIndex(): boolean { return true; }\n",
        query: "dynamicOwnerItemIndex",
        alg: "ts-harness-owner-items",
        item_symbol: "dynamicOwnerItemIndex",
    });
}

pub(super) fn asp_python_owner_items_cold_functional_path_stays_inside_scenario_gate() {
    assert_owner_items_cold_functional_path(OwnerItemsColdFunctionalScenario {
        scenario_dir: "asp_python_owner_items_cold_functional_path",
        scenario_id: "asp-python-owner-items-cold-functional-path",
        language_id: "python",
        binary: "py-harness",
        owner_path: "src/model.py",
        package_anchor_path: "pyproject.toml",
        package_anchor_text: "[project]\nname = \"scenario-python-owner-items-cold-functional\"\nversion = \"0.1.0\"\n",
        source_text: "def dynamic_owner_item_index() -> bool:\n    return True\n",
        query: "dynamic_owner_item_index",
        alg: "py-harness-owner-items",
        item_symbol: "dynamic_owner_item_index",
    });
}

pub(super) fn asp_julia_owner_items_cold_functional_path_stays_inside_scenario_gate() {
    assert_owner_items_cold_functional_path(OwnerItemsColdFunctionalScenario {
        scenario_dir: "asp_julia_owner_items_cold_functional_path",
        scenario_id: "asp-julia-owner-items-cold-functional-path",
        language_id: "julia",
        binary: "asp-julia-harness",
        owner_path: "src/Model.jl",
        package_anchor_path: "Project.toml",
        package_anchor_text: "name = \"ScenarioJuliaOwnerItemsColdFunctional\"\nversion = \"0.1.0\"\n",
        source_text: "dynamic_owner_item_index() = true\n",
        query: "dynamic_owner_item_index",
        alg: "asp-julia-harness-owner-items",
        item_symbol: "dynamic_owner_item_index",
    });
}

pub(super) fn asp_gerbil_scheme_owner_items_cold_functional_path_stays_inside_scenario_gate() {
    assert_owner_items_cold_functional_path(OwnerItemsColdFunctionalScenario {
        scenario_dir: "asp_gerbil_scheme_owner_items_cold_functional_path",
        scenario_id: "asp-gerbil-scheme-owner-items-cold-functional-path",
        language_id: "gerbil-scheme",
        binary: "gslph",
        owner_path: "src/model.ss",
        package_anchor_path: "gerbil.pkg",
        package_anchor_text: "(package: scenario-gerbil-owner-items-cold-functional)\n",
        source_text: "(def (dynamic-owner-item-index) #t)\n",
        query: "dynamic-owner-item-index",
        alg: "gslph-owner-items",
        item_symbol: "dynamic-owner-item-index",
    });
}

struct OwnerItemsColdFunctionalScenario {
    scenario_dir: &'static str,
    scenario_id: &'static str,
    language_id: &'static str,
    binary: &'static str,
    owner_path: &'static str,
    package_anchor_path: &'static str,
    package_anchor_text: &'static str,
    source_text: &'static str,
    query: &'static str,
    alg: &'static str,
    item_symbol: &'static str,
}

fn assert_owner_items_cold_functional_path(spec: OwnerItemsColdFunctionalScenario) {
    owner_items::assert_owner_items_cold_functional_path(spec);
}

pub(super) fn asp_unit_scenarios_cover_workspace_argument_guards() {
    let scenario_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("unit")
        .join("scenarios");
    let policy_ids = discover_scenario_policy_ids(&scenario_root);
    let missing = REQUIRED_WORKSPACE_ARGUMENT_POLICY_IDS
        .iter()
        .copied()
        .filter(|policy_id| !policy_ids.contains(*policy_id))
        .collect::<Vec<_>>();

    assert!(
        missing.is_empty(),
        "ASP unit scenarios must cover workspace argument guardrails before provider spawn; missing={missing:?}; observed={policy_ids:?}"
    );
}

pub(super) fn language_harnesses_have_shared_scenario_benchmark_schema_coverage() {
    let repo_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    for schema_path in [
        SHARED_SCENARIO_BENCHMARK_SCHEMA,
        SHARED_AGENT_POLICY_ID_SCHEMA,
    ] {
        assert!(
            repo_root.join(schema_path).is_file(),
            "shared scenario benchmark schema is missing: {schema_path}"
        );
    }

    let mut missing = Vec::new();
    let mut invalid = Vec::new();
    let mut hot_path_coverage = BTreeSet::new();
    for requirement in LANGUAGE_SCENARIO_BENCHMARK_REQUIREMENTS {
        let root = repo_root.join(requirement.root);
        match requirement.syntax {
            ScenarioBenchmarkSyntax::TomlPair => {
                let pairs = discover_toml_scenario_benchmark_roots(&root);
                if pairs.is_empty() {
                    missing.push(format!("{}:{}", requirement.language, requirement.root));
                }
                for pair_root in pairs {
                    validate_toml_scenario_benchmark(
                        requirement.language,
                        &pair_root,
                        &mut invalid,
                        &mut hot_path_coverage,
                    );
                }
            }
            ScenarioBenchmarkSyntax::GerbilBenchmarkSs => {
                let paths = discover_benchmark_ss_files(&root);
                if paths.is_empty() {
                    missing.push(format!("{}:{}", requirement.language, requirement.root));
                }
                for path in paths {
                    validate_gerbil_benchmark_ss(&path, &mut invalid, &mut hot_path_coverage);
                }
            }
        }
    }

    assert!(
        missing.is_empty(),
        "language harnesses must each expose shared scenario benchmark coverage through benchmark.toml or benchmark.ss; missing={missing:?}"
    );
    assert!(
        invalid.is_empty(),
        "language scenario benchmark manifests must satisfy {SHARED_SCENARIO_BENCHMARK_SCHEMA}; invalid={invalid:?}"
    );
    let missing_hot_path = LANGUAGE_SCENARIO_BENCHMARK_REQUIREMENTS
        .iter()
        .map(|requirement| requirement.language)
        .filter(|language| !hot_path_coverage.contains(*language))
        .collect::<Vec<_>>();
    assert!(
        missing_hot_path.is_empty(),
        "language harnesses must each expose at least one scenario benchmark with route_source/max_provider_process_count/max_stdout_bytes/fallback_reason hot-path metadata; missing={missing_hot_path:?}; observed={hot_path_coverage:?}"
    );
}

pub(super) fn large_library_sandtables_have_hard_elapsed_gates() {
    let repo_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let required_targets = [
        ("julia", "DataFrames"),
        ("julia", "Flux"),
        ("julia", "Makie"),
        ("python", "fastapi"),
        ("python", "pandas"),
        ("python", "rich"),
        ("rust", "bytes"),
        ("rust", "ignore"),
        ("rust", "tokio"),
        ("typescript", "playwright"),
        ("typescript", "typescript"),
        ("typescript", "vite"),
    ];
    let gates = discover_large_library_elapsed_gates(&repo_root);
    let covered = gates
        .iter()
        .map(|gate| (gate.language.as_str(), gate.package.as_str()))
        .collect::<BTreeSet<_>>();
    let missing = required_targets
        .iter()
        .copied()
        .filter(|target| !covered.contains(target))
        .collect::<Vec<_>>();

    assert!(
        missing.is_empty(),
        "large-library sandtables missing expect.maxElapsedMs hard gate; missing={missing:?}; observed={}",
        render_gates(&repo_root, &gates)
    );
    let missing_step_gates = gates
        .iter()
        .filter(|gate| gate.max_elapsed_ms.is_none_or(|value| value == 0))
        .cloned()
        .collect::<Vec<_>>();
    assert!(
        missing_step_gates.is_empty(),
        "every large-library sandtable step must declare expect.maxElapsedMs; missing={}",
        render_gates(&repo_root, &missing_step_gates)
    );
    let too_slow = gates
        .iter()
        .filter(|gate| {
            gate.max_elapsed_ms
                .is_some_and(|value| value > large_library_step_max_elapsed_ms(&gate.language))
        })
        .cloned()
        .collect::<Vec<_>>();
    assert!(
        too_slow.is_empty(),
        "large-library sandtables must stay inside {LARGE_LIBRARY_STEP_MAX_ELAPSED_MS}ms hard gates, except Julia warmup allowance {JULIA_LARGE_LIBRARY_STEP_MAX_ELAPSED_MS}ms; slow={}",
        render_gates(&repo_root, &too_slow)
    );
}

pub(super) fn julia_dataframes_sandtable_batch_execution_stays_inside_hard_gates() {
    let repo_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let scenario_path = repo_root.join("sandtables/julia/dataframes-intent-matrix.json");
    let scenario = read_json(&scenario_path);
    let workdir = match resolve_sandtable_workdir(&repo_root, &scenario) {
        Some(workdir) => workdir,
        None => {
            eprintln!(
                "skip julia DataFrames execution gate: no workdir for {}",
                scenario_path.display()
            );
            return;
        }
    };
    let binary = repo_root.join(".bin/asp-julia-harness");
    if !binary.is_file() {
        eprintln!(
            "skip julia DataFrames execution gate: missing {}",
            binary.display()
        );
        return;
    }
    let steps = scenario
        .get("steps")
        .and_then(Value::as_array)
        .expect("DataFrames scenario must define steps");
    let batch_input = steps
        .iter()
        .map(batch_argv_line)
        .collect::<Vec<_>>()
        .join("\n")
        + "\n";
    let samples = (0..JULIA_DATAFRAMES_BATCH_SAMPLE_COUNT)
        .map(|_| {
            let output = run_julia_batch(&binary, &workdir, &batch_input);
            let observed = parse_julia_batch_steps(&output);

            assert_eq!(
                observed.len(),
                steps.len(),
                "julia batch output count mismatch\nstdout:\n{output}"
            );
            observed
        })
        .collect::<Vec<_>>();

    for (step_index, step) in steps.iter().enumerate() {
        let step_id = string_field(step, "id").unwrap_or_else(|| "<unknown>".to_string());
        let step_samples = samples
            .iter()
            .map(|sample| &sample[step_index])
            .collect::<Vec<_>>();

        for observed_step in &step_samples {
            assert_eq!(
                observed_step.exit_code, 0,
                "julia batch step {step_id} failed with exit={}:\n{}",
                observed_step.exit_code, observed_step.stdout
            );
        }

        let best_elapsed_ms = step_samples
            .iter()
            .map(|observed_step| observed_step.elapsed_ms)
            .min()
            .expect("julia batch performance samples must not be empty");
        let sample_elapsed_ms = step_samples
            .iter()
            .map(|observed_step| observed_step.elapsed_ms.to_string())
            .collect::<Vec<_>>()
            .join(",");
        assert!(
            best_elapsed_ms <= JULIA_DATAFRAMES_BATCH_STEP_MAX_ELAPSED_MS,
            "julia batch step {step_id} best {best_elapsed_ms}ms exceeds hard gate {}ms across samples [{sample_elapsed_ms}]",
            JULIA_DATAFRAMES_BATCH_STEP_MAX_ELAPSED_MS,
        );
    }
}

pub(super) fn python_sandtable_runner_does_not_resolve_language_harness_binaries() {
    let repo_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let runner_path =
        repo_root.join("packages/python/tools/src/tools/semantic_sandtable/step_process.py");
    let runner = fs::read_to_string(&runner_path)
        .unwrap_or_else(|error| panic!("read {}: {error}", runner_path.display()));
    for forbidden in [
        "_typescript_harness_dist_entry",
        "_rust_harness_entry",
        "_python_harness_entry",
        "_julia_harness_entry",
        "command[0] == \"rs-harness\"",
        "command[0] == \"ts-harness\"",
        "command[0] == \"asp-julia-harness\"",
        "command[1] == \"python\"",
        "\"py-harness\"",
    ] {
        assert!(
            !runner.contains(forbidden),
            "Python sandtable runner must not resolve language harness binaries; found {forbidden} in {}",
            runner_path.display()
        );
    }
}

fn discover_scenario_policy_ids(scenario_root: &Path) -> BTreeSet<String> {
    let mut policy_ids = BTreeSet::new();
    for path in read_dir_sorted(scenario_root) {
        if !path.is_dir() || is_non_scenario_dir(&path) {
            continue;
        }
        let scenario_path = path.join("scenario.toml");
        if !scenario_path.is_file() {
            continue;
        }
        let text = fs::read_to_string(&scenario_path)
            .unwrap_or_else(|error| panic!("read {}: {error}", scenario_path.display()));
        policy_ids.extend(policy_ids_from_scenario_toml(&scenario_path, &text));
    }
    policy_ids
}

fn discover_toml_scenario_benchmark_roots(root: &Path) -> Vec<PathBuf> {
    let mut roots = Vec::new();
    collect_toml_scenario_benchmark_roots(root, &mut roots);
    roots.sort();
    roots
}

fn collect_toml_scenario_benchmark_roots(root: &Path, roots: &mut Vec<PathBuf>) {
    let scenario_path = root.join("scenario.toml");
    let benchmark_path = root.join("benchmark.toml");
    if scenario_path.is_file() || benchmark_path.is_file() {
        assert!(
            scenario_path.is_file() && benchmark_path.is_file(),
            "scenario benchmark root must carry both scenario.toml and benchmark.toml: {}",
            root.display()
        );
        roots.push(root.to_path_buf());
        return;
    }
    for path in read_dir_sorted(root) {
        if path.is_dir() && !is_non_scenario_dir(&path) {
            collect_toml_scenario_benchmark_roots(&path, roots);
        }
    }
}

fn discover_benchmark_ss_files(root: &Path) -> Vec<PathBuf> {
    let mut paths = Vec::new();
    collect_benchmark_ss_files(root, &mut paths);
    paths.sort();
    paths
}

fn collect_benchmark_ss_files(root: &Path, paths: &mut Vec<PathBuf>) {
    for path in read_dir_sorted(root) {
        if path.is_dir() && !is_non_scenario_dir(&path) {
            collect_benchmark_ss_files(&path, paths);
        } else if path.file_name().and_then(|name| name.to_str()) == Some("benchmark.ss") {
            paths.push(path);
        }
    }
}

fn validate_toml_scenario_benchmark(
    language: &str,
    root: &Path,
    invalid: &mut Vec<String>,
    hot_path_coverage: &mut BTreeSet<&'static str>,
) {
    let scenario_path = root.join("scenario.toml");
    let benchmark_path = root.join("benchmark.toml");
    let scenario: SharedScenarioToml = read_toml(&scenario_path);
    let benchmark: SharedBenchmarkToml = read_toml(&benchmark_path);

    require_non_empty_manifest_field(invalid, &scenario_path, "id", &scenario.id);
    require_non_empty_manifest_field(invalid, &scenario_path, "title", &scenario.title);
    require_non_empty_manifest_field(invalid, &scenario_path, "agent_goal", &scenario.agent_goal);
    require_non_empty_manifest_field(invalid, &scenario_path, "inputs", &scenario.inputs);
    require_non_empty_manifest_field(invalid, &scenario_path, "expected", &scenario.expected);
    validate_language_harness_json_boundary(root, &scenario, invalid);
    if scenario.policy_ids.is_empty() {
        invalid.push(format!(
            "{}: scenario.policy_ids must not be empty",
            scenario_path.display()
        ));
    }
    for policy_id in &scenario.policy_ids {
        if !is_agent_policy_id(policy_id) {
            invalid.push(format!(
                "{}: policy id {policy_id:?} must match {AGENT_POLICY_ID_GRAMMAR}",
                scenario_path.display()
            ));
        }
    }

    require_supported_language_harness(language, &benchmark_path, &benchmark.harness, invalid);
    if benchmark.test.as_deref().unwrap_or("").trim().is_empty()
        && benchmark.bench.as_deref().unwrap_or("").trim().is_empty()
    {
        invalid.push(format!(
            "{}: benchmark must name test or bench",
            benchmark_path.display()
        ));
    }
    for (field, value) in [
        ("target_total", benchmark.target_total.as_str()),
        ("max_total", benchmark.max_total.as_str()),
        ("observed_total", benchmark.observed_total.as_str()),
        ("regression_budget", benchmark.regression_budget.as_str()),
    ] {
        require_positive_duration_manifest_field(invalid, &benchmark_path, field, value);
    }
    if benchmark.memory_budget_bytes == 0 || benchmark.observed_memory_bytes == 0 {
        invalid.push(format!(
            "{}: memory budget and observed memory must be positive",
            benchmark_path.display()
        ));
    }
    require_non_empty_manifest_field(
        invalid,
        &benchmark_path,
        "target_rationale",
        &benchmark.target_rationale,
    );
    if benchmark.observed_timings.is_empty() {
        invalid.push(format!(
            "{}: benchmark.observed_timings must not be empty",
            benchmark_path.display()
        ));
    }
    for (field, value) in &benchmark.observed_timings {
        require_observed_timing_manifest_field(invalid, &benchmark_path, field, value);
    }
    if benchmark_has_hot_path_metadata(&benchmark) {
        hot_path_coverage.insert(canonical_benchmark_language(language));
    }
}

fn validate_language_harness_json_boundary(
    root: &Path,
    scenario: &SharedScenarioToml,
    invalid: &mut Vec<String>,
) {
    let rendered_text = root.join(&scenario.expected).join("rendered.txt");
    if rendered_text.exists() {
        invalid.push(format!(
            "{}: language harness scenario benchmarks must expose JSON schema data only; ASP owns render output",
            rendered_text.display()
        ));
    }
}

pub(super) fn language_harnesses_do_not_use_retired_agent_policy_ids() {
    let languages = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../languages");
    let mut invalid = Vec::new();
    for relative in RETIRED_POLICY_ID_SCAN_PATHS {
        let path = languages.join(relative);
        if path.exists() {
            collect_retired_policy_ids(&path, &mut invalid);
        }
    }

    assert!(
        invalid.is_empty(),
        "retired agent policy ids must use {AGENT_POLICY_ID_GRAMMAR}:\n{}",
        invalid.join("\n")
    );
}

const RETIRED_POLICY_ID_SCAN_PATHS: &[&str] = &[
    "JuliaLangProjectHarness.jl/src",
    "JuliaLangProjectHarness.jl/docs",
    "JuliaLangProjectHarness.jl/test",
    "JuliaLangProjectHarness.jl/tests",
    "gerbil-scheme-language-project-harness/docs",
    "gerbil-scheme-language-project-harness/src",
    "gerbil-scheme-language-project-harness/t",
    "org/contracts",
    "org/docs",
    "org/src",
    "org/tests",
    "orgize/benches",
    "orgize/docs",
    "orgize/src",
    "orgize/tests",
    "orgize/wasm/src",
    "python-lang-project-harness/src",
    "python-lang-project-harness/docs",
    "python-lang-project-harness/tests",
    "rust-lang-project-harness/src",
    "rust-lang-project-harness/docs",
    "rust-lang-project-harness/tests",
    "typescript-lang-project-harness/src",
    "typescript-lang-project-harness/docs",
    "typescript-lang-project-harness/tests",
];

fn validate_gerbil_benchmark_ss(
    path: &Path,
    invalid: &mut Vec<String>,
    hot_path_coverage: &mut BTreeSet<&'static str>,
) {
    let text =
        fs::read_to_string(path).unwrap_or_else(|error| panic!("read {}: {error}", path.display()));
    for token in [
        "max_total",
        "observed_total",
        "target_total",
        "regression_budget",
        "observedTimings",
        "targetRationale",
        "maxRssMb",
        "rule",
        "purpose",
    ] {
        if !text.contains(token) {
            invalid.push(format!("{}: benchmark.ss missing {token}", path.display()));
        }
    }
    if [
        "routeSource",
        "maxProviderProcessCount",
        "maxStdoutBytes",
        "fallbackReason",
    ]
    .iter()
    .all(|token| text.contains(token))
    {
        hot_path_coverage.insert("gerbil-scheme");
    }
    match gerbil_benchmark_rule(&text) {
        Some(rule) if is_agent_policy_id(rule) => {}
        Some(rule) => invalid.push(format!(
            "{}: rule {rule:?} must match {AGENT_POLICY_ID_GRAMMAR}",
            path.display()
        )),
        None => invalid.push(format!(
            "{}: benchmark.ss missing rule value",
            path.display()
        )),
    }
}

fn benchmark_has_hot_path_metadata(benchmark: &SharedBenchmarkToml) -> bool {
    benchmark
        .route_source
        .as_deref()
        .is_some_and(|value| !value.trim().is_empty())
        && benchmark.max_provider_process_count.is_some()
        && benchmark.max_stdout_bytes.is_some()
        && benchmark
            .fallback_reason
            .as_deref()
            .is_some_and(|value| !value.trim().is_empty())
}

fn canonical_benchmark_language(language: &str) -> &'static str {
    match language {
        "rust" => "rust",
        "typescript" => "typescript",
        "python" => "python",
        "julia" => "julia",
        "gerbil-scheme" => "gerbil-scheme",
        "orgize" => "orgize",
        _ => "unknown",
    }
}

fn gerbil_benchmark_rule(text: &str) -> Option<&str> {
    text.lines().find_map(|line| {
        let value = line.trim().strip_prefix("(rule . ")?;
        let value = value.trim_end_matches(')').trim();
        Some(
            value
                .strip_prefix('"')
                .and_then(|value| value.strip_suffix('"'))
                .unwrap_or_else(|| value.trim_start_matches('\'')),
        )
    })
}

fn collect_retired_policy_ids(dir: &Path, invalid: &mut Vec<String>) {
    if is_ignored_retired_policy_scan_path(dir) {
        return;
    }

    let entries = match fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(err) => {
            invalid.push(format!(
                "{}: failed to read directory: {err}",
                dir.display()
            ));
            return;
        }
    };

    for entry in entries {
        let entry = match entry {
            Ok(entry) => entry,
            Err(err) => {
                invalid.push(format!(
                    "{}: failed to read directory entry: {err}",
                    dir.display()
                ));
                continue;
            }
        };
        let path = entry.path();
        if is_ignored_retired_policy_scan_path(&path) {
            continue;
        }
        if path.is_dir() {
            collect_retired_policy_ids(&path, invalid);
        } else if path.is_file() {
            validate_no_retired_policy_ids(&path, invalid);
        }
    }
}

fn validate_no_retired_policy_ids(path: &Path, invalid: &mut Vec<String>) {
    let Ok(text) = fs::read_to_string(path) else {
        return;
    };

    for (line_index, line) in text.lines().enumerate() {
        for token in policy_id_tokens(line) {
            if is_retired_policy_id(token) {
                invalid.push(format!(
                    "{}:{}: retired policy id {token:?} must match {AGENT_POLICY_ID_GRAMMAR}",
                    path.display(),
                    line_index + 1
                ));
            }
        }
    }
}

fn policy_id_tokens(line: &str) -> impl Iterator<Item = &str> {
    line.split(|ch: char| !(ch.is_ascii_alphanumeric() || ch == '-' || ch == '_'))
        .filter(|token| !token.is_empty())
}

fn is_retired_policy_id(token: &str) -> bool {
    has_numbered_retired_marker(token, "-AGENT-R") || has_numbered_retired_marker(token, "-PROJ-R")
}

fn has_numbered_retired_marker(token: &str, marker: &str) -> bool {
    let Some(index) = token.find(marker) else {
        return false;
    };
    token[index + marker.len()..]
        .chars()
        .next()
        .is_some_and(|ch| ch.is_ascii_digit())
}

fn is_ignored_retired_policy_scan_path(path: &Path) -> bool {
    path.components().any(|component| {
        matches!(
            component.as_os_str().to_str(),
            Some(
                ".git"
                    | ".data"
                    | ".mypy_cache"
                    | ".pytest_cache"
                    | ".ruff_cache"
                    | ".venv"
                    | "__pycache__"
                    | "build"
                    | "coverage"
                    | "dist"
                    | "node_modules"
                    | "target"
            )
        )
    })
}

fn require_non_empty_manifest_field(
    invalid: &mut Vec<String>,
    path: &Path,
    field: &str,
    value: &str,
) {
    if value.trim().is_empty() {
        invalid.push(format!("{}: {field} must not be empty", path.display()));
    }
}

fn require_positive_duration_manifest_field(
    invalid: &mut Vec<String>,
    path: &Path,
    field: &str,
    value: &str,
) {
    let trimmed = value.trim();
    if !is_positive_duration_literal(trimmed) {
        invalid.push(format!(
            "{}: {field}={value:?} must be a positive duration such as 500us or 25ms",
            path.display()
        ));
    }
}

fn require_observed_timing_manifest_field(
    invalid: &mut Vec<String>,
    path: &Path,
    field: &str,
    value: &toml::Value,
) {
    let Some(value) = value.as_str() else {
        invalid.push(format!(
            "{}: observed_timings.{field} must be a duration string",
            path.display()
        ));
        return;
    };
    let trimmed = value.trim();
    if !is_duration_literal(trimmed) {
        invalid.push(format!(
            "{}: observed_timings.{field}={value:?} must use ns/us/ms/s duration units",
            path.display()
        ));
    }
    if trimmed == "0ms" {
        invalid.push(format!(
            "{}: observed_timings.{field} must use 0us for zero-duration branches, not 0ms",
            path.display()
        ));
    }
}

fn is_positive_duration_literal(value: &str) -> bool {
    is_duration_literal(value) && value.chars().next().is_some_and(|ch| ch != '0')
}

fn is_duration_literal(value: &str) -> bool {
    !value.is_empty()
        && ["ns", "us", "ms", "s"]
            .iter()
            .any(|suffix| value.strip_suffix(suffix).is_some_and(is_ascii_digits))
}

fn duration_millis_from_manifest(value: &str) -> u128 {
    let trimmed = value.trim();
    if let Some(value) = trimmed.strip_suffix("ns").and_then(parse_u128) {
        return value.div_ceil(1_000_000);
    }
    if let Some(value) = trimmed.strip_suffix("us").and_then(parse_u128) {
        return value.div_ceil(1_000);
    }
    if let Some(value) = trimmed.strip_suffix("ms").and_then(parse_u128) {
        return value;
    }
    if let Some(value) = trimmed.strip_suffix('s').and_then(parse_u128) {
        return value * 1_000;
    }
    panic!("duration manifest value must use ns/us/ms/s suffix: {value:?}");
}

fn observed_timing_millis_from_manifest(benchmark: &SharedBenchmarkToml, key: &str) -> u128 {
    let value = benchmark
        .observed_timings
        .get(key)
        .unwrap_or_else(|| panic!("benchmark must record observed timing key {key:?}"));
    let Some(text) = value.as_str() else {
        panic!("observed timing key {key:?} must be a duration string, got {value:?}");
    };
    duration_millis_from_manifest(text)
}

fn assert_observed_timing_inside_budget(
    benchmark: &SharedBenchmarkToml,
    key: &str,
    max_millis: u128,
    label: &str,
) {
    let observed = observed_timing_millis_from_manifest(benchmark, key);
    assert!(
        observed <= max_millis,
        "{label} observed timing {key}={observed}ms exceeds budget {max_millis}ms"
    );
}

fn parse_u128(value: &str) -> Option<u128> {
    value.parse::<u128>().ok()
}

fn duration_literal(duration: std::time::Duration) -> String {
    let micros = duration.as_micros();
    if micros == 0 {
        format!("{}ns", duration.as_nanos())
    } else if micros < 1_000 {
        format!("{micros}us")
    } else {
        format!("{}ms", duration.as_millis())
    }
}

fn source_trace_metric_ms(stdout: &str, metric: &str) -> u128 {
    let needle = format!("{metric}=");
    let Some(start) = stdout.find(&needle).map(|index| index + needle.len()) else {
        panic!("sourceTrace missing metric {metric:?}: {stdout}");
    };
    let digits = stdout[start..]
        .chars()
        .take_while(|ch| ch.is_ascii_digit())
        .collect::<String>();
    digits
        .parse::<u128>()
        .unwrap_or_else(|error| panic!("invalid metric {metric:?} in stdout={stdout}: {error}"))
}

fn require_supported_language_harness(
    language: &str,
    path: &Path,
    harness: &str,
    invalid: &mut Vec<String>,
) {
    let supported = match language {
        "rust" | "orgize" => ["libtest", "criterion", "divan", "iai-callgrind"].contains(&harness),
        "typescript" => harness == "vitest",
        "python" => harness == "pytest",
        "julia" => harness == "julia-test",
        _ => false,
    };
    if !supported {
        invalid.push(format!(
            "{}: harness {harness:?} is not supported for language {language}",
            path.display()
        ));
    }
}

fn resolve_sandtable_workdir(repo_root: &Path, scenario: &Value) -> Option<PathBuf> {
    let workdir = scenario.get("workdir")?;
    if let Some(env_name) = workdir.get("env").and_then(Value::as_str)
        && let Some(path) = env::var_os(env_name)
            .map(PathBuf::from)
            .filter(|path| path.is_dir())
    {
        return Some(path);
    }
    for candidate in workdir.get("candidates").and_then(Value::as_array)? {
        let Some(pattern) = candidate.as_str() else {
            continue;
        };
        if let Some(path) = resolve_sandtable_workdir_candidate(repo_root, pattern) {
            return Some(path);
        }
    }
    None
}

fn resolve_sandtable_workdir_candidate(repo_root: &Path, pattern: &str) -> Option<PathBuf> {
    let expanded = expand_home_path(pattern);
    if !expanded.contains('*') {
        let path = normalize_candidate_path(repo_root, &expanded);
        return path.is_dir().then_some(path);
    }
    let (prefix, suffix) = expanded.split_once('*')?;
    let root = normalize_candidate_path(repo_root, prefix.trim_end_matches('/'));
    let mut entries = fs::read_dir(root)
        .ok()?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.is_dir())
        .filter(|path| {
            suffix.is_empty()
                || path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| name.ends_with(suffix.trim_start_matches('/')))
        })
        .collect::<Vec<_>>();
    entries.sort();
    entries.pop()
}

fn expand_home_path(pattern: &str) -> String {
    if let Some(rest) = pattern.strip_prefix("~/")
        && let Some(home) = env::var_os("HOME")
    {
        return PathBuf::from(home).join(rest).display().to_string();
    }
    pattern.to_string()
}

fn normalize_candidate_path(repo_root: &Path, value: &str) -> PathBuf {
    let path = PathBuf::from(value);
    if path.is_absolute() {
        path
    } else {
        repo_root.join(path)
    }
}

fn batch_argv_line(step: &Value) -> String {
    let command = step
        .get("command")
        .and_then(Value::as_array)
        .unwrap_or_else(|| panic!("step command must be an array: {step:?}"));
    let argv = command
        .iter()
        .map(|value| {
            value
                .as_str()
                .unwrap_or_else(|| panic!("command argument must be a string: {value:?}"))
        })
        .collect::<Vec<_>>();
    assert_eq!(
        argv.first().copied(),
        Some("asp"),
        "julia batch command must start with asp facade: {argv:?}"
    );
    assert_eq!(
        argv.get(1).copied(),
        Some("julia"),
        "julia batch command must select the julia facade: {argv:?}"
    );
    argv[2..].join("\t")
}

fn run_julia_batch(binary: &Path, workdir: &Path, batch_input: &str) -> String {
    let mut child = Command::new(binary)
        .arg("batch")
        .current_dir(workdir)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap_or_else(|error| panic!("spawn {}: {error}", binary.display()));
    child
        .stdin
        .as_mut()
        .expect("julia batch stdin must be piped")
        .write_all(batch_input.as_bytes())
        .expect("write julia batch stdin");
    let output = child.wait_with_output().expect("wait for julia batch");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "julia batch process failed status={} stderr={stderr}",
        output.status
    );
    String::from_utf8(output.stdout).expect("julia batch stdout must be utf-8")
}

#[derive(Clone, Debug)]
struct JuliaBatchStep {
    exit_code: i32,
    elapsed_ms: u64,
    stdout: String,
}

fn parse_julia_batch_steps(stdout: &str) -> Vec<JuliaBatchStep> {
    let mut steps = Vec::new();
    let mut current: Option<JuliaBatchStep> = None;
    for line in stdout.split_inclusive('\n') {
        if let Some(header) = line.strip_prefix("%%ASP_JULIA_BATCH_STEP\t") {
            if let Some(step) = current.take() {
                steps.push(step);
            }
            current = Some(parse_julia_batch_step_header(header));
            continue;
        }
        if line.starts_with("%%ASP_JULIA_BATCH_END\t") {
            if let Some(step) = current.take() {
                steps.push(step);
            }
            continue;
        }
        if let Some(step) = current.as_mut() {
            step.stdout.push_str(line);
        }
    }
    if let Some(step) = current {
        steps.push(step);
    }
    steps
}

fn parse_julia_batch_step_header(header: &str) -> JuliaBatchStep {
    let fields = header.trim_end().split('\t').collect::<Vec<_>>();
    JuliaBatchStep {
        exit_code: fields
            .get(1)
            .and_then(|value| value.parse::<i32>().ok())
            .unwrap_or(-1),
        elapsed_ms: fields
            .get(3)
            .and_then(|value| value.parse::<u64>().ok())
            .unwrap_or(u64::MAX),
        stdout: String::new(),
    }
}

fn policy_ids_from_scenario_toml(path: &Path, text: &str) -> Vec<String> {
    toml::from_str::<ScenarioPolicyIds>(text)
        .unwrap_or_else(|error| panic!("failed to parse {}: {error}", path.display()))
        .policy_ids
}

fn is_agent_policy_id(value: &str) -> bool {
    let parts = value.split('-').collect::<Vec<_>>();
    if parts.len() < 4 {
        return false;
    }
    let Some(agent_index) = parts.iter().position(|part| *part == "AGENT") else {
        return false;
    };
    if agent_index == 0 || agent_index + 2 >= parts.len() {
        return false;
    }
    parts[..agent_index].iter().all(|part| is_upper_token(part))
        && parts[agent_index + 1..parts.len() - 1]
            .iter()
            .all(|part| is_upper_token(part))
        && parts
            .last()
            .is_some_and(|number| number.len() >= 3 && is_ascii_digits(number))
}

fn is_upper_token(value: &str) -> bool {
    let mut chars = value.chars();
    chars.next().is_some_and(|ch| ch.is_ascii_uppercase())
        && chars.all(|ch| ch.is_ascii_uppercase() || ch.is_ascii_digit())
}

fn is_ascii_digits(value: &str) -> bool {
    !value.is_empty() && value.chars().all(|ch| ch.is_ascii_digit())
}

fn large_library_step_max_elapsed_ms(language: &str) -> u64 {
    if language == "julia" {
        JULIA_LARGE_LIBRARY_STEP_MAX_ELAPSED_MS
    } else {
        LARGE_LIBRARY_STEP_MAX_ELAPSED_MS
    }
}

#[derive(Clone, Debug)]
struct LargeLibraryElapsedGate {
    path: PathBuf,
    language: String,
    package: String,
    step_id: String,
    max_elapsed_ms: Option<u64>,
}

fn discover_large_library_elapsed_gates(repo_root: &Path) -> Vec<LargeLibraryElapsedGate> {
    let sandtables_root = repo_root.join("sandtables");
    let mut gates = Vec::new();
    for language_dir in read_dir_sorted(&sandtables_root) {
        if !language_dir.is_dir() || is_non_scenario_dir(&language_dir) {
            continue;
        }
        for scenario_path in read_dir_sorted(&language_dir) {
            if scenario_path.extension().and_then(|ext| ext.to_str()) != Some("json") {
                continue;
            }
            collect_large_library_elapsed_gates(&scenario_path, &mut gates);
        }
    }
    gates.sort_by(|left, right| {
        left.language
            .cmp(&right.language)
            .then_with(|| left.package.cmp(&right.package))
            .then_with(|| left.path.cmp(&right.path))
            .then_with(|| left.step_id.cmp(&right.step_id))
    });
    gates
}

fn collect_large_library_elapsed_gates(path: &Path, gates: &mut Vec<LargeLibraryElapsedGate>) {
    let scenario = read_json(path);
    if !is_large_library_scenario(&scenario) {
        return;
    }
    let language = string_field(&scenario, "language").unwrap_or_default();
    let target_library = scenario
        .get("evidence")
        .and_then(|evidence| evidence.get("targetLibrary"))
        .unwrap_or(&Value::Null);
    let package = string_field(target_library, "package").unwrap_or_default();
    let Some(steps) = scenario.get("steps").and_then(Value::as_array) else {
        return;
    };
    for step in steps {
        let max_elapsed_ms = step
            .get("expect")
            .and_then(|expect| expect.get("maxElapsedMs"))
            .and_then(Value::as_u64);
        gates.push(LargeLibraryElapsedGate {
            path: path.to_path_buf(),
            language: language.clone(),
            package: package.clone(),
            step_id: string_field(step, "id").unwrap_or_else(|| "<unknown>".to_string()),
            max_elapsed_ms,
        });
    }
}

fn is_large_library_scenario(scenario: &Value) -> bool {
    let evidence = scenario.get("evidence").unwrap_or(&Value::Null);
    let coverage = scenario.get("coverage").and_then(Value::as_array);
    evidence.get("fixtureTier").and_then(Value::as_str) == Some("large-library")
        && coverage.is_some_and(|items| {
            items
                .iter()
                .any(|item| item.as_str() == Some("large-library"))
        })
}

fn read_json(path: &Path) -> Value {
    let text = fs::read_to_string(path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display()));
    serde_json::from_str(&text)
        .unwrap_or_else(|error| panic!("failed to parse {}: {error}", path.display()))
}

fn read_toml<T>(path: &Path) -> T
where
    T: for<'de> Deserialize<'de>,
{
    let text = fs::read_to_string(path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display()));
    toml::from_str(&text)
        .unwrap_or_else(|error| panic!("failed to parse {}: {error}", path.display()))
}

fn read_dir_sorted(path: &Path) -> Vec<PathBuf> {
    let mut entries = fs::read_dir(path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display()))
        .map(|entry| {
            entry
                .unwrap_or_else(|error| {
                    panic!("failed to read entry under {}: {error}", path.display())
                })
                .path()
        })
        .collect::<Vec<_>>();
    entries.sort();
    entries
}

fn string_field(value: &Value, field: &str) -> Option<String> {
    value
        .get(field)
        .and_then(Value::as_str)
        .map(ToString::to_string)
}

fn is_non_scenario_dir(path: &Path) -> bool {
    matches!(
        path.file_name().and_then(|name| name.to_str()),
        Some("fixtures" | "receipts" | "root")
    )
}

fn render_gates(repo_root: &Path, gates: &[LargeLibraryElapsedGate]) -> String {
    gates
        .iter()
        .map(|gate| {
            format!(
                "{}:{}:{}:{}ms",
                gate.language,
                gate.package,
                gate.step_id,
                gate.max_elapsed_ms
                    .map_or_else(|| "missing".to_string(), |value| value.to_string())
            )
        })
        .chain(gates.iter().map(|gate| {
            format!(
                "path={}",
                gate.path
                    .strip_prefix(repo_root)
                    .unwrap_or(&gate.path)
                    .display()
            )
        }))
        .collect::<Vec<_>>()
        .join(", ")
}
pub(super) fn asp_turso_overlay_search_adapter_cold_functional_path_stays_inside_scenario_gate() {
    let crate_root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let scenario_root = crate_root
        .join("tests")
        .join("unit")
        .join("scenarios")
        .join("asp_turso_overlay_search_adapter_cold_functional_path");
    let benchmark: SharedBenchmarkToml = read_toml(&scenario_root.join("benchmark.toml"));
    assert_turso_overlay_benchmark_contract(&benchmark);
    let max_total_ms = duration_millis_from_manifest(&benchmark.max_total);

    let root = temp_project_root("scenario-turso-overlay-search-adapter-cold");
    let project_root = root.join("project");
    let state_home = root.join("state-home");
    fs::create_dir_all(&project_root).expect("create temp project root");
    let state = agent_semantic_runtime::state_core::ResolvedState::resolve_with_state_home(
        &project_root,
        &state_home,
    )
    .expect("resolve state with explicit state home");
    state.ensure_minimal_layout().expect("create state layout");
    let engine = agent_semantic_client_db::ClientDbEngine::from_resolved_state(&state);

    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("build tokio runtime");
    let hits = runtime.block_on(async {
        agent_semantic_search::bootstrap_turso_overlay_search_store(&engine)
            .await
            .expect("bootstrap turso overlay search store");
        engine
            .upsert_search_document(&agent_semantic_client_db::TursoClientDbSearchDocument {
                namespace: "stable".to_string(),
                document_id: "stable-owner".to_string(),
                entity_id: "stable-owner".to_string(),
                selector: Some("rust://src/lib.rs#item/function/stable_owner".to_string()),
                document: "stable overlay_fixture_token".to_string(),
            })
            .await
            .expect("upsert stable search document");
        agent_semantic_search::upsert_turso_overlay_search_document(
            &engine,
            &agent_semantic_search::TursoOverlaySearchDocument {
                repo_id: "repo-1".to_string(),
                workspace_id: "workspace-1".to_string(),
                session_id: "session-1".to_string(),
                base_generation: "dirty-1".to_string(),
                document_id: "overlay-owner".to_string(),
                selector: Some("rust://src/lib.rs#item/function/overlay_owner".to_string()),
                document: "dynamic overlay_fixture_token owner".to_string(),
            },
        )
        .await
        .expect("upsert turso overlay search document");
        let started = Instant::now();
        agent_semantic_search::search_turso_overlay_documents(&engine, "overlay_fixture_token", 8)
            .await
            .expect("search turso overlay documents")
            .into_iter()
            .map(|hit| (started.elapsed(), hit))
            .collect::<Vec<_>>()
    });
    let elapsed = hits
        .first()
        .map(|(elapsed, _)| *elapsed)
        .unwrap_or_default();
    let hits = hits.into_iter().map(|(_, hit)| hit).collect::<Vec<_>>();

    assert_eq!(hits.len(), 1, "{hits:#?}");
    assert_eq!(hits[0].document_id, "overlay-owner");
    assert_eq!(
        hits[0].selector.as_deref(),
        Some("rust://src/lib.rs#item/function/overlay_owner")
    );
    assert!(
        elapsed.as_millis() <= max_total_ms,
        "turso overlay search adapter cold functional path exceeded benchmark max_total={} observed={}ms hits={hits:#?}",
        benchmark.max_total,
        elapsed.as_millis()
    );
    assert!(
        !root.join(".cache").join("agent-semantic-protocol").exists(),
        "turso overlay search adapter must not create project-local cache"
    );

    let observed_total = duration_literal(elapsed);
    let performance_gate = serde_json::json!({
        "schemaId": "agent.semantic-protocols.semantic-hot-path-performance-gate",
        "schemaVersion": "1",
        "scenarioId": "asp-turso-overlay-search-adapter-cold-functional-path",
        "languageId": "rust",
        "workspace": ".",
        "command": [
            "agent_semantic_search::bootstrap_turso_overlay_search_store",
            "agent_semantic_search::upsert_turso_overlay_search_document",
            "agent_semantic_search::search_turso_overlay_documents"
        ],
        "phase": "cold",
        "expected": {
            "targetTotal": benchmark.target_total,
            "maxTotal": benchmark.max_total,
            "regressionBudget": benchmark.regression_budget,
            "maxProviderProcessCount": 0,
            "maxSearchOverlayProcessCount": 0,
            "requireOverlayHit": true,
            "allowedFirstRoutes": ["dynamic-overlay"],
            "forbiddenRoutes": ["native-finder", "provider-process", "project-local-cache"],
            "requireNoExecutableLineRange": true
        },
        "observed": {
            "observedTotal": observed_total,
            "providerProcessCount": 0,
            "nativeFinderProcessCount": 0,
            "overlayHit": true,
            "hitCount": hits.len(),
            "firstRoute": "turso-overlay",
            "executedRoutes": ["turso-overlay"],
            "executableLineRangeSelectorCount": 0,
            "fallbackReason": "none"
        },
        "verdict": "pass",
        "evidenceRefs": ["scenario:asp-turso-overlay-search-adapter-cold-functional-path"]
    });
    assert_eq!(performance_gate["observed"]["providerProcessCount"], 0);
    assert_eq!(performance_gate["observed"]["nativeFinderProcessCount"], 0);
    assert_eq!(performance_gate["observed"]["overlayHit"], true);
    let _ = fs::remove_dir_all(root);
}
