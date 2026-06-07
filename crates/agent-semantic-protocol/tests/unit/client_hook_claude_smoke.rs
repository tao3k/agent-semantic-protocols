use std::io::Write;
use std::process::{Command, Stdio};

use serde_json::{Value, json};
use std::ffi::OsString;
use std::path::{Path, PathBuf};

#[test]
fn claude_install_writes_project_settings_hooks() {
    let root = claude_fixture();

    install_claude_hooks(root.as_path());

    let settings_path = root.as_path().join(".claude/settings.json");
    let settings: Value =
        serde_json::from_slice(&std::fs::read(&settings_path).expect("read claude settings"))
            .expect("parse claude settings");

    let pre_tool_matcher = settings["hooks"]["PreToolUse"][0]["matcher"]
        .as_str()
        .expect("pre-tool matcher");
    assert_ne!(
        pre_tool_matcher, "*",
        "Claude should reuse the shared tool-surface matcher instead of spawning hooks for every tool"
    );
    assert!(pre_tool_matcher.contains("Bash|Shell"));
    assert!(pre_tool_matcher.contains("functions\\.exec_command"));
    assert_eq!(
        settings["hooks"]["PermissionRequest"][0]["matcher"],
        pre_tool_matcher
    );
    assert_eq!(
        settings["hooks"]["PostToolUse"][0]["matcher"],
        pre_tool_matcher
    );
    assert!(
        settings["hooks"]["PreToolUse"][0]["hooks"][0]["command"]
            .as_str()
            .expect("pre-tool command")
            .contains("asp hook pre-tool --client claude")
    );
    assert!(
        settings["hooks"]["PermissionRequest"][0]["hooks"][0]["command"]
            .as_str()
            .expect("permission command")
            .contains("asp hook permission-request --client claude")
    );
}

#[test]
fn claude_pre_tool_denies_source_directory_enumeration() {
    let root = claude_fixture();

    install_claude_hooks(root.as_path());

    let decision = run_claude_pre_tool_decision(
        root.as_path(),
        json!({
            "session_id": "session-claude-list-files",
            "transcript_path": root.as_path().join("session.jsonl"),
            "cwd": root.as_path(),
            "hook_event_name": "PreToolUse",
            "tool_use_id": "toolu_list_files",
            "tool_name": "Bash",
            "tool_input": {
                "command": "ls src",
                "commandActions": [
                    {"type": "listFiles", "command": "ls src", "path": "src"}
                ]
            }
        }),
        &["--emit", "decision"],
    );

    assert_eq!(decision["platform"], "claude");
    assert_eq!(decision["decision"], "deny");
    assert_eq!(decision["reasonKind"], "source-directory-enumeration");
    assert_eq!(decision["routes"][0]["kind"], "ingest");
    assert_eq!(decision["subject"]["command"], "ls src");
}

#[test]
fn claude_platform_response_uses_hook_specific_permission_decision() {
    let root = claude_fixture();

    install_claude_hooks(root.as_path());

    let response = run_claude_pre_tool_decision(
        root.as_path(),
        json!({
            "session_id": "session-claude-read",
            "transcript_path": root.as_path().join("session.jsonl"),
            "cwd": root.as_path(),
            "hook_event_name": "PreToolUse",
            "tool_use_id": "toolu_read",
            "tool_name": "Read",
            "tool_input": {
                "file_path": root.as_path().join("src/lib.rs")
            }
        }),
        &[],
    );

    assert_eq!(
        response["hookSpecificOutput"]["hookEventName"],
        "PreToolUse"
    );
    assert_eq!(response["hookSpecificOutput"]["permissionDecision"], "deny");
    let reason = response["hookSpecificOutput"]["permissionDecisionReason"]
        .as_str()
        .expect("permission reason");
    assert!(reason.contains("# ASP Hook Recovery"), "{reason}");
    assert!(reason.contains("direct-source-read"), "{reason}");
    assert!(reason.contains("asp rust query --from-hook direct-source-read"));
    assert!(reason.contains("--code"));
    assert!(response.get("agentHookDecision").is_none());
}

#[test]
fn claude_platform_response_compacts_repeated_denied_source_lane() {
    let root = claude_fixture();

    install_claude_hooks(root.as_path());

    let payload = |tool_use_id: &str| {
        json!({
            "session_id": "session-claude-repeated-read",
            "transcript_path": root.as_path().join("session.jsonl"),
            "cwd": root.as_path(),
            "hook_event_name": "PreToolUse",
            "tool_use_id": tool_use_id,
            "tool_name": "Read",
            "tool_input": {
                "file_path": root.as_path().join("src/lib.rs")
            }
        })
    };
    let first = run_claude_pre_tool_decision(root.as_path(), payload("toolu_read_1"), &[]);
    let second = run_claude_pre_tool_decision(root.as_path(), payload("toolu_read_2"), &[]);

    assert_eq!(first["hookSpecificOutput"]["permissionDecision"], "deny");
    let reason = second["hookSpecificOutput"]["permissionDecisionReason"]
        .as_str()
        .expect("permission reason");
    assert!(reason.starts_with("ASP hook already denied `direct-source-read`"));
    assert!(reason.contains("Follow the previous recovery route"));
    assert!(!reason.contains("## Agent Flow"));
    let context = second["hookSpecificOutput"]["additionalContext"]
        .as_str()
        .expect("decision context");
    assert!(context.contains("\"denyReplay\":\"repeated\""));
}

fn claude_fixture() -> PathBuf {
    let root = std::env::temp_dir().join(format!(
        "asp-claude-smoke-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system time")
            .as_nanos()
    ));
    std::fs::create_dir_all(&root).expect("create temp root");
    Command::new("git")
        .args(["init", "-q"])
        .current_dir(&root)
        .status()
        .expect("git init");
    std::fs::create_dir_all(root.join("src")).expect("create src");
    std::fs::write(root.join("src/lib.rs"), "pub fn demo() {}\n").expect("write src");
    let bin_dir = root.join(".bin");
    std::fs::create_dir_all(&bin_dir).expect("create provider bin dir");
    let provider_path = bin_dir.join("rs-harness");
    std::fs::write(
        &provider_path,
        "#!/bin/sh\nif [ \"$1\" = \"agent\" ] && [ \"$2\" = \"guide\" ]; then\n  printf '[agent-guide] language=rust provider=rs-harness\\n'\nfi\nexit 0\n",
    )
    .expect("write fake provider");
    make_executable(&provider_path);
    root
}

fn install_claude_hooks(root: &Path) {
    let output = Command::new(env!("CARGO_BIN_EXE_asp"))
        .args(["hook", "install", "--client", "claude"])
        .arg(root)
        .env("PATH", prepend_path(&root.join(".bin")))
        .env_remove("PRJ_CACHE_HOME")
        .output()
        .expect("run asp hook install");
    assert!(
        output.status.success(),
        "install stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn run_claude_pre_tool_decision(root: &Path, payload: Value, extra_args: &[&str]) -> Value {
    let mut command = Command::new(env!("CARGO_BIN_EXE_asp"));
    command
        .args(["hook", "pre-tool", "--client", "claude"])
        .args(extra_args)
        .arg("--activation")
        .arg(root.join(".cache/agent-semantic-protocol/hooks/activation.json"))
        .arg("--config")
        .arg(root.join(".codex/agent-semantic-protocol/hooks/config.toml"))
        .current_dir(root)
        .env_remove("PRJ_CACHE_HOME")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let mut child = command.spawn().expect("spawn asp hook pre-tool");
    child
        .stdin
        .as_mut()
        .expect("stdin")
        .write_all(payload.to_string().as_bytes())
        .expect("write payload");
    let output = child.wait_with_output().expect("wait hook");
    assert!(
        output.status.success(),
        "hook stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).expect("parse hook stdout")
}

fn prepend_path(path_prefix: &Path) -> OsString {
    let mut paths = vec![path_prefix.to_path_buf()];
    if let Some(path) = std::env::var_os("PATH") {
        paths.extend(std::env::split_paths(&path));
    }
    std::env::join_paths(paths).expect("join PATH")
}

fn make_executable(path: &Path) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut permissions = std::fs::metadata(path)
            .expect("provider metadata")
            .permissions();
        permissions.set_mode(0o755);
        std::fs::set_permissions(path, permissions).expect("provider permissions");
    }
    #[cfg(not(unix))]
    {
        let _ = path;
    }
}
