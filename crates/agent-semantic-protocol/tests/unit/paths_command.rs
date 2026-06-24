use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::Value;

#[test]
fn paths_reports_project_root_and_org_state_paths() {
    let root = temp_project_root("paths");
    let output = Command::new(env!("CARGO_BIN_EXE_asp"))
        .current_dir(&root)
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
        stdout.contains(".cache/agent-semantic-protocol/org/templates/ASP_ORG_SKILL.org"),
        "stdout: {stdout}"
    );
    assert!(
        stdout.contains(".cache/agent-semantic-protocol/artifacts/org"),
        "stdout: {stdout}"
    );
    assert!(!root.join(".cache").exists());
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn paths_get_returns_single_absolute_field() {
    let root = temp_project_root("paths-get");
    let output = Command::new(env!("CARGO_BIN_EXE_asp"))
        .current_dir(&root)
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
        root.join(".cache")
            .join("agent-semantic-protocol")
            .join("org")
            .join("templates")
            .join("ASP_ORG_SKILL.org")
            .display()
            .to_string()
    );
    assert!(!root.join(".cache").exists());
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn paths_json_is_machine_readable() {
    let root = temp_project_root("paths-json");
    let output = Command::new(env!("CARGO_BIN_EXE_asp"))
        .current_dir(&root)
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
    assert!(
        value["orgArtifacts"]
            .as_str()
            .expect("org artifacts")
            .ends_with(".cache/agent-semantic-protocol/artifacts/org")
    );
    assert!(!root.join(".cache").exists());
    let _ = std::fs::remove_dir_all(root);
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
