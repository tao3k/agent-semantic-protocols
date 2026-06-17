use crate::cache_cli::{generation_file_hashes_match, provider_cache_probe};
use agent_semantic_client_core::{
    CacheArtifactId, CacheExportMethod, CacheStatus, ClientCacheFileHash, ClientCacheGeneration,
    ClientCacheManifest, ClientMethod, ClientRequest, LanguageId, ProviderExecution, ProviderId,
    ProviderRegistrySnapshot, ResolvedProvider, SemanticSchemaId,
};
use agent_semantic_client_db::{ClientDb, ClientDbGenerationHit};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

#[test]
fn generation_file_hashes_detect_changed_source() {
    let root = temp_root("changed-source");
    let source_path = root.join("src/lib.rs");
    std::fs::create_dir_all(source_path.parent().expect("source parent")).expect("mkdir");
    std::fs::write(&source_path, b"fn cached() {}\n").expect("write source");
    let hit = generation_hit(&root, vec![file_hash("src/lib.rs", b"fn cached() {}\n")]);

    assert!(generation_file_hashes_match(&root, &hit));

    std::fs::write(&source_path, b"fn changed() {}\n").expect("rewrite source");

    assert!(!generation_file_hashes_match(&root, &hit));
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn generation_without_file_hash_evidence_is_stale() {
    let root = temp_root("missing-evidence");
    let hit = generation_hit(&root, Vec::new());

    assert!(!generation_file_hashes_match(&root, &hit));
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn hook_direct_source_read_bypasses_cache_probe() {
    let root = temp_root("hook-direct-source-read-no-probe");
    let snapshot = ProviderRegistrySnapshot {
        activation_path: root.join(".cache/agent-semantic-protocol/hooks/activation.json"),
        providers: vec![rust_provider()],
    };
    let request = ClientRequest::new(ClientMethod::Query, &root)
        .with_language(LanguageId::from("rust"))
        .with_forwarded_args(vec![
            "--from-hook".to_string(),
            "direct-source-read".to_string(),
            "--selector".to_string(),
            "src/lib.rs:1:12".to_string(),
            "--code".to_string(),
            ".".to_string(),
        ]);

    assert!(provider_cache_probe(&root, &snapshot, &request).is_none());
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn tree_sitter_rows_replay_when_latest_unrelated_generation_is_stale() {
    let root = temp_root("syntax-row-replay-beats-unrelated-stale-generation");
    std::fs::create_dir_all(root.join(".git")).expect("create git marker");
    std::fs::create_dir_all(root.join("src")).expect("create src dir");
    let cache_root = ClientCacheManifest::inspect_project(&root)
        .cache_root
        .expect("cache root");
    let db_path = ClientDb::default_path(&cache_root);
    std::fs::write(
        root.join("src/lib.rs"),
        "pub fn parse_query() -> usize {\n    1\n}\n",
    )
    .expect("write lib source");
    std::fs::write(root.join("src/other.rs"), "pub fn stale() {}\n").expect("write other source");

    let fresh_generation = syntax_generation(
        &root,
        "syntax-row-fresh",
        vec![hash_project_file(&root, "src/lib.rs")],
    );
    let fresh_manifest = manifest_from_generation(&cache_root, fresh_generation.clone());
    let packet_bytes = serde_json::to_vec(&syntax_packet_with_matches()).expect("packet bytes");
    let mut db = ClientDb::open_or_create(&db_path).expect("open db");
    db.import_manifest(&fresh_manifest)
        .expect("import fresh manifest");
    db.import_semantic_tree_sitter_query_packet(&fresh_generation, &packet_bytes)
        .expect("import fresh rows");

    std::thread::sleep(Duration::from_secs(1));
    let stale_generation = syntax_generation(
        &root,
        "syntax-row-stale-latest",
        vec![hash_project_file(&root, "src/other.rs")],
    );
    let stale_manifest = manifest_from_generation(&cache_root, stale_generation);
    db.import_manifest(&stale_manifest)
        .expect("import stale manifest");
    std::fs::write(root.join("src/other.rs"), "pub fn changed() {}\n")
        .expect("mutate stale source");
    drop(db);

    let snapshot = ProviderRegistrySnapshot {
        activation_path: root.join(".cache/agent-semantic-protocol/hooks/activation.json"),
        providers: vec![rust_provider()],
    };
    let request = ClientRequest::new(ClientMethod::Query, &root).with_forwarded_args(vec![
        "--treesitter-query".to_string(),
        "(function_item name: (identifier) @function.name)".to_string(),
        "--selector".to_string(),
        "src/lib.rs:1:80".to_string(),
        ".".to_string(),
    ]);

    let probe = provider_cache_probe(&root, &snapshot, &request).expect("probe");
    let replay = probe.replay.as_ref().expect("row replay");
    let stdout = std::str::from_utf8(replay.stdout.as_ref()).expect("utf8");

    assert_eq!(probe.cache_status, CacheStatus::Hit);
    assert!(stdout.contains("C=capture:function.name(parse_query)@src/lib.rs:10!code"));
    assert_eq!(replay.sqlite_read_count, 1);
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn tree_sitter_rows_are_stale_when_matching_source_hash_changes() {
    let root = temp_root("syntax-row-matching-stale-source");
    std::fs::create_dir_all(root.join(".git")).expect("create git marker");
    std::fs::create_dir_all(root.join("src")).expect("create src dir");
    let cache_root = ClientCacheManifest::inspect_project(&root)
        .cache_root
        .expect("cache root");
    let db_path = ClientDb::default_path(&cache_root);
    std::fs::write(
        root.join("src/lib.rs"),
        "pub fn parse_query() -> usize {\n    1\n}\n",
    )
    .expect("write lib source");

    let generation = syntax_generation(
        &root,
        "syntax-row-stale-source",
        vec![hash_project_file(&root, "src/lib.rs")],
    );
    let manifest = manifest_from_generation(&cache_root, generation.clone());
    let packet_bytes = serde_json::to_vec(&syntax_packet_with_matches()).expect("packet bytes");
    let mut db = ClientDb::open_or_create(&db_path).expect("open db");
    db.import_manifest(&manifest).expect("import manifest");
    db.import_semantic_tree_sitter_query_packet(&generation, &packet_bytes)
        .expect("import syntax rows");
    std::fs::write(root.join("src/lib.rs"), "pub fn changed() {}\n").expect("mutate source");

    let snapshot = ProviderRegistrySnapshot {
        activation_path: root.join(".cache/agent-semantic-protocol/hooks/activation.json"),
        providers: vec![rust_provider()],
    };
    let request = ClientRequest::new(ClientMethod::Query, &root).with_forwarded_args(vec![
        "--treesitter-query".to_string(),
        "(function_item name: (identifier) @function.name)".to_string(),
        "--selector".to_string(),
        "src/lib.rs:1:80".to_string(),
        ".".to_string(),
    ]);

    let probe = provider_cache_probe(&root, &snapshot, &request).expect("probe");

    assert_eq!(probe.cache_status, CacheStatus::Stale);
    assert!(probe.replay.is_none());
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn prime_seed_probe_rejects_latest_fresh_prime_generation_after_fingerprint_miss() {
    let root = temp_root("fresh-prime-reuse");
    std::fs::create_dir_all(root.join(".git")).expect("create git marker");
    std::fs::create_dir_all(root.join("src")).expect("create src dir");
    std::fs::write(root.join("src/lib.rs"), "pub fn cached_prime() {}\n").expect("write source");
    let cache_root = ClientCacheManifest::inspect_project(&root)
        .cache_root
        .expect("cache root");
    let db_path = ClientDb::default_path(&cache_root);
    let stdout = "[search-prime] root=. view=seeds alg=seed-frontier\n\
legend: ID=kind:role(value)!next; edge SRC>{DST:rel}; frontier ID.next\n\
aliases=G:search,O:owner\n\
O=owner:path(src/lib.rs)!owner\n\
G>{O:selects}\n\
rank=O frontier=O.owner\n";
    write_search_output_artifact(&cache_root, "prime-fresh.txt", stdout);

    let generation = search_prime_generation(
        &root,
        "fresh-prime",
        vec![hash_project_file(&root, "src/lib.rs")],
    );
    let manifest = manifest_from_generation(&cache_root, generation);
    let mut db = ClientDb::open_or_create(&db_path).expect("open db");
    db.import_manifest(&manifest).expect("import manifest");
    drop(db);

    let snapshot = ProviderRegistrySnapshot {
        activation_path: root.join(".cache/agent-semantic-protocol/hooks/activation.json"),
        providers: vec![rust_provider()],
    };
    let request = ClientRequest::new(ClientMethod::Search, &root).with_forwarded_args(vec![
        "prime".to_string(),
        "--view".to_string(),
        "seeds".to_string(),
        ".".to_string(),
    ]);

    let probe = provider_cache_probe(&root, &snapshot, &request).expect("probe");

    assert_eq!(probe.cache_status, CacheStatus::Miss);
    assert!(probe.replay.is_none());
    assert_eq!(probe.sqlite_read_count, 3);
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn fzf_seed_probe_reuses_latest_fresh_matching_query_after_fingerprint_miss() {
    let root = temp_root("fresh-fzf-reuse");
    std::fs::create_dir_all(root.join(".git")).expect("create git marker");
    std::fs::create_dir_all(root.join("src")).expect("create src dir");
    std::fs::write(root.join("src/lib.rs"), "pub fn cached_fzf() {}\n").expect("write source");
    let cache_root = ClientCacheManifest::inspect_project(&root)
        .cache_root
        .expect("cache root");
    let db_path = ClientDb::default_path(&cache_root);
    let stdout = "[search-fzf] q=cache replay view=seeds alg=seed-frontier\n\
legend: ID=kind:role(value)!next; edge SRC>{DST:rel}; frontier ID.next\n\
aliases=G:search,Q:query\n\
Q=query:term(cache replay)!fzf\n\
G>{Q:matches}\n\
rank=Q frontier=Q.fzf\n";
    write_search_packet_artifact(&cache_root, "fzf-fresh.json", "cache replay");
    write_search_output_artifact(&cache_root, "fzf-fresh.txt", stdout);
    write_search_packet_artifact(&cache_root, "fzf-unrelated.json", "different query");
    write_search_output_artifact(
        &cache_root,
        "fzf-unrelated.txt",
        "[search-fzf] q=different query view=seeds alg=seed-frontier\n",
    );

    let generation = search_fzf_generation_with_artifacts(
        &root,
        "fresh-fzf",
        vec![hash_project_file(&root, "src/lib.rs")],
        "fzf-fresh.json",
        "fzf-fresh.txt",
    );
    let manifest = manifest_from_generation(&cache_root, generation);
    let mut db = ClientDb::open_or_create(&db_path).expect("open db");
    db.import_manifest(&manifest).expect("import manifest");
    std::thread::sleep(Duration::from_secs(1));
    let unrelated_generation = search_fzf_generation_with_artifacts(
        &root,
        "fresh-fzf-unrelated-latest",
        vec![hash_project_file(&root, "src/lib.rs")],
        "fzf-unrelated.json",
        "fzf-unrelated.txt",
    );
    let unrelated_manifest = manifest_from_generation(&cache_root, unrelated_generation);
    db.import_manifest(&unrelated_manifest)
        .expect("import unrelated latest manifest");
    drop(db);

    let snapshot = ProviderRegistrySnapshot {
        activation_path: root.join(".cache/agent-semantic-protocol/hooks/activation.json"),
        providers: vec![rust_provider()],
    };
    let request = ClientRequest::new(ClientMethod::Search, &root).with_forwarded_args(vec![
        "fzf".to_string(),
        "cache replay".to_string(),
        "--view=seeds".to_string(),
        ".".to_string(),
    ]);

    let probe = provider_cache_probe(&root, &snapshot, &request).expect("probe");
    let replay = probe.replay.as_ref().expect("fresh fzf replay");

    assert_eq!(probe.cache_status, CacheStatus::Hit);
    assert_eq!(
        std::str::from_utf8(replay.stdout.as_ref()).expect("utf8"),
        stdout
    );
    assert_eq!(probe.sqlite_read_count, 3);
    let _ = std::fs::remove_dir_all(root);
}

fn generation_hit(
    root: &std::path::Path,
    file_hashes: Vec<ClientCacheFileHash>,
) -> ClientDbGenerationHit {
    ClientDbGenerationHit {
        language_id: LanguageId::from("rust"),
        provider_id: ProviderId::from("rs-harness"),
        project_root: root.to_path_buf(),
        export_method: CacheExportMethod::from("query/tree-sitter"),
        schema_ids: vec![SemanticSchemaId::from(
            "agent.semantic-protocols.semantic-tree-sitter-query",
        )],
        request_fingerprint: Some("fnv64:0123456789abcdef".to_string()),
        file_hashes,
        artifact_ids: vec![CacheArtifactId::from(
            "semantic-tree-sitter-query/rust-query.json",
        )],
    }
}

fn file_hash(path: &str, bytes: &[u8]) -> ClientCacheFileHash {
    let digest = Sha256::digest(bytes);
    ClientCacheFileHash {
        path: path.to_string(),
        sha256: format!("{digest:x}"),
    }
}

fn temp_root(label: &str) -> std::path::PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time")
        .as_nanos();
    std::env::temp_dir().join(format!("agent-client-probe-{label}-{nanos}"))
}

fn syntax_generation(
    root: &std::path::Path,
    generation_id: &str,
    file_hashes: Vec<ClientCacheFileHash>,
) -> ClientCacheGeneration {
    serde_json::from_value(json!({
        "generationId": generation_id,
        "languageId": "rust",
        "providerId": "rs-harness",
        "providerVersion": "0.1.0",
        "exportMethod": "query/tree-sitter",
        "projectRoot": root.display().to_string(),
        "packageRoot": ".",
        "schemaIds": ["agent.semantic-protocols.semantic-tree-sitter-query"],
        "cacheStatus": "hit",
        "rawSourceStored": false,
        "requestFingerprint": format!("fnv64:{generation_id}"),
        "fileHashes": file_hashes,
        "artifactIds": [format!("semantic-tree-sitter-query/{generation_id}.json")]
    }))
    .expect("syntax generation")
}

fn search_prime_generation(
    root: &std::path::Path,
    generation_id: &str,
    file_hashes: Vec<ClientCacheFileHash>,
) -> ClientCacheGeneration {
    serde_json::from_value(json!({
        "generationId": generation_id,
        "languageId": "rust",
        "providerId": "rs-harness",
        "providerVersion": "0.1.0",
        "exportMethod": "search/prime",
        "projectRoot": root.display().to_string(),
        "packageRoot": ".",
        "schemaIds": ["agent.semantic-protocols.semantic-search-packet"],
        "cacheStatus": "hit",
        "rawSourceStored": false,
        "requestFingerprint": format!("fnv64:historical-{generation_id}"),
        "fileHashes": file_hashes,
        "artifactIds": ["search-output/prime-fresh.txt"]
    }))
    .expect("search prime generation")
}

fn search_fzf_generation_with_artifacts(
    root: &std::path::Path,
    generation_id: &str,
    file_hashes: Vec<ClientCacheFileHash>,
    packet_file_name: &str,
    output_file_name: &str,
) -> ClientCacheGeneration {
    serde_json::from_value(json!({
        "generationId": generation_id,
        "languageId": "rust",
        "providerId": "rs-harness",
        "providerVersion": "0.1.0",
        "exportMethod": "search/fzf",
        "projectRoot": root.display().to_string(),
        "packageRoot": ".",
        "schemaIds": ["agent.semantic-protocols.semantic-search-packet"],
        "cacheStatus": "hit",
        "rawSourceStored": false,
        "requestFingerprint": format!("fnv64:historical-{generation_id}"),
        "fileHashes": file_hashes,
        "artifactIds": [
            format!("search/{packet_file_name}"),
            format!("search-output/{output_file_name}")
        ]
    }))
    .expect("search fzf generation")
}

fn manifest_from_generation(
    cache_root: &std::path::Path,
    generation: ClientCacheGeneration,
) -> ClientCacheManifest {
    ClientCacheManifest {
        schema_id: "agent.semantic-protocols.client-cache-manifest".into(),
        schema_version: "1".into(),
        protocol_id: "agent.semantic-protocols.client".into(),
        protocol_version: "1".into(),
        cache_root: cache_root.display().to_string().into(),
        generations: vec![generation],
    }
}

fn hash_project_file(root: &std::path::Path, path: &str) -> ClientCacheFileHash {
    let bytes = std::fs::read(root.join(path)).expect("read source for hash");
    file_hash(path, &bytes)
}

fn write_search_output_artifact(cache_root: &std::path::Path, file_name: &str, stdout: &str) {
    let search_output_dir = cache_root
        .parent()
        .expect("cache root parent")
        .join("artifacts/search-output");
    std::fs::create_dir_all(&search_output_dir).expect("create search output dir");
    std::fs::write(search_output_dir.join(file_name), stdout).expect("write search output");
}

fn write_search_packet_artifact(cache_root: &std::path::Path, file_name: &str, query: &str) {
    let search_dir = cache_root
        .parent()
        .expect("cache root parent")
        .join("artifacts/search");
    std::fs::create_dir_all(&search_dir).expect("create search packet dir");
    let packet = json!({
        "schemaId": "agent.semantic-protocols.semantic-search-packet",
        "method": "search/fzf",
        "query": query,
        "nodes": []
    });
    let bytes = serde_json::to_vec(&packet).expect("search packet bytes");
    std::fs::write(search_dir.join(file_name), bytes).expect("write search packet");
}

fn syntax_packet_with_matches() -> Value {
    json!({
        "schemaId": "agent.semantic-protocols.semantic-tree-sitter-query",
        "method": "query",
        "languageId": "rust",
        "providerId": "rs-harness",
        "grammarId": "tree-sitter-rust",
        "grammarProfileVersion": "2026-06-04.v1",
        "query": {
            "input": "(function_item name: (identifier) @function.name)",
            "inputForm": "s-expression",
            "dialect": "tree-sitter-query",
            "compiledSource": "(function_item name: (identifier) @function.name)",
            "fields": {
                "selector": "src/lib.rs:1:80",
                "codeOutput": false,
                "captures": ["function.name"]
            }
        },
        "matches": [
            {
                "id": "m1",
                "range": {"path": "src/lib.rs", "lineRange": "10:12"},
                "captures": [
                    {
                        "id": "c1",
                        "name": "function.name",
                        "nodeType": "identifier",
                        "range": {"path": "src/lib.rs", "lineRange": "10:10"},
                        "fields": {"symbol": "parse_query"}
                    }
                ]
            }
        ],
        "truncated": false,
        "cache": {
            "artifactKind": "semantic-tree-sitter-query",
            "rawSourceStored": false
        }
    })
}

fn rust_provider() -> ResolvedProvider {
    ResolvedProvider {
        language_id: LanguageId::from("rust"),
        provider_id: ProviderId::from("rs-harness"),
        binary: "rs-harness".to_string(),
        execution: ProviderExecution::ExternalProcess,
        provider_command_prefix: Vec::new(),
        runtime_command_argv: None,
        runtime_profile_status: None,
        package_roots: vec![".".to_string()],
        source_roots: vec!["src".to_string()],
        config_files: vec!["Cargo.toml".to_string()],
        source_extensions: vec!["rs".to_string()],
        ignored_path_prefixes: Vec::new(),
    }
}
