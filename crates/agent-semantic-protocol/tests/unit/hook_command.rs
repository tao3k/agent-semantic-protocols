#![allow(dead_code)]

#[path = "../../src/command/hook.rs"]
mod hook;
#[path = "../../src/command/hook_runtime_context.rs"]
mod hook_runtime_context;
#[path = "../../src/command/protocol_binary.rs"]
mod protocol_binary;

use hook_runtime_context::payload_indicates_subagent_context;
use protocol_binary::install_protocol_binary;
use serde_json::json;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

mod hook_runtime {
    pub(crate) fn run_hook_runtime_args(_args: Vec<String>) -> Result<(), String> {
        Ok(())
    }
}

fn args(values: &[&str]) -> Vec<String> {
    values.iter().map(|value| value.to_string()).collect()
}

#[test]
fn install_and_doctor_delegate_to_hook_runtime() {
    assert_eq!(
        hook::forwarded_hook_args(&args(&["install", "--client", "codex", "."])).unwrap(),
        args(&["install", "--client", "codex", "."])
    );
    assert_eq!(
        hook::forwarded_hook_args(&args(&["doctor", "--client", "codex", "."])).unwrap(),
        args(&["doctor", "--client", "codex", "."])
    );
}

#[test]
fn help_requests_do_not_forward_to_hook_runtime() {
    for values in [&["--help"][..], &["-h"][..], &["help"][..]] {
        assert!(hook::is_help_request(&args(values)), "{values:?}");
    }
    for values in [
        &["install", "--help"][..],
        &["doctor", "-h"][..],
        &["event", "--help"][..],
    ] {
        assert!(!hook::is_help_request(&args(values)), "{values:?}");
    }
    for values in [&["install", "--help"][..], &["doctor", "-h"][..]] {
        assert!(hook::is_lifecycle_help_request(&args(values)), "{values:?}");
    }
    assert!(!hook::is_lifecycle_help_request(&args(&[
        "event", "--help"
    ])));
    assert!(!hook::is_help_request(&args(&[
        "install", "--client", "codex", "."
    ])));
}

#[test]
fn top_level_install_help_is_non_mutating_provider_surface() {
    let root = temp_project_root("top-level-install-help");
    let output = Command::new(env!("CARGO_BIN_EXE_asp"))
        .current_dir(&root)
        .env("PATH", "")
        .env("PRJ_CACHE_HOME", root.join(".cache"))
        .args(["install", "--help"])
        .output()
        .expect("run asp install --help");

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("usage: asp install provider <language> --rev <rev>"),
        "stderr: {stderr}"
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
fn hook_install_help_is_non_mutating() {
    let root = temp_project_root("hook-install-help");
    let output = Command::new(env!("CARGO_BIN_EXE_asp"))
        .current_dir(&root)
        .env("PATH", "")
        .env("PRJ_CACHE_HOME", root.join(".cache"))
        .args(["hook", "install", "--help"])
        .output()
        .expect("run asp hook install --help");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        String::from_utf8_lossy(&output.stdout).contains("usage: asp hook install --client claude"),
        "stdout: {}",
        String::from_utf8_lossy(&output.stdout)
    );
    assert!(
        String::from_utf8_lossy(&output.stdout).contains("asp plugin install codex"),
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

#[test]
fn protocol_binary_install_replaces_existing_target_file() {
    let root = temp_project_root("protocol-binary-replace");
    let source = root.join("source-asp");
    let target = root.join("asp");
    std::fs::write(&source, "new asp").expect("write source");
    std::fs::write(&target, "old asp").expect("write target");
    #[cfg(unix)]
    let old_inode = target_inode(&target);

    let status = install_protocol_binary(&source, &target).expect("install binary");

    assert_eq!(status, "updated");
    assert_eq!(
        std::fs::read_to_string(&target).expect("read target"),
        "new asp"
    );
    #[cfg(unix)]
    assert_ne!(old_inode, target_inode(&target));
    let _ = std::fs::remove_dir_all(root);
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

#[cfg(unix)]
fn target_inode(path: &std::path::Path) -> u64 {
    use std::os::unix::fs::MetadataExt;

    std::fs::metadata(path).expect("target metadata").ino()
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
