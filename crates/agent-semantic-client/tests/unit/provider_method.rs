use agent_semantic_client_core::{ClientMethod, ClientRequest};

use crate::provider_method::{
    last_check_output_path, persist_last_check_output, should_try_search_packet_first,
};
use crate::test_support::CACHE_TEST_LOCK;

#[test]
fn search_packet_first_skips_workspace_seed_discovery() {
    let request = ClientRequest::new(ClientMethod::Search, ".").with_forwarded_args(vec![
        "workspace".to_string(),
        "--view".to_string(),
        "seeds".to_string(),
    ]);

    assert!(!should_try_search_packet_first(&request));
}

#[test]
fn search_packet_first_still_handles_seed_fzf() {
    let request = ClientRequest::new(ClientMethod::Search, ".").with_forwarded_args(vec![
        "fzf".to_string(),
        "workspace".to_string(),
        "--view".to_string(),
        "seeds".to_string(),
    ]);

    assert!(should_try_search_packet_first(&request));
}

#[test]
fn search_packet_first_handles_dependency_search_without_seed_view() {
    let request = ClientRequest::new(ClientMethod::Search, ".").with_forwarded_args(vec![
        "deps".to_string(),
        "serde@1::Serialize".to_string(),
        ".".to_string(),
    ]);

    assert!(should_try_search_packet_first(&request));
}

#[test]
fn search_packet_first_skips_dependency_search_json_passthrough() {
    let request = ClientRequest::new(ClientMethod::Search, ".").with_forwarded_args(vec![
        "deps".to_string(),
        "serde@1::Serialize".to_string(),
        "--json".to_string(),
        ".".to_string(),
    ]);

    assert!(!should_try_search_packet_first(&request));
}

#[test]
fn search_packet_first_skips_compare_seed_passthrough() {
    let request = ClientRequest::new(ClientMethod::Search, ".").with_forwarded_args(vec![
        "compare".to_string(),
        "env".to_string(),
        "stable".to_string(),
        "nightly".to_string(),
        "--view".to_string(),
        "seeds".to_string(),
    ]);

    assert!(!should_try_search_packet_first(&request));
}

#[test]
fn failed_check_output_is_persisted_for_failure_frontier_search() {
    let _guard = CACHE_TEST_LOCK.lock().expect("cache test lock");
    let root = temp_project_root("failed-check-output");
    let cache_home = root.join(".cache");
    let previous_cache_home = std::env::var_os("PRJ_CACHE_HOME");
    unsafe {
        std::env::set_var("PRJ_CACHE_HOME", &cache_home);
    }

    persist_last_check_output(
        &root,
        101,
        b"[fail] rust blockingFindings=1\n",
        b"cache_cli::writeback expected hit actual miss\n",
    )
    .expect("persist last check");

    let path = last_check_output_path(&root);
    let transcript = std::fs::read_to_string(&path).expect("read last check");
    assert!(transcript.contains("[fail] rust"), "{transcript}");
    assert!(transcript.contains("cache_cli::writeback"), "{transcript}");

    restore_cache_home(previous_cache_home);
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn successful_check_clears_stale_failure_frontier_transcript() {
    let _guard = CACHE_TEST_LOCK.lock().expect("cache test lock");
    let root = temp_project_root("successful-check-output");
    let cache_home = root.join(".cache");
    let previous_cache_home = std::env::var_os("PRJ_CACHE_HOME");
    unsafe {
        std::env::set_var("PRJ_CACHE_HOME", &cache_home);
    }
    let path = last_check_output_path(&root);
    std::fs::create_dir_all(path.parent().expect("last check parent")).expect("create cache");
    std::fs::write(&path, "stale failure").expect("write stale failure");

    persist_last_check_output(&root, 0, b"[ok] rust\n", b"").expect("clear last check");

    assert!(
        !path.exists(),
        "successful check should clear stale transcript"
    );
    restore_cache_home(previous_cache_home);
    let _ = std::fs::remove_dir_all(root);
}

fn temp_project_root(name: &str) -> std::path::PathBuf {
    let root = std::env::temp_dir().join(format!(
        "asp-client-provider-method-{name}-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).expect("create temp project root");
    root
}

fn restore_cache_home(previous: Option<std::ffi::OsString>) {
    unsafe {
        if let Some(value) = previous {
            std::env::set_var("PRJ_CACHE_HOME", value);
        } else {
            std::env::remove_var("PRJ_CACHE_HOME");
        }
    }
}
