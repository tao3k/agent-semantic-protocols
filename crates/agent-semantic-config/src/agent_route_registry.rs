// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Canonical Host-agent routes, permissions, and compiled platform projections.

use serde::Deserialize;
use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

#[path = "agent_route_projection_validation.rs"]
mod projection_validation;

use projection_validation::markdown_frontmatter_value;
use projection_validation::projection_string;
use projection_validation::validate_claude_plugin_projection;
use projection_validation::validate_codex_agent_projection;

/// Stable schema identifier for the declarative agent-route registry.
pub const AGENT_ROUTE_REGISTRY_SCHEMA_ID: &str = "agent.semantic-protocols.agent-route-registry";
/// Stable schema version for the declarative agent-route registry.
pub const AGENT_ROUTE_REGISTRY_SCHEMA_VERSION: u64 = 1;

/// Canonical route key declared by the agent registry.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct AgentRouteKey(String);

impl AgentRouteKey {
    /// Return the canonical route-key text.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Canonical Host platform identifier.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlatformId(String);

impl PlatformId {
    /// Return the canonical platform identifier.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Native Host agent name bound to one route.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlatformHostAgentName(String);

impl PlatformHostAgentName {
    /// Return the Host-native agent name.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Lifetime semantics for a configured AgentSession.
#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum AgentSessionLifetime {
    /// Durable configured Agent.
    Resident,
    /// Ephemeral Host subagent.
    Temporary,
}

/// Closed Host distinction between configured Agents and temporary SubAgents.
#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum AgentKind {
    /// Configured Agent loaded from the Host Agent registry.
    Agent,
    /// Temporary SubAgent created for one bounded task.
    Subagent,
}

impl AgentKind {
    /// Return the stable schema spelling.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Agent => "agent",
            Self::Subagent => "subagent",
        }
    }
}

impl std::fmt::Display for AgentKind {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// Closed Codex sandbox modes admitted by an Agent definition.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HostSandboxMode {
    /// Source reads only.
    ReadOnly,
    /// Mutations restricted to the active workspace.
    WorkspaceWrite,
    /// Unrestricted Host filesystem access.
    DangerFullAccess,
}

impl HostSandboxMode {
    /// Parse the exact Codex schema spelling.
    pub fn parse(value: &str) -> Result<Self, String> {
        match value {
            "read-only" => Ok(Self::ReadOnly),
            "workspace-write" => Ok(Self::WorkspaceWrite),
            "danger-full-access" => Ok(Self::DangerFullAccess),
            _ => Err(format!("unsupported Host sandbox mode `{value}`")),
        }
    }

    /// Return the exact Codex schema spelling.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ReadOnly => "read-only",
            Self::WorkspaceWrite => "workspace-write",
            Self::DangerFullAccess => "danger-full-access",
        }
    }
}

impl AgentSessionLifetime {
    /// Return the schema spelling of the lifetime.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Resident => "resident",
            Self::Temporary => "temporary",
        }
    }
}

/// Host matcher that selects a platform projection.
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct PlatformAgentRegistrySpec {
    /// Glob or exact matcher declared by the Host profile.
    pub matcher: String,
}

/// Declarative routing policy for one configured Agent.
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct AgentRouteSpec {
    pub session_lifetime: AgentSessionLifetime,
    #[serde(default)]
    pub focus_mode: AgentFocusMode,
    pub roles: Vec<String>,
    pub allowed_rule_intents: Vec<String>,
    pub agent_kind: AgentKind,
    #[serde(default)]
    pub display_role: String,
    #[serde(default)]
    pub description: Option<String>,
}

/// Host focus behavior attached to an Agent route.
#[derive(Debug, Clone, Copy, Default, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum AgentFocusMode {
    #[default]
    /// General-purpose Agent route.
    Standard,
    /// Leaf task route that does not recursively coordinate Agents.
    Leaf,
}

/// Parsed agent registry document before Host-specific compilation.
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct AgentRouteRegistry {
    pub schema_id: String,
    pub schema_version: u64,
    pub platforms: BTreeMap<String, PlatformAgentRegistrySpec>,
    pub agents: BTreeMap<String, AgentRouteSpec>,
}

/// Registry plus its deterministic Host-specific lookup indexes.
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

    /// Resolve the unique route that declaratively owns the exact Host role.
    pub fn compile_route_for_platform_host_role(
        &self,
        platform: &str,
        role: &str,
    ) -> Result<Option<CompiledAgentRoute>, String> {
        let routes = self
            .registry
            .agents
            .iter()
            .filter(|(_, route)| route.roles.iter().any(|candidate| candidate == role))
            .filter_map(|(route_key, _)| {
                self.compiled_routes
                    .get(&(route_key.clone(), platform.to_owned()))
            })
            .cloned()
            .collect::<Vec<_>>();
        if routes.is_empty() {
            return Ok(None);
        }
        if routes.len() != 1 {
            let owners = routes
                .iter()
                .map(|route| route.route_key.as_str())
                .collect::<Vec<_>>()
                .join(", ");
            return Err(format!(
                "agent role `{role}` is ambiguous in the `{platform}` platform registry: {owners}"
            ));
        }
        Ok(routes.first().cloned())
    }

    /// Resolve either a canonical Host agent name or a native Host role.
    pub fn compile_route_for_platform_host_identity(
        &self,
        platform: &str,
        identity: &str,
    ) -> Result<Option<CompiledAgentRoute>, String> {
        let by_name = self.compile_route_for_platform_host_agent_name(platform, identity)?;
        let by_role = self.compile_route_for_platform_host_role(platform, identity)?;
        match (by_name, by_role) {
            (None, None) => Ok(None),
            (Some(route), None) | (None, Some(route)) => Ok(Some(route)),
            (Some(by_name), Some(by_role)) if by_name.route_key == by_role.route_key => {
                Ok(Some(by_name))
            }
            (Some(by_name), Some(by_role)) => Err(format!(
                "Host identity `{identity}` resolves to both `{}` and `{}` in the `{platform}` platform registry",
                by_name.route_key.as_str(),
                by_role.route_key.as_str(),
            )),
        }
    }
}

/// Schema identifier for Codex Agent definition projections.
pub const CODEX_AGENT_DEFINITION_SCHEMA_ID: &str =
    "urn:agent-semantic-protocols:schema:codex-agent-definition";
/// Schema identifier for Claude Agent frontmatter.
pub const ANTHROPIC_AGENT_FRONTMATTER_SCHEMA_ID: &str =
    "urn:agent-semantic-protocols:schema:anthropic-agent-frontmatter";
/// Schema identifier for Claude plugin Agent frontmatter.
pub const ANTHROPIC_PLUGIN_AGENT_FRONTMATTER_SCHEMA_ID: &str =
    "urn:agent-semantic-protocols:schema:anthropic-plugin-agent-frontmatter";
/// Schema identifier for the route-registry document projection.
pub const AGENT_ROUTE_REGISTRY_DOCUMENT_SCHEMA_ID: &str =
    "urn:agent-semantic-protocols:schema:agent-route-registry";
/// Embedded JSON Schema for Codex Agent definitions.
pub const CODEX_AGENT_DEFINITION_SCHEMA: &str =
    include_str!("../schemas/codex-agent-definition.schema.json");
/// Embedded JSON Schema for Claude Agent frontmatter.
pub const ANTHROPIC_AGENT_FRONTMATTER_SCHEMA: &str =
    include_str!("../schemas/anthropic-agent-frontmatter.schema.json");
/// Embedded JSON Schema for Claude plugin Agent frontmatter.
pub const ANTHROPIC_PLUGIN_AGENT_FRONTMATTER_SCHEMA: &str =
    include_str!("../schemas/anthropic-plugin-agent-frontmatter.schema.json");
/// Embedded JSON Schema for the route registry.
pub const AGENT_ROUTE_REGISTRY_SCHEMA: &str =
    include_str!("../schemas/agent-route-registry.schema.json");

/// Filesystem permission action controlled by an Agent sandbox.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgentPermissionAction {
    /// Source mutation capability.
    Edit,
}

impl AgentPermissionAction {
    /// Return the canonical permission-action spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Edit => "edit",
        }
    }
}

/// Effective permission constraints derived from the Host profile.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EffectiveAgentPermissions {
    denied_actions: Vec<AgentPermissionAction>,
}

impl EffectiveAgentPermissions {
    /// Test whether this route denies an action.
    #[must_use]
    pub fn denies(&self, action: AgentPermissionAction) -> bool {
        self.denied_actions.contains(&action)
    }

    /// Iterate over all denied actions.
    pub fn denied_actions(&self) -> impl Iterator<Item = AgentPermissionAction> + '_ {
        self.denied_actions.iter().copied()
    }

    fn read_only() -> Self {
        Self {
            denied_actions: vec![AgentPermissionAction::Edit],
        }
    }
}

/// Host-ready route compiled from one registry entry and platform projection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompiledAgentRoute {
    pub route_key: AgentRouteKey,
    pub session_lifetime: AgentSessionLifetime,
    pub focus_mode: AgentFocusMode,
    pub roles: Vec<String>,
    pub allowed_rule_intents: Vec<String>,
    pub agent_kind: AgentKind,
    pub display_role: String,
    pub description: String,
    pub platform: PlatformId,
    pub platform_host_agent_name: PlatformHostAgentName,
    pub profile_path: String,
    pub model: Option<String>,
    pub sandbox_mode: Option<HostSandboxMode>,
    pub definition_schema_id: &'static str,
    pub effective_permissions: EffectiveAgentPermissions,
}

/// Host-owned invocation projection for one semantic Agent route.
///
/// The Hook DSL stores only `route_key`. This projection is materialized from
/// the active platform profile, so Host syntax never leaks into policy rules.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HostAgentInvocationProjection {
    /// Host invocation kind.
    pub kind: &'static str,
    /// Native calling symbol derived from the active Host profile's `name`.
    pub symbol: String,
    /// Complete Host-native invocation syntax.
    pub syntax: String,
}

impl CompiledAgentRoute {
    /// Whether this Loader-owned route is eligible to become a durable child
    /// AgentSession after a real Host Agent call.
    ///
    /// Merely appearing in a SubagentStart payload is not registration
    /// authority. Temporary SubAgents and non-Agent Host kinds never become
    /// resident ASP DB records.
    /// Project one configured route into Host-native invocation syntax.
    #[must_use]
    pub fn is_resident_agent(&self) -> bool {
        self.agent_kind == AgentKind::Agent
            && self.session_lifetime == AgentSessionLifetime::Resident
    }

    #[must_use]
    pub fn host_invocation(&self, symbol: &str) -> HostAgentInvocationProjection {
        match self.platform.as_str() {
            "codex" => HostAgentInvocationProjection {
                kind: "codex-agent-symbol",
                syntax: symbol.to_owned(),
                symbol: symbol.to_owned(),
            },
            "claude" => HostAgentInvocationProjection {
                kind: "claude-agent-mention",
                syntax: symbol.to_owned(),
                symbol: symbol.to_owned(),
            },
            _ => HostAgentInvocationProjection {
                kind: "host-agent",
                syntax: symbol.to_owned(),
                symbol: symbol.to_owned(),
            },
        }
    }
}

/// Load and compile the full registry for publication-time validation.
pub fn load_agent_route_registry(config_path: &Path) -> Result<AgentsRegistry, String> {
    load_agent_route_registry_for_platforms(config_path, None)
}

/// Loads only the projection owned by the active Host platform.
///
/// Hook and lifecycle evaluation must not hydrate an unrelated Host's agent
/// artifacts. Full registry loading remains the publication and build gate.
pub fn load_agent_route_registry_for_platform(
    config_path: &Path,
    platform: &str,
) -> Result<AgentsRegistry, String> {
    load_agent_route_registry_for_platforms(config_path, Some(platform))
}

fn load_agent_route_registry_for_platforms(
    config_path: &Path,
    selected_platform: Option<&str>,
) -> Result<AgentsRegistry, String> {
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
    let mut compiled_platform_roles = BTreeMap::new();
    if let Some(platform) = selected_platform
        && !registry.platforms.contains_key(platform)
    {
        return Err(format!(
            "agent route registry does not define platform `{platform}`"
        ));
    }
    for (platform, platform_spec) in &registry.platforms {
        if selected_platform.is_some_and(|selected| selected != platform) {
            continue;
        }
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
            for role in &route.roles {
                let role_key = (platform.clone(), role.clone());
                compiled_platform_roles
                    .entry(role_key)
                    .or_insert_with(Vec::new)
                    .push(compiled.clone());
            }
            compiled_routes.insert((route_key.clone(), platform.clone()), compiled);
        }
    }
    for route_key in registry.agents.keys() {
        for platform in registry.platforms.keys() {
            if selected_platform.is_some_and(|selected| selected != platform) {
                continue;
            }
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
/// Parse and validate one route-registry source document.
pub fn parse_agent_route_registry(
    source: &str,
    source_label: &str,
) -> Result<AgentRouteRegistry, String> {
    let registry = toml::from_str::<AgentRouteRegistry>(source)
        .map_err(|error| format!("failed to parse {source_label}: {error}"))?;
    validate_agent_route_registry(&registry)?;
    Ok(registry)
}

/// Compile one agent route for an exact Host platform.
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
    let source = fs::read_to_string(projection_path)
        .map_err(|error| format!("failed to read {}: {error}", projection_path.display()))?;
    let (
        host_agent_name,
        description,
        model,
        sandbox_mode,
        definition_schema_id,
        effective_permissions,
    ) = match platform {
        "codex" => {
            let projection: toml::Value = toml::from_str(&source).map_err(|error| {
                format!("failed to parse {}: {error}", projection_path.display())
            })?;
            validate_codex_agent_projection(&projection, projection_path)?;
            let sandbox_mode = projection_string(&projection, "sandbox_mode", projection_path)?;
            (
                projection_string(&projection, "name", projection_path)?,
                projection_string(&projection, "description", projection_path)?,
                Some(projection_string(&projection, "model", projection_path)?),
                Some(HostSandboxMode::parse(&sandbox_mode)?),
                CODEX_AGENT_DEFINITION_SCHEMA_ID,
                if sandbox_mode == "read-only" {
                    EffectiveAgentPermissions::read_only()
                } else {
                    EffectiveAgentPermissions::default()
                },
            )
        }
        "claude" => {
            validate_claude_plugin_projection(&source, projection_path)?;
            let required_frontmatter = |field| {
                markdown_frontmatter_value(&source, field).ok_or_else(|| {
                    format!(
                        "Claude projection {} requires `{field}`",
                        projection_path.display()
                    )
                })
            };
            let tools = markdown_frontmatter_list(&source, "tools");
            let disallowed_tools = markdown_frontmatter_list(&source, "disallowedTools");
            let native_edit_is_available = ["Write", "Edit"].into_iter().any(|tool| {
                tools
                    .as_ref()
                    .is_none_or(|allowed| allowed.iter().any(|candidate| candidate == tool))
                    && !disallowed_tools
                        .iter()
                        .flatten()
                        .any(|candidate| candidate == tool)
            });
            (
                required_frontmatter("name")?,
                required_frontmatter("description")?,
                markdown_frontmatter_value(&source, "model"),
                None,
                ANTHROPIC_PLUGIN_AGENT_FRONTMATTER_SCHEMA_ID,
                if native_edit_is_available {
                    EffectiveAgentPermissions::default()
                } else {
                    EffectiveAgentPermissions::read_only()
                },
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
        allowed_rule_intents: agent.allowed_rule_intents.clone(),
        agent_kind: agent.agent_kind.clone(),
        display_role: agent.display_role.clone(),
        description: agent.description.clone().unwrap_or(description),
        platform: PlatformId(platform.to_string()),
        platform_host_agent_name: PlatformHostAgentName(host_agent_name),
        profile_path: projection_path.display().to_string(),
        model,
        sandbox_mode,
        definition_schema_id,
        effective_permissions,
    })
}

fn markdown_frontmatter_list(source: &str, key: &str) -> Option<Vec<String>> {
    markdown_frontmatter_value(source, key).map(|value| {
        value
            .trim_matches(['[', ']'])
            .split(',')
            .map(|item| item.trim().trim_matches(['\'', '"']).to_owned())
            .filter(|item| !item.is_empty())
            .collect()
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
        validate_string_set(
            &agent.allowed_rule_intents,
            agent_type,
            "allowed_rule_intents",
        )?;
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
