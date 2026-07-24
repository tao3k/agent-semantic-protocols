//! Runtime healthcheck for project-local ASP state.

use super::hook_runtime::active_codex_plugin_skill_path;
use super::protocol_binary::protocol_binary_on_path;
use agent_semantic_config::{PRJ_CACHE_HOME_ENV, ProjectRuntimeLayout, project_runtime_layout};
use agent_semantic_hook::{
    RuntimeProfiles, RuntimeProviderHealthStatus, load_activation, runtime_profiles_for_runtime,
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
    let layout = project_runtime_layout(&options.project_root);
    let context = agent_semantic_client_core::ProjectContext::resolve(&options.project_root)?;
    let project_state_paths = agent_semantic_runtime::project_state_paths(context.cwd())?;
    let activation_path = project_state_paths.activation_path;
    let (activation, activation_runtime) =
        check_activation_and_runtime(Some(&activation_path), context.cwd());
    let binary = check_binary(&activation_path);
    let skill = check_skill(&options.project_root);

    let mut issues = collect_layout_issues(&layout, &skill);
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

    let status = overall_status(&issues);
    if options.json {
        print_json(
            status,
            &layout,
            &activation_path,
            &activation,
            &activation_runtime,
            &binary,
            &skill,
            &issues,
        )?;
    } else {
        print_compact(
            status,
            &layout,
            &activation_path,
            &activation,
            &activation_runtime,
            &binary,
            &skill,
            &issues,
        );
    }

    Ok(())
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
    match load_activation(path) {
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

fn check_binary(activation_path: &Path) -> BinaryCheck {
    let current_asp = env::current_exe().ok();
    let path_asp = protocol_binary_on_path();
    let receipt_path = agent_semantic_hook::active_asp_artifact_receipt_path(activation_path).ok();
    let (status, artifact_root_digest, binary_artifact_digest, error) =
        match (&current_asp, &path_asp) {
            (Some(current), Some(on_path)) => {
                match agent_semantic_hook::verify_active_asp_artifact_receipt(
                    activation_path,
                    &[current, on_path],
                ) {
                    Ok(receipt) => (
                        "ok",
                        Some(receipt.artifact_root_digest.as_str().to_string()),
                        Some(
                            receipt
                                .asp_binary_leaf()
                                .artifact_digest
                                .as_str()
                                .to_string(),
                        ),
                        None,
                    ),
                    Err(error) => ("artifact-degraded", None, None, Some(error)),
                }
            }
            (Some(_), None) => ("path-missing", None, None, None),
            (None, Some(_)) => ("current-missing", None, None, None),
            (None, None) => ("missing", None, None, None),
        };
    BinaryCheck {
        current_asp,
        path_asp,
        status,
        receipt_path,
        artifact_root_digest,
        binary_artifact_digest,
        error,
    }
}

fn collect_layout_issues(
    layout: &ProjectRuntimeLayout,
    skill: &SkillHealthReceipt,
) -> Vec<HealthIssue> {
    let mut issues = Vec::new();
    if layout.git_toplevel.is_none() {
        issues.push(error(
            "missing-git-toplevel",
            format!(
                "failed to locate git toplevel from {}",
                layout.requested_root.display()
            ),
        ));
    }
    if layout.cache_home.is_none() {
        issues.push(error(
            "missing-cache-home",
            format!("set {PRJ_CACHE_HOME_ENV} or run inside a git worktree"),
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

fn print_compact(
    status: &str,
    layout: &ProjectRuntimeLayout,
    activation_path: &Path,
    activation: &ActivationCheck,
    activation_runtime: &ActivationRuntimeCheck,
    binary: &BinaryCheck,
    skill: &SkillHealthReceipt,
    issues: &[HealthIssue],
) {
    println!(
        "[asp-healthcheck] status={} gitToplevel={} cacheHome={} cacheSource={}",
        status,
        display_opt(layout.git_toplevel.as_deref()),
        display_opt(layout.cache_home.as_deref()),
        layout
            .cache_source
            .as_ref()
            .map(|source| source.as_str())
            .unwrap_or("missing"),
    );
    println!(
        "|env {}={}",
        PRJ_CACHE_HOME_ENV,
        env_status(layout.prj_cache_home.as_deref()),
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
        "|binary currentAsp={} pathAsp={} status={} activeArtifactReceipt={} artifactRoot={} binaryArtifactDigest={} verification=receipt-metadata subprocesses=0 dbOpens=0 manifestWrites=0 binaryByteReads=0 error={}",
        display_opt(binary.current_asp.as_deref()),
        display_opt(binary.path_asp.as_deref()),
        binary.status,
        display_opt(binary.receipt_path.as_deref()),
        binary.artifact_root_digest.as_deref().unwrap_or("none"),
        binary.binary_artifact_digest.as_deref().unwrap_or("none"),
        binary.error.as_deref().unwrap_or("none")
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
    for issue in issues {
        println!(
            "|{} code={} message={}",
            issue.severity,
            issue.code,
            single_line(&issue.message)
        );
    }
}

fn print_json(
    status: &str,
    layout: &ProjectRuntimeLayout,
    activation_path: &Path,
    activation: &ActivationCheck,
    activation_runtime: &ActivationRuntimeCheck,
    binary: &BinaryCheck,
    skill: &SkillHealthReceipt,
    issues: &[HealthIssue],
) -> Result<(), String> {
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
        "projectRoot": layout.requested_root.display().to_string(),
        "gitToplevel": path_value(layout.git_toplevel.as_deref()),
        "cacheHome": path_value(layout.cache_home.as_deref()),
        "cacheSource": layout.cache_source.as_ref().map(|source| source.as_str()),
        "env": {
            "PRJ_CACHE_HOME": path_value(layout.prj_cache_home.as_deref()),
        },
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
            "status": binary.status,
            "activeArtifactReceipt": path_value(binary.receipt_path.as_deref()),
            "artifactRootDigest": binary.artifact_root_digest,
            "binaryArtifactDigest": binary.binary_artifact_digest,
            "verification": "receipt-metadata",
            "subprocessCount": 0,
            "dbOpenCount": 0,
            "manifestWriteCount": 0,
            "binaryByteReadCount": 0,
            "error": binary.error,
        },
        "providers": providers,
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

fn env_status(path: Option<&Path>) -> String {
    match path {
        Some(path) => format!("set:{}", path.display()),
        None => "unset".to_string(),
    }
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
