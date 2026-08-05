use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

pub const AGENT_ROUTE_REGISTRY_SCHEMA_ID: &str = "agent.semantic-protocols.agent-route-registry";
pub const AGENT_ROUTE_REGISTRY_SCHEMA_VERSION: u64 = 1;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct AgentRouteKey(String);

impl AgentRouteKey {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentSessionName(String);

impl AgentSessionName {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlatformId(String);

impl PlatformId {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlatformHostAgentName(String);

impl PlatformHostAgentName {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum AgentSessionLifetime {
    Resident,
    Temporary,
}

impl AgentSessionLifetime {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Resident => "resident",
            Self::Temporary => "temporary",
        }
    }
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct PlatformAgentRouteSpec {
    pub projection: String,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct AgentRouteSpec {
    pub session_name: String,
    pub session_lifetime: AgentSessionLifetime,
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
    agents_root: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompiledAgentRoute {
    pub route_key: AgentRouteKey,
    pub session_name: AgentSessionName,
    pub session_lifetime: AgentSessionLifetime,
    pub roles: Vec<String>,
    pub platform: PlatformId,
    pub platform_host_agent_name: PlatformHostAgentName,
    pub projection: String,
    pub model: Option<String>,
    pub sandbox_mode: Option<String>,
}

pub fn load_agent_route_registry(config_path: &Path) -> Result<LoadedAgentRouteRegistry, String> {
    let text = fs::read_to_string(config_path)
        .map_err(|error| format!("failed to read {}: {error}", config_path.display()))?;
    let registry = toml::from_str::<AgentRouteRegistry>(&text)
        .map_err(|error| format!("failed to parse {}: {error}", config_path.display()))?;
    validate_agent_route_registry(&registry)?;
    let agents_root = config_path
        .parent()
        .ok_or_else(|| format!("agent route registry has no parent: {}", config_path.display()))?
        .to_path_buf();
    Ok(LoadedAgentRouteRegistry {
        registry,
        agents_root,
    })
}

#[derive(Serialize)]
struct HookAgentRouteProjection {
    agents: crate::HookClientAgentsConfig,
}

pub fn render_hook_agent_routes(agents_root: &Path) -> Result<String, String> {
    let loaded = load_agent_route_registry(&agents_root.join("config.toml"))?;
    let mut role_counts = BTreeMap::<String, usize>::new();
    for route in loaded.registry.agents.values() {
        for role in &route.roles {
            *role_counts.entry(role.clone()).or_default() += 1;
        }
    }

    let mut placeholders = BTreeMap::new();
    let mut resident_agents = Vec::new();
    for (route_key, route) in &loaded.registry.agents {
        for role in &route.roles {
            if role_counts.get(role) == Some(&1) {
                placeholders.insert(role.clone(), route.session_name.clone());
            }
        }
        let codex = compile_agent_route(&loaded, route_key, "codex")?;
        let claude = compile_agent_route(&loaded, route_key, "claude")?;
        if codex.platform_host_agent_name != claude.platform_host_agent_name {
            return Err(format!(
                "agent route `{route_key}` platform host name mismatch: Codex=`{}`, Claude=`{}`",
                codex.platform_host_agent_name.as_str(),
                claude.platform_host_agent_name.as_str()
            ));
        }
        let sandbox_mode = codex.sandbox_mode.clone().ok_or_else(|| {
            format!("Codex projection `{}` requires `sandbox_mode`", codex.projection)
        })?;
        resident_agents.push(crate::HookClientResidentAgentConfig {
            enabled: true,
            name: codex.session_name.as_str().to_owned(),
            role: codex.route_key.as_str().to_owned(),
            roles: codex.roles.clone(),
            permissions: vec![sandbox_mode],
            codex_agent_name: codex.platform_host_agent_name.as_str().to_owned(),
            session_lifetime: codex.session_lifetime.as_str().to_owned(),
        });
    }

    toml::to_string(&HookAgentRouteProjection {
        agents: crate::HookClientAgentsConfig {
            placeholders,
            resident_agents,
        },
    })
    .map_err(|error| format!("failed to serialize hook agent route projection: {error}"))
}

fn projection_string(value: &toml::Value, field: &str, path: &Path) -> Result<String, String> {
    value
        .get(field)
        .and_then(toml::Value::as_str)
        .map(str::to_owned)
        .ok_or_else(|| format!("projection {} requires `{field}`", path.display()))
}

fn markdown_frontmatter_value(source: &str, key: &str) -> Option<String> {
    let mut lines = source.lines();
    if lines.next()?.trim() != "---" {
        return None;
    }
    for line in lines {
        let line = line.trim();
        if line == "---" {
            break;
        }
        let Some((candidate, value)) = line.split_once(':') else {
            continue;
        };
        if candidate.trim() == key {
            return Some(value.trim().to_owned());
        }
    }
    None
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
    let projection_path = loaded.agents_root.join(&platform_spec.projection);
    let source = fs::read_to_string(&projection_path)
        .map_err(|error| format!("failed to read {}: {error}", projection_path.display()))?;
    let (host_agent_name, model, sandbox_mode) = match platform {
        "codex" => {
            let projection: toml::Value = toml::from_str(&source).map_err(|error| {
                format!("failed to parse {}: {error}", projection_path.display())
            })?;
            validate_codex_agent_projection(loaded, &projection_path, &projection)?;
            (
                projection_string(&projection, "name", &projection_path)?,
                Some(projection_string(&projection, "model", &projection_path)?),
                Some(projection_string(
                    &projection,
                    "sandbox_mode",
                    &projection_path,
                )?),
            )
        }
        "claude" => (
            markdown_frontmatter_value(&source, "name").ok_or_else(|| {
                format!("Claude projection {} requires `name`", projection_path.display())
            })?,
            markdown_frontmatter_value(&source, "model"),
            None,
        ),
        _ => {
            return Err(format!(
                "agent route `{agent_type}` declares unsupported platform `{platform}`"
            ));
        }
    };
    Ok(CompiledAgentRoute {
        route_key: AgentRouteKey(agent_type.to_string()),
        session_name: AgentSessionName(agent.session_name.clone()),
        session_lifetime: agent.session_lifetime,
        roles: agent.roles.clone(),
        platform: PlatformId(platform.to_string()),
        platform_host_agent_name: PlatformHostAgentName(host_agent_name),
        projection: platform_spec.projection.clone(),
        model,
        sandbox_mode,
    })
}

fn validate_codex_agent_projection(
    loaded: &LoadedAgentRouteRegistry,
    projection_path: &Path,
    projection: &toml::Value,
) -> Result<(), String> {
    let schema_path = loaded.agents_root.join("agents-config-schema.json");
    let schema_source = fs::read_to_string(&schema_path).map_err(|error| {
        format!(
            "Codex agents config schema is unavailable at {}: {error}",
            schema_path.display()
        )
    })?;
    let schema: serde_json::Value = serde_json::from_str(&schema_source)
        .map_err(|error| format!("failed to parse {}: {error}", schema_path.display()))?;
    let revision = schema
        .get("x-codex-source-revision")
        .and_then(serde_json::Value::as_str)
        .filter(|revision| !revision.trim().is_empty())
        .ok_or_else(|| {
            format!(
                "Codex agents config schema {} lacks x-codex-source-revision",
                schema_path.display()
            )
        })?;
    let validator = jsonschema::validator_for(&schema)
        .map_err(|error| format!("failed to compile {}: {error}", schema_path.display()))?;
    let instance = serde_json::to_value(projection).map_err(|error| {
        format!(
            "failed to project {} as JSON for Codex schema validation: {error}",
            projection_path.display()
        )
    })?;
    let errors = validator
        .iter_errors(&instance)
        .map(|error| error.to_string())
        .collect::<Vec<_>>();
    if errors.is_empty() {
        return Ok(());
    }
    Err(format!(
        "Codex agent projection {} violates agents-config-schema.json revision {revision}: {}",
        projection_path.display(),
        errors.join("; ")
    ))
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
        validate_string_set(&agent.roles, agent_type, "roles")?;
        if agent.platforms.is_empty() {
            return Err(format!(
                "agent route `{agent_type}` must define at least one platform"
            ));
        }
        for (platform, platform_spec) in &agent.platforms {
            validate_identifier(platform, "platform id")?;
            validate_projection_path(&platform_spec.projection, agent_type, platform)?;
        }
    }
    Ok(())
}

fn validate_projection_path(
    value: &str,
    agent_type: &str,
    platform: &str,
) -> Result<(), String> {
    let path = Path::new(value);
    if value.trim().is_empty()
        || path.is_absolute()
        || !path
            .components()
            .all(|component| matches!(component, std::path::Component::Normal(_)))
    {
        return Err(format!(
            "agent route `{agent_type}` platform `{platform}` requires a relative projection file name"
        ));
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
#[path = "../tests/unit/agent_route_registry.rs"]
mod tests;
