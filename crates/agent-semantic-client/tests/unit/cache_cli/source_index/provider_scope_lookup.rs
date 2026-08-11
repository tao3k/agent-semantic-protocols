use super::fixtures::{
    EnvVarGuard, RuntimeServerFixture, home_local_provider_path, isolate_home, temp_root,
    write_gerbil_activation_with_project_resolution, write_project_resolution_provider,
    write_rust_activation,
};
use agent_semantic_client_core::{ASP_PROVIDER_ACTIVATION_PATH_ENV, LanguageId};

#[tokio::test(flavor = "multi_thread")]
async fn cache_source_index_refresh_respects_cargo_workspace_exclude() {
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
        "pub fn project_resolution_symbol() {}\n",
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
        "pub fn project_resolution_symbol() {}\n",
    )
    .expect("write excluded source");
    let activation_path = write_rust_activation(&root);
    let _activation_env = EnvVarGuard::set(
        ASP_PROVIDER_ACTIVATION_PATH_ENV,
        activation_path.as_os_str(),
    );
    let server = RuntimeServerFixture::start(&root).await;
    server.rebuild(&root).await;
    let blocking_root = root.clone();
    let result = server
        .operation(move || async move {
            crate::test_support::lookup_current_source_index_for_language(
                &blocking_root,
                Some(&LanguageId::from("rust")),
                "project_resolution_symbol",
                8,
            )
            .await
            .expect("lookup source index")
        })
        .await;
    assert_eq!(result.state.as_str(), "hit");
    assert_eq!(result.candidates.len(), 1);
    assert_eq!(result.candidates[0].path, "crates/app/src/lib.rs");
    server.shutdown().await;
    let _ = std::fs::remove_dir_all(root);
}

#[tokio::test(flavor = "multi_thread")]
async fn source_index_lookup_ranks_query_dense_owner_before_low_coverage_path() {
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
    let activation_path = write_rust_activation(&root);
    let _activation_env = EnvVarGuard::set(
        ASP_PROVIDER_ACTIVATION_PATH_ENV,
        activation_path.as_os_str(),
    );
    let server = RuntimeServerFixture::start(&root).await;
    server.rebuild(&root).await;
    let blocking_root = root.clone();
    let (result, versioned_alias) = server
        .operation(move || async move {
            let result = crate::test_support::lookup_current_source_index_for_language(
                &blocking_root,
                Some(&LanguageId::from("rust")),
                "ablation sandtable topology membership report chain request policy",
                8,
            )
            .await
            .expect("lookup source index");
            let versioned_alias = crate::test_support::lookup_current_source_index_for_language(
                &blocking_root,
                Some(&LanguageId::from("rust")),
                "10.15.02-codex-resident-agent-lifecycle-v2.org",
                8,
            )
            .await
            .expect("lookup lifecycle v2 alias source index");
            (result, versioned_alias)
        })
        .await;
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
    assert_eq!(versioned_alias.state.as_str(), "hit");
    assert!(versioned_alias.candidates.iter().any(|candidate| candidate.path == "crates/app/src/semantic_sandtable/docs/10-15-02-codex-resident-agent-lifecycle-v1.rs"), "{:?}", versioned_alias.candidates);
    server.shutdown().await;
    let _ = std::fs::remove_dir_all(root);
}

#[tokio::test(flavor = "multi_thread")]
async fn cache_source_index_refresh_uses_provider_project_resolution() {
    let _guard = crate::test_support::CACHE_TEST_LOCK
        .lock()
        .expect("cache test lock");
    let root = temp_root("source-index-provider-project-resolution");
    let _home_env = isolate_home(&root);
    let git_status = std::process::Command::new("git")
        .args(["init", "-q"])
        .current_dir(&root)
        .status()
        .expect("initialize Git candidate fixture");
    assert!(git_status.success(), "initialize Git candidate fixture");
    std::fs::write(root.join(".gitignore"), "home/\n")
        .expect("exclude fixture-local runtime state from Git candidates");
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
    write_project_resolution_provider(
        &provider_bin,
        "gerbil-scheme",
        "gerbil-scheme-harness",
        ".ss",
        &["src"],
        &[],
    );
    let activation_path =
        write_gerbil_activation_with_project_resolution(&root, &provider_bin, &["src", "extra"]);
    let _activation_env = EnvVarGuard::set(
        ASP_PROVIDER_ACTIVATION_PATH_ENV,
        activation_path.as_os_str(),
    );
    let server = RuntimeServerFixture::start(&root).await;
    server.rebuild(&root).await;
    let blocking_root = root.clone();
    let result = server
        .operation(move || async move {
            let current_snapshot =
                crate::source_index::current_source_index_snapshot(&blocking_root)
                    .await
                    .expect("capture current source-index snapshot");
            assert_eq!(
                current_snapshot.source_snapshot.leaf_count,
                1,
                "current source-index snapshot owners={:?}",
                current_snapshot
                    .source_blobs
                    .iter()
                    .map(|(path, _)| path)
                    .collect::<Vec<_>>()
            );
            assert!(
                current_snapshot.source_blobs.contains_key(
                    &agent_semantic_client_db::ClientDbSourceIndexPath::new(
                        "src/included.ss".to_owned(),
                    ),
                ),
                "current source-index snapshot owners={:?}",
                current_snapshot
                    .source_blobs
                    .iter()
                    .map(|(path, _)| path)
                    .collect::<Vec<_>>()
            );
            crate::test_support::lookup_current_source_index_for_language(
                &blocking_root,
                Some(&LanguageId::from("gerbil-scheme")),
                "provider-scope-symbol",
                8,
            )
            .await
            .expect("lookup source index")
        })
        .await;
    assert_eq!(result.state.as_str(), "hit");
    assert_eq!(result.candidates.len(), 1);
    assert_eq!(result.candidates[0].path, "src/included.ss");
    server.shutdown().await;
    let _ = std::fs::remove_dir_all(root);
}
