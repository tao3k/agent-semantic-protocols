use std::{
    fs,
    path::Path,
    time::{Duration, Instant},
};

use agent_semantic_client_core::{ClientMethod, ClientRequest};

use super::runtime_gates::{duration_literal, duration_millis_from_manifest, read_toml};
use super::shared::SharedBenchmarkToml;
use crate::provider_command::support::temp_project_root;

pub(crate) fn asp_query_selector_directory_code_preflight_cold_functional_path_stays_inside_scenario_gate()
 {
    let crate_root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let scenario_root = crate_root
        .join("tests")
        .join("unit")
        .join("scenarios")
        .join("asp_query_selector_directory_code_preflight_cold_functional_path");
    let benchmark: SharedBenchmarkToml = read_toml(&scenario_root.join("benchmark.toml"));
    assert_query_selector_directory_preflight_benchmark_contract(&benchmark);
    let max_total_ms = duration_millis_from_manifest(&benchmark.max_total);

    let root = temp_project_root("scenario-query-selector-directory-code-preflight");
    fs::create_dir_all(root.join("src")).expect("create source root");
    fs::write(
        root.join("Cargo.toml"),
        "[package]\nname = \"scenario-query-selector-directory-code-preflight\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
    )
    .expect("write package anchor");
    fs::write(
        root.join("src/lib.rs"),
        "pub fn direct_source_read_boundary() {}\npub fn fallback_reason_boundary() {}\n",
    )
    .expect("write source");

    let matrix: &[(&str, &[&str], &str)] = &[
        (
            "directory-selector-query-code",
            &[
                "rust",
                "query",
                "--selector",
                "src",
                "--query",
                "from-hook fallback-reason direct-source-read",
                "--workspace",
                ".",
                "--code",
            ],
            "query --selector with --code requires an exact file",
        ),
        (
            "directory-selector-from-hook-code",
            &[
                "rust",
                "query",
                "--from-hook",
                "direct-source-read",
                "--fallback-reason",
                "directory-selector-preflight",
                "--selector",
                "src",
                "--workspace",
                ".",
                "--code",
            ],
            "query --selector with --code requires an exact file",
        ),
        (
            "directory-selector-treesitter-code",
            &[
                "rust",
                "query",
                "--treesitter-query",
                "(function_item name: (identifier) @function.name)",
                "--selector",
                "src",
                "--workspace",
                ".",
                "--code",
            ],
            "query --selector with --code requires an exact file",
        ),
        (
            "directory-selector-bare-code",
            &[
                "rust",
                "query",
                "--selector",
                "src",
                "--workspace",
                ".",
                "--code",
            ],
            "query --selector with --code requires an exact file",
        ),
    ];

    let mut max_case_elapsed = Duration::ZERO;
    let mut observed_routes = Vec::new();
    for (case, args, expected_reason) in matrix {
        let case_started_at = Instant::now();
        let request = ClientRequest::new(ClientMethod::Query, root.display().to_string())
            .with_language("rust")
            .with_forwarded_args(args[2..].iter().map(|arg| (*arg).to_string()).collect());
        let error = match agent_semantic_client::validate_client_syntax_query_request(&request) {
            Ok(()) => panic!("{case} must fail before provider execution"),
            Err(error) => error,
        };
        let case_elapsed = case_started_at.elapsed();
        max_case_elapsed = max_case_elapsed.max(case_elapsed);
        assert!(
            error.contains(expected_reason),
            "{case} must fail with expected preflight guard {expected_reason:?}; error={error}"
        );
        if expected_reason.contains("exact file") {
            assert!(
                error.contains("`src` is a directory"),
                "{case} must identify directory selector; error={error}"
            );
        }
        observed_routes.push(*case);
    }
    let max_case_elapsed_ms = max_case_elapsed.as_millis();
    let max_stdout_bytes = benchmark.max_stdout_bytes.unwrap_or(2048) as usize;
    let output_bytes = 0usize;
    assert!(output_bytes <= max_stdout_bytes);
    assert!(
        max_case_elapsed_ms <= max_total_ms,
        "directory selector preflight command exceeded benchmark max_total={} observed={}ms routes={observed_routes:?}",
        benchmark.max_total,
        max_case_elapsed_ms
    );

    let observed_total = duration_literal(max_case_elapsed);
    let performance_gate = serde_json::json!({
        "schemaId": "agent.semantic-protocols.semantic-hot-path-performance-gate",
        "schemaVersion": "1",
        "scenarioId": "asp-query-selector-directory-code-preflight-cold-functional-path",
        "languageId": "rust",
        "workspace": ".",
            "command": matrix.iter().map(|(_, args, _)| args.to_vec()).collect::<Vec<_>>(),
        "phase": "cold",
        "expected": {
            "targetTotal": benchmark.target_total,
            "maxTotal": benchmark.max_total,
            "regressionBudget": benchmark.regression_budget,
            "maxProviderProcessCount": benchmark.max_provider_process_count,
            "maxSearchOverlayProcessCount": 0,
            "maxStdoutBytes": benchmark.max_stdout_bytes,
            "requirePreflightDeny": true,
            "allowedFirstRoutes": ["query-preflight"],
            "forbiddenRoutes": ["provider-process", "search-overlay", "empty-code-projection"],
            "fallbackReason": "none"
        },
        "observed": {
            "observedTotal": observed_total,
            "providerProcessCount": 0,
            "providerElapsed": "0us",
            "nativeFinderProcessCount": 0,
            "nativeFinderElapsed": "0us",
            "firstRoute": "query-preflight",
            "executedRoutes": observed_routes,
            "stdoutBytes": output_bytes,
            "stderrBytes": output_bytes,
            "fallbackReason": "none"
        },
        "verdict": "pass",
        "evidenceRefs": ["scenario:asp-query-selector-directory-code-preflight-cold-functional-path"]
    });
    assert_eq!(performance_gate["observed"]["providerProcessCount"], 0);
    assert_eq!(performance_gate["observed"]["nativeFinderProcessCount"], 0);
    assert_eq!(performance_gate["observed"]["fallbackReason"], "none");
    let _ = fs::remove_dir_all(root);
}

fn assert_query_selector_directory_preflight_benchmark_contract(benchmark: &SharedBenchmarkToml) {
    assert_eq!(benchmark.route_source.as_deref(), Some("query-preflight"));
    assert_eq!(benchmark.max_provider_process_count, Some(0));
    assert_eq!(benchmark.max_stdout_bytes, Some(4096));
    assert_eq!(benchmark.fallback_reason.as_deref(), Some("none"));
}
