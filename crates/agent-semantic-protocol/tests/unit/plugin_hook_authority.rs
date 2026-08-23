use super::{
    ASP_CODEX_PLUGIN_HOOK_LAUNCHER, ASP_CODEX_PLUGIN_HOOKS_JSON, ASP_CODEX_PLUGIN_MANIFEST_JSON,
    ASP_CODEX_PLUGIN_MARKETPLACE_JSON, remove_codex_managed_global_hook_config,
};
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
    assert_eq!(events.len(), 8);
    for event in ["PreToolUse", "PermissionRequest", "PostToolUse"] {
        let groups = events[event]
            .as_array()
            .unwrap_or_else(|| panic!("plugin Hook event {event} must contain matcher groups"));
        assert!(
            groups.iter().all(|group| matches!(
                group.get("matcher").and_then(serde_json::Value::as_str),
                None | Some("") | Some("*")
            )),
            "plugin Hook event {event} must use a Codex match-all matcher (`*`, empty, or omitted) so Read, MCP, apply_patch, and Bash all reach the internal action classifier: encodedMatcher={} actual={groups:?}",
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
                assert_eq!(
                    handler["command"].as_str(),
                    Some(format!("\"$PLUGIN_ROOT/bin/asp-hook\" {suffix} --client codex").as_str())
                );
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
            .is_some_and(|reason| reason.contains("active and healthy artifact slots"))
    );
    std::fs::remove_dir_all(root).expect("remove isolated launcher state");
}

#[cfg(unix)]
#[test]
fn plugin_launcher_executes_only_the_canonical_one_shot_hook_binary() {
    use std::os::unix::fs::PermissionsExt;

    let root = temp_root("launcher-canonical-runtime");
    let runtime_dir = root.join("runtime/bin");
    std::fs::create_dir_all(&runtime_dir).expect("create canonical runtime fixture");
    let invocation = root.join("invocation.txt");
    let runtime = runtime_dir.join("asp");
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
        "hook pre-tool --client codex\n"
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
fn plugin_launcher_uses_healthy_slot_when_public_and_active_slots_are_unavailable() {
    use std::os::unix::fs::PermissionsExt;

    let root = temp_root("launcher-healthy-fallback");
    let profile = root.join("runtime/profiles/asp");
    std::fs::create_dir_all(&profile).expect("create ASP artifact profile");
    let invocation = root.join("healthy-invocation.txt");
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
        .expect("run launcher through healthy slot");
    assert_eq!(output.status.code(), Some(0));
    assert_eq!(
        std::fs::read_to_string(invocation).unwrap(),
        "hook pre-tool --client codex\n"
    );
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&output.stdout).unwrap(),
        serde_json::json!({"decision": "allow"})
    );
    std::fs::remove_dir_all(root).expect("remove fallback fixture");
}

#[test]
fn plugin_launcher_contains_no_policy_or_runtime_server_plane() {
    for forbidden in [
        "config.toml",
        "server reconcile",
        "server healthcheck",
        "runtime/server",
        ".local/bin",
        "ASP_NO_AGENT",
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
