use std::{
    ffi::OsString,
    io::Write,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::atomic::{AtomicU64, Ordering},
};

pub(super) use serde_json::{Value, json};

use super::{activation_fixture, rollout_fixture};

static NEXT_TEMP_ID: AtomicU64 = AtomicU64::new(0);

pub(in super::super) fn claude_fixture() -> PathBuf {
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
    let org_fixture = root.join(".agent-semantic-protocols/org");
    std::fs::create_dir_all(&org_fixture).expect("create local org fixture");
    Command::new("git")
        .args(["init", "-q"])
        .current_dir(&org_fixture)
        .status()
        .expect("git init local org fixture");
    Command::new("git")
        .args([
            "remote",
            "add",
            "origin",
            "https://github.com/tao3k/org.git",
        ])
        .current_dir(&org_fixture)
        .status()
        .expect("configure local org fixture origin");
    std::fs::write(org_fixture.join(".fixture"), "local-only\n")
        .expect("mark local org fixture dirty");
    std::fs::create_dir_all(root.join("src")).expect("create src");
    std::fs::write(root.join("src/lib.rs"), "pub fn demo() {}\n").expect("write src");
    let bin_dir = root.join(".bin");
    std::fs::create_dir_all(&bin_dir).expect("create provider bin dir");
    let asp_path = bin_dir.join("asp");
    std::fs::write(
        &asp_path,
        format!("#!/bin/sh\nexec \"{}\" \"$@\"\n", env!("CARGO_BIN_EXE_asp")),
    )
    .expect("write asp shim");
    make_executable(&asp_path);
    let provider_path = bin_dir.join("rs-harness");
    std::fs::write(
        &provider_path,
        "#!/bin/sh\nif [ \"$1\" = \"agent\" ] && [ \"$2\" = \"guide\" ]; then\n  printf '[agent-guide] language=rust provider=rs-harness\\n'\nfi\nexit 0\n",
    )
    .expect("write fake provider");
    make_executable(&provider_path);
    crate::provider_command::support::write_activation(
        &root,
        &[crate::provider_command::support::provider(
            "rust",
            Vec::new(),
        )],
    );
    write_test_codex_plugin(&root);
    write_fake_codex_cli(&bin_dir);
    root
}

pub(in super::super) fn install_claude_hooks(root: &Path) {
    let output = Command::new(env!("CARGO_BIN_EXE_asp"))
        .args(["install", "hook", "--client", "claude"])
        .arg(root)
        .env("PATH", prepend_path(&root.join(".bin")))
        .env("ASP_STATE_HOME", root.join(".agent-semantic-protocols"))
        .env_remove("CODEX_THREAD_ID")
        .env_remove("CODEX_PARENT_THREAD_ID")
        .env_remove("CLAUDE_CODE_SESSION_ID")
        .env_remove("AGENT_SESSION_ID")
        .env_remove("SESSION_ID")
        .env_remove("PRJ_CACHE_HOME")
        .output()
        .expect("run asp install hook");
    assert!(
        output.status.success(),
        "install stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

pub(in super::super) fn install_codex_hooks(root: &Path, codex_home: &Path) -> String {
    let hook_config = root.join(".agent-semantic-protocols/hooks/config.toml");
    std::fs::create_dir_all(hook_config.parent().expect("hook config parent"))
        .expect("create hook config dir");
    std::fs::write(
        &hook_config,
        agent_semantic_config::default_hook_client_config_template(),
    )
    .expect("write default hook config");
    let output = Command::new(env!("CARGO_BIN_EXE_asp"))
        .args(["install", "plugin", "--codex", "--project"])
        .arg(root)
        .env("PATH", prepend_path(&root.join(".bin")))
        .env("CODEX_HOME", codex_home)
        .env("ASP_STATE_HOME", root.join(".agent-semantic-protocols"))
        .env_remove("PRJ_CACHE_HOME")
        .output()
        .expect("run asp install plugin");
    assert!(
        output.status.success(),
        "install stdout: {}\ninstall stderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).expect("install stdout is utf8")
}

pub(in super::super) fn run_claude_pre_tool_decision(
    root: &Path,
    payload: Value,
    extra_args: &[&str],
) -> Value {
    let mut command = Command::new(env!("CARGO_BIN_EXE_asp"));
    command
        .args(["hook", "pre-tool", "--client", "claude"])
        .args(extra_args)
        .arg("--activation")
        .arg(codex_smoke_activation_path(root))
        .current_dir(root)
        .env("ASP_STATE_HOME", root.join(".agent-semantic-protocols"))
        .env_remove("PRJ_CACHE_HOME");
    clear_host_agent_identity(&mut command)
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

pub(in super::super) fn run_codex_pre_tool_decision(root: &Path, payload: Value) -> Value {
    run_codex_pre_tool_decision_with_env(root, payload, &[])
}

pub(in super::super) fn run_codex_pre_tool_decision_with_env(
    root: &Path,
    payload: Value,
    envs: &[(&str, &str)],
) -> Value {
    run_codex_hook_decision_with_env(root, "pre-tool", payload, envs)
}

pub(in super::super) fn run_codex_hook_decision_with_env(
    root: &Path,
    event: &str,
    payload: Value,
    envs: &[(&str, &str)],
) -> Value {
    let activation_path = codex_smoke_activation_path(root);
    run_codex_hook_decision_with_activation(root, event, payload, envs, &activation_path)
}

pub(in super::super) fn run_codex_hook_decision_with_activation(
    root: &Path,
    event: &str,
    payload: Value,
    envs: &[(&str, &str)],
    activation_path: &Path,
) -> Value {
    let output = run_codex_hook_output_with_activation(root, event, payload, envs, activation_path);
    assert!(
        output.status.success(),
        "hook stdout: {}\nhook stderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).expect("parse hook stdout")
}

pub(in super::super) fn run_codex_pre_tool_output_with_env(
    root: &Path,
    payload: Value,
    envs: &[(&str, &str)],
) -> std::process::Output {
    let activation_path = codex_smoke_activation_path(root);
    run_codex_hook_output_with_activation(root, "pre-tool", payload, envs, &activation_path)
}

fn run_codex_hook_output_with_activation(
    root: &Path,
    event: &str,
    payload: Value,
    envs: &[(&str, &str)],
    activation_path: &Path,
) -> std::process::Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_asp"));
    command
        .args(["hook", event, "--client", "codex", "--emit", "decision"])
        .arg("--activation")
        .arg(activation_path)
        .current_dir(root)
        .env("CODEX_HOME", root.join(".codex-home"))
        .env("ASP_STATE_HOME", root.join(".agent-semantic-protocols"))
        .env("ASP_CODEX_HOST_EVIDENCE_MODE", "rollout-only")
        .env_remove("PRJ_CACHE_HOME");
    let host_evidence_fixture = root.join(".agent-semantic-protocols/host-evidence.json");
    if host_evidence_fixture.exists() {
        command.env("ASP_CODEX_HOST_EVIDENCE_FIXTURE", host_evidence_fixture);
    }
    clear_host_agent_identity(&mut command);
    for (key, value) in envs {
        command.env(key, value);
    }
    command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = command.spawn().expect("spawn asp hook");
    child
        .stdin
        .as_mut()
        .expect("stdin")
        .write_all(payload.to_string().as_bytes())
        .expect("write payload");
    child.wait_with_output().expect("wait hook")
}

pub(in super::super) fn codex_asp_query_payload(command: &str) -> Value {
    json!({ "tool_name": "Bash", "tool_input": { "command": command } })
}

pub(in super::super) fn register_asp_explore_session(
    root: &Path,
    root_session_id: &str,
    child_session_id: &str,
) {
    rollout_fixture::write_codex_asp_explore_rollout(
        root,
        root_session_id,
        child_session_id,
        "gpt-5.4-mini",
    );
    write_host_evidence_fixture(root, root_session_id, child_session_id, Some("low"));
    let decision = run_codex_hook_decision_with_env(
        root,
        "subagent-start",
        serde_json::json!({
            "hook_event_name": "SubagentStart",
            "session_id": root_session_id,
            "agent_id": child_session_id,
            "agent_type": "asp_explorer",
            "model": "gpt-5.4-mini",
            "permission_mode": "default",
        }),
        &[("CODEX_THREAD_ID", root_session_id)],
    );
    assert_eq!(
        decision["decision"].as_str(),
        Some("allow"),
        "SubagentStart decision: {}",
        serde_json::to_string_pretty(&decision).expect("serialize SubagentStart decision")
    );
}

pub(in super::super) fn register_expired_asp_explore_session(
    root: &Path,
    root_session_id: &str,
    child_session_id: &str,
) {
    register_asp_explore_session_with_extra_args(
        root,
        root_session_id,
        child_session_id,
        &["--expires-at", "1"],
    );
}

fn register_asp_explore_session_with_extra_args(
    root: &Path,
    root_session_id: &str,
    child_session_id: &str,
    extra_args: &[&str],
) {
    rollout_fixture::write_codex_asp_explore_rollout(
        root,
        root_session_id,
        child_session_id,
        "gpt-5.4-mini",
    );
    write_host_evidence_fixture(root, root_session_id, child_session_id, Some("low"));
    let mut args = vec![
        "agent",
        "session",
        "register",
        "--name",
        "asp-explore",
        "--child-session-id",
        child_session_id,
        "--roles",
        "subagent,search",
    ];
    args.extend_from_slice(extra_args);
    let output = Command::new(env!("CARGO_BIN_EXE_asp"))
        .args(args)
        .current_dir(root)
        .env("PATH", prepend_path(&root.join(".bin")))
        .env("CODEX_HOME", root.join(".codex-home"))
        .env("CODEX_THREAD_ID", root_session_id)
        .env("ASP_STATE_HOME", root.join(".agent-semantic-protocols"))
        .env("ASP_CODEX_HOST_EVIDENCE_MODE", "rollout-only")
        .env(
            "ASP_CODEX_HOST_EVIDENCE_FIXTURE",
            root.join(".agent-semantic-protocols/host-evidence.json"),
        )
        .env_remove("PRJ_CACHE_HOME")
        .output()
        .expect("register asp-explore session");
    assert!(
        output.status.success(),
        "register asp-explore session failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn write_host_evidence_fixture(
    root: &Path,
    root_session_id: &str,
    child_session_id: &str,
    reasoning_effort: Option<&str>,
) {
    let mut runtime = serde_json::json!({ "model": "gpt-5.4-mini" });
    if let Some(reasoning_effort) = reasoning_effort {
        runtime["reasoningEffort"] = serde_json::json!(reasoning_effort);
    }
    let fixture = serde_json::json!({
        "threads": [{
            "id": child_session_id,
            "parentThreadId": root_session_id,
            "agentRole": "asp_explorer",
            "source": {
                "subAgent": {
                    "thread_spawn": {
                        "agent_path": "/root/asp_explorer",
                        "depth": 1
                    }
                }
            }
        }],
        "runtime": {
            (child_session_id): runtime
        }
    });
    let path = root.join(".agent-semantic-protocols/host-evidence.json");
    std::fs::create_dir_all(path.parent().expect("host evidence parent"))
        .expect("create host evidence fixture parent");
    std::fs::write(
        path,
        serde_json::to_vec_pretty(&fixture).expect("serialize host evidence fixture"),
    )
    .expect("write host evidence fixture");
}

pub(in super::super) fn show_agent_session_json(root: &Path, child_session_id: &str) -> Value {
    let output = Command::new(env!("CARGO_BIN_EXE_asp"))
        .args([
            "agent",
            "session",
            "show",
            "--child-session-id",
            child_session_id,
            "--json",
        ])
        .current_dir(root)
        .env("PATH", prepend_path(&root.join(".bin")))
        .env("ASP_STATE_HOME", root.join(".agent-semantic-protocols"))
        .env("ASP_NO_AGENT_PLATFORM", "1")
        .env_remove("PRJ_CACHE_HOME")
        .output()
        .expect("show agent session");
    assert!(
        output.status.success(),
        "show agent session failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).expect("parse agent session show json")
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
    let _ = path;
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
    let hook_config = plugin_root.join("templates/hooks/config.toml");
    std::fs::create_dir_all(hook_config.parent().expect("plugin hook config parent"))
        .expect("create plugin hook config dir");
    std::fs::write(
        &hook_config,
        agent_semantic_config::default_hook_client_config_template(),
    )
    .expect("write plugin hook config template");
}

fn write_fake_codex_cli(bin_dir: &Path) {
    let path = bin_dir.join("codex");
    std::fs::write(&path, r#"#!/bin/sh
set -eu
codex_home="${CODEX_HOME:-${HOME:-}/.codex}"
config="$codex_home/config.toml"
config_dir="${config%/*}"
/bin/mkdir -p "$config_dir"
if [ "${1:-}" = "plugin" ] && [ "${2:-}" = "marketplace" ] && [ "${3:-}" = "add" ]; then
  root="${4:-.}"
  if ! /usr/bin/grep -q '^\[marketplaces\.asp-project\]' "$config" 2>/dev/null; then
    { printf '[marketplaces.asp-project]\n'; printf 'last_updated = "2026-01-01T00:00:00Z"\n'; printf 'source_type = "local"\n'; printf 'source = "%s"\n\n' "$root"; } >> "$config"
  fi
  printf '{"marketplaceName":"asp-project","installedRoot":"%s","alreadyAdded":false}\n' "$root"
  exit 0
fi
if [ "${1:-}" = "plugin" ] && [ "${2:-}" = "add" ]; then
  /bin/mkdir -p "$codex_home/plugins/cache/asp-project/asp-codex-plugin/0.1.0+test"
  if ! /usr/bin/grep -q '^\[plugins\."asp-codex-plugin@asp-project"\]' "$config" 2>/dev/null; then
    { printf '[plugins."asp-codex-plugin@asp-project"]\n'; printf 'enabled = true\n'; } >> "$config"
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
"#).expect("write fake Codex CLI");
    make_executable(&path);
}

pub(in super::super) fn prepend_path(path_prefix: &Path) -> OsString {
    let mut paths = vec![path_prefix.to_path_buf()];
    if let Some(path) = std::env::var_os("PATH") {
        paths.extend(std::env::split_paths(&path));
    }
    std::env::join_paths(paths).expect("join PATH")
}

fn clear_host_agent_identity(command: &mut Command) -> &mut Command {
    for key in [
        "CODEX_THREAD_ID",
        "CODEX_PARENT_THREAD_ID",
        "CODEX_SESSION_ID",
        "CODEX_AGENT_SESSION_ID",
        "CLAUDE_CODE_SESSION_ID",
        "CLAUDE_SESSION_ID",
        "AGENT_SESSION_ID",
        "SESSION_ID",
        "ASP_ROOT_SESSION_ID",
    ] {
        command.env_remove(key);
    }
    command
}

fn codex_smoke_activation_path(root: &Path) -> PathBuf {
    activation_fixture::for_workspace(root)
}

pub(in super::super) fn force_activation_project_root_to_hook_state(root: &Path) {
    let activation_path = codex_smoke_activation_path(root);
    let hook_state = activation_path
        .parent()
        .expect("activation hook-state parent");
    let mut activation: Value =
        serde_json::from_slice(&std::fs::read(&activation_path).expect("read activation"))
            .expect("parse activation");
    activation["projectRoot"] = Value::String(hook_state.display().to_string());
    std::fs::write(
        &activation_path,
        serde_json::to_vec_pretty(&activation).expect("encode activation"),
    )
    .expect("write activation");
}
