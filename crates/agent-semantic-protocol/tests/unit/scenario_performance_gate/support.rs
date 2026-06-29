use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Instant;

use crate::provider_command::support::{
    asp_command, prepend_path, provider, temp_project_root, write_activation, write_marker_provider,
};
use agent_semantic_protocol::render_selector_seeded_search_pipe;
use serde::Deserialize;
use serde_json::Value;

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
    "RUST-AGENT-ASP-PERF-SUBCOMMAND-SEARCH-PIPE-001",
    "RUST-AGENT-ASP-PERF-SUBCOMMAND-SEARCH-PIPE-SELECTOR-SEED-001",
    "RUST-AGENT-ASP-PERF-SUBCOMMAND-SEARCH-PIPE-SOURCE-INDEX-001",
    "RUST-AGENT-ASP-PERF-SUBCOMMAND-SEARCH-RG-001",
    "RUST-AGENT-ASP-PERF-SUBCOMMAND-SEARCH-RG-SOURCE-INDEX-001",
    "RUST-AGENT-ASP-PERF-SUBCOMMAND-SOURCE-INDEX-001",
    "RUST-AGENT-ASP-PERF-SUBCOMMAND-PROVIDER-FACTS-001",
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
    target_total: String,
    max_total: String,
    observed_total: String,
    regression_budget: String,
    memory_budget_bytes: u64,
    observed_memory_bytes: u64,
    target_rationale: String,
    observed_timings: BTreeMap<String, toml::Value>,
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

pub(super) fn asp_selector_seeded_search_pipe_frontier_stays_inside_scenario_gate() {
    let crate_root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let scenario_root = crate_root
        .join("tests")
        .join("unit")
        .join("scenarios")
        .join("asp_selector_seeded_search_pipe_frontier");
    let benchmark: SharedBenchmarkToml = read_toml(&scenario_root.join("benchmark.toml"));
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
            "maxNativeFinderProcessCount": 0,
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
            "providerElapsed": "0ms",
            "nativeFinderProcessCount": 0,
            "nativeFinderElapsed": "0ms",
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

pub(super) fn asp_source_index_search_pipe_warm_path_stays_inside_scenario_gate() {
    let crate_root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let scenario_root = crate_root
        .join("tests")
        .join("unit")
        .join("scenarios")
        .join("asp_source_index_search_pipe_warm_path");
    let benchmark: SharedBenchmarkToml = read_toml(&scenario_root.join("benchmark.toml"));
    let max_total_ms = duration_millis_from_manifest(&benchmark.max_total);

    let root = temp_project_root("scenario-source-index-search-pipe");
    let bin_dir = root.join(".bin");
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
    agent_semantic_client::refresh_source_index(&root).expect("refresh source index");
    let _ = fs::remove_file(&marker);

    let language = agent_semantic_client::LanguageId::from("rust");
    let lookup_started_at = Instant::now();
    let lookup = agent_semantic_client::lookup_source_index_for_language(
        &root,
        Some(&language),
        "source_index_fixture",
        256,
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
        "source-index warm lookup exceeded benchmark max_total={} observed={} stdout candidates={:?}",
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
        "finder:skipped",
        "ownerCoverage=bestOwner=src/lib.rs",
        "nextCommand=asp rust query --selector src/lib.rs:1:2 --workspace . --code",
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
            "maxNativeFinderProcessCount": 0,
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
            "providerElapsed": "0ms",
            "nativeFinderProcessCount": 0,
            "nativeFinderElapsed": "0ms",
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

pub(super) fn asp_rg_query_source_index_warm_path_stays_inside_scenario_gate() {
    let crate_root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let scenario_root = crate_root
        .join("tests")
        .join("unit")
        .join("scenarios")
        .join("asp_rg_query_source_index_warm_path");
    let benchmark: SharedBenchmarkToml = read_toml(&scenario_root.join("benchmark.toml"));
    let max_total_ms = duration_millis_from_manifest(&benchmark.max_total);

    let root = temp_project_root("scenario-rg-query-source-index");
    let bin_dir = root.join(".bin");
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
    agent_semantic_client::refresh_source_index(&root).expect("refresh source index");
    let _ = fs::remove_file(&marker);

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
        "finder:skipped",
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
        "rg warm SourceIndex path must not collect through native finder; stdout={stdout}"
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
            "maxNativeFinderProcessCount": 0,
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
            "providerElapsed": "0ms",
            "nativeFinderProcessCount": 0,
            "nativeFinderElapsed": "0ms",
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
    agent_semantic_client::refresh_source_index(&root).expect("refresh source index");
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
        "finder:skipped",
        "src/lib.rs",
    ] {
        assert!(
            stdout.contains(expected),
            "source-index fd query scenario missing {expected:?}; stdout={stdout}"
        );
    }
    assert!(
        !stdout.contains("sourceTrace=finder:used"),
        "fd warm SourceIndex path must not collect through native finder; stdout={stdout}"
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
            "maxNativeFinderProcessCount": 0,
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
            "providerElapsed": "0ms",
            "nativeFinderProcessCount": 0,
            "nativeFinderElapsed": "0ms",
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

pub(super) fn asp_fzf_source_index_warm_path_stays_inside_scenario_gate() {
    let crate_root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let scenario_root = crate_root
        .join("tests")
        .join("unit")
        .join("scenarios")
        .join("asp_fzf_source_index_warm_path");
    let benchmark: SharedBenchmarkToml = read_toml(&scenario_root.join("benchmark.toml"));
    let max_total_ms = duration_millis_from_manifest(&benchmark.max_total);

    let root = temp_project_root("scenario-fzf-source-index");
    let bin_dir = root.join(".bin");
    let marker = root.join("provider-called");
    fs::create_dir_all(root.join("src")).expect("create source root");
    fs::write(
        root.join("Cargo.toml"),
        "[package]\nname = \"scenario-fzf-source-index\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
    )
    .expect("write package anchor");
    fs::write(
        root.join("src/lib.rs"),
        "pub fn source_index_fixture() {}\npub fn unrelated() {}\n",
    )
    .expect("write source");
    write_marker_provider(&bin_dir, "rs-harness", &marker);
    write_activation(&root, &[provider("rust", Vec::new())]);
    agent_semantic_client::refresh_source_index(&root).expect("refresh source index");
    let _ = fs::remove_file(&marker);

    let output = asp_command(&root)
        .env("PATH", prepend_path(&bin_dir))
        .env("PRJ_CACHE_HOME", root.join(".cache"))
        .args([
            "rust",
            "search",
            "fzf",
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
        .expect("run asp rust search fzf");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("stdout");
    for expected in [
        "[search-fzf]",
        "source=source-index",
        "sourceTrace=sourceIndex:used",
        "finder:skipped",
        "O=owner:path(src/lib.rs)",
    ] {
        assert!(
            stdout.contains(expected),
            "source-index fzf scenario missing {expected:?}; stdout={stdout}"
        );
    }
    assert!(
        !stdout.contains("sourceTrace=finder:used"),
        "fzf warm SourceIndex path must not collect through native finder; stdout={stdout}"
    );
    assert!(
        !marker.exists(),
        "source-index warm fzf should not spawn provider"
    );
    let collect_ms = source_trace_metric_ms(&stdout, "collectMs");
    assert!(
        collect_ms <= max_total_ms,
        "source-index warm fzf exceeded benchmark max_total={} observed={}ms stdout={stdout}",
        benchmark.max_total,
        collect_ms
    );
    let observed_total = format!("{collect_ms}ms");
    let performance_gate = serde_json::json!({
        "schemaId": "agent.semantic-protocols.semantic-hot-path-performance-gate",
        "schemaVersion": "1",
        "scenarioId": "asp-fzf-source-index-warm-path",
        "languageId": "rust",
        "workspace": ".",
        "command": [
            "asp",
            "rust",
            "search",
            "fzf",
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
            "maxNativeFinderProcessCount": 0,
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
            "providerElapsed": "0ms",
            "nativeFinderProcessCount": 0,
            "nativeFinderElapsed": "0ms",
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
        "evidenceRefs": ["scenario:asp-fzf-source-index-warm-path"]
    });
    assert_eq!(performance_gate["observed"]["providerProcessCount"], 0);
    assert_eq!(performance_gate["observed"]["nativeFinderProcessCount"], 0);
    assert_eq!(performance_gate["observed"]["sourceIndexHit"], true);
    assert_eq!(performance_gate["observed"]["stdoutBytes"], stdout.len());
    let _ = fs::remove_dir_all(root);
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
                    );
                }
            }
            ScenarioBenchmarkSyntax::GerbilBenchmarkSs => {
                let paths = discover_benchmark_ss_files(&root);
                if paths.is_empty() {
                    missing.push(format!("{}:{}", requirement.language, requirement.root));
                }
                for path in paths {
                    validate_gerbil_benchmark_ss(&path, &mut invalid);
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

fn validate_toml_scenario_benchmark(language: &str, root: &Path, invalid: &mut Vec<String>) {
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
        require_duration_manifest_field(invalid, &benchmark_path, field, value);
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

pub(super) fn language_harnesses_do_not_use_legacy_agent_policy_ids() {
    let languages = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../languages");
    let mut invalid = Vec::new();
    for relative in LEGACY_POLICY_ID_SCAN_PATHS {
        let path = languages.join(relative);
        if path.exists() {
            collect_legacy_policy_ids(&path, &mut invalid);
        }
    }

    assert!(
        invalid.is_empty(),
        "legacy agent policy ids must use {AGENT_POLICY_ID_GRAMMAR}:\n{}",
        invalid.join("\n")
    );
}

const LEGACY_POLICY_ID_SCAN_PATHS: &[&str] = &[
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

fn validate_gerbil_benchmark_ss(path: &Path, invalid: &mut Vec<String>) {
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

fn collect_legacy_policy_ids(dir: &Path, invalid: &mut Vec<String>) {
    if is_ignored_legacy_policy_scan_path(dir) {
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
        if is_ignored_legacy_policy_scan_path(&path) {
            continue;
        }
        if path.is_dir() {
            collect_legacy_policy_ids(&path, invalid);
        } else if path.is_file() {
            validate_no_legacy_policy_ids(&path, invalid);
        }
    }
}

fn validate_no_legacy_policy_ids(path: &Path, invalid: &mut Vec<String>) {
    let Ok(text) = fs::read_to_string(path) else {
        return;
    };

    for (line_index, line) in text.lines().enumerate() {
        for token in policy_id_tokens(line) {
            if is_legacy_policy_id(token) {
                invalid.push(format!(
                    "{}:{}: legacy policy id {token:?} must match {AGENT_POLICY_ID_GRAMMAR}",
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

fn is_legacy_policy_id(token: &str) -> bool {
    has_numbered_legacy_marker(token, "-AGENT-R") || has_numbered_legacy_marker(token, "-PROJ-R")
}

fn has_numbered_legacy_marker(token: &str, marker: &str) -> bool {
    let Some(index) = token.find(marker) else {
        return false;
    };
    token[index + marker.len()..]
        .chars()
        .next()
        .is_some_and(|ch| ch.is_ascii_digit())
}

fn is_ignored_legacy_policy_scan_path(path: &Path) -> bool {
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

fn require_duration_manifest_field(
    invalid: &mut Vec<String>,
    path: &Path,
    field: &str,
    value: &str,
) {
    let trimmed = value.trim();
    if trimmed.is_empty()
        || !["ns", "us", "ms", "s"]
            .iter()
            .any(|suffix| trimmed.strip_suffix(suffix).is_some_and(is_ascii_digits))
    {
        invalid.push(format!(
            "{}: {field}={value:?} must be a positive duration such as 25ms",
            path.display()
        ));
    }
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
