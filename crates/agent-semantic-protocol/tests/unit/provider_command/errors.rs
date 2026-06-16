use super::support::{
    asp_command, prepend_path, provider, temp_project_root, write_activation, write_marker_provider,
};

#[test]
fn missing_activation_is_reported_before_provider_spawn() {
    let root = temp_project_root("missing-activation");
    std::fs::write(
        root.join("Cargo.toml"),
        "[package]\nname = \"missing-activation\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
    )
    .expect("write Cargo.toml");
    std::fs::create_dir_all(root.join("src")).expect("create src dir");
    std::fs::write(root.join("src/lib.rs"), "").expect("write lib.rs");

    let output = asp_command(&root)
        .env_remove("PATH")
        .args(["rust", "search", "prime", "."])
        .output()
        .expect("run asp rust search");

    assert!(
        !output.status.success(),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("failed to read activation"), "{stderr}");
    assert!(
        stderr.contains(".cache/agent-semantic-protocol/hooks/activation.json"),
        "{stderr}"
    );
    assert!(
        stderr.contains("failed to sync generated activation"),
        "{stderr}"
    );
    assert!(
        stderr
            .contains("expected PATH to contain at least one executable semantic provider binary"),
        "{stderr}"
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn diagnostic_commands_do_not_require_activation() {
    let root = temp_project_root("diagnostics-without-activation");
    for args in [
        vec!["guide"],
        vec!["doctor"],
        vec!["providers"],
        vec!["cache", "status"],
    ] {
        let output = asp_command(&root)
            .args(&args)
            .output()
            .expect("run asp diagnostic command");
        assert!(output.status.success(), "{args:?}: {output:?}");
        let stdout = String::from_utf8_lossy(&output.stdout);
        match args.as_slice() {
            ["guide"] => assert!(stdout.contains("[asp-guide]"), "{stdout}"),
            ["doctor"] => {
                assert!(stdout.contains("[asp-doctor] status="), "{stdout}");
                assert!(
                    stdout.contains("status=degraded") || stdout.contains("status=ok"),
                    "{stdout}"
                );
                if stdout.contains("status=degraded") {
                    assert!(stdout.contains("activation=missing"), "{stdout}");
                    assert!(
                        stdout.contains("|cmd install=asp hook install --client codex ."),
                        "{stdout}"
                    );
                } else {
                    assert!(stdout.contains("providers="), "{stdout}");
                    assert!(stdout.contains("server=not-required"), "{stdout}");
                }
            }
            ["providers"] => {
                assert!(stdout.contains("[asp-providers]"), "{stdout}");
                if stdout.contains("activation=missing") {
                    assert!(stdout.contains("providers=0"), "{stdout}");
                } else {
                    assert!(stdout.contains("activation="), "{stdout}");
                    assert!(stdout.contains("providers="), "{stdout}");
                }
            }
            ["cache", "status"] => {
                assert!(stdout.contains("[asp-cache] status=missing"), "{stdout}");
                assert!(stdout.contains("activation="), "{stdout}");
            }
            _ => unreachable!("covered args"),
        }
    }
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn non_agent_command_surface_is_rejected_without_provider_spawn() {
    let root = temp_project_root("protocol-provider-errors-non-agent");
    let bin_dir = root.join(".bin");
    let called = root.join("called");
    write_marker_provider(&bin_dir, "rs-harness", &called);
    write_activation(&root, &[provider("rust", Vec::new())]);

    let output = asp_command(&root)
        .env("PATH", prepend_path(&bin_dir))
        .args(["rust", "fmt", "."])
        .output()
        .expect("run protocol");
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("<guide|search|query|check|agent doctor|ast-patch|evidence>"),
        "{stderr}"
    );
    assert!(!called.exists(), "provider should not have been spawned");

    assert!(!called.exists(), "provider should not have been spawned");
    std::fs::remove_dir_all(root).ok();
}
#[test]
fn provider_language_facades_forward_language_like_provider_args() {
    let root = temp_project_root("provider-dash-language-query");
    let bin_dir = root.join(".bin");
    super::support::write_echo_provider(&bin_dir, "rs-harness", "rs");
    super::support::write_echo_provider(&bin_dir, "asp-julia-harness", "jl");
    write_activation(
        &root,
        &[provider("rust", Vec::new()), provider("julia", Vec::new())],
    );
    let output = asp_command(&root)
        .env("PATH", prepend_path(&bin_dir))
        .args([
            "rust",
            "search",
            "fzf",
            "--query",
            "--language",
            "owner",
            "tests",
            "--view",
            "seeds",
            ".",
        ])
        .output()
        .expect("run asp rust search");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "stderr={stderr}\nstdout={}",
        String::from_utf8_lossy(&output.stdout)
    );
    assert!(
        !stderr.contains("--language has been removed"),
        "provider args were intercepted by the client parser: {stderr}"
    );
    let stdout = String::from_utf8(output.stdout).expect("stdout");
    for expected in [
        "search",
        "fzf",
        "--query",
        "--language",
        "owner",
        "tests",
        "--view",
        "seeds",
    ] {
        assert!(stdout.contains(expected), "missing {expected}: {stdout}");
    }
    let output = asp_command(&root)
        .args(["check", "--language", "rust", "."])
        .output()
        .expect("run asp check");
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("asp check is not a public command surface"),
        "{stderr}"
    );
    assert!(
        stderr.contains("use asp <rust|typescript|python|julia> check"),
        "{stderr}"
    );
    let output = asp_command(&root)
        .args(["julia", "search", "prime", "."])
        .output()
        .expect("run asp julia search");
    assert!(
        output.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("stdout");
    assert!(stdout.contains("jl args=[search][prime]"), "{stdout}");
    let _ = std::fs::remove_dir_all(root);
}
