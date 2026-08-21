//! Thin owner for protocol binary installation and Runtime bootstrap handoff.

use std::env;

use super::install_provider_cli_support::{has_help_flag, usage};
use crate::command::{cli_help, hook_runtime, install_binary_config_admission, protocol_binary};

pub(crate) async fn run_install_binary(args: &[String]) -> Result<(), String> {
    if !args.is_empty() {
        if has_help_flag(args) {
            println!("{}", usage());
            return Ok(());
        }
        return Err("asp install binary does not accept positional arguments or --target; the Runtime configuration owns its stable install slot".to_owned());
    }
    let project_root = env::current_dir().map_err(|error| format!("failed to resolve current project root: {error}"))?;
    let runtime_state = agent_semantic_runtime::project_runtime_state(&project_root)?;
    install_binary_config_admission::admit_embedded_hook_config()?;
    let artifact_root = runtime_state.protocol_home.join("runtime/artifacts");
    let plan = protocol_binary::ProtocolBinaryInstallPlan::capture(artifact_root.clone())?;
    let reconciliation_guard = protocol_binary::ProtocolBinaryReconciliationGuard::acquire(&runtime_state.protocol_home)?;
    let installed = protocol_binary::ensure_protocol_binary_installed(&plan).await?;
    let hook_config_publication = install_binary_config_admission::publish_embedded_hook_config_for_project(&runtime_state.protocol_home, &project_root)?;
    let active_artifact_receipt = agent_semantic_hook::rebind_active_asp_binary_receipt_if_present(&installed.path, &installed.artifact_digest, &runtime_state.activation_path)?;
    drop(reconciliation_guard);
    let install_source = plan.install_source_kind();
    let monitor_healthy = || -> bool {
        let path = runtime_state.protocol_home.join("runtime/server/monitor-state.json");
        let Ok(bytes) = std::fs::read(path) else { return false; };
        let Ok(value) = serde_json::from_slice::<serde_json::Value>(&bytes) else { return false; };
        let phase = value.get("phase").and_then(|value| value.as_str()).unwrap_or("");
        let heartbeat = value.get("heartbeat").and_then(|value| value.as_bool()).unwrap_or(false);
        let updated = value.get("updatedAtMillis").and_then(|value| value.as_u64()).unwrap_or(0);
        let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|value| value.as_millis() as u64).unwrap_or(0);
        heartbeat && matches!(phase, "watching" | "successor-started") && now.saturating_sub(updated) <= 1_000
    };
    let monitor_stalled = agent_semantic_client_db::read_runtime_server_endpoint(&runtime_state.protocol_home)
        .ok().flatten().is_some_and(|endpoint| endpoint.monitor_capability) && !monitor_healthy();
    let runtime_server_reconcile = if installed.status == "current" {
        "not-required"
    } else if agent_semantic_client_db::read_runtime_server_endpoint(&runtime_state.protocol_home).ok().flatten().is_some_and(|endpoint| endpoint.monitor_capability) && monitor_healthy() {
        "monitor-observed"
    } else {
        let reconciler = std::env::current_exe().map_err(|error| format!("resolve ASP Runtime Server daemon: {error}"))?;
        let mut command = tokio::process::Command::new(reconciler);
        command.args(["server", "daemon"]).env("ASP_STATE_HOME", &runtime_state.protocol_home).stdin(std::process::Stdio::null()).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null());
        #[cfg(unix)] { command.process_group(0); }
        let child = command.spawn().map_err(|error| format!("Runtime Server bootstrap handoff failed: {error}"))?;
        let process_id = child.id().unwrap_or(0);
        let nonce = format!("{}-{}", process_id, std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_err(|e| e.to_string())?.as_nanos());
        let candidate_dir = runtime_state.protocol_home.join("runtime/server/candidates");
        std::fs::create_dir_all(&candidate_dir).map_err(|e| format!("create candidate receipt dir: {e}"))?;
        let candidate = candidate_dir.join(format!("{nonce}.json"));
        let started_at = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_err(|e| e.to_string())?.as_millis();
        let identity_receipt = agent_semantic_runtime::runtime_artifact_identity::read_runtime_artifact_identity(&runtime_state.protocol_home, "asp").await?;
        let identity = identity_receipt.identity();
        let receipt = serde_json::json!({"schemaVersion":"1","state":"accepted","nonce":nonce,"processId":process_id,"startedAt":started_at,"stateHome":runtime_state.protocol_home,"desiredIdentity":identity.value(),"desiredIdentityKind":identity.kind(),"desiredIdentityAlgorithm":identity.algorithm(),"executablePath":std::env::current_exe().map_err(|e| e.to_string())?});
        let tmp = candidate.with_extension("json.tmp");
        std::fs::write(&tmp, serde_json::to_vec(&receipt).map_err(|e| e.to_string())?).map_err(|e| format!("write candidate receipt: {e}"))?;
        std::fs::rename(tmp, candidate).map_err(|e| format!("publish candidate receipt: {e}"))?;
        "bootstrap-accepted"
    };
    println!("[asp-install-binary] binaryPath={} binaryInstall={} binarySourceGeneration={} generationAlgorithm=blake3-metadata-v1 binaryCurrent={} binarySwitch=atomic hookConfigPublication={} hookConfigCoupling=binary-generation runtimeServerReconcile={} reasonKind={} providerReconciliation=not-on-binary-install globalProviderCatalog=not-on-binary-install developerIdentityReceipt={} installSource={}", installed.path.display(), installed.status, installed.artifact_digest, installed.path.display(), hook_config_publication, runtime_server_reconcile, if monitor_stalled { "monitor-stalled" } else { "none" }, active_artifact_receipt.as_str(), install_source);
    Ok(())
}

pub(crate) async fn run_install_hook(args: &[String]) -> Result<(), String> {
    if args.is_empty() || has_help_flag(args) { println!("{}", super::install_provider_cli_support::install_hook_usage()); return Ok(()); }
    if args.iter().any(|arg| arg == "--codex") { return Err("Codex plugin installation uses `asp install plugin --codex [PROJECT_ROOT]`".to_string()); }
    let mut forwarded = vec!["install".to_string()]; forwarded.extend(args.iter().cloned()); hook_runtime::run_hook_runtime_args(forwarded).await
}

pub(crate) async fn run_install_plugin(args: &[String]) -> Result<(), String> {
    if args.is_empty() || has_help_flag(args) { return cli_help::print_install_plugin_help(); }
    hook_runtime::run_codex_plugin_install_args(args).await
}
