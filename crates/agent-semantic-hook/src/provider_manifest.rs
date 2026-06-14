//! Built-in provider manifests and default project activations.

use serde::Deserialize;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

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
    schema_registry_provider_manifests()
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
        let config_path = project_root.join("asp.toml");
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
    let project_bin = project_root.join(".bin").join(&manifest.binary);
    if is_executable_file(&project_bin) {
        return project_bin.display().to_string();
    }
    if let Some(runtime_home) = project_runtime_layout(project_root).runtime_home {
        let managed_bin = runtime_home.join("bin").join(&manifest.binary);
        if is_executable_file(&managed_bin) {
            return managed_bin.display().to_string();
        }
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

fn discover_package_roots_for_manifests<'a>(
    project_root: &Path,
    manifests: impl IntoIterator<Item = &'a ProviderManifest>,
) -> BTreeMap<String, Vec<String>> {
    let manifests = manifests.into_iter().collect::<Vec<_>>();
    let candidates = package_root_candidates(project_root);
    let mut roots_by_manifest = BTreeMap::new();
    for manifest in manifests {
        let mut roots = candidates
            .iter()
            .filter(|candidate| package_root_matches(candidate, manifest))
            .map(|candidate| relative_package_root(project_root, &candidate.path))
            .collect::<Vec<_>>();
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
        roots_by_manifest.insert(manifest.manifest_id.clone(), roots);
    }
    roots_by_manifest
}

struct PackageRootCandidate {
    path: PathBuf,
    files: BTreeSet<String>,
    dirs: BTreeSet<String>,
}

fn package_root_candidates(project_root: &Path) -> Vec<PackageRootCandidate> {
    let mut candidates = Vec::new();
    collect_package_root_candidates(project_root, 0, &mut candidates);
    candidates
}

fn collect_package_root_candidates(
    current: &Path,
    depth: usize,
    candidates: &mut Vec<PackageRootCandidate>,
) {
    if depth > 5 {
        return;
    }
    let Ok(entries) = fs::read_dir(current) else {
        candidates.push(PackageRootCandidate {
            path: current.to_path_buf(),
            files: BTreeSet::new(),
            dirs: BTreeSet::new(),
        });
        return;
    };
    let mut files = BTreeSet::new();
    let mut dirs = BTreeSet::new();
    let mut children = Vec::new();
    for entry in entries.flatten() {
        let Some(name) = entry.file_name().to_str().map(str::to_string) else {
            continue;
        };
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        if file_type.is_dir() {
            dirs.insert(name);
            let path = entry.path();
            if depth < 5 && !should_skip_package_root_candidate_dir(&path) {
                children.push(path);
            }
        } else if file_type.is_file() {
            files.insert(name);
        }
    }
    candidates.push(PackageRootCandidate {
        path: current.to_path_buf(),
        files,
        dirs,
    });
    for child in children {
        collect_package_root_candidates(&child, depth + 1, candidates);
    }
}

fn package_root_matches(candidate: &PackageRootCandidate, manifest: &ProviderManifest) -> bool {
    manifest
        .source
        .default_config_files
        .iter()
        .any(|config| candidate_file_matches(candidate, config))
        && manifest
            .source
            .default_source_roots
            .iter()
            .any(|root| candidate_dir_matches(candidate, root))
}

fn candidate_file_matches(candidate: &PackageRootCandidate, file: &str) -> bool {
    if simple_child_name(file) {
        candidate.files.contains(file)
    } else {
        candidate.path.join(file).is_file()
    }
}

fn candidate_dir_matches(candidate: &PackageRootCandidate, dir: &str) -> bool {
    if simple_child_name(dir) {
        candidate.dirs.contains(dir)
    } else {
        candidate.path.join(dir).is_dir()
    }
}

fn simple_child_name(value: &str) -> bool {
    !value.contains('/') && !value.contains('\\')
}

fn relative_package_root(project_root: &Path, package_root: &Path) -> String {
    package_root
        .strip_prefix(project_root)
        .ok()
        .filter(|relative| !relative.as_os_str().is_empty())
        .map(|relative| relative.to_string_lossy().replace('\\', "/"))
        .unwrap_or_else(|| ".".to_string())
}

fn should_skip_package_root_candidate_dir(path: &Path) -> bool {
    let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
        return false;
    };
    if name.starts_with('.') {
        return true;
    }
    matches!(name, "target" | "node_modules" | "dist" | "build" | "venv")
}
