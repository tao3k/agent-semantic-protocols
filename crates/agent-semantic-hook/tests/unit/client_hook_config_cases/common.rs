pub(super) use std::fs;
pub(super) use std::path::{Path, PathBuf};
pub(super) use std::time::{Duration, Instant};

pub(super) use agent_semantic_hook::{
    ClientHookConfig, DecisionKind, HookClassificationRequest, classify_hook_with_config,
    load_client_config, load_client_config_for_project, render_platform_response,
};
pub(super) use serde_json::json;

pub(super) use crate::classifier::registry;

pub(super) fn temp_root(label: &str) -> PathBuf {
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system time")
        .as_nanos();
    let root = std::env::temp_dir().join(format!("agent-semantic-hook-{label}-{nonce}"));
    fs::create_dir_all(&root).expect("create temp root");
    canonical(&root)
}

pub(super) fn with_required_resident_agents(config: &str) -> String {
    format!(
        "{config}\n{}",
        r#"
[agents]

[[agents.residentAgents]]
enabled = true
name = "asp-explore"
role = "asp_explorer"
roles = []
permissions = []
codexAgentName = "asp_explorer"
sessionLifetime = "resident"

[[agents.residentAgents]]
enabled = true
name = "asp-testing"
role = "asp_testing"
roles = []
permissions = []
codexAgentName = "asp_testing"
sessionLifetime = "resident"
"#
    )
}

fn canonical(path: &Path) -> PathBuf {
    path.canonicalize().unwrap_or_else(|_| path.to_path_buf())
}

pub(super) fn org_artifacts_root(root: &Path) -> PathBuf {
    state_home(root)
        .join("projects")
        .join("by-id")
        .join("repo-test")
        .join("workspaces")
        .join("workspace-test")
        .join("artifacts")
        .join("org")
}

pub(super) fn org_state_skill_path(root: &Path) -> PathBuf {
    state_home(root)
        .join("org")
        .join("templates")
        .join("ASP_ORG_SKILL.org")
}

pub(super) fn state_home(root: &Path) -> PathBuf {
    root.join("home").join(".agent-semantic-protocols")
}

pub(super) fn write_org_artifact_set(root: &Path, count: usize, done_files: &[&str]) {
    let artifacts_root = org_artifacts_root(root);
    for index in 0..count {
        let path = artifacts_root
            .join("flow")
            .join("plans")
            .join(format!("active-{index:02}.org"));
        fs::create_dir_all(path.parent().expect("artifact parent")).expect("artifact dir");
        fs::write(&path, format!("* TODO Active {index}\n")).expect("write active org");
    }
    for relative in done_files {
        let path = artifacts_root.join(relative);
        fs::create_dir_all(path.parent().expect("done parent")).expect("done dir");
        fs::write(&path, "* DONE Ready to archive\n").expect("write done org");
    }
}

pub(super) fn contract_bound_org(title: &str) -> String {
    format!(
        r#"* TODO {title} :agent:
:PROPERTIES:
:CONTRACT_ORG: agent.task.v1
:END:
** Goal
Keep recoverable ASP Org state.
** Acceptance
- [X] Hook accepts contract-bound artifacts.
** Progress
- [X] Fixture written.
** Evidence
- cargo test -p agent-semantic-hook client_hook_config
"#
    )
}

pub(super) fn agent_org_artifacts_config(root: &Path, enabled: bool) -> String {
    let artifacts_path = org_artifacts_root(root);
    let entry_skill_path = org_state_skill_path(root);
    with_required_resident_agents(&format!(
        r#"
schemaId = "agent.semantic-protocols.hook.client-config"
schemaVersion = "1"
protocolId = "agent.semantic-protocols.hook"
protocolVersion = "1"

[agentOrgArtifacts]
enabled = {enabled}
inactiveAfterMinutes = 30
artifactsPath = "{}"
entrySkillPath = "{}"

[[rules]]
id = "deny-rg"
enabled = true
event = "pre-tool"
priority = 80
decision = "deny"
reasonKind = "bulk-source-dump"
message = "matched configured rg"

[rules.match]
tool = "Bash"
commandAny = ["rg"]
"#,
        artifacts_path.display().to_string().replace('\\', "\\\\"),
        entry_skill_path.display().to_string().replace('\\', "\\\\")
    ))
}

pub(super) fn agent_org_artifacts_default_config(root: &Path) -> String {
    let artifacts_path = org_artifacts_root(root);
    let entry_skill_path = org_state_skill_path(root);
    with_required_resident_agents(&format!(
        r#"
schemaId = "agent.semantic-protocols.hook.client-config"
schemaVersion = "1"
protocolId = "agent.semantic-protocols.hook"
protocolVersion = "1"

[agentOrgArtifacts]
artifactsPath = "{}"
entrySkillPath = "{}"

[[rules]]
id = "deny-rg"
enabled = true
event = "pre-tool"
priority = 80
decision = "deny"
reasonKind = "bulk-source-dump"
message = "matched configured rg"

[rules.match]
tool = "Bash"
commandAny = ["rg"]
"#,
        artifacts_path.display().to_string().replace('\\', "\\\\"),
        entry_skill_path.display().to_string().replace('\\', "\\\\")
    ))
}
