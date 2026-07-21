use super::fixtures::{
    EnvVarGuard, isolate_home, temp_root, write_gerbil_activation_with_command_prefix,
    write_rust_activation_with_ignored_prefixes,
};
use crate::cache_cli::run_cache;
use agent_semantic_client_core::{ASP_PROVIDER_ACTIVATION_PATH_ENV, LanguageId};
use agent_semantic_client_db::ClientDbEngine;

#[test]
fn cache_source_index_refresh_builds_db_engine_rows() {
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

    let rebuild_started = std::time::Instant::now();
    run_cache(
        &root,
        None,
        &["source-index".to_string(), "rebuild".to_string()],
        false,
    )
    .expect("rebuild source index");
    let rebuild_elapsed = rebuild_started.elapsed();
    assert!(
        rebuild_elapsed < std::time::Duration::from_secs(5),
        "source-index cold rebuild exceeded fixture gate: elapsedMs={}",
        rebuild_elapsed.as_millis()
    );
    run_cache(
        &root,
        None,
        &["source-index".to_string(), "refresh".to_string()],
        false,
    )
    .expect("reuse refreshed source index");

    let engine = ClientDbEngine::resolve(&root).expect("resolve DB Engine");
    assert!(
        engine.db_path().exists(),
        "source-index refresh must write the active DB Engine path"
    );
    let result = crate::test_support::lookup_current_source_index_for_language(
        &root,
        Some(&LanguageId::from("gerbil-scheme")),
        "gerbil-poo",
        8,
    )
    .expect("lookup source index");

    assert_eq!(result.state.as_str(), "hit");
    assert_eq!(result.candidates.len(), 1);
    assert_eq!(result.candidates[0].path, "src/usage.ss");
    assert_eq!(result.candidates[0].line_count, Some(3));
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn cache_source_index_refresh_without_generation_is_bounded_warm_check() {
    let _guard = crate::test_support::CACHE_TEST_LOCK
        .lock()
        .expect("cache test lock");
    let root = temp_root("source-index-refresh-cold-required");
    let _home_env = isolate_home(&root);
    std::fs::write(
        root.join("Cargo.toml"),
        "[workspace]\nmembers = []\nresolver = \"2\"\n",
    )
    .expect("write workspace manifest");
    let activation_path = write_rust_activation_with_ignored_prefixes(&root, &[]);
    let _activation_env = EnvVarGuard::set(
        ASP_PROVIDER_ACTIVATION_PATH_ENV,
        activation_path.as_os_str(),
    );

    let started = std::time::Instant::now();
    run_cache(
        &root,
        None,
        &["source-index".to_string(), "refresh".to_string()],
        false,
    )
    .expect("refresh without generation should be a warm check");
    let elapsed = started.elapsed();

    assert!(
        elapsed < std::time::Duration::from_millis(100),
        "source-index refresh warm check exceeded gate: elapsedMs={}",
        elapsed.as_millis()
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn cache_source_index_refresh_invalidates_when_empty_source_root_gains_file() {
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

    run_cache(
        &root,
        None,
        &["source-index".to_string(), "rebuild".to_string()],
        false,
    )
    .expect("rebuild source index");
    run_cache(
        &root,
        None,
        &["source-index".to_string(), "refresh".to_string()],
        false,
    )
    .expect("reuse source index");
    std::fs::write(
        extra_dir.join("new_usage.ss"),
        "(def (new-scope-symbol input)\n  input)\n",
    )
    .expect("write new extra source");
    run_cache(
        &root,
        None,
        &["source-index".to_string(), "rebuild".to_string()],
        false,
    )
    .expect("rebuild changed source index");

    let engine = ClientDbEngine::resolve(&root).expect("resolve DB Engine");
    assert!(engine.db_path().exists());
    let result = crate::test_support::lookup_current_source_index_for_language(
        &root,
        Some(&LanguageId::from("gerbil-scheme")),
        "new-scope-symbol",
        8,
    )
    .expect("lookup source index");

    assert_eq!(result.state.as_str(), "hit");
    assert_eq!(result.candidates.len(), 1);
    assert_eq!(result.candidates[0].path, "extra/new_usage.ss");
    let _ = std::fs::remove_dir_all(root);
}
