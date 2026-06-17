use crate::cache_cli::run_cache;
use agent_semantic_client_core::{
    AGENT_SEMANTIC_CLIENT_CACHE_MANIFEST_PROTOCOL_ID,
    AGENT_SEMANTIC_CLIENT_CACHE_MANIFEST_PROTOCOL_VERSION,
    AGENT_SEMANTIC_CLIENT_CACHE_MANIFEST_SCHEMA_ID,
    AGENT_SEMANTIC_CLIENT_CACHE_MANIFEST_SCHEMA_VERSION, CacheArtifactId, CacheStatus,
    ClientCacheFileHash, ClientCacheGeneration, ClientCacheManifest, ClientCachePath, LanguageId,
    ProviderId,
};
use agent_semantic_client_db::{
    ClientDb, ClientDbStructuralIndexLookup, ClientDbStructuralQueryKey,
};
use serde_json::{Value, json};
use std::{
    path::Path,
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};

#[test]
fn cache_usage_lists_flush() {
    let root = temp_root("usage");
    let error = run_cache(&root, &["unknown".to_string()], false).expect_err("usage");

    assert!(error.contains("status|import|source-index refresh|invalidate|flush [syntax-rows]"));
    assert!(error.contains("runtime-source acquire --language-id <id>"));
}

#[test]
fn cache_runtime_source_acquire_clones_versioned_source() {
    let root = temp_root("runtime-source-acquire");
    let upstream = root.join("upstream-gerbil");
    create_tagged_repo(&upstream, "v0.18.2");

    run_cache(
        &root,
        &[
            "runtime-source".to_string(),
            "acquire".to_string(),
            "--language-id".to_string(),
            "gerbil-scheme".to_string(),
            "--repository".to_string(),
            upstream.display().to_string(),
            "--checkout".to_string(),
            "v0.18.2".to_string(),
            "--state-namespace".to_string(),
            "runtime-source/gerbil-scheme".to_string(),
            "--index-owner".to_string(),
            "asp-structural-index".to_string(),
        ],
        false,
    )
    .expect("runtime source acquire");

    let checkout_dir =
        root.join(".cache/agent-semantic-protocol/client/runtime-source/gerbil-scheme/v0.18.2");
    assert_eq!(
        std::fs::read_to_string(checkout_dir.join("runtime.ss")).expect("runtime source file"),
        ";; runtime source fixture\n"
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn cache_runtime_source_acquire_requires_checkout() {
    let root = temp_root("runtime-source-missing-checkout");
    let error = run_cache(
        &root,
        &[
            "runtime-source".to_string(),
            "acquire".to_string(),
            "--language-id".to_string(),
            "gerbil-scheme".to_string(),
            "--repository".to_string(),
            "https://git.cons.io/mighty-gerbils/gerbil".to_string(),
            "--state-namespace".to_string(),
            "runtime-source/gerbil-scheme".to_string(),
            "--index-owner".to_string(),
            "asp-structural-index".to_string(),
        ],
        false,
    )
    .expect_err("missing checkout");

    assert_eq!(error, "--checkout is required");
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

    run_cache(&root, &["flush".to_string()], false).expect("flush invalid manifest");

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
    let artifact_path = cache_root
        .parent()
        .expect("cache root parent")
        .join("artifacts/structural-index/rust-index-import.json");
    std::fs::create_dir_all(artifact_path.parent().expect("artifact parent"))
        .expect("create artifact dir");
    let packet_bytes =
        serde_json::to_vec(&structural_index_packet(&root)).expect("structural packet");
    std::fs::write(&artifact_path, packet_bytes).expect("write structural artifact");

    run_cache(&root, &["import".to_string()], false).expect("cache import");

    let db_path = ClientDb::default_path(&cache_root);
    let db = ClientDb::open_read_only_existing(&db_path)
        .expect("open db")
        .expect("db exists");
    let summary = db.summary().expect("summary");
    let symbols = db
        .lookup_structural_symbols(&ClientDbStructuralIndexLookup {
            language_id: LanguageId::from("rust"),
            provider_id: "rs-harness".into(),
            project_root: root.clone(),
            query: ClientDbStructuralQueryKey::from("cache_imported_symbol"),
            limit: 8,
        })
        .expect("lookup structural symbol");

    assert_eq!(summary.structural_index_generation_count, 1);
    assert_eq!(summary.structural_index_symbol_count, 1);
    assert_eq!(symbols.len(), 1);
    assert_eq!(symbols[0].owner_path.as_str(), "src/lib.rs");
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
    std::fs::create_dir_all(root.join(".git")).expect("create temp project root");
    root
}

fn create_tagged_repo(repo: &Path, tag: &str) {
    std::fs::create_dir_all(repo).expect("create upstream repo");
    git(repo, ["init"]);
    std::fs::write(repo.join("runtime.ss"), ";; runtime source fixture\n").expect("write fixture");
    git(repo, ["add", "."]);
    git(
        repo,
        [
            "-c",
            "user.name=ASP Test",
            "-c",
            "user.email=asp-test@example.invalid",
            "commit",
            "-m",
            "runtime source fixture",
        ],
    );
    git(repo, ["tag", tag]);
}

fn git<const N: usize>(cwd: &Path, args: [&str; N]) {
    let status = Command::new("git")
        .args(args)
        .current_dir(cwd)
        .status()
        .expect("run git");
    assert!(status.success(), "git failed in {}", cwd.display());
}
