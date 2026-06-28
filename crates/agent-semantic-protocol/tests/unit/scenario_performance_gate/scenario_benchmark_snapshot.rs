use std::path::Path;

use serde::Deserialize;

use super::{
    LanguageScenarioBenchmarkRequirement, SHARED_AGENT_POLICY_ID_SCHEMA,
    SHARED_SCENARIO_BENCHMARK_SCHEMA, SharedBenchmarkToml, SharedScenarioToml, read_toml,
    require_non_empty_manifest_field, require_supported_language_harness,
};

#[derive(Debug, Deserialize)]
struct ScenarioBenchmarkSnapshot {
    schema: String,
    agent_policy_id_schema: String,
    language: String,
    scenario_id: String,
    scenario_path: String,
    benchmark_path: String,
    #[serde(default)]
    policy_ids: Vec<String>,
    harness: String,
    #[serde(default)]
    test: Option<String>,
    #[serde(default)]
    bench: Option<String>,
    snapshot_role: String,
}

pub(super) fn validate(
    requirement: &LanguageScenarioBenchmarkRequirement,
    root: &Path,
    invalid: &mut Vec<String>,
) {
    let Some(snapshot_root) = requirement.snapshot_root else {
        return;
    };

    let repo_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let scenario_path = root.join("scenario.toml");
    let benchmark_path = root.join("benchmark.toml");
    let scenario: SharedScenarioToml = read_toml(&scenario_path);
    let benchmark: SharedBenchmarkToml = read_toml(&benchmark_path);
    let snapshot_path = repo_root.join(snapshot_root).join(format!(
        "scenario_benchmark__{}.snap",
        snapshot_id(&scenario.id)
    ));

    if !snapshot_path.is_file() {
        invalid.push(format!(
            "{}: missing scenario benchmark snapshot for {} at {}",
            scenario_path.display(),
            scenario.id,
            snapshot_path.display()
        ));
        return;
    }

    let snapshot: ScenarioBenchmarkSnapshot = read_toml(&snapshot_path);
    validate_snapshot_schema(&snapshot_path, &snapshot, invalid);
    validate_snapshot_scenario(requirement, &snapshot_path, &scenario, &snapshot, invalid);
    validate_snapshot_benchmark(requirement, &snapshot_path, &benchmark, &snapshot, invalid);
}

fn validate_snapshot_schema(
    snapshot_path: &Path,
    snapshot: &ScenarioBenchmarkSnapshot,
    invalid: &mut Vec<String>,
) {
    if snapshot.schema != SHARED_SCENARIO_BENCHMARK_SCHEMA {
        invalid.push(format!(
            "{}: schema must be {SHARED_SCENARIO_BENCHMARK_SCHEMA:?}",
            snapshot_path.display()
        ));
    }
    if snapshot.agent_policy_id_schema != SHARED_AGENT_POLICY_ID_SCHEMA {
        invalid.push(format!(
            "{}: agent_policy_id_schema must be {SHARED_AGENT_POLICY_ID_SCHEMA:?}",
            snapshot_path.display()
        ));
    }
    require_non_empty_manifest_field(
        invalid,
        snapshot_path,
        "scenario_path",
        &snapshot.scenario_path,
    );
    require_non_empty_manifest_field(
        invalid,
        snapshot_path,
        "benchmark_path",
        &snapshot.benchmark_path,
    );
    require_non_empty_manifest_field(
        invalid,
        snapshot_path,
        "snapshot_role",
        &snapshot.snapshot_role,
    );
}

fn validate_snapshot_scenario(
    requirement: &LanguageScenarioBenchmarkRequirement,
    snapshot_path: &Path,
    scenario: &SharedScenarioToml,
    snapshot: &ScenarioBenchmarkSnapshot,
    invalid: &mut Vec<String>,
) {
    if snapshot.language != requirement.language {
        invalid.push(format!(
            "{}: language {:?} must match {:?}",
            snapshot_path.display(),
            snapshot.language,
            requirement.language
        ));
    }
    if snapshot.scenario_id != scenario.id {
        invalid.push(format!(
            "{}: scenario_id {:?} must match {:?}",
            snapshot_path.display(),
            snapshot.scenario_id,
            scenario.id
        ));
    }
    if snapshot.policy_ids != scenario.policy_ids {
        invalid.push(format!(
            "{}: policy_ids {:?} must match scenario {:?}",
            snapshot_path.display(),
            snapshot.policy_ids,
            scenario.policy_ids
        ));
    }
}

fn validate_snapshot_benchmark(
    requirement: &LanguageScenarioBenchmarkRequirement,
    snapshot_path: &Path,
    benchmark: &SharedBenchmarkToml,
    snapshot: &ScenarioBenchmarkSnapshot,
    invalid: &mut Vec<String>,
) {
    require_supported_language_harness(
        requirement.language,
        snapshot_path,
        &snapshot.harness,
        invalid,
    );
    if snapshot.test.as_deref().unwrap_or("").trim().is_empty()
        && snapshot.bench.as_deref().unwrap_or("").trim().is_empty()
    {
        invalid.push(format!(
            "{}: snapshot must name test or bench",
            snapshot_path.display()
        ));
    }
    if snapshot.harness != benchmark.harness {
        invalid.push(format!(
            "{}: harness {:?} must match benchmark {:?}",
            snapshot_path.display(),
            snapshot.harness,
            benchmark.harness
        ));
    }
    if snapshot.test != benchmark.test {
        invalid.push(format!(
            "{}: test {:?} must match benchmark {:?}",
            snapshot_path.display(),
            snapshot.test,
            benchmark.test
        ));
    }
    if snapshot.bench != benchmark.bench {
        invalid.push(format!(
            "{}: bench {:?} must match benchmark {:?}",
            snapshot_path.display(),
            snapshot.bench,
            benchmark.bench
        ));
    }
}

fn snapshot_id(id: &str) -> String {
    id.chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character.to_ascii_lowercase()
            } else {
                '_'
            }
        })
        .collect()
}
