use super::support::{provider, temp_project_root, write_activation, write_marker_provider};
use std::env;
use std::process::Command;

#[test]
fn missing_activation_is_reported_before_provider_spawn() {
    let root = temp_project_root("missing-activation");

    let output = Command::new(env!("CARGO_BIN_EXE_asp"))
        .current_dir(&root)
        .args(["rust", "search", "prime", "."])
        .output()
        .expect("run asp rust search");

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("[asp-provider] activation=missing"),
        "{stderr}"
    );
    assert!(
        stderr.contains(".cache/agent-semantic-protocol/hooks/activation.json"),
        "{stderr}"
    );
    assert!(
        stderr.contains("|reason provider-activation-missing"),
        "{stderr}"
    );
    assert!(
        stderr.contains("|cmd install=asp hook install --client codex ."),
        "{stderr}"
    );
    assert!(stderr.contains("|cmd guide=asp guide"), "{stderr}");
    assert!(stderr.contains("|cmd providers=asp providers"), "{stderr}");
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
        let output = Command::new(env!("CARGO_BIN_EXE_asp"))
            .current_dir(&root)
            .args(&args)
            .output()
            .expect("run asp diagnostic command");
        assert!(output.status.success(), "{args:?}: {output:?}");
        let stdout = String::from_utf8_lossy(&output.stdout);
        match args.as_slice() {
            ["guide"] => assert!(stdout.contains("[asp-guide]"), "{stdout}"),
            ["doctor"] => {
                assert!(stdout.contains("[asp-doctor] status=degraded"), "{stdout}");
                assert!(stdout.contains("activation=missing"), "{stdout}");
                assert!(
                    stdout.contains("|cmd install=asp hook install --client codex ."),
                    "{stdout}"
                );
            }
            ["providers"] => {
                assert!(
                    stdout.contains("[asp-providers] activation=missing"),
                    "{stdout}"
                );
                assert!(stdout.contains("providers=0"), "{stdout}");
            }
            ["cache", "status"] => {
                assert!(
                    stdout.contains("[asp-cache] status=unavailable"),
                    "{stdout}"
                );
                assert!(stdout.contains("activation=missing"), "{stdout}");
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

    let output = Command::new(env!("CARGO_BIN_EXE_asp"))
        .current_dir(&root)
        .env("PATH", &bin_dir)
        .args(["rust", "fmt", "."])
        .output()
        .expect("run protocol");
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("<search|query|check|agent guide|agent doctor|ast-patch|evidence>"),
        "{stderr}"
    );
    assert!(!called.exists(), "provider should not have been spawned");

    assert!(!called.exists(), "provider should not have been spawned");
    std::fs::remove_dir_all(root).ok();
}
#[test]
fn julia_is_not_a_global_language_facade() {
    let root = temp_project_root("provider-dash-language-query");
    let bin_dir = root.join(".bin");
    super::support::write_echo_provider(&bin_dir, "rs-harness", "rs");
    write_activation(&root, &[provider("rust", Vec::new())]);
    let output = Command::new(env!("CARGO_BIN_EXE_asp"))
        .current_dir(&root)
        .env("PATH", &bin_dir)
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
    let _ = std::fs::remove_dir_all(root);

    for command in ["search", "query", "check"] {
        let output = Command::new(env!("CARGO_BIN_EXE_asp"))
            .args([command, "--language", "rust", "."])
            .output()
            .expect("run asp command");
        assert!(!output.status.success());
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            stderr.contains(&format!("asp {command} is not a public command surface")),
            "{stderr}"
        );
        assert!(
            stderr.contains(&format!("use asp <rust|typescript|python> {command}")),
            "{stderr}"
        );
    }
    let output = Command::new(env!("CARGO_BIN_EXE_asp"))
        .args(["julia", "search", "prime", "."])
        .output()
        .expect("run asp");
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("rust|typescript|python"), "{stderr}");
    assert!(!stderr.contains("julia"), "{stderr}");
}
