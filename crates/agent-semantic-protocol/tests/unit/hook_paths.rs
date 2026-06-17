use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

#[test]
fn hook_paths_reports_runtime_layout_without_materializing_state() {
    let root = temp_project_root("hook-paths");
    let output = Command::new(env!("CARGO_BIN_EXE_asp"))
        .current_dir(&root)
        .env("PATH", "")
        .env("PRJ_CACHE_HOME", root.join(".cache"))
        .args(["hook", "paths", "."])
        .output()
        .expect("run asp hook paths");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("activation=")
            && stdout.contains(".cache/agent-semantic-protocol/hooks/activation.json"),
        "stdout: {stdout}"
    );
    assert!(
        stdout.contains("hookStateDir=")
            && stdout.contains(".cache/agent-semantic-protocol/hooks/state"),
        "stdout: {stdout}"
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
    root
}
