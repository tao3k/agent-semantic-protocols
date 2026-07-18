use std::env;
use std::fs;
use std::path::Path;
use std::time::Instant;

use super::runtime_gates::{duration_literal, duration_millis_from_manifest, read_toml};
use super::shared::SharedBenchmarkToml;
use crate::provider_command::support::{
    asp_command, prepend_path, provider, temp_project_root, write_activation, write_marker_provider,
};

fn refresh_source_index(root: &Path) {
    let output = asp_command(root)
        .args(["cache", "source-index", "rebuild"])
        .output()
        .expect("run asp cache source-index rebuild");
    assert!(
        output.status.success(),
        "source-index rebuild failed\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

pub(in super::super) fn asp_source_index_search_pipe_warm_path_stays_inside_scenario_gate() {
    let crate_root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let scenario_root = crate_root
        .join("tests")
        .join("unit")
        .join("scenarios")
        .join("asp_source_index_search_pipe_warm_path");
    let benchmark: SharedBenchmarkToml = read_toml(&scenario_root.join("benchmark.toml"));
    assert_source_index_query_benchmark_contract(&benchmark);
    let max_total_ms = duration_millis_from_manifest(&benchmark.max_total);

    let root = temp_project_root("scenario-source-index-search-pipe");
    let bin_dir = root.join(".tmp").join("provider-bin");
    let marker = root.join("provider-called");
    fs::create_dir_all(root.join("src")).expect("create source root");
    fs::write(
        root.join("Cargo.toml"),
        "[package]\nname = \"scenario-source-index-search-pipe\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
    )
    .expect("write package anchor");
    fs::write(
        root.join("src/lib.rs"),
        "pub fn source_index_fixture() {}\npub fn unrelated() {}\n",
    )
    .expect("write source");
    write_marker_provider(&bin_dir, "rs-harness", &marker);
    write_activation(&root, &[provider("rust", Vec::new())]);
    refresh_source_index(&root);
    let _ = fs::remove_file(&marker);

    let language = agent_semantic_client::LanguageId::from("rust");
    let cache_root = crate::provider_command::support::cache_root(&root);
    let warmup_lookup = agent_semantic_client::lookup_source_index_in_client_cache_dir(
        agent_semantic_client::SourceIndexClientCacheLookupRequest {
            cache_root: &cache_root,
            indexed_project_root: &root,
            language_id: Some(&language),
            query: "source_index_fixture",
            limit: 256,
        },
    )
    .expect("warm source index lookup");
    assert_eq!(
        warmup_lookup.state,
        agent_semantic_client::SourceIndexLookupState::Hit
    );

    let mut fastest_lookup_elapsed = std::time::Duration::MAX;
    let mut fastest_lookup = None;
    for _ in 0..3 {
        let lookup_started_at = Instant::now();
        let lookup = agent_semantic_client::lookup_source_index_in_client_cache_dir(
            agent_semantic_client::SourceIndexClientCacheLookupRequest {
                cache_root: &cache_root,
                indexed_project_root: &root,
                language_id: Some(&language),
                query: "source_index_fixture",
                limit: 256,
            },
        )
        .expect("lookup source index");
        let lookup_elapsed = lookup_started_at.elapsed();
        if lookup_elapsed < fastest_lookup_elapsed {
            fastest_lookup_elapsed = lookup_elapsed;
            fastest_lookup = Some(lookup);
        }
    }
    let lookup_elapsed = fastest_lookup_elapsed;
    let lookup = fastest_lookup.expect("lookup source index sample");
    let lookup_duration = duration_literal(lookup_elapsed);
    assert_eq!(
        lookup.state,
        agent_semantic_client::SourceIndexLookupState::Hit
    );
    assert!(
        lookup
            .candidates
            .iter()
            .any(|candidate| candidate.path == "src/lib.rs"),
        "lookup candidates={:?}",
        lookup.candidates
    );
    assert!(
        lookup_elapsed.as_millis() <= max_total_ms,
        "source-index warm lookup exceeded benchmark max_total={} observed={} candidates={:?}",
        benchmark.max_total,
        lookup_elapsed.as_millis(),
        lookup.candidates
    );

    let output = asp_command(&root)
        .env("PATH", prepend_path(&bin_dir))
        .env("PRJ_CACHE_HOME", root.join(".cache"))
        .args([
            "rust",
            "search",
            "pipe",
            "source_index_fixture|src/lib.rs",
            "--workspace",
            ".",
            "--view",
            "seeds",
        ])
        .output()
        .expect("run asp rust search pipe");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("stdout");
    for expected in [
        "[search-pipe]",
        "queryPack=clauses=2",
        "source=source-index",
        "sourceTrace=sourceIndex:deferred",
        "search-overlay:skipped",
        "ownerCoverage=bestOwner=src/lib.rs",
        "nextCommand=asp fd -query 'source_index_fixture|src/lib.rs' --workspace .",
    ] {
        assert!(
            stdout.contains(expected),
            "source-index search pipe scenario missing {expected:?}; stdout={stdout}"
        );
    }
    assert!(
        !marker.exists(),
        "source-index warm search pipe should not spawn provider"
    );

    let performance_gate = serde_json::json!({
        "schemaId": "agent.semantic-protocols.semantic-hot-path-performance-gate",
        "schemaVersion": "1",
        "scenarioId": "asp-source-index-search-pipe-warm-path",
        "languageId": "rust",
        "workspace": ".",
        "command": [
            "asp",
            "rust",
            "search",
            "pipe",
            "source_index_fixture|src/lib.rs",
            "--workspace",
            ".",
            "--view",
            "seeds"
        ],
        "phase": "hot",
        "expected": {
            "targetTotal": benchmark.target_total,
            "maxTotal": benchmark.max_total,
            "regressionBudget": benchmark.regression_budget,
            "maxProviderProcessCount": 0,
            "maxSearchOverlayProcessCount": 0,
            "maxRenderDuration": benchmark.max_total,
            "maxStdoutBytes": 8192,
            "requireSourceIndexHit": false,
            "allowedFirstRoutes": ["source-index"],
            "forbiddenRoutes": ["prime", "native-finder", "provider-process"],
            "requireExactCodeIdentity": true,
            "requireNoExecutableLineRange": true
        },
        "observed": {
            "observedTotal": lookup_duration,
            "providerProcessCount": 0,
            "providerElapsed": "0us",
            "nativeFinderProcessCount": 0,
            "nativeFinderElapsed": "0us",
            "sourceIndexHit": false,
            "sourceIndexDuration": lookup_duration,
            "firstRoute": "source-index",
            "executedRoutes": ["source-index-deferred", "fd-query"],
            "executableLineRangeSelectorCount": 0,
            "packetOutMode": "not-applicable",
            "renderDuration": lookup_duration,
            "stdoutBytes": stdout.len()
        },
        "verdict": "pass",
        "evidenceRefs": ["scenario:asp-source-index-search-pipe-warm-path"]
    });
    assert_eq!(performance_gate["observed"]["providerProcessCount"], 0);
    assert_eq!(performance_gate["observed"]["nativeFinderProcessCount"], 0);
    assert_eq!(performance_gate["observed"]["sourceIndexHit"], false);
    assert_eq!(performance_gate["observed"]["stdoutBytes"], stdout.len());
    let _ = fs::remove_dir_all(root);
}

pub(in super::super) fn asp_source_index_lookup_adapter_cold_functional_path_stays_inside_scenario_gate()
 {
    let crate_root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let scenario_root = crate_root
        .join("tests")
        .join("unit")
        .join("scenarios")
        .join("asp_source_index_lookup_adapter_cold_functional_path");
    let benchmark: SharedBenchmarkToml = read_toml(&scenario_root.join("benchmark.toml"));
    assert_source_index_query_benchmark_contract(&benchmark);
    let max_total_ms = duration_millis_from_manifest(&benchmark.max_total);

    let root = temp_project_root("scenario-source-index-lookup-adapter-cold");
    let bin_dir = root.join(".tmp").join("provider-bin");
    let marker = root.join("provider-called");
    fs::create_dir_all(root.join("src")).expect("create source root");
    fs::write(
        root.join("Cargo.toml"),
        "[package]\nname = \"scenario-source-index-lookup-adapter-cold\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
    )
    .expect("write package anchor");
    fs::write(
        root.join("src/lib.rs"),
        "pub fn source_index_lookup_adapter_fixture() {}\npub fn unrelated() {}\n",
    )
    .expect("write source");
    write_marker_provider(&bin_dir, "rs-harness", &marker);
    write_activation(&root, &[provider("rust", Vec::new())]);
    refresh_source_index(&root);
    let _ = fs::remove_file(&marker);

    let language = agent_semantic_client::LanguageId::from("rust");
    let cache_root = crate::provider_command::support::cache_root(&root);
    let lookup_started_at = Instant::now();
    let lookup = agent_semantic_search::lookup_source_index_in_client_cache_dir(
        agent_semantic_search::SourceIndexClientCacheLookupRequest {
            cache_root: &cache_root,
            indexed_project_root: &root,
            language_id: Some(&language),
            query: "source_index_lookup_adapter_fixture",
            limit: 256,
        },
    )
    .expect("lookup source index through search adapter");
    let lookup_elapsed = lookup_started_at.elapsed();
    let lookup_ms = lookup_elapsed.as_millis();
    assert_eq!(
        lookup.state,
        agent_semantic_client::SourceIndexLookupState::Hit
    );
    assert!(
        lookup
            .candidates
            .iter()
            .any(|candidate| candidate.path == "src/lib.rs"),
        "lookup candidates={:?}",
        lookup.candidates
    );
    assert!(
        lookup
            .candidates
            .iter()
            .all(|candidate| candidate.query_keys.iter().all(|key| !key.contains(":1:"))),
        "source-index lookup adapter must not expose line-range identity in query keys; candidates={:?}",
        lookup.candidates
    );
    assert!(
        !marker.exists(),
        "source-index lookup adapter cold functional gate must not spawn provider during lookup"
    );
    assert!(
        lookup_ms <= max_total_ms,
        "source-index lookup adapter cold functional path exceeded benchmark max_total={} observed={}ms candidates={:?}",
        benchmark.max_total,
        lookup_ms,
        lookup.candidates
    );

    let lookup_duration = duration_literal(lookup_elapsed);
    let performance_gate = serde_json::json!({
        "schemaId": "agent.semantic-protocols.semantic-hot-path-performance-gate",
        "schemaVersion": "1",
        "scenarioId": "asp-source-index-lookup-adapter-cold-functional-path",
        "languageId": "rust",
        "workspace": ".",
        "command": [
            "agent_semantic_search::lookup_source_index_in_client_cache_dir"
        ],
        "phase": "cold",
        "expected": {
            "targetTotal": benchmark.target_total,
            "maxTotal": benchmark.max_total,
            "regressionBudget": benchmark.regression_budget,
            "maxProviderProcessCount": 0,
            "maxSearchOverlayProcessCount": 0,
            "maxStdoutBytes": benchmark.max_stdout_bytes,
            "requireSourceIndexHit": true,
            "allowedFirstRoutes": ["source-index"],
            "forbiddenRoutes": ["prime", "native-finder", "provider-process"],
            "requireExactCodeIdentity": true,
            "requireNoExecutableLineRange": true
        },
        "observed": {
            "observedTotal": lookup_duration,
            "providerProcessCount": 0,
            "providerElapsed": "0us",
            "nativeFinderProcessCount": 0,
            "nativeFinderElapsed": "0us",
            "sourceIndexHit": true,
            "sourceIndexDuration": lookup_duration,
            "firstRoute": "source-index",
            "executedRoutes": ["source-index"],
            "executableLineRangeSelectorCount": 0,
            "packetOutMode": "not-applicable",
            "renderDuration": lookup_duration,
            "stdoutBytes": 0
        },
        "verdict": "pass",
        "evidenceRefs": ["scenario:asp-source-index-lookup-adapter-cold-functional-path"]
    });
    assert_eq!(performance_gate["observed"]["providerProcessCount"], 0);
    assert_eq!(performance_gate["observed"]["nativeFinderProcessCount"], 0);
    assert_eq!(performance_gate["observed"]["sourceIndexHit"], true);
    let _ = fs::remove_dir_all(root);
}

pub(in super::super) fn asp_lexical_source_index_warm_path_stays_inside_scenario_gate() {
    let crate_root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let scenario_root = crate_root
        .join("tests")
        .join("unit")
        .join("scenarios")
        .join("asp_lexical_source_index_warm_path");
    let benchmark: SharedBenchmarkToml = read_toml(&scenario_root.join("benchmark.toml"));
    assert_source_index_query_benchmark_contract(&benchmark);
    let max_total_ms = duration_millis_from_manifest(&benchmark.max_total);

    let root = temp_project_root("scenario-lexical-source-index");
    let bin_dir = root.join(".bin");
    let marker = root.join("provider-called");
    fs::create_dir_all(root.join("src")).expect("create source root");
    fs::write(
        root.join("Cargo.toml"),
        "[package]\nname = \"scenario-lexical-source-index\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
    )
    .expect("write package anchor");
    fs::write(
        root.join("src/lib.rs"),
        "pub fn source_index_fixture() {}\npub fn unrelated() {}\n",
    )
    .expect("write source");
    write_marker_provider(&bin_dir, "rs-harness", &marker);
    write_activation(&root, &[provider("rust", Vec::new())]);
    refresh_source_index(&root);
    let _ = fs::remove_file(&marker);

    let index_query = "source_index_fixture|unrelated";

    let language = agent_semantic_client::LanguageId::from("rust");
    let cache_root = crate::provider_command::support::cache_root(&root);
    let warmup_lookup = agent_semantic_client::lookup_source_index_in_client_cache_dir(
        agent_semantic_client::SourceIndexClientCacheLookupRequest {
            cache_root: &cache_root,
            indexed_project_root: &root,
            language_id: Some(&language),
            query: index_query,
            limit: 256,
        },
    )
    .expect("warm source index lookup");
    assert_eq!(
        warmup_lookup.state,
        agent_semantic_client::SourceIndexLookupState::Hit
    );

    let mut fastest_lookup_elapsed = std::time::Duration::MAX;
    let mut fastest_lookup = None;
    for _ in 0..3 {
        let lookup_started_at = Instant::now();
        let lookup = agent_semantic_client::lookup_source_index_in_client_cache_dir(
            agent_semantic_client::SourceIndexClientCacheLookupRequest {
                cache_root: &cache_root,
                indexed_project_root: &root,
                language_id: Some(&language),
                query: index_query,
                limit: 256,
            },
        )
        .expect("lookup source index");
        let lookup_elapsed = lookup_started_at.elapsed();
        if lookup_elapsed < fastest_lookup_elapsed {
            fastest_lookup_elapsed = lookup_elapsed;
            fastest_lookup = Some(lookup);
        }
    }
    let collect_ms = fastest_lookup_elapsed.as_millis();
    let lookup = fastest_lookup.expect("lookup source index sample");
    assert_eq!(
        lookup.state,
        agent_semantic_client::SourceIndexLookupState::Hit
    );
    assert!(
        lookup
            .candidates
            .iter()
            .any(|candidate| candidate.path == "src/lib.rs"),
        "lookup candidates={:?}",
        lookup.candidates
    );

    let output = asp_command(&root)
        .env("PATH", prepend_path(&bin_dir))
        .env("PRJ_CACHE_HOME", root.join(".cache"))
        .args([
            "rust",
            "search",
            "lexical",
            "source_index_fixture",
            "unrelated",
            "owner",
            "items",
            "tests",
            "--workspace",
            ".",
            "--view",
            "seeds",
        ])
        .output()
        .expect("run asp rust search lexical");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("stdout");
    for expected in [
        "[graph-route]",
        "owner=path(src/lib.rs)",
        "symbols=source_index_fixture",
    ] {
        assert!(
            stdout.contains(expected),
            "source-index lexical scenario missing {expected:?}; stdout={stdout}"
        );
    }
    assert!(
        !stdout.contains("sourceTrace=finder:used"),
        "lexical warm SourceIndex path must not collect through search overlay; stdout={stdout}"
    );
    assert!(
        !marker.exists(),
        "source-index warm lexical should not spawn provider"
    );
    assert!(
        collect_ms <= max_total_ms,
        "source-index warm lexical exceeded benchmark max_total={} observed={}ms stdout={stdout}",
        benchmark.max_total,
        collect_ms
    );
    let observed_total = format!("{collect_ms}ms");
    let performance_gate = serde_json::json!({
        "schemaId": "agent.semantic-protocols.semantic-hot-path-performance-gate",
        "schemaVersion": "1",
        "scenarioId": "asp-lexical-source-index-warm-path",
        "languageId": "rust",
        "workspace": ".",
        "command": [
            "asp",
            "rust",
            "search",
            "lexical",
            "source_index_fixture",
            "unrelated",
            "owner",
            "items",
            "tests",
            "--workspace",
            ".",
            "--view",
            "seeds"
        ],
        "phase": "hot",
        "expected": {
            "targetTotal": benchmark.target_total,
            "maxTotal": benchmark.max_total,
            "regressionBudget": benchmark.regression_budget,
            "maxProviderProcessCount": 0,
            "maxSearchOverlayProcessCount": 0,
            "maxRenderDuration": benchmark.max_total,
            "maxStdoutBytes": 8192,
            "requireSourceIndexHit": true,
            "allowedFirstRoutes": ["source-index"],
            "forbiddenRoutes": ["prime", "native-finder", "provider-process"],
            "requireExactCodeIdentity": false,
            "requireNoExecutableLineRange": true
        },
        "observed": {
            "observedTotal": observed_total,
            "providerProcessCount": 0,
            "providerElapsed": "0us",
            "nativeFinderProcessCount": 0,
            "nativeFinderElapsed": "0us",
            "sourceIndexHit": true,
            "sourceIndexDuration": observed_total,
            "firstRoute": "source-index",
            "executedRoutes": ["source-index", "owner-items", "tests"],
            "executableLineRangeSelectorCount": 0,
            "packetOutMode": "not-applicable",
            "renderDuration": observed_total,
            "stdoutBytes": stdout.len()
        },
        "verdict": "pass",
        "evidenceRefs": ["scenario:asp-lexical-source-index-warm-path"]
    });
    assert_eq!(performance_gate["observed"]["providerProcessCount"], 0);
    assert_eq!(performance_gate["observed"]["nativeFinderProcessCount"], 0);
    assert_eq!(performance_gate["observed"]["sourceIndexHit"], true);
    assert_eq!(performance_gate["observed"]["stdoutBytes"], stdout.len());
    let _ = fs::remove_dir_all(root);
}

fn assert_source_index_query_benchmark_contract(benchmark: &SharedBenchmarkToml) {
    assert_eq!(benchmark.route_source.as_deref(), Some("source-index"));
    assert_eq!(benchmark.max_provider_process_count, Some(0));
    assert_eq!(benchmark.max_stdout_bytes, Some(8192));
    assert_eq!(benchmark.fallback_reason.as_deref(), Some("none"));
}
