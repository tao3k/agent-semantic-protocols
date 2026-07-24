use serde_json::{Value, json};

use super::{
    claude_fixture, install_claude_hooks, install_codex_hooks, run_codex_pre_tool_decision,
};

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
    assert!(
        !prompt_path.exists(),
        "hook trigger prompt is hook-crate system policy, not an installed user/project file"
    );
}

#[test]
fn codex_install_without_explicit_binary_root_never_mutates_unrelated_path_asp() {
    let root = claude_fixture();
    let codex_home = root.join(".codex-home");
    let ambient_bin = root.join(".ambient-bin");
    std::fs::create_dir_all(&ambient_bin).expect("create ambient bin");
    let ambient_asp = ambient_bin.join("asp");
    std::fs::write(&ambient_asp, b"ambient-sentinel").expect("write ambient ASP sentinel");
    let ambient_before = std::fs::read(&ambient_asp).expect("snapshot ambient ASP sentinel");
    let isolated_asp = root.join(".bin").join("asp");
    let isolated_before = std::fs::read(&isolated_asp).ok();

    let output = super::support::install_codex_hooks_without_explicit_binary_root(
        root.as_path(),
        &codex_home,
        &ambient_bin,
    );

    assert!(!output.status.success(), "install unexpectedly succeeded");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("refusing to update unrelated PATH binary"),
        "unexpected install error: {stderr}"
    );
    let ambient_after = std::fs::read(&ambient_asp).expect("read ambient ASP sentinel");
    assert_eq!(ambient_after, ambient_before);
    assert_eq!(std::fs::read(&isolated_asp).ok(), isolated_before);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        !stdout.contains("activeArtifactReceipt="),
        "failed install emitted an active artifact receipt: {stdout}"
    );
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
    let isolated_binary = root.join(".bin").join("asp");
    assert!(
        first_install_stdout.contains(&format!("binaryPath={}", isolated_binary.display())),
        "install must target only the explicit fixture binary root: {first_install_stdout}"
    );
    let receipt_field = first_install_stdout
        .split_ascii_whitespace()
        .find_map(|field| field.strip_prefix("activeArtifactReceipt="))
        .expect("active artifact receipt field");
    let receipt_path = {
        let path = std::path::PathBuf::from(receipt_field);
        if path.is_absolute() {
            path
        } else {
            root.join(path)
        }
    };
    assert!(
        receipt_path.starts_with(root.join(".agent-semantic-protocols")),
        "active receipt escaped isolated state root: {}",
        receipt_path.display()
    );
    let receipt: serde_json::Value = serde_json::from_slice(
        &std::fs::read(&receipt_path).expect("read active artifact receipt"),
    )
    .expect("parse active artifact receipt");
    let runtime_asp = receipt["leaves"]
        .as_array()
        .expect("receipt leaves")
        .iter()
        .find(|leaf| leaf["logicalPath"].as_str() == Some("runtime/asp"))
        .expect("runtime ASP leaf");
    let runtime_asp_path = std::path::Path::new(
        runtime_asp["materializedPath"]
            .as_str()
            .expect("runtime ASP materialized path"),
    );
    let isolated_artifact_root = std::fs::canonicalize(root.join(".bin").join(".asp-artifacts"))
        .expect("canonical isolated ASP artifact root");
    assert!(
        runtime_asp_path.starts_with(&isolated_artifact_root),
        "runtime ASP receipt escaped explicit binary root: {}",
        runtime_asp_path.display()
    );
    let codex_config =
        std::fs::read_to_string(root.join(".codex").join("config.toml")).expect("read config");
    assert!(
        codex_config.contains("[plugins.\"asp-codex-plugin@asp-project\"]"),
        "install receipt:\n{first_install_stdout}\ngenerated Codex project config:\n{codex_config}"
    );
    assert!(!codex_config.contains("[agents.asp_explorer]"));
    assert!(!root.join(".codex/agents/asp-explorer.toml").exists());
    let codex_user_config =
        std::fs::read_to_string(codex_home.join("config.toml")).expect("read Codex user config");
    assert!(
        !codex_user_config.contains("[agents.asp_explorer]"),
        "asp-explorer role should be provisioned via agents table under managed state: {codex_user_config}"
    );
    assert!(codex_home.join("agents/asp-explorer.toml").is_file());
    let codex_agent =
        std::fs::read_to_string(codex_home.join("agents/asp-explorer.toml")).expect("read agent");
    assert!(codex_agent.contains("description = \"ASP search/query evidence explorer.\""));
    assert!(codex_agent.contains("asp.search.playbook-receipt"));
    assert!(codex_agent.contains("narrowest parser-owned ASP route"));
    assert!(!codex_agent.contains("fork_context=false"));
    assert!(!codex_agent.contains("fork_turns"));
    assert!(!codex_agent.contains("rootSessionId"));
    assert!(!codex_agent.contains("childSessionId"));
    assert!(!codex_agent.contains("CODEX_THREAD_ID"));
    assert!(!codex_agent.contains("ASP_ROOT_SESSION_ID"));
    assert!(
        !root
            .join("asp-codex-plugin/skills/agent-semantic-protocols/SKILL.org")
            .is_file()
    );
    assert!(root.join(".codex/plugins/cache/asp-project/asp-codex-plugin/0.1.0/skills/agent-semantic-protocols/SKILL.org").is_file());
    assert!(
        !root
            .join("asp-codex-plugin/skills/agent-semantic-protocols/SKILL.contract.org")
            .exists()
    );
    assert!(
        !root
            .join(".agents/skills/agent-semantic-protocols/SKILL.org")
            .exists()
    );
    assert!(!first_install_stdout.contains("skill="));
    assert!(!first_install_stdout.contains("skillContract="));
    assert!(first_install_stdout.contains("pluginSkill="));
    assert!(!first_install_stdout.contains("pluginSkillContract="));
    let second_install_stdout = install_codex_hooks(root.as_path(), &codex_home);
    assert!(
        second_install_stdout.contains("activationSync=reused"),
        "{second_install_stdout}"
    );
    let decision = run_codex_pre_tool_decision(
        root.as_path(),
        json!({"session_id":"session-codex-read","transcript_path":root.as_path().join("session.jsonl"),"cwd":root.as_path(),"tool_name":"Read","tool_input":{"file_path":root.as_path().join("src/lib.rs")}}),
    );
    let message = decision["message"].as_str().expect("decision message");
    assert!(
        message.starts_with("ASP denied source access (`direct-source-read`)"),
        "decision={decision}"
    );
    assert!(message.contains("Next:"), "{message}");
    assert!(message.contains("parser-owned route"), "{message}");
    assert!(!message.contains("[asp-search-subagent]"), "{message}");
    assert!(!message.contains("register --name"), "{message}");
    assert!(matches!(
        decision["fields"]["requiredAction"].as_str(),
        Some("send-to-asp-explore") | Some("enter-asp-explore-choice-pane")
    ));
    assert!(matches!(
        decision["fields"]["nextAction"].as_str(),
        Some("run-asp-command-in-registered-asp-explore-child")
            | Some("choose-one-bootstrap-pane-option")
            | Some("resume-or-send-follow-up-to-same-child")
    ));
    assert_eq!(
        decision["fields"]["targetAgentName"].as_str(),
        Some("asp_explorer")
    );
    assert_eq!(
        decision["fields"]["forbiddenUntilResolved"].as_str(),
        Some("raw-source-fallback")
    );
}
