//! Project state synchronization for `asp sync`.

use super::org_capture::{org_artifacts_root_for_project, run_org_state_sync};
use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::ops::Range;
use std::path::{Path, PathBuf};

const CODEX_AGENT_REGISTRY_BEGIN: &str = "# BEGIN ASP MANAGED CODEX AGENT REGISTRY";
const CODEX_AGENT_REGISTRY_END: &str = "# END ASP MANAGED CODEX AGENT REGISTRY";

pub(super) struct AgentConfigurationSync {
    pub(super) projected: usize,
    pub(super) codex_registry_entries: usize,
    pub(super) codex_spawn_agent_metadata: &'static str,
    pub(super) hook_config_status: &'static str,
}

struct CodexAgentRegistryEntry {
    profile: String,
    projection: String,
}

pub(crate) fn run_sync_command(args: &[String]) -> Result<(), String> {
    if args
        .iter()
        .any(|arg| matches!(arg.as_str(), "help" | "--help" | "-h"))
    {
        println!("{}", usage());
        return Ok(());
    }
    let project_root = project_root_arg(args)?;
    let sync = run_org_state_sync(&project_root)?;
    let agent_configs = sync_agent_configuration()?;
    sync_codex_plugin_activation_cache(&project_root)?;
    let org_state = agent_semantic_runtime::project_state_paths(&project_root)?
        .protocol_home
        .join("org");
    let org_artifacts = org_artifacts_root_for_project(&project_root)?;
    println!(
        "[asp-sync] orgState={} orgArtifacts={} orgRepo={} orgStatus={} agentConfigs={} codexAgentRegistry={} codexSpawnAgentMetadata={} hookConfig={}",
        display_path(&project_root, &org_state),
        display_path(&project_root, &org_artifacts),
        sync.source,
        sync.status,
        agent_configs.projected,
        agent_configs.codex_registry_entries,
        agent_configs.codex_spawn_agent_metadata,
        agent_configs.hook_config_status,
    );
    Ok(())
}

pub(super) fn sync_agent_configuration() -> Result<AgentConfigurationSync, String> {
    let hook_config_status = sync_global_hook_config()?;
    let mut sync = sync_global_agent_configs()?;
    sync.hook_config_status = hook_config_status;
    Ok(sync)
}

pub(super) fn ensure_codex_agent_configuration(
    agent_name: &str,
) -> Result<Option<AgentConfigurationSync>, String> {
    if codex_agent_configuration_ready(agent_name) {
        return Ok(None);
    }
    let sync = sync_agent_configuration()?;
    if codex_agent_configuration_ready(agent_name) {
        Ok(Some(sync))
    } else {
        Err(format!(
            "asp sync completed but configured Codex agent `{agent_name}` is still not registered"
        ))
    }
}

fn codex_agent_configuration_ready(agent_name: &str) -> bool {
    let Ok(state_home) = agent_semantic_runtime::state_core::resolve_state_home() else {
        return false;
    };
    let source_dir = state_home.join("agents");
    let Ok(registry) = load_codex_agent_registry(&source_dir) else {
        return false;
    };
    let Some(entry) = registry.get(agent_name) else {
        return false;
    };
    let source = source_dir.join(&entry.profile);
    let projection = codex_home().join("agents").join(&entry.projection);
    if source.canonicalize().ok() != projection.canonicalize().ok() {
        return false;
    }
    let config_path = codex_home().join("config.toml");
    let Ok(config) = fs::read_to_string(config_path) else {
        return false;
    };
    let Ok(config) = toml::from_str::<toml::Value>(&config) else {
        return false;
    };
    let expected_config_file = format!("agents/{}", entry.projection);
    let agent_registered = config
        .get("agents")
        .and_then(|agents| agents.get(agent_name))
        .and_then(|agent| agent.get("config_file"))
        .and_then(toml::Value::as_str)
        == Some(expected_config_file.as_str());
    let metadata_visible = config
        .get("features")
        .and_then(|features| features.get("multi_agent_v2"))
        .and_then(toml::Value::as_table)
        .is_some_and(|multi_agent_v2| {
            multi_agent_v2.get("enabled").and_then(toml::Value::as_bool) == Some(true)
                && multi_agent_v2
                    .get("hide_spawn_agent_metadata")
                    .and_then(toml::Value::as_bool)
                    == Some(false)
                && multi_agent_v2
                    .get("tool_namespace")
                    .and_then(toml::Value::as_str)
                    .is_some_and(|namespace| !namespace.trim().is_empty())
        });
    agent_registered && metadata_visible
}

fn sync_global_agent_configs() -> Result<AgentConfigurationSync, String> {
    let source_dir = agent_semantic_runtime::state_core::resolve_state_home()?.join("agents");
    let codex_registry = load_codex_agent_registry(&source_dir)?;
    if !source_dir.exists() {
        let codex_registry_entries = sync_codex_agent_registry(&codex_registry)?;
        return Ok(AgentConfigurationSync {
            projected: 0,
            codex_registry_entries,
            codex_spawn_agent_metadata: "visible-agent-type",
            hook_config_status: "unchanged",
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
        projected: synced,
        codex_registry_entries,
        codex_spawn_agent_metadata: "visible-agent-type",
        hook_config_status: "unchanged",
    })
}

fn sync_global_hook_config() -> Result<&'static str, String> {
    let state_home = agent_semantic_runtime::state_core::resolve_state_home()?;
    let config_path = state_home.join("hooks").join("config.toml");
    let default_config = agent_semantic_config::default_hook_client_config_template();
    let existing = match fs::read_to_string(&config_path) {
        Ok(existing) => existing,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            write_hook_config(&config_path, &default_config)?;
            return Ok("created");
        }
        Err(error) => {
            return Err(format!("failed to read {}: {error}", config_path.display()));
        }
    };

    match agent_semantic_config::load_hook_client_config_file(&config_path) {
        Ok(config)
            if config
                .agents
                .resident_agents
                .iter()
                .any(|agent| agent.lifecycle == "asp-command") =>
        {
            Ok("unchanged")
        }
        Ok(_) => {
            let default_value: toml::Value = toml::from_str(&default_config).map_err(|error| {
                format!("failed to parse built-in hook config template: {error}")
            })?;
            let resident = default_value
                .get("agents")
                .and_then(|agents| agents.get("residentAgents"))
                .and_then(toml::Value::as_array)
                .and_then(|agents| agents.first())
                .ok_or_else(|| {
                    "built-in hook config is missing agents.residentAgents[0]".to_string()
                })?
                .clone();
            let mut repaired_value: toml::Value = toml::from_str(&existing).map_err(|error| {
                format!(
                    "failed to parse {} for repair: {error}",
                    config_path.display()
                )
            })?;
            let root = repaired_value
                .as_table_mut()
                .ok_or_else(|| format!("{} must contain a TOML table", config_path.display()))?;
            let agents = root
                .entry("agents".to_string())
                .or_insert_with(|| toml::Value::Table(toml::Table::new()))
                .as_table_mut()
                .ok_or_else(|| format!("{}.agents must be a TOML table", config_path.display()))?;
            let resident_agents = agents
                .entry("residentAgents".to_string())
                .or_insert_with(|| toml::Value::Array(Vec::new()))
                .as_array_mut()
                .ok_or_else(|| {
                    format!(
                        "{}.agents.residentAgents must be an array",
                        config_path.display()
                    )
                })?;
            resident_agents.push(resident);
            let repaired = toml::to_string_pretty(&repaired_value)
                .map_err(|error| format!("failed to render repaired hook config: {error}"))?;
            write_hook_config(&config_path, &repaired)?;
            agent_semantic_config::load_hook_client_config_file(&config_path).map_err(|error| {
                format!(
                    "auto-repaired hook config still fails to load at {}: {error}",
                    config_path.display()
                )
            })?;
            Ok("repaired-resident-route")
        }
        Err(_) => {
            let backup_path = invalid_hook_config_backup_path(&config_path);
            if let Some(parent) = backup_path.parent() {
                fs::create_dir_all(parent)
                    .map_err(|error| format!("failed to create {}: {error}", parent.display()))?;
            }
            fs::rename(&config_path, &backup_path).map_err(|error| {
                format!(
                    "failed to preserve invalid hook config {} as {}: {error}",
                    config_path.display(),
                    backup_path.display()
                )
            })?;
            if let Err(error) = write_hook_config(&config_path, &default_config) {
                let _ = fs::rename(&backup_path, &config_path);
                return Err(error);
            }
            Ok("replaced-invalid-with-backup")
        }
    }
}

fn write_hook_config(path: &Path, contents: &str) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("failed to create {}: {error}", parent.display()))?;
    }
    fs::write(path, contents)
        .map_err(|error| format!("failed to write {}: {error}", path.display()))
}

fn invalid_hook_config_backup_path(config_path: &Path) -> PathBuf {
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or_default();
    config_path.with_extension(format!("toml.invalid-{timestamp}"))
}

fn load_codex_agent_registry(
    source_dir: &Path,
) -> Result<BTreeMap<String, CodexAgentRegistryEntry>, String> {
    let config_path = source_dir.join("config.toml");
    let source = match fs::read_to_string(&config_path) {
        Ok(source) => source,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(BTreeMap::new()),
        Err(error) => {
            return Err(format!("failed to read {}: {error}", config_path.display()));
        }
    };
    let config: toml::Value = toml::from_str(&source)
        .map_err(|error| format!("failed to parse {}: {error}", config_path.display()))?;
    let Some(agents) = config.get("agents").and_then(toml::Value::as_table) else {
        return Ok(BTreeMap::new());
    };
    let mut registry: BTreeMap<String, CodexAgentRegistryEntry> = BTreeMap::new();
    for (agent_id, value) in agents {
        let Some(agent) = value.as_table() else {
            continue;
        };
        let profile = required_agent_config_string(agent, agent_id, "profile", &config_path)?;
        if !profile.ends_with("_codex.toml") {
            continue;
        }
        let host_agent_name = agent
            .get("host_agent_name")
            .and_then(toml::Value::as_str)
            .unwrap_or(agent_id);
        if host_agent_name.is_empty()
            || !host_agent_name
                .chars()
                .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-'))
        {
            return Err(format!(
                "invalid Codex host_agent_name `{host_agent_name}` in {}",
                config_path.display()
            ));
        }
        let projection = required_agent_config_string(agent, agent_id, "projection", &config_path)?;
        if !projection.ends_with(".toml") || Path::new(projection).components().count() != 1 {
            return Err(format!(
                "invalid Codex projection `{projection}` for agent `{agent_id}` in {}",
                config_path.display()
            ));
        }
        let profile_path = source_dir.join(profile);
        if !profile_path.is_file() {
            return Err(format!(
                "Codex profile for agent `{agent_id}` does not exist: {}",
                profile_path.display()
            ));
        }
        if registry.contains_key(host_agent_name) {
            return Err(format!(
                "duplicate Codex host_agent_name `{host_agent_name}` in {}",
                config_path.display()
            ));
        }
        if registry.values().any(|entry| entry.profile == profile) {
            return Err(format!(
                "duplicate Codex profile `{profile}` in {}",
                config_path.display()
            ));
        }
        if registry
            .values()
            .any(|entry| entry.projection == projection)
        {
            return Err(format!(
                "duplicate Codex projection `{projection}` in {}",
                config_path.display()
            ));
        }
        registry.insert(
            host_agent_name.to_owned(),
            CodexAgentRegistryEntry {
                profile: profile.to_owned(),
                projection: projection.to_owned(),
            },
        );
    }
    Ok(registry)
}

fn required_agent_config_string<'a>(
    agent: &'a toml::Table,
    agent_id: &str,
    field: &str,
    config_path: &Path,
) -> Result<&'a str, String> {
    agent
        .get(field)
        .and_then(toml::Value::as_str)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            format!(
                "agent `{agent_id}` is missing string `{field}` in {}",
                config_path.display()
            )
        })
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

fn sync_codex_plugin_activation_cache(project_root: &Path) -> Result<(), String> {
    let source = agent_semantic_runtime::project_state_paths(project_root)?.activation_path;
    if !source.is_file() {
        return Ok(());
    }
    let target = codex_home()
        .join(".cache")
        .join("agent-semantic-protocol")
        .join("hooks")
        .join("activation.json");
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("failed to create {}: {error}", parent.display()))?;
    }
    fs::copy(&source, &target).map(|_| ()).map_err(|error| {
        format!(
            "failed to copy {} to {}: {error}",
            source.display(),
            target.display()
        )
    })
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

fn project_root_arg(args: &[String]) -> Result<PathBuf, String> {
    let cwd = env::current_dir().map_err(|error| format!("failed to read current dir: {error}"))?;
    let root = args
        .iter()
        .find(|arg| !arg.starts_with('-'))
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    Ok(if root.is_absolute() {
        root
    } else {
        cwd.join(root)
    })
}

fn display_path(project_root: &Path, path: &Path) -> String {
    path.strip_prefix(project_root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

fn usage() -> &'static str {
    "usage: asp sync [PROJECT_ROOT]\n\nSynchronizes project-owned ASP state. The Org resource tree is cloned or fast-forwarded from ASP_ORG_REPO_URL, defaulting to https://github.com/tao3k/org.git. Agent-authored Org state belongs under the root returned by `asp paths --get orgArtifacts [PROJECT_ROOT]`.\n\nAlso refreshes ASP-owned global agent config projections from ~/.agent-semantic-protocols/agents/*_codex.toml and *_claude.{md,toml} into the host agent directories."
}
