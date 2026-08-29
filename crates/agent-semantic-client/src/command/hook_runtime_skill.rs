// Installed ASP Org skill rendering for `asp install hook`.

#[path = "hook_runtime_skill_render.rs"]
pub(crate) mod hook_runtime_skill_render;

use self::hook_runtime_skill_render::render_agent_semantic_protocols_installed_skill;

use agent_semantic_hook::project_agent_config_path;
use agent_semantic_runtime::project_state_paths;
use std::fs;
use std::path::{Path, PathBuf};

pub(super) fn install_agent_semantic_protocols_skill(
    project_root: &Path,
) -> Result<InstalledAgentSkillPaths, String> {
    let skill_path = default_agent_skill_path(project_root);
    let paths = project_state_paths(project_root)?;
    let org_state_skill_path = paths
        .protocol_home
        .join("org")
        .join("templates")
        .join("ASP_ORG_SKILL.org");
    let org_artifacts_path = paths.artifacts_dir.join("org");
    let rendered_skill = render_agent_semantic_protocols_installed_skill(
        project_root,
        &org_state_skill_path,
        &org_artifacts_path,
    )?;
    write_agent_skill(&skill_path, &rendered_skill)?;
    Ok(InstalledAgentSkillPaths {
        skill_path: Some(skill_path),
        plugin_skill_path: None,
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
    pub plugin_skill_path: Option<PathBuf>,
}

fn default_agent_skill_path(project_root: &Path) -> PathBuf {
    project_root
        .join(".agents")
        .join("skills")
        .join("agent-semantic-protocols")
        .join("SKILL.org")
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
    let remove_empty_skills = root
        .get_mut("skills")
        .map(|skills| {
            let skills = skills
                .as_table_mut()
                .ok_or_else(|| "`skills` must be a TOML table".to_string())?;
            let remove_empty_asp_skill = skills
                .get_mut("agent-semantic-protocols")
                .map(|asp_skill| {
                    let asp_skill = asp_skill.as_table_mut().ok_or_else(|| {
                        "`skills.agent-semantic-protocols` must be a TOML table".to_string()
                    })?;
                    asp_skill.remove("pluginSkill");
                    asp_skill.remove("aspOrg");
                    asp_skill.remove("orgArtifacts");
                    Ok::<bool, String>(asp_skill.is_empty())
                })
                .transpose()?
                .unwrap_or(false);
            if remove_empty_asp_skill {
                skills.remove("agent-semantic-protocols");
            }
            Ok::<bool, String>(skills.is_empty())
        })
        .transpose()?
        .unwrap_or(false);
    if remove_empty_skills {
        root.remove("skills");
    }

    let remove_empty_hook = root
        .get_mut("hook")
        .and_then(toml::Value::as_table_mut)
        .map(|hook| {
            hook.remove("agentOrgArtifacts");
            hook.is_empty()
        })
        .unwrap_or(false);
    if remove_empty_hook {
        root.remove("hook");
    }
    toml::to_string_pretty(&config).map_err(|error| error.to_string())
}

fn write_agent_skill(skill_path: &Path, rendered_skill: &str) -> Result<(), String> {
    if let Some(parent) = skill_path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("failed to create {}: {error}", parent.display()))?;
    }
    remove_stale_skill_contract(skill_path)?;
    fs::write(skill_path, format!("{}\n", rendered_skill.trim_end()))
        .map_err(|error| format!("failed to write {}: {error}", skill_path.display()))?;
    Ok(())
}

fn remove_stale_skill_contract(skill_path: &Path) -> Result<(), String> {
    let contract_path = skill_path.with_file_name("SKILL.contract.org");
    match fs::remove_file(&contract_path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(format!(
            "failed to remove stale {}: {error}",
            contract_path.display()
        )),
    }
}
