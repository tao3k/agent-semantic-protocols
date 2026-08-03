//! Project state synchronization for `asp sync`.

use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::ops::Range;
use std::path::{Path, PathBuf};

const CODEX_AGENT_REGISTRY_BEGIN: &str = "# BEGIN ASP MANAGED CODEX AGENT REGISTRY";
const CODEX_AGENT_REGISTRY_END: &str = "# END ASP MANAGED CODEX AGENT REGISTRY";

pub(super) struct AgentConfigurationSync {
    pub(super) published: usize,
    pub(super) projected: usize,
    pub(super) codex_registry_entries: usize,
    pub(super) codex_spawn_agent_metadata: &'static str,
}

struct CodexAgentRegistryEntry {
    profile: String,
    projection: String,
}

pub(crate) fn run_agent_config_sync_command(args: &[String]) -> Result<(), String> {
    if args
        .iter()
        .any(|arg| matches!(arg.as_str(), "help" | "--help" | "-h"))
    {
        println!("{}", usage());
        return Ok(());
    }
    if let Some(argument) = args.first() {
        return Err(format!(
            "asp agent config sync does not accept positional arguments; unexpected argument `{argument}`"
        ));
    }
    let agent_configs = sync_global_agent_configs()?;
    println!(
        "[asp-agent-config-sync] scope=global-agent-config trigger=explicit orgStateSync=consumer-lazy gitPulls=0 gitFetches=0 gitClones=0 publishedAgentConfigs={} agentConfigs={} codexAgentRegistry={} codexSpawnAgentMetadata={} activationWrites=0 dbOpens=0 dbTransactions=0 sessionRegistryOpens=0",
        agent_configs.published,
        agent_configs.projected,
        agent_configs.codex_registry_entries,
        agent_configs.codex_spawn_agent_metadata,
    );
    Ok(())
}

fn sync_global_agent_configs() -> Result<AgentConfigurationSync, String> {
    let source_dir = agent_semantic_runtime::state_core::resolve_state_home()?.join("agents");
    let workspace_source_dir = env::current_dir()
        .map_err(|error| format!("failed to resolve current workspace: {error}"))?
        .join("agents");
    let published = publish_workspace_agent_configs(&workspace_source_dir, &source_dir)?;
    let codex_registry = load_codex_agent_registry(&source_dir)?;
    if !source_dir.exists() {
        let codex_registry_entries = sync_codex_agent_registry(&codex_registry)?;
        return Ok(AgentConfigurationSync {
            published,
            projected: 0,
            codex_registry_entries,
            codex_spawn_agent_metadata: "visible-agent-type",
        });
    }
    let mut synced = 0usize;
    for entry in fs::read_dir(&source_dir)
        .map_err(|error| format!("failed to read {}: {error}", source_dir.display()))?
    {
        let entry = entry.map_err(|error| {
            format!("failed to read entry in {}: {error}", source_dir.display())
        })?;
        let source = entry.path();
        if !source.is_file() {
            continue;
        }
        let Some(file_name) = source.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        if let Some(agent_name) = file_name.strip_suffix("_codex.toml") {
            let projection = codex_registry
                .values()
                .find(|entry| entry.profile == file_name)
                .map(|entry| entry.projection.clone())
                .unwrap_or_else(|| format!("{agent_name}.toml"));
            let target = codex_home().join("agents").join(projection);
            project_agent_config(&source, &target)?;
            synced += 1;
        } else if let Some(agent_name) = file_name.strip_suffix("_claude.md") {
            let target = claude_home()
                .join("agents")
                .join(format!("{agent_name}.md"));
            project_agent_config(&source, &target)?;
            synced += 1;
        } else if let Some(agent_name) = file_name.strip_suffix("_claude.toml") {
            let target = claude_home()
                .join("agents")
                .join(format!("{agent_name}.toml"));
            project_agent_config(&source, &target)?;
            synced += 1;
        }
    }
    let codex_registry_entries = sync_codex_agent_registry(&codex_registry)?;
    Ok(AgentConfigurationSync {
        published,
        projected: synced,
        codex_registry_entries,
        codex_spawn_agent_metadata: "visible-agent-type",
    })
}

fn publish_workspace_agent_configs(
    workspace_source_dir: &Path,
    state_source_dir: &Path,
) -> Result<usize, String> {
    if !workspace_source_dir.is_dir() {
        return Ok(0);
    }
    let state_root = state_source_dir.parent().ok_or_else(|| {
        format!(
            "global agent config directory has no state root: {}",
            state_source_dir.display()
        )
    })?;
    let publication = agent_semantic_config::subagent_manager::publish_subagent_catalog(
        &workspace_source_dir.join("config.toml"),
        state_source_dir,
        state_root,
    )?;
    Ok(publication.projections.len())
}

fn load_codex_agent_registry(
    source_dir: &Path,
) -> Result<BTreeMap<String, CodexAgentRegistryEntry>, String> {
    let config_path = source_dir.join("config.toml");
    if !config_path.is_file() {
        return Ok(BTreeMap::new());
    }
    let loaded = agent_semantic_config::subagent_manager::load_subagent_catalog(&config_path)?;
    let mut registry: BTreeMap<String, CodexAgentRegistryEntry> = BTreeMap::new();
    for (agent_id, agent) in &loaded.catalog.agents {
        let Some(codex) = agent.platforms.get("codex") else {
            continue;
        };
        let profile_path = source_dir.join(&codex.profile);
        if !profile_path.is_file() {
            return Err(format!(
                "Codex profile for agent `{agent_id}` does not exist: {}",
                profile_path.display()
            ));
        }
        if registry.contains_key(&codex.host_agent_name) {
            return Err(format!(
                "duplicate Codex host_agent_name `{}` in {}",
                codex.host_agent_name,
                config_path.display()
            ));
        }
        if registry
            .values()
            .any(|entry| entry.profile == codex.profile)
        {
            return Err(format!(
                "duplicate Codex profile `{}` in {}",
                codex.profile,
                config_path.display()
            ));
        }
        if registry
            .values()
            .any(|entry| entry.projection == codex.projection)
        {
            return Err(format!(
                "duplicate Codex projection `{}` in {}",
                codex.projection,
                config_path.display()
            ));
        }
        registry.insert(
            codex.host_agent_name.clone(),
            CodexAgentRegistryEntry {
                profile: codex.profile.clone(),
                projection: codex.projection.clone(),
            },
        );
    }
    Ok(registry)
}

fn sync_codex_agent_registry(
    registry: &BTreeMap<String, CodexAgentRegistryEntry>,
) -> Result<usize, String> {
    let config_path = codex_home().join("config.toml");
    let existing = match fs::read_to_string(&config_path) {
        Ok(existing) => existing,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(error) => {
            return Err(format!("failed to read {}: {error}", config_path.display()));
        }
    };
    let original = existing.clone();
    let managed_range = codex_managed_registry_range(&existing)?;
    let mut unmanaged = existing.clone();
    if let Some(range) = managed_range.clone() {
        unmanaged.replace_range(range, "");
    }
    unmanaged = remove_legacy_codex_multi_agent_v2_conflicts(&unmanaged);
    unmanaged = ensure_codex_multi_agent_v2_metadata_visible(&unmanaged);
    let unmanaged_config: toml::Value = if unmanaged.trim().is_empty() {
        toml::Value::Table(toml::Table::new())
    } else {
        toml::from_str(&unmanaged)
            .map_err(|error| format!("failed to parse {}: {error}", config_path.display()))?
    };
    if let Some(user_agents) = unmanaged_config
        .get("agents")
        .and_then(toml::Value::as_table)
    {
        for agent_name in registry.keys() {
            if user_agents.contains_key(agent_name) {
                return Err(format!(
                    "cannot manage [agents.{agent_name}] in {} because it is defined outside the ASP-managed registry block",
                    config_path.display()
                ));
            }
        }
    }
    let managed = render_codex_agent_registry(registry);
    let mut next = unmanaged;
    if !managed.is_empty() {
        if !next.is_empty() && !next.ends_with('\n') {
            next.push('\n');
        }
        if !next.is_empty() && !next.ends_with("\n\n") {
            next.push('\n');
        }
        next.push_str(&managed);
    }
    if next != original {
        if let Some(parent) = config_path.parent() {
            fs::create_dir_all(parent)
                .map_err(|error| format!("failed to create {}: {error}", parent.display()))?;
        }
        fs::write(&config_path, next)
            .map_err(|error| format!("failed to write {}: {error}", config_path.display()))?;
    }
    Ok(registry.len())
}

fn render_codex_agent_registry(registry: &BTreeMap<String, CodexAgentRegistryEntry>) -> String {
    if registry.is_empty() {
        return String::new();
    }
    let mut rendered = format!("{CODEX_AGENT_REGISTRY_BEGIN}\n");
    for (agent_name, entry) in registry {
        let config_file = toml::Value::String(format!("agents/{}", entry.projection)).to_string();
        rendered.push_str(&format!(
            "[agents.{agent_name}]\nconfig_file = {config_file}\n\n"
        ));
    }
    rendered.push_str(CODEX_AGENT_REGISTRY_END);
    rendered.push('\n');
    rendered
}

fn ensure_codex_multi_agent_v2_metadata_visible(config: &str) -> String {
    const SECTION: &str = "[features.multi_agent_v2]";
    const ENABLED: &str = "enabled = true";
    const METADATA: &str = "hide_spawn_agent_metadata = false";
    const TOOL_NAMESPACE: &str = "tool_namespace = \"collaboration_v2\"";

    let lines = config.lines().collect::<Vec<_>>();
    let Some(section_start) = lines.iter().position(|line| line.trim() == SECTION) else {
        let mut rendered = config.trim_end_matches('\n').to_string();
        if !rendered.is_empty() {
            rendered.push_str("\n\n");
        }
        rendered.push_str(SECTION);
        rendered.push('\n');
        rendered.push_str(ENABLED);
        rendered.push('\n');
        rendered.push_str(METADATA);
        rendered.push('\n');
        rendered.push_str(TOOL_NAMESPACE);
        rendered.push('\n');
        return rendered;
    };

    let section_end = lines
        .iter()
        .enumerate()
        .skip(section_start + 1)
        .find_map(|(index, line)| line.trim().starts_with('[').then_some(index))
        .unwrap_or(lines.len());
    let mut rendered = Vec::with_capacity(lines.len() + 2);
    let mut saw_enabled = false;
    let mut saw_metadata = false;
    let mut saw_tool_namespace = false;
    for (index, line) in lines.iter().enumerate() {
        if index == section_end {
            if !saw_enabled {
                rendered.push(ENABLED.to_string());
            }
            if !saw_metadata {
                rendered.push(METADATA.to_string());
            }
            if !saw_tool_namespace {
                rendered.push(TOOL_NAMESPACE.to_string());
            }
        }
        if index > section_start && index < section_end {
            let key = line.trim().split_once('=').map(|(key, _)| key.trim());
            match key {
                Some("enabled") if !saw_enabled => {
                    rendered.push(ENABLED.to_string());
                    saw_enabled = true;
                    continue;
                }
                Some("enabled") => continue,
                Some("hide_spawn_agent_metadata") if !saw_metadata => {
                    rendered.push(METADATA.to_string());
                    saw_metadata = true;
                    continue;
                }
                Some("hide_spawn_agent_metadata") => continue,
                Some("tool_namespace") if !saw_tool_namespace => {
                    let value = line
                        .split_once('=')
                        .map(|(_, value)| value.trim())
                        .unwrap_or_default();
                    if value == "\"asp_collaboration\"" {
                        // Migrate only the legacy namespace previously owned
                        // by ASP. Any other non-empty namespace belongs to the
                        // user and must remain byte-for-byte intact.
                        rendered.push(TOOL_NAMESPACE.to_string());
                    } else {
                        rendered.push((*line).to_string());
                    }
                    saw_tool_namespace = true;
                    continue;
                }
                Some("tool_namespace") => continue,
                // The running Codex host can lag the checked-out source and
                // deny unknown fields. ASP owns removal of this newer optional
                // projection so the global config remains loadable.
                Some("expose_spawn_agent_model_overrides") => continue,
                _ => {}
            }
        }
        rendered.push((*line).to_string());
    }
    if section_end == lines.len() {
        if !saw_enabled {
            rendered.push(ENABLED.to_string());
        }
        if !saw_metadata {
            rendered.push(METADATA.to_string());
        }
        if !saw_tool_namespace {
            rendered.push(TOOL_NAMESPACE.to_string());
        }
    }
    let mut rendered = rendered.join("\n");
    rendered.push('\n');
    rendered
}

fn remove_legacy_codex_multi_agent_v2_conflicts(config: &str) -> String {
    let mut section = "";
    let mut rendered = String::with_capacity(config.len());
    for line in config.split_inclusive('\n') {
        let trimmed = line.trim();
        if trimmed.starts_with('[') {
            section = trimmed;
        }
        let legacy_key = trimmed.split_once('=').map(|(key, _)| key.trim());
        let conflicts_with_multi_agent_v2 = matches!(
            (section, legacy_key),
            ("[features]", Some("multi_agent_v2")) | ("[agents]", Some("max_threads"))
        );
        if !conflicts_with_multi_agent_v2 {
            rendered.push_str(line);
        }
    }
    rendered
}

fn codex_managed_registry_range(config: &str) -> Result<Option<Range<usize>>, String> {
    let starts = config
        .match_indices(CODEX_AGENT_REGISTRY_BEGIN)
        .map(|(index, _)| index)
        .collect::<Vec<_>>();
    let ends = config
        .match_indices(CODEX_AGENT_REGISTRY_END)
        .map(|(index, _)| index)
        .collect::<Vec<_>>();
    match (starts.as_slice(), ends.as_slice()) {
        ([], []) => Ok(None),
        ([start], [end]) if start < end => {
            let mut range_end = end + CODEX_AGENT_REGISTRY_END.len();
            if config[range_end..].starts_with("\r\n") {
                range_end += 2;
            } else if config[range_end..].starts_with('\n') {
                range_end += 1;
            }
            Ok(Some(*start..range_end))
        }
        _ => Err(
            "invalid ASP-managed Codex agent registry markers: expected exactly one ordered begin/end pair"
                .to_string(),
        ),
    }
}

pub(super) fn codex_home() -> PathBuf {
    env::var_os("CODEX_HOME")
        .map(PathBuf::from)
        .or_else(|| env::var_os("HOME").map(|home| PathBuf::from(home).join(".codex")))
        .unwrap_or_else(|| PathBuf::from(".codex"))
}

fn claude_home() -> PathBuf {
    env::var_os("CLAUDE_HOME")
        .map(PathBuf::from)
        .or_else(|| env::var_os("HOME").map(|home| PathBuf::from(home).join(".claude")))
        .unwrap_or_else(|| PathBuf::from(".claude"))
}

fn project_agent_config(source: &Path, target: &Path) -> Result<(), String> {
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("failed to create {}: {error}", parent.display()))?;
    }
    match fs::symlink_metadata(target) {
        Ok(metadata) => {
            if metadata.is_dir() {
                return Err(format!("cannot replace directory {}", target.display()));
            }
            fs::remove_file(target)
                .map_err(|error| format!("failed to replace {}: {error}", target.display()))?;
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => {
            return Err(format!("failed to inspect {}: {error}", target.display()));
        }
    }
    link_or_copy_agent_config(source, target)
}

#[cfg(unix)]
fn link_or_copy_agent_config(source: &Path, target: &Path) -> Result<(), String> {
    std::os::unix::fs::symlink(source, target).map_err(|error| {
        format!(
            "failed to symlink {} -> {}: {error}",
            target.display(),
            source.display()
        )
    })
}

#[cfg(not(unix))]
fn link_or_copy_agent_config(source: &Path, target: &Path) -> Result<(), String> {
    fs::copy(source, target).map(|_| ()).map_err(|error| {
        format!(
            "failed to copy {} -> {}: {error}",
            source.display(),
            target.display()
        )
    })
}

fn usage() -> &'static str {
    "usage: asp agent config sync\n\nExplicitly publishes managed profiles from the current workspace's agents/ directory into ~/.agent-semantic-protocols/agents, then reconciles those ASP-owned global profiles into the host agent directories. Run it when repairing a missing or stale host projection. It is never triggered by search, query, PreToolUse, checkpoint, or workspace activation. It does not build source indexes, start resident services, write activation state, or clone, fetch, or pull Git repositories. Org consumers synchronize missing contract or template resources lazily."
}
