use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use super::agent_session_pressure;
use super::large_library::LargeLibraryElapsedGate;
use super::runtime_gates::{duration_millis_from_manifest, read_toml};
use super::scenario_policy_scan::assert_observed_timing_inside_budget;
use super::shared::SharedBenchmarkToml;
use super::turso_pressure;
use serde_json::Value;

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

pub(super) fn asp_codex_rollout_session_index_algorithm_pressure_stays_inside_scenario_gate() {
    agent_session_pressure::asp_codex_rollout_session_index_algorithm_pressure_stays_inside_scenario_gate();

    let crate_root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let scenario_root = crate_root
        .join("tests")
        .join("unit")
        .join("scenarios")
        .join("asp_codex_rollout_session_index_algorithm_pressure");
    let benchmark: SharedBenchmarkToml = read_toml(&scenario_root.join("benchmark.toml"));
    assert_eq!(benchmark.harness, "libtest");
    assert_eq!(
        benchmark.test.as_deref(),
        Some("asp_codex_rollout_session_index_algorithm_pressure_stays_inside_scenario_gate")
    );
    assert_eq!(benchmark.route_source.as_deref(), Some("codex-rollout"));
    assert_eq!(benchmark.max_provider_process_count, Some(0));
    assert_eq!(benchmark.max_stdout_bytes, Some(16384));
    assert_eq!(benchmark.fallback_reason.as_deref(), Some("none"));
    assert!(
        duration_millis_from_manifest(&benchmark.max_total) <= 10,
        "Codex rollout session index must stay below the 10ms algorithm budget: max_total={}",
        benchmark.max_total
    );
    assert!(
        duration_millis_from_manifest(&benchmark.observed_total) <= 10,
        "Codex rollout session index observed_total must stay below the algorithm budget: observed_total={}",
        benchmark.observed_total
    );
    assert_observed_timing_inside_budget(
        &benchmark,
        "codex_rollout_session_index",
        10,
        "Codex rollout session index",
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

pub(super) fn read_json(path: &Path) -> Value {
    let text = fs::read_to_string(path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display()));
    serde_json::from_str(&text)
        .unwrap_or_else(|error| panic!("failed to parse {}: {error}", path.display()))
}

pub(super) fn read_dir_sorted(path: &Path) -> Vec<PathBuf> {
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

pub(super) fn string_field(value: &Value, field: &str) -> Option<String> {
    value
        .get(field)
        .and_then(Value::as_str)
        .map(ToString::to_string)
}

pub(super) fn is_non_scenario_dir(path: &Path) -> bool {
    matches!(
        path.file_name().and_then(|name| name.to_str()),
        Some("fixtures" | "receipts" | "root")
    )
}

pub(super) fn render_gates(repo_root: &Path, gates: &[LargeLibraryElapsedGate]) -> String {
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
