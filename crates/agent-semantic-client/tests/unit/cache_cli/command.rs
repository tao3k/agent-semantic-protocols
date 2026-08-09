use crate::cache_cli::run_cache;
use crate::test_support::{CACHE_TEST_LOCK, EnvVarGuard, artifacts_root_from_cache_root};
use agent_semantic_client_core::{
    AGENT_SEMANTIC_CLIENT_CACHE_MANIFEST_PROTOCOL_ID,
    AGENT_SEMANTIC_CLIENT_CACHE_MANIFEST_PROTOCOL_VERSION,
    AGENT_SEMANTIC_CLIENT_CACHE_MANIFEST_SCHEMA_ID,
    AGENT_SEMANTIC_CLIENT_CACHE_MANIFEST_SCHEMA_VERSION, CacheArtifactId, CacheStatus,
    ClientCacheFileHash, ClientCacheGeneration, ClientCacheManifest, ClientCachePath, LanguageId,
    ProviderId,
};
use agent_semantic_client_db::{ClientDbEngine, TursoClientDbSearchHit};
use agent_semantic_runtime::runtime_block_on_current_thread;
use serde_json::{Value, json};
use std::{
    path::Path,
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};

fn search_turso_documents(
    engine: &ClientDbEngine,
    source_snapshot: &agent_semantic_content_identity::SourceSnapshotEvidence,
    query: &str,
    limit: u32,
) -> Vec<TursoClientDbSearchHit> {
    runtime_block_on_current_thread(engine.search_documents(
        "structural-index",
        source_snapshot,
        query,
        limit,
    ))
    .expect("run active Turso DB Engine search")
    .expect("search active Turso DB Engine documents")
    .hits
}

#[test]
fn cache_usage_lists_flush() {
    let root = temp_root("usage");
    let error = run_cache(&root, None, &["unknown".to_string()], false).expect_err("usage");

    assert!(error.contains(
        "status|gc [--grace-days <n>] [--apply]|clean --day[=<days>]|import|source-index refresh|source-index lookup"
    ));
    assert!(error.contains("source-index refresh"));
    assert!(error.contains("source-index lookup --query <term>"));
    assert!(error.contains("invalidate|flush [syntax-rows]"));
}

#[test]
fn cache_status_process_reader_helper() {
    if std::env::var("ASP_CACHE_STATUS_PROCESS_READER_CHILD")
        .ok()
        .as_deref()
        != Some("1")
    {
        return;
    }
    let root = std::path::PathBuf::from(
        std::env::var("ASP_CACHE_STATUS_PROCESS_ROOT").expect("ASP_CACHE_STATUS_PROCESS_ROOT"),
    );
    run_cache(&root, None, &["status".to_string()], false).expect("process cache status reader");
}

#[test]
fn cache_status_survives_concurrent_process_readers() {
    let _guard = CACHE_TEST_LOCK.lock().expect("cache test lock");
    let root = temp_root("cache-status-concurrent-readers");
    let state_home = root.join(".asp-state");
    let _state_home = EnvVarGuard::set("ASP_STATE_HOME", &state_home);
    let engine = ClientDbEngine::resolve(&root).expect("resolve DB Engine");
    ClientDbEngine::open_write_session_client_dir(engine.client_dir())
        .expect("bootstrap Turso DB for cache status readers");
    assert!(engine.db_path().exists());

    let current_exe = std::env::current_exe().expect("locate current test binary");
    let mut children = Vec::new();
    for _ in 0..6 {
        children.push(
            Command::new(&current_exe)
                .arg("--exact")
                .arg("cache_cli_command_tests::cache_status_process_reader_helper")
                .arg("--nocapture")
                .env("ASP_CACHE_STATUS_PROCESS_READER_CHILD", "1")
                .env("ASP_CACHE_STATUS_PROCESS_ROOT", &root)
                .env("ASP_STATE_HOME", &state_home)
                .spawn()
                .expect("spawn cache status reader"),
        );
    }

    for mut child in children {
        let status = child.wait().expect("wait for cache status reader");
        assert!(status.success(), "cache status reader failed: {status}");
    }

    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn cache_flush_repairs_invalid_manifest() {
    let _guard = crate::test_support::CACHE_TEST_LOCK
        .lock()
        .expect("cache test lock");
    let root = temp_root("invalid-manifest");
    let cache_report = ClientCacheManifest::inspect_project(&root);
    let manifest_path = cache_report.manifest_path.clone().expect("manifest path");
    std::fs::create_dir_all(manifest_path.parent().expect("manifest parent"))
        .expect("create cache dir");
    std::fs::write(&manifest_path, "{}\n{}\n").expect("write invalid manifest");

    run_cache(&root, None, &["flush".to_string()], false).expect("flush invalid manifest");

    let manifest = ClientCacheManifest::load_from_path(&manifest_path).expect("repaired manifest");
    assert!(manifest.generations.is_empty());
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn cache_import_replays_structural_index_artifact_into_db() {
    let _guard = crate::test_support::CACHE_TEST_LOCK
        .lock()
        .expect("cache test lock");
    let root = temp_root("structural-index-import");
    let _state_home =
        crate::test_support::EnvVarGuard::set("ASP_STATE_HOME", root.join(".asp-state"));
    let provider_binary = root.join(".asp-state/runtime/bin/rs-harness");
    crate::cache_cli_source_index_tests::fixtures::write_project_resolution_provider(
        &provider_binary,
        "rust",
        "rs-harness",
        ".rs",
        &["src"],
        &[],
    );
    crate::test_support::write_hermetic_provider_registry_config(&root, "rust", "rs-harness");
    crate::test_support::write_hermetic_provider_install_receipt(&root, "rust", &provider_binary);
    std::fs::write(
        root.join("Cargo.toml"),
        "[package]\nname = \"cache-import-fixture\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
    )
    .expect("write Cargo manifest");
    std::fs::create_dir_all(root.join("src")).expect("create source directory");
    std::fs::write(
        root.join("src/lib.rs"),
        "pub fn cache_imported_symbol() {}\n",
    )
    .expect("write source file");
    let cache_report = ClientCacheManifest::inspect_project(&root);
    let cache_root = cache_report.cache_root.clone().expect("cache root");
    let manifest_path = cache_report.manifest_path.clone().expect("manifest path");
    let generation = ClientCacheGeneration {
        generation_id: "rust-index-import".into(),
        language_id: LanguageId::from("rust"),
        provider_id: ProviderId::from("rs-harness"),
        provider_version: Some("0.1.0".to_string()),
        export_method: Some("index/structural".to_string()),
        project_root: root.display().to_string(),
        package_root: Some(".".to_string()),
        schema_ids: vec!["agent.semantic-protocols.semantic-structural-index".into()],
        cache_status: CacheStatus::Hit,
        raw_source_stored: false,
        request_fingerprint: Some("fnv64:structural-import".to_string()),
        file_hashes: Some(vec![ClientCacheFileHash {
            path: "src/lib.rs".to_string(),
            sha256: "2".repeat(64),
            byte_len: 0,
            mtime_ms: 0,
        }]),
        artifact_ids: Some(vec![CacheArtifactId::from(
            "structural-index/rust-index-import.json",
        )]),
    };
    let manifest = ClientCacheManifest {
        schema_id: AGENT_SEMANTIC_CLIENT_CACHE_MANIFEST_SCHEMA_ID.into(),
        schema_version: AGENT_SEMANTIC_CLIENT_CACHE_MANIFEST_SCHEMA_VERSION.into(),
        protocol_id: AGENT_SEMANTIC_CLIENT_CACHE_MANIFEST_PROTOCOL_ID.into(),
        protocol_version: AGENT_SEMANTIC_CLIENT_CACHE_MANIFEST_PROTOCOL_VERSION.into(),
        cache_root: ClientCachePath::from_path(&cache_root),
        generations: vec![generation],
    };
    std::fs::create_dir_all(manifest_path.parent().expect("manifest parent"))
        .expect("create cache dir");
    let manifest_bytes = serde_json::to_vec_pretty(&manifest).expect("manifest json");
    std::fs::write(&manifest_path, manifest_bytes).expect("write manifest");
    let artifact_path =
        artifacts_root_from_cache_root(&cache_root).join("structural-index/rust-index-import.json");
    std::fs::create_dir_all(artifact_path.parent().expect("artifact parent"))
        .expect("create artifact dir");
    let packet_bytes =
        serde_json::to_vec(&structural_index_packet(&root)).expect("structural packet");
    std::fs::write(&artifact_path, packet_bytes).expect("write structural artifact");

    run_cache(&root, None, &["import".to_string()], false).expect("cache import");

    let engine = ClientDbEngine::resolve(&root).expect("resolve DB Engine");
    let source_snapshot = crate::source_index::current_source_index_snapshot(&root)
        .expect("capture imported structural source snapshot");
    let symbols = search_turso_documents(
        &engine,
        &source_snapshot.source_snapshot,
        "cache_imported_symbol",
        8,
    );

    assert_eq!(symbols.len(), 1);
    assert!(
        symbols[0].document.contains("src/lib.rs"),
        "structural search document should retain owner path: {:?}",
        symbols[0]
    );
    let _ = std::fs::remove_dir_all(root);
}

fn structural_index_packet(root: &std::path::Path) -> Value {
    json!({
        "schemaId": "agent.semantic-protocols.semantic-structural-index",
        "schemaVersion": "1",
        "protocolId": "agent.semantic-protocols.semantic-language",
        "protocolVersion": "1",
        "generationId": "rust-index-import",
        "languageId": "rust",
        "providerId": "rs-harness",
        "providerVersion": "0.1.0",
        "exportMethod": "index/structural",
        "projectRoot": root.display().to_string(),
        "packageRoot": ".",
        "sourceAuthority": "native-parser",
        "sourceArtifactId": "structural-index/rust-index-import.json",
        "rawSourceStored": false,
        "fileHashes": [
            {
                "path": "src/lib.rs",
                "sha256": "2".repeat(64),
                "byteLen": 0,
                "mtimeMs": 0,
                "source": "provider"
            }
        ],
        "owners": [
            {
                "ownerPath": "src/lib.rs",
                "ownerKind": "source",
                "sourceAuthority": "native-parser",
                "location": {"path": "src/lib.rs", "lineRange": "1:20"},
                "queryKeys": ["cache_imported_symbol"]
            }
        ],
        "symbols": [
            {
                "ownerPath": "src/lib.rs",
                "name": "cache_imported_symbol",
                "qualifiedName": "crate::cache_imported_symbol",
                "kind": "function",
                "visibility": "public",
                "sourceLocator": "src/lib.rs:3:4",
                "queryKeys": ["cache_imported_symbol", "cache import"]
            }
        ],
        "dependencyUsages": []
    })
}

fn temp_root(label: &str) -> std::path::PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time")
        .as_nanos();
    let root = std::env::temp_dir().join(format!("agent-client-cache-cli-{label}-{nanos}"));
    std::fs::create_dir_all(&root).expect("create temp project root");
    git(&root, ["init", "-q"]);
    root
}

fn git<const N: usize>(cwd: &Path, args: [&str; N]) {
    let status = Command::new("git")
        .args(args)
        .current_dir(cwd)
        .status()
        .expect("run git");
    assert!(status.success(), "git failed in {}", cwd.display());
}
