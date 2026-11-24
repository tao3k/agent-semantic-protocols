// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Thin owner for Protocol binary installation.

use std::env;

use super::cli_support::has_help_flag;
use super::cli_support::usage;
#[cfg(feature = "mrr-data-search-composition")]
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
    let artifact_layout =
        agent_semantic_artifacts::RuntimeArtifactStateLayout::new(&runtime_state.protocol_home);
    let artifact_root = artifact_layout.root().to_path_buf();
    let plan = protocol_binary::ProtocolBinaryInstallPlan::capture(artifact_root)?;
    let mut provider_reconciliation =
        super::core::prepare_active_provider_reconciliation(&runtime_state.protocol_home)?;
    let execution_binding = provider_reconciliation
        .bind_runtime_execution_closure(plan.candidate_source())
        .await?;
    let mut members = Vec::new();
    #[cfg(feature = "mrr-data-search-composition")]
    let search_owner = install_binary_config_admission::admit_embedded_mrr_runtime_candidate(
        plan.candidate_source(),
    )
    .await?;
    #[cfg(feature = "mrr-data-search-composition")]
    members.push(
        agent_semantic_artifacts::runtime_artifact_publication::RuntimeArtifactBundleMemberSource {
            name: "mrr-search",
            source: &search_owner,
        },
    );
    members.extend(provider_reconciliation.member_sources());
    let installed =
        protocol_binary::ensure_protocol_binary_bound_bundle_members_installed_transaction(
            &plan,
            &members,
            &execution_binding,
            provider_reconciliation.predecessor_bundle(),
        )
        .await?;
    let install_registry_digest =
        crate::command::provider_install_registry::provider_install_registry_digest()?;
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
        "[asp-install-binary] binaryPath={} binaryInstall={} binaryContentDigest={} digestAlgorithm=blake3-256 binaryCurrent={} binarySwitch=atomic providerInstallRegistryDigest={} bundleLockAcquisitionCount={} runtimeServerLifecycle=resident-owner-independent reasonKind=none providerReconciliation=configured-artifact-authority providerReconciledCount={} activeRuntimeBundleDigest={} installSource={} installScope=state-home projectRoot={} executablePath={} stateHome={} stateHomeSource={:?} aspStateHomePresent={} homePresent={} pendingActivationPath={} appliedActivationPath={} runtimeEndpointPath={}",
        installed.path.display(),
        installed.status,
        installed.artifact_digest,
        installed.path.display(),
        install_registry_digest,
        installed.lock_acquisition_count,
        provider_reconciliation.provider_count(),
        installed.bundle_digest.as_deref().unwrap_or("unavailable"),
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
