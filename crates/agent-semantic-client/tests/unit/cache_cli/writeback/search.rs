use agent_semantic_client_core::{
    CacheArtifactId, CacheGenerationId, CacheStatus, ClientCacheGeneration, ClientCacheManifest,
    ClientMethod, ClientRequest, LanguageId, ProviderId, ProviderRegistrySnapshot,
    SemanticSchemaId, project_client_cache_manifest_path,
};
use agent_semantic_client_db::ClientDbEngine;
use serde_json::json;
use sha2::{Digest, Sha256};

use super::{rust_provider, temp_root};
use crate::cache_cli::writeback::{
    maybe_write_search_output_artifact, search_output_file_hashes,
    write_search_packet_cache_after_provider_success,
};
use crate::test_support::EnvVarGuard;
use crate::test_support::v2_cache_root;

#[test]
fn search_output_writeback_adds_replay_ready_stdout_artifact() {
    let root = temp_root("search-output-writeback");
    let cache_root = v2_cache_root(&root);
    let mut generation = ClientCacheGeneration {
        generation_id: CacheGenerationId::from("rust-search-lexical-abc123"),
        language_id: LanguageId::from("rust"),
        provider_id: ProviderId::from("rs-harness"),
        provider_version: None,
        export_method: Some("search/lexical".to_string()),
        project_root: root.display().to_string(),
        package_root: Some(".".to_string()),
        schema_ids: vec![SemanticSchemaId::from(
            "agent.semantic-protocols.semantic-search-packet",
        )],
        cache_status: CacheStatus::Hit,
        raw_source_stored: false,
        request_fingerprint: None,
        file_hashes: None,
        artifact_ids: Some(vec![CacheArtifactId::from(
            "search/rust-search-lexical-abc123.json",
        )]),
    };
    let stdout = "[search-lexical] q=cache view=lexical alg=seed-frontier\n\
legend: ID=kind:role(value)!next; edge SRC>{DST:rel}; frontier ID.next\n\
aliases=G:search,Q:query\n\
Q=query:term(cache)!lexical\n\
G>{Q:matches}\n\
rank=Q frontier=Q.lexical\n";

    maybe_write_search_output_artifact(&cache_root, &mut generation, stdout.as_bytes());

    let artifact_ids = generation.artifact_ids.expect("artifact ids");
    assert!(artifact_ids.iter().any(|artifact_id| {
        artifact_id.as_str() == "search-output/rust-search-lexical-abc123.txt"
    }));
    assert_eq!(
        std::fs::read_to_string(
            root.join("artifacts/search-output/rust-search-lexical-abc123.txt")
        )
        .expect("search output artifact"),
        stdout
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn search_output_file_hashes_use_prompt_facing_locators() {
    let root = temp_root("search-output-file-hashes");
    std::fs::create_dir_all(root.join("crates/client/src")).expect("create src");
    std::fs::create_dir_all(root.join("crates/client/tests/unit")).expect("create tests");
    std::fs::write(
        root.join("crates/client/src/cache.rs"),
        "pub fn cache() {}\n",
    )
    .expect("write source");
    std::fs::write(
        root.join("crates/client/tests/unit/cache.rs"),
        "#[test] fn cache() {}\n",
    )
    .expect("write test");
    let package_roots = vec!["crates/client".to_string()];
    let stdout = "[search-lexical] q=cache view=lexical alg=seed-frontier\n\
legend: ID=kind:role(value)!next; edge SRC>{DST:rel}; frontier ID.next\n\
aliases=G:search,Q:query,O:owner,T:test\n\
O=owner:path(src/cache.rs)!owner;T=test:path(tests/unit/cache.rs)!tests\n";

    let file_hashes =
        search_output_file_hashes(&root, &package_roots, stdout.as_bytes()).expect("file hashes");

    assert_eq!(
        file_hashes
            .iter()
            .map(|file_hash| file_hash.path.as_str())
            .collect::<Vec<_>>(),
        vec![
            "crates/client/src/cache.rs",
            "crates/client/tests/unit/cache.rs"
        ]
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn search_packet_writeback_replays_rendered_stdout_artifact() {
    let _guard = crate::test_support::CACHE_TEST_LOCK
        .lock()
        .expect("cache test lock");
    let root = temp_root("search-packet-writeback");
    let _state_home = EnvVarGuard::set("ASP_STATE_HOME", root.join("state-home"));
    std::fs::create_dir_all(root.join(".git")).expect("create git marker");
    std::fs::create_dir_all(root.join("src")).expect("create src");
    let source = b"pub fn cached_prime() {}\n";
    std::fs::write(root.join("src/lib.rs"), source).expect("write source");
    let digest = Sha256::digest(source);
    let snapshot = ProviderRegistrySnapshot {
        activation_path: root.join("activation.json"),
        providers: vec![rust_provider()],
    };
    let request = ClientRequest::new(ClientMethod::Search, &root)
        .with_language(LanguageId::from("rust"))
        .with_forwarded_args(vec![
            "prime".to_string(),
            "--view".to_string(),
            "seeds".to_string(),
            ".".to_string(),
        ]);
    let packet = serde_json::to_vec(&json!({
        "schemaId": "agent.semantic-protocols.semantic-search-packet",
        "schemaVersion": "1",
        "languageId": "rust",
        "providerId": "rs-harness",
        "renderMode": "seeds",
        "query": "prime",
        "owners": [{"path": "src/lib.rs"}],
        "hits": [],
        "searchSynthesis": {"algorithm": "seed-frontier", "seeds": []},
        "cache": {"fileHashes": [{"path": "src/lib.rs", "sha256": format!("{digest:x}")}]}
    }))
    .expect("packet json");
    let rendered_stdout = "[search-prime] root=. view=prime alg=seed-frontier\n\
|decision purpose=decision-primer answer=false code=false capabilities=pipe,lexical,fd-query,rg-query,owner-items,selector-code,treesitter-query ladder=pipe>lexical>fd-query|rg-query>owner-items>selector-code history=asp-artifacts:directReadRisk,repeatedPrime,repeatedPipe,bestPath risk=broad-direct-read,manual-window-scan,repeat-prime next=\"asp rust search pipe '<question-or-feature-term>' --workspace . --view seeds\"\n\
legend: ID=kind:role(value)!next; edge SRC>{DST:rel}; frontier ID.next\n\
aliases=G:search,O:owner\n\
O=owner:path(src/lib.rs)!owner\n\
G>{O:selects}\n\
rank=O frontier=O.owner\n";

    let probe = write_search_packet_cache_after_provider_success(
        &root,
        &snapshot,
        &request,
        &packet,
        rendered_stdout.as_bytes(),
    )
    .expect("writeback probe");
    let replay = probe.replay.expect("search output replay");

    assert_eq!(replay.stdout, rendered_stdout.as_bytes());
    assert_eq!(probe.db_write_count, 2);
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn dependency_search_packet_writeback_replays_rendered_stdout_artifact() {
    dependency_search_packet_writeback_replays_rendered_stdout_artifact_for("deps");
    dependency_search_packet_writeback_replays_rendered_stdout_artifact_for("dependency");
}

#[test]
fn dependency_search_packet_without_locators_uses_manifest_hashes_for_replay() {
    let _guard = crate::test_support::CACHE_TEST_LOCK
        .lock()
        .expect("cache test lock");
    let root = temp_root("dependency-search-packet-manifest-hashes");
    std::fs::create_dir_all(root.join(".git")).expect("create git marker");
    std::fs::write(
        root.join("Cargo.toml"),
        "[package]\nname = \"fixture\"\nversion = \"0.1.0\"\n",
    )
    .expect("write manifest");
    let snapshot = ProviderRegistrySnapshot {
        activation_path: root.join("activation.json"),
        providers: vec![rust_provider()],
    };
    let request = ClientRequest::new(ClientMethod::Search, &root)
        .with_language(LanguageId::from("rust"))
        .with_forwarded_args(vec![
            "deps".to_string(),
            "serde".to_string(),
            "--view".to_string(),
            "seeds".to_string(),
            ".".to_string(),
        ]);
    let packet = serde_json::to_vec(&json!({
        "schemaId": "agent.semantic-protocols.semantic-search-packet",
        "schemaVersion": "1",
        "languageId": "rust",
        "providerId": "rs-harness",
        "renderMode": "deps",
        "query": "serde",
        "nodes": [{"id": "D", "kind": "dependency", "role": "pkg", "label": "serde"}],
        "edges": [],
        "searchSynthesis": {"algorithm": "dependency-frontier", "seeds": []}
    }))
    .expect("packet json");
    let rendered_stdout = "[search-dependency] q=serde view=hits alg=seed-frontier\n\
legend: ID=kind:role(value)!next; edge SRC>{DST:rel}; frontier ID.next\n\
aliases: graph:{G=search,D=dependency}\n\
D=dependency:pkg(serde)!dependency\n\
G>{D:uses}\n\
rank=D frontier=D.dependency\n";

    let probe = write_search_packet_cache_after_provider_success(
        &root,
        &snapshot,
        &request,
        &packet,
        rendered_stdout.as_bytes(),
    )
    .expect("writeback probe");
    let replay = probe.replay.expect("dependency search output replay");

    assert_eq!(replay.stdout, rendered_stdout.as_bytes());
    assert_eq!(probe.db_write_count, 2);
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn invalid_retired_manifest_is_discarded_and_rebuilt_on_writeback() {
    let _guard = crate::test_support::CACHE_TEST_LOCK
        .lock()
        .expect("cache test lock");
    let root = temp_root("invalid-retired-manifest-rebuild");
    std::fs::create_dir_all(root.join(".git")).expect("create git marker");
    std::fs::create_dir_all(root.join("src")).expect("create src");
    let source = b"use serde::Serialize;\n";
    std::fs::write(root.join("src/lib.rs"), source).expect("write source");
    let digest = Sha256::digest(source);
    let manifest_path = project_client_cache_manifest_path(&root).expect("manifest path");
    std::fs::create_dir_all(manifest_path.parent().expect("manifest parent"))
        .expect("create manifest parent");
    std::fs::write(
        &manifest_path,
        serde_json::to_vec_pretty(&json!({
            "schemaId": "agent.semantic-protocols.client-cache-manifest",
            "schemaVersion": "1",
            "protocolId": "agent.semantic-protocols.client",
            "protocolVersion": "1",
            "cacheRoot": manifest_path.parent().expect("manifest parent").display().to_string(),
            "generations": [{
                "generationId": "rust-search-deps-retired",
                "languageId": "rust",
                "providerId": "rs-harness",
                "exportMethod": "search/deps",
                "projectRoot": root.display().to_string(),
                "packageRoot": ".",
                "schemaIds": ["agent.semantic-protocols.semantic-search-packet"],
                "cacheStatus": "hit",
                "rawSourceStored": false,
                "requestFingerprint": "retired",
                "fileHashes": [{"path": "src/lib.rs", "sha256": format!("{digest:x}")}],
                "artifactIds": ["search/rust-search-deps-retired.json"]
            }]
        }))
        .expect("retired manifest json"),
    )
    .expect("write retired manifest");
    assert!(
        ClientCacheManifest::load_from_path(&manifest_path).is_err(),
        "retired manifest without metadata must be invalid"
    );
    let snapshot = ProviderRegistrySnapshot {
        activation_path: root.join("activation.json"),
        providers: vec![rust_provider()],
    };
    let stale_request = ClientRequest::new(ClientMethod::Search, &root)
        .with_language(LanguageId::from("rust"))
        .with_forwarded_args(vec![
            "deps".to_string(),
            "regex".to_string(),
            "--view".to_string(),
            "seeds".to_string(),
            ".".to_string(),
        ]);
    let stale_packet = serde_json::to_vec(&json!({
        "schemaId": "agent.semantic-protocols.semantic-search-packet",
        "schemaVersion": "1",
        "languageId": "rust",
        "providerId": "rs-harness",
        "renderMode": "deps",
        "query": "regex",
        "owners": [{"path": "src/lib.rs"}],
        "hits": [],
        "searchSynthesis": {"algorithm": "dependency-frontier", "seeds": []},
        "cache": {
            "fileHashes": [{
                "path": "src/lib.rs",
                "sha256": format!("{digest:x}"),
                "byteLen": source.len(),
                "mtimeMs": std::fs::metadata(root.join("src/lib.rs"))
                    .expect("source metadata")
                    .modified()
                    .expect("source modified")
                    .duration_since(std::time::UNIX_EPOCH)
                    .expect("source mtime epoch")
                    .as_millis()
            }]
        }
    }))
    .expect("stale packet json");
    write_search_packet_cache_after_provider_success(
        &root,
        &snapshot,
        &stale_request,
        &stale_packet,
        b"[search-deps] q=regex\n",
    )
    .expect("stale writeback probe");
    std::fs::write(
        &manifest_path,
        serde_json::to_vec_pretty(&json!({
            "schemaId": "agent.semantic-protocols.client-cache-manifest",
            "schemaVersion": "1",
            "protocolId": "agent.semantic-protocols.client",
            "protocolVersion": "1",
            "cacheRoot": manifest_path.parent().expect("manifest parent").display().to_string(),
            "generations": [{
                "generationId": "rust-search-deps-retired",
                "languageId": "rust",
                "providerId": "rs-harness",
                "exportMethod": "search/deps",
                "projectRoot": root.display().to_string(),
                "packageRoot": ".",
                "schemaIds": ["agent.semantic-protocols.semantic-search-packet"],
                "cacheStatus": "hit",
                "rawSourceStored": false,
                "requestFingerprint": "retired",
                "fileHashes": [{"path": "src/lib.rs", "sha256": format!("{digest:x}")}],
                "artifactIds": ["search/rust-search-deps-retired.json"]
            }]
        }))
        .expect("retired manifest json"),
    )
    .expect("rewrite retired manifest");
    assert!(
        ClientCacheManifest::load_from_path(&manifest_path).is_err(),
        "retired manifest without metadata must remain invalid after stale db setup"
    );
    let request = ClientRequest::new(ClientMethod::Search, &root)
        .with_language(LanguageId::from("rust"))
        .with_forwarded_args(vec![
            "deps".to_string(),
            "serde".to_string(),
            "--view".to_string(),
            "seeds".to_string(),
            ".".to_string(),
        ]);
    let packet = serde_json::to_vec(&json!({
        "schemaId": "agent.semantic-protocols.semantic-search-packet",
        "schemaVersion": "1",
        "languageId": "rust",
        "providerId": "rs-harness",
        "renderMode": "deps",
        "query": "serde",
        "owners": [{"path": "src/lib.rs"}],
        "hits": [],
        "searchSynthesis": {"algorithm": "dependency-frontier", "seeds": []},
        "cache": {
            "fileHashes": [{
                "path": "src/lib.rs",
                "sha256": format!("{digest:x}"),
                "byteLen": source.len(),
                "mtimeMs": std::fs::metadata(root.join("src/lib.rs"))
                    .expect("source metadata")
                    .modified()
                    .expect("source modified")
                    .duration_since(std::time::UNIX_EPOCH)
                    .expect("source mtime epoch")
                    .as_millis()
            }]
        }
    }))
    .expect("packet json");
    let rendered_stdout = "[search-deps] q=serde pkg=. dep=1 own=1 api=0\n\
|owner src/lib.rs hit_kind=dependency-api locations=1:1 next=owner:src/lib.rs\n";

    let probe = write_search_packet_cache_after_provider_success(
        &root,
        &snapshot,
        &request,
        &packet,
        rendered_stdout.as_bytes(),
    )
    .expect("writeback probe");
    let replay = probe.replay.expect("dependency search output replay");
    let manifest = ClientCacheManifest::load_from_path(&manifest_path).expect("rebuilt manifest");
    let db_report =
        ClientDbEngine::inspect_client_dir(manifest_path.parent().expect("manifest parent"));

    assert_eq!(replay.stdout, rendered_stdout.as_bytes());
    assert_eq!(manifest.generations.len(), 1);
    assert_eq!(db_report.generation_count, 1);
    assert_ne!(
        manifest.generations[0].generation_id.as_str(),
        "rust-search-deps-retired"
    );
    assert!(
        manifest.generations[0]
            .file_hashes
            .as_ref()
            .expect("file hashes")
            .iter()
            .all(|hash| hash.byte_len > 0 && hash.mtime_ms > 0)
    );
    let _ = std::fs::remove_dir_all(root);
}

fn dependency_search_packet_writeback_replays_rendered_stdout_artifact_for(view: &str) {
    let _guard = crate::test_support::CACHE_TEST_LOCK
        .lock()
        .expect("cache test lock");
    let root = temp_root(&format!("dependency-search-packet-writeback-{view}"));
    std::fs::create_dir_all(root.join(".git")).expect("create git marker");
    std::fs::create_dir_all(root.join("src")).expect("create src");
    let source = b"use serde::Serialize;\n#[derive(Serialize)]\npub struct Thing;\n";
    std::fs::write(root.join("src/lib.rs"), source).expect("write source");
    let digest = Sha256::digest(source);
    let snapshot = ProviderRegistrySnapshot {
        activation_path: root.join("activation.json"),
        providers: vec![rust_provider()],
    };
    let request = ClientRequest::new(ClientMethod::Search, &root)
        .with_language(LanguageId::from("rust"))
        .with_forwarded_args(vec![
            view.to_string(),
            "serde@1::Serialize".to_string(),
            ".".to_string(),
        ]);
    let packet = serde_json::to_vec(&json!({
        "schemaId": "agent.semantic-protocols.semantic-search-packet",
        "schemaVersion": "1",
        "languageId": "rust",
        "providerId": "rs-harness",
        "renderMode": "deps",
        "query": "serde@1::Serialize",
        "owners": [{"path": "src/lib.rs"}],
        "hits": [],
        "searchSynthesis": {"algorithm": "dependency-frontier", "seeds": []},
        "cache": {"fileHashes": [{"path": "src/lib.rs", "sha256": format!("{digest:x}")}]}
    }))
    .expect("packet json");
    let rendered_stdout = "[search-deps] q=serde@1::Serialize pkg=. dep=1 own=1 api=0 requestedVersion=1 currentWorkspaceVersion=1 versionScope=current apiQuery=Serialize\n\
|dep serde import=serde pkg=serde version=1 kind=normal opt=false source=manifest manager=cargo feat=derive\n\
|owner src/lib.rs hit_kind=dependency-api apiQuery=Serialize locations=1:1 next=owner:src/lib.rs\n\
|next dependency:serde,docs:serde::Serialize,text:Serialize,tests:Serialize\n";

    let probe = write_search_packet_cache_after_provider_success(
        &root,
        &snapshot,
        &request,
        &packet,
        rendered_stdout.as_bytes(),
    )
    .expect("writeback probe");
    let replay = probe.replay.expect("dependency search output replay");

    assert_eq!(replay.stdout, rendered_stdout.as_bytes());
    assert_eq!(probe.db_write_count, 2);
    let _ = std::fs::remove_dir_all(root);
}
