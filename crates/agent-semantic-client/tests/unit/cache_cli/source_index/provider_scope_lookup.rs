use super::fixtures::{
    EnvVarGuard, home_local_provider_path, isolate_home, make_executable, temp_root,
    write_gerbil_activation_with_provider_scope, write_rust_activation_with_ignored_prefixes,
};
use crate::cache_cli::run_cache;
use agent_semantic_client_core::{ASP_PROVIDER_ACTIVATION_PATH_ENV, LanguageId};
use agent_semantic_client_db::ClientDbEngine;

#[test]
fn exact_owner_snapshot_includes_provider_ignored_file_without_workspace_scan() {
    let _guard = crate::test_support::CACHE_TEST_LOCK
        .lock()
        .expect("cache test lock");
    let root = temp_root("exact-owner-provider-ignored");
    let _home_env = isolate_home(&root);
    std::fs::write(
        root.join("Cargo.toml"),
        "[package]\nname = \"exact-owner\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
    )
    .expect("write package manifest");
    std::fs::create_dir_all(root.join("src")).expect("create default source");
    std::fs::write(root.join("src/lib.rs"), "pub fn default_owner() {}\n")
        .expect("write default owner");
    std::fs::create_dir_all(root.join("benches")).expect("create explicit owner dir");
    std::fs::write(
        root.join("benches/client_microbench.rs"),
        "fn explicit_owner() {}\n",
    )
    .expect("write explicit owner");
    let activation_path = write_rust_activation_with_ignored_prefixes(&root, &["benches"]);
    let _activation_env = EnvVarGuard::set(
        ASP_PROVIDER_ACTIVATION_PATH_ENV,
        activation_path.as_os_str(),
    );

    let snapshot = crate::source_index::current_source_index_snapshot_for_owner(
        &root,
        "benches/client_microbench.rs",
        "rust",
        "rs-harness",
    )
    .expect("capture explicit owner snapshot");

    assert_eq!(snapshot.source_snapshot.leaf_count, 1);
    assert!(
        snapshot
            .workspace_snapshot
            .file_digest("benches/client_microbench.rs")
            .is_some()
    );
    assert_eq!(snapshot.source_blobs.len(), 1);
    assert!(
        snapshot
            .source_blobs
            .contains_key("benches/client_microbench.rs")
    );

    let error = match crate::source_index::current_source_index_snapshot_for_owner(
        &root,
        "../outside.rs",
        "rust",
        "rs-harness",
    ) {
        Ok(_) => panic!("parent traversal must fail"),
        Err(error) => error,
    };
    assert!(error.contains("reasonKind=owner-outside-workspace"));

    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn cache_source_index_refresh_respects_provider_ignored_path_prefixes() {
    let _guard = crate::test_support::CACHE_TEST_LOCK
        .lock()
        .expect("cache test lock");
    let root = temp_root("source-index-workspace-exclude");
    let _home_env = isolate_home(&root);
    std::fs::write(root.join("Cargo.toml"), "[workspace]\nmembers = [\"crates/app\", \"vendor/tool\"]\nexclude = [\"vendor/tool\"]\nresolver = \"2\"\n").expect("write workspace manifest");
    std::fs::create_dir_all(root.join("crates/app/src")).expect("create app source dir");
    std::fs::write(
        root.join("crates/app/Cargo.toml"),
        "[package]\nname = \"app\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
    )
    .expect("write app manifest");
    std::fs::write(
        root.join("crates/app/src/lib.rs"),
        "pub fn workspace_scope_symbol() {}\n",
    )
    .expect("write app source");
    std::fs::create_dir_all(root.join("vendor/tool/src")).expect("create excluded source dir");
    std::fs::write(
        root.join("vendor/tool/Cargo.toml"),
        "[package]\nname = \"tool\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
    )
    .expect("write excluded manifest");
    std::fs::write(
        root.join("vendor/tool/src/lib.rs"),
        "pub fn workspace_scope_symbol() {}\n",
    )
    .expect("write excluded source");
    let activation_path = write_rust_activation_with_ignored_prefixes(&root, &["vendor"]);
    let _activation_env = EnvVarGuard::set(
        ASP_PROVIDER_ACTIVATION_PATH_ENV,
        activation_path.as_os_str(),
    );
    run_cache(
        &root,
        None,
        &["source-index".to_string(), "rebuild".to_string()],
        false,
    )
    .expect("rebuild source index");
    let engine = ClientDbEngine::resolve(&root).expect("resolve DB Engine");
    assert!(engine.db_path().exists());
    let result = crate::test_support::lookup_current_source_index_for_language(
        &root,
        Some(&LanguageId::from("rust")),
        "workspace_scope_symbol",
        8,
    )
    .expect("lookup source index");
    assert_eq!(result.state.as_str(), "hit");
    assert_eq!(result.candidates.len(), 1);
    assert_eq!(result.candidates[0].path, "crates/app/src/lib.rs");
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn source_index_lookup_ranks_query_dense_owner_before_low_coverage_path() {
    let _guard = crate::test_support::CACHE_TEST_LOCK
        .lock()
        .expect("cache test lock");
    let root = temp_root("source-index-query-axis-rank");
    let _home_env = isolate_home(&root);
    std::fs::write(
        root.join("Cargo.toml"),
        "[workspace]\nmembers = [\"crates/app\"]\nresolver = \"2\"\n",
    )
    .expect("write workspace manifest");
    std::fs::create_dir_all(root.join("crates/app/src/semantic_sandtable"))
        .expect("create package source dir");
    std::fs::write(
        root.join("crates/app/Cargo.toml"),
        "[package]\nname = \"app\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
    )
    .expect("write app manifest");
    std::fs::write(
        root.join("crates/app/src/semantic_sandtable/surface.rs"),
        "pub fn ablation_surface() {}\n",
    )
    .expect("write low coverage source");
    std::fs::write(
        root.join("crates/app/src/semantic_sandtable/report_chain.rs"),
        "pub fn topology_membership_report_chain_request_policy() {}\n",
    )
    .expect("write report chain source");
    std::fs::create_dir_all(root.join("crates/app/src/semantic_sandtable/lifecycle-v1"))
        .expect("create lifecycle v1 source dir");
    std::fs::create_dir_all(root.join("crates/app/src/semantic_sandtable/docs"))
        .expect("create lifecycle docs source dir");
    std::fs::write(
        root.join(
            "crates/app/src/semantic_sandtable/docs/10-15-02-codex-resident-agent-lifecycle-v1.rs",
        ),
        "pub fn codex_resident_agent_lifecycle_v1() {}\n",
    )
    .expect("write lifecycle v1 source");
    std::fs::write(
        root.join(
            "crates/app/src/semantic_sandtable/docs/10-15-02-codex-resident-agent-lifecycle-v1.rs",
        ),
        "pub fn codex_resident_agent_lifecycle_v2() {}\n",
    )
    .expect("write lifecycle v2 source");
    let activation_path = write_rust_activation_with_ignored_prefixes(&root, &[]);
    let _activation_env = EnvVarGuard::set(
        ASP_PROVIDER_ACTIVATION_PATH_ENV,
        activation_path.as_os_str(),
    );
    run_cache(
        &root,
        None,
        &["source-index".to_string(), "rebuild".to_string()],
        false,
    )
    .expect("rebuild source index");
    let result = crate::test_support::lookup_current_source_index_for_language(
        &root,
        Some(&LanguageId::from("rust")),
        "ablation sandtable topology membership report chain request policy",
        8,
    )
    .expect("lookup source index");
    assert_eq!(result.state.as_str(), "hit");
    assert_eq!(
        result.candidates[0].path,
        "crates/app/src/semantic_sandtable/report_chain.rs"
    );
    assert!(
        result
            .candidates
            .iter()
            .any(|candidate| candidate.path == "crates/app/src/semantic_sandtable/surface.rs"),
        "{:?}",
        result.candidates
    );
    let versioned_alias = crate::test_support::lookup_current_source_index_for_language(
        &root,
        Some(&LanguageId::from("rust")),
        "10.15.02-codex-resident-agent-lifecycle-v2.org",
        8,
    )
    .expect("lookup lifecycle v2 alias source index");
    assert_eq!(versioned_alias.state.as_str(), "hit");
    assert!(versioned_alias.candidates.iter().any(|candidate| candidate.path == "crates/app/src/semantic_sandtable/docs/10-15-02-codex-resident-agent-lifecycle-v1.rs"), "{:?}", versioned_alias.candidates);
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn cache_source_index_refresh_prefers_provider_workspace_scope_packet() {
    let _guard = crate::test_support::CACHE_TEST_LOCK
        .lock()
        .expect("cache test lock");
    let root = temp_root("source-index-provider-workspace-scope");
    let _home_env = isolate_home(&root);
    std::fs::write(
        root.join("gerbil.pkg"),
        "(package source-index-provider-scope)\n",
    )
    .expect("write gerbil package anchor");
    std::fs::create_dir_all(root.join("src")).expect("create src");
    std::fs::create_dir_all(root.join("extra")).expect("create extra");
    std::fs::write(
        root.join("src/included.ss"),
        "(def (provider-scope-symbol) 'included)\n",
    )
    .expect("write included source");
    std::fs::write(
        root.join("extra/excluded.ss"),
        "(def (provider-scope-symbol) 'excluded)\n",
    )
    .expect("write excluded source");
    let provider_bin = home_local_provider_path(&root, "gslph");
    std::fs::create_dir_all(provider_bin.parent().expect("provider parent"))
        .expect("create home local bin");
    std::fs::write(&provider_bin, r#"#!/bin/sh
if [ "$1" = "search" ] && [ "$2" = "workspace-scope" ]; then
  printf '%s\n' '{"schemaId":"agent.semantic-protocols.semantic-workspace-scope","schemaVersion":"1","status":"ready","languageId":"gerbil-scheme","providerId":"gerbil-scheme-harness","files":[{"path":"src/included.ss"}]}'
  exit 0
fi
exit 2
"#).expect("write fake provider");
    make_executable(&provider_bin);
    let activation_path =
        write_gerbil_activation_with_provider_scope(&root, &provider_bin, &["src", "extra"]);
    let _activation_env = EnvVarGuard::set(
        ASP_PROVIDER_ACTIVATION_PATH_ENV,
        activation_path.as_os_str(),
    );
    run_cache(
        &root,
        None,
        &["source-index".to_string(), "rebuild".to_string()],
        false,
    )
    .expect("rebuild source index");
    let engine = ClientDbEngine::resolve(&root).expect("resolve DB Engine");
    assert!(engine.db_path().exists());
    let result = crate::test_support::lookup_current_source_index_for_language(
        &root,
        Some(&LanguageId::from("gerbil-scheme")),
        "provider-scope-symbol",
        8,
    )
    .expect("lookup source index");
    assert_eq!(result.state.as_str(), "hit");
    assert_eq!(result.candidates.len(), 1);
    assert_eq!(result.candidates[0].path, "src/included.ss");
    let _ = std::fs::remove_dir_all(root);
}
