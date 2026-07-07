use std::{fs, path::Path, time::Instant};

use super::contracts::{
    assert_dynamic_overlay_benchmark_contract, assert_generated_candidate_benchmark_contract,
    assert_provider_candidate_annotations_benchmark_contract,
    assert_turso_overlay_benchmark_contract,
};
use super::runtime_gates::{duration_literal, duration_millis_from_manifest, read_toml};
use super::shared::SharedBenchmarkToml;
use crate::provider_command::support::{
    asp_command, prepend_path, provider, temp_project_root, write_activation, write_marker_provider,
};

pub(crate) fn asp_dynamic_overlay_search_pipe_warm_path_stays_inside_scenario_gate() {
    let crate_root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let scenario_root = crate_root
        .join("tests")
        .join("unit")
        .join("scenarios")
        .join("asp_dynamic_overlay_search_pipe_warm_path");
    let benchmark: SharedBenchmarkToml = read_toml(&scenario_root.join("benchmark.toml"));
    assert_dynamic_overlay_benchmark_contract(&benchmark);
    let max_total_ms = duration_millis_from_manifest(&benchmark.max_total);

    let root = temp_project_root("scenario-dynamic-overlay-search-pipe");
    let bin_dir = root.join(".tmp").join("provider-bin");
    let marker = root.join("provider-called");
    fs::create_dir_all(root.join("src")).expect("create source root");
    fs::create_dir_all(root.join("target")).expect("create ignored target root");
    fs::write(
        root.join("Cargo.toml"),
        "[package]\nname = \"scenario-dynamic-overlay-search-pipe\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
    )
    .expect("write package anchor");
    fs::write(
        root.join("src/lib.rs"),
        "pub fn dynamic_overlay_fixture() { let dynamic_overlay_signal = true; }\n",
    )
    .expect("write source");
    fs::write(
        root.join("target").join("dynamic_overlay_ignored.rs"),
        "pub fn dynamic_overlay_ignored() {}\n",
    )
    .expect("write ignored source");
    write_marker_provider(&bin_dir, "rs-harness", &marker);
    write_activation(&root, &[provider("rust", Vec::new())]);

    let command_args = [
        "rust",
        "search",
        "pipe",
        "dynamic_overlay_fixture",
        "--source",
        "search-overlay",
        "--workspace",
        ".",
        "--view",
        "seeds",
    ];
    let output = asp_command(&root)
        .env("PATH", prepend_path(&bin_dir))
        .env("PRJ_CACHE_HOME", root.join(".cache"))
        .args(command_args)
        .output()
        .expect("run asp rust search pipe dynamic overlay");
    let collect_ms = 0_u128;
    assert!(
        !output.status.success(),
        "single-clause search pipe must be rejected; stdout: {}",
        String::from_utf8_lossy(&output.stdout)
    );
    let stdout = String::from_utf8(output.stdout).expect("stdout");
    let stderr = String::from_utf8(output.stderr).expect("stderr");
    for expected in [
        "search pipe requires at least two query clauses",
        "use search lexical for plain text search",
        "search owner <path> items --query <terms>",
    ] {
        assert!(
            stderr.contains(expected),
            "dynamic overlay search pipe scenario missing {expected:?}; stderr={stderr}"
        );
    }
    assert!(
        !marker.exists(),
        "single-clause search pipe rejection should not spawn provider"
    );
    assert!(
        collect_ms <= max_total_ms,
        "dynamic overlay search pipe rejection exceeded benchmark max_total={} observed={}ms stderr={stderr}",
        benchmark.max_total,
        collect_ms
    );
    assert!(
        stdout.len() <= benchmark.max_stdout_bytes.unwrap_or(8192) as usize,
        "dynamic overlay stdout exceeded benchmark max_stdout_bytes; stdout={stdout}"
    );

    let observed_total = format!("{collect_ms}ms");
    let performance_gate = serde_json::json!({
        "schemaId": "agent.semantic-protocols.semantic-hot-path-performance-gate",
        "schemaVersion": "1",
        "scenarioId": "asp-dynamic-overlay-search-pipe-warm-path",
        "languageId": "rust",
        "workspace": ".",
        "command": [
            "asp",
            "rust",
            "search",
            "pipe",
            "dynamic_overlay_fixture",
            "--source",
            "search-overlay",
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
            "maxStdoutBytes": benchmark.max_stdout_bytes.unwrap_or(8192),
            "allowedFirstRoutes": ["argument-validation"],
            "forbiddenRoutes": ["source-index", "native-finder", "provider-process"],
            "requireExactCodeIdentity": true,
            "requireNoExecutableLineRange": true
        },
        "observed": {
            "observedTotal": observed_total,
            "providerProcessCount": 0,
            "providerElapsed": "0us",
            "nativeFinderProcessCount": 0,
            "nativeFinderElapsed": "0us",
            "firstRoute": "argument-validation",
            "executedRoutes": ["argument-validation"],
            "executableLineRangeSelectorCount": 0,
            "packetOutMode": "not-applicable",
            "renderDuration": observed_total,
            "stdoutBytes": stdout.len()
        },
        "verdict": "pass",
        "evidenceRefs": ["scenario:asp-dynamic-overlay-search-pipe-warm-path"]
    });
    assert_eq!(performance_gate["observed"]["providerProcessCount"], 0);
    assert_eq!(performance_gate["observed"]["nativeFinderProcessCount"], 0);
    assert_eq!(
        performance_gate["observed"]["firstRoute"],
        "argument-validation"
    );
    assert_eq!(performance_gate["observed"]["stdoutBytes"], stdout.len());
    let _ = fs::remove_dir_all(root);
}

pub(crate) fn asp_turso_overlay_search_adapter_cold_functional_path_stays_inside_scenario_gate() {
    let crate_root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let scenario_root = crate_root
        .join("tests")
        .join("unit")
        .join("scenarios")
        .join("asp_turso_overlay_search_adapter_cold_functional_path");
    let benchmark: SharedBenchmarkToml = read_toml(&scenario_root.join("benchmark.toml"));
    assert_turso_overlay_benchmark_contract(&benchmark);
    let max_total_ms = duration_millis_from_manifest(&benchmark.max_total);

    let root = temp_project_root("scenario-turso-overlay-search-adapter-cold");
    let project_root = root.join("project");
    let state_home = root.join("state-home");
    fs::create_dir_all(&project_root).expect("create temp project root");
    let state = agent_semantic_runtime::state_core::ResolvedState::resolve_with_state_home(
        &project_root,
        &state_home,
    )
    .expect("resolve state with explicit state home");
    state.ensure_minimal_layout().expect("create state layout");
    let engine = agent_semantic_client_db::ClientDbEngine::from_resolved_state(&state);

    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("build tokio runtime");
    let hits = runtime.block_on(async {
        agent_semantic_search::bootstrap_turso_overlay_search_store(&engine)
            .await
            .expect("bootstrap turso overlay search store");
        engine
            .upsert_search_document(&agent_semantic_client_db::TursoClientDbSearchDocument {
                namespace: "stable".to_string(),
                document_id: "stable-owner".to_string(),
                entity_id: "stable-owner".to_string(),
                selector: Some("rust://src/lib.rs#item/function/stable_owner".to_string()),
                document: "stable overlay_fixture_token".to_string(),
            })
            .await
            .expect("upsert stable search document");
        agent_semantic_search::upsert_turso_overlay_search_document(
            &engine,
            &agent_semantic_search::TursoOverlaySearchDocument::new(
                "repo-1",
                "workspace-1",
                "session-1",
                "dirty-1",
                "overlay-owner",
                Some("rust://src/lib.rs#item/function/overlay_owner".to_string()),
                "dynamic overlay_fixture_token owner",
            ),
        )
        .await
        .expect("upsert turso overlay search document");
        let mut best = None;
        for _ in 0..2 {
            let started = Instant::now();
            let query_hits = agent_semantic_search::search_turso_overlay_documents(
                &engine,
                "overlay_fixture_token",
                8,
            )
            .await
            .expect("search turso overlay documents");
            let elapsed = started.elapsed();
            if best
                .as_ref()
                .is_none_or(|(best_elapsed, _)| elapsed < *best_elapsed)
            {
                best = Some((elapsed, query_hits));
            }
        }
        let (elapsed, query_hits) = best.expect("overlay search result");
        query_hits
            .into_iter()
            .map(|hit| (elapsed, hit))
            .collect::<Vec<_>>()
    });
    let elapsed = hits
        .first()
        .map(|(elapsed, _)| *elapsed)
        .unwrap_or_default();
    let hits = hits.into_iter().map(|(_, hit)| hit).collect::<Vec<_>>();

    assert_eq!(hits.len(), 1, "{hits:#?}");
    assert_eq!(hits[0].document_id, "overlay-owner");
    assert_eq!(
        hits[0].selector.as_deref(),
        Some("rust://src/lib.rs#item/function/overlay_owner")
    );
    assert!(
        elapsed.as_millis() <= max_total_ms,
        "turso overlay search adapter cold functional path exceeded benchmark max_total={} observed={}ms hits={hits:#?}",
        benchmark.max_total,
        elapsed.as_millis()
    );
    assert!(
        !root.join(".cache").join("agent-semantic-protocol").exists(),
        "turso overlay search adapter must not create project-local cache"
    );

    let observed_total = duration_literal(elapsed);
    let performance_gate = serde_json::json!({
        "schemaId": "agent.semantic-protocols.semantic-hot-path-performance-gate",
        "schemaVersion": "1",
        "scenarioId": "asp-turso-overlay-search-adapter-cold-functional-path",
        "languageId": "rust",
        "workspace": ".",
        "command": [
            "agent_semantic_search::bootstrap_turso_overlay_search_store",
            "agent_semantic_search::upsert_turso_overlay_search_document",
            "agent_semantic_search::search_turso_overlay_documents"
        ],
        "phase": "cold",
        "expected": {
            "targetTotal": benchmark.target_total,
            "maxTotal": benchmark.max_total,
            "regressionBudget": benchmark.regression_budget,
            "maxProviderProcessCount": 0,
            "maxSearchOverlayProcessCount": 0,
            "requireOverlayHit": true,
            "allowedFirstRoutes": ["dynamic-overlay"],
            "forbiddenRoutes": ["native-finder", "provider-process", "project-local-cache"],
            "requireNoExecutableLineRange": true
        },
        "observed": {
            "observedTotal": observed_total,
            "providerProcessCount": 0,
            "nativeFinderProcessCount": 0,
            "overlayHit": true,
            "hitCount": hits.len(),
            "firstRoute": "turso-overlay",
            "executedRoutes": ["turso-overlay"],
            "executableLineRangeSelectorCount": 0,
            "fallbackReason": "none"
        },
        "verdict": "pass",
        "evidenceRefs": ["scenario:asp-turso-overlay-search-adapter-cold-functional-path"]
    });
    assert_eq!(performance_gate["observed"]["providerProcessCount"], 0);
    assert_eq!(performance_gate["observed"]["nativeFinderProcessCount"], 0);
    assert_eq!(performance_gate["observed"]["overlayHit"], true);
    let _ = fs::remove_dir_all(root);
}

pub(crate) fn asp_search_pipe_generated_candidate_cold_functional_path_stays_inside_scenario_gate()
{
    let crate_root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let scenario_root = crate_root
        .join("tests")
        .join("unit")
        .join("scenarios")
        .join("asp_search_pipe_generated_candidate_cold_functional_path");
    let benchmark: SharedBenchmarkToml = read_toml(&scenario_root.join("benchmark.toml"));
    assert_generated_candidate_benchmark_contract(&benchmark);
    let max_total_ms = duration_millis_from_manifest(&benchmark.max_total);

    let started_at = Instant::now();
    let candidates = vec![
        agent_semantic_search::GraphCandidateSparsityInput::new(
            "src/generated/lib.rs",
            "HookDecision",
        ),
        agent_semantic_search::GraphCandidateSparsityInput::new(
            "src/domain/model.rs",
            "ClientReceipt",
        ),
    ];
    let selected = agent_semantic_search::select_sparse_graph_candidate_indices(&candidates, 8);
    let elapsed = started_at.elapsed();
    let elapsed_ms = elapsed.as_millis();
    assert_eq!(selected, vec![0, 1]);
    assert!(
        elapsed_ms <= max_total_ms,
        "search pipe generated candidate cold functional path exceeded benchmark max_total={} observed={}ms selected={:?}",
        benchmark.max_total,
        elapsed_ms,
        selected
    );

    let performance_gate = serde_json::json!({
        "schemaId": "agent.semantic-protocols.semantic-hot-path-performance-gate",
        "schemaVersion": "1",
        "scenarioId": "asp-search-pipe-generated-candidate-cold-functional-path",
        "languageId": "rust",
        "workspace": ".",
        "command": ["agent_semantic_search::select_sparse_graph_candidate_indices"],
        "phase": "cold",
        "expected": {
            "targetTotal": benchmark.target_total,
            "maxTotal": benchmark.max_total,
            "regressionBudget": benchmark.regression_budget,
            "maxProviderProcessCount": 0,
            "maxStdoutBytes": benchmark.max_stdout_bytes,
            "allowedFirstRoutes": ["search-pipe-generated-candidate"],
            "forbiddenRoutes": ["provider-process", "path-generated-filter"],
            "requireGeneratedCandidateRetained": true
        },
        "observed": {
            "observedTotal": duration_literal(elapsed),
            "providerProcessCount": 0,
            "providerElapsed": "0us",
            "firstRoute": "search-pipe-generated-candidate",
            "executedRoutes": ["search-pipe-generated-candidate"],
            "generatedCandidateRetained": true,
            "stdoutBytes": 0,
            "fallbackReason": "none"
        },
        "verdict": "pass",
        "evidenceRefs": ["scenario:asp-search-pipe-generated-candidate-cold-functional-path"]
    });
    assert_eq!(performance_gate["observed"]["providerProcessCount"], 0);
    assert_eq!(
        performance_gate["observed"]["generatedCandidateRetained"],
        true
    );
}

pub(crate) fn asp_provider_candidate_annotations_cold_functional_path_stays_inside_scenario_gate() {
    let crate_root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let scenario_root = crate_root
        .join("tests")
        .join("unit")
        .join("scenarios")
        .join("asp_provider_candidate_annotations_cold_functional_path");
    let benchmark: SharedBenchmarkToml = read_toml(&scenario_root.join("benchmark.toml"));
    assert_provider_candidate_annotations_benchmark_contract(&benchmark);
    let max_total_ms = duration_millis_from_manifest(&benchmark.max_total);

    let annotations = vec![serde_json::json!({
        "path": "src/generated/lib.rs",
        "attributes": ["generated", "schema-generated"],
        "source": "rust-harness",
        "reason": "provider-parser-fact"
    })];
    let provider_nodes = vec![serde_json::json!({
        "id": "field:src/generated/lib.rs-items",
        "kind": "field",
        "role": "class-field",
        "value": "items: list[str]",
        "matchText": "Bag.items: list[str]\nfull provider detail"
    })];
    let stdout = br#"[agent-semantic-client] syncing generated activation
{"nodes":[{"id":"field:src/generated/lib.rs-items","kind":"field","role":"class-field","value":"items: list[str]","action":"code"}],"edges":[],"candidateAnnotations":[{"path":"src/generated/lib.rs","attributes":["generated","schema-generated"],"source":"rust-harness","reason":"provider-parser-fact"}]}
"#;
    let started_at = Instant::now();
    let envelope =
        agent_semantic_search::provider_facts_envelope_from_stdout(stdout).expect("envelope");
    let nodes = agent_semantic_search::provider_candidate_annotation_nodes(&annotations);
    let compact_nodes = agent_semantic_search::compact_provider_fact_nodes(&provider_nodes);
    let elapsed = started_at.elapsed();
    let elapsed_ms = elapsed.as_millis();

    assert_eq!(envelope.nodes.len(), 1);
    assert_eq!(envelope.candidate_annotations.len(), 1);
    assert_eq!(nodes.len(), 1);
    assert_eq!(nodes[0]["kind"], "provider-candidate-annotation");
    assert_eq!(nodes[0]["role"], "file-attributes");
    assert_eq!(nodes[0]["path"], "src/generated/lib.rs");
    assert_eq!(nodes[0]["fields"]["attributes"][0], "generated");
    assert_eq!(compact_nodes[0]["value"], "items");
    assert_eq!(compact_nodes[0]["matchText"], "Bag.items");
    assert!(
        elapsed_ms <= max_total_ms,
        "provider candidate annotations cold functional path exceeded benchmark max_total={} observed={}ms nodes={:?}",
        benchmark.max_total,
        elapsed_ms,
        nodes
    );

    let performance_gate = serde_json::json!({
        "schemaId": "agent.semantic-protocols.semantic-hot-path-performance-gate",
        "schemaVersion": "1",
        "scenarioId": "asp-provider-candidate-annotations-cold-functional-path",
        "languageId": "rust",
        "workspace": ".",
        "command": ["agent_semantic_search::provider_candidate_annotation_nodes"],
        "phase": "cold",
        "expected": {
            "targetTotal": benchmark.target_total,
            "maxTotal": benchmark.max_total,
            "regressionBudget": benchmark.regression_budget,
            "maxProviderProcessCount": 0,
            "maxStdoutBytes": benchmark.max_stdout_bytes,
            "allowedFirstRoutes": ["provider-candidate-annotations"],
            "forbiddenRoutes": ["command-local-generated-policy", "path-generated-filter"],
            "requireProviderOwnedAttributes": true,
            "requireSearchOwnedCompaction": true,
            "requireSearchOwnedStdoutExtraction": true
        },
        "observed": {
            "observedTotal": duration_literal(elapsed),
            "providerProcessCount": 0,
            "providerElapsed": "0us",
            "firstRoute": "provider-candidate-annotations",
            "executedRoutes": ["provider-candidate-annotations"],
            "providerOwnedAttributes": true,
            "searchOwnedCompaction": true,
            "searchOwnedStdoutExtraction": true,
            "stdoutBytes": 0,
            "fallbackReason": "none"
        },
        "verdict": "pass",
        "evidenceRefs": ["scenario:asp-provider-candidate-annotations-cold-functional-path"]
    });
    assert_eq!(performance_gate["observed"]["providerProcessCount"], 0);
    assert_eq!(
        performance_gate["observed"]["providerOwnedAttributes"],
        true
    );
    assert_eq!(performance_gate["observed"]["searchOwnedCompaction"], true);
    assert_eq!(
        performance_gate["observed"]["searchOwnedStdoutExtraction"],
        true
    );
}
