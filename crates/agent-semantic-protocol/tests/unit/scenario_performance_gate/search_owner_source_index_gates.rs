use std::{path::Path, path::PathBuf, time::Instant};

use super::contracts::assert_search_owner_source_index_trace_benchmark_contract;
use super::runtime_gates::{duration_literal, duration_millis_from_manifest, read_toml};
use super::shared::SharedBenchmarkToml;

pub(crate) fn asp_search_owner_source_index_trace_missing_db_cold_functional_path_stays_inside_scenario_gate()
 {
    let crate_root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let scenario_root = crate_root
        .join("tests")
        .join("unit")
        .join("scenarios")
        .join("asp_search_owner_source_index_trace_missing_db_cold_functional_path");
    let benchmark: SharedBenchmarkToml = read_toml(&scenario_root.join("benchmark.toml"));
    assert_search_owner_source_index_trace_benchmark_contract(&benchmark, "sourceIndex:missing-db");
    let max_total_ms = duration_millis_from_manifest(&benchmark.max_total);

    let started_at = Instant::now();
    let lookup = agent_semantic_search::QueryWrapperSearchSourceIndexTrace {
        lookup: agent_semantic_search::QueryWrapperSourceIndexLookup::new(
            PathBuf::from("live/client/client.turso"),
            "missing-db",
            Vec::new(),
        ),
        candidate_count: 0,
        elapsed: std::time::Duration::from_millis(3),
    };
    let projection = agent_semantic_search::query_wrapper_source_index_trace_projection(&lookup);
    let elapsed = started_at.elapsed();
    let elapsed_ms = elapsed.as_millis();

    assert_eq!(projection.source, "sourceIndex");
    assert_eq!(projection.status, "missing-db");
    assert_eq!(projection.skipped_count, 1);
    assert_eq!(
        projection.fields["nextCommand"],
        serde_json::json!("asp cache source-index refresh")
    );
    assert!(elapsed_ms <= max_total_ms);

    let observed_total = duration_literal(elapsed);
    let performance_gate = serde_json::json!({
        "schemaId": "agent.semantic-protocols.semantic-hot-path-performance-gate",
        "schemaVersion": "1",
        "scenarioId": "asp-search-owner-source-index-trace-missing-db-cold-functional-path",
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
            "allowedFirstRoutes": ["search-owner-source-index-trace"],
            "forbiddenRoutes": ["command-local-source-index-status", "native-finder", "provider-process"],
            "requireRefreshHintOnMiss": true
        },
        "observed": {
            "observedTotal": observed_total,
            "providerProcessCount": 0,
            "providerElapsed": "0us",
            "nativeFinderProcessCount": 0,
            "nativeFinderElapsed": "0us",
            "hitStatus": projection.status,
            "missingDbNextCommand": projection.fields["nextCommand"],
            "firstRoute": "search-owner-source-index-trace",
            "executedRoutes": ["search-owner-source-index-trace"],
            "stdoutBytes": 0,
            "fallbackReason": "sourceIndex:missing-db"
        },
        "verdict": "pass",
        "evidenceRefs": ["scenario:asp-search-owner-source-index-trace-missing-db-cold-functional-path"]
    });
    assert_eq!(performance_gate["observed"]["providerProcessCount"], 0);
    assert_eq!(performance_gate["observed"]["nativeFinderProcessCount"], 0);
    assert_eq!(performance_gate["observed"]["hitStatus"], "missing-db");
}

pub(crate) fn asp_search_owner_source_index_trace_hit_cold_functional_path_stays_inside_scenario_gate()
 {
    let crate_root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let scenario_root = crate_root
        .join("tests")
        .join("unit")
        .join("scenarios")
        .join("asp_search_owner_source_index_trace_hit_cold_functional_path");
    let benchmark: SharedBenchmarkToml = read_toml(&scenario_root.join("benchmark.toml"));
    assert_search_owner_source_index_trace_benchmark_contract(&benchmark, "none");
    let max_total_ms = duration_millis_from_manifest(&benchmark.max_total);

    let started_at = Instant::now();
    let lookup = agent_semantic_search::QueryWrapperSearchSourceIndexTrace {
        lookup: agent_semantic_search::QueryWrapperSourceIndexLookup::new(
            PathBuf::from("live/client/client.turso"),
            "hit",
            Vec::new(),
        ),
        candidate_count: 2,
        elapsed: std::time::Duration::from_micros(750),
    };
    let projection = agent_semantic_search::query_wrapper_source_index_trace_projection(&lookup);
    let elapsed = started_at.elapsed();
    let elapsed_ms = elapsed.as_millis();

    assert_eq!(projection.source, "sourceIndex");
    assert_eq!(projection.status, "used");
    assert_eq!(projection.candidate_count, 2);
    assert_eq!(projection.skipped_count, 0);
    assert!(!projection.fields.contains_key("nextCommand"));
    assert!(elapsed_ms <= max_total_ms);

    let observed_total = duration_literal(elapsed);
    let performance_gate = serde_json::json!({
        "schemaId": "agent.semantic-protocols.semantic-hot-path-performance-gate",
        "schemaVersion": "1",
        "scenarioId": "asp-search-owner-source-index-trace-hit-cold-functional-path",
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
            "allowedFirstRoutes": ["search-owner-source-index-trace"],
            "forbiddenRoutes": ["command-local-source-index-status", "native-finder", "provider-process"],
            "requireRefreshHintOnMiss": false
        },
        "observed": {
            "observedTotal": observed_total,
            "providerProcessCount": 0,
            "providerElapsed": "0us",
            "nativeFinderProcessCount": 0,
            "nativeFinderElapsed": "0us",
            "hitStatus": projection.status,
            "firstRoute": "search-owner-source-index-trace",
            "executedRoutes": ["search-owner-source-index-trace"],
            "stdoutBytes": 0,
            "fallbackReason": "none"
        },
        "verdict": "pass",
        "evidenceRefs": ["scenario:asp-search-owner-source-index-trace-hit-cold-functional-path"]
    });
    assert_eq!(performance_gate["observed"]["providerProcessCount"], 0);
    assert_eq!(performance_gate["observed"]["nativeFinderProcessCount"], 0);
    assert_eq!(performance_gate["observed"]["hitStatus"], "used");
}

pub(crate) fn source_trace_metric_ms(stdout: &str, metric: &str) -> u128 {
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
