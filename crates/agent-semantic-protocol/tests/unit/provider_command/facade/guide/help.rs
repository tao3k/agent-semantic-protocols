use crate::provider_command::support::{
    asp_command, make_executable, provider, temp_project_root, write_activation,
};

#[test]
fn language_facade_help_lists_client_and_provider_commands() {
    let root = temp_project_root("provider-language-help-facade");

    let output = asp_command(&root)
        .args(["gerbil-scheme", "--help"])
        .output()
        .expect("run asp gerbil-scheme --help");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&output.stderr), "");
    let stdout = String::from_utf8(output.stdout).expect("stdout");
    assert!(
        stdout.contains("Use a language-owned ASP facade"),
        "{stdout}"
    );
    assert!(
        stdout.contains("Usage: asp gerbil-scheme [COMMAND]"),
        "{stdout}"
    );
    assert!(stdout.contains("projection"), "{stdout}");
    assert!(stdout.contains("ast-patch"), "{stdout}");
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn language_facade_guide_help_is_handled_by_asp_without_provider_spawn() {
    let root = temp_project_root("provider-guide-help-facade");
    let profile_bin_dir = root.join(".profile-bin");
    std::fs::create_dir_all(&profile_bin_dir).expect("create profile bin dir");
    let provider_path = profile_bin_dir.join("rs-harness");
    std::fs::write(
        &provider_path,
        "#!/bin/sh\nprintf 'provider should not run\\n' >&2\nexit 42\n",
    )
    .expect("write provider");
    make_executable(&provider_path);

    write_activation(&root, &[provider("rust", Vec::new())]);

    let output = asp_command(&root)
        .args(["rust", "guide", "--help", "--workspace", "."])
        .output()
        .expect("run asp rust guide --help");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&output.stderr), "");
    let stdout = String::from_utf8(output.stdout).expect("stdout");
    assert!(
        stdout.contains("Usage: asp rust guide [OPTIONS] [ARGS]..."),
        "{stdout}"
    );
    assert!(stdout.contains("--workspace <ROOT>"), "{stdout}");
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn language_facade_search_help_documents_workspace_and_treesitter_discovery() {
    let root = temp_project_root("provider-search-help-facade");
    let profile_bin_dir = root.join(".profile-bin");
    std::fs::create_dir_all(&profile_bin_dir).expect("create profile bin dir");
    let provider_path = profile_bin_dir.join("rs-harness");
    std::fs::write(
        &provider_path,
        "#!/bin/sh\nprintf 'provider should not run\\n' >&2\nexit 42\n",
    )
    .expect("write provider");
    make_executable(&provider_path);

    write_activation(&root, &[provider("rust", Vec::new())]);

    let output = asp_command(&root)
        .args(["rust", "search", "--help", "--workspace", "."])
        .output()
        .expect("run asp rust search --help");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&output.stderr), "");
    let stdout = String::from_utf8(output.stdout).expect("stdout");
    assert!(
        stdout.contains("Usage: asp rust search [OPTIONS] [ARGS]..."),
        "{stdout}"
    );
    assert!(stdout.contains("--workspace <ROOT>"), "{stdout}");
    assert!(stdout.contains("--treesitter-query <QUERY>"), "{stdout}");
    assert!(
        stdout.contains("Tree-sitter discovery belongs to search"),
        "{stdout}"
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn language_facade_rejects_unknown_agent_subcommand() {
    let root = temp_project_root("provider-unknown-agent-subcommand");
    write_activation(&root, &[provider("rust", Vec::new())]);

    let output = asp_command(&root)
        .args(["rust", "agent", "status", "."])
        .output()
        .expect("run asp rust agent status");

    assert!(!output.status.success(), "{output:?}");
    let stderr = String::from_utf8(output.stderr).expect("stderr");
    assert!(stderr.contains("usage: asp <"), "{stderr}");
    assert!(
        stderr.contains("<guide|search|query|check|cache|info|bench|projection"),
        "{stderr}"
    );
    let _ = std::fs::remove_dir_all(root);
}
