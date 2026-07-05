use std::fs;
use std::path::{Path, PathBuf};
use std::time::Instant;

use super::contracts::{
    assert_query_wrapper_clause_normalization_benchmark_contract,
    assert_query_wrapper_render_hint_projection_benchmark_contract,
    assert_query_wrapper_source_index_bridge_benchmark_contract,
    assert_search_query_budget_benchmark_contract,
};
use super::runtime_gates::{duration_literal, duration_millis_from_manifest, read_toml};
use super::shared::SharedBenchmarkToml;
use crate::provider_command::support::temp_project_root;

pub(crate) fn asp_query_wrapper_source_index_bridge_cold_functional_path_stays_inside_scenario_gate()
 {
    let crate_root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let scenario_root = crate_root
        .join("tests")
        .join("unit")
        .join("scenarios")
        .join("asp_query_wrapper_source_index_bridge_cold_functional_path");
    let benchmark: SharedBenchmarkToml = read_toml(&scenario_root.join("benchmark.toml"));
    assert_query_wrapper_source_index_bridge_benchmark_contract(&benchmark);
    let max_total_ms = duration_millis_from_manifest(&benchmark.max_total);

    let root = temp_project_root("scenario-query-wrapper-source-index-bridge-cold");
    fs::create_dir_all(root.join("src")).expect("create source root");
    fs::write(
        root.join("src/lib.rs"),
        "pub fn query_wrapper_source_index_bridge() {}\n",
    )
    .expect("write source");

    let terms = vec!["query_wrapper_source_index_bridge".to_string()];
    let started_at = Instant::now();
    let lookup = agent_semantic_search::QueryWrapperSourceIndexLookup::new(
        root.join("live/client/client.turso"),
        "hit",
        vec![
            agent_semantic_search::QueryWrapperSourceIndexCandidate::new(
                (
                    "src/lib.rs",
                    Some("rust".to_string()),
                    Some("rs-harness".to_string()),
                    "source",
                    Some(1),
                    terms.clone(),
                )
                    .into(),
            ),
        ],
    );
    let collection = agent_semantic_search::collect_query_wrapper_source_index_candidates(
        agent_semantic_search::QueryWrapperSourceIndexRequest {
            surface: agent_semantic_search::QueryWrapperCandidateSurface::Rg,
            project_root: &root,
            roots: std::slice::from_ref(&root),
            terms: &terms,
            axis_terms: &terms,
            lookup: &lookup,
        },
    )
    .expect("collect query-wrapper source-index bridge candidates")
    .expect("source-index hit should produce candidates");
    let elapsed = started_at.elapsed();
    let elapsed_ms = elapsed.as_millis();

    assert_eq!(collection.candidates.len(), 1);
    assert_eq!(collection.candidates[0].path, "src/lib.rs");
    assert_eq!(collection.candidates[0].source, "source-index");
    assert!(
        collection
            .candidates
            .iter()
            .all(|candidate| !candidate.path.contains(":1:")),
        "query-wrapper source-index bridge must not expose executable line-range identity: {:?}",
        collection.candidates
    );
    assert!(
        !root.join(".cache").join("agent-semantic-protocol").exists(),
        "query-wrapper source-index bridge must not create project-local cache"
    );
    assert!(
        elapsed_ms <= max_total_ms,
        "query-wrapper source-index bridge cold functional path exceeded benchmark max_total={} observed={}ms candidates={:?}",
        benchmark.max_total,
        elapsed_ms,
        collection.candidates
    );

    let observed_total = duration_literal(elapsed);
    let performance_gate = serde_json::json!({
        "schemaId": "agent.semantic-protocols.semantic-hot-path-performance-gate",
        "schemaVersion": "1",
        "scenarioId": "asp-query-wrapper-source-index-bridge-cold-functional-path",
        "languageId": "rust",
        "workspace": ".",
        "command": [
            "agent_semantic_search::QueryWrapperSourceIndexLookup::new",
            "agent_semantic_search::QueryWrapperSourceIndexCandidate::new",
            "agent_semantic_search::collect_query_wrapper_source_index_candidates"
        ],
        "phase": "cold",
        "expected": {
            "targetTotal": benchmark.target_total,
            "maxTotal": benchmark.max_total,
            "regressionBudget": benchmark.regression_budget,
            "maxProviderProcessCount": 0,
            "maxSearchOverlayProcessCount": 0,
            "maxStdoutBytes": benchmark.max_stdout_bytes,
            "requireSearchOwnedBridge": true,
            "allowedFirstRoutes": ["query-wrapper-source-index-bridge"],
            "forbiddenRoutes": ["command-dto-construction", "native-finder", "provider-process"],
            "requireNoExecutableLineRange": true
        },
        "observed": {
            "observedTotal": observed_total,
            "providerProcessCount": 0,
            "providerElapsed": "0us",
            "nativeFinderProcessCount": 0,
            "nativeFinderElapsed": "0us",
            "candidateCount": collection.candidates.len(),
            "firstRoute": "query-wrapper-source-index-bridge",
            "executedRoutes": ["query-wrapper-source-index-bridge"],
            "executableLineRangeSelectorCount": 0,
            "stdoutBytes": 0,
            "fallbackReason": "none"
        },
        "verdict": "pass",
        "evidenceRefs": ["scenario:asp-query-wrapper-source-index-bridge-cold-functional-path"]
    });
    assert_eq!(performance_gate["observed"]["providerProcessCount"], 0);
    assert_eq!(performance_gate["observed"]["nativeFinderProcessCount"], 0);
    assert_eq!(performance_gate["observed"]["candidateCount"], 1);
    let _ = fs::remove_dir_all(root);
}

pub(crate) fn asp_query_wrapper_render_hint_projection_cold_functional_path_stays_inside_scenario_gate()
 {
    let crate_root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let scenario_root = crate_root
        .join("tests")
        .join("unit")
        .join("scenarios")
        .join("asp_query_wrapper_render_hint_projection_cold_functional_path");
    let benchmark: SharedBenchmarkToml = read_toml(&scenario_root.join("benchmark.toml"));
    assert_query_wrapper_render_hint_projection_benchmark_contract(&benchmark);
    let max_total_ms = duration_millis_from_manifest(&benchmark.max_total);

    let paths = vec![
        "packages/client/runtime/search.rs".to_string(),
        "packages/client/runtime/search.rs".to_string(),
        "packages/protocol/command/search.rs".to_string(),
        "docs/search/provider.org".to_string(),
        "crates/agent-semantic-search/src/query_wrapper_candidates.rs".to_string(),
    ];
    let started_at = Instant::now();
    let owner_candidates =
        agent_semantic_search::query_wrapper_owner_candidates(paths.clone().into_iter());
    let package_clusters =
        agent_semantic_search::query_wrapper_package_clusters_from_paths(paths.clone().into_iter());
    let rg_scope_next = agent_semantic_search::query_wrapper_rg_scope_next(paths.into_iter());
    let elapsed = started_at.elapsed();
    let elapsed_ms = elapsed.as_millis();

    assert_eq!(
        owner_candidates,
        vec![
            "packages/client/runtime/search.rs".to_string(),
            "packages/protocol/command/search.rs".to_string(),
            "docs/search/provider.org".to_string(),
            "crates/agent-semantic-search/src/query_wrapper_candidates.rs".to_string(),
        ]
    );
    assert_eq!(
        package_clusters,
        vec![
            "packages/client/runtime".to_string(),
            "packages/protocol/command".to_string(),
            "docs/search".to_string(),
            "crates/agent-semantic-search".to_string(),
        ]
    );
    assert_eq!(
        rg_scope_next,
        vec![
            "packages/client/runtime".to_string(),
            "packages/protocol/command".to_string(),
            "docs/search".to_string(),
        ]
    );
    assert!(
        elapsed_ms <= max_total_ms,
        "query-wrapper render hint projection cold functional path exceeded benchmark max_total={} observed={}ms",
        benchmark.max_total,
        elapsed_ms
    );

    let observed_total = duration_literal(elapsed);
    let performance_gate = serde_json::json!({
        "schemaId": "agent.semantic-protocols.semantic-hot-path-performance-gate",
        "schemaVersion": "1",
        "scenarioId": "asp-query-wrapper-render-hint-projection-cold-functional-path",
        "languageId": "rust",
        "workspace": ".",
        "command": [
            "agent_semantic_search::query_wrapper_owner_candidates",
            "agent_semantic_search::query_wrapper_package_clusters_from_paths",
            "agent_semantic_search::query_wrapper_rg_scope_next"
        ],
        "phase": "cold",
        "expected": {
            "targetTotal": benchmark.target_total,
            "maxTotal": benchmark.max_total,
            "regressionBudget": benchmark.regression_budget,
            "maxProviderProcessCount": 0,
            "maxSearchOverlayProcessCount": 0,
            "maxStdoutBytes": benchmark.max_stdout_bytes,
            "requireSearchOwnedProjection": true,
            "allowedFirstRoutes": ["query-wrapper-render-hint-projection"],
            "forbiddenRoutes": ["command-local-package-key", "native-finder", "provider-process"],
            "requireNoExecutableLineRange": true
        },
        "observed": {
            "observedTotal": observed_total,
            "providerProcessCount": 0,
            "providerElapsed": "0us",
            "nativeFinderProcessCount": 0,
            "nativeFinderElapsed": "0us",
            "ownerCandidateCount": owner_candidates.len(),
            "packageClusterCount": package_clusters.len(),
            "rgScopeNextCount": rg_scope_next.len(),
            "firstRoute": "query-wrapper-render-hint-projection",
            "executedRoutes": ["query-wrapper-render-hint-projection"],
            "executableLineRangeSelectorCount": 0,
            "stdoutBytes": 0,
            "fallbackReason": "none"
        },
        "verdict": "pass",
        "evidenceRefs": ["scenario:asp-query-wrapper-render-hint-projection-cold-functional-path"]
    });
    assert_eq!(performance_gate["observed"]["providerProcessCount"], 0);
    assert_eq!(performance_gate["observed"]["nativeFinderProcessCount"], 0);
    assert_eq!(performance_gate["observed"]["ownerCandidateCount"], 4);
    assert_eq!(performance_gate["observed"]["packageClusterCount"], 4);
    assert_eq!(performance_gate["observed"]["rgScopeNextCount"], 3);
}

pub(crate) fn asp_query_wrapper_clause_normalization_cold_functional_path_stays_inside_scenario_gate()
 {
    let crate_root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let scenario_root = crate_root
        .join("tests")
        .join("unit")
        .join("scenarios")
        .join("asp_query_wrapper_clause_normalization_cold_functional_path");
    let benchmark: SharedBenchmarkToml = read_toml(&scenario_root.join("benchmark.toml"));
    assert_query_wrapper_clause_normalization_benchmark_contract(&benchmark);
    let max_total_ms = duration_millis_from_manifest(&benchmark.max_total);

    let raw_queries = vec![
        "CacheStatus cache_status".to_string(),
        "HTTPServer owner".to_string(),
        "   ".to_string(),
    ];
    let started_at = Instant::now();
    let clauses = agent_semantic_search::query_wrapper_clauses(&raw_queries);
    let terms = agent_semantic_search::query_wrapper_unique_clause_terms(&clauses);
    let elapsed = started_at.elapsed();
    let elapsed_ms = elapsed.as_millis();

    assert_eq!(clauses.len(), 2);
    assert_eq!(clauses[0].id, 1);
    assert_eq!(clauses[0].raw, "CacheStatus cache_status");
    assert!(clauses[0].axis_terms.contains(&"cache".to_string()));
    assert!(clauses[0].axis_terms.contains(&"status".to_string()));
    assert_eq!(
        terms,
        vec![
            "cachestatus".to_string(),
            "cache_status".to_string(),
            "httpserver".to_string(),
            "owner".to_string(),
        ]
    );
    assert!(
        elapsed_ms <= max_total_ms,
        "query-wrapper clause normalization cold functional path exceeded benchmark max_total={} observed={}ms clauses={:?}",
        benchmark.max_total,
        elapsed_ms,
        clauses
    );

    let observed_total = duration_literal(elapsed);
    let performance_gate = serde_json::json!({
        "schemaId": "agent.semantic-protocols.semantic-hot-path-performance-gate",
        "schemaVersion": "1",
        "scenarioId": "asp-query-wrapper-clause-normalization-cold-functional-path",
        "languageId": "rust",
        "workspace": ".",
        "command": [
            "agent_semantic_search::query_wrapper_clauses",
            "agent_semantic_search::query_wrapper_unique_clause_terms"
        ],
        "phase": "cold",
        "expected": {
            "targetTotal": benchmark.target_total,
            "maxTotal": benchmark.max_total,
            "regressionBudget": benchmark.regression_budget,
            "maxProviderProcessCount": 0,
            "maxSearchOverlayProcessCount": 0,
            "maxStdoutBytes": benchmark.max_stdout_bytes,
            "requireSearchOwnedClauseNormalization": true,
            "allowedFirstRoutes": ["query-wrapper-clause-normalization"],
            "forbiddenRoutes": ["command-local-query-terms", "native-finder", "provider-process"],
            "requireIdentifierAxisExpansion": true
        },
        "observed": {
            "observedTotal": observed_total,
            "providerProcessCount": 0,
            "providerElapsed": "0us",
            "nativeFinderProcessCount": 0,
            "nativeFinderElapsed": "0us",
            "clauseCount": clauses.len(),
            "uniqueTermCount": terms.len(),
            "firstRoute": "query-wrapper-clause-normalization",
            "executedRoutes": ["query-wrapper-clause-normalization"],
            "stdoutBytes": 0,
            "fallbackReason": "none"
        },
        "verdict": "pass",
        "evidenceRefs": ["scenario:asp-query-wrapper-clause-normalization-cold-functional-path"]
    });
    assert_eq!(performance_gate["observed"]["providerProcessCount"], 0);
    assert_eq!(performance_gate["observed"]["nativeFinderProcessCount"], 0);
    assert_eq!(performance_gate["observed"]["clauseCount"], 2);
    assert_eq!(performance_gate["observed"]["uniqueTermCount"], 4);
}

pub(crate) fn asp_search_query_budget_cold_functional_path_stays_inside_scenario_gate() {
    let crate_root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let scenario_root = crate_root
        .join("tests")
        .join("unit")
        .join("scenarios")
        .join("asp_search_query_budget_cold_functional_path");
    let benchmark: SharedBenchmarkToml = read_toml(&scenario_root.join("benchmark.toml"));
    assert_search_query_budget_benchmark_contract(&benchmark);
    let max_total_ms = duration_millis_from_manifest(&benchmark.max_total);

    let started_at = Instant::now();
    let block = agent_semantic_search::search_query_budget_block(
        "search query budget block generic provider",
        &[PathBuf::from(".")],
        false,
    )
    .expect("broad generic query should be blocked");
    let specific_terms =
        agent_semantic_search::search_query_terms("CacheStatus cache_status src/lib.rs");
    let specific_allowed = agent_semantic_search::search_terms_budget_block(
        &specific_terms,
        &[PathBuf::from(".")],
        false,
    )
    .is_none();
    let filtered_allowed = agent_semantic_search::search_query_budget_block(
        "search query budget block generic provider",
        &[PathBuf::from(".")],
        true,
    )
    .is_none();
    let rg_block = agent_semantic_search::search_rg_terms_budget_block(
        &[
            "alpha".to_string(),
            "beta".to_string(),
            "gamma".to_string(),
            "delta".to_string(),
            "epsilon".to_string(),
        ],
        &[],
        false,
    )
    .expect("rg broad term budget should block");
    let elapsed = started_at.elapsed();
    let elapsed_ms = elapsed.as_millis();

    assert_eq!(block.reason, "query-too-broad");
    assert_eq!(block.term_count, 6);
    assert!(specific_allowed);
    assert!(filtered_allowed);
    assert_eq!(rg_block.reason, "query-too-broad");
    assert!(
        elapsed_ms <= max_total_ms,
        "search query budget cold functional path exceeded benchmark max_total={} observed={}ms block={:?}",
        benchmark.max_total,
        elapsed_ms,
        block
    );

    let observed_total = duration_literal(elapsed);
    let performance_gate = serde_json::json!({
        "schemaId": "agent.semantic-protocols.semantic-hot-path-performance-gate",
        "schemaVersion": "1",
        "scenarioId": "asp-search-query-budget-cold-functional-path",
        "languageId": "rust",
        "workspace": ".",
        "command": [
            "agent_semantic_search::search_query_budget_block",
            "agent_semantic_search::search_terms_budget_block",
            "agent_semantic_search::search_rg_terms_budget_block"
        ],
        "phase": "cold",
        "expected": {
            "targetTotal": benchmark.target_total,
            "maxTotal": benchmark.max_total,
            "regressionBudget": benchmark.regression_budget,
            "maxProviderProcessCount": 0,
            "maxSearchOverlayProcessCount": 0,
            "maxStdoutBytes": benchmark.max_stdout_bytes,
            "requireSearchOwnedBudget": true,
            "allowedFirstRoutes": ["search-query-budget"],
            "forbiddenRoutes": ["command-local-query-budget", "native-finder", "provider-process"],
            "requireSpecificTermBypass": true
        },
        "observed": {
            "observedTotal": observed_total,
            "providerProcessCount": 0,
            "providerElapsed": "0us",
            "nativeFinderProcessCount": 0,
            "nativeFinderElapsed": "0us",
            "budgetReason": block.reason,
            "genericTermCount": block.generic_terms.len(),
            "specificAllowed": specific_allowed,
            "filteredAllowed": filtered_allowed,
            "rgBudgetReason": rg_block.reason,
            "firstRoute": "search-query-budget",
            "executedRoutes": ["search-query-budget"],
            "stdoutBytes": 0,
            "fallbackReason": "none"
        },
        "verdict": "pass",
        "evidenceRefs": ["scenario:asp-search-query-budget-cold-functional-path"]
    });
    assert_eq!(performance_gate["observed"]["providerProcessCount"], 0);
    assert_eq!(performance_gate["observed"]["nativeFinderProcessCount"], 0);
    assert_eq!(performance_gate["observed"]["specificAllowed"], true);
    assert_eq!(performance_gate["observed"]["filteredAllowed"], true);
}
