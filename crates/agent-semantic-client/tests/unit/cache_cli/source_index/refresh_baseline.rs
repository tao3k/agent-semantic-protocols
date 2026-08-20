use super::fixtures::{
    EnvVarGuard, RuntimeServerFixture, isolate_home, temp_root,
    write_gerbil_activation_with_command_prefix, write_rust_activation,
};
use agent_semantic_client_core::{ASP_PROVIDER_ACTIVATION_PATH_ENV, LanguageId};

#[tokio::test(flavor = "multi_thread")]
async fn cache_source_index_refresh_builds_db_engine_rows() {
    let _guard = crate::test_support::CACHE_TEST_LOCK
        .lock()
        .expect("cache test lock");
    let root = temp_root("source-index-refresh");
    let _home_env = isolate_home(&root);
    let source_dir = root.join("src");
    std::fs::create_dir_all(&source_dir).expect("create source dir");
    std::fs::write(root.join("gerbil.pkg"), "(package source-index-refresh)\n")
        .expect("write gerbil package anchor");
    std::fs::write(
        source_dir.join("usage.ss"),
        "(def (poo-read input)\n  ;; gerbil-poo://usage\n  input)\n",
    )
    .expect("write gerbil source");
    let activation_path = write_gerbil_activation_with_command_prefix(
        &root,
        super::fixtures::noop_provider_command_prefix(),
        &["src"],
    );
    let _activation_env = EnvVarGuard::set(
        ASP_PROVIDER_ACTIVATION_PATH_ENV,
        activation_path.as_os_str(),
    );
    std::fs::create_dir_all(root.join(".data/codex")).expect("create external data dir");
    std::fs::write(
        root.join(".data/codex/usage.rs"),
        "fn external_gerbil_poo_usage() {}\n",
    )
    .expect("write ignored data source");
    std::fs::create_dir_all(root.join(".codex/plugins/cache")).expect("create plugin cache dir");
    std::fs::write(
        root.join(".codex/plugins/cache/usage.rs"),
        "fn plugin_cache_gerbil_poo_usage() {}\n",
    )
    .expect("write ignored plugin cache source");

    let server = RuntimeServerFixture::start(&root).await;
    let rebuild_started = std::time::Instant::now();
    server.rebuild(&root).await;
    let rebuild_elapsed = rebuild_started.elapsed();
    server.rebuild(&root).await;
    let blocking_root = root.clone();
    let result = server
        .operation(move || async move {
            let result = crate::test_support::lookup_current_source_index_for_language(
                &blocking_root,
                Some(&LanguageId::from("gerbil-scheme")),
                "gerbil-poo",
                8,
            )
            .await
            .expect("lookup source index");
            result
        })
        .await;
    assert!(
        rebuild_elapsed < std::time::Duration::from_secs(5),
        "source-index cold rebuild exceeded fixture gate: elapsedMs={}",
        rebuild_elapsed.as_millis()
    );
    assert_eq!(result.state.as_str(), "hit");
    assert_eq!(result.candidates.len(), 1);
    assert_eq!(result.candidates[0].path, "src/usage.ss");
    assert_eq!(result.candidates[0].line_count, Some(3));
    server.shutdown().await;
    let _ = std::fs::remove_dir_all(root);
}

#[tokio::test(flavor = "multi_thread")]
async fn cache_source_index_admission_without_generation_is_bounded_warm_check() {
    let _guard = crate::test_support::CACHE_TEST_LOCK
        .lock()
        .expect("cache test lock");
    let root = temp_root("source-index-refresh-cold-required");
    let _home_env = isolate_home(&root);
    std::fs::write(
        root.join("Cargo.toml"),
        "[package]\nname = \"warm-admission\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
    )
    .expect("write workspace manifest");
    std::fs::create_dir_all(root.join("src")).expect("create warm admission source root");
    std::fs::write(root.join("src/lib.rs"), "pub fn warm_admission() {}\n")
        .expect("write warm admission source");
    let activation_path = write_rust_activation(&root);
    let _activation_env = EnvVarGuard::set(
        ASP_PROVIDER_ACTIVATION_PATH_ENV,
        activation_path.as_os_str(),
    );

    let server = RuntimeServerFixture::start(&root).await;
    server.rebuild(&root).await;
    crate::test_support::lookup_current_source_index_for_language(
        &root,
        Some(&LanguageId::from("rust")),
        "warm_admission",
        8,
    )
    .await
    .expect("prewarm immutable source-index generation");
    let mut latencies = Vec::with_capacity(16);
    for _ in 0..16 {
        let started = std::time::Instant::now();
        let lookup = crate::test_support::lookup_current_source_index_for_language(
            &root,
            Some(&LanguageId::from("rust")),
            "warm_admission",
            8,
        )
        .await
        .expect("read warm immutable source-index generation");
        assert_eq!(lookup.state.as_str(), "hit");
        latencies.push(started.elapsed());
    }
    latencies.sort_unstable();
    let p75 = latencies[latencies.len() * 75 / 100];
    let max = *latencies.last().expect("warm latency samples");
    assert!(
        p75 < std::time::Duration::from_millis(200) && max < std::time::Duration::from_millis(500),
        "source-index immutable-generation warm search exceeded gate: p75Ms={} maxMs={}",
        p75.as_millis(),
        max.as_millis()
    );
    server.shutdown().await;
    let _ = std::fs::remove_dir_all(root);
}

#[tokio::test(flavor = "multi_thread")]
async fn cache_source_index_refresh_invalidates_when_empty_source_root_gains_file() {
    let _guard = crate::test_support::CACHE_TEST_LOCK
        .lock()
        .expect("cache test lock");
    let root = temp_root("source-index-empty-root-invalidates");
    let _home_env = isolate_home(&root);
    let source_dir = root.join("src");
    let extra_dir = root.join("extra");
    std::fs::create_dir_all(&source_dir).expect("create source dir");
    std::fs::create_dir_all(&extra_dir).expect("create empty extra source dir");
    std::fs::write(root.join("gerbil.pkg"), "(package source-index-refresh)\n")
        .expect("write gerbil package anchor");
    std::fs::write(
        source_dir.join("usage.ss"),
        "(def (poo-read input)\n  ;; gerbil-poo://usage\n  input)\n",
    )
    .expect("write gerbil source");
    let activation_path = write_gerbil_activation_with_command_prefix(
        &root,
        super::fixtures::noop_provider_command_prefix(),
        &["src", "extra"],
    );
    let _activation_env = EnvVarGuard::set(
        ASP_PROVIDER_ACTIVATION_PATH_ENV,
        activation_path.as_os_str(),
    );

    let server = RuntimeServerFixture::start(&root).await;
    server.rebuild(&root).await;
    server.rebuild(&root).await;
    std::fs::write(
        extra_dir.join("new_usage.ss"),
        "(def (new-scope-symbol input)\n  input)\n",
    )
    .expect("write new extra source");
    server
        .admit_changed_paths(
            &root,
            vec![extra_dir.join("new_usage.ss").display().to_string()],
        )
        .await;
    let changed_root = root.clone();
    let result = server
        .operation(move || async move {
            crate::test_support::lookup_current_source_index_for_language(
                &changed_root,
                Some(&LanguageId::from("gerbil-scheme")),
                "new-scope-symbol",
                8,
            )
            .await
            .expect("lookup source index")
        })
        .await;

    assert_eq!(result.state.as_str(), "hit");
    assert_eq!(result.candidates.len(), 1);
    assert_eq!(result.candidates[0].path, "extra/new_usage.ss");
    server.shutdown().await;
    let _ = std::fs::remove_dir_all(root);
}

#[tokio::test(flavor = "multi_thread")]
async fn cold_changed_path_without_an_active_generation_publishes_the_baseline() {
    let _guard = crate::test_support::CACHE_TEST_LOCK
        .lock()
        .expect("cache test lock");
    let root = temp_root("source-index-cold-changed-path-baseline");
    let _home_env = isolate_home(&root);
    let source_dir = root.join("src");
    std::fs::create_dir_all(&source_dir).expect("create source dir");
    std::fs::write(
        root.join("gerbil.pkg"),
        "(package source-index-cold-baseline)\n",
    )
    .expect("write gerbil package anchor");
    let changed_path = source_dir.join("cold_usage.ss");
    std::fs::write(
        &changed_path,
        "(def (cold-baseline-symbol input)\n  input)\n",
    )
    .expect("write cold baseline source");
    let activation_path = write_gerbil_activation_with_command_prefix(
        &root,
        super::fixtures::noop_provider_command_prefix(),
        &["src"],
    );
    let _activation_env = EnvVarGuard::set(
        ASP_PROVIDER_ACTIVATION_PATH_ENV,
        activation_path.as_os_str(),
    );

    let server = RuntimeServerFixture::start(&root).await;
    server
        .admit_changed_paths(&root, vec![changed_path.display().to_string()])
        .await;
    let lookup_root = root.clone();
    let result = server
        .operation(move || async move {
            crate::test_support::lookup_current_source_index_for_language(
                &lookup_root,
                Some(&LanguageId::from("gerbil-scheme")),
                "cold-baseline-symbol",
                8,
            )
            .await
            .expect("lookup cold baseline source index")
        })
        .await;

    assert_eq!(result.state.as_str(), "hit");
    assert_eq!(result.candidates.len(), 1);
    assert_eq!(result.candidates[0].path, "src/cold_usage.ss");
    server.shutdown().await;
    let _ = std::fs::remove_dir_all(root);
}
