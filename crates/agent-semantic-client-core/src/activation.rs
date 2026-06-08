//! Provider activation snapshot loading for `agent-semantic-client`.

use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use agent_semantic_config::project_hook_state_dir;
use agent_semantic_hook::{
    ActivatedProvider, HookRuntime, ProviderExecution, RuntimeProviderHealthStatus,
    builtin_provider_manifests, load_or_sync_activation, parse_activation,
    runtime_profile_command_argv, runtime_profiles_for_runtime,
    runtime_project_root_for_activation,
};

use crate::receipt::NativeProvenance;
use crate::types::{LanguageId, ProviderId};

/// Explicit activation.json path used when a protocol facade already resolved
/// the hook installation root separately from the provider execution root.
pub const ASP_PROVIDER_ACTIVATION_PATH_ENV: &str = "ASP_PROVIDER_ACTIVATION_PATH";

/// Provider resolved from the project hook activation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedProvider {
    pub language_id: LanguageId,
    pub provider_id: ProviderId,
    pub binary: String,
    pub execution: ProviderExecution,
    pub provider_command_prefix: Vec<String>,
    pub runtime_command_argv: Option<Vec<String>>,
    pub runtime_profile_status: Option<RuntimeProfileStatus>,
    pub package_roots: Vec<String>,
}

/// Health status copied from the provider runtime profile.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimeProfileStatus {
    /// Runtime profile command is available.
    Available,
    /// Runtime profile command is missing.
    Missing,
    /// Runtime profile command exists but is not executable.
    Unexecutable,
}

impl RuntimeProfileStatus {
    /// Return the receipt label for this runtime profile status.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Available => "available",
            Self::Missing => "missing",
            Self::Unexecutable => "unexecutable",
        }
    }
}

impl From<RuntimeProviderHealthStatus> for RuntimeProfileStatus {
    fn from(status: RuntimeProviderHealthStatus) -> Self {
        match status {
            RuntimeProviderHealthStatus::Available => Self::Available,
            RuntimeProviderHealthStatus::Missing => Self::Missing,
            RuntimeProviderHealthStatus::Unexecutable => Self::Unexecutable,
        }
    }
}

impl ResolvedProvider {
    /// Convert provider activation data into receipt provenance.
    #[must_use]
    pub fn provenance(&self) -> NativeProvenance {
        NativeProvenance {
            language_id: self.language_id.clone(),
            provider_id: self.provider_id.clone(),
            provider_binary: self.binary.clone(),
        }
    }

    /// Return the registry-owned provider command prefix.
    #[must_use]
    pub fn command_prefix(&self) -> Vec<String> {
        if self.provider_command_prefix.is_empty() {
            vec![self.binary.clone()]
        } else {
            self.provider_command_prefix.clone()
        }
    }

    /// Return the profile-pinned provider argv when runtime profile health is usable.
    #[must_use]
    pub fn runtime_command_prefix(&self) -> Option<Vec<String>> {
        if !self.provider_command_prefix.is_empty() {
            return None;
        }
        self.runtime_command_argv.clone()
    }
}

impl From<&ActivatedProvider> for ResolvedProvider {
    fn from(provider: &ActivatedProvider) -> Self {
        Self {
            language_id: provider.language_id.clone().into(),
            provider_id: provider.provider_id.clone().into(),
            binary: provider.binary.clone(),
            execution: provider.execution,
            provider_command_prefix: provider.provider_command_prefix.clone(),
            runtime_command_argv: None,
            runtime_profile_status: None,
            package_roots: provider.package_roots.clone(),
        }
    }
}

/// Snapshot of activated providers for one project root.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProviderRegistrySnapshot {
    pub activation_path: PathBuf,
    pub providers: Vec<ResolvedProvider>,
}

impl ProviderRegistrySnapshot {
    pub fn load(project_root: &Path) -> Result<Self, String> {
        if let Some(activation_path) = env::var_os(ASP_PROVIDER_ACTIVATION_PATH_ENV) {
            let activation_path = PathBuf::from(activation_path);
            if activation_path.is_file() {
                return Self::load_from_path(&activation_path);
            }
        }
        let direct_activation_path = match project_hook_state_dir(project_root) {
            Ok(hook_state_dir) => Some(hook_state_dir.join("activation.json")),
            Err(error) => {
                let mut current = Some(project_root);
                while let Some(candidate_root) = current {
                    let candidate =
                        candidate_root.join(".cache/agent-semantic-protocol/hooks/activation.json");
                    if candidate.is_file() {
                        return Self::load_from_path_for_project(&candidate, candidate_root);
                    }
                    current = candidate_root.parent();
                }
                return Err(error.to_string());
            }
        };

        if let Some(activation_path) = direct_activation_path {
            if activation_path.is_file() {
                return Self::load_from_path_for_project(&activation_path, project_root);
            }
            if project_root_has_provider_identity(project_root) {
                return Self::load_from_path_for_project(&activation_path, project_root);
            }
            let mut current = project_root.parent();
            while let Some(candidate_root) = current {
                let candidate =
                    candidate_root.join(".cache/agent-semantic-protocol/hooks/activation.json");
                if candidate.is_file() {
                    return Self::load_from_path_for_project(&candidate, candidate_root);
                }
                current = candidate_root.parent();
            }
            return Self::load_from_path_for_project(&activation_path, project_root);
        }

        Err("provider activation path could not be resolved".to_string())
    }

    /// Load provider activation from an explicit `activation.json` path.
    pub fn load_from_path(activation_path: &Path) -> Result<Self, String> {
        let text = fs::read_to_string(activation_path).map_err(|error| {
            format!(
                "failed to read agent semantic client provider activation at {}: {error}",
                activation_path.display()
            )
        })?;
        let manifests = builtin_provider_manifests();
        let activation =
            parse_activation(&text, &manifests).map_err(|error| format!("{error:?}"))?;
        Self::from_activation(activation_path, &activation)
    }

    fn load_from_path_for_project(
        activation_path: &Path,
        project_root: &Path,
    ) -> Result<Self, String> {
        let activation = load_or_sync_activation(activation_path, project_root)?;
        Self::from_activation(activation_path, &activation)
    }

    fn from_activation(activation_path: &Path, activation: &HookRuntime) -> Result<Self, String> {
        let runtime_profiles = activation_needs_runtime_profile_fallback(activation).then(|| {
            let runtime_project_root =
                runtime_project_root_for_activation(activation_path, &activation.project_root);
            runtime_profiles_for_runtime(&runtime_project_root, activation)
        });
        Ok(Self {
            activation_path: activation_path.to_path_buf(),
            providers: activation
                .providers
                .iter()
                .map(|provider| {
                    let mut resolved = ResolvedProvider::from(provider);
                    if let Some(runtime_profiles) = runtime_profiles.as_ref() {
                        resolved.runtime_command_argv =
                            runtime_profile_command_argv(runtime_profiles, provider);
                        resolved.runtime_profile_status = runtime_profiles
                            .providers
                            .iter()
                            .find(|profile| {
                                profile.manifest_id == provider.manifest_id
                                    && profile.language_id == provider.language_id
                                    && profile.provider_id == provider.provider_id
                                    && profile.binary == provider.binary
                            })
                            .map(|profile| profile.health.status.into());
                    }
                    resolved
                })
                .collect(),
        })
    }

    /// Resolve the activated provider for a language id.
    #[must_use]
    pub fn provider_for_language(&self, language_id: &LanguageId) -> Option<&ResolvedProvider> {
        self.providers
            .iter()
            .find(|provider| &provider.language_id == language_id)
    }

    /// Return provenance for every activated provider.
    #[must_use]
    pub fn native_provenance(&self) -> Vec<NativeProvenance> {
        self.providers
            .iter()
            .map(ResolvedProvider::provenance)
            .collect()
    }
}

fn activation_needs_runtime_profile_fallback(activation: &HookRuntime) -> bool {
    activation
        .providers
        .iter()
        .any(|provider| provider.provider_command_prefix.is_empty())
}

fn project_root_has_provider_identity(project_root: &Path) -> bool {
    project_root.join(".git").exists()
        || project_root.join("Cargo.toml").is_file()
        || project_root.join("package.json").is_file()
        || project_root.join("pyproject.toml").is_file()
        || project_root.join("Project.toml").is_file()
        || project_root.join("JuliaProject.toml").is_file()
}
