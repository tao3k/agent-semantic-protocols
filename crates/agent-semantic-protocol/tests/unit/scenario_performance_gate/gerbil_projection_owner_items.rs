use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::time::{Duration, Instant};

use crate::provider_command::support::{
    asp_command, prepend_path, provider, temp_project_root, write_activation,
};

use super::shared::SharedBenchmarkToml;

const SCENARIO_DIR: &str = "asp_gerbil_scheme_owner_items_cold_functional_path";
const OWNER_PATH: &str = "src/model.ss";
const QUERY: &str = "dynamic-owner-item-index";

#[test]
fn asp_gerbil_scheme_projection_owner_items_lifecycle_stays_inside_scenario_gate() {
    let benchmark: SharedBenchmarkToml = parse_benchmark(SCENARIO_DIR);
    assert_eq!(benchmark.harness, "libtest");
    assert_eq!(benchmark.phase.as_deref(), Some("lifecycle"));
    assert_eq!(
        benchmark.route_source.as_deref(),
        Some("turso-evidence-graph")
    );
    assert_eq!(benchmark.max_provider_process_count, Some(0));

    let root = temp_project_root(SCENARIO_DIR);
    let bin_dir = root.join(".bin");
    let provider_count = root.join("projection-provider-count");
    let owner = root.join(OWNER_PATH);
    fs::create_dir_all(owner.parent().expect("owner parent")).expect("create source root");
    fs::write(
        root.join("gerbil.pkg"),
        "(package: scenario-gerbil-projection-owner-items)\n",
    )
    .expect("write package anchor");
    fs::write(&owner, "(def (dynamic-owner-item-index) #t)\n").expect("write source");
    write_counting_projection_provider(&bin_dir, &provider_count);
    let installed_bin_dir = root.join("home").join(".local").join("bin");
    fs::create_dir_all(&installed_bin_dir).expect("create installed provider bin dir");
    fs::copy(bin_dir.join("gslph"), installed_bin_dir.join("gslph"))
        .expect("install counting projection provider");
    write_activation(&root, &[provider("gerbil-scheme", Vec::new())]);

    let process_warmup = asp_command(&root)
        .arg("--help")
        .output()
        .expect("warm ASP process image before projection-cold timing");
    assert!(process_warmup.status.success());
    let cold_started = Instant::now();
    let cold = run_owner_search(&root, &bin_dir);
    let cold_elapsed = cold_started.elapsed();
    assert!(
        cold.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&cold.stderr)
    );
    let cold_stdout = String::from_utf8(cold.stdout).expect("cold stdout");
    assert!(
        cold_stdout.contains("state=projection-cold-required"),
        "{cold_stdout}"
    );
    assert!(
        cold_stdout.contains("providerProcessCount=0"),
        "{cold_stdout}"
    );
    assert!(
        !provider_count.exists(),
        "cold graph read must not invoke the projection provider"
    );

    let imported = asp_command(&root)
        .env("PATH", prepend_path(&bin_dir))
        .args([
            "gerbil-scheme",
            "projection",
            "import",
            "--owner",
            OWNER_PATH,
            "--workspace",
            ".",
        ])
        .output()
        .expect("import Gerbil projection");
    assert!(
        imported.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&imported.stderr)
    );
    let imported_stdout = String::from_utf8(imported.stdout).expect("import stdout");
    assert!(
        imported_stdout.contains("parserProcessCount=1"),
        "{imported_stdout}"
    );
    assert_eq!(
        fs::read_to_string(&provider_count).expect("projection provider count"),
        "1"
    );

    let warm_started = Instant::now();
    let warm = run_owner_search(&root, &bin_dir);
    let warm_elapsed = warm_started.elapsed();
    assert!(
        warm.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&warm.stderr)
    );
    let warm_stdout = String::from_utf8(warm.stdout).expect("warm stdout");
    assert!(
        warm_stdout.contains("alg=graph-turbo-owner-items"),
        "{warm_stdout}"
    );
    assert!(
        warm_stdout.contains("dynamic-owner-item-index"),
        "{warm_stdout}"
    );
    assert_eq!(
        fs::read_to_string(&provider_count).expect("warm projection provider count"),
        "1",
        "warm graph read must not invoke the projection provider"
    );

    let max_total = parse_milliseconds(&benchmark.max_total);
    assert!(
        cold_elapsed <= max_total,
        "cold={cold_elapsed:?} max={max_total:?} stdout={cold_stdout}"
    );
    assert!(
        warm_elapsed <= max_total,
        "warm={warm_elapsed:?} max={max_total:?}"
    );
    let max_stdout_bytes = benchmark.max_stdout_bytes.unwrap_or(4096) as usize;
    assert!(cold_stdout.len() <= max_stdout_bytes, "{cold_stdout}");
    assert!(warm_stdout.len() <= max_stdout_bytes, "{warm_stdout}");
    let _ = fs::remove_dir_all(root);
}

fn run_owner_search(root: &Path, bin_dir: &Path) -> std::process::Output {
    let mut command = asp_command(root);
    command
        .env("PATH", prepend_path(bin_dir))
        .args([
            "gerbil-scheme",
            "search",
            "owner",
            OWNER_PATH,
            "items",
            "--query",
            QUERY,
            "--workspace",
            ".",
            "--view",
            "seeds",
        ])
        .output()
        .expect("run Gerbil owner-items search")
}

fn write_counting_projection_provider(bin_dir: &Path, provider_count: &Path) {
    fs::create_dir_all(bin_dir).expect("create provider bin dir");
    let provider_path = bin_dir.join("gslph");
    fs::write(
        &provider_path,
        format!(
            "#!/bin/sh\ncount=0\nif [ -f '{count}' ]; then count=$(cat '{count}'); fi\ncount=$((count + 1))\nprintf '%s' \"$count\" > '{count}'\ncat <<'ASP_PROJECTION'\n{projection}\nASP_PROJECTION\n",
            count = provider_count.display(),
            projection = projection_json(),
        ),
    )
    .expect("write projection provider");
    let mut permissions = fs::metadata(&provider_path)
        .expect("provider metadata")
        .permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&provider_path, permissions).expect("chmod projection provider");
}

fn projection_json() -> &'static str {
    r#"{
      "schemaId":"agent.semantic-protocols.semantic-language-projection",
      "schemaVersion":"1",
      "protocolId":"agent.semantic-protocols.language-projection",
      "protocolVersion":"1",
      "languageId":"gerbil-scheme",
      "harness":{"harnessId":"gerbil-scheme-language-project-harness","parserAbi":"gerbil-parser-v1","selectorDialect":"gerbil-scheme"},
      "sources":[{"sourceId":"source:src/model.ss","path":"src/model.ss","sourceKind":"source"}],
      "owners":[{"ownerId":"owner:src/model.ss","sourceId":"source:src/model.ss","kind":"module","name":"model"}],
      "items":[{"itemId":"item:dynamic-owner-item-index","ownerId":"owner:src/model.ss","kind":"function","name":"dynamic-owner-item-index","selector":"gerbil-scheme://src/model.ss#item/function/dynamic-owner-item-index"}],
      "relations":[
        {"from":{"kind":"source","id":"source:src/model.ss"},"kind":"contains","to":{"kind":"owner","id":"owner:src/model.ss"}},
        {"from":{"kind":"owner","id":"owner:src/model.ss"},"kind":"contains","to":{"kind":"item","id":"item:dynamic-owner-item-index"}}
      ]
    }"#
}

fn parse_benchmark(scenario_dir: &str) -> SharedBenchmarkToml {
    let crate_root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let benchmark_path = crate_root
        .join("tests")
        .join("unit")
        .join("scenarios")
        .join(scenario_dir)
        .join("benchmark.toml");
    let source = fs::read_to_string(&benchmark_path).expect("read scenario benchmark");
    toml::from_str(&source).expect("decode scenario benchmark")
}

fn parse_milliseconds(value: &str) -> Duration {
    let milliseconds = value
        .strip_suffix("ms")
        .expect("benchmark duration must use milliseconds")
        .parse::<u64>()
        .expect("benchmark milliseconds");
    Duration::from_millis(milliseconds)
}
