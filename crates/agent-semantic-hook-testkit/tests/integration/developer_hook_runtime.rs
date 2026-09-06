#![cfg(unix)]

use std::collections::BTreeSet;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;

use agent_semantic_hook_testkit::HookProcessSpec;
use agent_semantic_hook_testkit::run_hook_process;
use fs2::FileExt;
use serde_json::json;

static DEVELOPER_LAUNCHER_TEST_SERIAL: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

fn launcher() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../asp-codex-plugin/bin/asp-hook-exec")
}

fn hook_binary(root: &Path, label: &str) -> PathBuf {
    let path = root.join(format!("asp-hook-{label}"));
    std::fs::write(
        &path,
        format!("#!/bin/sh\nprintf '%s\\n' '{{\"runtimeHookBinary\":\"{label}\"}}'\n"),
    )
    .expect("write Hook binary");
    let mut permissions = std::fs::metadata(&path)
        .expect("Hook binary metadata")
        .permissions();
    permissions.set_mode(0o755);
    std::fs::set_permissions(&path, permissions).expect("Hook binary mode");
    path
}

async fn publish(state_home: &Path, sources: &Path, label: &str) -> String {
    let hook_source = hook_binary(sources, label);
    let runtime_source = hook_binary(sources, &format!("runtime-{label}"));
    let target = state_home.join("runtime/bin/asp");
    agent_semantic_artifacts::runtime_artifact_publication::publish_runtime_artifact_bundle(
        state_home,
        &runtime_source,
        &target,
        "dev",
        &hook_source,
    )
    .await
    .expect("publish canonical Runtime binary bundle")
    .bundle_digest
    .to_string()
}

async fn invoke(state_home: &Path) -> serde_json::Value {
    let mut spec = HookProcessSpec::new(launcher(), state_home);
    spec.args = vec![
        "pre-tool".to_owned(),
        "--client".to_owned(),
        "codex".to_owned(),
        "--host-match".to_owned(),
        "Bash".to_owned(),
    ];
    spec.env.push((
        "ASP_STATE_HOME".to_owned(),
        state_home.display().to_string(),
    ));
    run_hook_process(&spec, &json!({"tool_input": {}}))
        .await
        .expect("launcher terminal")
        .decision
}

#[tokio::test]
async fn real_launcher_observes_each_of_ten_runtime_hook_publications() {
    let _serial = DEVELOPER_LAUNCHER_TEST_SERIAL.lock().await;
    let temp = tempfile::tempdir().expect("temp state");
    let state_home = temp.path().join("state");
    let sources = temp.path().join("sources");
    std::fs::create_dir_all(&sources).expect("sources");
    let mut digests = BTreeSet::new();
    for index in 0..10 {
        let label = format!("generation-{index}");
        digests.insert(publish(&state_home, &sources, &label).await);
        let receipt = invoke(&state_home).await;
        assert_eq!(receipt["runtimeHookBinary"], label);
        assert!(!state_home.join("hooks/current").exists());
    }
    assert_eq!(digests.len(), 10);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 8)]
async fn thirty_two_real_launcher_calls_crossing_switch_observe_only_old_or_new() {
    let _serial = DEVELOPER_LAUNCHER_TEST_SERIAL.lock().await;
    let temp = tempfile::tempdir().expect("temp state");
    let state_home = temp.path().join("state");
    let sources = temp.path().join("sources");
    std::fs::create_dir_all(&sources).expect("sources");
    publish(&state_home, &sources, "old").await;
    let barrier = Arc::new(tokio::sync::Barrier::new(33));
    let mut calls = Vec::new();
    for _ in 0..32 {
        let state_home = state_home.clone();
        let barrier = Arc::clone(&barrier);
        calls.push(tokio::spawn(async move {
            barrier.wait().await;
            invoke(&state_home).await
        }));
    }
    barrier.wait().await;
    publish(&state_home, &sources, "new").await;
    for call in calls {
        let receipt = call.await.expect("launcher task");
        assert!(matches!(
            receipt["runtimeHookBinary"].as_str(),
            Some("old" | "new")
        ));
    }
}

#[tokio::test]
async fn mutable_config_source_edit_does_not_change_runtime_hook_binary() {
    let _serial = DEVELOPER_LAUNCHER_TEST_SERIAL.lock().await;
    let temp = tempfile::tempdir().expect("temp state");
    let state_home = temp.path().join("state");
    let sources = temp.path().join("sources");
    std::fs::create_dir_all(&sources).expect("sources");
    let digest = publish(&state_home, &sources, "active").await;
    std::fs::create_dir_all(state_home.join("hooks")).expect("hooks source root");
    std::fs::write(
        state_home.join("hooks/config.toml"),
        "schemaVersion = 1\nsourceOnly = \"edited-without-install\"\n",
    )
    .expect("edit mutable config source");
    let receipt = invoke(&state_home).await;
    assert_eq!(receipt["runtimeHookBinary"], "active");
    let active = std::fs::read_link(state_home.join("runtime/artifacts/active"))
        .expect("active Runtime bundle");
    let active_digest =
        agent_semantic_artifacts::runtime_artifact_slots::runtime_artifact_candidate_bundle_digest(
            &active,
        )
        .await
        .expect("active Runtime bundle digest")
        .to_string();
    assert_eq!(active_digest, digest);
    assert_eq!(
        std::fs::read_link(state_home.join("runtime/bin/asp-hook"))
            .expect("fixed Runtime Hook launcher"),
        state_home.join("runtime/artifacts/active/asp-hook")
    );
    assert!(!state_home.join("hooks/current").exists());
}

#[tokio::test]
async fn inherited_escape_precedes_missing_or_corrupt_runtime_hook_binary() {
    let _serial = DEVELOPER_LAUNCHER_TEST_SERIAL.lock().await;
    let temp = tempfile::tempdir().expect("temp state");
    let missing_state = temp.path().join("missing");
    let corrupt_state = temp.path().join("corrupt");
    std::fs::create_dir_all(corrupt_state.join("runtime/bin")).expect("Runtime bin root");
    std::fs::write(
        corrupt_state.join("runtime/bin/asp-hook"),
        b"not-an-executable",
    )
    .expect("corrupt Runtime Hook binary");
    for state_home in [&missing_state, &corrupt_state] {
        for event in [
            "pre-tool",
            "permission-request",
            "post-tool",
            "stop",
            "notification",
            "user-prompt",
            "session-start",
            "subagent-start",
            "subagent-stop",
        ] {
            let mut escaped = HookProcessSpec::new(launcher(), temp.path());
            escaped.args = vec![event.to_owned(), "--client".to_owned(), "codex".to_owned()];
            escaped.timeout = std::time::Duration::from_secs(1);
            escaped
                .env
                .push(("ASP_NO_AGENT".to_owned(), "1".to_owned()));
            escaped.env.push((
                "ASP_STATE_HOME".to_owned(),
                state_home.display().to_string(),
            ));
            let escaped = run_hook_process(&escaped, &json!({"invalid": true}))
                .await
                .unwrap_or_else(|error| panic!("layer-zero event={event}: {error}"));
            assert_eq!(escaped.decision, json!({}), "event={event}");
            assert!(escaped.stderr.is_empty(), "event={event}");
            assert!(escaped.elapsed < std::time::Duration::from_secs(1));
        }
    }
}

#[tokio::test]
async fn publication_failure_preserves_previous_runtime_hook_binary() {
    let _serial = DEVELOPER_LAUNCHER_TEST_SERIAL.lock().await;
    let temp = tempfile::tempdir().expect("temp state");
    let state_home = temp.path().join("state");
    let sources = temp.path().join("sources");
    std::fs::create_dir_all(&sources).expect("sources");
    publish(&state_home, &sources, "previous").await;

    let lock_path = state_home.join("runtime/locks/artifact-mutation.lock");
    let lock = std::fs::OpenOptions::new()
        .create(true)
        .read(true)
        .write(true)
        .open(lock_path)
        .expect("publication lock");
    lock.try_lock_exclusive().expect("hold publication lock");
    let next_hook = hook_binary(&sources, "next");
    let next_runtime = hook_binary(&sources, "runtime-next");
    let error =
        agent_semantic_artifacts::runtime_artifact_publication::publish_runtime_artifact_bundle(
            &state_home,
            &next_runtime,
            &state_home.join("runtime/bin/asp"),
            "dev",
            &next_hook,
        )
        .await
        .expect_err("concurrent Runtime bundle publication must fail closed");
    assert!(error.contains("artifact-publication-conflict"), "{error}");
    FileExt::unlock(&lock).expect("release publication lock");
    assert_eq!(invoke(&state_home).await["runtimeHookBinary"], "previous");
}
