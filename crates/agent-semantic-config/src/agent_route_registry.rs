use serde::Deserialize;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

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
pub struct PlatformAgentRegistrySpec {
    pub matcher: String,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct AgentRouteSpec {
    pub session_lifetime: AgentSessionLifetime,
    #[serde(default)]
    pub focus_mode: AgentFocusMode,
    pub roles: Vec<String>,
    #[serde(default = "default_agent_kind")]
    pub agent_kind: String,
    #[serde(default)]
    pub display_role: String,
    #[serde(default)]
    pub description: Option<String>,
}

fn default_agent_kind() -> String {
    "Subagent".to_owned()
}

#[derive(Debug, Clone, Copy, Default, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum AgentFocusMode {
    #[default]
    Standard,
    Leaf,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct AgentRouteRegistry {
    pub schema_id: String,
    pub schema_version: u64,
    pub platforms: BTreeMap<String, PlatformAgentRegistrySpec>,
    pub agents: BTreeMap<String, AgentRouteSpec>,
}

impl AgentRouteRegistry {
    /// Resolves the only configured route that owns `role`.
    pub fn unique_route_for_role(&self, role: &str) -> Result<(&str, &AgentRouteSpec), String> {
        let mut routes = self
            .agents
            .iter()
            .filter(|(_, route)| route.roles.iter().any(|candidate| candidate == role));
        let route = routes
            .next()
            .ok_or_else(|| format!("agent route registry omitted the `{role}` role"))?;
        if routes.next().is_some() {
            return Err(format!(
                "agent route registry defines multiple routes for the `{role}` role"
            ));
        }
        Ok((route.0.as_str(), route.1))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentsRegistry {
    pub registry: AgentRouteRegistry,
    compiled_routes: BTreeMap<(String, String), CompiledAgentRoute>,
    compiled_platform_names: BTreeMap<(String, String), CompiledAgentRoute>,
}

impl AgentsRegistry {
    /// Resolve the unique route whose platform profile owns the exact `name`.
    pub fn compile_route_for_platform_host_agent_name(
        &self,
        platform: &str,
        agent_name: &str,
    ) -> Result<Option<CompiledAgentRoute>, String> {
        Ok(self
            .compiled_platform_names
            .get(&(platform.to_owned(), agent_name.to_owned()))
            .cloned())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompiledAgentRoute {
    pub route_key: AgentRouteKey,
    pub session_lifetime: AgentSessionLifetime,
    pub focus_mode: AgentFocusMode,
    pub roles: Vec<String>,
    pub agent_kind: String,
    pub display_role: String,
    pub description: String,
    pub platform: PlatformId,
    pub platform_host_agent_name: PlatformHostAgentName,
    pub profile_path: String,
    pub model: Option<String>,
    pub sandbox_mode: Option<String>,
}

pub fn load_agent_route_registry(config_path: &Path) -> Result<AgentsRegistry, String> {
    let text = fs::read_to_string(config_path)
        .map_err(|error| format!("failed to read {}: {error}", config_path.display()))?;
    let registry = parse_agent_route_registry(&text, &config_path.display().to_string())?;
    let agents_root = config_path
        .parent()
        .ok_or_else(|| {
            format!(
                "agent route registry has no parent: {}",
                config_path.display()
            )
        })?
        .to_path_buf();
    let mut compiled_routes = BTreeMap::new();
    let mut compiled_platform_names = BTreeMap::new();
    for (platform, platform_spec) in &registry.platforms {
        for (route_key, profile_path) in
            matched_platform_profiles(&agents_root, platform, &platform_spec.matcher)?
        {
            let route = registry.agents.get(&route_key).ok_or_else(|| {
                format!(
                    "platform `{platform}` profile {} has no `[agents.{route_key}]` scheduling route",
                    profile_path.display()
                )
            })?;
            let compiled =
                compile_agent_route_from_source(route, &profile_path, &route_key, platform)?;
            let platform_name_key = (
                platform.clone(),
                compiled.platform_host_agent_name.as_str().to_owned(),
            );
            if let Some(existing) =
                compiled_platform_names.insert(platform_name_key, compiled.clone())
            {
                return Err(format!(
                    "agent name `{}` is owned by both `{}` and `{}` in the `{platform}` platform registry",
                    compiled.platform_host_agent_name.as_str(),
                    existing.route_key.as_str(),
                    route_key,
                ));
            }
            compiled_routes.insert((route_key.clone(), platform.clone()), compiled);
        }
    }
    for route_key in registry.agents.keys() {
        for platform in registry.platforms.keys() {
            if !compiled_routes.contains_key(&(route_key.clone(), platform.clone())) {
                return Err(format!(
                    "agent route `{route_key}` has no profile matching platform `{platform}` matcher"
                ));
            }
        }
    }
    Ok(AgentsRegistry {
        registry,
        compiled_routes,
        compiled_platform_names,
    })
}

/// Parses and validates an agent route registry from an immutable source.
pub fn parse_agent_route_registry(
    source: &str,
    source_label: &str,
) -> Result<AgentRouteRegistry, String> {
    let registry = toml::from_str::<AgentRouteRegistry>(source)
        .map_err(|error| format!("failed to parse {source_label}: {error}"))?;
    validate_agent_route_registry(&registry)?;
    Ok(registry)
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
    loaded: &AgentsRegistry,
    agent_type: &str,
    platform: &str,
) -> Result<CompiledAgentRoute, String> {
    loaded
        .compiled_routes
        .get(&(agent_type.to_owned(), platform.to_owned()))
        .cloned()
        .ok_or_else(|| {
            format!("agent route registry does not define `{agent_type}` for `{platform}`")
        })
}

fn compile_agent_route_from_source(
    agent: &AgentRouteSpec,
    projection_path: &Path,
    agent_type: &str,
    platform: &str,
) -> Result<CompiledAgentRoute, String> {
    let source = fs::read_to_string(&projection_path)
        .map_err(|error| format!("failed to read {}: {error}", projection_path.display()))?;
    let (host_agent_name, description, model, sandbox_mode) = match platform {
        "codex" => {
            let projection: toml::Value = toml::from_str(&source).map_err(|error| {
                format!("failed to parse {}: {error}", projection_path.display())
            })?;
            (
                projection_string(&projection, "name", &projection_path)?,
                projection_string(&projection, "description", &projection_path)?,
                Some(projection_string(&projection, "model", &projection_path)?),
                Some(projection_string(
                    &projection,
                    "sandbox_mode",
                    &projection_path,
                )?),
            )
        }
        "claude" => {
            let required_frontmatter = |field| {
                markdown_frontmatter_value(&source, field).ok_or_else(|| {
                    format!(
                        "Claude projection {} requires `{field}`",
                        projection_path.display()
                    )
                })
            };
            (
                required_frontmatter("name")?,
                required_frontmatter("description")?,
                markdown_frontmatter_value(&source, "model"),
                None,
            )
        }
        _ => {
            return Err(format!(
                "agent route `{agent_type}` declares unsupported platform `{platform}`"
            ));
        }
    };
    Ok(CompiledAgentRoute {
        route_key: AgentRouteKey(agent_type.to_string()),
        session_lifetime: agent.session_lifetime,
        focus_mode: agent.focus_mode,
        roles: agent.roles.clone(),
        agent_kind: agent.agent_kind.clone(),
        display_role: agent.display_role.clone(),
        description: agent.description.clone().unwrap_or(description),
        platform: PlatformId(platform.to_string()),
        platform_host_agent_name: PlatformHostAgentName(host_agent_name),
        profile_path: projection_path.display().to_string(),
        model,
        sandbox_mode,
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
    if registry.platforms.is_empty() {
        return Err("agent route registry must define at least one platform matcher".to_string());
    }
    for (platform, platform_spec) in &registry.platforms {
        validate_identifier(platform, "platform id")?;
        validate_platform_matcher(&platform_spec.matcher, platform)?;
    }
    for (agent_type, agent) in &registry.agents {
        validate_identifier(agent_type, "agent type")?;
        validate_string_set(&agent.roles, agent_type, "roles")?;
    }
    Ok(())
}

fn validate_platform_matcher(value: &str, platform: &str) -> Result<(), String> {
    let expected = match platform {
        "codex" => "*_codex.toml",
        "claude" => "*_claude.md",
        _ => return Err(format!("unsupported agent platform `{platform}`")),
    };
    if value != expected {
        return Err(format!(
            "platform `{platform}` matcher must be exactly `{expected}`, found `{value}`"
        ));
    }
    Ok(())
}

fn matched_platform_profiles(
    agents_root: &Path,
    platform: &str,
    matcher: &str,
) -> Result<Vec<(String, std::path::PathBuf)>, String> {
    let suffix = matcher
        .strip_prefix('*')
        .ok_or_else(|| format!("platform `{platform}` matcher `{matcher}` must start with `*`"))?;
    let mut profiles = Vec::new();
    for entry in fs::read_dir(agents_root)
        .map_err(|error| format!("failed to read {}: {error}", agents_root.display()))?
    {
        let entry = entry.map_err(|error| {
            format!(
                "failed to inspect agent profile in {}: {error}",
                agents_root.display()
            )
        })?;
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let file_name = entry.file_name();
        let file_name = file_name.to_string_lossy();
        let Some(route_key) = file_name.strip_suffix(suffix) else {
            continue;
        };
        validate_identifier(route_key, "agent route key")?;
        profiles.push((route_key.to_owned(), path));
    }
    profiles.sort_by(|left, right| left.0.cmp(&right.0));
    Ok(profiles)
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
