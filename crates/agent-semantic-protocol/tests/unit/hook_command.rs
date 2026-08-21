#[path = "../../src/command/hook.rs"]
mod hook;
#[path = "../../src/command/hook_runtime_context.rs"]
mod hook_runtime_context;
use hook_runtime_context::payload_indicates_subagent_context;
use serde_json::json;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

mod hook_runtime {
    pub(crate) fn read_hook_input_bounded() -> Result<String, String> {
        Ok("{}".to_string())
    }

    pub(crate) async fn run_hook_runtime_args(_args: Vec<String>) -> Result<(), String> {
        Ok(())
    }

    pub(crate) async fn run_hook_from_bootstrap(
        _args: &[String],
        _input: String,
    ) -> Result<(), String> {
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
fn accept_host_doctor_and_paths_delegate_to_hook_runtime() {
    assert_eq!(
        hook::forwarded_hook_args(&args(&[
            "accept-host",
            "--host-rollout",
            "task.jsonl",
            "--host-probe-path",
            "probe.rs",
            "--host-sentinel",
            "TOKEN",
        ]))
        .unwrap(),
        args(&[
            "accept-host",
            "--host-rollout",
            "task.jsonl",
            "--host-probe-path",
            "probe.rs",
            "--host-sentinel",
            "TOKEN",
        ])
    );
    assert_eq!(
        hook::forwarded_hook_args(&args(&["doctor", "--client", "codex", "."])).unwrap(),
        args(&["doctor", "--client", "codex", "."])
    );
    assert_eq!(
        hook::forwarded_hook_args(&args(&["paths", "."])).unwrap(),
        args(&["paths", "."])
    );
}

#[test]
fn help_requests_do_not_forward_to_hook_runtime() {
    for values in [&["--help"][..], &["-h"][..], &["help"][..]] {
        assert!(hook::is_help_request(&args(values)), "{values:?}");
    }
    for values in [
        &["accept-host", "--help"][..],
        &["doctor", "-h"][..],
        &["paths", "--help"][..],
        &["event", "--help"][..],
    ] {
        assert!(!hook::is_help_request(&args(values)), "{values:?}");
    }
    for values in [
        &["accept-host", "--help"][..],
        &["doctor", "-h"][..],
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
fn accept_host_cli_returns_a_schema_valid_success_receipt() {
    let root = temp_project_root("accept-host-cli");
    let rollout_path = root.join("normal-task.jsonl");
    std::fs::write(
        &rollout_path,
        concat!(
            "{\"type\":\"world_state\",\"payload\":{\"state\":{\"plugins_instructions\":true}}}\n",
            "{\"type\":\"response_item\",\"payload\":{\"type\":\"function_call\",\"arguments\":\"probe.rs\"}}\n",
            "{\"type\":\"response_item\",\"payload\":{\"type\":\"message\",\"content\":\"<hook_prompt>[asp-hook] {\\\"schemaId\\\":\\\"agent.semantic-protocols.hook.decision\\\",\\\"decision\\\":\\\"deny\\\"}</hook_prompt>\"}}\n",
            "{\"type\":\"response_item\",\"payload\":{\"type\":\"function_call_output\",\"output\":\"\"}}\n"
        ),
    )
    .expect("write normal-task rollout");

    let output = Command::new(env!("CARGO_BIN_EXE_asp"))
        .current_dir(&root)
        .args([
            "hook",
            "accept-host",
            "--host-rollout",
            rollout_path.to_str().expect("utf8 rollout path"),
            "--host-probe-path",
            "probe.rs",
            "--host-sentinel",
            "ASP_HOST_SENTINEL",
        ])
        .output()
        .expect("run accept-host");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let receipt: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("parse Host acceptance receipt");
    assert_eq!(receipt["state"], "accepted");
    assert_eq!(receipt["reasonKind"], "normal-task-hook-deny-observed");
    let _ = std::fs::remove_dir_all(root);
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
        stdout.contains("Usage: asp install binary --target <PATH>"),
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
fn install_plugin_codex_help_is_non_mutating() {
    let root = temp_project_root("install-plugin-codex-help");
    let output = Command::new(env!("CARGO_BIN_EXE_asp"))
        .current_dir(&root)
        .env("PATH", "")
        .env("PRJ_CACHE_HOME", root.join(".cache"))
        .args(["install", "plugin", "--codex", "--help"])
        .output()
        .expect("run asp install plugin --codex --help");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        String::from_utf8_lossy(&output.stdout)
            .contains("Usage: asp install plugin [OPTIONS] --codex [PROJECT_ROOT]"),
        "stdout: {}",
        String::from_utf8_lossy(&output.stdout)
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
fn event_alias_delegates_to_hook_runtime() {
    assert_eq!(
        hook::forwarded_hook_args(&args(&["pre-tool", "--client", "codex"])).unwrap(),
        args(&["hook", "--event", "pre-tool", "--client", "codex"])
    );
    assert_eq!(
        hook::forwarded_hook_args(&args(&["permission-request", "--client", "codex"])).unwrap(),
        args(&["hook", "--event", "permission-request", "--client", "codex"])
    );
    assert_eq!(
        hook::forwarded_hook_args(&args(&["subagent-stop", "--client", "codex"])).unwrap(),
        args(&["hook", "--event", "subagent-stop", "--client", "codex"])
    );
}

#[test]
fn platform_event_names_are_not_protocol_event_aliases() {
    assert!(hook::forwarded_hook_args(&args(&["PreToolUse", "--client", "codex"])).is_err());
}

#[test]
fn raw_hook_flags_stay_supported() {
    assert_eq!(
        hook::forwarded_hook_args(&args(&["--client", "codex", "--event", "stop"])).unwrap(),
        args(&["hook", "--client", "codex", "--event", "stop"])
    );
}

#[test]
fn payload_subagent_detection_accepts_explicit_context_flags() {
    assert!(payload_indicates_subagent_context(
        &json!({"isSubagent": true})
    ));
    assert!(payload_indicates_subagent_context(
        &json!({"parentAgentId": "agent-123"})
    ));
    assert!(payload_indicates_subagent_context(
        &json!({"thread": {"threadKind": "child-agent"}})
    ));
    assert!(payload_indicates_subagent_context(&json!({
        "agent_id": "019f-child",
        "agent_type": "asp_testing"
    })));
}

#[test]
fn payload_subagent_detection_ignores_main_thread_payloads() {
    assert!(!payload_indicates_subagent_context(&json!({
        "session_id": "session-123",
        "tool_name": "Bash",
        "tool_input": {
            "command": "asp rust search pipe 'subagent hook' --workspace . --view seeds"
        }
    })));
    assert!(!payload_indicates_subagent_context(
        &json!({"isSubagent": false})
    ));
    assert!(!payload_indicates_subagent_context(&json!({
        "agent_id": "019f-child"
    })));
    assert!(!payload_indicates_subagent_context(&json!({
        "agent_type": "asp_testing"
    })));
    assert!(!payload_indicates_subagent_context(&json!({
        "tool_input": {
            "agent_id": "business-record-id",
            "agent_type": "business-record-type"
        }
    })));
}

#[test]
fn asp_is_the_only_hook_binary_target() {
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
        Vec::<String>::new()
    );
    assert_eq!(
        package_bin_targets(&metadata, "agent-semantic-protocol"),
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
