#![cfg(feature = "turso-overlay")]

use std::ffi::{OsStr, OsString};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use agent_semantic_client_core::{
    CacheGenerationId, LanguageId, ProviderId, SemanticSchemaId, SemanticSchemaVersion,
    state_core::{ASP_STATE_HOME_ENV, ResolvedState},
};
use agent_semantic_client_db::{
    ClientDbEngine, ClientDbStructuralDependencyUsage, ClientDbStructuralIndexImport,
    ClientDbStructuralKind, ClientDbStructuralLocator, ClientDbStructuralName,
    ClientDbStructuralOwner, ClientDbStructuralPath, ClientDbStructuralQueryKey,
    ClientDbStructuralSource, ClientDbStructuralSymbol, TursoClientDbSearchDocument,
};
use agent_semantic_search::{
    TursoOverlaySearchDocument, TursoStructuralIndexCandidateRequest,
    bootstrap_turso_overlay_search_store, collect_turso_structural_index_ranked_candidates_async,
    search_turso_overlay_documents, search_turso_structural_index_documents,
    upsert_turso_overlay_search_document,
};

#[tokio::test(flavor = "current_thread")]
async fn turso_overlay_search_cold_functional_path_filters_to_overlay_hits() {
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
    let _state_home_env = EnvVarGuard::set(ASP_STATE_HOME_ENV, &state_home);
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
        elapsed.as_millis() <= 25,
        "overlay search should stay in the cold functional millisecond gate, elapsed={elapsed:?}"
    );
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
    let _state_home_env = EnvVarGuard::set(ASP_STATE_HOME_ENV, &state_home);
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
    let resolved_engine =
        ClientDbEngine::resolve(&project_root).expect("resolve DB Engine from temp project root");
    assert_eq!(resolved_engine.db_path(), engine.db_path());
    let ranked = collect_turso_structural_index_ranked_candidates_async(
        TursoStructuralIndexCandidateRequest {
            project_root: &project_root,
            query: "parse_config",
            limit: 8,
        },
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

struct EnvVarGuard {
    key: &'static str,
    previous: Option<OsString>,
}

impl EnvVarGuard {
    fn set(key: &'static str, value: impl AsRef<OsStr>) -> Self {
        let previous = std::env::var_os(key);
        // This integration test runs on a current-thread runtime and restores
        // the variable before returning.
        unsafe { std::env::set_var(key, value) };
        Self { key, previous }
    }
}

impl Drop for EnvVarGuard {
    fn drop(&mut self) {
        if let Some(previous) = &self.previous {
            // See `EnvVarGuard::set`; this is the paired restoration.
            unsafe { std::env::set_var(self.key, previous) };
        } else {
            // See `EnvVarGuard::set`; this is the paired restoration.
            unsafe { std::env::remove_var(self.key) };
        }
    }
}
