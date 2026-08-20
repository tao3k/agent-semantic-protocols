use crate::cache_cli::run_cache;
use crate::cache_cli_source_index_tests::fixtures::RuntimeServerFixture;
use crate::test_support::{CACHE_TEST_LOCK, EnvVarGuard};

#[tokio::test]
async fn cache_usage_excludes_legacy_mutation_aliases() {
    let root = temp_root("usage");
    let error = run_cache(&root, None, &["unknown".to_string()], false)
        .await
        .expect_err("usage");

    assert!(error.contains(
        "status|gc [--grace-days <n>] [--apply]|clean --day[=<days>]|source-index lookup"
    ));
    assert!(!error.contains("import"));
    assert!(!error.contains("source-index refresh"));
    assert!(error.contains("source-index lookup --query <term>"));
    assert!(error.contains("|invalidate>"));
    assert!(!error.contains("flush"));
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

fn temp_root(label: &str) -> std::path::PathBuf {
    crate::test_support::owner_backed_temp_root(label)
}
