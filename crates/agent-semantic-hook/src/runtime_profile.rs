//! Runtime provider command profiles derived from activation.

use crate::executable::{ExecutableStatus, is_executable_file, resolve_executable_with_status};
use crate::protocol_activation::protocol_activation_manifest::{
    ActivatedProvider, HookActivation, HookRuntime,
};
use crate::protocol_activation::protocol_activation_runtime::parse_activation;
use crate::provider_manifest::provider_manifests;
use agent_semantic_runtime::{is_project_activation_path, project_root_for_activation_path};
use serde::{Deserialize, Serialize};
use std::env;
use std::path::{Path, PathBuf};

/// Schema id for runtime provider profiles.
pub const RUNTIME_PROFILES_SCHEMA_ID: &str = "agent.semantic-protocols.runtime.profiles";
/// Schema version for runtime provider profiles.
pub const RUNTIME_PROFILES_SCHEMA_VERSION: &str = "1";
/// Protocol id for activation-derived ASP runtime profiles.
pub const RUNTIME_PROFILES_PROTOCOL_ID: &str = "agent.semantic-protocols.runtime";
/// Protocol version for activation-derived ASP runtime profiles.
pub const RUNTIME_PROFILES_PROTOCOL_VERSION: &str = "1";

/// Runtime profile registry derived from an activated project.
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

/// Runtime and version that generated runtime provider profiles.
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
    pub execution: crate::protocol_activation::protocol_activation_manifest::ProviderExecution,
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

/// Resolve the project root recorded by an activation path and activation file.
pub fn runtime_project_root_for_activation(
    activation_path: &Path,
    activation_project_root: &str,
) -> PathBuf {
    let project_root = PathBuf::from(activation_project_root);
    if project_root.is_absolute() {
        return project_root;
    }
    if is_project_activation_path(activation_path)
        && let Some(root) = project_root_for_activation_path(activation_path)
    {
        return root.join(project_root);
    }
    activation_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join(project_root)
}

/// Build runtime profiles from an activation file model.
pub fn runtime_profiles_for_activation(
    project_root: &Path,
    activation: &HookActivation,
) -> Result<RuntimeProfiles, String> {
    let contents = serde_json::to_string(activation)
        .map_err(|error| format!("failed to serialize activation for runtime profiles: {error}"))?;
    let runtime = parse_activation(&contents, &provider_manifests())
        .map_err(|error| format!("failed to resolve activation for runtime profiles: {error:?}"))?;
    Ok(runtime_profiles_for_runtime(project_root, &runtime))
}

/// Build runtime profiles for a parsed hook runtime.
#[must_use]
pub fn runtime_profiles_for_runtime(project_root: &Path, runtime: &HookRuntime) -> RuntimeProfiles {
    let runtime_home = project_root;
    build_runtime_profiles(project_root, runtime_home, runtime)
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
    let binary_resolution =
        if let Some(home_binary) = preferred_home_local_provider_binary(provider) {
            crate::executable::ExecutableResolution {
                path: Some(home_binary),
                status: ExecutableStatus::Available,
                reason: None,
            }
        } else if is_executable_file(&project_bin) {
            crate::executable::ExecutableResolution {
                path: Some(std::fs::canonicalize(&project_bin).unwrap_or(project_bin)),
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
        execution: provider.execution,
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
    if provider.provider_command_prefix.is_empty()
        || provider_prefers_resolved_binary_over_prefix(provider)
    {
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

fn provider_prefers_resolved_binary_over_prefix(provider: &ActivatedProvider) -> bool {
    provider_is_gerbil_scheme(provider)
}

fn preferred_home_local_provider_binary(provider: &ActivatedProvider) -> Option<PathBuf> {
    if !provider_is_gerbil_scheme(provider) {
        return None;
    }
    let candidate = env::var_os("HOME")
        .filter(|home| !home.is_empty())
        .map(PathBuf::from)?
        .join(".local/bin/gslph");
    is_executable_file(&candidate).then(|| std::fs::canonicalize(&candidate).unwrap_or(candidate))
}

fn provider_is_gerbil_scheme(provider: &ActivatedProvider) -> bool {
    provider.language_id == "gerbil-scheme"
        || provider
            .provider_command_prefix
            .iter()
            .any(|arg| arg == "gerbil-scheme")
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

#[cfg(test)]
#[path = "../tests/unit/runtime_profile.rs"]
mod runtime_profile_tests;
