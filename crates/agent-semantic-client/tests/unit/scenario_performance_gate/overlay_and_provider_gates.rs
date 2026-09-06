// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-only

use std::{path::Path, time::Instant};

use super::contracts::assert_provider_candidate_annotations_benchmark_contract;
use super::runtime_gates::{duration_literal, duration_millis_from_manifest, read_toml};
use super::shared::SharedBenchmarkToml;

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
        "source": "asp-rust",
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
{"nodes":[{"id":"field:src/generated/lib.rs-items","kind":"field","role":"class-field","value":"items: list[str]","action":"code"}],"edges":[],"candidateAnnotations":[{"path":"src/generated/lib.rs","attributes":["generated","schema-generated"],"source":"asp-rust","reason":"provider-parser-fact"}]}
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
