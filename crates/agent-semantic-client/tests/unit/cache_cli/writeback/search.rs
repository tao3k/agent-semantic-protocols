use agent_semantic_client_core::{
    CacheArtifactId, CacheGenerationId, CacheStatus, ClientCacheGeneration, ClientMethod,
    ClientRequest, LanguageId, ProviderId, ProviderRegistrySnapshot, SemanticSchemaId,
};
use serde_json::json;
use sha2::{Digest, Sha256};

use super::{rust_provider, temp_root};
use crate::cache_cli::writeback::{
    maybe_write_search_output_artifact, search_output_file_hashes,
    write_search_packet_cache_after_provider_success,
};

#[test]
fn search_output_writeback_adds_replay_ready_stdout_artifact() {
    let root = temp_root("search-output-writeback");
    let cache_root = root.join("client");
    let mut generation = ClientCacheGeneration {
        generation_id: CacheGenerationId::from("rust-search-fzf-abc123"),
        language_id: LanguageId::from("rust"),
        provider_id: ProviderId::from("rs-harness"),
        provider_version: None,
        export_method: Some("search/fzf".to_string()),
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
            "search/rust-search-fzf-abc123.json",
        )]),
    };
    let stdout = "[search-fzf] q=cache view=fzf alg=seed-frontier\n\
legend: ID=kind:role(value)!next; edge SRC>{DST:rel}; frontier ID.next\n\
aliases=G:search,Q:query\n\
Q=query:term(cache)!fzf\n\
G>{Q:matches}\n\
rank=Q frontier=Q.fzf\n";

    maybe_write_search_output_artifact(&cache_root, &mut generation, stdout.as_bytes());

    let artifact_ids = generation.artifact_ids.expect("artifact ids");
    assert!(
        artifact_ids.iter().any(|artifact_id| {
            artifact_id.as_str() == "search-output/rust-search-fzf-abc123.txt"
        })
    );
    assert_eq!(
        std::fs::read_to_string(root.join("artifacts/search-output/rust-search-fzf-abc123.txt"))
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
    let stdout = "[search-fzf] q=cache view=fzf alg=seed-frontier\n\
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
|decision purpose=decision-primer answer=false code=false capabilities=pipe,fzf,fd-query,rg-query,owner-items,selector-code,treesitter-query ladder=pipe>fzf>fd-query|rg-query>owner-items>selector-code history=asp-artifacts:directReadRisk,repeatedPrime,repeatedPipe,bestPath risk=broad-direct-read,manual-window-scan,repeat-prime next=\"asp rust search pipe '<question-or-feature-term>' --view seeds .\"\n\
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
    assert_eq!(probe.sqlite_write_count, 2);
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn dependency_search_packet_writeback_replays_rendered_stdout_artifact() {
    let _guard = crate::test_support::CACHE_TEST_LOCK
        .lock()
        .expect("cache test lock");
    let root = temp_root("dependency-search-packet-writeback");
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
            "deps".to_string(),
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
    assert_eq!(probe.sqlite_write_count, 2);
    let _ = std::fs::remove_dir_all(root);
}
