use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use agent_semantic_runtime::state_core::ResolvedState;
use serde_json::Value;

#[test]
fn paths_reports_project_root_and_org_state_paths() {
    let root = temp_project_root("paths");
    let state_home = temp_project_root("paths-state");
    let resolved = ResolvedState::resolve_with_state_home(&root, &state_home)
        .expect("resolved state")
        .paths;
    let expected_org_artifacts = resolved.artifacts_dir.join("org");
    let expected_hook_state_dir = resolved
        .workspace_dir
        .join("live")
        .join("hooks")
        .join("state");
    let expected_hook_cache_dir = resolved
        .workspace_dir
        .join("live")
        .join("hooks")
        .join("cache");
    let output = Command::new(env!("CARGO_BIN_EXE_asp"))
        .current_dir(&root)
        .env("ASP_STATE_HOME", &state_home)
        .env("PATH", "")
        .env("PRJ_CACHE_HOME", root.join(".cache"))
        .arg("paths")
        .output()
        .expect("run asp paths");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains(&format!("projectRoot={}", root.display())),
        "stdout: {stdout}"
    );
    assert!(
        stdout.contains(&format!("stateRoot={}", state_home.display())),
        "stdout: {stdout}"
    );
    assert!(
        stdout.contains(&format!(
            "orgStateSkill={}",
            state_home
                .join("org")
                .join("templates")
                .join("ASP_ORG_SKILL.org")
                .display()
        )),
        "stdout: {stdout}"
    );
    assert!(
        stdout.contains(&format!(
            "orgArtifacts={}",
            expected_org_artifacts.display()
        )),
        "stdout: {stdout}"
    );
    assert!(
        stdout.contains(&format!(
            "hookStateDir={}",
            expected_hook_state_dir.display()
        )),
        "stdout: {stdout}"
    );
    assert!(
        stdout.contains(&format!(
            "hookCacheDir={}",
            expected_hook_cache_dir.display()
        )),
        "stdout: {stdout}"
    );
    assert!(stdout.contains("/projects/by-id/"), "stdout: {stdout}");
    assert!(stdout.contains("/live/client/"), "stdout: {stdout}");
    assert!(!root.join(".cache").exists());
    let _ = std::fs::remove_dir_all(root);
    let _ = std::fs::remove_dir_all(state_home);
}

#[test]
fn paths_get_returns_single_absolute_field() {
    let root = temp_project_root("paths-get");
    let state_home = temp_project_root("paths-get-state");
    let output = Command::new(env!("CARGO_BIN_EXE_asp"))
        .current_dir(&root)
        .env("ASP_STATE_HOME", &state_home)
        .env("PATH", "")
        .env("PRJ_CACHE_HOME", root.join(".cache"))
        .args(["paths", "--get", "orgStateSkill"])
        .output()
        .expect("run asp paths --get");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(
        stdout.trim(),
        state_home
            .join("org")
            .join("templates")
            .join("ASP_ORG_SKILL.org")
            .display()
            .to_string()
    );
    assert!(!root.join(".cache").exists());
    let _ = std::fs::remove_dir_all(root);
    let _ = std::fs::remove_dir_all(state_home);
}

#[test]
fn paths_json_is_machine_readable() {
    let root = temp_project_root("paths-json");
    let state_home = temp_project_root("paths-json-state");
    let resolved = ResolvedState::resolve_with_state_home(&root, &state_home)
        .expect("resolved state")
        .paths;
    let expected_org_artifacts = resolved.artifacts_dir.join("org");
    let expected_hook_state_dir = resolved
        .workspace_dir
        .join("live")
        .join("hooks")
        .join("state");
    let expected_hook_cache_dir = resolved
        .workspace_dir
        .join("live")
        .join("hooks")
        .join("cache");
    let output = Command::new(env!("CARGO_BIN_EXE_asp"))
        .current_dir(&root)
        .env("ASP_STATE_HOME", &state_home)
        .env("PATH", "")
        .env("PRJ_CACHE_HOME", root.join(".cache"))
        .args(["paths", "--json"])
        .output()
        .expect("run asp paths --json");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let value: Value = serde_json::from_slice(&output.stdout).expect("json paths");
    assert_eq!(value["projectRoot"], root.display().to_string());
    assert_eq!(value["stateRoot"], state_home.display().to_string());
    assert_eq!(
        value["orgArtifacts"],
        expected_org_artifacts.display().to_string()
    );
    assert_eq!(
        value["hookStateDir"],
        expected_hook_state_dir.display().to_string()
    );
    assert_eq!(
        value["hookCacheDir"],
        expected_hook_cache_dir.display().to_string()
    );
    assert_eq!(
        value["activation"],
        expected_hook_state_dir
            .join("activation.json")
            .display()
            .to_string()
    );
    assert!(
        value["clientCacheDir"]
            .as_str()
            .expect("client cache dir")
            .contains("/projects/by-id/")
    );
    assert!(
        value["cacheManifest"]
            .as_str()
            .expect("cache manifest")
            .contains("/live/client/cache-manifest.json")
    );
    assert!(!root.join(".cache").exists());
    let _ = std::fs::remove_dir_all(root);
    let _ = std::fs::remove_dir_all(state_home);
}

fn temp_project_root(name: &str) -> std::path::PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let root = std::env::temp_dir().join(format!("agent-semantic-protocol-{name}-{unique}"));
    std::fs::create_dir_all(&root).expect("create temp project root");
    root.canonicalize().expect("canonical temp project root")
}
