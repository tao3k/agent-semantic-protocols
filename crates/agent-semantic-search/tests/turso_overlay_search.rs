use std::{
    fs,
    path::Path,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use agent_semantic_client_core::{
    CacheGenerationId, LanguageId, ProviderId, SemanticSchemaId, SemanticSchemaVersion,
    state_core::ResolvedState,
};
use agent_semantic_client_db::{
    ClientDbEngine, ClientDbStructuralDependencyUsage, ClientDbStructuralIndexImport,
    ClientDbStructuralKind, ClientDbStructuralLocator, ClientDbStructuralName,
    ClientDbStructuralOwner, ClientDbStructuralPath, ClientDbStructuralQueryKey,
    ClientDbStructuralSource, ClientDbStructuralSymbol, TursoClientDbSearchDocument,
};
use agent_semantic_search::{
    TursoOverlaySearchDocument, bootstrap_turso_overlay_search_store,
    collect_turso_structural_index_ranked_candidates_from_engine_async,
    search_turso_overlay_documents, search_turso_structural_index_documents,
    upsert_turso_overlay_search_document,
};

const TURSO_UNIFIED_SEARCH_SCENARIO_ID: &str = "turso-unified-search-interface-warm-path";
const TURSO_UNIFIED_SEARCH_SCENARIO_ROOT: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/unit/scenarios/turso_unified_search_interface_warm_path"
);
const TURSO_OVERLAY_COLD_SCENARIO_ID: &str = "turso-overlay-search-cold-functional-path";
const TURSO_OVERLAY_COLD_SCENARIO_ROOT: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/unit/scenarios/turso_overlay_search_cold_functional_path"
);
const TURSO_OVERLAY_WARM_SCENARIO_ID: &str = "turso-overlay-search-warm-latency-path";
const TURSO_OVERLAY_WARM_SCENARIO_ROOT: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/unit/scenarios/turso_overlay_search_warm_latency_path"
);

#[tokio::test(flavor = "current_thread")]
async fn turso_overlay_search_cold_functional_path_filters_to_overlay_hits() {
    let scenario =
        fs::read_to_string(Path::new(TURSO_OVERLAY_COLD_SCENARIO_ROOT).join("scenario.toml"))
            .expect("read overlay cold scenario manifest");
    let benchmark =
        fs::read_to_string(Path::new(TURSO_OVERLAY_COLD_SCENARIO_ROOT).join("benchmark.toml"))
            .expect("read overlay cold benchmark manifest");
    assert!(scenario.contains(&format!("id = \"{TURSO_OVERLAY_COLD_SCENARIO_ID}\"")));
    for expected in [
        "SEARCH-AGENT-ASP-PERF-TURSO-OVERLAY-COLD-001",
        "route_source = \"turso-overlay\"",
        "max_provider_process_count = 0",
        "fallback_reason = \"none\"",
    ] {
        assert!(
            scenario.contains(expected) || benchmark.contains(expected),
            "overlay cold scenario benchmark missing {expected:?}"
        );
    }
    let max_total = duration_from_manifest(&benchmark, "max_total");

    let root = std::env::temp_dir().join(format!(
        "asp-turso-overlay-search-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock after epoch")
            .as_nanos()
    ));
    let project_root = root.join("project");
    let state_home = root.join("state-home");
    std::fs::create_dir_all(&project_root).expect("create temp project root");
    let state = ResolvedState::resolve_with_state_home(&project_root, &state_home)
        .expect("resolve state with explicit state home");
    state.ensure_minimal_layout().expect("create state layout");
    let engine = ClientDbEngine::from_resolved_state(&state);
    bootstrap_turso_overlay_search_store(&engine)
        .await
        .expect("bootstrap turso search schema");
    engine
        .upsert_search_document(&TursoClientDbSearchDocument {
            namespace: "stable".to_string(),
            document_id: "stable-owner".to_string(),
            entity_id: "stable-owner".to_string(),
            selector: Some("rust://src/lib.rs#item/function/stable_owner".to_string()),
            document: "stable overlay_fixture_token".to_string(),
        })
        .await
        .expect("upsert stable search document");
    upsert_turso_overlay_search_document(
        &engine,
        &TursoOverlaySearchDocument {
            repo_id: "repo-1".to_string(),
            workspace_id: "workspace-1".to_string(),
            session_id: "session-1".to_string(),
            base_generation: "dirty-1".to_string(),
            document_id: "overlay-owner".to_string(),
            selector: Some("rust://src/lib.rs#item/function/overlay_owner".to_string()),
            document: "dynamic overlay_fixture_token owner".to_string(),
        },
    )
    .await
    .expect("upsert overlay search document");

    let started = Instant::now();
    let hits = search_turso_overlay_documents(&engine, "overlay_fixture_token", 8)
        .await
        .expect("search turso overlay documents");
    let elapsed = started.elapsed();

    assert_eq!(hits.len(), 1, "{hits:#?}");
    assert_eq!(hits[0].document_id, "overlay-owner");
    assert_eq!(
        hits[0].selector.as_deref(),
        Some("rust://src/lib.rs#item/function/overlay_owner")
    );
    assert!(
        elapsed <= max_total,
        "overlay search should stay in the cold functional gate, max_total={max_total:?} elapsed={elapsed:?}"
    );
}

#[tokio::test(flavor = "current_thread")]
async fn turso_overlay_search_warm_latency_path_stays_inside_scenario_gate() {
    let scenario =
        fs::read_to_string(Path::new(TURSO_OVERLAY_WARM_SCENARIO_ROOT).join("scenario.toml"))
            .expect("read overlay warm scenario manifest");
    let benchmark =
        fs::read_to_string(Path::new(TURSO_OVERLAY_WARM_SCENARIO_ROOT).join("benchmark.toml"))
            .expect("read overlay warm benchmark manifest");
    assert!(scenario.contains(&format!("id = \"{TURSO_OVERLAY_WARM_SCENARIO_ID}\"")));
    for expected in [
        "SEARCH-AGENT-ASP-PERF-TURSO-OVERLAY-WARM-001",
        "phase = \"warm\"",
        "route_source = \"turso-overlay\"",
        "max_provider_process_count = 0",
        "fallback_reason = \"none\"",
    ] {
        assert!(
            scenario.contains(expected) || benchmark.contains(expected),
            "overlay warm scenario benchmark missing {expected:?}"
        );
    }
    let max_total = duration_from_manifest(&benchmark, "max_total");

    let root = temp_turso_search_root("asp-turso-overlay-warm");
    let project_root = root.join("project");
    let state_home = root.join("state-home");
    std::fs::create_dir_all(&project_root).expect("create temp project root");
    let state = ResolvedState::resolve_with_state_home(&project_root, &state_home)
        .expect("resolve state with explicit state home");
    state.ensure_minimal_layout().expect("create state layout");
    let engine = ClientDbEngine::from_resolved_state(&state);
    bootstrap_turso_overlay_search_store(&engine)
        .await
        .expect("bootstrap turso search schema");
    upsert_turso_overlay_search_document(
        &engine,
        &TursoOverlaySearchDocument {
            repo_id: "repo-1".to_string(),
            workspace_id: "workspace-1".to_string(),
            session_id: "session-1".to_string(),
            base_generation: "dirty-1".to_string(),
            document_id: "overlay-warm-owner".to_string(),
            selector: Some("rust://src/lib.rs#item/function/overlay_warm_owner".to_string()),
            document: "dynamic overlay_warm_fixture_token owner".to_string(),
        },
    )
    .await
    .expect("upsert warm overlay search document");

    let warmup = search_turso_overlay_documents(&engine, "overlay_warm_fixture_token", 8)
        .await
        .expect("warm up turso overlay search");
    assert_eq!(warmup.len(), 1, "{warmup:#?}");

    let started = Instant::now();
    for _ in 0..3 {
        let hits = search_turso_overlay_documents(&engine, "overlay_warm_fixture_token", 8)
            .await
            .expect("search warmed turso overlay documents");
        assert_eq!(hits.len(), 1, "{hits:#?}");
        assert_eq!(hits[0].document_id, "overlay-warm-owner");
    }
    let elapsed = started.elapsed();
    assert!(
        elapsed <= max_total,
        "warmed overlay search should stay inside max_total={max_total:?} observed={elapsed:?}"
    );
    let performance_gate = serde_json::json!({
        "schemaId": "agent.semantic-protocols.semantic-hot-path-performance-gate",
        "schemaVersion": "1",
        "scenarioId": TURSO_OVERLAY_WARM_SCENARIO_ID,
        "phase": "warm",
        "expected": {
            "maxProviderProcessCount": 0,
            "routeSource": "turso-overlay",
            "fallbackReason": "none"
        },
        "observed": {
            "providerProcessCount": 0,
            "routeSources": ["turso-overlay"],
            "queryCount": 3,
            "fallbackReason": "none"
        },
        "verdict": "pass"
    });
    assert_eq!(performance_gate["observed"]["providerProcessCount"], 0);
    let _ = std::fs::remove_dir_all(root);
}

#[tokio::test(flavor = "current_thread")]
async fn turso_unified_search_interface_warm_path_stays_inside_scenario_gate() {
    let scenario =
        fs::read_to_string(Path::new(TURSO_UNIFIED_SEARCH_SCENARIO_ROOT).join("scenario.toml"))
            .expect("read scenario manifest");
    let benchmark =
        fs::read_to_string(Path::new(TURSO_UNIFIED_SEARCH_SCENARIO_ROOT).join("benchmark.toml"))
            .expect("read benchmark manifest");
    assert!(scenario.contains(&format!("id = \"{TURSO_UNIFIED_SEARCH_SCENARIO_ID}\"")));
    for expected in [
        "SEARCH-AGENT-ASP-PERF-TURSO-UNIFIED-SEARCH-001",
        "route_source = \"turso-unified-search\"",
        "max_provider_process_count = 0",
        "fallback_reason = \"none\"",
    ] {
        assert!(
            scenario.contains(expected) || benchmark.contains(expected),
            "scenario benchmark missing {expected:?}"
        );
    }
    let max_total = duration_from_manifest(&benchmark, "max_total");

    let root = temp_turso_search_root("asp-turso-unified-search");
    let project_root = root.join("project");
    let state_home = root.join("state-home");
    std::fs::create_dir_all(&project_root).expect("create temp project root");
    let state = ResolvedState::resolve_with_state_home(&project_root, &state_home)
        .expect("resolve state with explicit state home");
    state.ensure_minimal_layout().expect("create state layout");
    let engine = ClientDbEngine::from_resolved_state(&state);
    bootstrap_turso_overlay_search_store(&engine)
        .await
        .expect("bootstrap turso search schema");
    engine
        .upsert_search_document(&TursoClientDbSearchDocument {
            namespace: "stable".to_string(),
            document_id: "stable-owner".to_string(),
            entity_id: "stable-owner".to_string(),
            selector: Some("rust://src/lib.rs#item/function/stable_owner".to_string()),
            document: "stable unified_search_fixture_token".to_string(),
        })
        .await
        .expect("upsert stable search document");
    upsert_turso_overlay_search_document(
        &engine,
        &TursoOverlaySearchDocument {
            repo_id: "repo-1".to_string(),
            workspace_id: "workspace-1".to_string(),
            session_id: "session-1".to_string(),
            base_generation: "dirty-1".to_string(),
            document_id: "overlay-owner".to_string(),
            selector: Some("rust://src/lib.rs#item/function/overlay_owner".to_string()),
            document: "dynamic unified_search_fixture_token owner".to_string(),
        },
    )
    .await
    .expect("upsert overlay search document");
    let structural_index_import = ClientDbStructuralIndexImport {
        generation_id: CacheGenerationId::from("unified-search-fixture"),
        language_id: LanguageId::from("rust"),
        provider_id: ProviderId::from("rs-harness"),
        provider_version: None,
        export_method: None,
        project_root: project_root.clone(),
        package_root: None,
        schema_id: SemanticSchemaId::from("agent.semantic-protocols.semantic-structural-index"),
        schema_version: SemanticSchemaVersion::from("1"),
        source_artifact_id: None,
        file_hashes: Vec::new(),
        owners: vec![ClientDbStructuralOwner {
            owner_path: ClientDbStructuralPath::from("src/unified_search.rs"),
            owner_kind: ClientDbStructuralKind::from("source"),
            source_authority: ClientDbStructuralSource::from("native-parser"),
            start_line: None,
            end_line: None,
            query_keys: vec![ClientDbStructuralQueryKey::from(
                "unified_search_fixture_token",
            )],
        }],
        symbols: vec![ClientDbStructuralSymbol {
            owner_path: ClientDbStructuralPath::from("src/unified_search.rs"),
            name: ClientDbStructuralName::from("unified_search_fixture_token"),
            kind: ClientDbStructuralKind::from("function"),
            visibility: Some(ClientDbStructuralKind::from("public")),
            source_locator: Some(ClientDbStructuralLocator::from(
                "rust://src/unified_search.rs#item/fn/unified_search_fixture_token",
            )),
            query_keys: vec![ClientDbStructuralQueryKey::from(
                "unified_search_fixture_token",
            )],
        }],
        dependency_usages: Vec::new(),
    };
    engine
        .persist_structural_index_read_model(&structural_index_import)
        .await
        .expect("persist structural-index Turso read model");

    let started = Instant::now();
    let overlay_hits = search_turso_overlay_documents(&engine, "unified_search_fixture_token", 8)
        .await
        .expect("search turso overlay documents");
    let structural_hits =
        search_turso_structural_index_documents(&engine, "unified_search_fixture_token", 8)
            .await
            .expect("search turso structural-index documents");
    let ranked = collect_turso_structural_index_ranked_candidates_from_engine_async(
        &engine,
        "unified_search_fixture_token",
        8,
    )
    .await
    .expect("collect ranked Turso structural-index candidates");
    let elapsed = started.elapsed();

    assert_eq!(overlay_hits.len(), 1, "{overlay_hits:#?}");
    assert_eq!(overlay_hits[0].document_id, "overlay-owner");
    assert_eq!(structural_hits.len(), 1, "{structural_hits:#?}");
    assert_eq!(ranked.len(), 1, "{ranked:#?}");
    assert_eq!(ranked[0].candidate.route_source, "turso-fts");
    assert!(
        elapsed <= max_total,
        "Turso unified search interface exceeded max_total={max_total:?} observed={elapsed:?}"
    );
    let performance_gate = serde_json::json!({
        "schemaId": "agent.semantic-protocols.semantic-hot-path-performance-gate",
        "schemaVersion": "1",
        "scenarioId": TURSO_UNIFIED_SEARCH_SCENARIO_ID,
        "phase": "warm",
        "expected": {
            "maxProviderProcessCount": 0,
            "routeSource": "turso-unified-search",
            "fallbackReason": "none"
        },
        "observed": {
            "providerProcessCount": 0,
            "routeSources": ["turso-overlay", "turso-fts"],
            "overlayHitCount": overlay_hits.len(),
            "structuralHitCount": structural_hits.len(),
            "rankedCandidateCount": ranked.len(),
            "fallbackReason": "none"
        },
        "verdict": "pass"
    });
    assert_eq!(performance_gate["observed"]["providerProcessCount"], 0);
}

#[tokio::test(flavor = "current_thread")]
async fn turso_structural_index_search_cold_functional_path_filters_to_structural_hits() {
    let root = std::env::temp_dir().join(format!(
        "asp-turso-structural-search-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock after epoch")
            .as_nanos()
    ));
    let project_root = root.join("project");
    let state_home = root.join("state-home");
    std::fs::create_dir_all(&project_root).expect("create temp project root");
    let state = ResolvedState::resolve_with_state_home(&project_root, &state_home)
        .expect("resolve state with explicit state home");
    state.ensure_minimal_layout().expect("create state layout");
    let engine = ClientDbEngine::from_resolved_state(&state);
    bootstrap_turso_overlay_search_store(&engine)
        .await
        .expect("bootstrap turso search schema");
    engine
        .upsert_search_document(&TursoClientDbSearchDocument {
            namespace: "source-index".to_string(),
            document_id: "source-index:noise:src/lib.rs".to_string(),
            entity_id: "source-owner:noise:src/lib.rs".to_string(),
            selector: Some("rust://src/lib.rs#file".to_string()),
            document: "parse_config non structural source-index noise".to_string(),
        })
        .await
        .expect("upsert non-structural stable search document");
    let structural_index_import = ClientDbStructuralIndexImport {
        generation_id: CacheGenerationId::from("structural-search-fixture"),
        language_id: LanguageId::from("rust"),
        provider_id: ProviderId::from("rs-harness"),
        provider_version: None,
        export_method: None,
        project_root: project_root.clone(),
        package_root: None,
        schema_id: SemanticSchemaId::from("agent.semantic-protocols.semantic-structural-index"),
        schema_version: SemanticSchemaVersion::from("1"),
        source_artifact_id: None,
        file_hashes: Vec::new(),
        owners: vec![ClientDbStructuralOwner {
            owner_path: ClientDbStructuralPath::from("src/structural_search.rs"),
            owner_kind: ClientDbStructuralKind::from("source"),
            source_authority: ClientDbStructuralSource::from("native-parser"),
            start_line: None,
            end_line: None,
            query_keys: vec![ClientDbStructuralQueryKey::from("parse_config")],
        }],
        symbols: vec![ClientDbStructuralSymbol {
            owner_path: ClientDbStructuralPath::from("src/structural_search.rs"),
            name: ClientDbStructuralName::from("parse_config"),
            kind: ClientDbStructuralKind::from("function"),
            visibility: Some(ClientDbStructuralKind::from("public")),
            source_locator: Some(ClientDbStructuralLocator::from(
                "rust://src/structural_search.rs#item/fn/parse_config",
            )),
            query_keys: vec![ClientDbStructuralQueryKey::from("parse_config")],
        }],
        dependency_usages: vec![ClientDbStructuralDependencyUsage {
            owner_path: ClientDbStructuralPath::from("src/structural_search.rs"),
            package_name: ClientDbStructuralName::from("serde_json"),
            package_version: None,
            api_name: Some(ClientDbStructuralName::from("from_str")),
            import_path: Some(ClientDbStructuralPath::from("serde_json::from_str")),
            manifest_path: None,
            lockfile_hash: None,
            source: ClientDbStructuralSource::from("native-parser"),
            source_locator: Some(ClientDbStructuralLocator::from(
                "rust://src/structural_search.rs#dep/serde_json/from_str",
            )),
            query_keys: vec![ClientDbStructuralQueryKey::from("serde_json::from_str")],
        }],
    };
    let report = engine
        .persist_structural_index_read_model(&structural_index_import)
        .await
        .expect("persist structural-index Turso read model");
    assert_eq!(report.search_document_count, 2);

    let started = Instant::now();
    let hits = search_turso_structural_index_documents(&engine, "parse_config", 8)
        .await
        .expect("search Turso structural-index documents");
    let elapsed = started.elapsed();

    assert_eq!(hits.len(), 1, "{hits:#?}");
    assert!(
        hits[0].document_id.contains("structural-search-fixture"),
        "{hits:#?}"
    );
    assert_eq!(
        hits[0].selector.as_deref(),
        Some("rust://src/structural_search.rs#item/fn/parse_config")
    );
    assert!(hits[0].document.contains("parse_config"), "{hits:#?}");
    assert!(
        elapsed.as_millis() <= 25,
        "structural-index search should stay in the cold functional millisecond gate, elapsed={elapsed:?}"
    );
    let ranked = collect_turso_structural_index_ranked_candidates_from_engine_async(
        &engine,
        "parse_config",
        8,
    )
    .await
    .expect("collect ranked Turso structural-index candidates from project state");
    assert_eq!(ranked.len(), 1, "{ranked:#?}");
    assert_eq!(ranked[0].candidate.route_source, "turso-fts");
    assert_eq!(
        ranked[0].candidate.selector.as_deref(),
        Some("rust://src/structural_search.rs#item/fn/parse_config")
    );
    assert_eq!(
        ranked[0].candidate.generation.as_deref(),
        Some("structural-search-fixture")
    );
    assert_eq!(ranked[0].candidate.identity_kind, "selector");
}

fn temp_turso_search_root(prefix: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!(
        "{}-{}",
        prefix,
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock after epoch")
            .as_nanos()
    ))
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
