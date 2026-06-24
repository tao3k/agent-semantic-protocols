use std::time::{Duration, Instant};

use crate::provider_command::support::{
    asp_command, make_executable, prepend_path, provider, temp_project_root, write_activation,
    write_echo_provider,
};

const ASP_FACADE_PERFORMANCE_GATE: Duration = Duration::from_secs(2);
// Process wall time includes binary startup and test-host scheduling. Search SLA is enforced by
// sourceTrace elapsedMs below; this wall gate only catches hangs.
const ASP_QUERY_WRAPPER_WALL_SANITY_GATE: Duration = Duration::from_secs(3);
// SourceTrace includes provider process startup under cargo-test parallelism; keep these gates
// tight enough to catch hangs while functional tests assert candidate/input bounds separately.
const ASP_SEARCH_PHASE_PERFORMANCE_GATE_MS: u64 = 250;
const ASP_RENDER_PHASE_PERFORMANCE_GATE_MS: u64 = 100;
const ASP_BLOCKED_QUERY_PHASE_PERFORMANCE_GATE_MS: u64 = 10;
const ASP_PROVIDER_FACTS_PHASE_PERFORMANCE_GATE_MS: u64 = 2_000;
const JULIA_FACADE_PERFORMANCE_GATE: Duration = Duration::from_secs(3);

#[derive(Clone, Copy)]
struct FacadePerformanceProvider {
    language: &'static str,
    binary: &'static str,
    label: &'static str,
    owner: &'static str,
    query: &'static str,
}

#[test]
fn language_facade_regular_commands_finish_inside_performance_gate() {
    let root = temp_project_root("language-facade-performance-gate");
    let bin_dir = root.join(".bin");
    let cache_home = root.join(".cache");
    let providers = [
        FacadePerformanceProvider {
            language: "rust",
            binary: "rs-harness",
            label: "rs",
            owner: "src/lib.rs",
            query: "RustGate",
        },
        FacadePerformanceProvider {
            language: "typescript",
            binary: "ts-harness",
            label: "ts",
            owner: "src/index.ts",
            query: "typescriptGate",
        },
        FacadePerformanceProvider {
            language: "python",
            binary: "py-harness",
            label: "py",
            owner: "src/main.py",
            query: "python_gate",
        },
        FacadePerformanceProvider {
            language: "julia",
            binary: "asp-julia-harness",
            label: "julia",
            owner: "src/main.jl",
            query: "julia_gate",
        },
        FacadePerformanceProvider {
            language: "gerbil-scheme",
            binary: "gslph",
            label: "gerbil",
            owner: "src/main.ss",
            query: "gerbil-gate",
        },
    ];
    std::fs::create_dir_all(&bin_dir).expect("create bin dir");
    write_regular_search_fixtures(&root);
    for provider in providers {
        write_echo_provider(&bin_dir, provider.binary, provider.label);
    }
    write_activation(
        &root,
        &providers
            .iter()
            .map(|provider_config| {
                provider(
                    provider_config.language,
                    vec![bin_dir.join(provider_config.binary).display().to_string()],
                )
            })
            .collect::<Vec<_>>(),
    );

    for provider in providers {
        let command_suite = [
            vec![
                provider.language,
                "query",
                provider.owner,
                "--query",
                provider.query,
                ".",
            ],
            vec![provider.language, "search", "prime", "--view", "seeds", "."],
            vec![
                provider.language,
                "search",
                "prime",
                "--workspace",
                ".",
                "--view",
                "seeds",
            ],
            vec![
                provider.language,
                "search",
                "pipe",
                provider.query,
                "--workspace",
                ".",
                "--view",
                "seeds",
            ],
            vec![
                provider.language,
                "search",
                "pipe",
                provider.query,
                "--view",
                "graph-turbo-request",
                ".",
            ],
        ];
        for args in command_suite {
            let warmup = asp_command(&root)
                .env("PATH", prepend_path(&bin_dir))
                .env("PRJ_CACHE_HOME", &cache_home)
                .args(&args)
                .output()
                .unwrap_or_else(|error| panic!("warm asp {args:?}: {error}"));
            assert!(
                warmup.status.success(),
                "warm args={args:?} stderr={}",
                String::from_utf8_lossy(&warmup.stderr)
            );

            let started_at = Instant::now();
            let output = asp_command(&root)
                .env("PATH", prepend_path(&bin_dir))
                .env("PRJ_CACHE_HOME", &cache_home)
                .args(&args)
                .output()
                .unwrap_or_else(|error| panic!("run asp {args:?}: {error}"));
            let elapsed = started_at.elapsed();
            assert!(
                output.status.success(),
                "args={args:?} stderr={}",
                String::from_utf8_lossy(&output.stderr)
            );
            let gate = performance_gate_for_language(provider.language);
            assert!(
                elapsed < gate,
                "asp {args:?} exceeded {gate:?}; elapsed={elapsed:?}; stdout={}; stderr={}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
            let stdout = String::from_utf8(output.stdout).expect("stdout");
            assert_regular_command_output(&args, &stdout, provider.label);
        }
    }
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn query_wrapper_commands_finish_inside_search_phase_gate() {
    let root = temp_project_root("query-wrapper-performance-gate");
    write_regular_search_fixtures(&root);

    let command_suite = [
        [
            "fd",
            "-query",
            "RustGate|typescriptGate|python_gate|julia_gate|gerbil-gate",
            ".",
        ],
        [
            "rg",
            "-query",
            "RustGate typescriptGate python_gate julia_gate gerbil-gate",
            ".",
        ],
    ];
    for args in command_suite {
        let started_at = Instant::now();
        let output = asp_command(&root)
            .args(args)
            .output()
            .unwrap_or_else(|error| panic!("run asp {args:?}: {error}"));
        let elapsed = started_at.elapsed();
        assert!(
            output.status.success(),
            "args={args:?} stderr={}",
            String::from_utf8_lossy(&output.stderr)
        );
        let stdout = String::from_utf8(output.stdout).expect("stdout");
        assert!(
            elapsed < ASP_QUERY_WRAPPER_WALL_SANITY_GATE,
            "asp {args:?} exceeded wrapper wall sanity gate {ASP_QUERY_WRAPPER_WALL_SANITY_GATE:?}; elapsed={elapsed:?}; stdout={stdout}; stderr={}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(
            stdout.contains("sourceTrace=") && stdout.contains("elapsedMs="),
            "args={args:?} stdout={stdout}"
        );
        assert_trace_elapsed_under_gate(&args, &stdout);
    }
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn broad_rg_query_requires_native_filter_or_narrow_scope() {
    let root = temp_project_root("broad-rg-query-performance-gate");
    std::fs::create_dir_all(root.join("src")).expect("create src");
    for file_index in 0..160 {
        let mut text = String::new();
        text.push_str(&format!(
            ";; typed-combinator-style source quality file {file_index}\n"
        ));
        for line_index in 0..120 {
            text.push_str(&format!(
                ";; line {line_index} routes source comments to engineering quality helpers\n"
            ));
        }
        std::fs::write(root.join(format!("src/broad_{file_index}.ss")), text)
            .expect("write broad fixture");
    }
    std::fs::write(root.join("gerbil.pkg"), "(package: broad-rg-gate)\n")
        .expect("write gerbil.pkg");

    let broad_output = asp_command(&root)
        .args([
            "rg",
            "-query",
            "typed-combinator-style R013 self apply old comment style migrate source helpers to",
            ".",
        ])
        .output()
        .expect("run broad asp rg without filter");

    assert!(
        broad_output.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&broad_output.stderr)
    );
    let stdout = String::from_utf8(broad_output.stdout).expect("stdout");
    assert!(
        stdout.contains("noOutput reason=query-too-broad") && stdout.contains("refineHint="),
        "broad query should be blocked before native rg without filters; stdout={stdout}"
    );
    assert_trace_elapsed_under_gate_ms(
        &["rg", "-query", "broad"],
        &stdout,
        ASP_BLOCKED_QUERY_PHASE_PERFORMANCE_GATE_MS,
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn broad_fd_query_requires_path_or_symbol_terms() {
    let root = temp_project_root("broad-fd-query-budget-gate");
    std::fs::create_dir_all(root.join("src")).expect("create src");
    std::fs::write(
        root.join("src/search_query_budget.rs"),
        "pub struct BudgetGate;\n",
    )
    .expect("write fixture");

    let output = asp_command(&root)
        .args([
            "fd",
            "-query",
            "wrapper backend interface budget gate broad generic input",
            ".",
        ])
        .output()
        .expect("run broad asp fd");

    assert!(
        output.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("stdout");
    assert!(
        stdout.contains("noOutput reason=query-too-broad")
            && stdout.contains("nextCommand=asp fd -query 'path-or-symbol|error-code'"),
        "broad fd query should be blocked with a granular query example; stdout={stdout}"
    );
    assert_trace_elapsed_under_gate_ms(
        &["fd", "-query", "broad"],
        &stdout,
        ASP_BLOCKED_QUERY_PHASE_PERFORMANCE_GATE_MS,
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn provider_facts_receive_bounded_candidate_input() {
    let root = temp_project_root("provider-facts-candidate-budget-gate");
    let bin_dir = root.join(".bin");
    let cache_home = root.join(".cache");
    std::fs::create_dir_all(root.join("src")).expect("create src");
    std::fs::create_dir_all(&bin_dir).expect("create bin dir");
    for index in 0..80 {
        std::fs::write(
            root.join(format!("src/queue_candidate_{index}.rs")),
            format!("pub fn queue_candidate_{index}() {{}}\n"),
        )
        .expect("write candidate");
    }
    std::fs::write(
        root.join("Cargo.toml"),
        "[package]\nname = \"provider-facts-budget\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
    )
    .expect("write Cargo.toml");
    let line_count_file = root.join("semantic-facts-lines.txt");
    let provider_path = bin_dir.join("rs-harness");
    std::fs::write(
        &provider_path,
        format!(
            "#!/bin/sh\nif [ \"$1\" = search ] && [ \"$2\" = semantic-facts ]; then wc -l > '{}'; printf '{{\"nodes\":[],\"edges\":[]}}\\n'; exit 0; fi\nprintf 'rs args='; for arg in \"$@\"; do printf '[%s]' \"$arg\"; done; printf '\\n'\n",
            line_count_file.display()
        ),
    )
    .expect("write provider");
    make_executable(&provider_path);
    write_activation(
        &root,
        &[provider("rust", vec![provider_path.display().to_string()])],
    );

    let output = asp_command(&root)
        .env("PATH", prepend_path(&bin_dir))
        .env("PRJ_CACHE_HOME", &cache_home)
        .args(["rust", "search", "pipe", "queue", "--view", "seeds", "."])
        .output()
        .expect("run asp search pipe");

    assert!(
        output.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("stdout");
    assert!(
        stdout.contains("providerFacts:used[")
            && stdout.contains("factCandidates=24")
            && stdout.contains("truncatedCandidates="),
        "{stdout}"
    );
    let line_count = std::fs::read_to_string(&line_count_file)
        .expect("read semantic facts line count")
        .trim()
        .parse::<usize>()
        .expect("parse semantic facts line count");
    assert_eq!(line_count, 24, "{stdout}");
    assert_trace_elapsed_under_gate_ms(
        &["rust", "search", "pipe"],
        &stdout,
        ASP_PROVIDER_FACTS_PHASE_PERFORMANCE_GATE_MS,
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn search_pipe_does_not_call_provider_facts_without_capability() {
    let root = temp_project_root("provider-facts-capability-gate");
    let bin_dir = root.join(".bin");
    let cache_home = root.join(".cache");
    let marker = root.join("semantic-facts-called");
    std::fs::create_dir_all(&bin_dir).expect("create bin dir");
    write_regular_search_fixtures(&root);
    let provider_path = bin_dir.join("gslph");
    std::fs::write(
        &provider_path,
        format!(
            "#!/bin/sh\nprintf called > '{}'\nsleep 4\nprintf '{{\"nodes\":[],\"edges\":[]}}\\n'\n",
            marker.display()
        ),
    )
    .expect("write provider");
    make_executable(&provider_path);
    write_activation(
        &root,
        &[provider(
            "gerbil-scheme",
            vec![provider_path.display().to_string()],
        )],
    );

    let output = asp_command(&root)
        .env("PATH", prepend_path(&bin_dir))
        .env("PRJ_CACHE_HOME", &cache_home)
        .args([
            "gerbil-scheme",
            "search",
            "pipe",
            "list.ss",
            "--workspace",
            ".",
            "--view",
            "seeds",
        ])
        .output()
        .expect("run asp search pipe");

    assert!(
        output.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        !marker.exists(),
        "provider semantic-facts command should not run without semanticFacts capability"
    );
    let stdout = String::from_utf8(output.stdout).expect("stdout");
    assert!(stdout.contains("providerFacts:skipped["), "stdout={stdout}");
    assert_trace_elapsed_under_gate(&["gerbil-scheme", "search", "pipe", "list.ss"], &stdout);
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn search_pipe_generic_action_query_skips_source_index_inside_phase_gate() {
    let root = temp_project_root("search-pipe-source-index-generic-action-gate");
    let bin_dir = root.join(".bin");
    let provider_path = bin_dir.join("rs-harness");
    write_regular_search_fixtures(&root);
    write_echo_provider(&bin_dir, "rs-harness", "rs");
    write_activation(
        &root,
        &[provider("rust", vec![provider_path.display().to_string()])],
    );
    agent_semantic_client::refresh_source_index(&root).expect("refresh source index");

    let output = asp_command(&root)
        .args([
            "rust",
            "search",
            "pipe",
            "owner-items selector-code",
            "--workspace",
            ".",
            "--view",
            "seeds",
        ])
        .output()
        .expect("run asp search pipe generic action query");

    assert!(
        output.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("stdout");
    assert!(stdout.contains("sourceIndex:skipped"), "stdout={stdout}");
    assert!(stdout.contains("reason=query-gate"), "stdout={stdout}");
    assert!(!stdout.contains("sourceIndex:used"), "stdout={stdout}");
    assert_trace_elapsed_under_gate_ms(
        &["rust", "search", "pipe", "generic-action-query"],
        &stdout,
        ASP_SEARCH_PHASE_PERFORMANCE_GATE_MS,
    );
    assert_render_trace_under_gate(&["rust", "search", "pipe", "generic-action-query"], &stdout);
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn search_pipe_broad_query_blocks_before_backend_collection() {
    let root = temp_project_root("search-pipe-broad-query-budget-gate");
    let bin_dir = root.join(".bin");
    let cache_home = root.join(".cache");
    let marker = root.join("provider-called");
    std::fs::create_dir_all(&bin_dir).expect("create bin dir");
    write_regular_search_fixtures(&root);
    let provider_path = bin_dir.join("gslph");
    std::fs::write(
        &provider_path,
        format!(
            "#!/bin/sh\nprintf called > '{}'\nsleep 4\nprintf 'unexpected provider call\\n'\n",
            marker.display()
        ),
    )
    .expect("write provider");
    make_executable(&provider_path);
    write_activation(
        &root,
        &[provider(
            "gerbil-scheme",
            vec![provider_path.display().to_string()],
        )],
    );

    let output = asp_command(&root)
        .env("PATH", prepend_path(&bin_dir))
        .env("PRJ_CACHE_HOME", &cache_home)
        .args([
            "gerbil-scheme",
            "search",
            "pipe",
            "self apply old comment style migrate source helpers to gerbil-utils list.ss typed-combinator-style doc examples R013 engineering comment quality",
            "--workspace",
            ".",
            "--view",
            "seeds",
        ])
        .output()
        .expect("run broad search pipe");

    assert!(
        output.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(!marker.exists(), "broad query should not reach provider");
    let stdout = String::from_utf8(output.stdout).expect("stdout");
    assert!(
        stdout.contains("queryBudget:blocked[") && stdout.contains("reason=query-too-broad"),
        "stdout={stdout}"
    );
    assert!(
        stdout.contains(
            "nextCommand=asp fd -query 'gerbil-utils|list.ss|typed-combinator-style|r013'"
        ),
        "stdout={stdout}"
    );
    assert_trace_elapsed_under_gate_ms(
        &["gerbil-scheme", "search", "pipe", "broad-query"],
        &stdout,
        ASP_BLOCKED_QUERY_PHASE_PERFORMANCE_GATE_MS,
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn search_fzf_broad_query_blocks_before_backend_collection() {
    let root = temp_project_root("search-fzf-broad-query-budget-gate");
    let bin_dir = root.join(".bin");
    let marker = root.join("provider-called");
    std::fs::create_dir_all(&bin_dir).expect("create bin dir");
    write_regular_search_fixtures(&root);
    let provider_path = bin_dir.join("gslph");
    std::fs::write(
        &provider_path,
        format!(
            "#!/bin/sh\nprintf called > '{}'\nsleep 4\nprintf 'unexpected provider call\\n'\n",
            marker.display()
        ),
    )
    .expect("write provider");
    make_executable(&provider_path);
    write_activation(
        &root,
        &[provider(
            "gerbil-scheme",
            vec![provider_path.display().to_string()],
        )],
    );

    let output = asp_command(&root)
        .env("PATH", prepend_path(&bin_dir))
        .args([
            "gerbil-scheme",
            "search",
            "fzf",
            "self apply old comment style migrate source helpers to gerbil-utils list.ss typed-combinator-style doc examples R013 engineering comment quality",
            "--view",
            "seeds",
        ])
        .output()
        .expect("run broad search fzf");

    assert!(
        output.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(!marker.exists(), "broad query should not reach provider");
    let stdout = String::from_utf8(output.stdout).expect("stdout");
    assert!(
        stdout.contains("[search-fzf]")
            && stdout.contains("noOutput reason=query-too-broad")
            && stdout.contains(
                "nextCommand=asp fd -query 'gerbil-utils|list.ss|typed-combinator-style|r013'"
            ),
        "stdout={stdout}"
    );
    assert_trace_elapsed_under_gate_ms(
        &["gerbil-scheme", "search", "fzf", "broad-query"],
        &stdout,
        ASP_BLOCKED_QUERY_PHASE_PERFORMANCE_GATE_MS,
    );
    let _ = std::fs::remove_dir_all(root);
}

fn performance_gate_for_language(language: &str) -> Duration {
    if language == "julia" {
        JULIA_FACADE_PERFORMANCE_GATE
    } else {
        ASP_FACADE_PERFORMANCE_GATE
    }
}

fn assert_regular_command_output(args: &[&str], stdout: &str, label: &str) {
    if matches!(args.get(1), Some(&"query")) {
        assert!(
            stdout.contains(&format!("{label} args="))
                || stdout.contains("reason=owner-not-found")
                || stdout.contains("[search-owner]"),
            "args={args:?} stdout={stdout}"
        );
        return;
    }
    if matches!(args.get(1..3), Some(["search", "prime"])) {
        assert!(
            stdout.contains("[search-prime]") && stdout.contains("native-fd-prime-frontier-v1"),
            "args={args:?} stdout={stdout}"
        );
        return;
    }
    if matches!(args.get(1..3), Some(["search", "pipe"])) && args.contains(&"graph-turbo-request") {
        let payload: serde_json::Value = serde_json::from_str(stdout)
            .unwrap_or_else(|error| panic!("args={args:?} graph request json: {error}; {stdout}"));
        assert_eq!(
            payload["packetKind"].as_str(),
            Some("graph-turbo-request"),
            "{payload}"
        );
        assert_trace_elapsed_under_gate(args, stdout);
        return;
    }
    assert!(
        stdout.contains("[search-pipe]"),
        "args={args:?} stdout={stdout}"
    );
    if matches!(args.get(1..3), Some(["search", "pipe"])) {
        assert!(
            (stdout.contains("providerFacts:used[") || stdout.contains("providerFacts:skipped["))
                && stdout.contains("elapsedMs=")
                && stdout.contains("render:used[")
                && stdout.contains("totalMs="),
            "args={args:?} stdout={stdout}"
        );
        assert_trace_elapsed_under_gate(args, stdout);
        assert_render_trace_under_gate(args, stdout);
    }
}

fn assert_trace_elapsed_under_gate(args: &[&str], stdout: &str) {
    assert_trace_elapsed_under_gate_ms(args, stdout, ASP_SEARCH_PHASE_PERFORMANCE_GATE_MS);
}

fn assert_trace_elapsed_under_gate_ms(args: &[&str], stdout: &str, gate_ms: u64) {
    let max_elapsed_ms = stdout
        .match_indices("elapsedMs=")
        .filter_map(|(index, _)| {
            let value_start = index + "elapsedMs=".len();
            let digits = stdout[value_start..]
                .chars()
                .take_while(|character| character.is_ascii_digit())
                .collect::<String>();
            digits.parse::<u64>().ok()
        })
        .max()
        .unwrap_or(0);
    assert!(
        max_elapsed_ms < gate_ms,
        "args={args:?} exceeded search phase gate {gate_ms}ms; maxElapsedMs={max_elapsed_ms}; stdout={stdout}"
    );
}

fn assert_render_trace_under_gate(args: &[&str], stdout: &str) {
    for field in ["compactMs", "graphMs", "totalMs"] {
        assert_trace_field_under_gate_ms(args, stdout, field, ASP_RENDER_PHASE_PERFORMANCE_GATE_MS);
    }
}

fn assert_trace_field_under_gate_ms(args: &[&str], stdout: &str, field: &str, gate_ms: u64) {
    let marker = format!("{field}=");
    let max_field_ms = stdout
        .match_indices(&marker)
        .filter_map(|(index, _)| {
            let value_start = index + marker.len();
            let digits = stdout[value_start..]
                .chars()
                .take_while(|character| character.is_ascii_digit())
                .collect::<String>();
            digits.parse::<u64>().ok()
        })
        .max()
        .unwrap_or(0);
    assert!(
        max_field_ms < gate_ms,
        "args={args:?} exceeded render phase gate {gate_ms}ms for {field}; maxFieldMs={max_field_ms}; stdout={stdout}"
    );
}

fn write_regular_search_fixtures(root: &std::path::Path) {
    std::fs::create_dir_all(root.join("src")).expect("create src");
    std::fs::write(root.join("src/lib.rs"), "pub struct RustGate;\n").expect("write rust");
    std::fs::write(
        root.join("src/index.ts"),
        "export const typescriptGate = 1;\n",
    )
    .expect("write ts");
    std::fs::write(
        root.join("src/main.py"),
        "def python_gate():\n    return 1\n",
    )
    .expect("write python");
    std::fs::write(root.join("src/main.jl"), "const julia_gate = 1\n").expect("write julia");
    std::fs::write(root.join("src/main.ss"), "(export gerbil-gate)\n").expect("write gerbil");
    std::fs::write(
        root.join("Cargo.toml"),
        "[package]\nname = \"regular-gate\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
    )
    .expect("write Cargo.toml");
    std::fs::write(root.join("package.json"), "{\"name\":\"regular-gate\"}\n")
        .expect("write package.json");
    std::fs::write(
        root.join("pyproject.toml"),
        "[project]\nname = \"regular-gate\"\nversion = \"0.1.0\"\n",
    )
    .expect("write pyproject.toml");
    std::fs::write(root.join("Project.toml"), "name = \"regular-gate\"\n")
        .expect("write Project.toml");
    std::fs::write(root.join("gerbil.pkg"), "(package: regular-gate)\n").expect("write gerbil.pkg");
}

#[test]
fn dependency_manifest_graph_requests_finish_inside_performance_gate() {
    let root = temp_project_root("dependency-manifest-performance-gate");
    let bin_dir = root.join(".bin");
    let cache_home = root.join(".cache");
    let providers = [
        ("rust", "rs-harness", "rs"),
        ("typescript", "ts-harness", "ts"),
        ("python", "py-harness", "py"),
        ("julia", "asp-julia-harness", "julia"),
        ("gerbil-scheme", "gslph", "gerbil"),
    ];
    std::fs::create_dir_all(&bin_dir).expect("create bin dir");
    write_dependency_manifest_fixtures(&root);
    for (_, binary, label) in providers.iter().copied() {
        write_echo_provider(&bin_dir, binary, label);
    }
    write_activation(
        &root,
        &providers
            .iter()
            .map(|(language, binary, _)| {
                provider(language, vec![bin_dir.join(binary).display().to_string()])
            })
            .collect::<Vec<_>>(),
    );

    for (language, _, _) in providers.iter().copied() {
        let args = [
            language,
            "search",
            "pipe",
            "dep159",
            "--view",
            "graph-turbo-request",
            ".",
        ];
        let warmup = asp_command(&root)
            .env("PATH", prepend_path(&bin_dir))
            .env("PRJ_CACHE_HOME", &cache_home)
            .args(args)
            .output()
            .unwrap_or_else(|error| panic!("warm asp {args:?}: {error}"));
        assert!(
            warmup.status.success(),
            "warm args={args:?} stderr={}",
            String::from_utf8_lossy(&warmup.stderr)
        );

        let started_at = Instant::now();
        let output = asp_command(&root)
            .env("PATH", prepend_path(&bin_dir))
            .env("PRJ_CACHE_HOME", &cache_home)
            .args(args)
            .output()
            .unwrap_or_else(|error| panic!("run asp {args:?}: {error}"));
        let elapsed = started_at.elapsed();
        assert!(
            output.status.success(),
            "args={args:?} stderr={}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(
            elapsed < ASP_FACADE_PERFORMANCE_GATE,
            "asp {args:?} exceeded {ASP_FACADE_PERFORMANCE_GATE:?}; elapsed={elapsed:?}; stdout={}; stderr={}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        let payload: serde_json::Value =
            serde_json::from_slice(&output.stdout).expect("graph request json");
        assert!(
            payload["graph"]["nodes"].as_array().is_some_and(|nodes| {
                nodes.iter().any(|node| {
                    node["kind"].as_str() == Some("dependency")
                        && node["value"].as_str() == Some("dep159")
                        && node["confidence"].as_str() == Some("exact")
                })
            }),
            "{payload}"
        );
    }
    let _ = std::fs::remove_dir_all(root);
}

fn write_dependency_manifest_fixtures(root: &std::path::Path) {
    std::fs::create_dir_all(root.join("src")).expect("create src");
    std::fs::write(root.join("src/lib.rs"), "pub struct DependencyGate;\n").expect("write rust");
    std::fs::write(
        root.join("src/index.ts"),
        "export const dependencyGate = 1;\n",
    )
    .expect("write ts");
    std::fs::write(root.join("src/main.py"), "dependency_gate = 1\n").expect("write python");
    std::fs::write(root.join("src/main.jl"), "const dependency_gate = 1\n").expect("write julia");
    std::fs::write(root.join("src/main.ss"), "(export dependency-gate)\n").expect("write gerbil");
    std::fs::write(
        root.join("Cargo.toml"),
        format!(
            "[package]\nname = \"dependency-gate\"\nversion = \"0.1.0\"\nedition = \"2024\"\n\n[dependencies]\n{}",
            (0..160)
                .map(|index| format!("dep{index} = \"1.{index}.0\"\n"))
                .collect::<String>()
        ),
    )
    .expect("write Cargo.toml");
    std::fs::write(
        root.join("package.json"),
        format!(
            "{{\n  \"dependencies\": {{\n{}\n  }}\n}}\n",
            (0..160)
                .map(|index| {
                    let suffix = if index == 159 { "" } else { "," };
                    format!("    \"dep{index}\": \"1.{index}.0\"{suffix}")
                })
                .collect::<Vec<_>>()
                .join("\n")
        ),
    )
    .expect("write package.json");
    std::fs::write(
        root.join("pyproject.toml"),
        format!(
            "[project]\nname = \"dependency-gate\"\nversion = \"0.1.0\"\ndependencies = [\n{}\n]\n",
            (0..160)
                .map(|index| format!("  \"dep{index}>=1.{index}.0\","))
                .collect::<Vec<_>>()
                .join("\n")
        ),
    )
    .expect("write pyproject.toml");
    std::fs::write(
        root.join("Project.toml"),
        format!(
            "[deps]\n{}",
            (0..160)
                .map(|index| format!("dep{index} = \"00000000-0000-0000-0000-{index:012}\"\n"))
                .collect::<String>()
        ),
    )
    .expect("write Project.toml");
    std::fs::write(
        root.join("Manifest.toml"),
        (0..160)
            .map(|index| {
                format!(
                    "[[deps.dep{index}]]\nuuid = \"00000000-0000-0000-0000-{index:012}\"\nversion = \"1.{index}.0\"\n"
                )
            })
            .collect::<String>(),
    )
    .expect("write Manifest.toml");
    std::fs::write(
        root.join("gerbil.pkg"),
        format!(
            "(package: dependency-gate\n{})\n",
            (0..160)
                .map(|index| format!(" depend: (\"dep{index}\")"))
                .collect::<Vec<_>>()
                .join("\n")
        ),
    )
    .expect("write gerbil.pkg");
}
