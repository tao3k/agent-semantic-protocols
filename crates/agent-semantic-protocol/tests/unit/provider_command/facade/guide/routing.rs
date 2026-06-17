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
    write_provider_bin_override(&root, "rust", &profile_bin_dir.join("rs-harness"));
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
    write_provider_bin_override(&root, "rust", &provider_bin);

    write_activation(
        &root,
        &[provider("rust", vec![provider_bin.display().to_string()])],
    );

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
    let profile_bin_dir = root.join(".profile-bin");
    std::fs::create_dir_all(&profile_bin_dir).expect("create profile bin dir");
    let provider_bin = profile_bin_dir.join("gerbil-scheme-harness");
    std::fs::write(
        &provider_bin,
        "#!/bin/sh\nprintf 'provider should not run\\n' >&2\nexit 42\n",
    )
    .expect("write provider");
    make_executable(&provider_bin);
    write_activation(
        &root,
        &[provider(
            "gerbil-scheme",
            vec![provider_bin.display().to_string()],
        )],
    );

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
    let profile_bin_dir = root.join(".profile-bin");
    write_echo_provider(&profile_bin_dir, "ts-harness", "profile");
    write_provider_bin_override(&root, "typescript", &profile_bin_dir.join("ts-harness"));

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
        stdout.contains("|cmd agent-doctor=asp typescript agent doctor --workspace . --json"),
        "{stdout}"
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn python_language_facade_guide_routes_to_agent_guide() {
    let root = temp_project_root("provider-python-guide-facade");
    let profile_bin_dir = root.join(".profile-bin");
    write_echo_provider(&profile_bin_dir, "py-harness", "profile");
    write_provider_bin_override(&root, "python", &profile_bin_dir.join("py-harness"));

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
        stdout.contains("|cmd agent-doctor=asp python agent doctor --workspace . --json"),
        "{stdout}"
    );
    let _ = std::fs::remove_dir_all(root);
}

fn write_provider_bin_override(
    root: &std::path::Path,
    language_id: &str,
    provider_bin: &std::path::Path,
) {
    std::fs::write(
        root.join("asp.toml"),
        format!(
            "[languages.{language_id}]\nbin = \"{}\"\n",
            provider_bin.display()
        ),
    )
    .expect("write asp.toml provider override");
}
