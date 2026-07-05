use super::large_library::{
    discover_large_library_elapsed_gates, large_library_step_max_elapsed_ms,
};
use super::scenario_benchmark_manifest::{
    discover_benchmark_ss_files, discover_toml_scenario_benchmark_roots,
    validate_toml_scenario_benchmark,
};
use super::scenario_performance_gate_impl::{read_json, render_gates, string_field};
use super::scenario_policy_scan::{
    batch_argv_line, parse_julia_batch_steps, resolve_sandtable_workdir, run_julia_batch,
    validate_gerbil_benchmark_ss,
};
use super::shared::{
    JULIA_DATAFRAMES_BATCH_SAMPLE_COUNT, JULIA_DATAFRAMES_BATCH_STEP_MAX_ELAPSED_MS,
    JULIA_LARGE_LIBRARY_STEP_MAX_ELAPSED_MS, LANGUAGE_SCENARIO_BENCHMARK_REQUIREMENTS,
    LARGE_LIBRARY_STEP_MAX_ELAPSED_MS, SHARED_AGENT_POLICY_ID_SCHEMA,
    SHARED_SCENARIO_BENCHMARK_SCHEMA, ScenarioBenchmarkSyntax,
};
use serde_json::Value;
use std::collections::BTreeSet;
use std::env;
use std::fs;
use std::path::Path;

pub(super) fn language_harnesses_have_shared_scenario_benchmark_schema_coverage() {
    let repo_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    for schema_path in [
        SHARED_SCENARIO_BENCHMARK_SCHEMA,
        SHARED_AGENT_POLICY_ID_SCHEMA,
    ] {
        assert!(
            repo_root.join(schema_path).is_file(),
            "shared scenario benchmark schema is missing: {schema_path}"
        );
    }

    let mut missing = Vec::new();
    let mut invalid = Vec::new();
    let mut hot_path_coverage = BTreeSet::new();
    for requirement in LANGUAGE_SCENARIO_BENCHMARK_REQUIREMENTS {
        let root = repo_root.join(requirement.root);
        match requirement.syntax {
            ScenarioBenchmarkSyntax::TomlPair => {
                let pairs = discover_toml_scenario_benchmark_roots(&root);
                if pairs.is_empty() {
                    missing.push(format!("{}:{}", requirement.language, requirement.root));
                }
                for pair_root in pairs {
                    validate_toml_scenario_benchmark(
                        requirement.language,
                        &pair_root,
                        &mut invalid,
                        &mut hot_path_coverage,
                    );
                }
            }
            ScenarioBenchmarkSyntax::GerbilBenchmarkSs => {
                let paths = discover_benchmark_ss_files(&root);
                if paths.is_empty() {
                    missing.push(format!("{}:{}", requirement.language, requirement.root));
                }
                for path in paths {
                    validate_gerbil_benchmark_ss(&path, &mut invalid, &mut hot_path_coverage);
                }
            }
        }
    }

    assert!(
        missing.is_empty(),
        "language harnesses must each expose shared scenario benchmark coverage through benchmark.toml or benchmark.ss; missing={missing:?}"
    );
    assert!(
        invalid.is_empty(),
        "language scenario benchmark manifests must satisfy {SHARED_SCENARIO_BENCHMARK_SCHEMA}; invalid={invalid:?}"
    );
    let missing_hot_path = LANGUAGE_SCENARIO_BENCHMARK_REQUIREMENTS
        .iter()
        .map(|requirement| requirement.language)
        .filter(|language| !hot_path_coverage.contains(*language))
        .collect::<Vec<_>>();
    assert!(
        missing_hot_path.is_empty(),
        "language harnesses must each expose at least one scenario benchmark with route_source/max_provider_process_count/max_stdout_bytes/fallback_reason hot-path metadata; missing={missing_hot_path:?}; observed={hot_path_coverage:?}"
    );
}

pub(super) fn large_library_sandtables_have_hard_elapsed_gates() {
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

pub(super) fn julia_dataframes_sandtable_batch_execution_stays_inside_hard_gates() {
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
    let samples = (0..JULIA_DATAFRAMES_BATCH_SAMPLE_COUNT)
        .map(|_| {
            let output = run_julia_batch(&binary, &workdir, &batch_input);
            let observed = parse_julia_batch_steps(&output);

            assert_eq!(
                observed.len(),
                steps.len(),
                "julia batch output count mismatch\nstdout:\n{output}"
            );
            observed
        })
        .collect::<Vec<_>>();

    for (step_index, step) in steps.iter().enumerate() {
        let step_id = string_field(step, "id").unwrap_or_else(|| "<unknown>".to_string());
        let step_samples = samples
            .iter()
            .map(|sample| &sample[step_index])
            .collect::<Vec<_>>();

        for observed_step in &step_samples {
            assert_eq!(
                observed_step.exit_code, 0,
                "julia batch step {step_id} failed with exit={}:\n{}",
                observed_step.exit_code, observed_step.stdout
            );
        }

        let best_elapsed_ms = step_samples
            .iter()
            .map(|observed_step| observed_step.elapsed_ms)
            .min()
            .expect("julia batch performance samples must not be empty");
        let sample_elapsed_ms = step_samples
            .iter()
            .map(|observed_step| observed_step.elapsed_ms.to_string())
            .collect::<Vec<_>>()
            .join(",");
        assert!(
            best_elapsed_ms <= JULIA_DATAFRAMES_BATCH_STEP_MAX_ELAPSED_MS,
            "julia batch step {step_id} best {best_elapsed_ms}ms exceeds hard gate {}ms across samples [{sample_elapsed_ms}]",
            JULIA_DATAFRAMES_BATCH_STEP_MAX_ELAPSED_MS,
        );
    }
}

pub(super) fn python_sandtable_runner_does_not_resolve_language_harness_binaries() {
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
