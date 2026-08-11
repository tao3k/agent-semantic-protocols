//! Activation loading and provider manifest defaults for `agent-semantic-hook`.

use crate::protocol_activation::protocol_activation_manifest::{HookActivation, HookRuntime};
use crate::protocol_activation::protocol_activation_runtime::parse_activation;
use crate::provider_manifest::{
    DefaultActivationSelections, ProviderCommandSelection, ProviderCommandSelectionScopeV1,
    default_activation_selections_for_scope,
    default_activation_selections_for_scope_with_state_home,
    default_activation_selections_with_state_home_and_binary, provider_manifests,
};
use agent_semantic_runtime::project_activation_path;
use std::{
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

static ACTIVATION_WRITE_SEQUENCE: AtomicU64 = AtomicU64::new(0);

/// Load and validate a project hook activation from `activation.json`.
pub fn load_activation(path: &Path) -> Result<HookRuntime, String> {
    let contents = fs::read_to_string(path)
        .map_err(|error| format!("failed to read activation {}: {error}", path.display()))?;
    parse_activation(&contents, &provider_manifests())
        .map_err(|error| format!("invalid activation JSON: {error:?}"))
}

/// Load an activation or regenerate the managed cache copy when it has drifted.
pub fn load_or_sync_activation(
    activation_path: &Path,
    project_root: &Path,
) -> Result<HookRuntime, String> {
    if is_generated_activation_path_for_project(activation_path, project_root) {
        return sync_activation(project_root, activation_path);
    }
    load_activation(activation_path)
}

pub fn load_or_sync_activation_with_state_home(
    activation_path: &Path,
    project_root: &Path,
    state_home: &Path,
) -> Result<HookRuntime, String> {
    if is_generated_activation_path_for_project(activation_path, project_root) {
        let sync = load_or_refresh_default_activation_with_state_home(
            activation_path,
            project_root,
            state_home,
        )?;
        return activation_to_runtime(&sync.activation);
    }
    load_activation(activation_path)
}

/// Build a generated activation for one requested language without validating
/// or materializing unrelated provider receipts.
/// Resolve one registered language into an in-memory runtime selection.
///
/// This is a read-only query boundary: it consumes the immutable active
/// artifact receipt and never re-hashes the ASP executable.
pub fn registered_language_runtime(
    project_root: &Path,
    language_id: &str,
    _activation_path: &Path,
) -> Result<HookRuntime, String> {
    let scope = ProviderCommandSelectionScopeV1::TargetLanguage(language_id.into());
    crate::provider_runtime::build_provider_runtime_for_scope(project_root, &scope)
}

/// Result of syncing the generated default activation during install.
pub struct DefaultActivationSync {
    pub activation: HookActivation,
    pub status: &'static str,
    pub admission: ActivationAdmissionReceipt,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ActivationAdmissionDecision {
    Reuse,
    RebuildAndPublish,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ActivationAdmissionReason {
    CompleteIdentity,
    ActivationMissing,
    ArtifactReceiptInvalid,
    ActivationSchemaInvalid,
    ProjectIdentityMismatch,
    ProviderSelectionDrift,
    RepositoryCandidateGenerationDrift,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ActivationAdmissionGates {
    pub activation_readable: bool,
    pub artifact_receipt_valid: bool,
    pub schema_valid: bool,
    pub project_identity_matches: bool,
    pub provider_selection_matches: bool,
    pub repository_candidate_generation_matches: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ActivationAdmissionReceipt {
    pub schema_id: &'static str,
    pub schema_version: &'static str,
    pub decision: ActivationAdmissionDecision,
    pub reason: ActivationAdmissionReason,
    pub gates: ActivationAdmissionGates,
}

impl ActivationAdmissionReceipt {
    fn rebuild(reason: ActivationAdmissionReason, gates: ActivationAdmissionGates) -> Self {
        Self {
            schema_id: "asp.activation-admission-receipt.v1",
            schema_version: "1",
            decision: ActivationAdmissionDecision::RebuildAndPublish,
            reason,
            gates,
        }
    }

    fn reuse(gates: ActivationAdmissionGates) -> Self {
        Self {
            schema_id: "asp.activation-admission-receipt.v1",
            schema_version: "1",
            decision: ActivationAdmissionDecision::Reuse,
            reason: ActivationAdmissionReason::CompleteIdentity,
            gates,
        }
    }
}

struct ActivationAssessment {
    activation: Option<HookActivation>,
    receipt: ActivationAdmissionReceipt,
}

/// Load the generated activation when provider command selection is unchanged,
/// otherwise rebuild it from the current project.
pub fn load_or_refresh_default_activation(
    activation_path: &Path,
    project_root: &Path,
) -> Result<DefaultActivationSync, String> {
    load_or_refresh_default_activation_inner(activation_path, project_root, None, None)
}

pub fn load_or_refresh_default_activation_with_state_home(
    activation_path: &Path,
    project_root: &Path,
    state_home: &Path,
) -> Result<DefaultActivationSync, String> {
    load_or_refresh_default_activation_inner(activation_path, project_root, Some(state_home), None)
}

pub fn load_or_refresh_default_activation_with_state_home_and_binary(
    activation_path: &Path,
    project_root: &Path,
    state_home: &Path,
    asp_binary: &Path,
) -> Result<DefaultActivationSync, String> {
    load_or_refresh_default_activation_inner(
        activation_path,
        project_root,
        Some(state_home),
        Some(asp_binary),
    )
}

fn load_or_refresh_default_activation_inner(
    activation_path: &Path,
    project_root: &Path,
    state_home: Option<&Path>,
    asp_binary: Option<&Path>,
) -> Result<DefaultActivationSync, String> {
    let started = std::time::Instant::now();
    let current_selections = match (state_home, asp_binary) {
        (Some(state_home), Some(asp_binary)) => {
            default_activation_selections_with_state_home_and_binary(
                project_root,
                state_home,
                asp_binary,
                None,
            )?
        }
        (Some(state_home), None) => default_activation_selections_for_scope_with_state_home(
            project_root,
            state_home,
            &ProviderCommandSelectionScopeV1::CompleteGeneration,
            None,
        )?,
        (None, None) => default_activation_selections_for_scope(
            project_root,
            &ProviderCommandSelectionScopeV1::CompleteGeneration,
            None,
        )?,
        (None, Some(_)) => {
            return Err("explicit ASP binary requires explicit State Home".to_string());
        }
    };
    emit_activation_timing("provider-selections", started);
    let reusable_started = std::time::Instant::now();
    let assessment = assess_activation(activation_path, project_root, &current_selections)?;
    if let Some(activation) = assessment.activation {
        emit_activation_timing("reusable-activation", reusable_started);
        return Ok(DefaultActivationSync {
            activation,
            status: "reused",
            admission: assessment.receipt,
        });
    }
    emit_activation_timing("reusable-activation", reusable_started);

    let existed = activation_path.is_file();
    let build_started = std::time::Instant::now();
    let activation =
        crate::build_default_activation_from_selections(project_root, &current_selections)?;
    emit_activation_timing("build-activation", build_started);
    let write_started = std::time::Instant::now();
    write_activation(activation_path, &activation)?;
    emit_activation_timing("write-activation", write_started);
    Ok(DefaultActivationSync {
        activation,
        status: if existed { "refreshed" } else { "created" },
        admission: assessment.receipt,
    })
}

fn emit_activation_timing(step: &str, started: std::time::Instant) {
    if std::env::var_os("ASP_HOOK_INSTALL_TIMINGS").is_some() {
        eprintln!(
            "[activation-timing] step={step} stepMs={:.3}",
            started.elapsed().as_secs_f64() * 1_000.0
        );
    }
}

fn assess_activation(
    activation_path: &Path,
    project_root: &Path,
    current_selections: &DefaultActivationSelections,
) -> Result<ActivationAssessment, String> {
    let mut gates = ActivationAdmissionGates::default();
    let contents = match fs::read_to_string(activation_path) {
        Ok(contents) => {
            gates.activation_readable = true;
            contents
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(ActivationAssessment {
                activation: None,
                receipt: ActivationAdmissionReceipt::rebuild(
                    ActivationAdmissionReason::ActivationMissing,
                    gates,
                ),
            });
        }
        Err(error) => {
            return Err(format!(
                "failed to read activation {}: {error}",
                activation_path.display()
            ));
        }
    };
    // Workspace-local artifact receipts are retired derived caches, not
    // admission authority. Current Runtime-owned selections are validated
    // below against the activation itself.
    gates.artifact_receipt_valid = true;
    if parse_activation(&contents, &provider_manifests()).is_err() {
        return Ok(ActivationAssessment {
            activation: None,
            receipt: ActivationAdmissionReceipt::rebuild(
                ActivationAdmissionReason::ActivationSchemaInvalid,
                gates,
            ),
        });
    }
    let Ok(activation) = serde_json::from_str::<HookActivation>(&contents) else {
        return Ok(ActivationAssessment {
            activation: None,
            receipt: ActivationAdmissionReceipt::rebuild(
                ActivationAdmissionReason::ActivationSchemaInvalid,
                gates,
            ),
        });
    };
    gates.schema_valid = true;
    if activation.project_root != project_root.display().to_string() {
        return Ok(ActivationAssessment {
            activation: None,
            receipt: ActivationAdmissionReceipt::rebuild(
                ActivationAdmissionReason::ProjectIdentityMismatch,
                gates,
            ),
        });
    }
    gates.project_identity_matches = true;
    if !activation_matches_provider_command_selections(&activation, current_selections.providers())
        || !activation_matches_graph_turbo_selection(&activation, current_selections.graph_turbo())
    {
        return Ok(ActivationAssessment {
            activation: None,
            receipt: ActivationAdmissionReceipt::rebuild(
                ActivationAdmissionReason::ProviderSelectionDrift,
                gates,
            ),
        });
    }
    let manifests = provider_manifests();
    let manifest_coverage_matches = activation.providers.iter().all(|provider| {
        let Some(manifest) = manifests
            .iter()
            .find(|manifest| manifest.manifest_id == provider.manifest_id)
        else {
            return false;
        };
        let Ok(current) = crate::provider_manifest::activation_capability_coverage(manifest) else {
            return false;
        };
        provider.coverage.package_roots == current.package_roots
            && provider.coverage.config_files == current.config_files
            && provider.coverage.source_extensions == current.source_extensions
    });
    if !manifest_coverage_matches {
        return Ok(ActivationAssessment {
            activation: None,
            receipt: ActivationAdmissionReceipt::rebuild(
                ActivationAdmissionReason::ProviderSelectionDrift,
                gates,
            ),
        });
    }
    gates.provider_selection_matches = true;
    // The current selections were resolved from this invocation's repository
    // candidate generation. Reaching reuse means the persisted activation
    // matched that freshly resolved selection set.
    gates.repository_candidate_generation_matches = true;
    Ok(ActivationAssessment {
        activation: Some(activation),
        receipt: ActivationAdmissionReceipt::reuse(gates),
    })
}

fn activation_matches_graph_turbo_selection(
    activation: &HookActivation,
    current_selection: &crate::provider_manifest::RuntimeBinarySelectionV1,
) -> bool {
    activation
        .rankers
        .iter()
        .find(|ranker| ranker.ranker_id == "asp-graph-turbo")
        .is_some_and(|ranker| {
            ranker.binary == current_selection.binary()
                && ranker.content_digest == current_selection.content_digest()
                && ranker.artifact_metadata_digest == current_selection.artifact_metadata_digest()
        })
}

fn activation_matches_provider_command_selections(
    activation: &HookActivation,
    current_selections: &[ProviderCommandSelection],
) -> bool {
    let current_registry_digest = crate::provider_registry::semantic_registry_digest();
    let manifests = provider_manifests();
    activation.providers.len() == current_selections.len()
        && activation.providers.iter().all(|provider| {
            current_selections
                .iter()
                .find(|selection| {
                    selection.manifest_id == provider.manifest_id
                        && selection.language_id == provider.language_id
                        && selection.provider_id == provider.provider_id
                })
                .is_some_and(|selection| {
                    provider.manifest_digest == selection.manifest_digest
                        && provider.binary == selection.binary
                        && provider.execution == selection.execution
                        && provider.execution_command_digest == selection.execution_command_digest
                        && provider.semantic_registry_digest == current_registry_digest
                        && manifests
                            .iter()
                            .find(|manifest| {
                                manifest.manifest_id == provider.manifest_id
                                    && manifest.language_id == provider.language_id
                                    && manifest.provider_id == provider.provider_id
                            })
                            .and_then(|manifest| {
                                crate::provider_registry::materialize_provider_routes(manifest).ok()
                            })
                            .is_some_and(|routes| routes == provider.routes)
                })
        })
}

fn sync_activation(project_root: &Path, activation_path: &Path) -> Result<HookRuntime, String> {
    let sync = load_or_refresh_default_activation(activation_path, project_root)?;
    activation_to_runtime(&sync.activation)
}

pub(crate) fn activation_to_runtime(activation: &HookActivation) -> Result<HookRuntime, String> {
    let contents = serde_json::to_string(activation)
        .map_err(|error| format!("failed to serialize generated activation: {error}"))?;
    parse_activation(&contents, &provider_manifests()).map_err(|error| format!("{error:?}"))
}

/// Write a pretty JSON project hook activation.
pub fn write_activation(path: &Path, activation: &HookActivation) -> Result<(), String> {
    let output = serde_json::to_string_pretty(activation)
        .map_err(|error| format!("failed to serialize activation: {error}"))?;
    let output = format!("{}\n", output.trim_end());
    if fs::read(path).is_ok_and(|current| current == output.as_bytes()) {
        return Ok(());
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("failed to create {}: {error}", parent.display()))?;
    }
    let temporary = path.with_extension(format!(
        "json.tmp-{}-{}",
        std::process::id(),
        ACTIVATION_WRITE_SEQUENCE.fetch_add(1, Ordering::Relaxed)
    ));
    fs::write(&temporary, output)
        .map_err(|error| format!("failed to write {}: {error}", temporary.display()))?;
    fs::rename(&temporary, path).map_err(|error| {
        let _ = fs::remove_file(&temporary);
        format!(
            "failed to atomically replace activation {}: {error}",
            path.display()
        )
    })?;
    Ok(())
}

/// Return the managed cache path for a project's hook activation.
pub fn default_activation_path(project_root: &Path) -> PathBuf {
    project_activation_path(project_root)
        .expect("State Core activation path should resolve for default activation")
}

/// Return the State Core managed activation path when it already exists.
pub fn discover_activation_path(start: &Path) -> Option<PathBuf> {
    project_activation_path(start)
        .ok()
        .filter(|path| path.is_file())
}

fn is_generated_activation_path_for_project(path: &Path, project_root: &Path) -> bool {
    project_activation_path(project_root)
        .map(|default_path| default_path == path)
        .unwrap_or(false)
        || agent_semantic_runtime::state::project_root_for_activation_path(path)
            .map(|root| canonicalize_if_possible(&root) == canonicalize_if_possible(project_root))
            .unwrap_or(false)
}

fn canonicalize_if_possible(path: &Path) -> PathBuf {
    std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}
/// Parses a project hook activation using the built-in provider manifests.
pub fn parse_hook_activation(input: &str) -> Result<HookRuntime, crate::protocol::AgentHookError> {
    let manifests = provider_manifests();
    parse_activation(input, &manifests)
}
