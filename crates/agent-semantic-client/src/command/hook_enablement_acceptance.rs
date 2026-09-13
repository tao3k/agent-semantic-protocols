// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

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
    host_acceptance: Option<crate::command::hook_host_acceptance::HostAcceptanceReceipt>,
}

#[derive(Debug)]
struct HostEvidenceArgs {
    rollout: PathBuf,
    hook_events: PathBuf,
    probe_path: String,
    sentinel: String,
}

#[derive(Debug)]
struct EnablementArgs {
    json: bool,
    project_root: PathBuf,
    host_evidence: Option<HostEvidenceArgs>,
}

pub(super) async fn run(args: &[String]) -> Result<(), String> {
    let parsed = parse_args(args)?;
    let project_root = tokio::fs::canonicalize(&parsed.project_root)
        .await
        .map_err(|error| {
            format!(
                "resolve Hook enablement project root {}: {error}",
                parsed.project_root.display()
            )
        })?;
    let mut receipt = accept(&project_root).await?;
    if let Some(host) = parsed.host_evidence {
        let host_receipt = crate::command::hook_host_acceptance::inspect_host_rollout(
            &host.rollout,
            &host.hook_events,
            &host.probe_path,
            &host.sentinel,
        )?;
        if !host_receipt.accepted() {
            return Err(format!(
                "normal-task Hook Host acceptance failed: reasonKind={}",
                host_receipt.reason_kind()
            ));
        }
        receipt.status = "ready";
        receipt.reason_kind = "none";
        receipt.host_acceptance = Some(host_receipt);
    }
    if parsed.json {
        println!(
            "{}",
            serde_json::to_string(&receipt)
                .map_err(|error| format!("encode Hook enablement receipt: {error}"))?
        );
    } else {
        println!(
            "[hook-enablement] status={} reasonKind={} activeDigest={} healthyDigest={} policyDecisionSamples={} policyDecisionMaxCpuMicros={} policyDecisionBudgetMicros={} launcherSamples={} launcherP99Micros={} launcherMaxMicros={} hostDeliveryAccepted={} pluginLauncher={}",
            receipt.status,
            receipt.reason_kind,
            receipt.active_digest,
            receipt.healthy_digest,
            receipt.policy_decision_samples,
            receipt.policy_decision_max_cpu_micros,
            receipt.policy_decision_budget_micros,
            receipt.launcher_samples,
            receipt.launcher_p99_micros,
            receipt.launcher_max_micros,
            receipt.host_acceptance.is_some(),
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
    let runtime_layout =
        agent_semantic_artifacts::StateHomeLayout::new(&runtime_state.protocol_home)
            .runtime_state();
    let public = which::which("asp").map_err(|error| {
        format!(
            "resolve PATH-visible ASP entry: reasonKind=path-visible-asp-unavailable error={error}"
        )
    })?;
    let public_hook = public
        .parent()
        .ok_or_else(|| {
            format!(
                "PATH-visible ASP entry has no parent: reasonKind=path-visible-asp-invalid path={}",
                public.display()
            )
        })?
        .join("asp-hook");
    let runtime_alias = runtime_layout.bin().join("asp");
    let runtime_hook_alias = runtime_layout.bin().join("asp-hook");
    let active_slot = runtime_layout.artifacts().active_slot().join("asp");
    let healthy_slot = runtime_layout.artifacts().healthy_slot().join("asp");
    let active_hook_slot = runtime_layout.artifacts().active_slot().join("asp-hook");
    let healthy_hook_slot = runtime_layout.artifacts().healthy_slot().join("asp-hook");
    require_symlink_target(&public, &active_slot, "public ASP alias").await?;
    require_symlink_target(&public_hook, &active_hook_slot, "public ASP Hook alias").await?;
    require_symlink_target(&runtime_alias, &public, "Runtime ASP compatibility alias").await?;
    require_symlink_target(
        &runtime_hook_alias,
        &public_hook,
        "Runtime ASP Hook compatibility alias",
    )
    .await?;
    let active = canonical_executable(&active_slot, "active ASP artifact").await?;
    let healthy = canonical_executable(&healthy_slot, "healthy ASP artifact").await?;
    let active_hook = canonical_executable(&active_hook_slot, "active ASP Hook artifact").await?;
    let healthy_hook =
        canonical_executable(&healthy_hook_slot, "healthy ASP Hook artifact").await?;
    let acceptance_binary = std::env::current_exe()
        .map_err(|error| format!("resolve running ASP executable: {error}"))?;
    let acceptance_binary =
        canonical_executable(&acceptance_binary, "running Hook acceptance ASP artifact").await?;
    if acceptance_binary != active {
        return Err(format!(
            "Hook enablement is not running from the active ASP artifact: reasonKind=acceptance-artifact-not-active active={} acceptance={}",
            active.display(),
            acceptance_binary.display()
        ));
    }
    let active_digest = bundle_digest(&active)?;
    let healthy_digest = bundle_digest(&healthy)?;
    if bundle_digest(&active_hook)? != active_digest
        || bundle_digest(&healthy_hook)? != healthy_digest
    {
        return Err(
            "ASP and ASP Hook artifacts do not share their active/healthy bundle identities: reasonKind=runtime-bundle-cohort-drift"
                .to_owned(),
        );
    }
    let mut protected = vec![active_digest.clone(), healthy_digest.clone()];
    protected.sort();
    protected.dedup();
    admit_retention_receipt(runtime_layout.root(), &protected).await?;

    let policy_decision_max_cpu_micros =
        policy_decision_acceptance(&active_hook, project_root, &runtime_state.protocol_home)
            .await?;
    let mut samples =
        launcher_scenario_samples(&launcher, &active_hook, LAUNCHER_SAMPLE_COUNT).await?;
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
        schema_version: "2",
        status: "probe-ready",
        reason_kind: "host-delivery-evidence-required",
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
        host_acceptance: None,
    })
}

fn parse_args(args: &[String]) -> Result<EnablementArgs, String> {
    let mut json = false;
    let mut project_root = None;
    let mut rollout = None;
    let mut hook_events = None;
    let mut probe_path = None;
    let mut sentinel = None;
    let mut index = 0;
    while index < args.len() {
        let argument = args[index].as_str();
        match argument {
            "--json" => json = true,
            "--help" | "-h" => return Err(usage()),
            "--host-rollout" | "--hook-events" | "--host-probe-path" | "--host-sentinel" => {
                let value = args.get(index + 1).ok_or_else(usage)?.clone();
                let target = match argument {
                    "--host-rollout" => &mut rollout,
                    "--hook-events" => &mut hook_events,
                    "--host-probe-path" => &mut probe_path,
                    "--host-sentinel" => &mut sentinel,
                    _ => unreachable!(),
                };
                if target.replace(value).is_some() {
                    return Err(format!(
                        "duplicate Hook enablement option {argument}: reasonKind=host-delivery-evidence-invalid\n{}",
                        usage()
                    ));
                }
                index += 1;
            }
            value if value.starts_with('-') => {
                return Err(format!(
                    "unknown Hook enablement option {value}\n{}",
                    usage()
                ));
            }
            value if project_root.is_none() => project_root = Some(PathBuf::from(value)),
            _ => return Err(usage()),
        }
        index += 1;
    }
    let host_argument_count = [
        rollout.is_some(),
        hook_events.is_some(),
        probe_path.is_some(),
        sentinel.is_some(),
    ]
    .into_iter()
    .filter(|present| *present)
    .count();
    let host_evidence = match host_argument_count {
        0 => None,
        4 => Some(HostEvidenceArgs {
            rollout: PathBuf::from(rollout.expect("complete Host evidence")),
            hook_events: PathBuf::from(hook_events.expect("complete Host evidence")),
            probe_path: probe_path.expect("complete Host evidence"),
            sentinel: sentinel.expect("complete Host evidence"),
        }),
        _ => {
            return Err(format!(
                "Hook enablement Host evidence must provide all four arguments: reasonKind=host-delivery-evidence-incomplete\n{}",
                usage()
            ));
        }
    };
    Ok(EnablementArgs {
        json,
        project_root: project_root.unwrap_or_else(|| PathBuf::from(".")),
        host_evidence,
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
    active_hook: &Path,
    project_root: &Path,
    state_home: &Path,
) -> Result<u64, String> {
    admit_linked_hook_identity(active_hook, project_root, state_home).await?;
    let config_path = agent_semantic_artifacts::StateHomeLayout::new(state_home)
        .control()
        .hook_client_config();
    let generation = agent_semantic_hook::aot_compiler::compile_serving_hook_policy_bundle_at(
        Some(&config_path),
    )?;
    let generation = std::str::from_utf8(&generation)
        .map_err(|error| format!("decode compiled Hook policy generation: {error}"))?;
    let payload = serde_json::json!({
        "session_id": "hook-enablement-policy-budget",
        "cwd": project_root.to_string_lossy(),
        "hook_event_name": "PreToolUse",
        "tool_name": "Bash",
        "tool_input": {"command": "cargo test -p agent-semantic-hook"}
    })
    .to_string();

    // Warm the exact immutable serving generation once. The following 16
    // samples measure only the borrowed matcher/evaluator kernel, with no Host
    // event-state writes or subprocess timing mixed into the policy budget.
    require_policy_decision(generation, &payload)?;
    let mut max_cpu_micros = 0;
    for sample in 0..POLICY_DECISION_SAMPLE_COUNT {
        let started = current_thread_cpu_nanos()?;
        require_policy_decision(generation, &payload)
            .map_err(|error| format!("policy decision sample {sample} failed: {error}"))?;
        let cpu = current_thread_cpu_nanos()?
            .saturating_sub(started)
            .div_ceil(1_000)
            .min(u128::from(u64::MAX)) as u64;
        if cpu >= POLICY_DECISION_BUDGET_MICROS {
            return Err(format!(
                "Hook policy decision exceeds enablement budget: reasonKind=policy-decision-budget-exceeded sample={sample} cpuMicros={cpu} budgetMicros={POLICY_DECISION_BUDGET_MICROS}"
            ));
        }
        max_cpu_micros = max_cpu_micros.max(cpu);
    }
    Ok(max_cpu_micros)
}

async fn admit_linked_hook_identity(
    active_hook: &Path,
    project_root: &Path,
    state_home: &Path,
) -> Result<(), String> {
    let output = run_process(
        active_hook,
        &["--identity"],
        project_root,
        state_home,
        Vec::new(),
    )
    .await
    .map_err(|error| format!("Hook Runtime identity acceptance failed: {error}"))?;
    let active_identity: agent_semantic_hook::HookRuntimeIdentityReceipt =
        serde_json::from_slice(&output).map_err(|error| {
        format!(
            "parse active Hook Runtime identity: reasonKind=hook-runtime-identity-invalid error={error} output={}",
            String::from_utf8_lossy(&output)
        )
    })?;
    active_identity.validate().map_err(|error| {
        format!("invalid active Hook Runtime identity: reasonKind=hook-runtime-identity-invalid error={error}")
    })?;
    let linked_identity = agent_semantic_hook::hook_runtime_identity_receipt()?;
    if active_identity.policy_content_digest != linked_identity.policy_content_digest {
        return Err(format!(
            "active and acceptance Hook policy identities differ: reasonKind=hook-runtime-identity-drift active={} acceptance={}",
            active_identity.policy_content_digest, linked_identity.policy_content_digest
        ));
    }
    Ok(())
}

fn require_policy_decision(generation: &str, payload: &str) -> Result<(), String> {
    let decision =
        agent_semantic_hook::aot_evaluator::evaluate_pre_tool(generation, payload, "Bash")?
            .ok_or_else(|| {
                "canonical Hook policy produced no decision: reasonKind=policy-decision-missing"
                    .to_owned()
            })?;
    if decision.probe_process_launched {
        return Err(
            "canonical Hook policy launched a probe: reasonKind=policy-decision-probe-launched"
                .to_owned(),
        );
    }
    Ok(())
}

#[cfg(unix)]
fn current_thread_cpu_nanos() -> Result<u128, String> {
    let mut time = libc::timespec {
        tv_sec: 0,
        tv_nsec: 0,
    };
    let status = unsafe { libc::clock_gettime(libc::CLOCK_THREAD_CPUTIME_ID, &mut time) };
    if status != 0 {
        return Err(format!(
            "read Hook enablement thread CPU clock: reasonKind=policy-decision-clock-unavailable error={}",
            std::io::Error::last_os_error()
        ));
    }
    Ok((time.tv_sec as u128) * 1_000_000_000 + time.tv_nsec as u128)
}

#[cfg(not(unix))]
fn current_thread_cpu_nanos() -> Result<u128, String> {
    static STARTED: std::sync::OnceLock<Instant> = std::sync::OnceLock::new();
    Ok(STARTED.get_or_init(Instant::now).elapsed().as_nanos())
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
        .join("asp-hook");
    tokio::fs::create_dir_all(public_path.parent().expect("public binary parent"))
        .await
        .map_err(|error| format!("create Hook acceptance scenario: {error}"))?;
    create_file_symlink(artifact, &public_path).await?;
    let asp_config = root.join("control/config/asp.toml");
    tokio::fs::create_dir_all(asp_config.parent().expect("ASP config parent"))
        .await
        .map_err(|error| format!("create Hook acceptance init config: {error}"))?;
    tokio::fs::write(&asp_config, b"[hook-engine]\nenabled = false\n")
        .await
        .map_err(|error| format!("write Hook acceptance init config: {error}"))?;
    let mut samples = Vec::with_capacity(count);
    let result = async {
        for _ in 0..count {
            let started = Instant::now();
            let output = run_process(
                launcher,
                &["pre-tool", "--client", "codex"],
                Path::new("/"),
                &root,
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
    stdin: Vec<u8>,
) -> Result<Vec<u8>, String> {
    let mut command = tokio::process::Command::new(program);
    command
        .args(args)
        .current_dir(cwd)
        .env("ASP_STATE_HOME", state_home)
        .kill_on_drop(true)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
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

fn bundle_digest(path: &Path) -> Result<String, String> {
    agent_semantic_artifacts::runtime_artifact_bundle_digest_from_member_path(path)
        .ok_or_else(|| {
            format!(
                "Runtime artifact is not bundle-digest-addressed: reasonKind=artifact-bundle-path-invalid path={}",
                path.display()
            )
        })
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
    "usage: asp hook enablement [PROJECT_ROOT] [--json] [--host-rollout PATH --hook-events PATH --host-probe-path PATH --host-sentinel TOKEN]".to_owned()
}

#[cfg(test)]
#[path = "../../tests/unit/command/hook_enablement_acceptance.rs"]
mod tests;
