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
