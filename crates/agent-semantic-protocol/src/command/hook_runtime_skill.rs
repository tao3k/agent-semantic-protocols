// Installed ASP Org skill rendering for `asp install hook`.

#[path = "hook_runtime_skill_render.rs"]
pub(crate) mod hook_runtime_skill_render;

pub(super) use hook_runtime_skill_render::render_agent_semantic_protocols_skill_contract;
use hook_runtime_skill_render::{
    render_agent_semantic_protocols_installed_skill, render_agent_semantic_protocols_plugin_skill,
};

use agent_semantic_hook::{HookActivation, RuntimeProfiles, project_agent_config_path};
use agent_semantic_runtime::project_state_paths;
use std::fs;
use std::path::{Path, PathBuf};

const ASP_CODEX_PLUGIN_NAME: &str = "asp-codex-plugin";
const ASP_CODEX_PLUGIN_MARKETPLACE_NAME: &str = "asp-project";
const ASP_CODEX_PLUGIN_MANIFEST_JSON: &str =
    include_str!("../../../../asp-codex-plugin/.codex-plugin/plugin.json");

pub(super) fn install_agent_semantic_protocols_skill(
    project_root: &Path,
    activation: &HookActivation,
    runtime_profiles: &RuntimeProfiles,
) -> Result<InstalledAgentSkillPaths, String> {
    let skill_path = default_agent_skill_path(project_root);
    let org_state_skill_path = project_state_paths(project_root)?
        .protocol_home
        .join("org")
        .join("skills")
        .join("ASP_ORG.org");
    let org_artifacts_path = project_state_paths(project_root)?
        .protocol_home
        .join("artifacts")
        .join("org");
    let rendered_skill = render_agent_semantic_protocols_installed_skill(
        project_root,
        &org_state_skill_path,
        &org_artifacts_path,
        activation,
        runtime_profiles,
    )?;
    write_agent_skill(&skill_path, &rendered_skill)?;
    let skill_contract_path = write_agent_skill_contract(&skill_path, &org_state_skill_path)?;
    Ok(InstalledAgentSkillPaths {
        skill_path: Some(skill_path),
        skill_contract_path: Some(skill_contract_path),
        plugin_skill_path: None,
    })
}

pub(super) fn install_agent_semantic_protocols_plugin_skill(
    project_root: &Path,
    activation: &HookActivation,
    runtime_profiles: &RuntimeProfiles,
) -> Result<InstalledAgentSkillPaths, String> {
    let plugin_skill_path = plugin_skill_path(project_root)?;
    let org_state_skill_path = project_state_paths(project_root)?
        .protocol_home
        .join("org")
        .join("skills")
        .join("ASP_ORG.org");
    let org_artifacts_path = project_state_paths(project_root)?
        .protocol_home
        .join("artifacts")
        .join("org");
    let rendered_skill = render_agent_semantic_protocols_plugin_skill(
        project_root,
        &org_state_skill_path,
        &org_artifacts_path,
        activation,
        runtime_profiles,
    )?;
    write_agent_skill(&plugin_skill_path, &rendered_skill)?;
    remove_plugin_skill_contract(&plugin_skill_path)?;
    Ok(InstalledAgentSkillPaths {
        skill_path: None,
        skill_contract_path: None,
        plugin_skill_path: Some(plugin_skill_path),
    })
}

pub(super) fn install_agent_semantic_protocols_agent_config(
    project_root: &Path,
) -> Result<PathBuf, String> {
    let config_path = project_agent_config_path(project_root);
    if let Some(parent) = config_path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("failed to create {}: {error}", parent.display()))?;
    }
    let existing = match fs::read_to_string(&config_path) {
        Ok(contents) => contents,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(error) => return Err(format!("failed to read {}: {error}", config_path.display())),
    };
    let merged = merge_agent_semantic_protocols_agent_config(&existing)
        .map_err(|error| format!("invalid {}: {error}", config_path.display()))?;
    if merged != existing {
        fs::write(&config_path, merged.as_bytes())
            .map_err(|error| format!("failed to write {}: {error}", config_path.display()))?;
    }
    Ok(config_path)
}

pub(super) struct InstalledAgentSkillPaths {
    pub skill_path: Option<PathBuf>,
    pub skill_contract_path: Option<PathBuf>,
    pub plugin_skill_path: Option<PathBuf>,
}

fn default_agent_skill_path(project_root: &Path) -> PathBuf {
    project_root
        .join(".agents")
        .join("skills")
        .join("agent-semantic-protocols")
        .join("SKILL.org")
}

fn plugin_skill_path(project_root: &Path) -> Result<PathBuf, String> {
    let plugin_root = project_root.join("asp-codex-plugin");
    if !plugin_root
        .join(".codex-plugin")
        .join("plugin.json")
        .is_file()
    {
        return Err(format!(
            "Codex plugin bundle is missing {}; run plugin bundle installation before rendering plugin SKILL.org",
            plugin_root
                .join(".codex-plugin")
                .join("plugin.json")
                .display()
        ));
    }
    let skill_dir = plugin_root.join("skills").join("agent-semantic-protocols");
    Ok(skill_dir.join("SKILL.org"))
}

fn merge_agent_semantic_protocols_agent_config(existing: &str) -> Result<String, String> {
    let mut config = if existing.trim().is_empty() {
        toml::Value::Table(toml::Table::new())
    } else {
        toml::from_str::<toml::Value>(existing).map_err(|error| error.to_string())?
    };
    let root = config
        .as_table_mut()
        .ok_or_else(|| "root document must be a TOML table".to_string())?;
    let skills = root
        .entry("skills".to_string())
        .or_insert_with(|| toml::Value::Table(toml::Table::new()));
    let skills = skills
        .as_table_mut()
        .ok_or_else(|| "`skills` must be a TOML table".to_string())?;
    let mut asp_skill = toml::Table::new();
    asp_skill.insert(
        "template".to_string(),
        toml::Value::String("SKILL.org".to_string()),
    );
    asp_skill.insert(
        "pluginSkill".to_string(),
        toml::Value::String(codex_project_plugin_cache_skill_config_path()?),
    );
    asp_skill.insert(
        "projectSkill".to_string(),
        toml::Value::String(".agents/skills/agent-semantic-protocols/SKILL.org".to_string()),
    );
    asp_skill.insert(
        "aspOrg".to_string(),
        toml::Value::String(
            ".cache/agent-semantic-protocol/org/skills/ASP_ORG.org#asp-org".to_string(),
        ),
    );
    asp_skill.insert(
        "orgArtifacts".to_string(),
        toml::Value::String(".cache/agent-semantic-protocol/artifacts/org".to_string()),
    );
    skills.insert(
        "agent-semantic-protocols".to_string(),
        toml::Value::Table(asp_skill),
    );
    let hook = root
        .entry("hook".to_string())
        .or_insert_with(|| toml::Value::Table(toml::Table::new()));
    let hook = hook
        .as_table_mut()
        .ok_or_else(|| "`hook` must be a TOML table".to_string())?;
    let mut agent_org_artifacts = toml::Table::new();
    agent_org_artifacts.insert("enabled".to_string(), toml::Value::Boolean(true));
    agent_org_artifacts.insert("inactiveAfterMinutes".to_string(), toml::Value::Integer(30));
    agent_org_artifacts.insert(
        "artifactsPath".to_string(),
        toml::Value::String(".cache/agent-semantic-protocol/artifacts/org".to_string()),
    );
    agent_org_artifacts.insert(
        "entrySkillPath".to_string(),
        toml::Value::String(".cache/agent-semantic-protocol/org/skills/ASP_ORG.org".to_string()),
    );
    hook.entry("agentOrgArtifacts".to_string())
        .or_insert_with(|| toml::Value::Table(agent_org_artifacts));
    toml::to_string_pretty(&config).map_err(|error| error.to_string())
}

fn codex_project_plugin_cache_skill_config_path() -> Result<String, String> {
    let manifest = serde_json::from_str::<serde_json::Value>(ASP_CODEX_PLUGIN_MANIFEST_JSON)
        .map_err(|error| format!("invalid ASP Codex plugin manifest JSON: {error}"))?;
    let version = manifest
        .get("version")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| "ASP Codex plugin manifest missing string `version`".to_string())?;
    Ok(format!(
        ".codex/plugins/cache/{ASP_CODEX_PLUGIN_MARKETPLACE_NAME}/{ASP_CODEX_PLUGIN_NAME}/{version}/skills/agent-semantic-protocols/SKILL.org"
    ))
}

fn write_agent_skill(skill_path: &Path, rendered_skill: &str) -> Result<(), String> {
    if let Some(parent) = skill_path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("failed to create {}: {error}", parent.display()))?;
    }
    fs::write(skill_path, format!("{}\n", rendered_skill.trim_end()))
        .map_err(|error| format!("failed to write {}: {error}", skill_path.display()))?;
    Ok(())
}

fn write_agent_skill_contract(
    skill_path: &Path,
    org_state_skill_path: &Path,
) -> Result<PathBuf, String> {
    let contract_path = skill_path.with_file_name("SKILL.contract.org");
    let rendered_contract =
        render_agent_semantic_protocols_skill_contract(&contract_path, org_state_skill_path)?;
    write_agent_skill(&contract_path, &rendered_contract)?;
    Ok(contract_path)
}

fn remove_plugin_skill_contract(skill_path: &Path) -> Result<(), String> {
    let contract_path = skill_path.with_file_name("SKILL.contract.org");
    if contract_path.exists() {
        fs::remove_file(&contract_path)
            .map_err(|error| format!("failed to remove {}: {error}", contract_path.display()))?;
    }
    Ok(())
}
