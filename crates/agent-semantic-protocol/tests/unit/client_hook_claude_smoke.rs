use std::io::Write;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};

use serde_json::{Value, json};
use std::ffi::OsString;
use std::path::{Path, PathBuf};

static NEXT_TEMP_ID: AtomicU64 = AtomicU64::new(0);

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
    assert!(
        settings["hooks"].get("PermissionRequest").is_none(),
        "Claude SDK-backed sandtables use can_use_tool for permission; managed Claude settings must not install PermissionRequest hooks"
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
    let prompt_path = root
        .join(".claude")
        .join("agent-semantic-protocol")
        .join("hooks")
        .join("hook_trigger_prompt.md");
    let prompt = std::fs::read_to_string(&prompt_path).expect("read hook trigger prompt");
    assert!(prompt.contains("ASP-HOOK-TRIGGER-PROMPT:MANAGED-BEGIN"));
    assert!(prompt.contains("ASP-HOOK-TRIGGER-PROMPT:USER-EXTENSIONS-BEGIN"));
    assert!(prompt.contains("spawn_agent"));
    assert!(prompt.contains("asp-search-subagent"));
}

#[test]
fn codex_install_writes_project_plugin_and_runtime_decision_config() {
    let root = claude_fixture();
    let codex_home = root.join(".codex-home");
    std::fs::create_dir_all(&codex_home).expect("create codex home");

    let first_install_stdout = install_codex_hooks(root.as_path(), &codex_home);
    assert!(
        first_install_stdout.contains("activationSync=created")
            || first_install_stdout.contains("activationSync=refreshed"),
        "{first_install_stdout}"
    );

    let codex_config =
        std::fs::read_to_string(root.join(".codex").join("config.toml")).expect("read config");
    assert!(codex_config.contains("[marketplaces.asp-project]"));
    assert!(codex_config.contains("[plugins.\"asp-codex-plugin@asp-project\"]"));
    assert!(codex_config.contains("[agents.asp_explorer]"));
    assert!(root.join(".codex/agents/asp-explorer.toml").is_file());
    let codex_agent =
        std::fs::read_to_string(root.join(".codex/agents/asp-explorer.toml")).expect("read agent");
    assert!(codex_agent.contains("fork_context=false"));
    assert!(!codex_agent.contains("fork_turns"));
    assert!(
        root.join("asp-codex-plugin/skills/agent-semantic-protocols/SKILL.org")
            .is_file()
    );
    assert!(
        root.join("asp-codex-plugin/skills/agent-semantic-protocols/SKILL.contract.org")
            .is_file()
    );
    assert!(
        !root
            .join(".agents/skills/agent-semantic-protocols/SKILL.org")
            .exists()
    );

    let second_install_stdout = install_codex_hooks(root.as_path(), &codex_home);
    assert!(
        second_install_stdout.contains("activationSync=reused"),
        "{second_install_stdout}"
    );

    let decision = run_codex_pre_tool_decision(
        root.as_path(),
        json!({
            "session_id": "session-codex-read",
            "transcript_path": root.as_path().join("session.jsonl"),
            "cwd": root.as_path(),
            "tool_name": "Read",
            "tool_input": {
                "file_path": root.as_path().join("src/lib.rs")
            }
        }),
    );
    let message = decision["message"].as_str().expect("decision message");
    assert!(message.contains("spawn_agent"), "{message}");
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
    assert!(reason.contains("ASP hook blocked"), "{reason}");
    assert!(reason.contains("direct-source-read"), "{reason}");
    assert!(reason.contains("asp rust query --selector"));
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
    let unique = NEXT_TEMP_ID.fetch_add(1, Ordering::Relaxed);
    let root = std::env::temp_dir().join(format!(
        "asp-claude-smoke-{}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system time")
            .as_nanos(),
        unique,
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
    write_test_codex_plugin(&root);
    write_fake_codex_cli(&bin_dir);
    root
}

fn write_test_codex_plugin(root: &Path) {
    let plugin_root = root.join("asp-codex-plugin");
    let manifest = plugin_root.join(".codex-plugin/plugin.json");
    std::fs::create_dir_all(manifest.parent().expect("plugin manifest parent"))
        .expect("create plugin manifest dir");
    std::fs::write(
        &manifest,
        r#"{
  "name": "asp-codex-plugin",
  "version": "0.1.0+test",
  "description": "Test ASP Codex plugin",
  "author": {"name": "ASP"},
  "skills": "./skills/",
  "hooks": "./hooks/hooks.json",
  "interface": {"displayName": "ASP Test"}
}
"#,
    )
    .expect("write plugin manifest");
    let hooks = plugin_root.join("hooks/hooks.json");
    std::fs::create_dir_all(hooks.parent().expect("plugin hooks parent"))
        .expect("create plugin hooks dir");
    std::fs::write(&hooks, r#"{"hooks":{}}"#).expect("write plugin hooks");
}

fn write_fake_codex_cli(bin_dir: &Path) {
    let path = bin_dir.join("codex");
    std::fs::write(
        &path,
        r#"#!/bin/sh
set -eu
codex_home="${CODEX_HOME:-${HOME:-}/.codex}"
config="$codex_home/config.toml"
config_dir="${config%/*}"
/bin/mkdir -p "$config_dir"
if [ "${1:-}" = "plugin" ] && [ "${2:-}" = "marketplace" ] && [ "${3:-}" = "add" ]; then
  root="${4:-.}"
  if ! /usr/bin/grep -q '^\[marketplaces\.asp-project\]' "$config" 2>/dev/null; then
    {
      printf '[marketplaces.asp-project]\n'
      printf 'last_updated = "2026-01-01T00:00:00Z"\n'
      printf 'source_type = "local"\n'
      printf 'source = "%s"\n\n' "$root"
    } >> "$config"
  fi
  printf '{"marketplaceName":"asp-project","installedRoot":"%s","alreadyAdded":false}\n' "$root"
  exit 0
fi
if [ "${1:-}" = "plugin" ] && [ "${2:-}" = "add" ]; then
  /bin/mkdir -p "$codex_home/plugins/cache/asp-project/asp-codex-plugin/0.1.0+test"
  if ! /usr/bin/grep -q '^\[plugins\."asp-codex-plugin@asp-project"\]' "$config" 2>/dev/null; then
    {
      printf '[plugins."asp-codex-plugin@asp-project"]\n'
      printf 'enabled = true\n'
    } >> "$config"
  fi
  printf '{"pluginId":"asp-codex-plugin@asp-project","name":"asp-codex-plugin","marketplaceName":"asp-project","version":"0.1.0+test","installedPath":"%s/plugins/cache/asp-project/asp-codex-plugin/0.1.0+test","authPolicy":"ON_INSTALL"}\n' "$codex_home"
  exit 0
fi
if [ "${1:-}" = "plugin" ] && [ "${2:-}" = "list" ]; then
  if /usr/bin/grep -q '^\[plugins\."asp-codex-plugin@asp-project"\]' "$config" 2>/dev/null; then
    printf '{"installed":[{"pluginId":"asp-codex-plugin@asp-project","name":"asp-codex-plugin","marketplaceName":"asp-project","version":"0.1.0+test","installed":true,"enabled":true}],"available":[]}\n'
  else
    printf '{"installed":[],"available":[]}\n'
  fi
  exit 0
fi
printf 'unsupported fake codex command: %s\n' "$*" >&2
exit 2
"#,
    )
    .expect("write fake Codex CLI");
    make_executable(&path);
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

fn install_codex_hooks(root: &Path, codex_home: &Path) -> String {
    let output = Command::new(env!("CARGO_BIN_EXE_asp"))
        .args(["hook", "install", "--client", "codex"])
        .arg(root)
        .env("PATH", prepend_path(&root.join(".bin")))
        .env("CODEX_HOME", codex_home)
        .env_remove("PRJ_CACHE_HOME")
        .output()
        .expect("run asp hook install");
    assert!(
        output.status.success(),
        "install stdout: {}\ninstall stderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).expect("install stdout is utf8")
}

fn run_codex_pre_tool_decision(root: &Path, payload: Value) -> Value {
    let mut command = Command::new(env!("CARGO_BIN_EXE_asp"));
    command
        .args([
            "hook", "pre-tool", "--client", "codex", "--emit", "decision",
        ])
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
        "hook stdout: {}\nhook stderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).expect("parse hook stdout")
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
