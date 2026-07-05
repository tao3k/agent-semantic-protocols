use std::collections::BTreeSet;
use std::env;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use serde_json::Value;

use super::runtime_gates::{duration_millis_from_manifest, is_ascii_digits};
use super::shared::{
    AGENT_POLICY_ID_GRAMMAR, REQUIRED_WORKSPACE_ARGUMENT_POLICY_IDS, ScenarioPolicyIds,
    SharedBenchmarkToml, SharedScenarioToml,
};

pub(crate) fn asp_unit_scenarios_cover_workspace_argument_guards() {
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

pub(super) fn discover_scenario_policy_ids(scenario_root: &Path) -> BTreeSet<String> {
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

pub(super) fn validate_language_harness_json_boundary(
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

pub(crate) fn language_harnesses_do_not_use_retired_agent_policy_ids() {
    let languages = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../languages");
    let mut invalid = Vec::new();
    for relative in RETIRED_POLICY_ID_SCAN_PATHS {
        let path = languages.join(relative);
        if path.exists() {
            collect_retired_policy_ids(&path, &mut invalid);
        }
    }

    assert!(
        invalid.is_empty(),
        "retired agent policy ids must use {AGENT_POLICY_ID_GRAMMAR}:\n{}",
        invalid.join("\n")
    );
}

const RETIRED_POLICY_ID_SCAN_PATHS: &[&str] = &[
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

pub(super) fn validate_gerbil_benchmark_ss(
    path: &Path,
    invalid: &mut Vec<String>,
    hot_path_coverage: &mut BTreeSet<&'static str>,
) {
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
    if [
        "routeSource",
        "maxProviderProcessCount",
        "maxStdoutBytes",
        "fallbackReason",
    ]
    .iter()
    .all(|token| text.contains(token))
    {
        hot_path_coverage.insert("gerbil-scheme");
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

pub(super) fn benchmark_has_hot_path_metadata(benchmark: &SharedBenchmarkToml) -> bool {
    benchmark
        .route_source
        .as_deref()
        .is_some_and(|value| !value.trim().is_empty())
        && benchmark.max_provider_process_count.is_some()
        && benchmark.max_stdout_bytes.is_some()
        && benchmark
            .fallback_reason
            .as_deref()
            .is_some_and(|value| !value.trim().is_empty())
}

pub(super) fn canonical_benchmark_language(language: &str) -> &'static str {
    match language {
        "rust" => "rust",
        "typescript" => "typescript",
        "python" => "python",
        "julia" => "julia",
        "gerbil-scheme" => "gerbil-scheme",
        "orgize" => "orgize",
        _ => "unknown",
    }
}

pub(super) fn gerbil_benchmark_rule(text: &str) -> Option<&str> {
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

pub(super) fn collect_retired_policy_ids(dir: &Path, invalid: &mut Vec<String>) {
    if is_ignored_retired_policy_scan_path(dir) {
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
        if is_ignored_retired_policy_scan_path(&path) {
            continue;
        }
        if path.is_dir() {
            collect_retired_policy_ids(&path, invalid);
        } else if path.is_file() {
            validate_no_retired_policy_ids(&path, invalid);
        }
    }
}

pub(super) fn validate_no_retired_policy_ids(path: &Path, invalid: &mut Vec<String>) {
    let Ok(text) = fs::read_to_string(path) else {
        return;
    };

    for (line_index, line) in text.lines().enumerate() {
        for token in policy_id_tokens(line) {
            if is_retired_policy_id(token) {
                invalid.push(format!(
                    "{}:{}: retired policy id {token:?} must match {AGENT_POLICY_ID_GRAMMAR}",
                    path.display(),
                    line_index + 1
                ));
            }
        }
    }
}

pub(super) fn policy_id_tokens(line: &str) -> impl Iterator<Item = &str> {
    line.split(|ch: char| !(ch.is_ascii_alphanumeric() || ch == '-' || ch == '_'))
        .filter(|token| !token.is_empty())
}

pub(super) fn is_retired_policy_id(token: &str) -> bool {
    has_numbered_retired_marker(token, "-AGENT-R") || has_numbered_retired_marker(token, "-PROJ-R")
}

pub(super) fn has_numbered_retired_marker(token: &str, marker: &str) -> bool {
    let Some(index) = token.find(marker) else {
        return false;
    };
    token[index + marker.len()..]
        .chars()
        .next()
        .is_some_and(|ch| ch.is_ascii_digit())
}

pub(super) fn is_ignored_retired_policy_scan_path(path: &Path) -> bool {
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

pub(super) fn require_non_empty_manifest_field(
    invalid: &mut Vec<String>,
    path: &Path,
    field: &str,
    value: &str,
) {
    if value.trim().is_empty() {
        invalid.push(format!("{}: {field} must not be empty", path.display()));
    }
}

pub(super) fn observed_timing_millis_from_manifest(
    benchmark: &SharedBenchmarkToml,
    key: &str,
) -> u128 {
    let value = benchmark
        .observed_timings
        .get(key)
        .unwrap_or_else(|| panic!("benchmark must record observed timing key {key:?}"));
    let Some(text) = value.as_str() else {
        panic!("observed timing key {key:?} must be a duration string, got {value:?}");
    };
    duration_millis_from_manifest(text)
}

pub(super) fn assert_observed_timing_inside_budget(
    benchmark: &SharedBenchmarkToml,
    key: &str,
    max_millis: u128,
    label: &str,
) {
    let observed = observed_timing_millis_from_manifest(benchmark, key);
    assert!(
        observed <= max_millis,
        "{label} observed timing {key}={observed}ms exceeds budget {max_millis}ms"
    );
}

pub(super) fn require_supported_language_harness(
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

pub(super) fn resolve_sandtable_workdir(repo_root: &Path, scenario: &Value) -> Option<PathBuf> {
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

pub(super) fn resolve_sandtable_workdir_candidate(
    repo_root: &Path,
    pattern: &str,
) -> Option<PathBuf> {
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

pub(super) fn expand_home_path(pattern: &str) -> String {
    if let Some(rest) = pattern.strip_prefix("~/")
        && let Some(home) = env::var_os("HOME")
    {
        return PathBuf::from(home).join(rest).display().to_string();
    }
    pattern.to_string()
}

pub(super) fn normalize_candidate_path(repo_root: &Path, value: &str) -> PathBuf {
    let path = PathBuf::from(value);
    if path.is_absolute() {
        path
    } else {
        repo_root.join(path)
    }
}

pub(super) fn batch_argv_line(step: &Value) -> String {
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

pub(super) fn run_julia_batch(binary: &Path, workdir: &Path, batch_input: &str) -> String {
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
pub(super) struct JuliaBatchStep {
    pub(crate) exit_code: i32,
    pub(crate) elapsed_ms: u64,
    pub(crate) stdout: String,
}

pub(super) fn parse_julia_batch_steps(stdout: &str) -> Vec<JuliaBatchStep> {
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

pub(super) fn parse_julia_batch_step_header(header: &str) -> JuliaBatchStep {
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

pub(super) fn policy_ids_from_scenario_toml(path: &Path, text: &str) -> Vec<String> {
    toml::from_str::<ScenarioPolicyIds>(text)
        .unwrap_or_else(|error| panic!("failed to parse {}: {error}", path.display()))
        .policy_ids
}

pub(super) fn is_agent_policy_id(value: &str) -> bool {
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

pub(super) fn is_upper_token(value: &str) -> bool {
    let mut chars = value.chars();
    chars.next().is_some_and(|ch| ch.is_ascii_uppercase())
        && chars.all(|ch| ch.is_ascii_uppercase() || ch.is_ascii_digit())
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

pub(super) fn is_non_scenario_dir(path: &Path) -> bool {
    matches!(
        path.file_name().and_then(|name| name.to_str()),
        Some("fixtures" | "receipts" | "root")
    )
}
