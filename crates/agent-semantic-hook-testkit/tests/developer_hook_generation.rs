#![cfg(unix)]

use std::collections::BTreeSet;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use agent_semantic_artifacts::hook_generation::{
    HookGenerationCandidate, commit_hook_generation, prepare_hook_generation,
    read_current_hook_generation,
};
use agent_semantic_hook_testkit::{HookProcessSpec, run_hook_process};
use fs2::FileExt;
use serde_json::json;

static DEVELOPER_LAUNCHER_TEST_SERIAL: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

fn launcher() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../asp-codex-plugin/bin/asp-hook")
}

fn evaluator(root: &Path, label: &str) -> PathBuf {
    let path = root.join(format!("asp-{label}"));
    std::fs::write(
        &path,
        format!(
            "#!/bin/sh\nprintf '%s\\n' '{{\"generation\":\"{label}\",\"launcherGenerationRoot\":\"'\"$ASP_HOOK_GENERATION_ROOT\"'\"}}'\n"
        ),
    )
    .expect("write evaluator");
    let mut permissions = std::fs::metadata(&path)
        .expect("evaluator metadata")
        .permissions();
    permissions.set_mode(0o755);
    std::fs::set_permissions(&path, permissions).expect("evaluator mode");
    path
}

fn publish(state_home: &Path, sources: &Path, label: &str) -> String {
    let prepared = prepare_candidate(state_home, sources, label);
    commit_hook_generation(state_home, &prepared)
        .expect("commit generation")
        .generation
        .generation_digest
        .to_string()
}

fn prepare_candidate(
    state_home: &Path,
    sources: &Path,
    label: &str,
) -> agent_semantic_artifacts::hook_generation::PreparedHookGeneration {
    let evaluator = evaluator(sources, label);
    prepare_hook_generation(
        state_home,
        HookGenerationCandidate {
            evaluator_binary: &evaluator,
            config: format!("schemaVersion = 1\nlabel = \"{label}\"\n").as_bytes(),
            compiled_matcher: format!("matcher:{label}").as_bytes(),
            registry: format!("registry:{label}").as_bytes(),
        },
    )
    .expect("prepare generation")
}

async fn invoke(state_home: &Path, payload: serde_json::Value) -> serde_json::Value {
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
    run_hook_process(&spec, &payload)
        .await
        .expect("launcher terminal")
        .decision
}

#[tokio::test]
async fn real_launcher_observes_each_of_ten_complete_developer_generations() {
    let _serial = DEVELOPER_LAUNCHER_TEST_SERIAL.lock().await;
    let temp = tempfile::tempdir().expect("temp state");
    let state_home = temp.path().join("state");
    let sources = temp.path().join("sources");
    std::fs::create_dir_all(&sources).expect("sources");
    let runtime_state = state_home.join("runtime/activation");
    std::fs::create_dir_all(&runtime_state).expect("runtime state fixture");
    let mut digests = BTreeSet::new();
    for index in 0..10 {
        let label = format!("generation-{index}");
        digests.insert(publish(&state_home, &sources, &label));
        std::fs::write(
            runtime_state.join("pending.json"),
            match index % 3 {
                0 => b"stopped".as_slice(),
                1 => b"pending".as_slice(),
                _ => b"failed".as_slice(),
            },
        )
        .expect("independent Runtime state");
        let receipt = invoke(&state_home, json!({"tool_input": {}})).await;
        assert_eq!(receipt["generation"], label);
        assert!(
            receipt["launcherGenerationRoot"]
                .as_str()
                .is_some_and(|root| root.contains("/hooks/generations/blake3-256/"))
        );
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
    publish(&state_home, &sources, "old");
    let next_evaluator = evaluator(&sources, "new");
    let next = prepare_hook_generation(
        &state_home,
        HookGenerationCandidate {
            evaluator_binary: &next_evaluator,
            config: b"schemaVersion = 1\nlabel = \"new\"\n",
            compiled_matcher: b"matcher:new",
            registry: b"registry:new",
        },
    )
    .expect("prepare next");
    let barrier = Arc::new(tokio::sync::Barrier::new(33));
    let mut calls = Vec::new();
    for _ in 0..32 {
        let state_home = state_home.clone();
        let barrier = Arc::clone(&barrier);
        calls.push(tokio::spawn(async move {
            barrier.wait().await;
            invoke(&state_home, json!({"tool_input": {}})).await
        }));
    }
    barrier.wait().await;
    commit_hook_generation(&state_home, &next).expect("commit next");
    for call in calls {
        let receipt = call.await.expect("launcher task");
        assert!(matches!(
            receipt["generation"].as_str(),
            Some("old" | "new")
        ));
    }
}

#[tokio::test]
async fn mutable_config_source_edit_does_not_change_active_launcher_generation() {
    let _serial = DEVELOPER_LAUNCHER_TEST_SERIAL.lock().await;
    let temp = tempfile::tempdir().expect("temp state");
    let state_home = temp.path().join("state");
    let sources = temp.path().join("sources");
    std::fs::create_dir_all(&sources).expect("sources");
    let digest = publish(&state_home, &sources, "active");
    std::fs::write(
        state_home.join("hooks/config.toml"),
        "schemaVersion = 1\nsourceOnly = \"edited-without-install\"\n",
    )
    .expect("edit mutable config source");
    let receipt = invoke(&state_home, json!({"tool_input": {}})).await;
    assert_eq!(receipt["generation"], "active");
    assert_current(&state_home, &digest);
}

#[tokio::test]
async fn inherited_layer_zero_escape_precedes_missing_generation_but_payload_assignment_does_not() {
    let _serial = DEVELOPER_LAUNCHER_TEST_SERIAL.lock().await;
    let temp = tempfile::tempdir().expect("temp state");
    let missing_state = temp.path().join("missing");
    let mut escaped = HookProcessSpec::new(launcher(), temp.path());
    escaped.args = vec![
        "pre-tool".to_owned(),
        "--client".to_owned(),
        "codex".to_owned(),
    ];
    escaped
        .env
        .push(("ASP_NO_AGENT".to_owned(), "1".to_owned()));
    escaped.env.push((
        "ASP_STATE_HOME".to_owned(),
        missing_state.display().to_string(),
    ));
    let escaped = run_hook_process(&escaped, &json!({"invalid": true}))
        .await
        .expect("layer zero escape");
    assert_eq!(escaped.decision, json!({}));

    let sources = temp.path().join("sources");
    std::fs::create_dir_all(&sources).expect("sources");
    publish(temp.path(), &sources, "ordinary-policy");
    let observed = invoke(
        temp.path(),
        json!({"tool_input": {"command": "ASP_NO_AGENT=1 cat source.rs"}}),
    )
    .await;
    assert_eq!(observed["generation"], "ordinary-policy");
}

#[test]
fn parse_compile_validate_and_publication_failures_preserve_previous_generation() {
    let temp = tempfile::tempdir().expect("temp state");
    let state_home = temp.path().join("state");
    let sources = temp.path().join("sources");
    std::fs::create_dir_all(&sources).expect("sources");
    let previous_digest = publish(&state_home, &sources, "previous");
    let candidate_root = temp.path().join("candidate");
    std::fs::create_dir_all(&candidate_root).expect("candidate root");
    let registry =
        std::fs::read(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../agents/config.toml"))
            .expect("canonical registry fixture");
    assert!(!registry.is_empty());
    let config_path = candidate_root.join("config.toml");

    std::fs::write(&config_path, "[").expect("invalid config source");
    assert!(
        agent_semantic_hook::compile_hook_matcher_generation(&config_path, &candidate_root)
            .is_err()
    );
    assert_current(&state_home, &previous_digest);

    let semantic_invalid = agent_semantic_config::default_hook_client_config_template().replacen(
        "matcher = \"^apply_patch$\"",
        "matcher = \"[\"",
        1,
    );
    std::fs::write(&config_path, semantic_invalid).expect("semantic-invalid config");
    assert!(
        agent_semantic_hook::compile_hook_matcher_generation(&config_path, &candidate_root)
            .is_err()
    );
    assert_current(&state_home, &previous_digest);

    std::fs::write(
        &config_path,
        agent_semantic_config::default_hook_client_config_template(),
    )
    .expect("valid config");
    let mut compiled =
        agent_semantic_hook::compile_hook_matcher_generation(&config_path, &candidate_root)
            .expect("compile candidate")
            .bytes;
    compiled[0] ^= 0xff;
    assert!(agent_semantic_hook::validate_compiled_hook_matcher(&compiled).is_err());
    assert_current(&state_home, &previous_digest);

    let next = prepare_candidate(&state_home, &sources, "next");
    let lock_path = state_home.join("hooks/publication.lock");
    let lock = std::fs::OpenOptions::new()
        .create(true)
        .read(true)
        .write(true)
        .open(lock_path)
        .expect("publication lock");
    lock.try_lock_exclusive().expect("hold publication lock");
    assert!(commit_hook_generation(&state_home, &next).is_err());
    FileExt::unlock(&lock).expect("release publication lock");
    assert_current(&state_home, &previous_digest);
}

fn assert_current(state_home: &Path, expected: &str) {
    let current = read_current_hook_generation(state_home)
        .expect("read current generation")
        .expect("current generation");
    assert_eq!(current.generation_digest.as_str(), expected);
}
