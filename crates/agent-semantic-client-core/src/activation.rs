//! Provider activation snapshot loading for `agent-semantic-client`.

use std::collections::BTreeSet;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use agent_semantic_hook::{
    ActivatedProvider, HookRuntime, ProviderExecution, RuntimeProviderHealthStatus,
    builtin_provider_manifests, load_or_sync_activation, parse_activation,
    runtime_profile_command_argv, runtime_profiles_for_runtime,
    runtime_project_root_for_activation,
};
use agent_semantic_runtime::{discover_project_activation_path, project_activation_path};

use crate::receipt::NativeProvenance;
use crate::types::{LanguageId, ProviderId};

/// Explicit activation.json path used when a protocol facade already resolved
/// the hook installation root separately from the provider execution root.
pub const ASP_PROVIDER_ACTIVATION_PATH_ENV: &str = "ASP_PROVIDER_ACTIVATION_PATH";

/// Provider resolved from the project hook activation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedProvider {
    pub manifest_id: String,
    pub manifest_digest: String,
    pub namespace: String,
    pub language_id: LanguageId,
    pub provider_id: ProviderId,
    pub binary: String,
    pub execution: ProviderExecution,
    pub provider_command_prefix: Vec<String>,
    pub execution_command_digest: String,
    pub runtime_command_argv: Option<Vec<String>>,
    pub runtime_profile_status: Option<RuntimeProfileStatus>,
    pub package_roots: Vec<String>,
    pub source_roots: Vec<String>,
    pub config_files: Vec<String>,
    pub source_extensions: Vec<String>,
    pub ignored_path_prefixes: Vec<String>,
    pub search_capabilities: agent_semantic_hook::ProviderSearchCapabilities,
    pub query_pack_descriptor: agent_semantic_hook::ProviderQueryPackDescriptor,
    pub semantic_facts_descriptor: Option<agent_semantic_hook::ProviderSemanticFactsDescriptor>,
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
}

impl TryFrom<&ActivatedProvider> for ResolvedProvider {
    type Error = String;

    fn try_from(provider: &ActivatedProvider) -> Result<Self, Self::Error> {
        Ok(Self {
            manifest_id: provider.manifest_id.clone(),
            manifest_digest: provider.manifest_digest.clone(),
            namespace: provider.namespace.clone(),
            language_id: provider.language_id.clone().into(),
            provider_id: provider.provider_id.clone().into(),
            binary: provider.binary.clone(),
            execution: provider.execution,
            provider_command_prefix: provider.provider_command_prefix.clone(),
            execution_command_digest: provider.execution_command_digest.clone(),
            runtime_command_argv: None,
            runtime_profile_status: None,
            package_roots: provider.package_roots.clone(),
            source_roots: provider.source_roots.clone(),
            config_files: provider.config_files.clone(),
            source_extensions: provider.source_extensions.clone(),
            ignored_path_prefixes: provider.ignored_path_prefixes.clone(),
            search_capabilities: provider.search_capabilities.clone(),
            query_pack_descriptor: provider.query_pack_descriptor.clone(),
            semantic_facts_descriptor: provider.semantic_facts_descriptor.clone(),
        })
    }
}

/// Snapshot of activated providers for one project root.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProviderRegistrySnapshot {
    pub activation_path: PathBuf,
    pub providers: Vec<ResolvedProvider>,
}

/// Derived provider-registry evidence used to guard cache reuse.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProviderRegistryEvidence {
    pub fingerprint: String,
    pub scope_dirs: BTreeSet<String>,
}

impl ProviderRegistrySnapshot {
    pub fn load(project_root: &Path) -> Result<Self, String> {
        if let Some(activation_path) = env::var_os(ASP_PROVIDER_ACTIVATION_PATH_ENV) {
            let activation_path = PathBuf::from(activation_path);
            if activation_path.is_file() {
                return Self::load_from_path(&activation_path);
            }
        }
        let direct_activation_path = match project_activation_path(project_root) {
            Ok(activation_path) => Some(activation_path),
            Err(error) => {
                if let Some(candidate) = discover_project_activation_path(project_root) {
                    return Self::load_from_path_for_project(&candidate, project_root);
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
            if let Some(candidate) = discover_project_activation_path(project_root) {
                return Self::load_from_path_for_project(&candidate, project_root);
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

    pub fn from_activation(
        activation_path: &Path,
        activation: &HookRuntime,
    ) -> Result<Self, String> {
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
                    let mut resolved = ResolvedProvider::try_from(provider)?;
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
                    Ok(resolved)
                })
                .collect::<Result<Vec<_>, String>>()?,
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

    /// Build stable provider-registry evidence for cache reuse checks.
    #[must_use]
    pub fn evidence(&self, project_root: &Path) -> ProviderRegistryEvidence {
        ProviderRegistryEvidence {
            fingerprint: provider_registry_fingerprint(self),
            scope_dirs: provider_registry_scope_dirs(project_root, self),
        }
    }
}

fn provider_registry_fingerprint(snapshot: &ProviderRegistrySnapshot) -> String {
    let mut rows = vec![format!("activation={}", snapshot.activation_path.display())];
    for provider in &snapshot.providers {
        rows.push(provider_fingerprint(provider));
    }
    rows.join("\n")
}

fn provider_registry_scope_dirs(
    project_root: &Path,
    snapshot: &ProviderRegistrySnapshot,
) -> BTreeSet<String> {
    let mut dirs = BTreeSet::new();
    dirs.insert(".".to_string());
    for provider in &snapshot.providers {
        append_provider_scope_dirs(project_root, provider, &mut dirs);
    }
    dirs
}

fn append_provider_scope_dirs(
    project_root: &Path,
    provider: &ResolvedProvider,
    dirs: &mut BTreeSet<String>,
) {
    let package_roots = if provider.package_roots.is_empty() {
        vec![".".to_string()]
    } else {
        provider.package_roots.clone()
    };
    for package_root in package_roots {
        insert_existing_scope_dir(project_root, &project_root.join(&package_root), dirs);
        for source_root in &provider.source_roots {
            insert_existing_scope_dir(
                project_root,
                &project_root.join(&package_root).join(source_root),
                dirs,
            );
        }
        for config_file in &provider.config_files {
            if let Some(parent) = project_root.join(&package_root).join(config_file).parent() {
                insert_existing_scope_dir(project_root, parent, dirs);
            }
        }
    }
}

fn insert_existing_scope_dir(project_root: &Path, dir: &Path, dirs: &mut BTreeSet<String>) {
    if !dir.is_dir() {
        return;
    }
    let relative = dir
        .strip_prefix(project_root)
        .ok()
        .and_then(|path| path.to_str())
        .filter(|value| !value.is_empty())
        .unwrap_or(".");
    dirs.insert(relative.replace(std::path::MAIN_SEPARATOR, "/"));
}

fn provider_fingerprint(provider: &ResolvedProvider) -> String {
    [
        format!("manifest={}", provider.manifest_id),
        format!("manifestDigest={}", provider.manifest_digest),
        format!("namespace={}", provider.namespace),
        format!("language={}", provider.language_id),
        format!("provider={}", provider.provider_id),
        format!("binary={}", provider.binary),
        format!("execution={:?}", provider.execution),
        format!("prefix={}", provider.provider_command_prefix.join("\u{1f}")),
        format!(
            "runtime={}",
            provider
                .runtime_command_argv
                .as_ref()
                .map(|argv| argv.join("\u{1f}"))
                .unwrap_or_default()
        ),
        format!(
            "runtimeStatus={}",
            provider
                .runtime_profile_status
                .map(|status| status.as_str())
                .unwrap_or_default()
        ),
        format!("packageRoots={}", provider.package_roots.join("\u{1f}")),
        format!("sourceRoots={}", provider.source_roots.join("\u{1f}")),
        format!("configFiles={}", provider.config_files.join("\u{1f}")),
        format!(
            "sourceExtensions={}",
            provider.source_extensions.join("\u{1f}")
        ),
        format!(
            "ignoredPathPrefixes={}",
            provider.ignored_path_prefixes.join("\u{1f}")
        ),
        format!(
            "searchCapabilities={}",
            serde_json::to_string(&provider.search_capabilities)
                .expect("search-capabilities serialization must be infallible")
        ),
        format!(
            "queryPackDescriptor={}",
            serde_json::to_string(&provider.query_pack_descriptor)
                .expect("query-pack descriptor serialization must be infallible")
        ),
        format!(
            "semanticFactsDescriptor={}",
            serde_json::to_string(&provider.semantic_facts_descriptor)
                .expect("semantic-facts descriptor serialization must be infallible")
        ),
    ]
    .join("\u{1e}")
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
