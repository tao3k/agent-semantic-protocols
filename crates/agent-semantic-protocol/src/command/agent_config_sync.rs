use serde_json::json;
use std::{
    collections::BTreeSet,
    ffi::OsString,
    path::{Path, PathBuf},
};

const SYNC_SCHEMA_ID: &str = "agent.semantic-protocols.agent-config-sync-receipt.v1";

pub(super) fn run_agent_config_command(args: &[String]) -> Result<(), String> {
    match args.first().map(String::as_str) {
        Some("sync") if args.len() == 1 => sync_agent_config(),
        Some("help" | "--help" | "-h") | None => {
            println!("usage: asp agent config sync");
            Ok(())
        }
        Some(command) => Err(format!(
            "unknown agent config command {command}\nusage: asp agent config sync"
        )),
    }
}

fn sync_agent_config() -> Result<(), String> {
    let project_root = std::env::current_dir()
        .map_err(|error| format!("failed to read current directory: {error}"))?;
    let state_home = agent_semantic_runtime::resolve_state_home()?;
    let receipt = synchronize_agent_config_from_project_root(&project_root, &state_home)?;
    println!(
        "{}",
        serde_json::to_string(&receipt)
            .map_err(|error| format!("failed to encode agent config sync receipt: {error}"))?
    );
    Ok(())
}

pub(crate) fn synchronize_agent_config_from_project_root(
    project_root: &Path,
    state_home: &Path,
) -> Result<serde_json::Value, String> {
    synchronize_agent_config_to_roots(project_root, state_home, &codex_home()?)
}

pub(crate) fn synchronize_agent_config_to_roots(
    project_root: &Path,
    state_home: &Path,
    codex_home: &Path,
) -> Result<serde_json::Value, String> {
    let project_catalog = project_root.join("agents/config.toml");
    if !project_catalog.is_file() {
        return Err(format!(
            "project agent catalog is missing: {}",
            project_catalog.display()
        ));
    }
    let loaded =
        agent_semantic_config::agent_route_registry::load_agent_route_registry(&project_catalog)?;
    let contents = std::fs::read(&project_catalog).map_err(|error| {
        format!(
            "failed to read project agent catalog {}: {error}",
            project_catalog.display()
        )
    })?;
    let state_agents = state_home.join("agents");
    std::fs::create_dir_all(&state_agents).map_err(|error| {
        format!(
            "failed to create ASP agent state directory {}: {error}",
            state_agents.display()
        )
    })?;
    let state_catalog = state_agents.join("config.toml");
    atomic_write(&state_catalog, &contents)?;
    std::fs::create_dir_all(codex_home.join("agents")).map_err(|error| {
        format!(
            "failed to create Codex agent directory {}: {error}",
            codex_home.join("agents").display()
        )
    })?;
    let mut projections = serde_json::Map::new();
    let mut expected_state_profiles = BTreeSet::new();
    for route_key in loaded.registry.agents.keys() {
        for platform in loaded.registry.platforms.keys() {
            let route = agent_semantic_config::agent_route_registry::compile_agent_route(
                &loaded, route_key, platform,
            )?;
            let source_path = Path::new(&route.profile_path);
            let source = std::fs::read(source_path).map_err(|error| {
                format!(
                    "failed to read agent projection {}: {error}",
                    source_path.display()
                )
            })?;
            let profile_file_name = source_path.file_name().ok_or_else(|| {
                format!("agent profile has no file name: {}", source_path.display())
            })?;
            expected_state_profiles.insert(profile_file_name.to_os_string());
            atomic_write(&state_agents.join(profile_file_name), &source)?;
            if platform != "codex" {
                continue;
            }
            let model = route
                .model
                .as_deref()
                .ok_or_else(|| format!("Codex agent route {route_key} requires a model"))?;
            let rendered = String::from_utf8(source)
                .map_err(|error| format!("agent projection is not UTF-8: {error}"))?;
            let parsed: toml::Value = toml::from_str(&rendered).map_err(|error| {
                format!("failed to parse rendered Codex agent {route_key}: {error}")
            })?;
            let projected_name = parsed
                .get("name")
                .and_then(toml::Value::as_str)
                .ok_or_else(|| format!("Codex agent projection {route_key} requires name"))?;
            if projected_name != route.platform_host_agent_name.as_str() {
                return Err(format!(
                    "Codex agent projection {route_key} names {projected_name}, expected {}",
                    route.platform_host_agent_name.as_str()
                ));
            }
            let target = codex_home
                .join("agents")
                .join(format!("{projected_name}.toml"));
            atomic_write(&target, rendered.as_bytes())?;
            projections.insert(
                route_key.clone(),
                json!({
                    "routeKey": route.route_key.as_str(),
                    "platformHostAgentName": route.platform_host_agent_name.as_str(),
                    "model": model,
                    "profilePath": target,
                }),
            );
        }
    }
    let removed_stale_profiles = remove_stale_state_profiles(
        &state_agents,
        &expected_state_profiles,
        loaded
            .registry
            .platforms
            .values()
            .map(|platform| platform.matcher.as_str()),
    )?;
    Ok(json!({
        "schemaId": SYNC_SCHEMA_ID,
        "schemaVersion": "1",
        "stateCatalog": state_catalog,
        "projectCatalog": project_catalog,
        "agents": projections,
        "removedStaleProfiles": removed_stale_profiles,
    }))
}

fn remove_stale_state_profiles<'a>(
    state_agents: &Path,
    expected: &BTreeSet<OsString>,
    matchers: impl Iterator<Item = &'a str>,
) -> Result<Vec<String>, String> {
    let suffixes = matchers
        .map(|matcher| matcher.strip_prefix('*').unwrap_or(matcher).to_owned())
        .collect::<Vec<_>>();
    let mut removed = Vec::new();
    for entry in std::fs::read_dir(state_agents).map_err(|error| {
        format!(
            "failed to inspect ASP agent state directory {}: {error}",
            state_agents.display()
        )
    })? {
        let entry = entry.map_err(|error| format!("failed to inspect agent profile: {error}"))?;
        let file_name = entry.file_name();
        let file_name_text = file_name.to_string_lossy();
        if !entry.path().is_file()
            || expected.contains(&file_name)
            || !suffixes
                .iter()
                .any(|suffix| file_name_text.ends_with(suffix))
        {
            continue;
        }
        std::fs::remove_file(entry.path()).map_err(|error| {
            format!(
                "failed to remove stale agent profile {}: {error}",
                entry.path().display()
            )
        })?;
        removed.push(file_name_text.into_owned());
    }
    removed.sort();
    Ok(removed)
}

fn atomic_write(path: &Path, contents: &[u8]) -> Result<(), String> {
    let temporary = path.with_extension("toml.tmp");
    std::fs::write(&temporary, contents).map_err(|error| {
        format!(
            "failed to write temporary agent catalog {}: {error}",
            temporary.display()
        )
    })?;
    std::fs::rename(&temporary, path).map_err(|error| {
        format!(
            "failed to publish agent catalog {} -> {}: {error}",
            temporary.display(),
            path.display()
        )
    })
}

fn codex_home() -> Result<PathBuf, String> {
    if let Some(path) = std::env::var_os("CODEX_HOME").filter(|path| !path.is_empty()) {
        return Ok(PathBuf::from(path));
    }
    std::env::var_os("HOME")
        .filter(|path| !path.is_empty())
        .map(PathBuf::from)
        .map(|home| home.join(".codex"))
        .ok_or_else(|| "CODEX_HOME and HOME are both unavailable".to_owned())
}
