use serde::Deserialize;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

pub const AGENT_ROUTE_REGISTRY_SCHEMA_ID: &str = "agent.semantic-protocols.agent-route-registry";
pub const AGENT_ROUTE_REGISTRY_SCHEMA_VERSION: u64 = 1;

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct PlatformAgentRouteSpec {
    pub host_agent_name: String,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct AgentRouteSpec {
    pub session_name: String,
    pub session_lifetime: String,
    pub roles: Vec<String>,
    pub platforms: BTreeMap<String, PlatformAgentRouteSpec>,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct AgentRouteRegistry {
    pub schema_id: String,
    pub schema_version: u64,
    pub agents: BTreeMap<String, AgentRouteSpec>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoadedAgentRouteRegistry {
    pub registry: AgentRouteRegistry,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompiledAgentRoute {
    pub agent_type: String,
    pub platform: String,
    pub host_agent_name: String,
}

pub fn load_agent_route_registry(config_path: &Path) -> Result<LoadedAgentRouteRegistry, String> {
    let text = fs::read_to_string(config_path)
        .map_err(|error| format!("failed to read {}: {error}", config_path.display()))?;
    let registry = toml::from_str::<AgentRouteRegistry>(&text)
        .map_err(|error| format!("failed to parse {}: {error}", config_path.display()))?;
    validate_agent_route_registry(&registry)?;
    Ok(LoadedAgentRouteRegistry { registry })
}

pub fn compile_agent_route(
    loaded: &LoadedAgentRouteRegistry,
    agent_type: &str,
    platform: &str,
) -> Result<CompiledAgentRoute, String> {
    let agent = loaded
        .registry
        .agents
        .get(agent_type)
        .ok_or_else(|| format!("agent route registry does not define `{agent_type}`"))?;
    let platform_spec = agent.platforms.get(platform).ok_or_else(|| {
        format!("agent route `{agent_type}` does not define platform `{platform}`")
    })?;
    Ok(CompiledAgentRoute {
        agent_type: agent_type.to_string(),
        platform: platform.to_string(),
        host_agent_name: platform_spec.host_agent_name.clone(),
    })
}

fn validate_agent_route_registry(registry: &AgentRouteRegistry) -> Result<(), String> {
    if registry.schema_id != AGENT_ROUTE_REGISTRY_SCHEMA_ID {
        return Err(format!(
            "unsupported agent route registry schema_id `{}`",
            registry.schema_id
        ));
    }
    if registry.schema_version != AGENT_ROUTE_REGISTRY_SCHEMA_VERSION {
        return Err(format!(
            "unsupported agent route registry schema_version `{}`",
            registry.schema_version
        ));
    }
    if registry.agents.is_empty() {
        return Err("agent route registry must define at least one agent".to_string());
    }
    for (agent_type, agent) in &registry.agents {
        validate_identifier(agent_type, "agent type")?;
        validate_nonempty(&agent.session_name, agent_type, "session_name")?;
        validate_nonempty(&agent.session_lifetime, agent_type, "session_lifetime")?;
        validate_string_set(&agent.roles, agent_type, "roles")?;
        if agent.platforms.is_empty() {
            return Err(format!(
                "agent route `{agent_type}` must define at least one platform"
            ));
        }
        for (platform, platform_spec) in &agent.platforms {
            validate_identifier(platform, "platform id")?;
            validate_nonempty(
                &platform_spec.host_agent_name,
                agent_type,
                "platform.host_agent_name",
            )?;
        }
    }
    Ok(())
}

fn validate_identifier(value: &str, label: &str) -> Result<(), String> {
    let valid = !value.is_empty()
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'));
    if !valid {
        return Err(format!("invalid {label} `{value}`"));
    }
    Ok(())
}

fn validate_nonempty(value: &str, agent_type: &str, field_name: &str) -> Result<(), String> {
    if value.trim().is_empty() {
        return Err(format!(
            "agent route `{agent_type}` requires non-empty `{field_name}`"
        ));
    }
    Ok(())
}

fn validate_string_set(
    values: &[String],
    agent_type: &str,
    field_name: &str,
) -> Result<(), String> {
    if values.is_empty() {
        return Err(format!(
            "agent route `{agent_type}` requires at least one `{field_name}` entry"
        ));
    }
    let mut unique = BTreeSet::new();
    for value in values {
        validate_identifier(value, field_name)?;
        if !unique.insert(value) {
            return Err(format!(
                "agent route `{agent_type}` contains duplicate `{field_name}` entry `{value}`"
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn canonical_registry_path() -> std::path::PathBuf {
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../agents/config.toml")
    }

    #[test]
    fn canonical_registry_compiles_host_routes() {
        let loaded =
            load_agent_route_registry(&canonical_registry_path()).expect("canonical registry");
        let codex = compile_agent_route(&loaded, "asp_explorer", "codex").expect("Codex route");
        let claude = compile_agent_route(&loaded, "asp_explorer", "claude").expect("Claude route");
        assert_eq!(codex.agent_type, "asp_explorer");
        assert_eq!(claude.agent_type, "asp_explorer");
        assert_eq!(codex.host_agent_name, "asp_explorer");
        assert_eq!(claude.host_agent_name, "asp-explorer");
    }

    #[test]
    fn legacy_manager_projection_fields_fail_closed() {
        let temp = tempfile::tempdir().expect("tempdir");
        let path = temp.path().join("config.toml");
        fs::write(
            &path,
            r#"schema_id = "agent.semantic-protocols.agent-route-registry"
schema_version = 1

[agents.bad]
session_name = "bad"
session_lifetime = "resident"
roles = ["subagent"]

[agents.bad.platforms.codex]
host_agent_name = "bad"
model = "legacy-shadow"
"#,
        )
        .expect("write invalid route registry");
        let error = load_agent_route_registry(&path)
            .expect_err("legacy manager projection field must fail closed");
        assert!(error.contains("unknown field `model`"));
    }

    #[test]
    fn unknown_platform_fails_closed() {
        let loaded =
            load_agent_route_registry(&canonical_registry_path()).expect("canonical registry");
        let error = compile_agent_route(&loaded, "asp_testing", "unknown-host")
            .expect_err("unknown platform must fail closed");
        assert!(error.contains("does not define platform `unknown-host`"));
    }
}
