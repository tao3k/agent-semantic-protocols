use std::process::Command;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;

#[test]
fn hook_paths_reports_runtime_layout_without_materializing_state() {
    let root = temp_project_root("hook-paths");
    let state_home = temp_project_root("hook-paths-state");
    let expected = agent_semantic_runtime::project_state_paths_with_state_home(&root, &state_home)
        .expect("resolved project state paths");
    let output = Command::new(env!("CARGO_BIN_EXE_asp"))
        .current_dir(&root)
        .env("ASP_STATE_HOME", &state_home)
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
        stdout.contains(&format!(
            "activation={}",
            expected.activation_path.display()
        )),
        "stdout: {stdout}"
    );
    assert!(
        stdout.contains(&format!(
            "hookStateDir={}",
            expected.hook_state_dir.display()
        )),
        "stdout: {stdout}"
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
    root
}
