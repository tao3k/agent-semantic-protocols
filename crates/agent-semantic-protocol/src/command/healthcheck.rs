//! Runtime healthcheck for project-local ASP state.

use super::hook_runtime::active_codex_plugin_skill_path;
use super::protocol_binary::protocol_binary_on_path;
use agent_semantic_hook::{
    RuntimeProfiles, RuntimeProviderHealthStatus, load_or_sync_activation,
    runtime_profiles_for_runtime,
};
use serde::Serialize;
use serde_json::{Value, json};
use std::env;
use std::path::{Path, PathBuf};

const HEALTHCHECK_SCHEMA_ID: &str = "agent.semantic-protocols.healthcheck";
const HEALTHCHECK_SCHEMA_VERSION: &str = "1";
const HEALTHCHECK_PROTOCOL_ID: &str = "agent.semantic-protocols.runtime";
const HEALTHCHECK_PROTOCOL_VERSION: &str = "1";

pub(super) fn run_healthcheck_command(args: &[String]) -> Result<(), String> {
    let options = HealthcheckOptions::parse(args)?;
    let layout = HealthcheckStateLayout::resolve(&options.project_root)?;
    let context = agent_semantic_client_core::ProjectContext::resolve(&options.project_root)?;
    let resolved_state =
        agent_semantic_client_core::state_core::ResolvedState::resolve(context.cwd())?;
    let project_state_paths = agent_semantic_runtime::project_state_paths(context.cwd())?;
    let activation_path = project_state_paths.activation_path;
    let (activation, activation_runtime) =
        check_activation_and_runtime(Some(&activation_path), context.cwd());
    let binary = check_binary(
        &activation_path,
        layout.state_home.join("runtime").join("bin").join("asp"),
    );
    let skill = check_skill(&options.project_root);
    let (
        resident_result,
        workspace_generation_durability_result,
        workspace_generation_elapsed_micros,
    ) = super::runtime_server::block_on_runtime_server_client(async {
        let resident_result = match super::runtime_server::healthcheck_runtime_server_at(
                &layout.state_home,
            )
            .await
            {
            Ok(receipt)
                if receipt.state
                    == agent_semantic_client_db::runtime_server_control::RuntimeServerState::Healthy =>
            {
                Ok(receipt)
            }
            Ok(_) | Err(_) => {
                super::runtime_server_supervisor::reconcile_healthy_runtime_server(
                    &layout.state_home,
                )
                .await
            }
            };
        let workspace_generation_started = tokio::time::Instant::now();
        let workspace_generation_durability_result = match &resident_result {
                Ok(_) => {
                    match super::runtime_server::runtime_server_workspace_session_for_resolved_admission_async(
                        resolved_state.workspace.workspace_id.to_string(),
                        &resolved_state.workspace.root,
                    )
                    .await
                    {
                        Ok(session) => {
                            session
                                .runtime_generation_durability(
                                    context.cwd().to_string_lossy().as_ref(),
                                )
                                .await
                        }
                        Err(error) => Err(error),
                    }
                }
                Err(error) => {
                    Err(format!(
                        "workspace generation admission requires a healthy Runtime Server: {error}"
                    ))
                }
            };
        let workspace_generation_elapsed_micros =
            u64::try_from(workspace_generation_started.elapsed().as_micros()).unwrap_or(u64::MAX);
        (
            resident_result,
            workspace_generation_durability_result,
            workspace_generation_elapsed_micros,
        )
    })?;
    let resident = match &resident_result {
        Ok(receipt) => GlobalResidentRuntimeCheck {
            status: "ready".to_owned(),
            transport_contract_digest: Some(receipt.transport_contract_digest.clone()),
            runtime_binary_digest: Some(receipt.runtime_artifact_digest.clone()),
            workspace_entry_count: Some(receipt.workspace_entry_count),
            error: None,
        },
        Err(resident_error) => GlobalResidentRuntimeCheck {
            status: "error".to_owned(),
            transport_contract_digest: None,
            runtime_binary_digest: None,
            workspace_entry_count: None,
            error: Some(resident_error.clone()),
        },
    };
    let workspace_generation = match &workspace_generation_durability_result {
        Ok(Some(durability)) => WorkspaceGenerationHealthCheck {
            status: match durability.state {
                agent_semantic_client_db::runtime_server_workspace::WorkspaceGenerationDurabilityState::ResidentReady => {
                    "resident-ready"
                }
                agent_semantic_client_db::runtime_server_workspace::WorkspaceGenerationDurabilityState::DurableReady => {
                    "durable-ready"
                }
                agent_semantic_client_db::runtime_server_workspace::WorkspaceGenerationDurabilityState::Failed => "failed",
            }
            .to_owned(),
            workspace_identity: Some(durability.workspace_identity.clone()),
            generation_digest: Some(durability.generation_digest.clone()),
            reconciled: None,
            durability_state: Some(durability.state),
            durability_failure: durability.failure.clone(),
            elapsed_micros: workspace_generation_elapsed_micros,
            error: durability.failure.clone(),
        },
        Ok(None) => WorkspaceGenerationHealthCheck {
            status: "missing".to_owned(),
            workspace_identity: Some(resolved_state.workspace.workspace_id.to_string()),
            generation_digest: None,
            reconciled: None,
            durability_state: None,
            durability_failure: None,
            elapsed_micros: workspace_generation_elapsed_micros,
            error: None,
        },
        Err(error) => WorkspaceGenerationHealthCheck {
            status: "error".to_owned(),
            workspace_identity: None,
            generation_digest: None,
            reconciled: None,
            durability_state: None,
            durability_failure: None,
            elapsed_micros: workspace_generation_elapsed_micros,
            error: Some(error.clone()),
        },
    };

    let mut issues = collect_layout_issues(&layout, &skill);
    if let Err(resident_error) = &resident_result {
        issues.push(error(
            "global-resident-runtime-degraded",
            resident_error.clone(),
        ));
    }
    if workspace_generation.status == "failed" || workspace_generation.status == "error" {
        issues.push(error(
            "workspace-generation-admission-failed",
            workspace_generation
                .error
                .clone()
                .unwrap_or_else(|| "workspace generation admission failed".to_owned()),
        ));
    }
    collect_read_issue(
        &mut issues,
        "missing-activation",
        "hook activation is missing",
        "invalid-activation",
        "hook activation is invalid",
        activation.status,
        activation.error.as_deref(),
    );
    collect_binary_issue(&mut issues, &binary);

    let catalog = collect_catalog_readiness(
        &mut issues,
        super::global_provider_catalog::read_global_provider_catalog_readiness(),
    );
    let status = overall_status(&issues);
    let report = HealthcheckReport {
        status,
        layout: &layout,
        activation_path: &activation_path,
        activation: &activation,
        activation_runtime: &activation_runtime,
        binary: &binary,
        resident: &resident,
        workspace_generation: &workspace_generation,
        skill: &skill,
        catalog: &catalog,
        issues: &issues,
    };
    if options.json {
        print_json(&report)?;
    } else {
        print_compact(&report);
    }

    healthcheck_command_result(status)
}

#[derive(Debug)]
struct HealthcheckOptions {
    json: bool,
    project_root: PathBuf,
}

impl HealthcheckOptions {
    fn parse(args: &[String]) -> Result<Self, String> {
        let mut json = false;
        let mut project_root = None;
        for arg in args {
            match arg.as_str() {
                "--json" => json = true,
                "--help" | "-h" => return Err(usage()),
                _ if arg.starts_with('-') => {
                    return Err(format!("unknown asp healthcheck option {arg}\n{}", usage()));
                }
                _ => {
                    if project_root.replace(PathBuf::from(arg)).is_some() {
                        return Err(format!(
                            "asp healthcheck accepts at most one PROJECT_ROOT\n{}",
                            usage()
                        ));
                    }
                }
            }
        }
        Ok(Self {
            json,
            project_root: project_root.unwrap_or_else(|| PathBuf::from(".")),
        })
    }
}

fn usage() -> String {
    "usage: asp healthcheck [--json] [PROJECT_ROOT]".to_string()
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct HealthIssue {
    severity: &'static str,
    code: &'static str,
    message: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(tag = "status", rename_all = "kebab-case")]
enum CatalogReadinessReceipt {
    Ready {
        catalog_generation: String,
        provider_count: usize,
        elapsed_micros: u128,
    },
    Error {
        message: String,
    },
}

fn collect_catalog_readiness(
    issues: &mut Vec<HealthIssue>,
    readiness: Result<super::global_provider_catalog::GlobalProviderCatalogReadiness, String>,
) -> CatalogReadinessReceipt {
    match readiness {
        Ok(readiness) => CatalogReadinessReceipt::Ready {
            catalog_generation: readiness.catalog_generation,
            provider_count: readiness.provider_count,
            elapsed_micros: readiness.elapsed_micros,
        },
        Err(message) => {
            issues.push(HealthIssue {
                severity: "error",
                code: "global-provider-catalog-readiness-failed",
                message: message.clone(),
            });
            CatalogReadinessReceipt::Error { message }
        }
    }
}

fn healthcheck_command_result(status: &str) -> Result<(), String> {
    if status == "error" {
        Err("healthcheck status=error".to_owned())
    } else {
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct SkillHealthReceipt {
    authority: &'static str,
    path: Option<PathBuf>,
    status: &'static str,
    error: Option<String>,
}

#[derive(Clone, Debug)]
struct ActivationCheck {
    status: &'static str,
    provider_count: Option<usize>,
    error: Option<String>,
}

#[derive(Clone, Debug)]
struct ActivationRuntimeCheck {
    status: &'static str,
    provider_count: Option<usize>,
    error: Option<String>,
    profiles: Option<RuntimeProfiles>,
}

#[derive(Clone, Debug)]
struct BinaryCheck {
    current_asp: Option<PathBuf>,
    path_asp: Option<PathBuf>,
    canonical_runtime_asp: PathBuf,
    path_link_target: Option<PathBuf>,
    status: &'static str,
    receipt_path: Option<PathBuf>,
    artifact_root_digest: Option<String>,
    binary_artifact_digest: Option<String>,
    error: Option<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum FsKind {
    Dir,
    File,
}

fn check_activation_and_runtime(
    path: Option<&Path>,
    project_root: &Path,
) -> (ActivationCheck, ActivationRuntimeCheck) {
    let Some(path) = path else {
        return (
            ActivationCheck {
                status: "unresolved",
                provider_count: None,
                error: None,
            },
            ActivationRuntimeCheck {
                status: "unresolved",
                provider_count: None,
                error: None,
                profiles: None,
            },
        );
    };
    if !path.is_file() {
        return (
            ActivationCheck {
                status: "missing",
                provider_count: None,
                error: None,
            },
            ActivationRuntimeCheck {
                status: "missing",
                provider_count: None,
                error: None,
                profiles: None,
            },
        );
    }
    match load_or_sync_activation(path, project_root) {
        Ok(runtime) => {
            let provider_count = runtime.providers.len();
            let profiles = runtime_profiles_for_runtime(project_root, &runtime);
            (
                ActivationCheck {
                    status: "ok",
                    provider_count: Some(provider_count),
                    error: None,
                },
                ActivationRuntimeCheck {
                    status: "ok",
                    provider_count: Some(profiles.providers.len()),
                    error: None,
                    profiles: Some(profiles),
                },
            )
        }
        Err(error) => (
            ActivationCheck {
                status: "invalid",
                provider_count: None,
                error: Some(error.clone()),
            },
            ActivationRuntimeCheck {
                status: "invalid",
                provider_count: None,
                error: Some(error),
                profiles: None,
            },
        ),
    }
}

fn check_binary(activation_path: &Path, canonical_runtime_asp: PathBuf) -> BinaryCheck {
    let current_asp = env::current_exe().ok();
    let path_asp = protocol_binary_on_path();
    let path_link_target = path_asp
        .as_deref()
        .and_then(|path| std::fs::read_link(path).ok());
    let receipt_path = agent_semantic_hook::active_asp_artifact_receipt_path(activation_path).ok();
    let (status, artifact_root_digest, binary_artifact_digest, error) = match (
        &current_asp,
        &path_asp,
    ) {
        (Some(current), Some(on_path)) => {
            if path_link_target.as_deref() != Some(canonical_runtime_asp.as_path()) {
                (
                    "path-entry-degraded",
                    None,
                    None,
                    Some(format!(
                        "global ASP PATH entry must be a direct symlink to canonical runtime: path={} expectedTarget={} actualTarget={}",
                        on_path.display(),
                        canonical_runtime_asp.display(),
                        path_link_target
                            .as_deref()
                            .map(|path| path.display().to_string())
                            .unwrap_or_else(|| "not-a-symlink".to_string())
                    )),
                )
            } else {
                match agent_semantic_hook::verify_active_asp_artifact_receipt(
                    activation_path,
                    &[current, on_path],
                ) {
                    Ok(receipt) => (
                        "ok",
                        Some(receipt.artifact_root_digest().as_str().to_string()),
                        Some(
                            receipt
                                .asp_binary_leaf()
                                .artifact_digest()
                                .as_str()
                                .to_string(),
                        ),
                        None,
                    ),
                    Err(error) => ("artifact-degraded", None, None, Some(error)),
                }
            }
        }
        (Some(_), None) => ("path-missing", None, None, None),
        (None, Some(_)) => ("current-missing", None, None, None),
        (None, None) => ("missing", None, None, None),
    };
    BinaryCheck {
        current_asp,
        path_asp,
        canonical_runtime_asp,
        path_link_target,
        status,
        receipt_path,
        artifact_root_digest,
        binary_artifact_digest,
        error,
    }
}

fn collect_layout_issues(
    layout: &HealthcheckStateLayout,
    skill: &SkillHealthReceipt,
) -> Vec<HealthIssue> {
    let mut issues = Vec::new();
    if layout.git_toplevel.is_none() {
        issues.push(error(
            "missing-git-toplevel",
            format!(
                "failed to locate git toplevel from {}",
                layout.project_root.display()
            ),
        ));
    }
    collect_file_issue(
        &mut issues,
        "missing-agents-dir",
        "git toplevel .agents directory is missing",
        fs_status(layout.agents_dir.as_deref(), FsKind::Dir),
    );
    match skill.status {
        "ok" => {}
        "missing" => issues.push(error(
            "missing-agent-skill",
            "active plugin-installed agent-semantic-protocols skill is missing".to_owned(),
        )),
        _ => issues.push(error(
            "invalid-agent-skill",
            skill
                .error
                .clone()
                .unwrap_or_else(|| "failed to resolve active plugin-installed skill".to_owned()),
        )),
    }
    issues
}

fn check_skill(project_root: &Path) -> SkillHealthReceipt {
    match active_codex_plugin_skill_path(project_root) {
        Ok(path) => {
            let status = fs_status(path.as_deref(), FsKind::File);
            SkillHealthReceipt {
                authority: "plugin-installed",
                path,
                status,
                error: None,
            }
        }
        Err(error) => SkillHealthReceipt {
            authority: "plugin-installed",
            path: None,
            status: "invalid",
            error: Some(error.to_string()),
        },
    }
}

fn collect_file_issue(
    issues: &mut Vec<HealthIssue>,
    missing_code: &'static str,
    missing_message: &'static str,
    status: &'static str,
) {
    match status {
        "missing" => issues.push(warn(missing_code, missing_message.to_string())),
        "unresolved" => issues.push(warn(
            missing_code,
            format!("{missing_message}; path could not be resolved"),
        )),
        _ => {}
    }
}

fn collect_read_issue(
    issues: &mut Vec<HealthIssue>,
    missing_code: &'static str,
    missing_message: &'static str,
    invalid_code: &'static str,
    invalid_message: &'static str,
    status: &str,
    error_detail: Option<&str>,
) {
    match status {
        "missing" | "unresolved" => issues.push(warn(missing_code, missing_message.to_string())),
        "invalid" => issues.push(error(
            invalid_code,
            match error_detail {
                Some(detail) => format!("{invalid_message}: {detail}"),
                None => invalid_message.to_string(),
            },
        )),
        _ => {}
    }
}

fn collect_binary_issue(issues: &mut Vec<HealthIssue>, binary: &BinaryCheck) {
    match binary.status {
        "ok" => {}
        "artifact-degraded" => issues.push(warn(
            "active-asp-artifact-receipt-invalid",
            binary
                .error
                .clone()
                .unwrap_or_else(|| "active ASP artifact receipt could not be verified".to_string()),
        )),
        "path-entry-degraded" => issues.push(error(
            "asp-global-path-entry-invalid",
            binary.error.clone().unwrap_or_else(|| {
                "global ASP PATH entry does not target the canonical State Home runtime".to_string()
            }),
        )),
        _ => issues.push(warn(
            "asp-binary-path-missing",
            "asp executable could not be resolved consistently".to_string(),
        )),
    }
}

fn error(code: &'static str, message: String) -> HealthIssue {
    HealthIssue {
        severity: "error",
        code,
        message,
    }
}

fn warn(code: &'static str, message: String) -> HealthIssue {
    HealthIssue {
        severity: "warn",
        code,
        message,
    }
}

fn overall_status(issues: &[HealthIssue]) -> &'static str {
    if issues.iter().any(|issue| issue.severity == "error") {
        "error"
    } else if issues.iter().any(|issue| issue.severity == "warn") {
        "degraded"
    } else {
        "ok"
    }
}

fn fs_status(path: Option<&Path>, kind: FsKind) -> &'static str {
    match path {
        None => "unresolved",
        Some(path) if kind == FsKind::Dir && path.is_dir() => "ok",
        Some(path) if kind == FsKind::File && path.is_file() => "ok",
        Some(_) => "missing",
    }
}

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct GlobalResidentRuntimeCheck {
    status: String,
    transport_contract_digest: Option<String>,
    runtime_binary_digest: Option<String>,
    workspace_entry_count: Option<usize>,
    error: Option<String>,
}

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct WorkspaceGenerationHealthCheck {
    status: String,
    workspace_identity: Option<String>,
    generation_digest: Option<String>,
    reconciled: Option<bool>,
    durability_state: Option<
        agent_semantic_client_db::runtime_server_workspace::WorkspaceGenerationDurabilityState,
    >,
    durability_failure: Option<String>,
    elapsed_micros: u64,
    error: Option<String>,
}

struct HealthcheckStateLayout {
    project_root: PathBuf,
    state_home: PathBuf,
    repo_id: String,
    workspace_id: String,
    git_toplevel: Option<PathBuf>,
    agents_dir: Option<PathBuf>,
}

impl HealthcheckStateLayout {
    fn resolve(project_root: &Path) -> Result<Self, String> {
        let state = agent_semantic_runtime::state_core::ResolvedState::resolve(project_root)?;
        let git_toplevel = state.repo.git_toplevel.clone();
        Ok(Self {
            project_root: project_root.to_path_buf(),
            state_home: state.state_home,
            repo_id: state.repo.repo_id.to_string(),
            workspace_id: state.workspace.workspace_id.to_string(),
            agents_dir: git_toplevel.as_ref().map(|root| root.join(".agents")),
            git_toplevel,
        })
    }
}

struct HealthcheckReport<'a> {
    status: &'a str,
    layout: &'a HealthcheckStateLayout,
    activation_path: &'a Path,
    activation: &'a ActivationCheck,
    activation_runtime: &'a ActivationRuntimeCheck,
    binary: &'a BinaryCheck,
    resident: &'a GlobalResidentRuntimeCheck,
    workspace_generation: &'a WorkspaceGenerationHealthCheck,
    skill: &'a SkillHealthReceipt,
    catalog: &'a CatalogReadinessReceipt,
    issues: &'a [HealthIssue],
}

#[cfg(test)]
#[path = "../../tests/unit/command/healthcheck.rs"]
mod tests;

fn print_compact(report: &HealthcheckReport<'_>) {
    let HealthcheckReport {
        status,
        layout,
        activation_path,
        activation,
        activation_runtime,
        binary,
        resident,
        workspace_generation,
        skill,
        catalog,
        issues,
    } = report;
    println!(
        "[asp-healthcheck] status={} stateHome={} repoId={} workspaceId={} gitToplevel={}",
        status,
        layout.state_home.display(),
        layout.repo_id,
        layout.workspace_id,
        display_opt(layout.git_toplevel.as_deref()),
    );
    println!(
        "|path agentsDir={} status={}",
        display_opt(layout.agents_dir.as_deref()),
        fs_status(layout.agents_dir.as_deref(), FsKind::Dir)
    );
    println!(
        "|skill authority={} path={} status={} error={}",
        skill.authority,
        display_opt(skill.path.as_deref()),
        skill.status,
        skill.error.as_deref().unwrap_or("none")
    );
    println!(
        "|path activation={} status={} providers={}",
        activation_path.display(),
        activation.status,
        display_count(activation.provider_count)
    );
    println!(
        "|activationRuntime status={} providers={}",
        activation_runtime.status,
        display_count(activation_runtime.provider_count)
    );
    println!(
        "|binary currentAsp={} pathAsp={} canonicalRuntimeAsp={} pathLinkTarget={} status={} activeArtifactReceipt={} artifactRoot={} binaryArtifactDigest={} verification=receipt-metadata+path-link subprocesses=0 dbOpens=0 manifestWrites=0 binaryByteReads=0 error={}",
        display_opt(binary.current_asp.as_deref()),
        display_opt(binary.path_asp.as_deref()),
        binary.canonical_runtime_asp.display(),
        display_opt(binary.path_link_target.as_deref()),
        binary.status,
        display_opt(binary.receipt_path.as_deref()),
        binary.artifact_root_digest.as_deref().unwrap_or("none"),
        binary.binary_artifact_digest.as_deref().unwrap_or("none"),
        binary.error.as_deref().unwrap_or("none")
    );
    println!(
        "|residentRuntime scope=global status={} workspaceEntryCount={} transportContractDigest={} runtimeBinaryDigest={} error={}",
        resident.status,
        resident
            .workspace_entry_count
            .map(|value| value.to_string())
            .as_deref()
            .unwrap_or("none"),
        resident
            .transport_contract_digest
            .as_deref()
            .unwrap_or("none"),
        resident.runtime_binary_digest.as_deref().unwrap_or("none"),
        resident.error.as_deref().unwrap_or("none"),
    );
    println!(
        "|workspaceGeneration status={} workspaceIdentity={} generationDigest={} reconciled={} durabilityState={} durabilityFailure={} elapsedMicros={} error={}",
        workspace_generation.status,
        workspace_generation
            .workspace_identity
            .as_deref()
            .unwrap_or("none"),
        workspace_generation
            .generation_digest
            .as_deref()
            .unwrap_or("none"),
        workspace_generation
            .reconciled
            .map(|value| value.to_string())
            .as_deref()
            .unwrap_or("none"),
        workspace_generation
            .durability_state
            .map(|state| match state {
                agent_semantic_client_db::runtime_server_workspace::WorkspaceGenerationDurabilityState::ResidentReady => {
                    "resident-ready"
                }
                agent_semantic_client_db::runtime_server_workspace::WorkspaceGenerationDurabilityState::DurableReady => {
                    "durable-ready"
                }
                agent_semantic_client_db::runtime_server_workspace::WorkspaceGenerationDurabilityState::Failed => "failed",
            })
            .unwrap_or("none"),
        workspace_generation
            .durability_failure
            .as_deref()
            .unwrap_or("none"),
        workspace_generation.elapsed_micros,
        workspace_generation.error.as_deref().unwrap_or("none"),
    );
    if let Some(profiles) = activation_runtime.profiles.as_ref() {
        for provider in &profiles.providers {
            println!(
                "|provider language={} provider={} runtime={} resolvedBinary={} argv={}",
                provider.language_id,
                provider.provider_id,
                runtime_provider_status(provider.health.status),
                provider.resolved_binary.as_deref().unwrap_or("missing"),
                provider.argv.join(" ")
            );
        }
    }
    match catalog {
        CatalogReadinessReceipt::Ready {
            catalog_generation,
            provider_count,
            elapsed_micros,
        } => println!(
            "|catalogReadiness status=ready generation={} providerCount={} elapsedMicros={}",
            catalog_generation, provider_count, elapsed_micros
        ),
        CatalogReadinessReceipt::Error { message } => {
            println!(
                "|catalogReadiness status=error message={}",
                single_line(message)
            )
        }
    }
    for issue in *issues {
        println!(
            "|{} code={} message={}",
            issue.severity,
            issue.code,
            single_line(&issue.message)
        );
    }
}

fn print_json(report: &HealthcheckReport<'_>) -> Result<(), String> {
    let HealthcheckReport {
        status,
        layout,
        activation_path,
        activation,
        activation_runtime,
        binary,
        resident,
        workspace_generation,
        skill,
        catalog,
        issues,
    } = report;
    let providers = activation_runtime
        .profiles
        .as_ref()
        .map(|profiles| {
            profiles
                .providers
                .iter()
                .map(|provider| {
                    json!({
                        "languageId": provider.language_id,
                        "providerId": provider.provider_id,
                        "manifestId": provider.manifest_id,
                        "binary": provider.binary,
                        "resolvedBinary": provider.resolved_binary,
                        "argv": provider.argv,
                        "healthStatus": runtime_provider_status(provider.health.status),
                    })
                })
                .collect::<Vec<Value>>()
        })
        .unwrap_or_default();
    let document = json!({
        "schemaId": HEALTHCHECK_SCHEMA_ID,
        "schemaVersion": HEALTHCHECK_SCHEMA_VERSION,
        "protocolId": HEALTHCHECK_PROTOCOL_ID,
        "protocolVersion": HEALTHCHECK_PROTOCOL_VERSION,
        "status": status,
        "projectRoot": layout.project_root.display().to_string(),
        "stateHome": layout.state_home.display().to_string(),
        "repoId": layout.repo_id,
        "workspaceId": layout.workspace_id,
        "gitToplevel": path_value(layout.git_toplevel.as_deref()),
        "paths": {
            "agentsDir": path_report(layout.agents_dir.as_deref(), fs_status(layout.agents_dir.as_deref(), FsKind::Dir), None, None),
            "activation": path_report(Some(activation_path), activation.status, activation.provider_count, activation.error.as_deref()),
        },
        "skill": skill,
        "activationRuntime": {
            "status": activation_runtime.status,
            "providerCount": activation_runtime.provider_count,
            "error": activation_runtime.error,
        },
        "binary": {
            "currentAsp": path_value(binary.current_asp.as_deref()),
            "pathAsp": path_value(binary.path_asp.as_deref()),
            "canonicalRuntimeAsp": binary.canonical_runtime_asp.display().to_string(),
            "pathLinkTarget": path_value(binary.path_link_target.as_deref()),
            "status": binary.status,
            "activeArtifactReceipt": path_value(binary.receipt_path.as_deref()),
            "artifactRootDigest": binary.artifact_root_digest,
            "binaryArtifactDigest": binary.binary_artifact_digest,
            "verification": "receipt-metadata+path-link",
            "subprocessCount": 0,
            "dbOpenCount": 0,
            "manifestWriteCount": 0,
            "binaryByteReadCount": 0,
            "error": binary.error,
        },
        "residentRuntime": resident,
        "workspaceGeneration": workspace_generation,
        "providers": providers,
        "catalogReadiness": catalog,
        "issues": issues,
    });
    let text = serde_json::to_string_pretty(&document)
        .map_err(|error| format!("failed to serialize healthcheck JSON: {error}"))?;
    println!("{text}");
    Ok(())
}

fn path_report(
    path: Option<&Path>,
    status: &'static str,
    provider_count: Option<usize>,
    error: Option<&str>,
) -> Value {
    json!({
        "path": path_value(path),
        "status": status,
        "providerCount": provider_count,
        "error": error,
    })
}

fn path_value(path: Option<&Path>) -> Value {
    path.map(|path| json!(path.display().to_string()))
        .unwrap_or(Value::Null)
}

fn display_opt(path: Option<&Path>) -> String {
    path.map(|path| path.display().to_string())
        .unwrap_or_else(|| "missing".to_string())
}

fn display_count(count: Option<usize>) -> String {
    count
        .map(|count| count.to_string())
        .unwrap_or_else(|| "n/a".to_string())
}

fn runtime_provider_status(status: RuntimeProviderHealthStatus) -> &'static str {
    match status {
        RuntimeProviderHealthStatus::Available => "available",
        RuntimeProviderHealthStatus::Missing => "missing",
        RuntimeProviderHealthStatus::Unexecutable => "unexecutable",
    }
}

fn single_line(message: &str) -> String {
    message.replace(['\n', '\r'], " ")
}
