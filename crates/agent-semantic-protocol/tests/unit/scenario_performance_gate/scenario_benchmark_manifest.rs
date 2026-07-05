use std::{
    collections::BTreeSet,
    env, fs,
    path::{Path, PathBuf},
};

use super::runtime_gates::{
    read_toml, require_observed_timing_manifest_field, require_positive_duration_manifest_field,
};
use super::scenario_performance_gate_impl::{is_non_scenario_dir, read_dir_sorted};
use super::scenario_policy_scan::{
    benchmark_has_hot_path_metadata, canonical_benchmark_language, discover_scenario_policy_ids,
    is_agent_policy_id, require_non_empty_manifest_field, require_supported_language_harness,
    validate_language_harness_json_boundary,
};
use super::shared::{
    AGENT_POLICY_ID_GRAMMAR, COLD_FIRST_SEARCH_LANGUAGE_IDS,
    LANGUAGE_SCENARIO_BENCHMARK_REQUIREMENTS, REQUIRED_PERFORMANCE_SENSITIVE_SUBCOMMAND_POLICY_IDS,
    SharedBenchmarkToml, SharedScenarioToml,
};

pub(super) fn discover_toml_scenario_benchmark_roots(root: &Path) -> Vec<PathBuf> {
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

pub(super) fn discover_benchmark_ss_files(root: &Path) -> Vec<PathBuf> {
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

pub(super) fn validate_toml_scenario_benchmark(
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

pub(super) fn asp_unit_scenarios_cover_perf_sensitive_subcommands() {
    let scenario_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("unit")
        .join("scenarios");
    let policy_ids = discover_scenario_policy_ids(&scenario_root);
    let missing = REQUIRED_PERFORMANCE_SENSITIVE_SUBCOMMAND_POLICY_IDS
        .iter()
        .copied()
        .filter(|policy_id| !policy_ids.contains(*policy_id))
        .collect::<Vec<_>>();

    assert!(
        missing.is_empty(),
        "ASP unit scenarios must cover performance-sensitive subcommands; missing={missing:?}; observed={policy_ids:?}"
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
