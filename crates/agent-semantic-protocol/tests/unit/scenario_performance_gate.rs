use std::collections::BTreeSet;
use std::env;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use serde_json::Value;

const LARGE_LIBRARY_STEP_MAX_ELAPSED_MS: u64 = 300;
const JULIA_LARGE_LIBRARY_STEP_MAX_ELAPSED_MS: u64 = 5_000;
const JULIA_DATAFRAMES_BATCH_STEP_MAX_ELAPSED_MS: u64 = 20;
const REQUIRED_PERFORMANCE_SUBCOMMAND_POLICY_IDS: &[&str] = &[
    "ASP-PERF-SUBCOMMAND-QUERY-SELECTOR",
    "ASP-PERF-SUBCOMMAND-QUERY-TREESITTER",
    "ASP-PERF-SUBCOMMAND-SEARCH-DEPS",
    "ASP-PERF-SUBCOMMAND-SEARCH-FD",
    "ASP-PERF-SUBCOMMAND-SEARCH-FZF",
    "ASP-PERF-SUBCOMMAND-SEARCH-OWNER",
    "ASP-PERF-SUBCOMMAND-SEARCH-PIPE",
    "ASP-PERF-SUBCOMMAND-SEARCH-RG",
    "ASP-PERF-SUBCOMMAND-SOURCE-INDEX",
    "ASP-PERF-SUBCOMMAND-PROVIDER-FACTS",
];

#[test]
fn asp_unit_scenarios_have_rust_harness_benchmark_toml_gates() {
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
        "{}",
        rust_lang_project_harness::render_rust_scenario_benchmark_gate_failure(&receipt)
    );
    assert!(receipt.receipts.iter().all(|receipt| {
        receipt.benchmark.observed_total_ms <= receipt.benchmark.max_total_ms
            && receipt.benchmark.observed_memory_bytes <= receipt.benchmark.memory_budget_bytes
    }));
}

#[test]
fn asp_unit_scenarios_cover_perf_sensitive_query_search_subcommands() {
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

#[test]
fn large_library_sandtables_have_hard_elapsed_gates() {
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

#[test]
fn julia_dataframes_sandtable_batch_execution_stays_inside_hard_gates() {
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
    let output = run_julia_batch(&binary, &workdir, &batch_input);
    let observed = parse_julia_batch_steps(&output);

    assert_eq!(
        observed.len(),
        steps.len(),
        "julia batch output count mismatch\nstdout:\n{output}"
    );
    for (step, observed_step) in steps.iter().zip(observed.iter()) {
        let step_id = string_field(step, "id").unwrap_or_else(|| "<unknown>".to_string());
        assert_eq!(
            observed_step.exit_code, 0,
            "julia batch step {step_id} failed with exit={}:\n{}",
            observed_step.exit_code, observed_step.stdout
        );
        assert!(
            observed_step.elapsed_ms <= JULIA_DATAFRAMES_BATCH_STEP_MAX_ELAPSED_MS,
            "julia batch step {step_id} observed {}ms exceeds hard gate {}ms",
            observed_step.elapsed_ms,
            JULIA_DATAFRAMES_BATCH_STEP_MAX_ELAPSED_MS
        );
    }
}

#[test]
fn python_sandtable_runner_does_not_resolve_language_harness_binaries() {
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
        policy_ids.extend(policy_ids_from_scenario_toml(&text));
    }
    policy_ids
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
        Some("asp-julia-harness"),
        "julia batch command must start with asp-julia-harness: {argv:?}"
    );
    argv[1..].join("\t")
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

fn policy_ids_from_scenario_toml(text: &str) -> Vec<String> {
    text.lines()
        .find_map(|line| line.trim().strip_prefix("policy_ids").map(str::trim))
        .and_then(|value| value.strip_prefix('=').map(str::trim))
        .and_then(|value| {
            value
                .strip_prefix('[')
                .and_then(|value| value.strip_suffix(']'))
        })
        .map(|inner| {
            inner
                .split(',')
                .filter_map(|value| {
                    let value = value.trim();
                    value
                        .strip_prefix('"')
                        .and_then(|value| value.strip_suffix('"'))
                        .map(str::to_string)
                })
                .collect()
        })
        .unwrap_or_default()
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
