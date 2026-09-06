//! Thin owner for Protocol binary installation.

use std::env;

// Binary installation is a private leaf of the provider-install branch.
use super::cli_support::has_help_flag;
use super::cli_support::usage;
use crate::command::agent_config_sync;
use crate::command::cli_help;
use crate::command::hook_runtime;
use crate::command::install_binary_config_admission;
use crate::command::protocol_binary;

pub(crate) async fn run_install_binary(args: &[String]) -> Result<(), String> {
    if !args.is_empty() {
        if has_help_flag(args) {
            println!("{}", usage());
            return Ok(());
        }
        return Err("asp install binary does not accept positional arguments or --target; the Runtime configuration owns its stable install slot".to_owned());
    }
    let project_root = env::current_dir()
        .map_err(|error| format!("failed to resolve current project root: {error}"))?;
    let runtime_state = agent_semantic_runtime::project_runtime_state(&project_root)?;
    install_binary_config_admission::admit_embedded_hook_config()?;
    let artifact_layout =
        agent_semantic_artifacts::RuntimeArtifactStateLayout::new(&runtime_state.protocol_home);
    let artifact_root = artifact_layout.root().to_path_buf();
    let plan = protocol_binary::ProtocolBinaryInstallPlan::capture(artifact_root)?;
    let hook_candidate =
        install_binary_config_admission::admit_embedded_hook_runtime_candidate(plan.current_exe())
            .await?;
    let hook_config_status = install_binary_config_admission::publish_embedded_hook_config(
        &runtime_state.protocol_home,
    )?;
    let installed = protocol_binary::ensure_protocol_binary_bundle_installed_transaction(
        &plan,
        &hook_candidate.source,
    )
    .await?;
    let install_registry_digest =
        crate::command::provider_install_registry::provider_install_registry_digest()?;
    let legacy_hook_generation =
        install_binary_config_admission::retire_legacy_hook_generation_pointer(
            &runtime_state.protocol_home,
        )?;
    let active_artifact_receipt = agent_semantic_hook::rebind_active_asp_binary_receipt_if_present(
        &installed.path,
        &installed.artifact_digest,
        &runtime_state.activation_path,
    )?;
    agent_config_sync::synchronize_embedded_agent_state_config(&runtime_state.protocol_home)?;
    let provider_artifacts =
        crate::command::installed_provider_artifacts::publish_current_installed_provider_artifacts(
            &runtime_state.protocol_home,
        )?;
    let state_resolution = agent_semantic_runtime::state_core::resolve_state_home_projection()?;
    if state_resolution.state_home != runtime_state.protocol_home {
        return Err(format!(
            "reasonKind=runtime-state-home-authority-drift resolved={} install={}",
            state_resolution.state_home.display(),
            runtime_state.protocol_home.display()
        ));
    }
    let pending_activation_path = agent_semantic_artifacts::runtime_artifact_activation::runtime_artifact_activation_event_path(
        &runtime_state.protocol_home,
    );
    let applied_activation_path = artifact_layout.applied_activation();
    let runtime_endpoint_path =
        agent_semantic_client_db::runtime_server_endpoint_path(&runtime_state.protocol_home)?;
    println!(
        "[asp-install-binary] binaryPath={} binaryInstall={} binaryContentDigest={} digestAlgorithm=blake3-256 binaryCurrent={} binarySwitch=atomic providerInstallRegistryDigest={} hookBinaryPath={} hookBinaryDigest={} hookBinarySwitch=active-healthy-bundle bundleLockAcquisitionCount={} hookConfigPublication={} hookConfigCoupling=embedded-in-hook-binary legacyHookGeneration={} hookAuthority=runtime-active-bundle-content-digest agentConfigPublication=current agentConfigCoupling=embedded-in-hook-binary runtimeServerLifecycle=resident-owner-independent reasonKind=none providerReconciliation=automatic installedProviderArtifactsGeneration={} installedProviderArtifactsWrite={} installedProviderArtifactsChangedLeaves={} developerIdentityReceipt={} installSource={} installScope=state-home projectRoot={} executablePath={} stateHome={} stateHomeSource={:?} aspStateHomePresent={} homePresent={} pendingActivationPath={} appliedActivationPath={} runtimeEndpointPath={}",
        installed.path.display(),
        installed.status,
        installed.artifact_digest,
        installed.path.display(),
        install_registry_digest,
        agent_semantic_artifacts::RuntimeArtifactStateLayout::new(&runtime_state.protocol_home)
            .active_slot()
            .join("asp-hook")
            .display(),
        hook_candidate.artifact_digest,
        installed.lock_acquisition_count,
        hook_config_status,
        legacy_hook_generation,
        provider_artifacts.generation(),
        provider_artifacts.artifact_write(),
        provider_artifacts.changed_leaf_count(),
        active_artifact_receipt.as_str(),
        plan.install_source_kind(),
        project_root.display(),
        plan.current_exe().display(),
        state_resolution.state_home.display(),
        state_resolution.source,
        state_resolution.asp_state_home_present,
        state_resolution.home_present,
        pending_activation_path.display(),
        applied_activation_path.display(),
        runtime_endpoint_path.display(),
    );
    Ok(())
}

pub(crate) async fn run_install_hook(args: &[String]) -> Result<(), String> {
    if args.is_empty() || has_help_flag(args) {
        println!("{}", super::cli_support::install_hook_usage());
        return Ok(());
    }
    if args.iter().any(|arg| arg == "--codex") {
        return Err(
            "Codex plugin publication uses `asp install plugin <status|publish> --codex [PROJECT_ROOT]`".to_owned(),
        );
    }
    let mut forwarded = vec!["install".to_owned()];
    forwarded.extend(args.iter().cloned());
    hook_runtime::run_hook_runtime_args(forwarded).await
}

pub(crate) async fn run_install_plugin(args: &[String]) -> Result<(), String> {
    if args.is_empty() || has_help_flag(args) {
        return cli_help::print_install_plugin_help(args);
    }
    hook_runtime::run_codex_plugin_install_args(args).await
}
