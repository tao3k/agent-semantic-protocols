// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

#[path = "../../src/command/hook.rs"]
mod hook;
use std::process::Command;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;

mod hook_runtime {
    pub(crate) async fn run_hook_runtime_args(_args: Vec<String>) -> Result<(), String> {
        Ok(())
    }
}

mod hook_break_glass {
    pub(crate) fn run_hook_break_glass(_args: &[String]) -> Result<(), String> {
        Ok(())
    }
}

fn assert_hook_command_future(
    future: impl std::future::Future<Output = Result<(), String>> + Send,
) {
    drop(future);
}

#[test]
fn hook_command_exposes_an_async_adapter() {
    assert_hook_command_future(hook::run_hook_command(&[]));
}
fn args(values: &[&str]) -> Vec<String> {
    values.iter().map(|value| value.to_string()).collect()
}

#[test]
fn lifecycle_commands_delegate_to_hook_runtime() {
    assert_eq!(
        hook::forwarded_hook_args(&args(&["doctor", "--client", "codex", "."])).unwrap(),
        args(&["doctor", "--client", "codex", "."])
    );
    assert_eq!(
        hook::forwarded_hook_args(&args(&["paths", "."])).unwrap(),
        args(&["paths", "."])
    );
    assert_eq!(
        hook::forwarded_hook_args(&args(&["enablement", ".", "--json"])).unwrap(),
        args(&["enablement", ".", "--json"])
    );
}

#[test]
fn hook_control_commands_are_runtime_independent_and_events_are_not() {
    for command in ["break-glass", "doctor", "enablement", "paths", "refresh"] {
        assert!(hook::is_runtime_independent_control_command(&args(&[
            command
        ])));
    }
    for command in ["pre-tool", "post-tool", "session-start", "unknown"] {
        assert!(!hook::is_runtime_independent_control_command(&args(&[
            command
        ])));
    }
}

#[test]
fn help_requests_do_not_forward_to_hook_runtime() {
    for values in [&["--help"][..], &["-h"][..], &["help"][..]] {
        assert!(hook::is_help_request(&args(values)), "{values:?}");
    }
    for values in [
        &["doctor", "-h"][..],
        &["enablement", "--help"][..],
        &["paths", "--help"][..],
        &["event", "--help"][..],
    ] {
        assert!(!hook::is_help_request(&args(values)), "{values:?}");
    }
    for values in [
        &["doctor", "-h"][..],
        &["enablement", "--help"][..],
        &["paths", "--help"][..],
    ] {
        assert!(hook::is_lifecycle_help_request(&args(values)), "{values:?}");
    }
    assert!(!hook::is_lifecycle_help_request(&args(&[
        "install", "--help"
    ])));
    assert!(!hook::is_lifecycle_help_request(&args(&[
        "event", "--help"
    ])));
    assert!(!hook::is_help_request(&args(&[
        "install", "--client", "codex", "."
    ])));
}

#[test]
fn accept_host_is_not_a_public_hook_subcommand() {
    let error = hook::forwarded_hook_args(&args(&["accept-host"]))
        .expect_err("accept-host must not be a public Hook subcommand");
    assert!(!error.contains("asp hook accept-host"));
}

#[test]
fn top_level_install_help_is_non_mutating_unified_surface() {
    let root = temp_project_root("top-level-install-help");
    let output = Command::new(env!("CARGO_BIN_EXE_asp"))
        .current_dir(&root)
        .env("PATH", "")
        .env("PRJ_CACHE_HOME", root.join(".cache"))
        .args(["install", "--help"])
        .output()
        .expect("run asp install --help");

    assert!(output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("Usage: asp install [COMMAND]"),
        "stdout: {stdout}"
    );
    for command in ["binary", "hook", "plugin", "language"] {
        let prefix = format!("{command} ");
        assert!(
            stdout
                .lines()
                .any(|line| line.trim_start().starts_with(&prefix)),
            "missing {command}: {stdout}"
        );
    }
    assert!(
        !stdout.contains("--rev")
            && !stdout.contains("--archive")
            && !stdout.contains("--repo")
            && !stdout.contains("--from-workspace"),
        "stdout: {stdout}\nstderr: {stderr}"
    );
    assert!(!root.join(".codex/config.toml").exists());
    assert!(
        !root
            .join(".cache/agent-semantic-protocol/hooks/activation.json")
            .exists()
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn install_hook_help_is_non_mutating() {
    let root = temp_project_root("install-hook-help");
    let output = Command::new(env!("CARGO_BIN_EXE_asp"))
        .current_dir(&root)
        .env("PATH", "")
        .env("PRJ_CACHE_HOME", root.join(".cache"))
        .args(["install", "hook", "--help"])
        .output()
        .expect("run asp install hook --help");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("Usage: asp install hook --client <CLIENT> [PROJECT_ROOT]"),
        "stdout: {stdout}"
    );
    assert!(stdout.contains("--client <CLIENT>"), "stdout: {stdout}");
    assert!(
        stdout.contains("[possible values: claude]"),
        "stdout: {stdout}"
    );
    assert!(!root.join(".codex/config.toml").exists());
    assert!(
        !root
            .join(".cache/agent-semantic-protocol/hooks/activation.json")
            .exists()
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn install_binary_help_is_non_mutating() {
    let root = temp_project_root("install-binary-help");
    let output = Command::new(env!("CARGO_BIN_EXE_asp"))
        .current_dir(&root)
        .env("PATH", "")
        .env("PRJ_CACHE_HOME", root.join(".cache"))
        .args(["install", "binary", "--help"])
        .output()
        .expect("run asp install binary --help");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("Usage: asp install binary"),
        "stdout: {stdout}"
    );
    assert!(!stdout.contains("--target"), "stdout: {stdout}");
    assert!(!root.join(".codex/config.toml").exists());
    assert!(
        !root
            .join(".cache/agent-semantic-protocol/hooks/activation.json")
            .exists()
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn install_plugin_codex_help_is_non_mutating() {
    let root = temp_project_root("install-plugin-codex-help");
    let output = Command::new(env!("CARGO_BIN_EXE_asp"))
        .current_dir(&root)
        .env("PATH", "")
        .env("PRJ_CACHE_HOME", root.join(".cache"))
        .args(["install", "plugin", "status", "--codex", "--help"])
        .output()
        .expect("run asp install plugin status --codex --help");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("Usage: asp install plugin"),
        "stdout: {stdout}"
    );
    assert!(stdout.contains("[PROJECT_ROOT]"), "stdout: {stdout}");
    assert!(!root.join(".codex/config.toml").exists());
    assert!(
        !root
            .join(".cache/agent-semantic-protocol/hooks/activation.json")
            .exists()
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn host_events_are_not_client_hook_subcommands() {
    for event in [
        "pre-tool",
        "permission-request",
        "post-tool",
        "subagent-stop",
    ] {
        assert!(
            hook::forwarded_hook_args(&args(&[event, "--client", "codex"])).is_err(),
            "event={event} must be owned by the standalone asp-hook binary"
        );
    }
}

#[test]
fn platform_event_names_are_not_protocol_event_aliases() {
    assert!(hook::forwarded_hook_args(&args(&["PreToolUse", "--client", "codex"])).is_err());
}

#[test]
fn raw_host_event_flags_are_not_client_hook_commands() {
    assert!(hook::forwarded_hook_args(&args(&["--client", "codex", "--event", "stop"])).is_err());
}

#[test]
fn hook_binaries_follow_the_policy_and_lifecycle_package_boundary() {
    let output = Command::new("cargo")
        .args(["metadata", "--format-version", "1", "--no-deps"])
        .current_dir(workspace_root())
        .output()
        .expect("run cargo metadata");
    assert!(
        output.status.success(),
        "cargo metadata stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let metadata: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("metadata JSON");
    assert_eq!(
        package_bin_targets(&metadata, "agent-semantic-hook"),
        vec!["asp-hook".to_string()]
    );
    assert_eq!(
        package_bin_targets(&metadata, "agent-semantic-client"),
        vec!["asp".to_string()]
    );
}

fn workspace_root() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("crates dir")
        .parent()
        .expect("workspace root")
        .to_path_buf()
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

fn package_bin_targets(metadata: &serde_json::Value, package_name: &str) -> Vec<String> {
    let packages = metadata["packages"].as_array().expect("metadata packages");
    let package = packages
        .iter()
        .find(|package| package["name"] == package_name)
        .unwrap_or_else(|| panic!("missing package {package_name}"));
    package["targets"]
        .as_array()
        .expect("package targets")
        .iter()
        .filter(|target| {
            target["kind"]
                .as_array()
                .expect("target kind")
                .iter()
                .any(|kind| kind == "bin")
        })
        .map(|target| target["name"].as_str().expect("target name").to_string())
        .collect()
}
