use std::path::{Path, PathBuf};

pub(super) struct SessionLifetime {
    pub(super) value: String,
    pub(super) resident: bool,
    pub(super) source: String,
}

pub(super) fn resolve_session_lifetime(
    project_root: &Path,
    name: Option<&str>,
    host_client: Option<&str>,
) -> SessionLifetime {
    let (value, source) = configured_session_lifetime(project_root, name, host_client)
        .unwrap_or_else(|| ("temporary".to_string(), "default".to_string()));
    let resident = value == "resident";
    SessionLifetime {
        value,
        resident,
        source,
    }
}

fn configured_session_lifetime(
    project_root: &Path,
    name: Option<&str>,
    host_client: Option<&str>,
) -> Option<(String, String)> {
    let agents_dir = agents_config_dir(project_root)?;
    read_resident_agent_lifetime(&agents_dir.join("config.toml"), name)
        .map(|lifetime| (lifetime, "agent-config".to_string()))
        .or_else(|| {
            read_agent_file_lifetime(&agents_dir, name, host_client)
                .map(|lifetime| (lifetime, "agent-file".to_string()))
        })
        .or_else(|| {
            if host_client != Some("codex") {
                return None;
            }
            read_agent_file_lifetime(&codex_agents_dir()?, name, host_client)
                .map(|lifetime| (lifetime, "codex-agent-file".to_string()))
        })
}

fn agents_config_dir(project_root: &Path) -> Option<PathBuf> {
    std::env::var_os("ASP_AGENTS_HOME")
        .map(PathBuf::from)
        .or_else(|| {
            agent_semantic_runtime::state_core::ResolvedState::resolve(project_root)
                .ok()
                .map(|state| state.state_home.join("agents"))
        })
}

fn read_resident_agent_lifetime(path: &Path, name: Option<&str>) -> Option<String> {
    let name = name?;
    let config: toml::Value = toml::from_str(&std::fs::read_to_string(path).ok()?).ok()?;
    let agents = config.get("agents")?;
    if let Some(lifetime) = agents
        .get("residentAgents")
        .or_else(|| agents.get("resident_agents"))
        .and_then(toml::Value::as_array)
        .and_then(|resident_agents| {
            resident_agents.iter().find_map(|agent| {
                if resident_agent_matches(agent, name) {
                    agent
                        .get("sessionLifetime")
                        .or_else(|| agent.get("session_lifetime"))
                        .and_then(toml::Value::as_str)
                        .map(normalize_session_lifetime)
                } else {
                    None
                }
            })
        })
    {
        return Some(lifetime);
    }
    agents.as_table()?.iter().find_map(|(agent_key, agent)| {
        if matches!(agent_key.as_str(), "residentAgents" | "resident_agents")
            || !resident_agent_table_matches(agent_key, agent, name)
        {
            return None;
        }
        if resident_agent_matches(agent, name) {
            agent
                .get("sessionLifetime")
                .or_else(|| agent.get("session_lifetime"))
                .and_then(toml::Value::as_str)
                .map(normalize_session_lifetime)
        } else {
            None
        }
    })
}

fn resident_agent_matches(agent: &toml::Value, name: &str) -> bool {
    let session_name = agent.get("session_name").and_then(toml::Value::as_str);
    let session_name_camel = agent.get("sessionName").and_then(toml::Value::as_str);
    let agent_name = agent.get("name").and_then(toml::Value::as_str);
    let host_agent_name = agent.get("host_agent_name").and_then(toml::Value::as_str);
    let host_agent_name_camel = agent.get("hostAgentName").and_then(toml::Value::as_str);
    let codex_agent_name = agent.get("codexAgentName").and_then(toml::Value::as_str);
    let codex_agent_name_snake = agent.get("codex_agent_name").and_then(toml::Value::as_str);
    let role = agent.get("role").and_then(toml::Value::as_str);
    [
        session_name,
        session_name_camel,
        agent_name,
        host_agent_name,
        host_agent_name_camel,
        codex_agent_name,
        codex_agent_name_snake,
        role,
    ]
    .into_iter()
    .flatten()
    .any(|candidate| session_name_matches(name, candidate))
}

fn resident_agent_table_matches(agent_key: &str, agent: &toml::Value, name: &str) -> bool {
    session_name_matches(name, agent_key) || resident_agent_matches(agent, name)
}

fn read_agent_file_lifetime(
    agents_dir: &Path,
    name: Option<&str>,
    host_client: Option<&str>,
) -> Option<String> {
    let name = name?;
    let host_client = host_client.unwrap_or("codex");
    for path in agent_file_candidates(agents_dir, name, host_client) {
        let Ok(contents) = std::fs::read_to_string(path) else {
            continue;
        };
        let Ok(config) = toml::from_str::<toml::Value>(&contents) else {
            continue;
        };
        if let Some(lifetime) = config
            .get("session_lifetime")
            .or_else(|| config.get("sessionLifetime"))
            .and_then(toml::Value::as_str)
        {
            return Some(normalize_session_lifetime(lifetime));
        }
    }
    None
}

fn agent_file_candidates(agents_dir: &Path, name: &str, host_client: &str) -> Vec<PathBuf> {
    let normalized = name.replace('_', "-");
    let mut stems = vec![normalized.clone(), normalized.replace('-', "_")];
    if normalized == "asp-explore" {
        stems.push("asp-explorer".to_string());
    }
    stems.sort();
    stems.dedup();
    let mut candidates = Vec::new();
    for stem in stems {
        candidates.push(agents_dir.join(format!("{stem}_{host_client}.toml")));
        candidates.push(agents_dir.join(format!("{stem}.toml")));
    }
    candidates
}

fn codex_agents_dir() -> Option<PathBuf> {
    if let Some(codex_home) = std::env::var_os("CODEX_HOME") {
        return Some(PathBuf::from(codex_home).join("agents"));
    }
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .map(|home| home.join(".codex").join("agents"))
}

fn session_name_matches(left: &str, right: &str) -> bool {
    left == right || left.replace('-', "_") == right || left.replace('_', "-") == right
}

fn normalize_session_lifetime(value: &str) -> String {
    match value {
        "resident" | "permanent" | "persistent" => "resident".to_string(),
        "temporary" | "transient" | "ephemeral" => "temporary".to_string(),
        other => other.to_string(),
    }
}
