use crate::provider_command::support::{
    asp_command, install_state_home_provider, make_executable, prepend_path, provider,
    state_runtime_bin, temp_project_root, write_activation, write_echo_provider,
};

#[test]
fn language_facade_guide_routes_to_state_home_runtime() {
    let root = temp_project_root("provider-guide-facade");
    let path_bin_dir = root.join(".path-bin");
    write_echo_provider(&state_runtime_bin(&root), "rs-harness", "state-home");
    std::fs::create_dir_all(&path_bin_dir).expect("create path bin dir");
    let path_provider = path_bin_dir.join("rs-harness");
    std::fs::write(
        &path_provider,
        "#!/bin/sh\nprintf 'path provider should not run\\n' >&2\nexit 42\n",
    )
    .expect("write path provider");
    make_executable(&path_provider);

    write_activation(&root, &[provider("rust", Vec::new())]);

    let output = asp_command(&root)
        .env("PRJ_CACHE_HOME", root.join(".cache"))
        .env("PATH", prepend_path(&path_bin_dir))
        .args(["rust", "guide", "."])
        .output()
        .expect("run asp rust guide");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("stdout");
    assert!(stdout.contains("state-home args=[guide]\n"), "{stdout}");
    assert!(
        stdout.contains("|cmd agent-doctor=asp rust agent doctor --workspace . --json"),
        "{stdout}"
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn language_facade_guide_code_preserves_pure_provider_stdout() {
    let root = temp_project_root("provider-guide-code-facade");
    let profile_bin_dir = root.join(".profile-bin");
    std::fs::create_dir_all(&profile_bin_dir).expect("create profile bin dir");
    let provider_bin = profile_bin_dir.join("rs-harness");
    std::fs::write(
        &provider_bin,
        "#!/bin/sh\nprintf ';;; source comment\\n(def (example) #t)\\n'\n",
    )
    .expect("write code provider");
    make_executable(&provider_bin);
    install_state_home_provider(&root, "rust", &provider_bin);

    write_activation(&root, &[provider("rust", Vec::new())]);

    let output = asp_command(&root)
        .env("PRJ_CACHE_HOME", root.join(".cache"))
        .args(["rust", "guide", "--code"])
        .output()
        .expect("run asp rust guide --code");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("stdout");
    assert_eq!(stdout, ";;; source comment\n(def (example) #t)\n");
    assert!(!stdout.contains("agent-doctor"), "{stdout}");
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn language_facade_cache_source_index_lookup_routes_to_client_backend() {
    let root = temp_project_root("provider-cache-source-index-facade");
    write_activation(&root, &[provider("gerbil-scheme", Vec::new())]);

    let output = asp_command(&root)
        .env("PRJ_CACHE_HOME", root.join(".cache"))
        .args([
            "gerbil-scheme",
            "cache",
            "source-index",
            "lookup",
            "--query",
            "runtime",
            "--index-root",
            ".",
        ])
        .output()
        .expect("run asp gerbil-scheme cache source-index lookup");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("stdout");
    let stderr = String::from_utf8(output.stderr).expect("stderr");
    assert!(
        stdout.contains("noOutput reason=source-index-missing-db"),
        "{stdout}"
    );
    assert!(!stderr.contains("provider should not run"), "{stderr}");
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn typescript_language_facade_guide_routes_to_agent_guide() {
    let root = temp_project_root("provider-typescript-guide-facade");
    write_echo_provider(&state_runtime_bin(&root), "asp-typescript", "state-home");
    write_activation(&root, &[provider("typescript", Vec::new())]);

    let output = asp_command(&root)
        .env("PRJ_CACHE_HOME", root.join(".cache"))
        .args(["typescript", "guide", "."])
        .output()
        .expect("run asp typescript guide");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("stdout");
    assert!(
        stdout.contains("state-home args=[agent][guide]\n"),
        "{stdout}"
    );
    assert!(
        stdout.contains("|cmd agent-doctor=asp typescript agent doctor --workspace . --json"),
        "{stdout}"
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn python_language_facade_guide_routes_to_agent_guide() {
    let root = temp_project_root("provider-python-guide-facade");
    write_echo_provider(&state_runtime_bin(&root), "py-harness", "state-home");
    write_activation(&root, &[provider("python", Vec::new())]);

    let output = asp_command(&root)
        .env("PRJ_CACHE_HOME", root.join(".cache"))
        .args(["python", "guide", "."])
        .output()
        .expect("run asp python guide");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("stdout");
    assert!(
        stdout.contains("state-home args=[agent][guide]\n"),
        "{stdout}"
    );
    assert!(
        stdout.contains("|cmd agent-doctor=asp python agent doctor --workspace . --json"),
        "{stdout}"
    );
    let _ = std::fs::remove_dir_all(root);
}
