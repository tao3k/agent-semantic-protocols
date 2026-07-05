use std::{fs, path::Path, time::Instant};

use serde::Deserialize;

use super::contracts::{
    assert_runtime_owner_items_receipt_benchmark_contract,
    assert_runtime_timeout_policy_benchmark_contract,
};
use super::shared::SharedBenchmarkToml;
use crate::provider_command::support::temp_project_root;

pub(crate) fn scenario_benchmark_duration_contract_rejects_zero_budget() {
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

pub(crate) fn duration_millis_from_manifest(value: &str) -> u128 {
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

pub(crate) fn duration_literal(duration: std::time::Duration) -> String {
    let micros = duration.as_micros();
    if micros == 0 {
        format!("{}ns", duration.as_nanos())
    } else if micros < 1_000 {
        format!("{micros}us")
    } else {
        format!("{}ms", duration.as_millis())
    }
}

pub(crate) fn read_toml<T>(path: &Path) -> T
where
    T: for<'de> Deserialize<'de>,
{
    let text = fs::read_to_string(path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display()));
    toml::from_str(&text)
        .unwrap_or_else(|error| panic!("failed to parse {}: {error}", path.display()))
}

pub(super) fn require_positive_duration_manifest_field(
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

pub(super) fn require_observed_timing_manifest_field(
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

pub(super) fn is_ascii_digits(value: &str) -> bool {
    !value.is_empty() && value.chars().all(|ch| ch.is_ascii_digit())
}

fn parse_u128(value: &str) -> Option<u128> {
    value.parse::<u128>().ok()
}

pub(crate) fn asp_runtime_owner_items_receipt_cold_functional_path_stays_inside_scenario_gate() {
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

pub(crate) fn asp_runtime_timeout_policy_cold_functional_path_stays_inside_scenario_gate() {
    let crate_root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let scenario_root = crate_root
        .join("tests")
        .join("unit")
        .join("scenarios")
        .join("asp_runtime_timeout_policy_cold_functional_path");
    let benchmark: SharedBenchmarkToml = read_toml(&scenario_root.join("benchmark.toml"));
    assert_runtime_timeout_policy_benchmark_contract(&benchmark);
    let max_total_ms = duration_millis_from_manifest(&benchmark.max_total);

    let policy =
        agent_semantic_runtime::RuntimeOperationTimeoutPolicy::new("owner-items-provider", 10, 25);
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
