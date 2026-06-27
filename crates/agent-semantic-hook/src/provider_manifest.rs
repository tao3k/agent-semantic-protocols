//! Built-in provider manifests and default project activations.

use serde::Deserialize;
use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use agent_semantic_config::project_runtime_layout;

use crate::executable::{is_executable_file, resolve_executable_with_status};
use crate::protocol::{
    HOOK_ACTIVATION_SCHEMA_ID, HOOK_ACTIVATION_SCHEMA_VERSION, HOOK_PROTOCOL_ID,
    HOOK_PROTOCOL_VERSION,
};
use crate::protocol_activation::{
    ActivatedProviderConfig, ActivationCoverage, ActivationGeneratedBy, HookActivation,
    ProviderExecution, ProviderManifest, provider_manifest_digest,
};
use crate::provider_registry::schema_registry_provider_manifests;

/// Returns the built-in language provider manifests known to this hook runtime.
pub fn builtin_provider_manifests() -> Vec<ProviderManifest> {
    provider_manifests()
}

pub(crate) fn provider_manifests() -> Vec<ProviderManifest> {
    static PROVIDER_MANIFESTS: OnceLock<Vec<ProviderManifest>> = OnceLock::new();
    PROVIDER_MANIFESTS
        .get_or_init(schema_registry_provider_manifests)
        .clone()
}

/// Build the default project activation from configured project providers.
pub fn build_default_activation(project_root: &Path) -> Result<HookActivation, String> {
    let project_config = ProjectProviderConfigSet::load(project_root)?;
    let mut selected_providers = Vec::new();
    for manifest in provider_manifests() {
        let Some(provider_config) = project_config.provider_config(&manifest.language_id) else {
            continue;
        };
        let Some(command_prefix) =
            provider_command_prefix(project_root, &manifest, provider_config)?
        else {
            continue;
        };
        selected_providers.push((manifest, command_prefix));
    }
    if selected_providers.is_empty() {
        return Err(
            "expected PATH to contain at least one executable semantic provider binary".to_string(),
        );
    }
    let package_roots = discover_package_roots_for_manifests(
        project_root,
        selected_providers.iter().map(|(manifest, _)| manifest),
    );
    let mut providers = Vec::new();
    for (manifest, command_prefix) in selected_providers {
        let roots = package_roots
            .get(&manifest.manifest_id)
            .cloned()
            .unwrap_or_else(|| vec![".".to_string()]);
        providers.push(activate_provider(&manifest, command_prefix, roots)?);
    }
    Ok(HookActivation {
        schema_id: HOOK_ACTIVATION_SCHEMA_ID.to_string(),
        schema_version: HOOK_ACTIVATION_SCHEMA_VERSION.to_string(),
        protocol_id: HOOK_PROTOCOL_ID.to_string(),
        protocol_version: HOOK_PROTOCOL_VERSION.to_string(),
        project_root: project_root.display().to_string(),
        generated_by: ActivationGeneratedBy {
            runtime: "asp".to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
        },
        generated_at: None,
        providers,
    })
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProviderCommandSelection {
    pub manifest_id: String,
    pub manifest_digest: String,
    pub language_id: String,
    pub provider_id: String,
    pub binary: String,
    pub execution: ProviderExecution,
    pub provider_command_prefix: Vec<String>,
}

pub fn provider_command_selections(
    project_root: &Path,
) -> Result<Vec<ProviderCommandSelection>, String> {
    let project_config = ProjectProviderConfigSet::load(project_root)?;
    let mut providers = Vec::new();
    for manifest in provider_manifests() {
        let Some(provider_config) = project_config.provider_config(&manifest.language_id) else {
            continue;
        };
        let Some(command_prefix) =
            provider_command_prefix(project_root, &manifest, provider_config)?
        else {
            continue;
        };
        providers.push(ProviderCommandSelection {
            manifest_id: manifest.manifest_id.clone(),
            manifest_digest: provider_manifest_digest(&manifest)
                .map_err(|error| format!("failed to digest provider manifest: {error:?}"))?,
            language_id: manifest.language_id.clone(),
            provider_id: manifest.provider_id.clone(),
            binary: manifest.binary.clone(),
            execution: manifest.execution,
            provider_command_prefix: command_prefix,
        });
    }
    if providers.is_empty() {
        return Err(
            "expected PATH to contain at least one executable semantic provider binary".to_string(),
        );
    }
    Ok(providers)
}

pub fn project_agent_config_path(project_root: &Path) -> PathBuf {
    project_root.join(".agents").join("asp.toml")
}

fn legacy_project_agent_config_path(project_root: &Path) -> PathBuf {
    project_root.join("asp.toml")
}

pub fn migrate_legacy_project_agent_config(project_root: &Path) -> Result<Option<PathBuf>, String> {
    let legacy_path = legacy_project_agent_config_path(project_root);
    if !legacy_path.is_file() {
        return Ok(None);
    }

    let config_path = project_agent_config_path(project_root);
    if config_path.is_file() {
        let legacy_contents = fs::read_to_string(&legacy_path)
            .map_err(|error| format!("failed to read {}: {error}", legacy_path.display()))?;
        let config_contents = fs::read_to_string(&config_path)
            .map_err(|error| format!("failed to read {}: {error}", config_path.display()))?;
        if legacy_contents == config_contents {
            fs::remove_file(&legacy_path)
                .map_err(|error| format!("failed to remove {}: {error}", legacy_path.display()))?;
            return Ok(Some(config_path));
        }
        return Err(format!(
            "cannot migrate legacy provider config {}; {} already exists with different contents. Merge the files into {} and remove the top-level asp.toml.",
            legacy_path.display(),
            config_path.display(),
            config_path.display()
        ));
    }

    if let Some(parent) = config_path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("failed to create {}: {error}", parent.display()))?;
    }
    fs::rename(&legacy_path, &config_path).map_err(|error| {
        format!(
            "failed to migrate {} to {}: {error}",
            legacy_path.display(),
            config_path.display()
        )
    })?;
    Ok(Some(config_path))
}

fn activate_provider(
    manifest: &ProviderManifest,
    provider_command_prefix: Vec<String>,
    package_roots: Vec<String>,
) -> Result<ActivatedProviderConfig, String> {
    Ok(ActivatedProviderConfig {
        manifest_id: manifest.manifest_id.clone(),
        manifest_digest: provider_manifest_digest(manifest)
            .map_err(|error| format!("failed to digest provider manifest: {error:?}"))?,
        language_id: manifest.language_id.clone(),
        provider_id: manifest.provider_id.clone(),
        binary: manifest.binary.clone(),
        execution: manifest.execution,
        provider_command_prefix,
        coverage: ActivationCoverage {
            package_roots,
            source_roots: manifest.source.default_source_roots.clone(),
            config_files: manifest.source.default_config_files.clone(),
            source_extensions: manifest.source.default_extensions.clone(),
            ignored_path_prefixes: manifest.source.default_ignored_path_prefixes.clone(),
        },
    })
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AgentSemanticProjectConfig {
    #[serde(default)]
    providers: BTreeMap<String, ProjectProviderConfig>,
    #[serde(default)]
    languages: BTreeMap<String, ProjectProviderConfig>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ProjectProviderConfig {
    #[serde(default)]
    enabled: Option<bool>,
    #[serde(default)]
    #[serde(alias = "bin")]
    binary: Option<String>,
}

#[derive(Debug, Default)]
struct ProjectProviderConfigSet {
    providers: BTreeMap<String, ProjectProviderConfig>,
}

impl ProjectProviderConfigSet {
    fn load(project_root: &Path) -> Result<Self, String> {
        let config_path = project_agent_config_path(project_root);
        if !config_path.is_file() {
            return Ok(Self::default());
        }
        let contents = fs::read_to_string(&config_path)
            .map_err(|error| format!("failed to read {}: {error}", config_path.display()))?;
        let config: AgentSemanticProjectConfig = toml::from_str(&contents)
            .map_err(|error| format!("invalid {}: {error}", config_path.display()))?;
        let mut providers = config.languages;
        providers.extend(config.providers);
        Ok(Self { providers })
    }

    fn provider_config(&self, language_id: &str) -> Option<&ProjectProviderConfig> {
        let config = self.providers.get(language_id);
        if config.and_then(|config| config.enabled) == Some(false) {
            return None;
        }
        Some(config.unwrap_or(&DEFAULT_PROVIDER_CONFIG))
    }
}

static DEFAULT_PROVIDER_CONFIG: ProjectProviderConfig = ProjectProviderConfig {
    enabled: None,
    binary: None,
};

fn provider_command_prefix(
    project_root: &Path,
    manifest: &ProviderManifest,
    config: &ProjectProviderConfig,
) -> Result<Option<Vec<String>>, String> {
    let has_binary_override = config.binary.is_some();
    let configured_binary = config.binary.as_deref().unwrap_or(&manifest.binary);
    let provider_binary = if has_binary_override {
        project_root_relative_binary(project_root, configured_binary)
    } else {
        default_provider_binary(project_root, manifest)
    };
    let resolution = resolve_executable_with_status(&provider_binary);
    let Some(path) = resolution.path else {
        if config.enabled == Some(true) || config.binary.is_some() {
            return Err(format!(
                "provider `{}` language `{}` binary `{configured_binary}` is not executable: {}",
                manifest.provider_id,
                manifest.language_id,
                resolution
                    .reason
                    .unwrap_or_else(|| "provider binary unavailable".to_string())
            ));
        }
        return Ok(None);
    };
    Ok(Some(vec![path.display().to_string()]))
}

fn default_provider_binary(project_root: &Path, manifest: &ProviderManifest) -> String {
    if provider_prefers_home_local_binary(manifest)
        && let Some(user_bin) = home_local_provider_binary(&manifest.binary)
    {
        return user_bin.display().to_string();
    }
    let project_bin = project_root.join(".bin").join(&manifest.binary);
    if is_executable_file(&project_bin) {
        return project_bin.display().to_string();
    }
    if let Some(workspace_bin) = ancestor_workspace_provider_binary(project_root, &manifest.binary)
    {
        return workspace_bin.display().to_string();
    }
    if let Some(runtime_home) = project_runtime_layout(project_root).runtime_home {
        let managed_bin = runtime_home.join("bin").join(&manifest.binary);
        if is_executable_file(&managed_bin) {
            return managed_bin.display().to_string();
        }
    }
    if let Some(user_bin) = home_local_provider_binary(&manifest.binary) {
        return user_bin.display().to_string();
    }
    manifest.binary.clone()
}

fn provider_prefers_home_local_binary(manifest: &ProviderManifest) -> bool {
    manifest.binary == "gslph"
}

fn ancestor_workspace_provider_binary(project_root: &Path, binary: &str) -> Option<PathBuf> {
    project_root.ancestors().skip(1).find_map(|ancestor| {
        let candidate = ancestor.join(".bin").join(binary);
        (project_agent_config_path(ancestor).is_file() && is_executable_file(&candidate))
            .then_some(candidate)
    })
}

fn home_local_provider_binary(binary: &str) -> Option<PathBuf> {
    let home = env::var_os("HOME")?;
    let candidate = PathBuf::from(home).join(".local").join("bin").join(binary);
    is_executable_file(&candidate).then_some(candidate)
}

fn project_root_relative_binary(project_root: &Path, binary: &str) -> String {
    let path = PathBuf::from(binary);
    if path.is_absolute() {
        return binary.to_string();
    }
    if binary.contains('/') || binary.contains('\\') || binary.starts_with('.') {
        return project_root.join(path).display().to_string();
    }
    binary.to_string()
}

fn discover_package_roots_for_manifests<'a>(
    project_root: &Path,
    manifests: impl IntoIterator<Item = &'a ProviderManifest>,
) -> BTreeMap<String, Vec<String>> {
    let mut roots_by_manifest = BTreeMap::new();
    for manifest in manifests {
        roots_by_manifest.insert(
            manifest.manifest_id.clone(),
            vec![relative_package_root(project_root, project_root)],
        );
    }
    roots_by_manifest
}

fn relative_package_root(project_root: &Path, package_root: &Path) -> String {
    package_root
        .strip_prefix(project_root)
        .ok()
        .filter(|relative| !relative.as_os_str().is_empty())
        .map(|relative| relative.to_string_lossy().replace('\\', "/"))
        .unwrap_or_else(|| ".".to_string())
}
