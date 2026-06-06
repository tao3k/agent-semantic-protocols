use crate::provider_command::support::{
    asp_command, make_executable, prepend_path, provider, temp_project_root, write_activation,
    write_echo_provider,
};

#[test]
fn language_facade_guide_routes_to_activation_prefix() {
    let root = temp_project_root("provider-guide-facade");
    let profile_bin_dir = root.join(".profile-bin");
    let path_bin_dir = root.join(".path-bin");
    write_echo_provider(&profile_bin_dir, "rs-harness", "profile");
    std::fs::create_dir_all(&path_bin_dir).expect("create path bin dir");
    let path_provider = path_bin_dir.join("rs-harness");
    std::fs::write(
        &path_provider,
        "#!/bin/sh\nprintf 'path provider should not run\\n' >&2\nexit 42\n",
    )
    .expect("write path provider");
    make_executable(&path_provider);

    write_activation(
        &root,
        &[provider(
            "rust",
            vec![profile_bin_dir.join("rs-harness").display().to_string()],
        )],
    );

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
    assert!(stdout.contains("profile args=[guide]\n"), "{stdout}");
    assert!(
        stdout.contains("|cmd agent-doctor=asp rust agent doctor --json ."),
        "{stdout}"
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn typescript_language_facade_guide_routes_to_legacy_agent_guide() {
    let root = temp_project_root("provider-typescript-guide-facade");
    let profile_bin_dir = root.join(".profile-bin");
    write_echo_provider(&profile_bin_dir, "ts-harness", "profile");

    write_activation(
        &root,
        &[provider(
            "typescript",
            vec![profile_bin_dir.join("ts-harness").display().to_string()],
        )],
    );

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
    assert!(stdout.contains("profile args=[agent][guide]\n"), "{stdout}");
    assert!(
        stdout.contains("|cmd agent-doctor=asp typescript agent doctor --json ."),
        "{stdout}"
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn python_language_facade_guide_routes_to_legacy_agent_guide() {
    let root = temp_project_root("provider-python-guide-facade");
    let profile_bin_dir = root.join(".profile-bin");
    write_echo_provider(&profile_bin_dir, "py-harness", "profile");

    write_activation(
        &root,
        &[provider(
            "python",
            vec![profile_bin_dir.join("py-harness").display().to_string()],
        )],
    );

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
    assert!(stdout.contains("profile args=[agent][guide]\n"), "{stdout}");
    assert!(
        stdout.contains("|cmd agent-doctor=asp python agent doctor --json ."),
        "{stdout}"
    );
    let _ = std::fs::remove_dir_all(root);
}
