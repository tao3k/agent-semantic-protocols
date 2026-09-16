// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use super::ASP_CODEX_PLUGIN_HOOK_LAUNCHER;
use super::ASP_CODEX_PLUGIN_HOOKS_JSON;
use super::ASP_CODEX_PLUGIN_MANIFEST_JSON;
use super::ASP_CODEX_PLUGIN_MARKETPLACE_JSON;
use super::remove_codex_managed_global_hook_config;
use std::path::PathBuf;

fn temp_root(label: &str) -> PathBuf {
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("clock must be after epoch")
        .as_nanos();
    std::env::temp_dir().join(format!(
        "asp-plugin-hook-{label}-{}-{nonce}",
        std::process::id()
    ))
}

#[test]
fn bundled_manifest_uses_standard_hook_directory_without_unsupported_fields() {
    let manifest: serde_json::Value =
        serde_json::from_str(ASP_CODEX_PLUGIN_MANIFEST_JSON).expect("valid plugin manifest");
    assert!(manifest.get("hooks").is_none());
    assert!(manifest.get("skills").is_none());

    let hooks: serde_json::Value =
        serde_json::from_str(ASP_CODEX_PLUGIN_HOOKS_JSON).expect("valid plugin hooks");
    let events = hooks
        .get("hooks")
        .and_then(serde_json::Value::as_object)
        .expect("plugin hook event map");
    assert_eq!(events.len(), 7);
    let pre_tool_matchers = events["PreToolUse"]
        .as_array()
        .expect("PreToolUse matcher groups")
        .iter()
        .map(|group| {
            group
                .get("matcher")
                .and_then(serde_json::Value::as_str)
                .expect("native Action matcher")
        })
        .collect::<Vec<_>>();
    assert_eq!(
        &pre_tool_matchers[..3],
        ["^apply_patch$", "Bash", "spawn_agent"]
    );
    assert!(
        pre_tool_matchers[3..]
            .iter()
            .all(|matcher| matcher.starts_with("mcp__codex_app__"))
    );
    assert!(!pre_tool_matchers.contains(&"^mcp__.*$"));

    {
        let event = "PostToolUse";
        let groups = events[event]
            .as_array()
            .unwrap_or_else(|| panic!("plugin Hook event {event} must contain matcher groups"));
        assert!(
            groups.iter().all(|group| matches!(
                group.get("matcher").and_then(serde_json::Value::as_str),
                None | Some("") | Some("*")
            )),
            "observational plugin Hook event {event} must retain its match-all delivery contract: encodedMatcher={} actual={groups:?}",
            serde_json::to_string(&groups[0]["matcher"]).expect("encode matcher diagnostic")
        );
    }
    for (event, groups) in events {
        let groups = groups
            .as_array()
            .unwrap_or_else(|| panic!("plugin Hook event {event} must contain matcher groups"));
        for group in groups {
            let handlers = group["hooks"]
                .as_array()
                .unwrap_or_else(|| panic!("plugin Hook event {event} must contain handlers"));
            for handler in handlers {
                assert_eq!(
                    handler["type"].as_str(),
                    Some("command"),
                    "plugin Hook event {event} must use the command transport"
                );
                assert_eq!(
                    handler["timeout"].as_u64(),
                    Some(1),
                    "plugin Hook event {event} must keep the host hard deadline at one second"
                );
                let suffix = match event.as_str() {
                    "SessionStart" => "session-start",
                    "UserPromptSubmit" => "user-prompt",
                    "PreToolUse" => "pre-tool",
                    "PermissionRequest" => "permission-request",
                    "PostToolUse" => "post-tool",
                    "SubagentStart" => "subagent-start",
                    "SubagentStop" => "subagent-stop",
                    "Stop" => "stop",
                    _ => panic!("unexpected plugin event {event}"),
                };
                let command = handler["command"].as_str().expect("Hook command");
                let base = format!("\"$PLUGIN_ROOT/bin/asp-hook-exec\" {suffix} --client codex");
                assert!(
                    command.starts_with(&base),
                    "event={event} command={command}"
                );
                if event == "PreToolUse" {
                    assert!(
                        command.contains("--host-match ")
                            || command.contains("--host-match-prefix "),
                        "{command}"
                    );
                    assert!(!command.contains("--host-action "), "{command}");
                    assert!(!command.contains("--host-matcher"), "{command}");
                } else {
                    assert_eq!(command, base);
                }
            }
        }
    }

    let marketplace: serde_json::Value = serde_json::from_str(ASP_CODEX_PLUGIN_MARKETPLACE_JSON)
        .expect("valid repo marketplace manifest");
    assert_eq!(
        marketplace["plugins"][0]["source"]["path"].as_str(),
        Some("./asp-codex-plugin")
    );
}

#[cfg(unix)]
#[test]
fn plugin_launcher_types_missing_binary_instead_of_exiting_127() {
    let root = temp_root("launcher-missing-binary");
    std::fs::create_dir_all(&root).expect("create isolated launcher state");
    let launcher = root.join("asp-hook");
    std::fs::write(&launcher, ASP_CODEX_PLUGIN_HOOK_LAUNCHER)
        .expect("write isolated plugin launcher");
    let output = std::process::Command::new("/bin/sh")
        .arg(&launcher)
        .args(["pre-tool", "--client", "codex"])
        .env("ASP_STATE_HOME", root.join("missing-state"))
        .env("HOME", root.join("missing-home"))
        .env("PATH", "")
        .output()
        .expect("run plugin launcher without PATH");
    assert_eq!(output.status.code(), Some(0));
    let receipt: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("typed missing binary receipt");
    assert_eq!(receipt["hookSpecificOutput"]["permissionDecision"], "deny");
    assert!(
        receipt["hookSpecificOutput"]["permissionDecisionReason"]
            .as_str()
            .is_some_and(|reason| reason.contains("canonical Runtime Hook binary"))
    );
    std::fs::remove_dir_all(root).expect("remove isolated launcher state");
}

#[cfg(unix)]
#[test]
fn plugin_launcher_types_non_executable_runtime_hook_binary() {
    let root = temp_root("launcher-non-executable-runtime-hook");
    std::fs::create_dir_all(root.join("runtime/artifacts/active"))
        .expect("create active Runtime slot fixture");
    std::fs::write(
        root.join("runtime/artifacts/active/asp-hook"),
        b"not executable",
    )
    .expect("write non-executable Runtime Hook binary");
    let launcher = root.join("asp-hook");
    std::fs::write(&launcher, ASP_CODEX_PLUGIN_HOOK_LAUNCHER)
        .expect("write isolated plugin launcher");

    let output = std::process::Command::new("/bin/sh")
        .arg(&launcher)
        .args(["pre-tool", "--client", "codex", "--host-match", "Bash"])
        .env("ASP_STATE_HOME", &root)
        .env("HOME", root.join("missing-home"))
        .env("PATH", "")
        .output()
        .expect("run plugin launcher without Hook binary");
    assert_eq!(output.status.code(), Some(0));
    let host_output: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("valid Codex failure envelope");
    assert_eq!(
        host_output["hookSpecificOutput"]["permissionDecision"],
        "deny"
    );
    assert!(output.stderr.is_empty());
    std::fs::remove_dir_all(root).expect("remove isolated launcher state");
}

#[cfg(unix)]
#[test]
fn plugin_launcher_executes_only_the_canonical_runtime_hook_binary() {
    use std::os::unix::fs::PermissionsExt;

    let root = temp_root("launcher-runtime-hook");
    std::fs::create_dir_all(root.join("runtime/artifacts/active"))
        .expect("create active Runtime slot fixture");
    let invocation = root.join("invocation.txt");
    let runtime = root.join("runtime/artifacts/active/asp-hook");
    std::fs::write(
        &runtime,
        format!(
            "#!/bin/sh\nprintf '%s\\n' \"$*\" > '{}'\nprintf '%s\\n' '{{\"decision\":\"allow\"}}'\n",
            invocation.display()
        ),
    )
    .expect("write canonical runtime fixture");
    let mut permissions = std::fs::metadata(&runtime)
        .expect("canonical runtime metadata")
        .permissions();
    permissions.set_mode(0o755);
    std::fs::set_permissions(&runtime, permissions).expect("make canonical runtime executable");
    let launcher = root.join("asp-hook");
    std::fs::write(&launcher, ASP_CODEX_PLUGIN_HOOK_LAUNCHER)
        .expect("write isolated plugin launcher");

    let output = std::process::Command::new("/bin/sh")
        .arg(&launcher)
        .args(["pre-tool", "--client", "codex"])
        .env("ASP_STATE_HOME", &root)
        .env("HOME", root.join("missing-home"))
        .env("PATH", "")
        .output()
        .expect("run plugin launcher with canonical runtime");
    assert_eq!(output.status.code(), Some(0));
    assert_eq!(
        std::fs::read_to_string(&invocation).expect("read canonical runtime invocation"),
        "pre-tool --client codex\n"
    );
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&output.stdout)
            .expect("canonical runtime output"),
        serde_json::json!({"decision": "allow"})
    );
    std::fs::remove_dir_all(root).expect("remove isolated launcher state");
}

#[cfg(unix)]
#[test]
fn plugin_launcher_never_falls_back_to_legacy_profile_slots() {
    use std::os::unix::fs::PermissionsExt;

    let root = temp_root("launcher-healthy-fallback");
    let profile = root.join("runtime/artifacts");
    std::fs::create_dir_all(&profile).expect("create ASP artifact profile");
    let invocation = root.join("legacy-invocation.txt");
    let artifact = root
        .join("runtime/artifacts/blake3-256")
        .join("a".repeat(64))
        .join("asp");
    std::fs::create_dir_all(artifact.parent().expect("artifact parent"))
        .expect("create artifact generation");
    std::fs::write(
        &artifact,
        format!(
            "#!/bin/sh\nprintf '%s\\n' \"$*\" > '{}'\nprintf '%s\\n' '{{\"decision\":\"allow\"}}'\n",
            invocation.display()
        ),
    )
    .expect("write healthy ASP fixture");
    let mut permissions = std::fs::metadata(&artifact)
        .expect("artifact metadata")
        .permissions();
    permissions.set_mode(0o755);
    std::fs::set_permissions(&artifact, permissions).expect("make healthy artifact executable");
    std::os::unix::fs::symlink(&artifact, profile.join("healthy")).expect("publish healthy slot");
    std::os::unix::fs::symlink(
        root.join("runtime/artifacts/missing/asp"),
        profile.join("active"),
    )
    .expect("publish broken active slot");
    let launcher = root.join("asp-hook");
    std::fs::write(&launcher, ASP_CODEX_PLUGIN_HOOK_LAUNCHER).expect("write launcher");

    let output = std::process::Command::new("/bin/sh")
        .arg(&launcher)
        .args(["pre-tool", "--client", "codex"])
        .env("ASP_STATE_HOME", &root)
        .env("PATH", "")
        .output()
        .expect("run launcher with only legacy slots");
    assert_eq!(output.status.code(), Some(0));
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&output.stdout).unwrap(),
        serde_json::json!({
            "hookSpecificOutput": {
                "hookEventName": "PreToolUse",
                "permissionDecision": "deny",
                "permissionDecisionReason": "ASP Hook evaluator is unavailable because the canonical Runtime Hook binary is missing or non-executable. Publish it with: asp install binary"
            },
            "systemMessage": "ASP Hook evaluator is unavailable because the canonical Runtime Hook binary is missing or non-executable. Publish it with: asp install binary"
        })
    );
    assert!(!invocation.exists(), "legacy artifact must never execute");
    std::fs::remove_dir_all(root).expect("remove fallback fixture");
}

#[test]
fn plugin_launcher_contains_no_policy_or_runtime_server_plane() {
    for forbidden in [
        "config.toml",
        "server start",
        "server healthcheck",
        "runtime/server",
        ".local/bin",
        "tool_name",
        "toolName",
        "curl ",
        "nc ",
    ] {
        assert!(
            !ASP_CODEX_PLUGIN_HOOK_LAUNCHER.contains(forbidden),
            "plugin launcher crossed into evaluator/server responsibility: {forbidden}"
        );
    }
    let current = ASP_CODEX_PLUGIN_HOOK_LAUNCHER
        .find("runtime/artifacts/active/asp-hook")
        .expect("canonical Runtime Hook binary resolution");
    assert!(
        current > 0,
        "launcher must resolve the canonical Hook artifact"
    );
}

#[test]
fn production_cleanup_removes_inline_hook_without_touching_other_config() {
    let root = temp_root("cleanup");
    std::fs::create_dir_all(&root).expect("create isolated config fixture");
    let config_path = root.join("config.toml");
    let inline = agent_semantic_hook::codex_global_hook_block_with_binary(None);
    std::fs::write(&config_path, format!("model = \"gpt-test\"\n\n{inline}\n"))
        .expect("write isolated config fixture");

    let receipt = remove_codex_managed_global_hook_config(&config_path)
        .expect("remove native inline Hook injection");
    let cleaned = std::fs::read_to_string(&config_path).expect("read cleaned config");
    assert!(receipt.changed);
    assert!(cleaned.contains("model = \"gpt-test\""));
    assert!(!cleaned.contains(agent_semantic_hook::ROOT_BLOCK_BEGIN));
    assert!(!cleaned.contains("[[hooks.PreToolUse]]"));
    std::fs::remove_dir_all(&root).expect("remove isolated config fixture");
}
