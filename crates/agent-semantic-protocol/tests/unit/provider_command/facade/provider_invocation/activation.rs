use crate::provider_command::support::{
    asp_command, provider, state_runtime_bin, temp_project_root, write_activation,
    write_cache_source_fixture, write_echo_provider,
};

#[test]
fn language_facade_uses_state_home_runtime_binary() {
    let root = temp_project_root("provider-state-home-facade");
    let runtime_bin = state_runtime_bin(&root);
    write_echo_provider(&runtime_bin, "rs-harness", "state-home");
    write_activation(&root, &[provider("rust", Vec::new())]);

    let output = asp_command(&root)
        .env("PRJ_CACHE_HOME", root.join(".cache"))
        .env_remove("PATH")
        .args(["rust", "evidence", "."])
        .output()
        .expect("run asp rust evidence with provider bin override");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8(output.stdout).expect("stdout"),
        "state-home args=[evidence][.]\n"
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn language_facade_uses_state_home_runtime_without_activation_prefix() {
    let root = temp_project_root("provider-state-home-wrapper-facade");
    let runtime_bin = state_runtime_bin(&root);
    write_echo_provider(&runtime_bin, "rs-harness", "state-home");
    write_activation(&root, &[provider("rust", Vec::new())]);

    let output = asp_command(&root)
        .env("PRJ_CACHE_HOME", root.join(".cache"))
        .env_remove("PATH")
        .args(["rust", "evidence", "."])
        .output()
        .expect("run asp rust evidence");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8(output.stdout).expect("stdout"),
        "state-home args=[evidence][.]\n"
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn language_facade_query_uses_state_home_runtime_binary() {
    let root = temp_project_root("provider-query-state-home-facade");
    write_cache_source_fixture(&root);
    let runtime_bin = state_runtime_bin(&root);
    write_echo_provider(&runtime_bin, "rs-harness", "state-home");
    write_activation(&root, &[provider("rust", Vec::new())]);

    let output = asp_command(&root)
        .env("PRJ_CACHE_HOME", root.join(".cache"))
        .env_remove("PATH")
        .args(["rust", "query", "src/lib.rs", "--workspace", "."])
        .output()
        .expect("run asp rust query");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8(output.stdout).expect("stdout"),
        "state-home args=[query][src/lib.rs]\n"
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn language_facade_guide_uses_state_home_runtime_binary() {
    let root = temp_project_root("provider-guide-state-home-facade");
    let runtime_bin = state_runtime_bin(&root);
    write_echo_provider(&runtime_bin, "rs-harness", "state-home");
    write_activation(&root, &[provider("rust", Vec::new())]);

    for args in [
        ["rust", "query", "guide", "."],
        ["rust", "search", "guide", "."],
    ] {
        let output = asp_command(&root)
            .env("PRJ_CACHE_HOME", root.join(".cache"))
            .env_remove("PATH")
            .args(args)
            .output()
            .expect("run asp rust guide");

        assert!(
            output.status.success(),
            "args={args:?} stderr: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let stdout = String::from_utf8(output.stdout).expect("stdout");
        assert!(stdout.starts_with("state-home args="), "{stdout}");
    }
    let _ = std::fs::remove_dir_all(root);
}
