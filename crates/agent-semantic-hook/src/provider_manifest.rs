//! Built-in provider manifests and default project activations.

use serde::Deserialize;
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use crate::executable::{is_executable_file, resolve_executable_with_status};
use crate::protocol::{
    HOOK_ACTIVATION_SCHEMA_ID, HOOK_ACTIVATION_SCHEMA_VERSION, HOOK_PROTOCOL_ID,
    HOOK_PROTOCOL_VERSION,
};
use crate::protocol_activation::{
    ActivatedProviderConfig, ActivationCoverage, ActivationGeneratedBy, HookActivation,
    ProviderManifest, provider_manifest_digest,
};
use crate::provider_registry::schema_registry_provider_manifests;

/// Returns the built-in language provider manifests known to this hook runtime.
pub fn builtin_provider_manifests() -> Vec<ProviderManifest> {
    provider_manifests()
}

pub(crate) fn provider_manifests() -> Vec<ProviderManifest> {
    schema_registry_provider_manifests()
}

/// Build the default project activation from configured project providers.
pub fn build_default_activation(project_root: &Path) -> Result<HookActivation, String> {
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
        providers.push(activate_provider(project_root, &manifest, command_prefix)?);
    }
    if providers.is_empty() {
        return Err(
            "expected PATH to contain at least one executable semantic provider binary".to_string(),
        );
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

fn activate_provider(
    project_root: &Path,
    manifest: &ProviderManifest,
    provider_command_prefix: Vec<String>,
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
            package_roots: discover_package_roots(project_root, manifest),
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
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ProjectProviderConfig {
    #[serde(default)]
    enabled: Option<bool>,
    #[serde(default)]
    binary: Option<String>,
}

#[derive(Debug, Default)]
struct ProjectProviderConfigSet {
    providers: BTreeMap<String, ProjectProviderConfig>,
}

impl ProjectProviderConfigSet {
    fn load(project_root: &Path) -> Result<Self, String> {
        let config_path = project_root.join("asp.toml");
        if !config_path.is_file() {
            return Ok(Self::default());
        }
        let contents = fs::read_to_string(&config_path)
            .map_err(|error| format!("failed to read {}: {error}", config_path.display()))?;
        let config: AgentSemanticProjectConfig = toml::from_str(&contents)
            .map_err(|error| format!("invalid {}: {error}", config_path.display()))?;
        Ok(Self {
            providers: config.providers,
        })
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
    let project_bin = project_root.join(".bin").join(&manifest.binary);
    if is_executable_file(&project_bin) {
        return project_bin.display().to_string();
    }
    manifest.binary.clone()
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

fn discover_package_roots(project_root: &Path, manifest: &ProviderManifest) -> Vec<String> {
    let mut roots = Vec::new();
    collect_package_roots(project_root, project_root, manifest, 0, &mut roots);
    if roots.is_empty() {
        roots.push(".".to_string());
    }
    roots.sort_by(|left, right| {
        left.matches('/')
            .count()
            .cmp(&right.matches('/').count())
            .then(left.cmp(right))
    });
    roots.dedup();
    roots
}

fn collect_package_roots(
    project_root: &Path,
    current: &Path,
    manifest: &ProviderManifest,
    depth: usize,
    roots: &mut Vec<String>,
) {
    if depth > 5 {
        return;
    }
    if package_root_matches(current, manifest) {
        roots.push(relative_package_root(project_root, current));
    }
    let Ok(entries) = fs::read_dir(current) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() || should_skip_package_root_dir(&path, manifest) {
            continue;
        }
        collect_package_roots(project_root, &path, manifest, depth + 1, roots);
    }
}

fn package_root_matches(candidate: &Path, manifest: &ProviderManifest) -> bool {
    manifest
        .source
        .default_config_files
        .iter()
        .any(|config| candidate.join(config).is_file())
        && manifest
            .source
            .default_source_roots
            .iter()
            .any(|root| candidate.join(root).is_dir())
}

fn relative_package_root(project_root: &Path, package_root: &Path) -> String {
    package_root
        .strip_prefix(project_root)
        .ok()
        .filter(|relative| !relative.as_os_str().is_empty())
        .map(|relative| relative.to_string_lossy().replace('\\', "/"))
        .unwrap_or_else(|| ".".to_string())
}

fn should_skip_package_root_dir(path: &Path, manifest: &ProviderManifest) -> bool {
    let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
        return false;
    };
    if name.starts_with('.') {
        return true;
    }
    matches!(name, "target" | "node_modules" | "dist" | "build" | "venv")
        || manifest
            .source
            .default_ignored_path_prefixes
            .iter()
            .any(|ignored| ignored == name)
}
