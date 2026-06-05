//! Project-local runtime profiles for activated language providers.

use crate::executable::{ExecutableStatus, is_executable_file, resolve_executable_with_status};
use crate::parse_activation;
use crate::protocol_activation::{ActivatedProvider, HookActivation, HookRuntime};
use crate::provider_manifest::provider_manifests;
use agent_semantic_runtime::{
    default_runtime_profiles_path as runtime_default_runtime_profiles_path,
    runtime_profiles_path_from_cache_home as runtime_profiles_path_from_cache_home_dir,
};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

/// Schema id for runtime profile files.
pub const RUNTIME_PROFILES_SCHEMA_ID: &str = "agent.semantic-protocols.runtime.profiles";
/// Schema version for runtime profile files.
pub const RUNTIME_PROFILES_SCHEMA_VERSION: &str = "1";
/// Protocol id for project-local ASP runtime profiles.
pub const RUNTIME_PROFILES_PROTOCOL_ID: &str = "agent.semantic-protocols.runtime";
/// Protocol version for project-local ASP runtime profiles.
pub const RUNTIME_PROFILES_PROTOCOL_VERSION: &str = "1";

/// Runtime profile registry generated for an activated project.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RuntimeProfiles {
    pub schema_id: String,
    pub schema_version: String,
    pub protocol_id: String,
    pub protocol_version: String,
    pub project_root: String,
    pub runtime_home: String,
    pub generated_by: RuntimeProfilesGeneratedBy,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub generated_at: Option<String>,
    pub providers: Vec<RuntimeProviderProfile>,
}

/// Runtime and version that generated a runtime profile file.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RuntimeProfilesGeneratedBy {
    pub runtime: String,
    pub version: String,
}

/// Executable argv and health for one activated language provider.
///
/// Raw DTO boundary: runtime profile JSON keeps primitive transport fields
/// that `runtime_profile_command_argv` validates before provider execution.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RuntimeProviderProfile {
    pub manifest_id: String,
    pub manifest_digest: String,
    pub language_id: String,
    pub provider_id: String,
    pub binary: String,
    #[serde(default)]
    pub provider_command_prefix: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resolved_binary: Option<String>,
    #[serde(default)]
    pub argv: Vec<String>,
    pub health: RuntimeProviderHealth,
}

/// Health information for one resolved provider command.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RuntimeProviderHealth {
    pub status: RuntimeProviderHealthStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub checked_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

/// Runtime profile health status for a provider command.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum RuntimeProviderHealthStatus {
    /// Provider command was found and is executable.
    Available,
    /// Provider command was not found.
    Missing,
    /// Provider command exists but is not executable.
    Unexecutable,
}

impl From<ExecutableStatus> for RuntimeProviderHealthStatus {
    fn from(status: ExecutableStatus) -> Self {
        match status {
            ExecutableStatus::Available => Self::Available,
            ExecutableStatus::Missing => Self::Missing,
            ExecutableStatus::Unexecutable => Self::Unexecutable,
        }
    }
}

/// Resolve the default runtime profiles path for a project root.
pub fn default_runtime_profiles_path(project_root: impl AsRef<Path>) -> Result<PathBuf, String> {
    runtime_default_runtime_profiles_path(project_root)
}

/// Resolve the runtime profiles path below a known state cache home.
pub fn runtime_profiles_path_from_cache_home(cache_home: impl AsRef<Path>) -> PathBuf {
    runtime_profiles_path_from_cache_home_dir(cache_home)
}

/// Resolve the runtime profiles path beside an activation file.
pub fn runtime_profiles_path_for_activation(activation_path: &Path) -> PathBuf {
    if is_generated_activation_path(activation_path) {
        return runtime_profiles_path_from_cache_home(
            activation_storage_root(activation_path).join(".cache"),
        );
    }
    runtime_profiles_path_from_cache_home(
        activation_path
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join(".cache"),
    )
}

/// Resolve the project root recorded by an activation path and activation file.
pub fn runtime_project_root_for_activation(
    activation_path: &Path,
    activation_project_root: &str,
) -> PathBuf {
    let project_root = PathBuf::from(activation_project_root);
    if project_root.is_absolute() {
        return project_root;
    }
    if is_generated_activation_path(activation_path) {
        return activation_storage_root(activation_path).join(project_root);
    }
    activation_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join(project_root)
}

/// Load and validate a runtime profiles file.
pub fn load_runtime_profiles(path: &Path) -> Result<RuntimeProfiles, String> {
    let contents = fs::read_to_string(path).map_err(|error| {
        format!(
            "failed to read runtime profiles {}: {error}",
            path.display()
        )
    })?;
    let profiles: RuntimeProfiles = serde_json::from_str(&contents)
        .map_err(|error| format!("invalid runtime profiles JSON {}: {error}", path.display()))?;
    validate_runtime_profiles_protocol(&profiles)?;
    Ok(profiles)
}

/// Load an existing profile file or refresh it when provider commands drift.
pub fn load_or_refresh_runtime_profiles(
    path: &Path,
    project_root: &Path,
    runtime: &HookRuntime,
) -> Result<RuntimeProfiles, String> {
    if let Ok(profiles) = load_runtime_profiles(path)
        && profiles_match_project_root(&profiles, project_root)
        && profiles_match_runtime(&profiles, runtime)
        && profiles_have_usable_commands(&profiles, runtime)
    {
        return Ok(profiles);
    }
    write_runtime_profiles_for_runtime(path, project_root, runtime)
}

/// Write runtime profiles from an activation file model.
pub fn write_runtime_profiles_for_activation(
    path: &Path,
    project_root: &Path,
    activation: &HookActivation,
) -> Result<RuntimeProfiles, String> {
    let contents = serde_json::to_string(activation)
        .map_err(|error| format!("failed to serialize activation for runtime profiles: {error}"))?;
    let runtime = parse_activation(&contents, &provider_manifests())
        .map_err(|error| format!("failed to resolve activation for runtime profiles: {error:?}"))?;
    write_runtime_profiles_for_runtime(path, project_root, &runtime)
}

/// Write runtime profiles for a parsed hook runtime.
pub fn write_runtime_profiles_for_runtime(
    path: &Path,
    project_root: &Path,
    runtime: &HookRuntime,
) -> Result<RuntimeProfiles, String> {
    let runtime_home = path.parent().unwrap_or_else(|| Path::new("."));
    let profiles = build_runtime_profiles(project_root, runtime_home, runtime);
    write_runtime_profiles(path, &profiles)?;
    Ok(profiles)
}

/// Serialize runtime profiles to disk.
pub fn write_runtime_profiles(path: &Path, profiles: &RuntimeProfiles) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("failed to create {}: {error}", parent.display()))?;
    }
    let output = serde_json::to_string_pretty(profiles)
        .map_err(|error| format!("failed to serialize runtime profiles: {error}"))?;
    fs::write(path, format!("{}\n", output.trim_end()))
        .map_err(|error| format!("failed to write {}: {error}", path.display()))
}

/// Return the stored executable argv for an available provider profile.
pub fn runtime_profile_command_argv(
    profiles: &RuntimeProfiles,
    provider: &ActivatedProvider,
) -> Option<Vec<String>> {
    let profile = runtime_provider_profile(profiles, provider)?;
    if profile.health.status != RuntimeProviderHealthStatus::Available || profile.argv.is_empty() {
        return None;
    }
    is_executable_file(Path::new(&profile.argv[0])).then(|| profile.argv.clone())
}

/// Build an invocation by appending request args to a provider profile argv.
pub fn runtime_profile_invocation(
    profiles: &RuntimeProfiles,
    provider: &ActivatedProvider,
    args: &[String],
) -> Option<Vec<String>> {
    let mut invocation = runtime_profile_command_argv(profiles, provider)?;
    invocation.extend(args.iter().cloned());
    Some(invocation)
}

fn build_runtime_profiles(
    project_root: &Path,
    runtime_home: &Path,
    runtime: &HookRuntime,
) -> RuntimeProfiles {
    RuntimeProfiles {
        schema_id: RUNTIME_PROFILES_SCHEMA_ID.to_string(),
        schema_version: RUNTIME_PROFILES_SCHEMA_VERSION.to_string(),
        protocol_id: RUNTIME_PROFILES_PROTOCOL_ID.to_string(),
        protocol_version: RUNTIME_PROFILES_PROTOCOL_VERSION.to_string(),
        project_root: project_root.display().to_string(),
        runtime_home: runtime_home.display().to_string(),
        generated_by: RuntimeProfilesGeneratedBy {
            runtime: "asp".to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
        },
        generated_at: None,
        providers: runtime
            .providers
            .iter()
            .map(|provider| runtime_provider_profile_for_provider(project_root, provider))
            .collect(),
    }
}

fn runtime_provider_profile_for_provider(
    project_root: &Path,
    provider: &ActivatedProvider,
) -> RuntimeProviderProfile {
    let project_bin = project_root.join(".bin").join(&provider.binary);
    let binary_resolution = if is_executable_file(&project_bin) {
        crate::executable::ExecutableResolution {
            path: Some(fs::canonicalize(&project_bin).unwrap_or(project_bin)),
            status: ExecutableStatus::Available,
            reason: None,
        }
    } else {
        resolve_executable_with_status(&provider.binary)
    };
    let command = runtime_provider_command(provider, binary_resolution.path.as_ref());
    let resolved_binary = command.argv.first().cloned().or_else(|| {
        binary_resolution
            .path
            .as_ref()
            .map(|path| path.display().to_string())
    });
    let health = RuntimeProviderHealth {
        status: command
            .status
            .unwrap_or_else(|| binary_resolution.status.into()),
        checked_at: None,
        reason: command.reason.or(binary_resolution.reason),
    };
    RuntimeProviderProfile {
        manifest_id: provider.manifest_id.clone(),
        manifest_digest: provider.manifest_digest.clone(),
        language_id: provider.language_id.clone(),
        provider_id: provider.provider_id.clone(),
        binary: provider.binary.clone(),
        provider_command_prefix: provider.provider_command_prefix.clone(),
        resolved_binary,
        argv: command.argv,
        health,
    }
}

struct RuntimeProviderCommand {
    argv: Vec<String>,
    status: Option<RuntimeProviderHealthStatus>,
    reason: Option<String>,
}

fn runtime_provider_command(
    provider: &ActivatedProvider,
    resolved_binary: Option<&PathBuf>,
) -> RuntimeProviderCommand {
    if provider.provider_command_prefix.is_empty() {
        return match resolved_binary {
            Some(binary) => RuntimeProviderCommand {
                argv: vec![binary.display().to_string()],
                status: Some(RuntimeProviderHealthStatus::Available),
                reason: None,
            },
            None => RuntimeProviderCommand {
                argv: Vec::new(),
                status: None,
                reason: None,
            },
        };
    }

    let Some((program, forwarded)) = provider.provider_command_prefix.split_first() else {
        return RuntimeProviderCommand {
            argv: Vec::new(),
            status: Some(RuntimeProviderHealthStatus::Missing),
            reason: Some("provider command prefix is empty".to_string()),
        };
    };
    let program_resolution = resolve_executable_with_status(program);
    let Some(program_path) = program_resolution.path else {
        return RuntimeProviderCommand {
            argv: Vec::new(),
            status: Some(program_resolution.status.into()),
            reason: program_resolution.reason,
        };
    };

    let argv = std::iter::once(program_path.display().to_string())
        .chain(forwarded.iter().cloned())
        .collect();
    RuntimeProviderCommand {
        argv,
        status: Some(RuntimeProviderHealthStatus::Available),
        reason: None,
    }
}

fn validate_runtime_profiles_protocol(profiles: &RuntimeProfiles) -> Result<(), String> {
    expect_field("schemaId", &profiles.schema_id, RUNTIME_PROFILES_SCHEMA_ID)?;
    expect_field(
        "schemaVersion",
        &profiles.schema_version,
        RUNTIME_PROFILES_SCHEMA_VERSION,
    )?;
    expect_field(
        "protocolId",
        &profiles.protocol_id,
        RUNTIME_PROFILES_PROTOCOL_ID,
    )?;
    expect_field(
        "protocolVersion",
        &profiles.protocol_version,
        RUNTIME_PROFILES_PROTOCOL_VERSION,
    )?;
    if profiles.generated_by.runtime != "asp" {
        return Err(format!(
            "invalid runtime profile generatedBy.runtime: expected asp, got {}",
            profiles.generated_by.runtime
        ));
    }
    Ok(())
}

fn expect_field(name: &str, actual: &str, expected: &str) -> Result<(), String> {
    if actual == expected {
        Ok(())
    } else {
        Err(format!(
            "invalid runtime profile {name}: expected {expected}, got {actual}"
        ))
    }
}

fn profiles_match_runtime(profiles: &RuntimeProfiles, runtime: &HookRuntime) -> bool {
    runtime.providers.iter().all(|provider| {
        runtime_provider_profile(profiles, provider).is_some_and(|profile| {
            profile.manifest_digest == provider.manifest_digest
                && profile.provider_command_prefix == provider.provider_command_prefix
        })
    })
}

fn profiles_match_project_root(profiles: &RuntimeProfiles, project_root: &Path) -> bool {
    paths_match(Path::new(&profiles.project_root), project_root)
}

fn paths_match(left: &Path, right: &Path) -> bool {
    match (fs::canonicalize(left), fs::canonicalize(right)) {
        (Ok(left), Ok(right)) => left == right,
        _ => left == right,
    }
}

fn profiles_have_usable_commands(profiles: &RuntimeProfiles, runtime: &HookRuntime) -> bool {
    runtime
        .providers
        .iter()
        .all(|provider| runtime_profile_command_argv(profiles, provider).is_some())
}

fn runtime_provider_profile<'a>(
    profiles: &'a RuntimeProfiles,
    provider: &ActivatedProvider,
) -> Option<&'a RuntimeProviderProfile> {
    profiles.providers.iter().find(|profile| {
        profile.manifest_id == provider.manifest_id
            && profile.language_id == provider.language_id
            && profile.provider_id == provider.provider_id
            && profile.binary == provider.binary
    })
}

fn is_generated_activation_path(path: &Path) -> bool {
    let normalized = path.to_string_lossy().replace('\\', "/");
    normalized.ends_with(".cache/agent-semantic-protocol/hooks/activation.json")
}

fn activation_storage_root(activation_path: &Path) -> PathBuf {
    activation_path
        .parent()
        .and_then(Path::parent)
        .and_then(Path::parent)
        .and_then(Path::parent)
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."))
}

#[cfg(test)]
#[path = "../tests/unit/runtime_profile.rs"]
mod runtime_profile_tests;
