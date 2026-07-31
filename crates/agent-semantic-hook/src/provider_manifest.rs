//! Built-in provider manifests and default project activations.

use serde::Deserialize;
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use crate::executable::resolve_executable_with_status;
use crate::protocol::{
    HOOK_ACTIVATION_SCHEMA_ID, HOOK_ACTIVATION_SCHEMA_VERSION, HOOK_PROTOCOL_ID,
    HOOK_PROTOCOL_VERSION,
};
use crate::protocol_activation::digest::provider_manifest_digest;
use crate::protocol_activation::protocol_activation_manifest::{
    ActivatedProviderConfig, ActivatedRankerConfig, ActivationCoverage, ActivationGeneratedBy,
    HookActivation, ProviderExecution, ProviderManifest,
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
    let selections = default_activation_selections(project_root)?;
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
    let repository_candidates =
        agent_semantic_runtime::git::discover_repository_candidate_snapshot(project_root)
            .map_err(|error| format!("discover activation repository candidates: {error}"))?
            .ok_or_else(|| {
                format!(
                    "provider activation requires a Git candidate snapshot: workspace={}",
                    project_root.display()
                )
            })?;
    let mut providers = Vec::new();
    for (manifest, selection) in selected_providers {
        if !provider_applies_to_repository_candidates(manifest, &repository_candidates) {
            continue;
        }
        let coverage =
            resolve_activation_coverage(project_root, manifest, selection, &repository_candidates)?;
        providers.push(activate_provider(
            manifest,
            selection.manifest_digest.clone(),
            selection.execution_command_digest.clone(),
            selection.binary.clone(),
            coverage,
            &semantic_registry_digest,
        )?);
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

fn provider_applies_to_repository_candidates(
    manifest: &ProviderManifest,
    snapshot: &agent_semantic_runtime::git::RepositoryCandidateSnapshot,
) -> bool {
    let candidate_paths = snapshot
        .candidates
        .iter()
        .filter_map(|candidate| candidate.path.to_str())
        .collect::<Vec<_>>();
    provider_applies_to_candidate_paths(manifest, &candidate_paths)
}

fn provider_applies_to_candidate_paths(
    manifest: &ProviderManifest,
    candidate_paths: &[&str],
) -> bool {
    if let Some(descriptor) = manifest.project_resolution() {
        candidate_paths.iter().any(|candidate| {
            descriptor.entry_markers.iter().any(|marker| {
                *candidate == marker.as_str()
                    || candidate
                        .strip_suffix(marker.as_str())
                        .is_some_and(|prefix| prefix.ends_with('/'))
            })
        })
    } else if let Some(descriptor) = manifest.document_resolution() {
        candidate_paths.iter().any(|candidate| {
            descriptor
                .extensions
                .iter()
                .any(|extension| candidate.ends_with(extension))
        })
    } else {
        false
    }
}

#[cfg(test)]
mod provider_candidate_applicability_tests {
    use super::{provider_applies_to_candidate_paths, provider_manifests};

    #[test]
    fn project_provider_requires_a_git_candidate_entry_marker() {
        let manifests = provider_manifests();
        let typescript = manifests
            .iter()
            .find(|manifest| manifest.provider_id().as_str() == "ts-harness")
            .expect("typescript provider manifest");

        assert!(!provider_applies_to_candidate_paths(
            typescript,
            &["Cargo.toml", "src/lib.rs"],
        ));
        assert!(provider_applies_to_candidate_paths(
            typescript,
            &["apps/web/package.json", "apps/web/src/index.ts"],
        ));
    }

    #[test]
    fn document_provider_requires_a_git_candidate_document_extension() {
        let manifests = provider_manifests();
        let org = manifests
            .iter()
            .find(|manifest| manifest.language_id().as_str() == "org")
            .expect("org provider manifest");

        assert!(!provider_applies_to_candidate_paths(
            org,
            &["Cargo.toml", "src/lib.rs"],
        ));
        assert!(provider_applies_to_candidate_paths(
            org,
            &["docs/design.org"],
        ));
    }
}

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
    if let Some(selection) = activation_path.and_then(|activation_path| {
        reuse_asp_binary_selection_from_active_receipt(&binary, activation_path)
    }) {
        return Ok(selection);
    }
    let content_digest =
        agent_semantic_content_identity::file_content_digest_v1(&binary)?.to_string();
    let artifact_metadata_digest =
        agent_semantic_content_identity::file_artifact_metadata_digest_v1(&binary)?.to_string();
    RuntimeBinarySelectionV1::new(
        binary.display().to_string(),
        content_digest,
        artifact_metadata_digest,
    )
}

fn reuse_asp_binary_selection_from_active_receipt(
    binary: &Path,
    activation_path: &Path,
) -> Option<RuntimeBinarySelectionV1> {
    let receipt = crate::verify_active_asp_artifact_receipt(activation_path, &[binary]).ok()?;
    let artifact_metadata_digest =
        agent_semantic_content_identity::file_artifact_metadata_digest_v1(binary).ok()?;
    RuntimeBinarySelectionV1::new(
        binary.display().to_string(),
        receipt
            .asp_binary_leaf()
            .artifact_digest()
            .as_str()
            .to_string(),
        artifact_metadata_digest.to_string(),
    )
    .ok()
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

pub fn default_activation_selections_for_scope(
    project_root: &Path,
    scope: &ProviderCommandSelectionScopeV1,
    activation_path: Option<&Path>,
) -> Result<DefaultActivationSelections, String> {
    let providers = provider_command_selections_for_scope(project_root, scope)?;
    let graph_turbo = capture_current_asp_binary_selection(activation_path)?;
    Ok(DefaultActivationSelections::new(providers, graph_turbo))
}

pub fn provider_command_selections_for_scope(
    project_root: &Path,
    scope: &ProviderCommandSelectionScopeV1,
) -> Result<Vec<ProviderCommandSelection>, String> {
    let project_config = ProjectProviderConfigSet::load(project_root)?;
    let state_paths = agent_semantic_runtime::project_state_paths(project_root)
        .map_err(|error| format!("failed to resolve ASP project state paths: {error}"))?;
    let mut providers = Vec::new();
    for manifest in provider_manifests() {
        if !scope.selects(&manifest.language_id, &manifest.provider_id) {
            continue;
        }
        let Some(provider_config) = project_config.provider_config(manifest.language_id.as_str())
        else {
            continue;
        };
        let Some(resolved_command) =
            provider_command_prefix(&manifest, provider_config, &state_paths.runtime_bin_dir)?
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

pub fn validate_provider_manifest_contract(manifest: &ProviderManifest) -> Vec<String> {
    let mut errors = Vec::new();
    if manifest.language_id.is_empty() {
        errors.push("provider manifest languageId must be non-empty".to_string());
    }
    if manifest.provider_id.is_empty() {
        errors.push("provider manifest providerId must be non-empty".to_string());
    }

    if let Err(error) =
        crate::protocol_activation::provider_query_pack::validate_query_pack_descriptor(manifest)
    {
        errors.push(error.to_string());
    }
    if let Err(error) =
        crate::protocol_activation::provider_query_pack::validate_semantic_facts_descriptor(
            manifest,
        )
    {
        errors.push(error.to_string());
    }
    if let Err(error) = validate_source_snapshot_capability(
        manifest.language_id.as_str(),
        &manifest.search_capabilities,
    ) {
        errors.push(error);
    }
    if let Err(error) = validate_project_resolution_descriptor(manifest) {
        errors.push(error);
    }

    errors
}

fn validate_project_resolution_descriptor(manifest: &ProviderManifest) -> Result<(), String> {
    let Some(descriptor) = manifest.project_resolution() else {
        return validate_document_resolution_descriptor(manifest);
    };
    if manifest.document_resolution().is_some() {
        return Err(format!(
            "provider {} must declare exactly one of projectResolution or documentResolution",
            manifest.provider_id()
        ));
    }
    if descriptor.schema_id != "agent.semantic-protocols.provider-project-resolution-descriptor"
        || descriptor.schema_version != "1"
    {
        return Err(format!(
            "provider {} projectResolution schema must be agent.semantic-protocols.provider-project-resolution-descriptor v1",
            manifest.provider_id()
        ));
    }
    if descriptor.capability_id != "project-resolution" {
        return Err(format!(
            "provider {} projectResolution capabilityId must be project-resolution",
            manifest.provider_id()
        ));
    }
    if !descriptor.supports_git_candidates && !descriptor.supports_provider_only {
        return Err(format!(
            "provider {} projectResolution must support at least one candidate mode",
            manifest.provider_id()
        ));
    }
    if descriptor.entry_markers.is_empty()
        || descriptor
            .entry_markers
            .iter()
            .any(|marker| marker.is_empty())
    {
        return Err(format!(
            "provider {} projectResolution entryMarkers must be non-empty",
            manifest.provider_id()
        ));
    }
    if descriptor.parser_id.is_empty() || descriptor.command_binding != "project-resolution-stdin" {
        return Err(format!(
            "provider {} projectResolution requires a non-empty parserId and commandBinding=project-resolution-stdin",
            manifest.provider_id()
        ));
    }
    for (field, actual, expected) in [
        (
            "candidateSnapshotSchema",
            descriptor.candidate_snapshot_schema.as_str(),
            "https://schemas.agent-semantic-protocols.dev/repository-candidate-snapshot.v1.schema.json",
        ),
        (
            "packageGraphSchema",
            descriptor.package_graph_schema.as_str(),
            "https://schemas.agent-semantic-protocols.dev/language-package-graph.v1.schema.json",
        ),
        (
            "resolvedSourceScopeSchema",
            descriptor.resolved_source_scope_schema.as_str(),
            "https://schemas.agent-semantic-protocols.dev/resolved-source-scope.v1.schema.json",
        ),
        (
            "projectResolutionSchema",
            descriptor.project_resolution_schema.as_str(),
            "https://schemas.agent-semantic-protocols.dev/project-resolution.v1.schema.json",
        ),
    ] {
        if actual != expected {
            return Err(format!(
                "provider {} projectResolution {field} must be {expected}",
                manifest.provider_id()
            ));
        }
    }
    Ok(())
}

fn validate_document_resolution_descriptor(manifest: &ProviderManifest) -> Result<(), String> {
    let descriptor = manifest.document_resolution().ok_or_else(|| {
        format!(
            "provider {} must declare exactly one of projectResolution or documentResolution",
            manifest.provider_id()
        )
    })?;
    if descriptor.schema_id != "agent.semantic-protocols.provider-document-resolution-descriptor"
        || descriptor.schema_version != "1"
        || descriptor.capability_id != "document-resolution"
        || !descriptor.supports_git_candidates
        || descriptor.parser_id.is_empty()
        || descriptor.extensions.is_empty()
        || descriptor
            .extensions
            .iter()
            .any(|extension| !extension.starts_with('.'))
    {
        return Err(format!(
            "provider {} has an invalid documentResolution descriptor",
            manifest.provider_id()
        ));
    }
    Ok(())
}

fn validate_source_snapshot_capability(
    language_id: &str,
    search_capabilities: &crate::protocol_activation::protocol_activation_manifest::ProviderSearchCapabilities,
) -> Result<(), String> {
    let descriptor = search_capabilities
        .source_snapshot
        .as_ref()
        .ok_or_else(|| {
            format!(
                "provider `{language_id}` is missing required searchCapabilities.sourceSnapshot descriptor"
            )
        })?;
    let descriptor_id = (!descriptor.descriptor_id().is_empty())
        .then_some(descriptor.descriptor_id())
        .ok_or_else(|| format!("provider `{language_id}` source snapshot descriptorId is empty"))?;
    for (field, actual, expected) in [
        (
            "descriptorVersion",
            descriptor.descriptor_version(),
            "1",
        ),
        ("languageId", descriptor.language_id(), language_id),
        (
            "packetSchemaId",
            descriptor.packet_schema_id(),
            "asp.source-snapshot.v1",
        ),
        (
            "exactSourcePacketSchemaId",
            descriptor.exact_source_packet_schema_id(),
            "asp.exact-source-query-result.v1",
        ),
        (
            "canonicalItemSelectorSchemaId",
            descriptor.canonical_item_selector_schema_id(),
            agent_semantic_content_identity::canonical_item_identity::CANONICAL_ITEM_SELECTOR_SCHEMA_ID,
        ),
        (
            "sourceSnapshotEnvelopeSchemaId",
            descriptor.source_snapshot_envelope_schema_id(),
            "asp.exact-source-snapshot-envelope.v1",
        ),
        (
            "derivedArtifactEvidenceSchemaId",
            descriptor.derived_artifact_evidence_schema_id(),
            "asp.derived-source-artifact-evidence.v1",
        ),
        (
            "algorithm",
            descriptor.algorithm(),
            "blake3-merkle-v1",
        ),
        ("authority", descriptor.authority(), "live-parser"),
        (
            "exactSelectorResolution",
            descriptor.exact_selector_resolution(),
            "pinned-live-module-graph",
        ),
        (
            "overlayMode",
            descriptor.overlay_mode(),
            "merkle-delta",
        ),
    ] {
        if actual != expected {
            return Err(format!(
                "provider `{language_id}` source snapshot descriptor `{descriptor_id}` requires {field}={expected}, got {actual}"
            ));
        }
    }
    Ok(())
}

fn resolve_document_activation_coverage(
    manifest: &ProviderManifest,
    snapshot: &agent_semantic_runtime::git::RepositoryCandidateSnapshot,
) -> Result<ActivationCoverage, String> {
    let descriptor = manifest.document_resolution().ok_or_else(|| {
        format!(
            "provider {} omitted documentResolution",
            manifest.provider_id()
        )
    })?;
    let candidate_generation = snapshot.candidate_generation.digest.clone();
    let snapshot = serde_json::to_value(snapshot)
        .map_err(|error| format!("encode document repository candidates: {error}"))?;
    let source_paths = snapshot
        .get("candidates")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| "repository candidate snapshot omitted candidates".to_string())?
        .iter()
        .filter_map(|candidate| candidate.get("path").and_then(serde_json::Value::as_str))
        .filter(|path| {
            descriptor
                .extensions
                .iter()
                .any(|extension| path.ends_with(extension))
        })
        .map(str::to_string)
        .collect::<std::collections::BTreeSet<_>>();
    if source_paths.is_empty() {
        return Err(format!(
            "document resolution found no Git candidate documents: providerId={}",
            manifest.provider_id()
        ));
    }
    Ok(ActivationCoverage {
        package_roots: Vec::new(),
        config_files: Vec::new(),
        source_extensions: descriptor.extensions.clone(),
        source_paths: source_paths.into_iter().collect(),
        repository_candidate_generation: candidate_generation.clone(),
        project_resolution_generation: format!(
            "document:{}:{}",
            candidate_generation, descriptor.parser_id
        ),
    })
}

fn resolve_activation_coverage(
    project_root: &Path,
    manifest: &ProviderManifest,
    selection: &ProviderCommandSelection,
    snapshot: &agent_semantic_runtime::git::RepositoryCandidateSnapshot,
) -> Result<ActivationCoverage, String> {
    use std::io::Write as _;
    use std::process::Stdio;

    if manifest.document_resolution().is_some() {
        return resolve_document_activation_coverage(manifest, snapshot);
    }
    let descriptor = manifest.project_resolution().ok_or_else(|| {
        format!(
            "provider {} omitted projectResolution",
            manifest.provider_id()
        )
    })?;
    let repository_candidate_generation = snapshot.candidate_generation.digest.clone();
    let snapshot_value = serde_json::to_value(&snapshot)
        .map_err(|error| format!("encode repository candidate snapshot: {error}"))?;
    let request = serde_json::to_vec(&serde_json::json!({
        "schemaId": "agent.semantic-protocols.provider-project-resolution-request",
        "schemaVersion": "1",
        "languageId": manifest.language_id(),
        "providerId": manifest.provider_id(),
        "workspaceRoot": project_root,
        "repositoryCandidates": snapshot_value,
    }))
    .map_err(|error| format!("encode provider project-resolution request: {error}"))?;

    let executable = selection.provider_command_prefix.first().ok_or_else(|| {
        format!(
            "provider {} resolved an empty command prefix",
            manifest.provider_id()
        )
    })?;
    let mut command = std::process::Command::new(executable);
    command
        .args(selection.provider_command_prefix.iter().skip(1))
        .arg(&descriptor.command_binding)
        .current_dir(project_root)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = command.spawn().map_err(|error| {
        format!(
            "spawn provider project-resolution command for {}: {error}",
            manifest.provider_id()
        )
    })?;
    child
        .stdin
        .take()
        .ok_or_else(|| {
            format!(
                "provider project-resolution stdin unavailable for {}",
                manifest.provider_id()
            )
        })?
        .write_all(&request)
        .map_err(|error| {
            format!(
                "write provider project-resolution request for {}: {error}",
                manifest.provider_id()
            )
        })?;
    let output = child.wait_with_output().map_err(|error| {
        format!(
            "wait for provider project-resolution command for {}: {error}",
            manifest.provider_id()
        )
    })?;
    if !output.status.success() {
        return Err(format!(
            "provider project-resolution failed: languageId={} providerId={} status={} stderr={}",
            manifest.language_id(),
            manifest.provider_id(),
            output.status,
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    let response: serde_json::Value = serde_json::from_slice(&output.stdout).map_err(|error| {
        format!(
            "decode provider project-resolution response for {}: {error}",
            manifest.provider_id()
        )
    })?;
    let response_string = |field: &str| {
        response
            .get(field)
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| {
                format!(
                    "provider project-resolution response omitted string field {field}: providerId={}",
                    manifest.provider_id()
                )
            })
    };
    if response_string("schemaId")?
        != "agent.semantic-protocols.provider-project-resolution-response"
        || response_string("schemaVersion")? != "1"
        || response_string("state")? != "resolved"
        || response_string("languageId")? != manifest.language_id().as_str()
        || response_string("providerId")? != manifest.provider_id().as_str()
    {
        return Err(format!(
            "provider project-resolution response identity/state mismatch: providerId={}",
            manifest.provider_id()
        ));
    }
    let resolution = response
        .get("resolution")
        .and_then(serde_json::Value::as_object)
        .ok_or_else(|| {
            format!(
                "resolved provider project-resolution omitted resolution: providerId={}",
                manifest.provider_id()
            )
        })?;
    let resolution_string = |field: &str| {
        resolution
            .get(field)
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| {
                format!(
                    "project-resolution omitted string field {field}: providerId={}",
                    manifest.provider_id()
                )
            })
    };
    if resolution_string("schemaId")? != "agent.semantic-protocols.project-resolution"
        || resolution_string("schemaVersion")? != "1"
        || resolution_string("state")? != "resolved"
        || resolution_string("completeness")? != "exact"
    {
        return Err(format!(
            "provider project-resolution contract is not exact/resolved: providerId={}",
            manifest.provider_id()
        ));
    }

    let candidate_paths = snapshot_value
        .get("candidates")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| "repository candidate snapshot omitted candidates".to_string())?
        .iter()
        .filter_map(|candidate| candidate.get("path").and_then(serde_json::Value::as_str))
        .collect::<Vec<_>>();
    let scopes = resolution
        .get("resolvedSourceScopes")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| "project-resolution omitted resolvedSourceScopes".to_string())?;
    let mut package_roots = std::collections::BTreeSet::new();
    let mut source_extensions = std::collections::BTreeSet::new();
    let mut source_paths = std::collections::BTreeSet::new();
    for scope in scopes {
        let roots = scope
            .get("roots")
            .and_then(serde_json::Value::as_array)
            .ok_or_else(|| "resolved source scope omitted roots".to_string())?
            .iter()
            .filter_map(serde_json::Value::as_str)
            .collect::<Vec<_>>();
        let extensions = scope
            .get("extensions")
            .and_then(serde_json::Value::as_array)
            .ok_or_else(|| "resolved source scope omitted extensions".to_string())?
            .iter()
            .filter_map(serde_json::Value::as_str)
            .collect::<Vec<_>>();
        let exclusions = scope
            .get("exclusions")
            .and_then(serde_json::Value::as_array)
            .ok_or_else(|| "resolved source scope omitted exclusions".to_string())?
            .iter()
            .filter_map(|exclusion| exclusion.get("prefix").and_then(serde_json::Value::as_str))
            .collect::<Vec<_>>();
        if roots.is_empty() || extensions.is_empty() {
            return Err(format!(
                "resolved source scope must declare roots and extensions: providerId={}",
                manifest.provider_id()
            ));
        }
        package_roots.extend(roots.iter().map(|root| (*root).to_string()));
        source_extensions.extend(extensions.iter().map(|extension| (*extension).to_string()));
        for candidate in &candidate_paths {
            let within_root = roots.iter().any(|root| {
                *root == "."
                    || *candidate == *root
                    || candidate
                        .strip_prefix(root)
                        .is_some_and(|suffix| suffix.starts_with('/'))
            });
            let has_extension = extensions
                .iter()
                .any(|extension| candidate.ends_with(extension));
            let excluded = exclusions.iter().any(|prefix| {
                *candidate == *prefix
                    || candidate
                        .strip_prefix(prefix)
                        .is_some_and(|suffix| suffix.starts_with('/'))
            });
            if within_root && has_extension && !excluded {
                source_paths.insert((*candidate).to_string());
            }
        }
    }
    if source_paths.is_empty() {
        return Err(format!(
            "provider project-resolution resolved no Git candidate source files: providerId={}",
            manifest.provider_id()
        ));
    }
    let project_entry = resolution
        .get("projectIdentity")
        .and_then(|identity| identity.get("projectEntry"))
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| "project-resolution omitted projectIdentity.projectEntry".to_string())?;
    Ok(ActivationCoverage {
        package_roots: package_roots.into_iter().collect(),
        config_files: vec![project_entry.to_string()],
        source_extensions: source_extensions.into_iter().collect(),
        source_paths: source_paths.into_iter().collect(),
        repository_candidate_generation,
        project_resolution_generation: resolution_string("resolutionGeneration")?.to_string(),
    })
}

fn activate_provider(
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
    Ok(Some(ResolvedProviderCommand {
        binary: configured_binary.to_string(),
        command_prefix: vec![path.display().to_string()],
    }))
}
