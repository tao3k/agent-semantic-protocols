use std::{
    fs,
    path::Path,
    time::{Duration, Instant},
};

use agent_semantic_client_core::{ClientMethod, ClientRequest};

use super::runtime_gates::{duration_literal, duration_millis_from_manifest, read_toml};
use super::shared::SharedBenchmarkToml;
use crate::provider_command::support::{asp_command, temp_project_root};

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

#[test]
fn asp_rust_query_structural_selector_materialization_stays_inside_scenario_gate() {
    use agent_semantic_protocol::query_owner_core::run_fast_owner_query_to_string;

    let scenario_started_at = Instant::now();
    let trace_timings = std::env::var_os("ASP_TEST_TIMINGS").is_some();
    let crate_root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let scenario_root = crate_root
        .join("tests")
        .join("unit")
        .join("scenarios")
        .join("asp_rust_query_structural_selector_materialization");
    let benchmark: SharedBenchmarkToml = read_toml(&scenario_root.join("benchmark.toml"));
    assert_eq!(
        benchmark.route_source.as_deref(),
        Some("query-owner-exact-selector")
    );
    assert_eq!(benchmark.max_provider_process_count, Some(0));
    assert_eq!(benchmark.max_stdout_bytes, Some(4096));
    assert_eq!(benchmark.fallback_reason.as_deref(), Some("none"));
    let max_total_ms = duration_millis_from_manifest(&benchmark.max_total);

    let root = temp_project_root("scenario-rust-query-structural-selector-materialization");
    fs::create_dir_all(root.join("src")).expect("create source root");
    fs::write(
        root.join("Cargo.toml"),
        "[package]\nname = \"scenario-rust-query-structural-selector-materialization\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
    )
    .expect("write package anchor");
    fs::write(
        root.join("src/lib.rs"),
        "pub struct AspSessionPolicy;\n\
         impl AspSessionPolicy {\n\
             fn main_asp_command_allowed(&self) -> bool {\n\
                 true\n\
             }\n\
        }\n",
    )
    .expect("write source");
    let locator_root = temp_project_root("scenario-rust-query-structural-selector-shadow");
    fs::create_dir_all(locator_root.join("src")).expect("create shadow source root");
    fs::write(
        locator_root.join("src/lib.rs"),
        "pub fn main_asp_command_allowed() -> bool { false }\n",
    )
    .expect("write shadow source");
    fs::create_dir_all(root.join("src/tree_sitter_query_projection"))
        .expect("create canonical nested source root");
    fs::write(
        root.join("src/tree_sitter_query_projection/core.rs"),
        "pub fn project_native_tree_sitter_query() -> bool { true }\n",
    )
    .expect("write canonical nested source");
    fs::write(
        locator_root.join("src/tree_sitter_query_projection.rs"),
        "pub fn project_native_tree_sitter_query() -> bool { false }\n",
    )
    .expect("write shadow leaf source");
    let setup_elapsed = scenario_started_at.elapsed();

    let matrix: &[(&str, &str, bool, &str)] = &[
        (
            "impl-code",
            "rust://src/lib.rs#item/impl/AspSessionPolicy",
            true,
            "impl AspSessionPolicy {",
        ),
        (
            "missing-item",
            "rust://src/lib.rs#item/function/missing_policy",
            false,
            "state=not-found",
        ),
        (
            "kind-mismatch",
            "rust://src/lib.rs#item/function/AspSessionPolicy",
            false,
            "state=kind-mismatch",
        ),
    ];

    let mut max_case_elapsed = Duration::ZERO;
    let mut output_bytes = 0usize;
    let mut observed_routes = Vec::new();
    for (case, selector, should_succeed, expected) in matrix {
        let case_started_at = Instant::now();
        let args = vec![
            "query".to_string(),
            "--selector".to_string(),
            (*selector).to_string(),
            "--workspace".to_string(),
            ".".to_string(),
            "--code".to_string(),
        ];
        let result =
            run_fast_owner_query_to_string("rust", &args, &root, &root).and_then(|rendered| {
                rendered.ok_or_else(|| format!("fast owner query did not handle {selector}"))
            });
        max_case_elapsed = max_case_elapsed.max(case_started_at.elapsed());
        observed_routes.push(*case);
        assert_eq!(
            result.is_ok(),
            *should_succeed,
            "{case} status mismatch result={result:?}"
        );
        let rendered = match result {
            Ok(rendered) => rendered,
            Err(rendered) => rendered,
        };
        output_bytes += rendered.len();
        assert!(
            rendered.contains(expected),
            "{case} expected {expected:?}; rendered={rendered}"
        );
    }

    let canonical_started_at = Instant::now();
    let canonical_args = vec![
        "query".to_string(),
        "--selector".to_string(),
        "rust://src/tree_sitter_query_projection/core.rs#item/function/project_native_tree_sitter_query"
            .to_string(),
        "--workspace".to_string(),
        ".".to_string(),
        "--code".to_string(),
    ];
    let canonical = run_fast_owner_query_to_string("rust", &canonical_args, &root, &locator_root)
        .and_then(|rendered| {
            rendered.ok_or_else(|| "canonical selector was not handled".to_string())
        })
        .expect("canonical workspace selector must resolve");
    max_case_elapsed = max_case_elapsed.max(canonical_started_at.elapsed());
    output_bytes += canonical.len();
    observed_routes.push("canonical-workspace-owner");
    assert!(canonical.contains("true"), "{canonical}");
    assert!(!canonical.contains("false"), "{canonical}");

    let missing_leaf_started_at = Instant::now();
    let missing_leaf_args = vec![
        "query".to_string(),
        "--selector".to_string(),
        "rust://src/tree_sitter_query_projection.rs#item/function/project_native_tree_sitter_query"
            .to_string(),
        "--workspace".to_string(),
        ".".to_string(),
        "--code".to_string(),
    ];
    let missing_leaf =
        run_fast_owner_query_to_string("rust", &missing_leaf_args, &root, &locator_root)
            .expect_err("missing workspace leaf must return a deterministic not-found error");
    max_case_elapsed = max_case_elapsed.max(missing_leaf_started_at.elapsed());
    observed_routes.push("missing-workspace-leaf");
    assert!(missing_leaf.contains("state=not-found"), "{missing_leaf}");
    assert!(
        missing_leaf.contains("reason=owner-not-found"),
        "{missing_leaf}"
    );
    assert!(
        !missing_leaf.contains("project_native_tree_sitter_query() -> bool { false }"),
        "{missing_leaf}"
    );

    let elapsed_ms = max_case_elapsed.as_millis();
    assert!(
        elapsed_ms <= max_total_ms,
        "structural selector materialization exceeded benchmark max_total={} maxCaseObserved={}ms routes={observed_routes:?}",
        benchmark.max_total,
        elapsed_ms
    );
    assert!(
        output_bytes <= benchmark.max_stdout_bytes.unwrap_or(4096) as usize,
        "scenario output exceeded max_stdout_bytes={:?} observed={output_bytes}",
        benchmark.max_stdout_bytes
    );

    let observed_total = duration_literal(max_case_elapsed);
    let performance_gate = serde_json::json!({
        "schemaId": "agent.semantic-protocols.semantic-hot-path-performance-gate",
        "schemaVersion": "1",
        "scenarioId": "asp-rust-query-structural-selector-materialization",
        "languageId": "rust",
        "workspace": ".",
        "command": matrix.iter().map(|(_, selector, _, _)| {
            vec!["rust", "query", "--selector", selector, "--workspace", ".", "--code"]
        }).collect::<Vec<_>>(),
        "phase": "warm",
        "expected": {
            "targetTotal": benchmark.target_total,
            "maxTotal": benchmark.max_total,
            "regressionBudget": benchmark.regression_budget,
            "maxProviderProcessCount": benchmark.max_provider_process_count,
            "maxStdoutBytes": benchmark.max_stdout_bytes,
            "allowedFirstRoutes": ["query-owner-exact-selector"],
            "forbiddenRoutes": ["provider-process", "search-overlay", "empty-code-projection", "raw-source-read"],
            "fallbackReason": "none"
        },
        "observed": {
            "observedTotal": observed_total,
            "providerProcessCount": 0,
            "executedRoutes": observed_routes,
            "outputBytes": output_bytes,
            "fallbackReason": "none"
        },
        "verdict": "pass",
        "evidenceRefs": ["scenario:asp-rust-query-structural-selector-materialization"]
    });
    if trace_timings {
        eprintln!(
            "structural-selector-materialization setup={}ms max_case={}ms total_before_cleanup={}ms output_bytes={output_bytes}",
            setup_elapsed.as_millis(),
            max_case_elapsed.as_millis(),
            scenario_started_at.elapsed().as_millis()
        );
    }
    assert_eq!(performance_gate["observed"]["providerProcessCount"], 0);
    assert_eq!(performance_gate["observed"]["fallbackReason"], "none");
    let _ = fs::remove_dir_all(root);
    let _ = fs::remove_dir_all(locator_root);
}

#[test]
fn asp_rust_query_native_treesitter_query_uses_workspace_without_positional_root() {
    let root = temp_project_root("scenario-rust-native-treesitter-query");
    fs::create_dir_all(root.join("src")).expect("create source root");
    fs::write(
        root.join("Cargo.toml"),
        "[package]\nname = \"scenario-rust-native-treesitter-query\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
    )
    .expect("write package anchor");
    fs::write(
        root.join("src/lib.rs"),
        "pub fn project_native_tree_sitter_query() -> bool { true }\n",
    )
    .expect("write source");

    let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("workspace root");
    let mut install_command = asp_command(&root);
    install_command.args([
        "install",
        "language",
        "rust",
        "--from-workspace",
        "--project",
    ]);
    install_command.arg(workspace_root);
    let install = install_command
        .output()
        .expect("install current wrapped rust provider");
    assert!(
        install.status.success(),
        "rust provider install failed stderr={}",
        String::from_utf8_lossy(&install.stderr)
    );

    let output = asp_command(&root)
        .args([
            "rust",
            "query",
            "--treesitter-query",
            "((function_item name: (identifier) @name) (#eq? @name \"project_native_tree_sitter_query\"))",
            "--selector",
            "src/lib.rs",
            "--workspace",
            ".",
        ])
        .output()
        .expect("run wrapped native tree-sitter query");
    assert!(
        output.status.success(),
        "native tree-sitter query failed stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("utf8 stdout");
    assert!(
        stdout.contains("project_native_tree_sitter_query"),
        "{stdout}"
    );
    assert!(!stdout.contains("state=not-found"), "{stdout}");
    assert!(!stdout.contains("provider-index-gap"), "{stdout}");
    let _ = fs::remove_dir_all(root);
}

#[test]
fn asp_cli_binary_startup_profile_stays_inside_scenario_gate() {
    let trace_timings = std::env::var_os("ASP_TEST_TIMINGS").is_some();
    let crate_root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let scenario_root = crate_root
        .join("tests")
        .join("unit")
        .join("scenarios")
        .join("asp_cli_binary_startup_profile");
    let benchmark: SharedBenchmarkToml = read_toml(&scenario_root.join("benchmark.toml"));
    assert_eq!(
        benchmark.route_source.as_deref(),
        Some("cli-binary-startup")
    );
    assert_eq!(benchmark.max_provider_process_count, Some(0));
    assert_eq!(benchmark.fallback_reason.as_deref(), Some("none"));
    let max_total_ms = duration_millis_from_manifest(&benchmark.max_total);

    let root = temp_project_root("scenario-cli-binary-startup-profile");
    let warmup = asp_command(&root)
        .arg("--help")
        .output()
        .expect("warm asp cli startup profile");
    assert!(
        warmup.status.success(),
        "asp --help warmup failed stderr={}",
        String::from_utf8_lossy(&warmup.stderr)
    );

    let mut best_elapsed = None;
    let mut best_output_bytes = None;
    for _ in 0..3 {
        let scenario_started_at = Instant::now();
        let output = asp_command(&root)
            .arg("--help")
            .output()
            .expect("run asp cli startup profile");
        let elapsed = scenario_started_at.elapsed();
        assert!(
            output.status.success(),
            "asp --help failed stderr={}",
            String::from_utf8_lossy(&output.stderr)
        );
        let output_bytes = output.stdout.len() + output.stderr.len();
        if best_elapsed.is_none_or(|best| elapsed < best) {
            best_elapsed = Some(elapsed);
            best_output_bytes = Some(output_bytes);
        }
    }
    let elapsed = best_elapsed.expect("startup profile sample");
    assert!(
        elapsed.as_millis() <= max_total_ms,
        "cli binary startup exceeded benchmark max_total={} observed={}",
        benchmark.max_total,
        duration_literal(elapsed)
    );
    let output_bytes = best_output_bytes.expect("startup profile output bytes");
    assert!(
        output_bytes <= benchmark.max_stdout_bytes.unwrap_or(65536) as usize,
        "scenario output exceeded max_stdout_bytes={:?} observed={output_bytes}",
        benchmark.max_stdout_bytes
    );
    if trace_timings {
        eprintln!(
            "cli-binary-startup elapsed={} output_bytes={output_bytes}",
            duration_literal(elapsed)
        );
    }
    let _ = fs::remove_dir_all(root);
}
