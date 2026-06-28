//! ASP-owned Org capture state materialization.

use super::org_capture_contract_materialize::{
    ContractCaptureArgs, materialize_contract_capture_args,
};
use agent_semantic_runtime::project_state_paths;
use orgize::agent;
use std::{
    env, fs,
    io::ErrorKind,
    path::{Path, PathBuf},
    process::Command,
};

const FLOW_DIRS: &[&str] = &["plans", "sdd", "bdd", "tdd", "bdr"];
const ORG_ARTIFACTS_DIR: &str = "artifacts/org";
const DEFAULT_ASP_ORG_REPO_URL: &str = "https://github.com/tao3k/org.git";
const ASP_ORG_REPO_URL_ENV: &str = "ASP_ORG_REPO_URL";

pub(crate) fn run_org_capture_command(args: &[String]) -> Result<(), String> {
    if args
        .iter()
        .any(|arg| matches!(arg.as_str(), "-h" | "--help" | "help"))
    {
        println!("{}", capture_usage());
        return Ok(());
    }
    if args.first().is_some_and(|arg| arg == "init") {
        return Err(
            "asp org capture init is not a public command; Org resources are synchronized automatically when capture resolves a contract or template"
                .to_string(),
        );
    }
    if capture_contract_requested(args) {
        return run_contract_capture(args);
    }
    Err("asp org capture expects `--contract CONTRACT_ID`".to_string())
}

fn run_contract_capture(args: &[String]) -> Result<(), String> {
    let contract_id = capture_contract_id(args)?;
    let template_path = resolve_capture_template(&contract_id)?;
    let contract_registry_path = resolve_capture_contract_registry_from_args(args, &contract_id)?;
    let capture_args = match materialize_contract_capture_args(
        args,
        &contract_id,
        template_path.as_deref(),
        Some(&contract_registry_path),
    )? {
        ContractCaptureArgs::Continue(capture_args) => capture_args,
        ContractCaptureArgs::DeferredChoice(output) => {
            println!("{output}");
            return Ok(());
        }
    };
    let mut orgize_args = Vec::with_capacity(capture_args.len() + 1);
    orgize_args.push("capture-plan".to_string());
    orgize_args.extend(capture_args.iter().cloned());
    if !capture_contract_registry_provided(args) {
        orgize_args.push("--org-contract-registry".to_string());
        orgize_args.push(contract_registry_path.display().to_string());
    }
    agent::run_org_cli_command(orgize_args)
}

pub(crate) fn run_org_state_sync(project_root: &Path) -> Result<OrgStateSync, String> {
    let state_root = project_state_paths(project_root)?.protocol_home.join("org");
    let sync = sync_default_org_state(project_root, &state_root)?;
    let artifacts_root = org_artifacts_root(&state_root)?;
    migrate_legacy_flow(&state_root, &artifacts_root)?;
    ensure_flow_dirs(&artifacts_root)?;
    Ok(sync)
}

pub(crate) fn org_artifacts_root_for_project(project_root: &Path) -> Result<PathBuf, String> {
    let state_root = project_state_paths(project_root)?.protocol_home.join("org");
    org_artifacts_root(&state_root)
}

#[derive(Debug, Clone)]
pub(crate) struct OrgStateSync {
    pub(crate) source: String,
    pub(crate) status: &'static str,
    pub(crate) legacy_backup: Option<PathBuf>,
}

fn sync_default_org_state(project_root: &Path, state_root: &Path) -> Result<OrgStateSync, String> {
    let repo_url = default_org_repo_url();
    sync_org_state_repo(project_root, state_root, &repo_url)
}

fn default_org_repo_url() -> String {
    env::var(ASP_ORG_REPO_URL_ENV)
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| DEFAULT_ASP_ORG_REPO_URL.to_string())
}

fn sync_org_state_repo(
    project_root: &Path,
    state_root: &Path,
    repo_url: &str,
) -> Result<OrgStateSync, String> {
    if state_root.join(".git").is_dir() {
        ensure_org_repo_remote(state_root, repo_url)?;
        if !git_output(&["status", "--porcelain"], Some(state_root))?
            .trim()
            .is_empty()
        {
            ensure_org_repo_local_excludes(state_root)?;
            return Ok(OrgStateSync {
                source: repo_url.to_string(),
                status: "dirty-skipped",
                legacy_backup: None,
            });
        }
        run_git(&["pull", "--ff-only"], Some(state_root))?;
        ensure_org_repo_local_excludes(state_root)?;
        return Ok(OrgStateSync {
            source: repo_url.to_string(),
            status: "updated",
            legacy_backup: None,
        });
    }

    let legacy_backup = if state_root.exists() {
        let backup = legacy_backup_path(project_root, state_root)?;
        fs::rename(state_root, &backup).map_err(|error| {
            format!(
                "failed to preserve existing ASP Org state {} as {}: {error}",
                state_root.display(),
                backup.display()
            )
        })?;
        Some(backup)
    } else {
        None
    };

    if let Some(parent) = state_root.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("failed to create {}: {error}", parent.display()))?;
    }
    let state_root_string = state_root.display().to_string();
    run_git(&["clone", repo_url, &state_root_string], None)?;
    if let Some(backup) = legacy_backup.as_ref() {
        restore_legacy_flow(backup, state_root)?;
    }
    ensure_org_repo_local_excludes(state_root)?;
    Ok(OrgStateSync {
        source: repo_url.to_string(),
        status: "cloned",
        legacy_backup,
    })
}

fn ensure_org_repo_remote(state_root: &Path, repo_url: &str) -> Result<(), String> {
    let current = git_output(&["remote", "get-url", "origin"], Some(state_root))?;
    if current.trim() != repo_url {
        run_git(&["remote", "set-url", "origin", repo_url], Some(state_root))?;
    }
    Ok(())
}

fn restore_legacy_flow(backup: &Path, state_root: &Path) -> Result<(), String> {
    let legacy_flow = backup.join("flow");
    if !legacy_flow.is_dir() {
        return Ok(());
    }
    let artifacts_root = org_artifacts_root(state_root)?;
    let target_flow = artifacts_root.join("flow");
    if target_flow.exists() {
        return Ok(());
    }
    if let Some(parent) = target_flow.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("failed to create {}: {error}", parent.display()))?;
    }
    fs::rename(&legacy_flow, &target_flow).map_err(|error| {
        format!(
            "failed to restore legacy ASP Org flow {} to {}: {error}",
            legacy_flow.display(),
            target_flow.display()
        )
    })
}

fn migrate_legacy_flow(state_root: &Path, artifacts_root: &Path) -> Result<(), String> {
    let legacy_flow = state_root.join("flow");
    if !legacy_flow.is_dir() {
        return Ok(());
    }
    let target_flow = artifacts_root.join("flow");
    if target_flow.exists() {
        return Ok(());
    }
    if let Some(parent) = target_flow.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("failed to create {}: {error}", parent.display()))?;
    }
    fs::rename(&legacy_flow, &target_flow).map_err(|error| {
        format!(
            "failed to migrate legacy ASP Org flow {} to {}: {error}",
            legacy_flow.display(),
            target_flow.display()
        )
    })
}

fn org_artifacts_root(state_root: &Path) -> Result<PathBuf, String> {
    let protocol_home = state_root.parent().ok_or_else(|| {
        format!(
            "failed to compute ASP Org artifacts root for {}",
            state_root.display()
        )
    })?;
    Ok(protocol_home.join(ORG_ARTIFACTS_DIR))
}

fn ensure_org_repo_local_excludes(state_root: &Path) -> Result<(), String> {
    let exclude_path = state_root.join(".git").join("info").join("exclude");
    let mut contents = fs::read_to_string(&exclude_path).unwrap_or_default();
    if contents.lines().any(|line| line.trim() == "flow/") {
        return Ok(());
    }
    if !contents.is_empty() && !contents.ends_with('\n') {
        contents.push('\n');
    }
    contents.push_str("flow/\n");
    fs::write(&exclude_path, contents)
        .map_err(|error| format!("failed to update {}: {error}", exclude_path.display()))
}

fn legacy_backup_path(project_root: &Path, state_root: &Path) -> Result<PathBuf, String> {
    let parent = state_root.parent().ok_or_else(|| {
        format!(
            "failed to compute legacy backup path for {}",
            state_root.display()
        )
    })?;
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|error| format!("system time before Unix epoch: {error}"))?
        .as_nanos();
    let name = state_root
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("org");
    Ok(parent.join(format!(
        ".{name}.legacy-{}-{nonce}",
        path_segment(&project_root.display().to_string())
    )))
}

fn run_git(args: &[&str], cwd: Option<&Path>) -> Result<(), String> {
    let output = git_output_bytes(args, cwd)?;
    if output.status.success() {
        return Ok(());
    }
    Err(format!(
        "git {} failed with status {}: {}",
        args.join(" "),
        output.status,
        String::from_utf8_lossy(&output.stderr).trim()
    ))
}

fn git_output(args: &[&str], cwd: Option<&Path>) -> Result<String, String> {
    let output = git_output_bytes(args, cwd)?;
    if !output.status.success() {
        return Err(format!(
            "git {} failed with status {}: {}",
            args.join(" "),
            output.status,
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    String::from_utf8(output.stdout)
        .map_err(|error| format!("git {} output was not UTF-8: {error}", args.join(" ")))
}

fn git_output_bytes(args: &[&str], cwd: Option<&Path>) -> Result<std::process::Output, String> {
    let mut not_found = Vec::new();
    for git in git_command_candidates() {
        let mut command = Command::new(&git);
        command.args(args);
        if let Some(cwd) = cwd {
            command.current_dir(cwd);
        }
        match command.output() {
            Ok(output) if output.status.success() => return Ok(output),
            Ok(output) if looks_like_tool_resolution_failure(&output.stderr) => {
                not_found.push(format!(
                    "{git}: {}",
                    String::from_utf8_lossy(&output.stderr).trim()
                ));
            }
            Ok(output) => return Ok(output),
            Err(error) if error.kind() == ErrorKind::NotFound => {
                not_found.push(format!("{git}: {error}"));
            }
            Err(error) => {
                return Err(format!("failed to run {git} {}: {error}", args.join(" ")));
            }
        }
    }
    Err(format!(
        "failed to find git for {}: {}",
        args.join(" "),
        not_found.join("; ")
    ))
}

fn git_command_candidates() -> Vec<String> {
    let mut candidates = Vec::new();
    push_git_candidate_from_env(&mut candidates, "ASP_GIT_BIN");
    candidates.push("git".to_string());
    for path in [
        "/usr/bin/git",
        "/opt/homebrew/bin/git",
        "/usr/local/bin/git",
        "/run/current-system/sw/bin/git",
    ] {
        push_git_candidate(&mut candidates, path);
    }
    if let Ok(user) = env::var("USER") {
        push_git_candidate(
            &mut candidates,
            &format!("/etc/profiles/per-user/{user}/bin/git"),
        );
    }
    if let Ok(home) = env::var("HOME") {
        push_git_candidate(&mut candidates, &format!("{home}/.nix-profile/bin/git"));
    }
    candidates
}

fn push_git_candidate_from_env(candidates: &mut Vec<String>, key: &str) {
    if let Ok(value) = env::var(key) {
        push_git_candidate(candidates, value.trim());
    }
}

fn push_git_candidate(candidates: &mut Vec<String>, path: &str) {
    if !path.is_empty() && !candidates.iter().any(|candidate| candidate == path) {
        candidates.push(path.to_string());
    }
}

fn looks_like_tool_resolution_failure(stderr: &[u8]) -> bool {
    let stderr = String::from_utf8_lossy(stderr);
    stderr.contains("tool 'git' not found") || stderr.contains("tool `git` not found")
}

fn ensure_flow_dirs(artifacts_root: &Path) -> Result<Vec<PathBuf>, String> {
    FLOW_DIRS
        .iter()
        .map(|dir| {
            let path = artifacts_root.join("flow").join(dir);
            fs::create_dir_all(&path)
                .map_err(|error| format!("failed to create {}: {error}", path.display()))?;
            Ok(path)
        })
        .collect()
}

fn path_segment(value: &str) -> String {
    value
        .chars()
        .map(|character| match character {
            'A'..='Z' | 'a'..='z' | '0'..='9' | '.' | '-' | '_' => character,
            _ => '_',
        })
        .collect()
}

fn required_flag_value<'a>(
    args: &'a [String],
    index: usize,
    flag: &str,
) -> Result<&'a str, String> {
    args.get(index)
        .map(String::as_str)
        .ok_or_else(|| format!("{flag} requires a value"))
}

fn capture_usage() -> &'static str {
    "usage: asp org capture --contract CONTRACT_ID --title TITLE --target-file ORG_FILE [--choice KEY=VALUE] [--outline OUTLINE] [--kind KIND] [--tag TAG] [--property KEY=VALUE] [--body TEXT]\n\n`capture --contract CONTRACT_ID ...` renders a non-mutating Org entry and validates it against the ASP Org contract registry before returning org-entry. CONTRACT_ID must be explicit, such as agent.task.v1, agent.plan.v1, agent.sdd.v1, agent.adr.v1, agent.bdd.v1, agent.tdd.v1, agent.bdr.v1, agent.prd.v1, or agent.execplan.v1. The agent.task.v1 and agent.plan.v1 capture shapes are materialized from .cache/agent-semantic-protocol/org/templates/<CONTRACT_ID>.org unless the caller overrides kind, tags, properties, or body. When a contract declares `org-contract :type agent-interactive` with `method: choice` and `stage: pre-capture`, capture prints the compact choice window until the caller passes `--choice <id>=N|ID|?`; `<id>` comes from that Org block. ASP resolves CONTRACT_ID from .cache/agent-semantic-protocol/org/contracts/<CONTRACT_ID>.org, synchronizing the Org resource git remote and creating artifacts/org/flow/{plans,sdd,bdd,tdd,bdr} when needed."
}

fn capture_contract_requested(args: &[String]) -> bool {
    args.iter().any(|arg| arg == "--contract")
}

fn capture_contract_registry_provided(args: &[String]) -> bool {
    let flags = ["--org-contract-registry", "--contract-registry"];
    args.iter().any(|arg| {
        flags
            .iter()
            .any(|flag| arg == flag || arg.starts_with(&format!("{flag}=")))
    })
}

fn capture_contract_id(args: &[String]) -> Result<String, String> {
    let Some(index) = args.iter().position(|arg| arg == "--contract") else {
        return Err("asp org capture requires --contract CONTRACT_ID".to_string());
    };
    required_flag_value(args, index + 1, "--contract").and_then(|value| {
        if value.trim().is_empty() {
            Err("asp org capture --contract must not be empty".to_string())
        } else {
            Ok(value.to_string())
        }
    })
}

fn resolve_capture_contract_registry(contract_id: &str) -> Result<PathBuf, String> {
    let project_root =
        env::current_dir().map_err(|error| format!("failed to read current directory: {error}"))?;
    let state_root = project_state_paths(&project_root)?
        .protocol_home
        .join("org");
    let registry_path = state_root
        .join("contracts")
        .join(contract_registry_file_name(contract_id)?);
    if !registry_path.is_file() {
        run_org_state_sync(&project_root)?;
    }
    if registry_path.is_file() {
        return Ok(registry_path);
    }
    Err(format!(
        "ASP Org contract `{contract_id}` was not found at {}; run `asp sync` or pass --org-contract-registry PATH.org",
        registry_path.display()
    ))
}

fn resolve_capture_contract_registry_from_args(
    args: &[String],
    contract_id: &str,
) -> Result<PathBuf, String> {
    if let Some(path) = capture_contract_registry_arg(args)? {
        return Ok(path);
    }
    resolve_capture_contract_registry(contract_id)
}

fn capture_contract_registry_arg(args: &[String]) -> Result<Option<PathBuf>, String> {
    let flags = ["--org-contract-registry", "--contract-registry"];
    for (index, arg) in args.iter().enumerate() {
        for flag in flags {
            if arg == flag {
                let value = args
                    .get(index + 1)
                    .ok_or_else(|| format!("{flag} requires a value"))?;
                return Ok(Some(PathBuf::from(value)));
            }
            if let Some(value) = arg.strip_prefix(&format!("{flag}=")) {
                return Ok(Some(PathBuf::from(value)));
            }
        }
    }
    Ok(None)
}

fn resolve_capture_template(contract_id: &str) -> Result<Option<PathBuf>, String> {
    let project_root =
        env::current_dir().map_err(|error| format!("failed to read current directory: {error}"))?;
    let state_root = project_state_paths(&project_root)?
        .protocol_home
        .join("org");
    let template_file_name = contract_registry_file_name(contract_id)?;
    let template_path = state_root.join("templates").join(&template_file_name);
    if !template_path.is_file() {
        run_org_state_sync(&project_root)?;
    }
    if template_path.is_file() {
        return Ok(Some(template_path));
    }
    Ok(None)
}

fn contract_registry_file_name(contract_id: &str) -> Result<String, String> {
    if contract_id
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '.' | '_' | '-'))
    {
        Ok(format!("{contract_id}.org"))
    } else {
        Err(format!(
            "ASP Org contract id `{contract_id}` must be a registry id such as agent.plan.v1"
        ))
    }
}
