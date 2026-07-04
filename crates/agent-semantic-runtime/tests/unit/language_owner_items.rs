use std::cell::Cell;
use std::fs;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::{
    LanguageOwnerItemsAttempt, LanguageOwnerItemsCacheRequest, LanguageOwnerItemsDispatchPlan,
    LanguageOwnerItemsProviderOutput, LanguageOwnerItemsRuntimeOutcome,
    compact_language_owner_items_stdout, language_owner_items_failure,
    language_owner_items_runtime_receipt, language_owner_items_workspace_root,
    read_language_owner_items_cache, resolve_language_owner_items_runtime_outcome,
    run_language_owner_items_dispatch_plan, write_language_owner_items_cache,
};

#[test]
fn owner_items_dispatch_plan_runs_provider() {
    let root = temp_root("owner-items-provider-first");
    fs::create_dir_all(root.join("src")).expect("create source root");
    fs::write(root.join("src/lib.rs"), "pub fn owner() {}\n").expect("write owner");
    let provider_calls = Cell::new(0);

    let result = run_language_owner_items_dispatch_plan(LanguageOwnerItemsDispatchPlan {
        language_id: "rust",
        owner: std::path::Path::new("src/lib.rs"),
        project_root: &root,
        provider: || {
            provider_calls.set(provider_calls.get() + 1);
            Ok(LanguageOwnerItemsAttempt::Handled)
        },
    })
    .expect("dispatch owner items");

    assert_eq!(result, LanguageOwnerItemsAttempt::Handled);
    assert_eq!(provider_calls.get(), 1);
    let _ = fs::remove_dir_all(root);
}

#[test]
fn owner_items_dispatch_plan_fails_closed_when_provider_is_unsupported() {
    let root = temp_root("owner-items-provider-unsupported");
    fs::create_dir_all(root.join("src")).expect("create source root");
    fs::write(root.join("src/lib.rs"), "pub fn owner() {}\n").expect("write owner");
    let provider_calls = Cell::new(0);

    let error = run_language_owner_items_dispatch_plan(LanguageOwnerItemsDispatchPlan {
        language_id: "rust",
        owner: std::path::Path::new("src/lib.rs"),
        project_root: &root,
        provider: || {
            provider_calls.set(provider_calls.get() + 1);
            Ok(LanguageOwnerItemsAttempt::Unsupported)
        },
    })
    .expect_err("unsupported provider must fail closed");

    assert!(
        error.contains("requires a language-harness owner-items interface"),
        "{error}"
    );
    assert_eq!(provider_calls.get(), 1);
    let _ = fs::remove_dir_all(root);
}

#[test]
fn owner_items_cache_round_trip_is_owned_by_runtime() {
    let root = temp_root("owner-items-cache");
    let cache_home = root.join(".cache");
    fs::create_dir_all(root.join("src")).expect("create source root");
    fs::write(root.join("src/lib.rs"), "pub fn owner() {}\n").expect("write owner");
    let args = vec![
        "items".to_string(),
        "--view".to_string(),
        "seeds".to_string(),
    ];
    let invocation = vec!["rs-harness".to_string(), "query".to_string()];
    let request = LanguageOwnerItemsCacheRequest {
        language_id: "rust",
        args: &args,
        invocation: &invocation,
        owner: std::path::Path::new("src/lib.rs"),
        project_root: &root,
        cache_home: &cache_home,
    };

    assert!(
        read_language_owner_items_cache(&request)
            .expect("read missing cache")
            .is_none()
    );
    write_language_owner_items_cache(&request, b"owner-items\n").expect("write cache");
    assert_eq!(
        read_language_owner_items_cache(&request).expect("read cache"),
        Some(b"owner-items\n".to_vec())
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn owner_items_runtime_outcome_uses_cache_before_provider_output() {
    let root = temp_root("owner-items-cache-outcome");
    let cache_home = root.join(".cache");
    fs::create_dir_all(root.join("src")).expect("create source root");
    fs::write(root.join("src/lib.rs"), "pub fn owner() {}\n").expect("write owner");
    let args = vec!["items".to_string()];
    let invocation = vec!["rs-harness".to_string(), "query".to_string()];
    let request = LanguageOwnerItemsCacheRequest {
        language_id: "rust",
        args: &args,
        invocation: &invocation,
        owner: std::path::Path::new("src/lib.rs"),
        project_root: &root,
        cache_home: &cache_home,
    };
    write_language_owner_items_cache(&request, b"cached owner-items\n").expect("write cache");

    let outcome =
        resolve_language_owner_items_runtime_outcome(&request, true, None).expect("resolve cache");
    assert_eq!(
        outcome,
        LanguageOwnerItemsRuntimeOutcome::Handled {
            stdout: b"cached owner-items\n".to_vec(),
            stderr: Vec::new(),
            cache_hit: true,
        }
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn owner_items_runtime_outcome_compacts_and_caches_provider_success() {
    let root = temp_root("owner-items-provider-outcome");
    let cache_home = root.join(".cache");
    fs::create_dir_all(root.join("src")).expect("create source root");
    fs::write(root.join("src/lib.rs"), "pub fn owner() {}\n").expect("write owner");
    let args = vec!["items".to_string()];
    let invocation = vec!["rs-harness".to_string(), "query".to_string()];
    let request = LanguageOwnerItemsCacheRequest {
        language_id: "rust",
        args: &args,
        invocation: &invocation,
        owner: std::path::Path::new("src/lib.rs"),
        project_root: &root,
        cache_home: &cache_home,
    };

    let outcome = resolve_language_owner_items_runtime_outcome(
        &request,
        true,
        Some(LanguageOwnerItemsProviderOutput {
            status_success: true,
            stdout: b"actionFrontier=internal\npublic owner item\n",
            stderr: b"provider note\n",
        }),
    )
    .expect("resolve provider output");
    assert_eq!(
        outcome,
        LanguageOwnerItemsRuntimeOutcome::Handled {
            stdout: b"public owner item\n".to_vec(),
            stderr: b"provider note\n".to_vec(),
            cache_hit: false,
        }
    );
    assert_eq!(
        read_language_owner_items_cache(&request).expect("read cache"),
        Some(b"public owner item\n".to_vec())
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn owner_items_runtime_receipt_records_provider_count_and_output_size() {
    let outcome = LanguageOwnerItemsRuntimeOutcome::Handled {
        stdout: b"public owner item\n".to_vec(),
        stderr: b"provider note\n".to_vec(),
        cache_hit: false,
    };

    let receipt = language_owner_items_runtime_receipt(&outcome, 1, 2);

    assert_eq!(receipt.outcome, "handled");
    assert_eq!(receipt.provider_process_count, 1);
    assert_eq!(receipt.stdout_bytes, b"public owner item\n".len());
    assert_eq!(receipt.stderr_bytes, b"provider note\n".len());
    assert!(!receipt.cache_hit);
    assert_eq!(receipt.fallback_reason, "none");
    assert_eq!(receipt.elapsed_ms, 2);
}

#[test]
fn owner_items_runtime_receipt_records_fail_closed_without_fallback() {
    let outcome = LanguageOwnerItemsRuntimeOutcome::Failed(
        "provider-owned owner-items failed for existing owner `src/lib.rs`; no fallback executed"
            .to_string(),
    );

    let receipt = language_owner_items_runtime_receipt(&outcome, 1, 3);

    assert_eq!(receipt.outcome, "failed");
    assert_eq!(receipt.provider_process_count, 1);
    assert_eq!(receipt.stdout_bytes, 0);
    assert_eq!(receipt.fallback_reason, "fail-closed-no-fallback");
    assert_eq!(receipt.elapsed_ms, 3);
}

#[test]
fn owner_items_runtime_outcome_unsupported_when_missing_owner_fails() {
    let root = temp_root("owner-items-missing-owner-outcome");
    let cache_home = root.join(".cache");
    let args = vec!["items".to_string()];
    let invocation = vec!["rs-harness".to_string(), "query".to_string()];
    let request = LanguageOwnerItemsCacheRequest {
        language_id: "rust",
        args: &args,
        invocation: &invocation,
        owner: std::path::Path::new("src/missing.rs"),
        project_root: &root,
        cache_home: &cache_home,
    };

    let outcome = resolve_language_owner_items_runtime_outcome(
        &request,
        false,
        Some(LanguageOwnerItemsProviderOutput {
            status_success: false,
            stdout: b"",
            stderr: b"missing owner\n",
        }),
    )
    .expect("resolve missing owner output");
    assert_eq!(outcome, LanguageOwnerItemsRuntimeOutcome::Unsupported);
    let _ = fs::remove_dir_all(root);
}

#[test]
fn owner_items_compact_render_drops_internal_search_lines() {
    let compact = compact_language_owner_items_stdout(
        b"actionFrontier=internal\npublic owner item\n[graph-frontier] internal\n",
    );

    assert_eq!(compact, b"public owner item\n");
}

#[test]
fn owner_items_failure_reports_no_fallback() {
    let failure = language_owner_items_failure(
        "provider-owned owner-items failed",
        std::path::Path::new("src/lib.rs"),
        b"provider stderr\n",
        true,
    );

    assert!(failure.contains("no fallback executed"), "{failure}");
    assert!(failure.contains("provider stderr"), "{failure}");
}

#[test]
fn owner_items_workspace_root_defaults_to_project_root() {
    let project_root = std::path::Path::new("/repo/project");
    let locator_root = std::path::Path::new("/repo");

    assert_eq!(
        language_owner_items_workspace_root(project_root, locator_root, None),
        project_root
    );
}

#[test]
fn owner_items_workspace_root_resolves_relative_workspace_from_locator_root() {
    let project_root = std::path::Path::new("/repo/project");
    let locator_root = std::path::Path::new("/repo");

    assert_eq!(
        language_owner_items_workspace_root(
            project_root,
            locator_root,
            Some(std::path::Path::new("project/./crates/../crates/runtime")),
        ),
        std::path::PathBuf::from("/repo/project/crates/runtime")
    );
}

#[test]
fn owner_items_workspace_root_keeps_absolute_workspace() {
    let project_root = std::path::Path::new("/repo/project");
    let locator_root = std::path::Path::new("/repo");

    assert_eq!(
        language_owner_items_workspace_root(
            project_root,
            locator_root,
            Some(std::path::Path::new("/tmp/worktree/./crate")),
        ),
        std::path::PathBuf::from("/tmp/worktree/crate")
    );
}

fn temp_root(name: &str) -> std::path::PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let root = std::env::temp_dir().join(format!("agent-semantic-runtime-{name}-{unique}"));
    fs::create_dir_all(&root).expect("create temp root");
    root
}
