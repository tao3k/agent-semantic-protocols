use std::env;
use std::path::Path;
use std::time::Instant;

use agent_semantic_protocol::{
    SelectorSeededSearchPipeRequest, render_selector_seeded_search_pipe,
};

use super::runtime_gates::{duration_literal, duration_millis_from_manifest, read_toml};
use super::shared::{SearchFrameReferenceBenchmarkToml, SharedBenchmarkToml, SharedScenarioToml};

pub(in super::super) fn asp_selector_seeded_search_pipe_frontier_stays_inside_scenario_gate() {
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
    let stdout = render_selector_seeded_search_pipe(SelectorSeededSearchPipeRequest {
        language_id: "rust",
        selector,
        query,
        workspace: ".",
    });
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

pub(in super::super) fn search_frame_codebase_memory_mcp_reference_benchmark_records_round_delta() {
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

pub(in super::super) fn exact_selector_inventory_routes_to_main_direct_fetch() {
    let crate_root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let scenario_root = crate_root
        .join("tests")
        .join("unit")
        .join("scenarios")
        .join("exact_selector_inventory_direct_fetch");
    let scenario: SharedScenarioToml = read_toml(&scenario_root.join("scenario.toml"));
    let benchmark: SharedBenchmarkToml = read_toml(&scenario_root.join("benchmark.toml"));

    assert_eq!(scenario.id, "exact-selector-inventory-direct-fetch");
    assert!(
        scenario
            .policy_ids
            .iter()
            .any(|policy_id| policy_id == "RUST-AGENT-ASP-SEARCH-FRAME-DIRECT-INVENTORY-FETCH-001"),
        "scenario must carry the Direct Inventory/Fetch policy id"
    );
    assert_eq!(benchmark.harness, "libtest");
    assert_eq!(
        benchmark.test.as_deref(),
        Some("exact_selector_inventory_routes_to_main_direct_fetch")
    );
    assert_eq!(
        benchmark.route_source.as_deref(),
        Some("direct-inventory-fetch")
    );
    assert_eq!(benchmark.max_provider_process_count, Some(0));
    assert_eq!(benchmark.max_stdout_bytes, Some(4096));
    assert_eq!(benchmark.fallback_reason.as_deref(), Some("none"));

    let direct_lane_contract = serde_json::json!({
        "schemaId": "agent.semantic-protocols.search-frame-direct-inventory-fetch-gate",
        "schemaVersion": "1",
        "scenarioId": "exact-selector-inventory-direct-fetch",
        "commandShape": [
            "asp",
            "md|org",
            "query",
            "--selector",
            "<exact-selector>",
            "--workspace",
            ".",
            "--view",
            "metadata"
        ],
        "expected": {
            "metadataRoute": "direct-inventory",
            "finalEvidenceRoute": "direct-fetch",
            "maxSubagentDispatchCount": 0,
            "maxDuplicateReadCount": 0,
            "allowSearchPrime": false,
            "allowSearchPipe": false,
            "allowRawRead": false,
            "allowMainContentRead": true,
            "allowMainCodeRead": true
        },
        "observed": {
            "metadataRoute": "direct-inventory",
            "nextEvidenceRoute": "direct-fetch",
            "subagentDispatchCount": 0,
            "duplicateReadCount": 0,
            "searchPrimeCount": 0,
            "searchPipeCount": 0,
            "rawReadCount": 0,
            "subagentContentReadCount": 0
        },
        "verdict": "pass",
        "evidenceRefs": ["scenario:exact-selector-inventory-direct-fetch"]
    });

    assert_eq!(
        direct_lane_contract["expected"]["maxSubagentDispatchCount"],
        0
    );
    assert_eq!(direct_lane_contract["observed"]["subagentDispatchCount"], 0);
    assert_eq!(direct_lane_contract["observed"]["duplicateReadCount"], 0);
    assert_eq!(
        direct_lane_contract["observed"]["nextEvidenceRoute"],
        "direct-fetch"
    );
    assert_eq!(
        direct_lane_contract["observed"]["subagentContentReadCount"],
        0
    );
}

fn assert_selector_seed_benchmark_contract(benchmark: &SharedBenchmarkToml) {
    assert_eq!(benchmark.route_source.as_deref(), Some("selector-seed"));
    assert_eq!(benchmark.max_provider_process_count, Some(0));
    assert_eq!(benchmark.max_stdout_bytes, Some(4096));
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
#[test]
fn file_range_selector_seeded_search_pipe_does_not_materialize_query_code() {
    let selector = "src/lib.rs:1:5";
    let query = "runtime_profile_invocation RuntimeProfiles provider_command_prefix";
    let stdout = render_selector_seeded_search_pipe(SelectorSeededSearchPipeRequest {
        language_id: "rust",
        selector,
        query,
        workspace: ".",
    });

    assert!(stdout.contains("source=selector"), "{stdout}");
    assert!(stdout.contains("selectorSeed=src/lib.rs:1:5"), "{stdout}");
    assert!(!stdout.contains("query-code"), "{stdout}");
    assert!(
        !stdout.contains("nextCommand=asp rust query --selector src/lib.rs:1:5"),
        "{stdout}"
    );
    assert!(
        stdout.contains("actionFrontier=A1.owner-items,A2.rg-query"),
        "{stdout}"
    );
    assert!(
        stdout.contains("recommendedNext=A1.owner-items"),
        "{stdout}"
    );
}

#[test]
fn symbol_selector_seeded_search_pipe_does_not_materialize_query_code() {
    let selector = "rust://src/lib.rs#item/symbol/vec";
    let query = "Vec collection fields";
    let stdout = render_selector_seeded_search_pipe(SelectorSeededSearchPipeRequest {
        language_id: "rust",
        selector,
        query,
        workspace: ".",
    });

    assert!(stdout.contains("source=selector"), "{stdout}");
    assert!(
        stdout.contains("selectorSeed=rust://src/lib.rs#item/symbol/vec"),
        "{stdout}"
    );
    assert!(!stdout.contains("query-code"), "{stdout}");
    assert!(
        stdout.contains("actionFrontier=A1.owner-items,A2.rg-query"),
        "{stdout}"
    );
    assert!(
        stdout.contains("recommendedNext=A1.owner-items"),
        "{stdout}"
    );
}
