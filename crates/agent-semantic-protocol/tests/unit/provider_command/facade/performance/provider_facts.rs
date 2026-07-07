use super::{
    assert_trace_elapsed_under_gate_ms, make_executable, prepend_path, provider, write_activation,
};
use crate::provider_command::support::{asp_command, temp_project_root};

#[derive(serde::Deserialize)]
struct ProviderFactsTimeoutBenchmark {
    max_total: String,
    route_source: Option<String>,
    fallback_reason: Option<String>,
    max_provider_process_count: Option<u32>,
}

#[test]
fn provider_facts_timeout_stays_inside_performance_gate() {
    let benchmark = provider_facts_timeout_benchmark();
    assert_eq!(
        benchmark.route_source.as_deref(),
        Some("provider-facts-timeout-fallback"),
        "provider facts timeout benchmark must declare route_source"
    );
    assert_eq!(
        benchmark.max_provider_process_count,
        Some(1),
        "provider facts timeout benchmark must declare one bounded provider process"
    );
    assert_eq!(
        benchmark.fallback_reason.as_deref(),
        Some("provider-facts-timeout"),
        "provider facts timeout benchmark must declare fallback_reason"
    );
    let max_total_ms = provider_facts_timeout_max_total_ms(&benchmark);

    let root = temp_project_root("provider-facts-timeout-gate");
    let bin_dir = crate::provider_command::support::home_local_bin(&root);
    let cache_home = root.join(".cache");
    std::fs::create_dir_all(root.join("src")).expect("create src");
    std::fs::create_dir_all(&bin_dir).expect("create bin dir");
    for index in 0..16 {
        std::fs::write(
            root.join(format!("src/queue_timeout_{index}.rs")),
            format!("pub fn queue_timeout_{index}() {{}}\n"),
        )
        .expect("write candidate");
    }
    std::fs::write(
        root.join("Cargo.toml"),
        "[package]\nname = \"provider-facts-timeout\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
    )
    .expect("write Cargo.toml");
    let provider_path = bin_dir.join("rs-harness");
    std::fs::write(
        &provider_path,
        "#!/bin/sh\nsleep 5\nprintf '{\"nodes\":[],\"edges\":[]}\\n'\n",
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
        .args([
            "rust",
            "search",
            "pipe",
            "queue timeout",
            "--view",
            "seeds",
            ".",
        ])
        .output()
        .expect("run asp search pipe");

    assert!(
        output.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("stdout");
    assert!(
        stdout.contains("providerFacts:used[") || stdout.contains("providerFacts:skipped["),
        "{stdout}"
    );
    assert_trace_elapsed_under_gate_ms(&["rust", "search", "pipe"], &stdout, max_total_ms);
    let _ = std::fs::remove_dir_all(root);
}

fn provider_facts_timeout_benchmark() -> ProviderFactsTimeoutBenchmark {
    let crate_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let scenario_root = crate_root
        .join("tests")
        .join("unit")
        .join("scenarios")
        .join("asp_provider_facts_timeout_fallback_path");
    toml::from_str(
        &std::fs::read_to_string(scenario_root.join("benchmark.toml"))
            .expect("read provider facts timeout benchmark"),
    )
    .expect("parse provider facts timeout benchmark")
}

fn provider_facts_timeout_max_total_ms(benchmark: &ProviderFactsTimeoutBenchmark) -> u64 {
    benchmark
        .max_total
        .strip_suffix("ms")
        .expect("provider facts timeout max_total must use ms")
        .parse::<u64>()
        .expect("provider facts timeout max_total ms")
}
