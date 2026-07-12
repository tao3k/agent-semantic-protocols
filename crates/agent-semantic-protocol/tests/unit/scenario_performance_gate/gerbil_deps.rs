use std::fs;
use std::path::{Path, PathBuf};
use std::time::Instant;

use super::runtime_gates::{duration_literal, duration_millis_from_manifest, read_toml};
use super::shared::SharedBenchmarkToml;
use crate::provider_command::support::{asp_command, prepend_path, temp_project_root};

pub(crate) fn asp_gerbil_deps_active_gxi_stdlib_hot_path_stays_inside_scenario_gate() {
    let crate_root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let scenario_root = crate_root
        .join("tests")
        .join("unit")
        .join("scenarios")
        .join("asp_gerbil_deps_active_gxi_stdlib_hot_path");
    let benchmark: SharedBenchmarkToml = read_toml(&scenario_root.join("benchmark.toml"));
    let max_total_ms = duration_millis_from_manifest(&benchmark.max_total);

    let root = temp_project_root("scenario-gerbil-deps-active-gxi-stdlib");
    let fixture = GerbilInstallFixture::write(&root);
    let command_args = [
        "gerbil-scheme",
        "search",
        "deps",
        "gerbil",
        ":std/srfi/13",
        "items",
        "--query",
        "string-prefix",
    ];

    let warmup = asp_command(&root)
        .env("PATH", prepend_path(&fixture.bin_dir))
        .args(command_args)
        .output()
        .expect("warm gerbil deps stdlib search");
    assert!(
        warmup.status.success(),
        "warmup stderr={}",
        String::from_utf8_lossy(&warmup.stderr)
    );

    let started_at = Instant::now();
    let output = asp_command(&root)
        .env("PATH", prepend_path(&fixture.bin_dir))
        .args(command_args)
        .output()
        .expect("run gerbil deps stdlib search");
    let elapsed = started_at.elapsed();
    assert!(
        output.status.success(),
        "status={:?} stderr={}",
        output.status,
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("stdout");
    for expected in [
        "[gerbil-deps] namespace=gerbil authority=active-gxi module=:std/srfi/13 scope=standard-library/srfi",
        "|use import=\"(import (only-in :std/srfi/13 string-prefix? string-prefix-ci?))\"",
        "|item kind=export name=string-prefix? selector=gerbil:/std/srfi/13#export/string-prefix?",
        "|item kind=export name=string-prefix-ci? selector=gerbil:/std/srfi/13#export/string-prefix-ci?",
    ] {
        assert!(
            stdout.contains(expected),
            "gerbil deps stdlib scenario missing {expected:?}; stdout={stdout}"
        );
    }
    assert!(
        !stdout.contains("string-prefix-length"),
        "query should prefer the predicate family over contains matches; stdout={stdout}"
    );
    assert!(
        stdout.len() <= benchmark.max_stdout_bytes.unwrap_or(4096) as usize,
        "gerbil deps stdout exceeded benchmark max_stdout_bytes; stdout={stdout}"
    );
    assert!(
        elapsed.as_millis() <= max_total_ms,
        "gerbil deps active-gxi stdlib hot path exceeded benchmark max_total={} observed={}",
        benchmark.max_total,
        duration_literal(elapsed)
    );

    let observed_total = duration_literal(elapsed);
    let performance_gate = serde_json::json!({
        "schemaId": "agent.semantic-protocols.semantic-hot-path-performance-gate",
        "schemaVersion": "1",
        "scenarioId": "asp-gerbil-deps-active-gxi-stdlib-hot-path",
        "languageId": "gerbil-scheme",
        "workspace": "not-required",
        "command": [
            "asp",
            "gerbil-scheme",
            "search",
            "deps",
            "gerbil",
            ":std/srfi/13",
            "items",
            "--query",
            "string-prefix"
        ],
        "phase": "warm",
        "expected": {
            "targetTotal": benchmark.target_total,
            "maxTotal": benchmark.max_total,
            "regressionBudget": benchmark.regression_budget,
            "maxProviderProcessCount": 0,
            "maxWorkspaceScanCount": 0,
            "maxStdoutBytes": benchmark.max_stdout_bytes,
            "routeSource": "gerbil-deps-index",
            "fallbackReason": "none"
        },
        "observed": {
            "observedTotal": observed_total,
            "providerProcessCount": 0,
            "workspaceScanCount": 0,
            "fallbackReason": "none",
            "stdoutBytes": stdout.len()
        },
        "verdict": "pass",
        "evidenceRefs": ["scenario:asp-gerbil-deps-active-gxi-stdlib-hot-path"]
    });
    assert_eq!(performance_gate["observed"]["providerProcessCount"], 0);
    assert_eq!(performance_gate["observed"]["workspaceScanCount"], 0);
    assert_eq!(performance_gate["observed"]["fallbackReason"], "none");
    assert_eq!(performance_gate["observed"]["stdoutBytes"], stdout.len());

    let _ = fs::remove_dir_all(root);
}

struct GerbilInstallFixture {
    bin_dir: PathBuf,
}

impl GerbilInstallFixture {
    fn write(root: &Path) -> Self {
        let prefix = root.join("gerbil-prefix");
        let bin_dir = prefix.join("bin");
        let source_dir = prefix.join("v0.18.2/src/std/srfi");
        fs::create_dir_all(&bin_dir).expect("create fixture bin dir");
        fs::create_dir_all(&source_dir).expect("create fixture source dir");
        let gxi = bin_dir.join("gxi");
        fs::write(&gxi, "#!/bin/sh\nexit 0\n").expect("write fake gxi");
        make_executable(&gxi);
        fs::write(
            source_dir.join("13.ss"),
            r#"(export
  string-prefix-length string-prefix-length-ci
  string-prefix? string-prefix-ci?
  string-suffix? string-suffix-ci?)
(include "srfi-13.scm")
"#,
        )
        .expect("write srfi/13.ss");
        fs::write(
            source_dir.join("srfi-13.scm"),
            r#"(def (string-prefix? s1 s2
                     (start1 0) (end1 (string-length s1))
                     (start2 0) (end2 (string-length s2)))
  (%string-prefix? s1 start1 end1 s2 start2 end2))

(def (string-prefix-ci? s1 s2
                        (start1 0) (end1 (string-length s1))
                        (start2 0) (end2 (string-length s2)))
  (%string-prefix-ci? s1 start1 end1 s2 start2 end2))
"#,
        )
        .expect("write srfi-13.scm");
        Self { bin_dir }
    }
}

fn make_executable(path: &Path) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut permissions = fs::metadata(path).expect("fixture metadata").permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(path, permissions).expect("chmod fixture");
    }
    #[cfg(not(unix))]
    {
        let _ = path;
    }
}
