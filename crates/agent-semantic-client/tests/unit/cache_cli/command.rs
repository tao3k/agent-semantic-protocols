use crate::cache_cli::run_cache;
use crate::cache_cli_source_index_tests::fixtures::RuntimeServerFixture;
use crate::test_support::{CACHE_TEST_LOCK, EnvVarGuard};
use agent_semantic_client_core::LanguageId;

fn prepare_runtime_owned_rust_fixture(root: &std::path::Path, symbol: &str) {
    let provider_binary = root.join(".asp-state/runtime/bin/rs-harness");
    crate::cache_cli_source_index_tests::fixtures::write_project_resolution_provider(
        &provider_binary,
        "rust",
        "rs-harness",
        ".rs",
        &["src"],
        &[],
    );
    crate::test_support::write_hermetic_provider_registry_config(root, "rust", "rs-harness");
    crate::test_support::write_hermetic_provider_install_receipt(root, "rust", &provider_binary);
    std::fs::write(
        root.join("Cargo.toml"),
        "[package]\nname = \"cache-command-fixture\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
    )
    .expect("write Cargo manifest");
    std::fs::create_dir_all(root.join("src")).expect("create source directory");
    std::fs::write(root.join("src/lib.rs"), format!("pub fn {symbol}() {{}}\n"))
        .expect("write source file");
}

#[tokio::test]
async fn cache_usage_lists_flush() {
    let root = temp_root("usage");
    let error = run_cache(&root, None, &["unknown".to_string()], false)
        .await
        .expect_err("usage");

    assert!(error.contains(
        "status|gc [--grace-days <n>] [--apply]|clean --day[=<days>]|import|source-index refresh|source-index lookup"
    ));
    assert!(error.contains("source-index refresh"));
    assert!(error.contains("source-index lookup --query <term>"));
    assert!(error.contains("invalidate|flush [syntax-rows]"));
}

#[tokio::test]
async fn cache_status_process_reader_helper() {
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
    run_cache(&root, None, &["status".to_string()], false)
        .await
        .expect("process cache status reader");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn cache_status_survives_concurrent_process_readers() {
    let _guard = CACHE_TEST_LOCK.lock().expect("cache test lock");
    let root = temp_root("cache-status-concurrent-readers");
    let state_home = root.join(".asp-state");
    let _state_home = EnvVarGuard::set("ASP_STATE_HOME", &state_home);
    let server = RuntimeServerFixture::start(&root).await;

    let current_exe = std::env::current_exe().expect("locate current test binary");
    let mut children = Vec::new();
    for _ in 0..6 {
        children.push(
            tokio::process::Command::new(&current_exe)
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
        let status = child.wait().await.expect("wait for cache status reader");
        assert!(status.success(), "cache status reader failed: {status}");
    }

    server.shutdown().await;
    let _ = std::fs::remove_dir_all(root);
}

#[tokio::test]
async fn cache_flush_invalidates_the_runtime_owned_generation() {
    let _guard = crate::test_support::CACHE_TEST_LOCK
        .lock()
        .expect("cache test lock");
    let root = temp_root("invalid-manifest");
    let _state_home = EnvVarGuard::set("ASP_STATE_HOME", root.join(".asp-state"));
    prepare_runtime_owned_rust_fixture(&root, "cache_flush_symbol");
    let server = RuntimeServerFixture::start(&root).await;
    server.rebuild(&root).await;

    run_cache(&root, None, &["flush".to_string()], false)
        .await
        .expect("flush Runtime-owned workspace generation");

    server.shutdown().await;
    let _ = std::fs::remove_dir_all(root);
}

#[tokio::test]
async fn cache_import_rebuilds_the_runtime_owned_source_index() {
    let _guard = crate::test_support::CACHE_TEST_LOCK
        .lock()
        .expect("cache test lock");
    let root = temp_root("structural-index-import");
    let _state_home =
        crate::test_support::EnvVarGuard::set("ASP_STATE_HOME", root.join(".asp-state"));
    prepare_runtime_owned_rust_fixture(&root, "cache_imported_symbol");
    let server = RuntimeServerFixture::start(&root).await;

    run_cache(&root, None, &["import".to_string()], false)
        .await
        .expect("Runtime-owned source-index import");

    let result = crate::test_support::lookup_current_source_index_for_language(
        &root,
        Some(&LanguageId::from("rust")),
        "cache_imported_symbol",
        8,
    )
    .await
    .expect("query Runtime-owned imported source index");

    assert_eq!(result.candidates.len(), 1);
    assert!(
        result.candidates[0].path.contains("src/lib.rs"),
        "Runtime source index should retain owner path: {:?}",
        result.candidates[0]
    );
    server.shutdown().await;
    let _ = std::fs::remove_dir_all(root);
}

fn temp_root(label: &str) -> std::path::PathBuf {
    crate::test_support::owner_backed_temp_root(label)
}
