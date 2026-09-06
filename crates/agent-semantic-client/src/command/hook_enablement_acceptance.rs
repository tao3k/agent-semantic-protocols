//! Read-only acceptance authority for enabling the Codex Hook.

use std::path::Path;
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering;
use std::time::Duration;
use std::time::Instant;

use serde::Serialize;
use tokio::io::AsyncWriteExt as _;

const LAUNCHER_SAMPLE_COUNT: usize = 16;
const LAUNCHER_SAMPLE_BUDGET: Duration = Duration::from_millis(100);
const LAUNCHER_P99_BUDGET_MICROS: u64 = 50_000;
const POLICY_DECISION_SAMPLE_COUNT: usize = 16;
const POLICY_DECISION_BUDGET_MICROS: u64 = 1_000;
static ACCEPTANCE_NONCE: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct HookEnablementAcceptanceReceipt {
    schema_id: &'static str,
    schema_version: &'static str,
    status: &'static str,
    reason_kind: &'static str,
    project_root: PathBuf,
    plugin_launcher: PathBuf,
    active_digest: String,
    healthy_digest: String,
    retention_protected_digests: Vec<String>,
    policy_decision_samples: usize,
    policy_decision_max_cpu_micros: u64,
    policy_decision_budget_micros: u64,
    launcher_samples: usize,
    launcher_p99_micros: u64,
    launcher_max_micros: u64,
}

pub(super) async fn run(args: &[String]) -> Result<(), String> {
    let mut json = false;
    let mut project_root = None;
    for arg in args {
        match arg.as_str() {
            "--json" => json = true,
            "--help" | "-h" => return Err(usage()),
            value if value.starts_with('-') => {
                return Err(format!(
                    "unknown Hook enablement option {value}\n{}",
                    usage()
                ));
            }
            value if project_root.is_none() => project_root = Some(PathBuf::from(value)),
            _ => return Err(usage()),
        }
    }
    let project_root = project_root.unwrap_or_else(|| PathBuf::from("."));
    let project_root = tokio::fs::canonicalize(&project_root)
        .await
        .map_err(|error| {
            format!(
                "resolve Hook enablement project root {}: {error}",
                project_root.display()
            )
        })?;
    let receipt = accept(&project_root).await?;
    if json {
        println!(
            "{}",
            serde_json::to_string(&receipt)
                .map_err(|error| format!("encode Hook enablement receipt: {error}"))?
        );
    } else {
        println!(
            "[hook-enablement] status=ready reasonKind=none activeDigest={} healthyDigest={} policyDecisionSamples={} policyDecisionMaxCpuMicros={} policyDecisionBudgetMicros={} launcherSamples={} launcherP99Micros={} launcherMaxMicros={} pluginLauncher={}",
            receipt.active_digest,
            receipt.healthy_digest,
            receipt.policy_decision_samples,
            receipt.policy_decision_max_cpu_micros,
            receipt.policy_decision_budget_micros,
            receipt.launcher_samples,
            receipt.launcher_p99_micros,
            receipt.launcher_max_micros,
            receipt.plugin_launcher.display(),
        );
    }
    Ok(())
}

async fn accept(project_root: &Path) -> Result<HookEnablementAcceptanceReceipt, String> {
    let launcher = project_root.join("asp-codex-plugin/bin/asp-hook-exec");
    let hooks = project_root.join("asp-codex-plugin/hooks/hooks.json");
    require_executable(&launcher).await?;
    require_regular_file(&hooks, "Codex Hook manifest").await?;
    admit_global_plugin_config(project_root).await?;

    let runtime_state = agent_semantic_runtime::project_runtime_state(project_root)?;
    let runtime_root = runtime_state.protocol_home.join("runtime");
    let public = runtime_root.join("bin/asp");
    let active_slot = runtime_root.join("resident/active/asp");
    let healthy_slot = runtime_root.join("resident/healthy/asp");
    require_symlink_target(&public, &active_slot, "public ASP alias").await?;
    let active = canonical_executable(&active_slot, "active ASP artifact").await?;
    let healthy = canonical_executable(&healthy_slot, "healthy ASP artifact").await?;
    let active_digest = artifact_digest(&active)?;
    let healthy_digest = artifact_digest(&healthy)?;
    let mut protected = vec![active_digest.clone(), healthy_digest.clone()];
    protected.sort();
    protected.dedup();
    admit_retention_receipt(&runtime_root, &protected).await?;

    let policy_decision_max_cpu_micros =
        policy_decision_acceptance(&active, project_root, &runtime_state.protocol_home).await?;
    let mut samples = launcher_scenario_samples(&launcher, &active, LAUNCHER_SAMPLE_COUNT).await?;
    samples.sort_unstable();
    let launcher_max_micros = *samples
        .last()
        .ok_or_else(|| "Hook enablement produced no launcher samples".to_owned())?;
    let p99_index = ((samples.len() * 99).div_ceil(100)).saturating_sub(1);
    let launcher_p99_micros = samples[p99_index];
    if launcher_max_micros >= LAUNCHER_SAMPLE_BUDGET.as_micros() as u64 {
        return Err(format!(
            "Hook launcher sample exceeds enablement budget: reasonKind=launcher-budget-exceeded maxMicros={launcher_max_micros} budgetMicros={}",
            LAUNCHER_SAMPLE_BUDGET.as_micros()
        ));
    }
    if launcher_p99_micros >= LAUNCHER_P99_BUDGET_MICROS {
        return Err(format!(
            "Hook launcher p99 exceeds enablement budget: reasonKind=launcher-p99-budget-exceeded p99Micros={launcher_p99_micros} budgetMicros={LAUNCHER_P99_BUDGET_MICROS}"
        ));
    }

    Ok(HookEnablementAcceptanceReceipt {
        schema_id: "agent.semantic-protocols.hook-enablement-acceptance",
        schema_version: "1",
        status: "ready",
        reason_kind: "none",
        project_root: project_root.to_path_buf(),
        plugin_launcher: launcher,
        active_digest,
        healthy_digest,
        retention_protected_digests: protected,
        policy_decision_samples: POLICY_DECISION_SAMPLE_COUNT,
        policy_decision_max_cpu_micros,
        policy_decision_budget_micros: POLICY_DECISION_BUDGET_MICROS,
        launcher_samples: samples.len(),
        launcher_p99_micros,
        launcher_max_micros,
    })
}

async fn admit_global_plugin_config(project_root: &Path) -> Result<(), String> {
    let codex_home = std::env::var_os("CODEX_HOME")
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var_os("HOME")
                .map(PathBuf::from)
                .map(|home| home.join(".codex"))
        })
        .ok_or_else(|| {
            "resolve global Codex home: reasonKind=global-codex-home-unavailable".to_owned()
        })?;
    let config_path = codex_home.join("config.toml");
    let input = tokio::fs::read_to_string(&config_path)
        .await
        .map_err(|error| {
            format!(
                "read global Codex plugin configuration {}: {error}",
                config_path.display()
            )
        })?;
    let document: toml::Value = toml::from_str(&input)
        .map_err(|error| format!("parse global Codex plugin configuration: {error}"))?;
    let source = document
        .get("marketplaces")
        .and_then(|value| value.get("asp-project"))
        .and_then(|value| value.get("source"))
        .and_then(toml::Value::as_str)
        .map(PathBuf::from)
        .ok_or_else(|| {
            "Global Codex marketplace has no ASP source: reasonKind=plugin-marketplace-missing"
                .to_owned()
        })?;
    let source = tokio::fs::canonicalize(&source).await.map_err(|error| {
        format!(
            "resolve global Codex marketplace source {}: {error}",
            source.display()
        )
    })?;
    if source != project_root {
        return Err(format!(
            "Global Codex marketplace source drift: reasonKind=plugin-marketplace-source-drift expected={} actual={}",
            project_root.display(),
            source.display()
        ));
    }
    let enabled = document
        .get("plugins")
        .and_then(|value| value.get("asp-codex-plugin@asp-project"))
        .and_then(|value| value.get("enabled"))
        .and_then(toml::Value::as_bool)
        .unwrap_or(false);
    if !enabled {
        return Err(
            "Global ASP Codex plugin is disabled: reasonKind=plugin-not-enabled".to_owned(),
        );
    }
    Ok(())
}

async fn admit_retention_receipt(
    runtime_root: &Path,
    required_digests: &[String],
) -> Result<(), String> {
    let receipt_path = runtime_root.join("artifacts/retention-receipt.json");
    let bytes = tokio::fs::read(&receipt_path).await.map_err(|error| {
        format!(
            "read Runtime artifact retention receipt {}: {error}",
            receipt_path.display()
        )
    })?;
    let receipt: agent_semantic_artifacts::runtime_artifact_retention::RuntimeArtifactRetentionReceipt =
        serde_json::from_slice(&bytes)
            .map_err(|error| format!("parse Runtime artifact retention receipt: {error}"))?;
    if receipt.schema_version != "1"
        || receipt.retention_policy != "active-healthy-reachability"
        || receipt.retained_slots_per_binary != 2
        || !required_digests
            .iter()
            .all(|digest| receipt.protected_digests.contains(digest))
    {
        return Err(format!(
            "Runtime artifact retention does not protect Hook slots: reasonKind=retention-receipt-drift required={required_digests:?} protected={:?}",
            receipt.protected_digests
        ));
    }
    Ok(())
}

async fn policy_decision_acceptance(
    active: &Path,
    project_root: &Path,
    state_home: &Path,
) -> Result<u64, String> {
    // Readiness warms the immutable matcher generation before enabling the
    // Host. Every decision after this control-plane admission is measured;
    // Ready therefore never hides a failed post-warm sample.
    let _ = policy_decision_sample(active, project_root, state_home, false).await?;
    let mut max_cpu_micros = 0;
    for sample in 0..POLICY_DECISION_SAMPLE_COUNT {
        let cpu = policy_decision_sample(active, project_root, state_home, true)
            .await
            .map_err(|error| format!("policy decision sample {sample} failed: {error}"))?;
        max_cpu_micros = max_cpu_micros.max(cpu);
    }
    Ok(max_cpu_micros)
}

async fn policy_decision_sample(
    active: &Path,
    project_root: &Path,
    state_home: &Path,
    enforce_budget: bool,
) -> Result<u64, String> {
    let payload = serde_json::json!({
        "tool_name": "Bash",
        "tool_input": {"command": "cat Cargo.toml"}
    });
    let output = run_process(
        active,
        &[
            "hook", "pre-tool", "--client", "codex", "--emit", "decision",
        ],
        project_root,
        state_home,
        false,
        serde_json::to_vec(&payload)
            .map_err(|error| format!("encode Hook acceptance payload: {error}"))?,
    )
    .await
    .map_err(|error| format!("policy decision acceptance failed: {error}"))?;
    let decision: serde_json::Value = serde_json::from_slice(&output).map_err(|error| {
        format!(
            "parse Hook acceptance decision: reasonKind=policy-decision-invalid error={error} output={}",
            String::from_utf8_lossy(&output)
        )
    })?;
    let fields = decision
        .get("fields")
        .and_then(serde_json::Value::as_object)
        .ok_or_else(|| {
            "Hook acceptance decision has no fields: reasonKind=policy-decision-invalid".to_owned()
        })?;
    let status = fields
        .get("hookDecisionBudgetStatus")
        .and_then(serde_json::Value::as_str);
    let budget = fields
        .get("hookDecisionBudgetMicros")
        .and_then(serde_json::Value::as_u64);
    let cpu = fields
        .get("hookDecisionCpuMicros")
        .and_then(serde_json::Value::as_u64)
        .ok_or_else(|| {
            "Hook acceptance decision has no CPU receipt: reasonKind=policy-decision-cpu-missing"
                .to_owned()
        })?;
    if budget != Some(POLICY_DECISION_BUDGET_MICROS)
        || (enforce_budget && status != Some("within-budget"))
    {
        return Err(format!(
            "Hook policy decision exceeds enablement budget: reasonKind=policy-decision-budget-exceeded cpuMicros={cpu} budgetMicros={budget:?} status={status:?}"
        ));
    }
    Ok(cpu)
}

async fn launcher_scenario_samples(
    launcher: &Path,
    artifact: &Path,
    count: usize,
) -> Result<Vec<u64>, String> {
    let root = std::env::temp_dir().join(format!(
        "asp-hook-enablement-{}-{}",
        std::process::id(),
        ACCEPTANCE_NONCE.fetch_add(1, Ordering::Relaxed)
    ));
    let public_path = agent_semantic_artifacts::RuntimeArtifactStateLayout::new(&root)
        .active_slot()
        .join("asp");
    tokio::fs::create_dir_all(public_path.parent().expect("public binary parent"))
        .await
        .map_err(|error| format!("create Hook acceptance scenario: {error}"))?;
    create_file_symlink(artifact, &public_path).await?;
    let mut samples = Vec::with_capacity(count);
    let result = async {
        for _ in 0..count {
            let started = Instant::now();
            let output = run_process(
                launcher,
                &["pre-tool", "--client", "codex"],
                Path::new("/"),
                &root,
                true,
                Vec::new(),
            )
            .await
            .map_err(|error| {
                format!("isolated canonical launcher acceptance failed: {error}")
            })?;
            let elapsed = u64::try_from(started.elapsed().as_micros()).unwrap_or(u64::MAX);
            let value: serde_json::Value = serde_json::from_slice(&output).map_err(|error| {
                format!(
                    "parse isolated canonical launcher output: reasonKind=launcher-output-invalid error={error} output={}",
                    String::from_utf8_lossy(&output)
                )
            })?;
            if value != serde_json::json!({}) {
                return Err(format!(
                    "isolated canonical launcher did not passthrough: reasonKind=launcher-passthrough-drift output={value}"
                ));
            }
            samples.push(elapsed);
        }
        Ok(samples)
    }
    .await;
    let _ = tokio::fs::remove_dir_all(&root).await;
    result
}

async fn run_process(
    program: &Path,
    args: &[&str],
    cwd: &Path,
    state_home: &Path,
    no_agent: bool,
    stdin: Vec<u8>,
) -> Result<Vec<u8>, String> {
    let mut command = tokio::process::Command::new(program);
    command
        .args(args)
        .current_dir(cwd)
        .env("ASP_STATE_HOME", state_home)
        .env_remove("ASP_NO_AGENT")
        .kill_on_drop(true)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if no_agent {
        command.env("ASP_NO_AGENT", "1");
    }
    let mut child = command.spawn().map_err(|error| {
        format!(
            "spawn Hook enablement process {}: reasonKind=launcher-spawn-failed error={error}",
            program.display()
        )
    })?;
    if let Some(mut child_stdin) = child.stdin.take() {
        child_stdin
            .write_all(&stdin)
            .await
            .map_err(|error| format!("write Hook enablement stdin: {error}"))?;
        child_stdin
            .shutdown()
            .await
            .map_err(|error| format!("close Hook enablement stdin: {error}"))?;
    }
    let output = tokio::time::timeout(LAUNCHER_SAMPLE_BUDGET, child.wait_with_output())
        .await
        .map_err(|_| {
            format!(
                "Hook enablement process exceeded budget: reasonKind=launcher-budget-exceeded budgetMicros={}",
                LAUNCHER_SAMPLE_BUDGET.as_micros()
            )
        })?
        .map_err(|error| format!("wait for Hook enablement process: {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "Hook enablement process failed: reasonKind=launcher-process-failed status={} stderr={}",
            output.status,
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    Ok(output.stdout)
}

async fn require_symlink_target(link: &Path, expected: &Path, label: &str) -> Result<(), String> {
    let target = tokio::fs::read_link(link)
        .await
        .map_err(|error| format!("read {label} {}: {error}", link.display()))?;
    if target != expected {
        return Err(format!(
            "{label} drift: reasonKind=artifact-slot-alias-drift expected={} actual={}",
            expected.display(),
            target.display()
        ));
    }
    Ok(())
}

async fn canonical_executable(path: &Path, label: &str) -> Result<PathBuf, String> {
    let canonical = tokio::fs::canonicalize(path)
        .await
        .map_err(|error| format!("resolve {label} {}: {error}", path.display()))?;
    require_executable(&canonical).await?;
    Ok(canonical)
}

async fn require_executable(path: &Path) -> Result<(), String> {
    let metadata = require_regular_file(path, "Hook executable").await?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        if metadata.permissions().mode() & 0o111 == 0 {
            return Err(format!(
                "Hook executable has no execute bit: reasonKind=artifact-not-executable path={}",
                path.display()
            ));
        }
    }
    Ok(())
}

async fn require_regular_file(path: &Path, label: &str) -> Result<std::fs::Metadata, String> {
    let metadata = tokio::fs::metadata(path)
        .await
        .map_err(|error| format!("inspect {label} {}: {error}", path.display()))?;
    if !metadata.is_file() {
        return Err(format!(
            "{label} is not a regular file: reasonKind=artifact-not-file path={}",
            path.display()
        ));
    }
    Ok(metadata)
}

fn artifact_digest(path: &Path) -> Result<String, String> {
    crate::command::protocol_binary::protocol_binary_digest_from_canonical_artifact_path(path).ok_or_else(
        || {
            format!(
                "Runtime artifact is not digest-addressed: reasonKind=artifact-content-path-invalid path={}",
                path.display()
            )
        },
    )
}

#[cfg(unix)]
async fn create_file_symlink(source: &Path, target: &Path) -> Result<(), String> {
    tokio::fs::symlink(source, target)
        .await
        .map_err(|error| format!("create Hook acceptance slot {}: {error}", target.display()))
}

#[cfg(windows)]
async fn create_file_symlink(source: &Path, target: &Path) -> Result<(), String> {
    tokio::fs::symlink_file(source, target)
        .await
        .map_err(|error| format!("create Hook acceptance slot {}: {error}", target.display()))
}

fn usage() -> String {
    "usage: asp hook enablement [PROJECT_ROOT] [--json]".to_owned()
}
