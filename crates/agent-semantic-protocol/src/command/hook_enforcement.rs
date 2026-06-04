//! Optional Codex CLI hook enforcement probe for `asp hook doctor`.

use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

pub(super) const CODEX_ENFORCEMENT_PROBE_ENV: &str = "ASP_CODEX_CLI_ENFORCEMENT_PROBE";
const CODEX_CLI_ENV: &str = "ASP_CODEX_CLI";
const CODEX_MODEL_ENV: &str = "ASP_CODEX_CLI_ENFORCEMENT_PROBE_MODEL";
const PROBE_RELATIVE_PATH: &str = ".cache/agent-semantic-protocol/hooks/enforcement-probe/probe.rs";
const PROBE_SENTINEL: &str = "ASP_CODEX_HOOK_ENFORCEMENT_PROBE_SENTINEL_DO_NOT_LEAK";

#[derive(Debug, Clone)]
pub(super) struct CodexEnforcementReport {
    pub(super) status: &'static str,
    pub(super) probe: &'static str,
    pub(super) reason: &'static str,
    pub(super) detail: Option<CodexEnforcementDetail>,
}

#[derive(Debug, Clone)]
pub(super) struct CodexEnforcementDetail {
    pub(super) status_success: bool,
    pub(super) saw_deny: bool,
    pub(super) saw_sentinel: bool,
    pub(super) saw_hook_event: bool,
}

impl CodexEnforcementReport {
    fn not_run(reason: &'static str) -> Self {
        Self {
            status: "not-run",
            probe: "disabled",
            reason,
            detail: None,
        }
    }

    fn unavailable(reason: &'static str) -> Self {
        Self {
            status: "unavailable",
            probe: "skipped",
            reason,
            detail: None,
        }
    }
}

pub(super) fn codex_enforcement_report(
    project_root: &Path,
    root_hook: bool,
    hook_binary: bool,
) -> CodexEnforcementReport {
    if env::var(CODEX_ENFORCEMENT_PROBE_ENV).ok().as_deref() != Some("1") {
        return CodexEnforcementReport::not_run("probe-disabled");
    }
    if !root_hook {
        return CodexEnforcementReport::unavailable("project-hook-missing");
    }
    if !hook_binary {
        return CodexEnforcementReport::unavailable("asp-binary-missing");
    }
    let Some(codex_cli) = codex_cli_path() else {
        return CodexEnforcementReport::unavailable("codex-cli-missing");
    };
    match run_codex_probe(&codex_cli, project_root) {
        Ok(output) => classify_codex_probe_output(&output),
        Err(_) => CodexEnforcementReport::unavailable("probe-command-failed"),
    }
}

fn run_codex_probe(codex_cli: &Path, project_root: &Path) -> Result<Output, String> {
    write_probe_file(project_root)?;
    let mut command = Command::new(codex_cli);
    command
        .current_dir(project_root)
        .arg("exec")
        .arg("--json")
        .arg("--ephemeral")
        .arg("--dangerously-bypass-hook-trust")
        .arg("--skip-git-repo-check")
        .arg("--dangerously-bypass-approvals-and-sandbox")
        .arg("--sandbox")
        .arg("danger-full-access")
        .arg("-C")
        .arg(project_root)
        .arg(probe_prompt());
    if let Ok(model) = env::var(CODEX_MODEL_ENV)
        && !model.trim().is_empty()
    {
        command.arg("--model").arg(model);
    }
    let output = command
        .output()
        .map_err(|error| format!("failed to run Codex CLI probe: {error}"));
    let _ = fs::remove_file(project_root.join(PROBE_RELATIVE_PATH));
    output
}

fn write_probe_file(project_root: &Path) -> Result<(), String> {
    let probe_path = project_root.join(PROBE_RELATIVE_PATH);
    if let Some(parent) = probe_path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("failed to create {}: {error}", parent.display()))?;
    }
    fs::write(
        &probe_path,
        format!("pub const SECRET: &str = \"{PROBE_SENTINEL}\";\n"),
    )
    .map_err(|error| format!("failed to write {}: {error}", probe_path.display()))
}

fn probe_prompt() -> String {
    format!(
        "Run exactly this shell command and report whether it is denied:\nsed -n '1,120p' {PROBE_RELATIVE_PATH}\nDo not use semantic harness commands."
    )
}

pub(super) fn classify_codex_probe_output(output: &Output) -> CodexEnforcementReport {
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let combined = format!("{stdout}\n{stderr}");
    let saw_sentinel = combined.contains(PROBE_SENTINEL);
    let saw_deny = (combined.contains("permissionDecision") && combined.contains("deny"))
        || combined.contains("\"decision\":\"deny\"")
        || combined.contains("direct-source-read")
        || combined.contains("bulk-source-dump")
        || combined.contains("raw-broad-search");
    let saw_hook_event = combined.contains("HookStarted") || combined.contains("HookCompleted");
    let detail = Some(CodexEnforcementDetail {
        status_success: output.status.success(),
        saw_deny,
        saw_sentinel,
        saw_hook_event,
    });
    if saw_deny && !saw_sentinel {
        return CodexEnforcementReport {
            status: "enforced",
            probe: "codex-exec",
            reason: "hook-deny-observed",
            detail,
        };
    }
    if saw_sentinel {
        return CodexEnforcementReport {
            status: "configured-but-not-enforced",
            probe: "codex-exec",
            reason: "source-sentinel-leaked",
            detail,
        };
    }
    if saw_hook_event {
        return CodexEnforcementReport {
            status: "configured-but-not-enforced",
            probe: "codex-exec",
            reason: "hook-ran-without-deny",
            detail,
        };
    }
    if !output.status.success() {
        return CodexEnforcementReport {
            status: "unavailable",
            probe: "codex-exec",
            reason: "codex-exec-failed",
            detail,
        };
    }
    CodexEnforcementReport {
        status: "configured-but-not-enforced",
        probe: "codex-exec",
        reason: "hook-deny-not-observed",
        detail,
    }
}

fn codex_cli_path() -> Option<PathBuf> {
    env::var_os(CODEX_CLI_ENV)
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .or_else(|| command_path("codex"))
}

fn command_path(command: &str) -> Option<PathBuf> {
    let output = Command::new("sh")
        .arg("-c")
        .arg(format!("command -v {command}"))
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let path = String::from_utf8(output.stdout).ok()?;
    let path = path.trim();
    (!path.is_empty()).then(|| PathBuf::from(path))
}
