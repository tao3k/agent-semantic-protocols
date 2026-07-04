use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

const SCENARIO_ID: &str = "search-package-linear-performance-monitoring";
const SCENARIO_ROOT: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/unit/scenarios/search_package_linear_performance_monitoring"
);

#[test]
fn search_package_linear_performance_monitoring_covers_all_unit_surfaces() {
    let started_at = Instant::now();
    let scenario = fs::read_to_string(Path::new(SCENARIO_ROOT).join("scenario.toml"))
        .expect("read scenario manifest");
    let benchmark = fs::read_to_string(Path::new(SCENARIO_ROOT).join("benchmark.toml"))
        .expect("read benchmark manifest");
    assert!(scenario.contains(&format!("id = \"{SCENARIO_ID}\"")));
    assert!(
        scenario.contains("SEARCH-AGENT-ASP-PERF-PACKAGE-LINEAR-MONITORING-001"),
        "scenario must carry the package-level search performance policy id"
    );
    assert_benchmark_contract(&benchmark);

    let surfaces = monitored_surfaces();
    assert!(
        !surfaces.is_empty(),
        "search package must expose monitored surfaces"
    );
    for surface in &surfaces {
        assert!(
            scenario.contains(surface),
            "scenario manifest must list monitored surface {surface:?}"
        );
        assert!(
            benchmark.contains(&format!("{surface} = \"")),
            "benchmark observed_timings must list monitored surface {surface:?}"
        );
    }
    assert!(
        started_at.elapsed() <= duration_from_manifest(&benchmark, "max_total"),
        "linear monitoring contract check exceeded benchmark max_total"
    );
}

fn monitored_surfaces() -> Vec<String> {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut surfaces = BTreeSet::new();
    for path in search_unit_test_files(manifest_dir) {
        let Some(stem) = path.file_stem().and_then(|stem| stem.to_str()) else {
            continue;
        };
        if stem == "search_package_scenarios" {
            continue;
        }
        surfaces.insert(stem.to_string());
    }
    for path in search_integration_test_files(manifest_dir) {
        let Some(stem) = path.file_stem().and_then(|stem| stem.to_str()) else {
            continue;
        };
        surfaces.insert(stem.to_string());
    }
    surfaces.into_iter().collect()
}

fn search_unit_test_files(manifest_dir: &Path) -> Vec<PathBuf> {
    read_rs_files(manifest_dir.join("tests").join("unit"))
}

fn search_integration_test_files(manifest_dir: &Path) -> Vec<PathBuf> {
    read_rs_files(manifest_dir.join("tests"))
        .into_iter()
        .filter(|path| {
            path.parent()
                .is_some_and(|parent| parent == manifest_dir.join("tests"))
        })
        .collect()
}

fn read_rs_files(dir: PathBuf) -> Vec<PathBuf> {
    let mut files = fs::read_dir(&dir)
        .unwrap_or_else(|error| panic!("read {}: {error}", dir.display()))
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.extension().and_then(|ext| ext.to_str()) == Some("rs"))
        .collect::<Vec<_>>();
    files.sort();
    files
}

fn assert_benchmark_contract(text: &str) {
    for expected in [
        "harness = \"libtest\"",
        "test = \"search_package_linear_performance_monitoring_covers_all_unit_surfaces\"",
        "route_source = \"search-package-linear-monitor\"",
        "max_provider_process_count = 0",
        "fallback_reason = \"none\"",
    ] {
        assert!(text.contains(expected), "benchmark missing {expected:?}");
    }
}

fn duration_from_manifest(text: &str, field: &str) -> Duration {
    let prefix = format!("{field} = \"");
    let value = text
        .lines()
        .find_map(|line| line.trim().strip_prefix(&prefix))
        .and_then(|value| value.strip_suffix('"'))
        .unwrap_or_else(|| panic!("benchmark missing duration field {field}"));
    if let Some(value) = value.strip_suffix("ms") {
        return Duration::from_millis(value.parse().expect("parse ms duration"));
    }
    if let Some(value) = value.strip_suffix("us") {
        return Duration::from_micros(value.parse().expect("parse us duration"));
    }
    panic!("unsupported benchmark duration {value:?}");
}
