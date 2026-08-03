use crate::codex_agent_projection::render_codex_agent_profile;
use serde::Deserialize;
use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

pub const SUBAGENT_CATALOG_SCHEMA_ID: &str = "asp.subagent-catalog.v1";
pub const SUBAGENT_CATALOG_SCHEMA_VERSION: u64 = 1;

static PUBLICATION_TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct SubagentCatalog {
    pub schema_id: String,
    pub schema_version: u64,
    pub agents: BTreeMap<String, CanonicalAgentSpec>,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct CanonicalAgentSpec {
    pub session_name: String,
    pub session_lifetime: String,
    pub model_policy: String,
    pub reasoning_policy: String,
    pub permission_policy: String,
    pub sandbox_policy: String,
    pub roles: Vec<String>,
    pub capabilities: Vec<String>,
    pub platforms: BTreeMap<String, PlatformAgentProjectionSpec>,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct PlatformAgentProjectionSpec {
    pub host_agent_name: String,
    pub profile: String,
    pub projection: String,
    pub model: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoadedSubagentCatalog {
    pub source_dir: PathBuf,
    pub catalog: SubagentCatalog,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompiledSubagentProjection {
    pub agent_id: String,
    pub platform: String,
    pub host_agent_name: String,
    pub profile: String,
    pub projection: String,
    pub model_policy: String,
    pub resolved_model: Option<String>,
    pub content: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PublishedSubagentProjection {
    pub agent_id: String,
    pub platform: String,
    pub target: PathBuf,
    pub content_changed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubagentCatalogPublication {
    pub catalog_target: PathBuf,
    pub catalog_changed: bool,
    pub projections: Vec<PublishedSubagentProjection>,
}

pub fn load_subagent_catalog(config_path: &Path) -> Result<LoadedSubagentCatalog, String> {
    let source_dir = config_path
        .parent()
        .ok_or_else(|| format!("{} has no parent directory", config_path.display()))?;
    let text = fs::read_to_string(config_path)
        .map_err(|error| format!("failed to read {}: {error}", config_path.display()))?;
    let catalog = toml::from_str::<SubagentCatalog>(&text)
        .map_err(|error| format!("failed to parse {}: {error}", config_path.display()))?;
    validate_subagent_catalog(source_dir, &catalog)?;
    Ok(LoadedSubagentCatalog {
        source_dir: source_dir.to_path_buf(),
        catalog,
    })
}

pub fn compile_subagent_projection(
    loaded: &LoadedSubagentCatalog,
    agent_id: &str,
    platform: &str,
    state_root: &Path,
) -> Result<CompiledSubagentProjection, String> {
    let agent = loaded
        .catalog
        .agents
        .get(agent_id)
        .ok_or_else(|| format!("subagent catalog does not define agent `{agent_id}`"))?;
    let platform_spec = agent
        .platforms
        .get(platform)
        .ok_or_else(|| format!("subagent `{agent_id}` does not define platform `{platform}`"))?;
    let template_path = loaded.source_dir.join(&platform_spec.profile);
    let template = fs::read_to_string(&template_path)
        .map_err(|error| format!("failed to read {}: {error}", template_path.display()))?;
    let (content, resolved_model) = match platform {
        "codex" => {
            let model = platform_spec.model.as_deref().ok_or_else(|| {
                format!("subagent `{agent_id}` Codex projection requires an explicit model")
            })?;
            (
                render_codex_agent_profile(&template, model, state_root).map_err(|error| {
                    format!("failed to render Codex subagent `{agent_id}`: {error}")
                })?,
                Some(model.to_string()),
            )
        }
        "claude" => (template, platform_spec.model.clone()),
        _ => {
            return Err(format!(
                "subagent manager has no renderer for platform `{platform}`"
            ));
        }
    };
    Ok(CompiledSubagentProjection {
        agent_id: agent_id.to_string(),
        platform: platform.to_string(),
        host_agent_name: platform_spec.host_agent_name.clone(),
        profile: platform_spec.profile.clone(),
        projection: platform_spec.projection.clone(),
        model_policy: agent.model_policy.clone(),
        resolved_model,
        content,
    })
}

pub fn publish_subagent_catalog(
    config_path: &Path,
    state_source_dir: &Path,
    state_root: &Path,
) -> Result<SubagentCatalogPublication, String> {
    let loaded = load_subagent_catalog(config_path)?;
    fs::create_dir_all(state_source_dir).map_err(|error| {
        format!(
            "failed to create subagent publication directory {}: {error}",
            state_source_dir.display()
        )
    })?;
    let catalog_bytes = fs::read(config_path)
        .map_err(|error| format!("failed to read {}: {error}", config_path.display()))?;
    let catalog_target = state_source_dir.join("config.toml");
    let catalog_changed = write_atomic_if_changed(&catalog_target, &catalog_bytes)?;

    let mut projections = Vec::new();
    for (agent_id, agent) in &loaded.catalog.agents {
        for platform in agent.platforms.keys() {
            let compiled = compile_subagent_projection(&loaded, agent_id, platform, state_root)?;
            let target = state_source_dir.join(&compiled.profile);
            let content_changed = write_atomic_if_changed(&target, compiled.content.as_bytes())?;
            projections.push(PublishedSubagentProjection {
                agent_id: agent_id.clone(),
                platform: platform.clone(),
                target,
                content_changed,
            });
        }
    }
    Ok(SubagentCatalogPublication {
        catalog_target,
        catalog_changed,
        projections,
    })
}

fn validate_subagent_catalog(source_dir: &Path, catalog: &SubagentCatalog) -> Result<(), String> {
    if catalog.schema_id != SUBAGENT_CATALOG_SCHEMA_ID {
        return Err(format!(
            "unsupported subagent catalog schema_id `{}`",
            catalog.schema_id
        ));
    }
    if catalog.schema_version != SUBAGENT_CATALOG_SCHEMA_VERSION {
        return Err(format!(
            "unsupported subagent catalog schema_version `{}`",
            catalog.schema_version
        ));
    }
    if catalog.agents.is_empty() {
        return Err("subagent catalog must define at least one agent".to_string());
    }
    let mut published_profiles = BTreeSet::new();
    let mut published_projections = BTreeSet::new();
    for (agent_id, agent) in &catalog.agents {
        validate_identifier(agent_id, "agent id")?;
        validate_nonempty(&agent.session_name, agent_id, "session_name")?;
        validate_nonempty(&agent.session_lifetime, agent_id, "session_lifetime")?;
        validate_nonempty(&agent.model_policy, agent_id, "model_policy")?;
        validate_nonempty(&agent.reasoning_policy, agent_id, "reasoning_policy")?;
        validate_nonempty(&agent.permission_policy, agent_id, "permission_policy")?;
        validate_nonempty(&agent.sandbox_policy, agent_id, "sandbox_policy")?;
        validate_string_set(&agent.roles, agent_id, "roles")?;
        validate_string_set(&agent.capabilities, agent_id, "capabilities")?;
        if agent.platforms.is_empty() {
            return Err(format!(
                "subagent `{agent_id}` must define at least one platform"
            ));
        }
        for (platform, platform_spec) in &agent.platforms {
            validate_identifier(platform, "platform id")?;
            validate_nonempty(
                &platform_spec.host_agent_name,
                agent_id,
                "platform.host_agent_name",
            )?;
            validate_file_name(&platform_spec.profile, "profile")?;
            validate_file_name(&platform_spec.projection, "projection")?;
            if !published_profiles.insert(platform_spec.profile.clone()) {
                return Err(format!(
                    "duplicate subagent profile `{}`",
                    platform_spec.profile
                ));
            }
            let projection_identity = (platform.clone(), platform_spec.projection.clone());
            if !published_projections.insert(projection_identity) {
                return Err(format!(
                    "duplicate subagent projection `{}` for platform `{platform}`",
                    platform_spec.projection
                ));
            }
            let profile_path = source_dir.join(&platform_spec.profile);
            if !profile_path.is_file() {
                return Err(format!(
                    "subagent `{agent_id}` platform `{platform}` profile does not exist: {}",
                    profile_path.display()
                ));
            }
            if platform == "codex"
                && platform_spec
                    .model
                    .as_deref()
                    .is_none_or(|model| model.trim().is_empty())
            {
                return Err(format!(
                    "subagent `{agent_id}` Codex projection requires an explicit model"
                ));
            }
        }
    }
    Ok(())
}

fn validate_identifier(value: &str, label: &str) -> Result<(), String> {
    if value.is_empty()
        || !value
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-'))
    {
        return Err(format!("invalid {label} `{value}`"));
    }
    Ok(())
}

fn validate_nonempty(value: &str, agent_id: &str, field: &str) -> Result<(), String> {
    if value.trim().is_empty() {
        return Err(format!(
            "subagent `{agent_id}` field `{field}` must not be empty"
        ));
    }
    Ok(())
}

fn validate_string_set(values: &[String], agent_id: &str, field: &str) -> Result<(), String> {
    if values.is_empty() || values.iter().any(|value| value.trim().is_empty()) {
        return Err(format!(
            "subagent `{agent_id}` field `{field}` must contain non-empty values"
        ));
    }
    let mut sorted = values.to_vec();
    sorted.sort();
    sorted.dedup();
    if sorted.len() != values.len() {
        return Err(format!(
            "subagent `{agent_id}` field `{field}` must not contain duplicates"
        ));
    }
    Ok(())
}

fn validate_file_name(value: &str, label: &str) -> Result<(), String> {
    let mut components = Path::new(value).components();
    let valid =
        matches!(components.next(), Some(Component::Normal(_))) && components.next().is_none();
    if !valid {
        return Err(format!("invalid subagent {label} `{value}`"));
    }
    Ok(())
}

fn write_atomic_if_changed(target: &Path, bytes: &[u8]) -> Result<bool, String> {
    if fs::read(target)
        .map(|existing| existing == bytes)
        .unwrap_or(false)
    {
        return Ok(false);
    }
    let parent = target
        .parent()
        .ok_or_else(|| format!("{} has no parent directory", target.display()))?;
    fs::create_dir_all(parent)
        .map_err(|error| format!("failed to create {}: {error}", parent.display()))?;
    let file_name = target
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| format!("{} has no UTF-8 file name", target.display()))?;
    let temporary = parent.join(format!(
        ".{file_name}.asp-publish-{}-{}",
        std::process::id(),
        PUBLICATION_TEMP_COUNTER.fetch_add(1, Ordering::Relaxed)
    ));
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary)
        .map_err(|error| {
            format!(
                "failed to create staged subagent projection {}: {error}",
                temporary.display()
            )
        })?;
    if let Err(error) = file.write_all(bytes).and_then(|()| file.sync_all()) {
        let _ = fs::remove_file(&temporary);
        return Err(format!(
            "failed to stage subagent projection {}: {error}",
            temporary.display()
        ));
    }
    drop(file);
    if let Err(error) = fs::rename(&temporary, target) {
        let _ = fs::remove_file(&temporary);
        return Err(format!(
            "failed to publish subagent projection {} to {}: {error}",
            temporary.display(),
            target.display()
        ));
    }
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn canonical_catalog_path() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../agents/config.toml")
    }

    #[test]
    fn canonical_catalog_compiles_codex_and_claude_from_one_agent_spec() {
        let loaded = load_subagent_catalog(&canonical_catalog_path()).expect("canonical catalog");
        let state_root = Path::new("/tmp/asp-subagent-manager-state");
        let codex = compile_subagent_projection(&loaded, "asp_explorer", "codex", state_root)
            .expect("Codex projection");
        let claude = compile_subagent_projection(&loaded, "asp_explorer", "claude", state_root)
            .expect("Claude projection");

        assert_eq!(codex.agent_id, claude.agent_id);
        assert_eq!(codex.model_policy, claude.model_policy);
        assert_eq!(codex.resolved_model.as_deref(), Some("gpt-5.6-luna"));
        assert!(codex.content.contains("model = \"gpt-5.6-luna\""));
        assert!(!codex.content.contains("{{MODEL_TOML}}"));
        assert!(!codex.content.contains("gpt-5.4-mini"));
        assert!(!claude.content.trim().is_empty());
    }

    #[test]
    fn canonical_catalog_compilation_is_deterministic() {
        let loaded = load_subagent_catalog(&canonical_catalog_path()).expect("canonical catalog");
        let state_root = Path::new("/tmp/asp-subagent-manager-state");
        let first = compile_subagent_projection(&loaded, "asp_testing", "codex", state_root)
            .expect("first projection");
        let second = compile_subagent_projection(&loaded, "asp_testing", "codex", state_root)
            .expect("second projection");
        assert_eq!(first, second);
    }

    #[test]
    fn canonical_catalog_publication_is_atomic_and_idempotent() {
        let temp = tempfile::tempdir().expect("tempdir");
        let state_root = temp.path().join("state");
        let state_agents = state_root.join("agents");
        let first = publish_subagent_catalog(&canonical_catalog_path(), &state_agents, &state_root)
            .expect("first publication");
        assert!(first.catalog_changed);
        assert_eq!(first.projections.len(), 4);
        assert!(
            first
                .projections
                .iter()
                .all(|projection| projection.content_changed)
        );
        let explorer = fs::read_to_string(state_agents.join("asp-explorer_codex.toml"))
            .expect("published explorer");
        assert!(explorer.contains("model = \"gpt-5.6-luna\""));
        assert!(!explorer.contains("{{MODEL_TOML}}"));

        let second =
            publish_subagent_catalog(&canonical_catalog_path(), &state_agents, &state_root)
                .expect("second publication");
        assert!(!second.catalog_changed);
        assert!(
            second
                .projections
                .iter()
                .all(|projection| !projection.content_changed)
        );
    }

    #[test]
    fn unregistered_platform_is_rejected() {
        let loaded = load_subagent_catalog(&canonical_catalog_path()).expect("canonical catalog");
        let error = compile_subagent_projection(
            &loaded,
            "asp_explorer",
            "unknown-host",
            Path::new("/tmp/asp-subagent-manager-state"),
        )
        .expect_err("unknown platform must fail closed");
        assert!(error.contains("does not define platform `unknown-host`"));
    }

    #[test]
    fn profile_path_traversal_is_rejected() {
        let temp = tempfile::tempdir().expect("tempdir");
        fs::write(
            temp.path().join("config.toml"),
            r#"schema_id = "asp.subagent-catalog.v1"
schema_version = 1

[agents.bad]
session_name = "bad"
session_lifetime = "resident"
model_policy = "low-cost"
reasoning_policy = "low"
permission_policy = "read-only"
sandbox_policy = "read-only"
roles = ["subagent"]
capabilities = ["search"]

[agents.bad.platforms.codex]
host_agent_name = "bad"
profile = "../bad.toml"
projection = "bad.toml"
model = "test-model"
"#,
        )
        .expect("write invalid catalog");
        let error = load_subagent_catalog(&temp.path().join("config.toml"))
            .expect_err("path traversal must fail closed");
        assert!(error.contains("invalid subagent profile"));
    }
}
