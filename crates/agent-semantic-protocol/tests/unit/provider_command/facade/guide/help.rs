use crate::provider_command::support::{
    asp_command, make_executable, provider, temp_project_root, write_activation,
};

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
        .args(["rust", "guide", "--help", "."])
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
        stdout.contains("usage: asp rust guide [--help] [PROJECT_ROOT]"),
        "{stdout}"
    );
    assert!(stdout.contains("query guide treesitter"), "{stdout}");
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
    assert!(
        stderr.contains(
            "usage: asp <rust|typescript|python|julia|gerbil-scheme|org|md> [--help|--version] <guide|search|query|check|agent doctor|ast-patch|evidence> ..."
        ),
        "{stderr}"
    );
    let _ = std::fs::remove_dir_all(root);
}
