use crate::provider_command::support::{
    asp_command, provider, state_runtime_bin, temp_project_root, write_activation,
    write_echo_provider,
};

#[test]
fn state_home_runtime_binary_ignores_ambient_path_provider() {
    let root = temp_project_root("provider-state-home-over-path");
    let path_bin = root.join(".path-bin");
    write_echo_provider(&path_bin, "rs-harness", "path");
    write_echo_provider(
        &state_runtime_bin(&root),
        "rs-harness",
        "state-home-runtime",
    );
    write_activation(&root, &[provider("rust", Vec::new())]);

    let output = asp_command(&root)
        .env("PATH", &path_bin)
        .args(["rust", "guide", "."])
        .output()
        .expect("run asp rust guide");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("stdout");
    assert!(
        stdout.contains("state-home-runtime args=[guide]"),
        "{stdout}"
    );
    assert!(!stdout.contains("path args="), "{stdout}");
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn nested_workspace_uses_same_state_home_runtime_binary() {
    let root = temp_project_root("provider-state-home-nested-workspace");
    write_echo_provider(
        &state_runtime_bin(&root),
        "rs-harness",
        "state-home-runtime",
    );
    write_activation(&root, &[provider("rust", Vec::new())]);
    let nested_root = root.join("languages/rust-lang-project-harness");
    std::fs::create_dir_all(&nested_root).expect("create nested root");

    let output = asp_command(&root)
        .args(["rust", "guide", "languages/rust-lang-project-harness"])
        .output()
        .expect("run asp rust guide for nested workspace");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("stdout");
    assert!(
        stdout.contains("state-home-runtime args=[guide]"),
        "{stdout}"
    );
    let _ = std::fs::remove_dir_all(root);
}
