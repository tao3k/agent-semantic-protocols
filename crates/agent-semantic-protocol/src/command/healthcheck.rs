//! Runtime healthcheck for project-local ASP state.

use super::protocol_binary::protocol_binary_on_path;
use agent_semantic_hook::{
    PRJ_CACHE_HOME_ENV, ProjectRuntimeLayout, RuntimeProfiles, RuntimeProviderHealthStatus,
    load_activation, project_runtime_layout, runtime_profiles_for_runtime,
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
    let activation = check_activation(layout.activation_path.as_deref());
    let activation_runtime = check_activation_runtime(
        layout.activation_path.as_deref(),
        layout
            .git_toplevel
            .as_deref()
            .unwrap_or(&options.project_root),
    );
    let binary = check_binary();

    let mut issues = collect_layout_issues(&layout);
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
            &activation,
            &activation_runtime,
            &binary,
            &issues,
        )?;
    } else {
        print_compact(
            status,
            &layout,
            &activation,
            &activation_runtime,
            &binary,
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
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum FsKind {
    Dir,
    File,
}

fn check_activation(path: Option<&Path>) -> ActivationCheck {
    let Some(path) = path else {
        return ActivationCheck {
            status: "unresolved",
            provider_count: None,
            error: None,
        };
    };
    if !path.is_file() {
        return ActivationCheck {
            status: "missing",
            provider_count: None,
            error: None,
        };
    }
    match load_activation(path) {
        Ok(runtime) => ActivationCheck {
            status: "ok",
            provider_count: Some(runtime.providers.len()),
            error: None,
        },
        Err(error) => ActivationCheck {
            status: "invalid",
            provider_count: None,
            error: Some(error),
        },
    }
}

fn check_activation_runtime(path: Option<&Path>, project_root: &Path) -> ActivationRuntimeCheck {
    let Some(path) = path else {
        return ActivationRuntimeCheck {
            status: "unresolved",
            provider_count: None,
            error: None,
            profiles: None,
        };
    };
    if !path.is_file() {
        return ActivationRuntimeCheck {
            status: "missing",
            provider_count: None,
            error: None,
            profiles: None,
        };
    }
    match load_activation(path).map(|runtime| runtime_profiles_for_runtime(project_root, &runtime))
    {
        Ok(profiles) => ActivationRuntimeCheck {
            status: "ok",
            provider_count: Some(profiles.providers.len()),
            error: None,
            profiles: Some(profiles),
        },
        Err(error) => ActivationRuntimeCheck {
            status: "invalid",
            provider_count: None,
            error: Some(error),
            profiles: None,
        },
    }
}

fn check_binary() -> BinaryCheck {
    let current_asp = env::current_exe().ok();
    let path_asp = protocol_binary_on_path();
    let status = match (&current_asp, &path_asp) {
        (Some(current), Some(on_path)) if same_path(current, on_path) => "ok",
        (Some(_), Some(_)) => "mismatch",
        (Some(_), None) => "path-missing",
        (None, Some(_)) => "current-missing",
        (None, None) => "missing",
    };
    BinaryCheck {
        current_asp,
        path_asp,
        status,
    }
}

fn collect_layout_issues(layout: &ProjectRuntimeLayout) -> Vec<HealthIssue> {
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
    collect_file_issue(
        &mut issues,
        "missing-agent-skill",
        "git toplevel agent-semantic-protocols skill is missing",
        fs_status(layout.agent_skill_path.as_deref(), FsKind::File),
    );
    issues
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
        "mismatch" => issues.push(warn(
            "asp-binary-mismatch",
            "current asp executable differs from asp on PATH".to_string(),
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

fn same_path(left: &Path, right: &Path) -> bool {
    let left = left.canonicalize().unwrap_or_else(|_| left.to_path_buf());
    let right = right.canonicalize().unwrap_or_else(|_| right.to_path_buf());
    left == right
}

fn print_compact(
    status: &str,
    layout: &ProjectRuntimeLayout,
    activation: &ActivationCheck,
    activation_runtime: &ActivationRuntimeCheck,
    binary: &BinaryCheck,
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
        "|path agentsSkill={} status={}",
        display_opt(layout.agent_skill_path.as_deref()),
        fs_status(layout.agent_skill_path.as_deref(), FsKind::File)
    );
    println!(
        "|path activation={} status={} providers={}",
        display_opt(layout.activation_path.as_deref()),
        activation.status,
        display_count(activation.provider_count)
    );
    println!(
        "|activationRuntime status={} providers={}",
        activation_runtime.status,
        display_count(activation_runtime.provider_count)
    );
    println!(
        "|binary currentAsp={} pathAsp={} status={}",
        display_opt(binary.current_asp.as_deref()),
        display_opt(binary.path_asp.as_deref()),
        binary.status
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
    activation: &ActivationCheck,
    activation_runtime: &ActivationRuntimeCheck,
    binary: &BinaryCheck,
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
            "agentsSkill": path_report(layout.agent_skill_path.as_deref(), fs_status(layout.agent_skill_path.as_deref(), FsKind::File), None, None),
            "activation": path_report(layout.activation_path.as_deref(), activation.status, activation.provider_count, activation.error.as_deref()),
        },
        "activationRuntime": {
            "status": activation_runtime.status,
            "providerCount": activation_runtime.provider_count,
            "error": activation_runtime.error,
        },
        "binary": {
            "currentAsp": path_value(binary.current_asp.as_deref()),
            "pathAsp": path_value(binary.path_asp.as_deref()),
            "status": binary.status,
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
