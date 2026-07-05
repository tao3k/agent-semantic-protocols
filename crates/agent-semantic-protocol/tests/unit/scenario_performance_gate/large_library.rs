use std::path::{Path, PathBuf};

use serde_json::Value;

use super::scenario_performance_gate_impl::{
    is_non_scenario_dir, read_dir_sorted, read_json, string_field,
};
use super::shared::{JULIA_LARGE_LIBRARY_STEP_MAX_ELAPSED_MS, LARGE_LIBRARY_STEP_MAX_ELAPSED_MS};

pub(super) fn large_library_step_max_elapsed_ms(language: &str) -> u64 {
    if language == "julia" {
        JULIA_LARGE_LIBRARY_STEP_MAX_ELAPSED_MS
    } else {
        LARGE_LIBRARY_STEP_MAX_ELAPSED_MS
    }
}

#[derive(Clone, Debug)]
pub(super) struct LargeLibraryElapsedGate {
    pub(crate) path: PathBuf,
    pub(crate) language: String,
    pub(crate) package: String,
    pub(crate) step_id: String,
    pub(crate) max_elapsed_ms: Option<u64>,
}

pub(super) fn discover_large_library_elapsed_gates(
    repo_root: &Path,
) -> Vec<LargeLibraryElapsedGate> {
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
