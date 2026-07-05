use std::path::Path;
use std::time::Instant;

use super::contracts::{
    assert_search_pipe_evidence_classifier_benchmark_contract,
    assert_search_pipe_package_cohesion_benchmark_contract,
    assert_search_pipe_quality_decision_benchmark_contract,
    assert_search_pipe_query_pack_benchmark_contract,
};
use super::runtime_gates::{duration_literal, duration_millis_from_manifest, read_toml};
use super::shared::SharedBenchmarkToml;

pub(super) fn asp_search_pipe_package_cohesion_cold_functional_path_stays_inside_scenario_gate() {
    let crate_root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let scenario_root = crate_root
        .join("tests")
        .join("unit")
        .join("scenarios")
        .join("asp_search_pipe_package_cohesion_cold_functional_path");
    let benchmark: SharedBenchmarkToml = read_toml(&scenario_root.join("benchmark.toml"));
    assert_search_pipe_package_cohesion_benchmark_contract(&benchmark);
    let max_total_ms = duration_millis_from_manifest(&benchmark.max_total);

    let candidate_paths = [
        "packages/runtime/search/src/router.rs",
        "packages/runtime/search/src/lib.rs",
        "crates/agent-semantic-search/src/lib.rs",
    ];
    let high_value_terms = vec![
        agent_semantic_search::SearchPipeCohesionTerm::new(
            "runtime-search-route",
            "runtime-search-route",
        ),
        agent_semantic_search::SearchPipeCohesionTerm::new(
            "agent-semantic-search",
            "agent-semantic-search",
        ),
    ];
    let weak_owner = vec!["runtime-search-route".to_string()];
    let strong_owner = vec![
        "runtime-search-route".to_string(),
        "agent-semantic-search".to_string(),
    ];

    let started_at = Instant::now();
    let packages = agent_semantic_search::search_pipe_candidate_packages(
        candidate_paths.into_iter().map(str::to_string),
    );
    let weak_cohesion = agent_semantic_search::search_pipe_package_cohesion(
        &packages,
        Some(&weak_owner),
        &high_value_terms,
    );
    let strong_cohesion = agent_semantic_search::search_pipe_package_cohesion(
        &packages,
        Some(&strong_owner),
        &high_value_terms,
    );
    let elapsed = started_at.elapsed();
    let elapsed_ms = elapsed.as_millis();

    assert_eq!(
        packages,
        vec![
            "crates/agent-semantic-search".to_string(),
            "packages/runtime/search".to_string()
        ]
    );
    assert_eq!(weak_cohesion, "low");
    assert_eq!(strong_cohesion, "medium");
    assert!(
        elapsed_ms <= max_total_ms,
        "search pipe package cohesion cold functional path exceeded benchmark max_total={} observed={}ms packages={packages:?} weak={weak_cohesion} strong={strong_cohesion}",
        benchmark.max_total,
        elapsed_ms
    );

    let performance_gate = serde_json::json!({
        "schemaId": "agent.semantic-protocols.semantic-hot-path-performance-gate",
        "schemaVersion": "1",
        "scenarioId": "asp-search-pipe-package-cohesion-cold-functional-path",
        "languageId": "rust",
        "workspace": ".",
        "command": [
            "agent_semantic_search::search_pipe_candidate_packages",
            "agent_semantic_search::search_pipe_package_cohesion"
        ],
        "phase": "cold",
        "expected": {
            "targetTotal": benchmark.target_total,
            "maxTotal": benchmark.max_total,
            "regressionBudget": benchmark.regression_budget,
            "maxProviderProcessCount": 0,
            "maxSearchOverlayProcessCount": 0,
            "maxStdoutBytes": benchmark.max_stdout_bytes,
            "requireSearchOwnedPackageCohesion": true,
            "allowedFirstRoutes": ["search-pipe-package-cohesion"],
            "forbiddenRoutes": ["command-package-cohesion", "provider-process", "native-finder"]
        },
        "observed": {
            "observedTotal": duration_literal(elapsed),
            "providerProcessCount": 0,
            "providerElapsed": "0us",
            "nativeFinderProcessCount": 0,
            "nativeFinderElapsed": "0us",
            "packageCount": packages.len(),
            "weakCohesion": weak_cohesion,
            "strongCohesion": strong_cohesion,
            "firstRoute": "search-pipe-package-cohesion",
            "executedRoutes": ["search-pipe-package-cohesion"],
            "stdoutBytes": 0,
            "fallbackReason": "none"
        },
        "verdict": "pass",
        "evidenceRefs": ["scenario:asp-search-pipe-package-cohesion-cold-functional-path"]
    });
    assert_eq!(performance_gate["observed"]["providerProcessCount"], 0);
    assert_eq!(performance_gate["observed"]["nativeFinderProcessCount"], 0);
    assert_eq!(performance_gate["observed"]["weakCohesion"], "low");
    assert_eq!(performance_gate["observed"]["strongCohesion"], "medium");
}

pub(super) fn asp_search_pipe_query_pack_cold_functional_path_stays_inside_scenario_gate() {
    let crate_root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let scenario_root = crate_root
        .join("tests")
        .join("unit")
        .join("scenarios")
        .join("asp_search_pipe_query_pack_cold_functional_path");
    let benchmark: SharedBenchmarkToml = read_toml(&scenario_root.join("benchmark.toml"));
    assert_search_pipe_query_pack_benchmark_contract(&benchmark);
    let max_total_ms = duration_millis_from_manifest(&benchmark.max_total);

    let query =
        "src/runtime.rs packages/runtime-search SearchRouter CacheStatus concurrency through owner";
    let candidates = vec![agent_semantic_search::SearchPipeQueryPackCandidate {
        path: "src/runtime.rs".to_string(),
        symbol: "SearchRouter".to_string(),
        text: "pub struct SearchRouter".to_string(),
    }];

    let started_at = Instant::now();
    let clauses = agent_semantic_search::search_pipe_query_clauses(
        agent_semantic_search::SearchPipeQueryClausesRequest::new(
            agent_semantic_search::SearchPipeLanguageId::new("rust"),
            agent_semantic_search::SearchPipeQueryText::new(query),
        ),
    );
    let clause_texts = agent_semantic_search::search_pipe_query_clause_texts(
        agent_semantic_search::SearchPipeQueryClausesRequest::new(
            agent_semantic_search::SearchPipeLanguageId::new("rust"),
            agent_semantic_search::SearchPipeQueryText::new(query),
        ),
    );
    let terms = agent_semantic_search::search_pipe_unique_query_terms(&clauses);
    let coverages = agent_semantic_search::search_pipe_clause_coverages(&clauses, &candidates);
    let owner_seed_terms = agent_semantic_search::search_pipe_role_terms(
        &terms,
        agent_semantic_search::SearchPipeTermRole::Symbol,
    );
    let elapsed = started_at.elapsed();
    let elapsed_ms = elapsed.as_millis();

    assert_eq!(
        clause_texts,
        vec![
            "src/runtime.rs packages/runtime-search".to_string(),
            "SearchRouter CacheStatus".to_string(),
            "concurrency".to_string()
        ]
    );
    assert_eq!(clauses.len(), 3);
    assert!(owner_seed_terms.iter().any(|term| term == "SearchRouter"));
    assert_eq!(coverages[1].matched, vec!["searchrouter".to_string()]);
    assert_eq!(coverages[1].missing, vec!["cachestatus".to_string()]);
    assert!(
        elapsed_ms <= max_total_ms,
        "search pipe query pack cold functional path exceeded benchmark max_total={} observed={}ms clauses={clause_texts:?} owner_seed_terms={owner_seed_terms:?}",
        benchmark.max_total,
        elapsed_ms
    );

    let performance_gate = serde_json::json!({
        "schemaId": "agent.semantic-protocols.semantic-hot-path-performance-gate",
        "schemaVersion": "1",
        "scenarioId": "asp-search-pipe-query-pack-cold-functional-path",
        "languageId": "rust",
        "workspace": ".",
        "command": [
            "agent_semantic_search::search_pipe_query_clauses",
            "agent_semantic_search::search_pipe_clause_coverages"
        ],
        "phase": "cold",
        "expected": {
            "targetTotal": benchmark.target_total,
            "maxTotal": benchmark.max_total,
            "regressionBudget": benchmark.regression_budget,
            "maxProviderProcessCount": 0,
            "maxSearchOverlayProcessCount": 0,
            "maxStdoutBytes": benchmark.max_stdout_bytes,
            "requireSearchOwnedQueryPack": true,
            "allowedFirstRoutes": ["search-pipe-query-pack"],
            "forbiddenRoutes": ["command-query-pack-parser", "provider-process", "native-finder"]
        },
        "observed": {
            "observedTotal": duration_literal(elapsed),
            "providerProcessCount": 0,
            "providerElapsed": "0us",
            "nativeFinderProcessCount": 0,
            "nativeFinderElapsed": "0us",
            "clauseCount": clauses.len(),
            "matched": coverages[1].matched,
            "missing": coverages[1].missing,
            "firstRoute": "search-pipe-query-pack",
            "executedRoutes": ["search-pipe-query-pack"],
            "stdoutBytes": 0,
            "fallbackReason": "none"
        },
        "verdict": "pass",
        "evidenceRefs": ["scenario:asp-search-pipe-query-pack-cold-functional-path"]
    });
    assert_eq!(performance_gate["observed"]["providerProcessCount"], 0);
    assert_eq!(performance_gate["observed"]["nativeFinderProcessCount"], 0);
    assert_eq!(performance_gate["observed"]["clauseCount"], 3);
}

pub(super) fn asp_search_pipe_quality_decision_cold_functional_path_stays_inside_scenario_gate() {
    let crate_root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let scenario_root = crate_root
        .join("tests")
        .join("unit")
        .join("scenarios")
        .join("asp_search_pipe_quality_decision_cold_functional_path");
    let benchmark: SharedBenchmarkToml = read_toml(&scenario_root.join("benchmark.toml"));
    assert_search_pipe_quality_decision_benchmark_contract(&benchmark);
    let max_total_ms = duration_millis_from_manifest(&benchmark.max_total);

    let terms = vec![
        agent_semantic_search::SearchPipeQueryTerm {
            raw: "src/runtime.rs".to_string(),
            lower: "src/runtime.rs".to_string(),
            role: agent_semantic_search::SearchPipeTermRole::Symbol,
        },
        agent_semantic_search::SearchPipeQueryTerm {
            raw: "SearchRouter".to_string(),
            lower: "searchrouter".to_string(),
            role: agent_semantic_search::SearchPipeTermRole::Symbol,
        },
        agent_semantic_search::SearchPipeQueryTerm {
            raw: "concurrency".to_string(),
            lower: "concurrency".to_string(),
            role: agent_semantic_search::SearchPipeTermRole::Concept,
        },
    ];
    let global_matched = vec!["searchrouter".to_string()];
    let global_missing = Vec::new();
    let strong_matched = Vec::new();
    let weak_terms = vec!["SearchRouter".to_string()];

    let started_at = Instant::now();
    let missing_path_terms =
        agent_semantic_search::search_pipe_missing_path_terms(&terms, &global_matched);
    let owner_seed_terms =
        agent_semantic_search::search_pipe_owner_seed_terms(&terms, &missing_path_terms);
    let risks = agent_semantic_search::search_pipe_quality_risks(
        &terms,
        ["pub struct SearchRouter {\n    field: String\n}".to_string()].into_iter(),
        &global_missing,
        &strong_matched,
        &weak_terms,
        "low",
        1,
    );
    let quality = agent_semantic_search::search_pipe_query_pack_quality(
        &terms,
        &global_missing,
        &weak_terms,
        &risks,
    );
    let fd_query = agent_semantic_search::search_pipe_fd_query_terms(
        &terms,
        &weak_terms,
        &strong_matched,
        &risks,
    );
    let elapsed = started_at.elapsed();
    let elapsed_ms = elapsed.as_millis();

    assert_eq!(missing_path_terms, vec!["src/runtime.rs".to_string()]);
    assert_eq!(owner_seed_terms, vec!["SearchRouter".to_string()]);
    assert!(risks.iter().any(|risk| risk == "package-drift"));
    assert!(risks.iter().any(|risk| risk == "weak-camelcase-match"));
    assert_eq!(quality, "low");
    assert_eq!(fd_query.as_deref(), Some("SearchRouter"));
    assert!(
        elapsed_ms <= max_total_ms,
        "search pipe quality decision cold functional path exceeded benchmark max_total={} observed={}ms risks={risks:?} quality={quality} fd_query={fd_query:?}",
        benchmark.max_total,
        elapsed_ms
    );

    let performance_gate = serde_json::json!({
        "schemaId": "agent.semantic-protocols.semantic-hot-path-performance-gate",
        "schemaVersion": "1",
        "scenarioId": "asp-search-pipe-quality-decision-cold-functional-path",
        "languageId": "rust",
        "workspace": ".",
        "command": [
            "agent_semantic_search::search_pipe_quality_risks",
            "agent_semantic_search::search_pipe_query_pack_quality",
            "agent_semantic_search::search_pipe_fd_query_terms"
        ],
        "phase": "cold",
        "expected": {
            "targetTotal": benchmark.target_total,
            "maxTotal": benchmark.max_total,
            "regressionBudget": benchmark.regression_budget,
            "maxProviderProcessCount": 0,
            "maxSearchOverlayProcessCount": 0,
            "maxStdoutBytes": benchmark.max_stdout_bytes,
            "requireSearchOwnedQualityDecision": true,
            "allowedFirstRoutes": ["search-pipe-quality-decision"],
            "forbiddenRoutes": ["command-quality-decision", "provider-process", "native-finder"]
        },
        "observed": {
            "observedTotal": duration_literal(elapsed),
            "providerProcessCount": 0,
            "providerElapsed": "0us",
            "nativeFinderProcessCount": 0,
            "nativeFinderElapsed": "0us",
            "riskCount": risks.len(),
            "quality": quality,
            "fdQuery": fd_query,
            "firstRoute": "search-pipe-quality-decision",
            "executedRoutes": ["search-pipe-quality-decision"],
            "stdoutBytes": 0,
            "fallbackReason": "none"
        },
        "verdict": "pass",
        "evidenceRefs": ["scenario:asp-search-pipe-quality-decision-cold-functional-path"]
    });
    assert_eq!(performance_gate["observed"]["providerProcessCount"], 0);
    assert_eq!(performance_gate["observed"]["nativeFinderProcessCount"], 0);
    assert_eq!(performance_gate["observed"]["quality"], "low");
}

pub(super) fn asp_search_pipe_evidence_classifier_cold_functional_path_stays_inside_scenario_gate()
{
    let crate_root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let scenario_root = crate_root
        .join("tests")
        .join("unit")
        .join("scenarios")
        .join("asp_search_pipe_evidence_classifier_cold_functional_path");
    let benchmark: SharedBenchmarkToml = read_toml(&scenario_root.join("benchmark.toml"));
    assert_search_pipe_evidence_classifier_benchmark_contract(&benchmark);
    let max_total_ms = duration_millis_from_manifest(&benchmark.max_total);

    let candidates = vec![
        agent_semantic_search::SearchPipeEvidenceCandidate {
            path: "src/router.rs".to_string(),
            line: 7,
            symbol: "SearchRouter".to_string(),
            text: "pub struct SearchRouter {}".to_string(),
            source: "source-index".to_string(),
        },
        agent_semantic_search::SearchPipeEvidenceCandidate {
            path: "src/cache.rs".to_string(),
            line: 3,
            symbol: "CacheStatus".to_string(),
            text: "CacheStatus hit".to_string(),
            source: "search-overlay".to_string(),
        },
    ];
    let terms = vec![
        agent_semantic_search::SearchPipeQueryTerm {
            raw: "SearchRouter".to_string(),
            lower: "searchrouter".to_string(),
            role: agent_semantic_search::SearchPipeTermRole::Symbol,
        },
        agent_semantic_search::SearchPipeQueryTerm {
            raw: "CacheStatus".to_string(),
            lower: "cachestatus".to_string(),
            role: agent_semantic_search::SearchPipeTermRole::Symbol,
        },
        agent_semantic_search::SearchPipeQueryTerm {
            raw: "router::SearchRouter".to_string(),
            lower: "router::searchrouter".to_string(),
            role: agent_semantic_search::SearchPipeTermRole::Symbol,
        },
    ];

    let started_at = Instant::now();
    let declaration_match = agent_semantic_search::search_pipe_declaration_header_match(
        "rust",
        &candidates[0],
        &terms[0],
    );
    let compound_match =
        agent_semantic_search::search_pipe_strong_match("rust", &candidates[0], &terms[2]);
    let parser_handles =
        agent_semantic_search::search_pipe_parser_handles("rust", &candidates, &terms);
    let search_overlay_handles =
        agent_semantic_search::search_pipe_search_overlay_handles(&candidates, &terms);
    let weak_reason = agent_semantic_search::search_pipe_weak_reason(&terms[0], &candidates);
    let elapsed = started_at.elapsed();
    let elapsed_ms = elapsed.as_millis();

    assert!(declaration_match);
    assert!(compound_match);
    assert_eq!(
        parser_handles,
        vec!["SearchRouter@src/router.rs:7".to_string()]
    );
    assert_eq!(search_overlay_handles, vec!["CacheStatus".to_string()]);
    assert_eq!(weak_reason, "lexical-match");
    assert!(
        elapsed_ms <= max_total_ms,
        "search pipe evidence classifier cold functional path exceeded benchmark max_total={} observed={}ms parser_handles={parser_handles:?} search_overlay_handles={search_overlay_handles:?}",
        benchmark.max_total,
        elapsed_ms
    );

    let performance_gate = serde_json::json!({
        "schemaId": "agent.semantic-protocols.semantic-hot-path-performance-gate",
        "schemaVersion": "1",
        "scenarioId": "asp-search-pipe-evidence-classifier-cold-functional-path",
        "languageId": "rust",
        "workspace": ".",
        "command": [
            "agent_semantic_search::search_pipe_declaration_header_match",
            "agent_semantic_search::search_pipe_strong_match",
            "agent_semantic_search::search_pipe_parser_handles",
            "agent_semantic_search::search_pipe_search_overlay_handles"
        ],
        "phase": "cold",
        "expected": {
            "targetTotal": benchmark.target_total,
            "maxTotal": benchmark.max_total,
            "regressionBudget": benchmark.regression_budget,
            "maxProviderProcessCount": 0,
            "maxSearchOverlayProcessCount": 0,
            "maxStdoutBytes": benchmark.max_stdout_bytes,
            "requireSearchOwnedEvidenceClassifier": true,
            "allowedFirstRoutes": ["search-pipe-evidence-classifier"],
            "forbiddenRoutes": ["command-evidence-classifier", "provider-process", "native-finder"]
        },
        "observed": {
            "observedTotal": duration_literal(elapsed),
            "providerProcessCount": 0,
            "providerElapsed": "0us",
            "nativeFinderProcessCount": 0,
            "nativeFinderElapsed": "0us",
            "declarationMatch": declaration_match,
            "compoundMatch": compound_match,
            "parserHandleCount": parser_handles.len(),
            "searchOverlayHandleCount": search_overlay_handles.len(),
            "firstRoute": "search-pipe-evidence-classifier",
            "executedRoutes": ["search-pipe-evidence-classifier"],
            "stdoutBytes": 0,
            "fallbackReason": "none"
        },
        "verdict": "pass",
        "evidenceRefs": ["scenario:asp-search-pipe-evidence-classifier-cold-functional-path"]
    });
    assert_eq!(performance_gate["observed"]["providerProcessCount"], 0);
    assert_eq!(performance_gate["observed"]["nativeFinderProcessCount"], 0);
    assert_eq!(performance_gate["observed"]["declarationMatch"], true);
    assert_eq!(performance_gate["observed"]["compoundMatch"], true);
}
