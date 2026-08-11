//! Built-in provider manifests and default project activations.

use serde::Deserialize;
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use crate::protocol::{
    HOOK_ACTIVATION_SCHEMA_ID, HOOK_ACTIVATION_SCHEMA_VERSION, HOOK_PROTOCOL_ID,
    HOOK_PROTOCOL_VERSION,
};
use crate::protocol_activation::provider_manifest_digest;
use crate::protocol_activation::{
    ActivatedProviderConfig, ActivatedRankerConfig, ActivationCoverage, ActivationGeneratedBy,
    HookActivation, ProviderExecution, ProviderManifest,
};
use crate::provider_registry::schema_registry_provider_manifests;
use crate::resolve_executable_with_status;

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
    let selections = default_activation_selections(project_root)?;
    build_default_activation_from_selections(project_root, &selections)
}

/// Build the default activation against an explicitly selected State Home.
///
/// Keeping State Home in the call graph makes concurrent callers independent;
/// the environment-resolving entry point above is only a process-boundary
/// convenience wrapper.
pub fn build_default_activation_with_state_home(
    project_root: &Path,
    state_home: &Path,
) -> Result<HookActivation, String> {
    let selections = default_activation_selections_with_state_home(project_root, state_home)?;
    build_default_activation_from_selections(project_root, &selections)
}

#[cfg(test)]
#[path = "../tests/unit/provider_command_selection_scope.rs"]
mod provider_command_selection_scope_tests;
#[cfg(test)]
#[path = "../tests/unit/provider_manifest_selection_identity.rs"]
mod provider_manifest_selection_identity_tests;

/// Build an activation from the provider selections already resolved for this project.
pub fn build_default_activation_from_selections(
    project_root: &Path,
    selections: &DefaultActivationSelections,
) -> Result<HookActivation, String> {
    if selections.providers.is_empty() {
        return Err(
            "expected State Home runtime bin to contain at least one executable semantic provider binary"
                .to_string(),
        );
    }
    let manifests = provider_manifests();
    let selected_providers = selections
        .providers
        .iter()
        .map(|selection| {
            let manifest = manifests
                .iter()
                .find(|manifest| manifest.manifest_id == selection.manifest_id)
                .ok_or_else(|| {
                    format!(
                        "provider selection has no registered manifest: manifestId={} language={} provider={}",
                        selection.manifest_id, selection.language_id, selection.provider_id
                    )
                })?;
            Ok((manifest, selection))
        })
        .collect::<Result<Vec<_>, String>>()?;
    let registry_started = std::time::Instant::now();
    let semantic_registry_digest = crate::provider_registry::semantic_registry_digest();
    if std::env::var_os("ASP_HOOK_INSTALL_TIMINGS").is_some() {
        eprintln!(
            "[activation-timing] step=semantic-registry-digest stepMs={:.3}",
            registry_started.elapsed().as_secs_f64() * 1_000.0
        );
    }
    let mut providers = Vec::new();
    for (manifest, selection) in selected_providers {
        let coverage = activation_capability_coverage(manifest)?;
        providers.push(activate_provider(
            manifest,
            selection.manifest_digest.clone(),
            selection.execution_command_digest.clone(),
            selection.binary.clone(),
            coverage,
            &semantic_registry_digest,
        )?);
    }
    if providers.is_empty() {
        return Err(
            "no installed provider exposes a typed project or document resolver".to_string(),
        );
    }
    let rankers = vec![activate_builtin_graph_turbo_ranker(&selections.graph_turbo)];
    Ok(HookActivation {
        schema_id: HOOK_ACTIVATION_SCHEMA_ID.to_string(),
        schema_version: HOOK_ACTIVATION_SCHEMA_VERSION.to_string(),
        schema_authority: crate::protocol::CANONICAL_SCHEMA_AUTHORITY.to_string(),
        protocol_id: HOOK_PROTOCOL_ID.to_string(),
        protocol_version: HOOK_PROTOCOL_VERSION.to_string(),
        project_root: project_root.display().to_string(),
        generated_by: ActivationGeneratedBy {
            runtime: "asp".to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
        },
        generated_at: None,
        rankers,
        providers,
    })
}

pub(crate) fn activation_capability_coverage(
    manifest: &ProviderManifest,
) -> Result<ActivationCoverage, String> {
    if let Some(descriptor) = manifest.project_resolution() {
        return Ok(ActivationCoverage {
            package_roots: Vec::new(),
            config_files: descriptor.entry_markers.clone(),
            source_extensions: descriptor.source_extensions.clone(),
        });
    }
    if let Some(descriptor) = manifest.document_resolution() {
        return Ok(ActivationCoverage {
            package_roots: Vec::new(),
            config_files: Vec::new(),
            source_extensions: descriptor.extensions.clone(),
        });
    }
    Err(format!(
        "provider {} omitted both projectResolution and documentResolution",
        manifest.provider_id()
    ))
}

#[cfg(test)]
#[path = "../tests/unit/provider_manifest/activation_capability.rs"]
mod provider_activation_capability_tests;

fn capture_current_asp_binary_selection(
    activation_path: Option<&Path>,
) -> Result<RuntimeBinarySelectionV1, String> {
    let binary = std::env::var_os("SEMANTIC_AGENT_PROTOCOL_BIN")
        .map(PathBuf::from)
        .unwrap_or(std::env::current_exe().map_err(|error| {
            format!("failed to resolve ASP binary for built-in graph-turbo ranker: {error}")
        })?);
    capture_asp_binary_selection(&binary, activation_path)
}

fn capture_asp_binary_selection(
    binary: &Path,
    activation_path: Option<&Path>,
) -> Result<RuntimeBinarySelectionV1, String> {
    let binary = binary.canonicalize().map_err(|error| {
        format!("failed to canonicalize ASP graph-turbo ranker binary: {error}")
    })?;
    if let Some(activation_path) = activation_path {
        return asp_binary_selection_from_active_receipt(&binary, activation_path);
    }
    let content_digest =
        match agent_semantic_content_identity::blake3_digest_from_canonical_artifact_path(&binary) {
            Some(digest) => digest,
            None => agent_semantic_content_identity::file_content_digest_v1(&binary)?.to_string(),
        };
    let artifact_metadata_digest =
        agent_semantic_content_identity::file_artifact_metadata_digest_v1(&binary)?.to_string();
    RuntimeBinarySelectionV1::new(
        binary.display().to_string(),
        content_digest,
        artifact_metadata_digest,
    )
}

fn asp_binary_selection_from_active_receipt(
    binary: &Path,
    activation_path: &Path,
) -> Result<RuntimeBinarySelectionV1, String> {
    let receipt = crate::verify_active_asp_artifact_receipt(activation_path, &[binary])?;
    let artifact_metadata_digest =
        agent_semantic_content_identity::file_artifact_metadata_digest_v1(binary)?;
    RuntimeBinarySelectionV1::new(
        binary.display().to_string(),
        receipt
            .asp_binary_leaf()
            .artifact_digest()
            .as_str()
            .to_string(),
        artifact_metadata_digest.to_string(),
    )
}

fn activate_builtin_graph_turbo_ranker(
    selection: &RuntimeBinarySelectionV1,
) -> ActivatedRankerConfig {
    ActivatedRankerConfig {
        schema_id: "asp.activated-ranker.v1".to_string(),
        ranker_id: "asp-graph-turbo".to_string(),
        capability_id: "graph-turbo".to_string(),
        protocol_version: "1".to_string(),
        binary: selection.binary.clone(),
        argv_prefix: vec!["graph".to_string(), "render".to_string()],
        content_digest: selection.content_digest.clone(),
        artifact_metadata_digest: selection.artifact_metadata_digest.clone(),
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProviderCommandSelection {
    pub(crate) manifest_id: String,
    pub(crate) manifest_digest: String,
    pub(crate) execution_command_digest: String,
    pub(crate) language_id: agent_semantic_config::LanguageId,
    pub(crate) provider_id: agent_semantic_config::ProviderId,
    pub(crate) binary: String,
    pub(crate) execution: ProviderExecution,
    pub(crate) provider_command_prefix: Vec<String>,
}

impl ProviderCommandSelection {
    #[must_use]
    pub fn manifest_id(&self) -> &str {
        &self.manifest_id
    }

    #[must_use]
    pub fn manifest_digest(&self) -> &str {
        &self.manifest_digest
    }

    #[must_use]
    pub fn language_id(&self) -> &agent_semantic_config::LanguageId {
        &self.language_id
    }

    #[must_use]
    pub fn provider_id(&self) -> &agent_semantic_config::ProviderId {
        &self.provider_id
    }

    #[must_use]
    pub fn binary(&self) -> &str {
        &self.binary
    }

    #[must_use]
    pub fn execution(&self) -> &ProviderExecution {
        &self.execution
    }

    #[must_use]
    pub fn provider_command_prefix(&self) -> &[String] {
        &self.provider_command_prefix
    }
}

/// Producer-owned identity for the ASP executable used by a built-in runtime.
///
/// Activation materialization consumes this selection without reopening or
/// hashing the executable.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeBinarySelectionV1 {
    binary: String,
    content_digest: String,
    artifact_metadata_digest: String,
}

impl RuntimeBinarySelectionV1 {
    pub fn new(
        binary: String,
        content_digest: String,
        artifact_metadata_digest: String,
    ) -> Result<Self, String> {
        if binary.is_empty() {
            return Err("runtime binary selection requires a non-empty binary path".to_string());
        }
        if content_digest.is_empty() {
            return Err("runtime binary selection requires a content digest".to_string());
        }
        if artifact_metadata_digest.is_empty() {
            return Err(
                "runtime binary selection requires an artifact metadata digest".to_string(),
            );
        }
        Ok(Self {
            binary,
            content_digest,
            artifact_metadata_digest,
        })
    }

    #[must_use]
    pub fn binary(&self) -> &str {
        &self.binary
    }

    #[must_use]
    pub fn content_digest(&self) -> &str {
        &self.content_digest
    }

    #[must_use]
    pub fn artifact_metadata_digest(&self) -> &str {
        &self.artifact_metadata_digest
    }
}

/// Complete typed producer input for default activation materialization.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DefaultActivationSelections {
    providers: Vec<ProviderCommandSelection>,
    graph_turbo: RuntimeBinarySelectionV1,
}

impl DefaultActivationSelections {
    #[must_use]
    pub fn new(
        providers: Vec<ProviderCommandSelection>,
        graph_turbo: RuntimeBinarySelectionV1,
    ) -> Self {
        Self {
            providers,
            graph_turbo,
        }
    }

    #[must_use]
    pub fn providers(&self) -> &[ProviderCommandSelection] {
        &self.providers
    }

    #[must_use]
    pub fn graph_turbo(&self) -> &RuntimeBinarySelectionV1 {
        &self.graph_turbo
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProviderCommandSelectionScopeV1 {
    TargetLanguage(agent_semantic_config::LanguageId),
    TargetProviderId(agent_semantic_config::ProviderId),
    CompleteGeneration,
}

impl ProviderCommandSelectionScopeV1 {
    fn selects(
        &self,
        language_id: &agent_semantic_config::LanguageId,
        provider_id: &agent_semantic_config::ProviderId,
    ) -> bool {
        match self {
            Self::TargetLanguage(target) => target == language_id,
            Self::TargetProviderId(target) => target == provider_id,
            Self::CompleteGeneration => true,
        }
    }
}

pub fn provider_command_selections(
    project_root: &Path,
) -> Result<Vec<ProviderCommandSelection>, String> {
    provider_command_selections_for_scope(
        project_root,
        &ProviderCommandSelectionScopeV1::CompleteGeneration,
    )
}

pub fn default_activation_selections(
    project_root: &Path,
) -> Result<DefaultActivationSelections, String> {
    default_activation_selections_for_scope(
        project_root,
        &ProviderCommandSelectionScopeV1::CompleteGeneration,
        None,
    )
}

pub fn default_activation_selections_with_state_home(
    project_root: &Path,
    state_home: &Path,
) -> Result<DefaultActivationSelections, String> {
    default_activation_selections_for_scope_with_state_home(
        project_root,
        state_home,
        &ProviderCommandSelectionScopeV1::CompleteGeneration,
        None,
    )
}

pub fn default_activation_selections_for_scope(
    project_root: &Path,
    scope: &ProviderCommandSelectionScopeV1,
    activation_path: Option<&Path>,
) -> Result<DefaultActivationSelections, String> {
    let state_paths = agent_semantic_runtime::project_state_paths(project_root)
        .map_err(|error| format!("failed to resolve ASP project state paths: {error}"))?;
    default_activation_selections_for_scope_with_paths(
        project_root,
        &state_paths,
        scope,
        activation_path,
    )
}

pub fn default_activation_selections_for_scope_with_state_home(
    project_root: &Path,
    state_home: &Path,
    scope: &ProviderCommandSelectionScopeV1,
    activation_path: Option<&Path>,
) -> Result<DefaultActivationSelections, String> {
    let state_paths =
        agent_semantic_runtime::project_state_paths_with_state_home(project_root, state_home)
            .map_err(|error| format!("failed to resolve ASP project state paths: {error}"))?;
    default_activation_selections_for_scope_with_paths(
        project_root,
        &state_paths,
        scope,
        activation_path,
    )
}

fn default_activation_selections_for_scope_with_paths(
    project_root: &Path,
    state_paths: &agent_semantic_runtime::state::ProjectStatePaths,
    scope: &ProviderCommandSelectionScopeV1,
    activation_path: Option<&Path>,
) -> Result<DefaultActivationSelections, String> {
    default_activation_selections_for_scope_with_context(
        project_root,
        state_paths,
        scope,
        activation_path,
        None,
    )
}

fn default_activation_selections_for_scope_with_context(
    project_root: &Path,
    state_paths: &agent_semantic_runtime::state::ProjectStatePaths,
    scope: &ProviderCommandSelectionScopeV1,
    activation_path: Option<&Path>,
    asp_binary: Option<&Path>,
) -> Result<DefaultActivationSelections, String> {
    let providers =
        provider_command_selections_for_scope_with_paths(project_root, state_paths, scope)?;
    let graph_turbo = match asp_binary {
        Some(binary) => capture_asp_binary_selection(binary, activation_path)?,
        None => capture_current_asp_binary_selection(activation_path)?,
    };
    Ok(DefaultActivationSelections::new(providers, graph_turbo))
}

pub fn default_activation_selections_with_state_home_and_binary(
    project_root: &Path,
    state_home: &Path,
    asp_binary: &Path,
    activation_path: Option<&Path>,
) -> Result<DefaultActivationSelections, String> {
    let state_paths =
        agent_semantic_runtime::project_state_paths_with_state_home(project_root, state_home)
            .map_err(|error| format!("failed to resolve ASP project state paths: {error}"))?;
    default_activation_selections_for_scope_with_context(
        project_root,
        &state_paths,
        &ProviderCommandSelectionScopeV1::CompleteGeneration,
        activation_path,
        Some(asp_binary),
    )
}

pub fn provider_command_selections_for_scope(
    project_root: &Path,
    scope: &ProviderCommandSelectionScopeV1,
) -> Result<Vec<ProviderCommandSelection>, String> {
    let state_paths = agent_semantic_runtime::project_state_paths(project_root)
        .map_err(|error| format!("failed to resolve ASP project state paths: {error}"))?;
    provider_command_selections_for_scope_with_paths(project_root, &state_paths, scope)
}

pub fn provider_command_selections_for_scope_with_state_home(
    project_root: &Path,
    state_home: &Path,
    scope: &ProviderCommandSelectionScopeV1,
) -> Result<Vec<ProviderCommandSelection>, String> {
    let state_paths =
        agent_semantic_runtime::project_state_paths_with_state_home(project_root, state_home)
            .map_err(|error| format!("failed to resolve ASP project state paths: {error}"))?;
    provider_command_selections_for_scope_with_paths(project_root, &state_paths, scope)
}

pub(crate) fn provider_command_selections_for_scope_with_paths(
    project_root: &Path,
    state_paths: &agent_semantic_runtime::state::ProjectStatePaths,
    scope: &ProviderCommandSelectionScopeV1,
) -> Result<Vec<ProviderCommandSelection>, String> {
    let project_config = ProjectProviderConfigSet::load(project_root)?;
    let mut providers = Vec::new();
    for manifest in provider_manifests() {
        if crate::provider_registry::registered_provider_kind(manifest.language_id.as_str())?
            == crate::provider_registry::RegisteredProviderKind::Document
        {
            continue;
        }
        if !scope.selects(&manifest.language_id, &manifest.provider_id) {
            continue;
        }
        let Some(provider_config) = project_config.provider_config(manifest.language_id.as_str())
        else {
            continue;
        };
        let required = !matches!(scope, ProviderCommandSelectionScopeV1::CompleteGeneration);
        let Some(resolved_command) = provider_command_prefix(
            &manifest,
            provider_config,
            &state_paths.runtime_bin_dir,
            required,
        )?
        else {
            continue;
        };
        let ResolvedProviderCommand {
            binary,
            command_prefix,
        } = resolved_command;
        let executable = command_prefix
            .first()
            .ok_or_else(|| {
                format!(
                    "provider `{}` language `{}` resolved an empty command prefix",
                    manifest.provider_id, manifest.language_id
                )
            })
            .map(PathBuf::from)?;
        let artifact_digest = crate::active_artifact_receipt::installed_provider_artifact_digest(
            &state_paths.provider_lock_dir,
            &manifest.language_id,
            &manifest.provider_id,
            executable,
        )?;
        providers.push(ProviderCommandSelection {
            manifest_id: manifest.manifest_id.clone(),
            manifest_digest: provider_manifest_digest(&manifest)
                .map_err(|error| format!("failed to digest provider manifest: {error:?}"))?,
            execution_command_digest:
                crate::protocol_activation::digest::provider_execution_command_digest(
                    &command_prefix,
                    &artifact_digest,
                )?,
            language_id: manifest.language_id.clone(),
            provider_id: manifest.provider_id.clone(),
            binary,
            execution: manifest.execution,
            provider_command_prefix: command_prefix,
        });
    }
    if providers.is_empty() {
        return Err(match scope {
            ProviderCommandSelectionScopeV1::TargetLanguage(language_id) => format!(
                "requested language provider is unavailable: languageId={} runtimeBin={}",
                language_id.as_str(),
                state_paths.runtime_bin_dir.display()
            ),
            ProviderCommandSelectionScopeV1::TargetProviderId(provider_id) => format!(
                "requested provider is unavailable: providerId={} runtimeBin={}",
                provider_id.as_str(),
                state_paths.runtime_bin_dir.display()
            ),
            ProviderCommandSelectionScopeV1::CompleteGeneration => format!(
                "expected State Home runtime bin to contain at least one executable semantic provider binary: {}",
                state_paths.runtime_bin_dir.display()
            ),
        });
    }
    Ok(providers)
}

pub fn project_agent_config_path(project_root: &Path) -> PathBuf {
    project_root.join(".agents").join("asp.toml")
}

#[path = "provider_manifest_contract.rs"]
mod manifest_contract;
pub use manifest_contract::validate_provider_manifest_contract;
use manifest_contract::validate_source_snapshot_capability;

pub(crate) fn activate_provider(
    manifest: &ProviderManifest,
    manifest_digest: String,
    execution_command_digest: String,
    binary: String,
    coverage: ActivationCoverage,
    semantic_registry_digest: &str,
) -> Result<ActivatedProviderConfig, String> {
    validate_source_snapshot_capability(
        manifest.language_id.as_str(),
        &manifest.search_capabilities,
    )?;
    let routes_started = std::time::Instant::now();
    let routes = crate::provider_registry::materialize_provider_routes(manifest)?;
    if std::env::var_os("ASP_HOOK_INSTALL_TIMINGS").is_some() {
        eprintln!(
            "[activation-timing] step=provider-routes language={} stepMs={:.3}",
            manifest.language_id,
            routes_started.elapsed().as_secs_f64() * 1_000.0
        );
    }
    Ok(ActivatedProviderConfig {
        search_capabilities: manifest.search_capabilities.clone(),
        semantic_facts_descriptor: manifest.semantic_facts_descriptor.clone(),
        query_pack_descriptor: manifest.query_pack_descriptor.clone(),
        manifest_id: manifest.manifest_id.clone(),
        manifest_digest,
        language_id: manifest.language_id.clone(),
        provider_id: manifest.provider_id.clone(),
        binary,
        execution: manifest.execution,
        execution_command_digest,
        provider_command_prefix: Vec::new(),
        semantic_registry_digest: semantic_registry_digest.to_string(),
        routes,
        coverage,
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

struct ResolvedProviderCommand {
    binary: String,
    command_prefix: Vec<String>,
}

fn provider_command_prefix(
    manifest: &ProviderManifest,
    config: &ProjectProviderConfig,
    managed_bin_dir: &Path,
    required: bool,
) -> Result<Option<ResolvedProviderCommand>, String> {
    let configured_binary = config.binary.as_deref().unwrap_or(&manifest.binary);
    let configured_path = Path::new(configured_binary);
    if configured_path.components().count() != 1
        || configured_path.file_name().and_then(|name| name.to_str()) != Some(configured_binary)
    {
        return Err(format!(
            "provider `{}` language `{}` binary must be a logical basename resolved under State Home runtime/bin, got `{configured_binary}`",
            manifest.provider_id, manifest.language_id
        ));
    }
    let provider_binary = managed_bin_dir.join(configured_path);
    let provider_binary = provider_binary.to_str().ok_or_else(|| {
        format!(
            "provider `{}` language `{}` State Home binary path is not valid UTF-8: {}",
            manifest.provider_id,
            manifest.language_id,
            provider_binary.display()
        )
    })?;
    let resolution = resolve_executable_with_status(provider_binary);
    let Some(path) = resolution.path else {
        if required {
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
    Ok(Some(ResolvedProviderCommand {
        binary: configured_binary.to_string(),
        command_prefix: vec![path.display().to_string()],
    }))
}
