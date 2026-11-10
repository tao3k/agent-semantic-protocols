// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use std::path::Path;

use agent_semantic_runtime::project_runtime_state;

// Workspace publication receipts remain private to the provider-install branch.
use super::workspace::BuiltProviderWorkspace;

#[allow(clippy::too_many_arguments)]
pub(in super::super) async fn record_registered_provider_workspace_install(
    language_id: &str,
    provider_id: &str,
    registered_binary: &str,
    target: &str,
    invocation_root: &Path,
    configured_dev_root: &Path,
    registration: &super::super::provider_install_registry::ProviderInstallRegistration,
    built: BuiltProviderWorkspace,
) -> Result<(), String> {
    let dev_root = configured_dev_root.canonicalize().map_err(|error| {
        format!(
            "failed to canonicalize configured [dev].root {}: {error}",
            configured_dev_root.display()
        )
    })?;
    validate_registration_identity(language_id, provider_id, registered_binary, registration)?;
    let provider_source_root = dev_root
        .join(&registration.source_root)
        .canonicalize()
        .map_err(|error| format!("failed to canonicalize provider sourceRoot: {error}"))?;
    let runtime_state = project_runtime_state(invocation_root)?;
    let provider_binary = super::archive::binary_file_name(registered_binary, target);
    let install_target =
        super::target::resolve_provider_binary_install_target(language_id, &provider_binary)?;
    let stable_entry = install_target.path.clone();
    let published = super::workspace::publish_provider_workspace(
        &runtime_state.protocol_home,
        &stable_entry,
        registration,
        built,
    )
    .await?;
    let installed_entrypoint_digest =
        agent_semantic_content_identity::file_content_digest_v1(&published.installed_path)?;
    let installed_entrypoint_metadata_digest =
        agent_semantic_content_identity::file_artifact_metadata_digest_v1(
            &published.installed_path,
        )?;
    let execution_command_digest =
        agent_semantic_content_identity::provider_execution_command_digest(
            &[published.installed_path.to_string_lossy().to_string()],
            &installed_entrypoint_digest,
        )?;
    let provenance = super::development::capture_development_artifact_provenance(
        &dev_root,
        registration,
        target,
    )?;
    let installed_sha256 = super::archive::sha256_file(&published.installed_path)?;
    let artifact_entrypoint_sha256 = super::archive::sha256_file(&published.artifact_entrypoint)?;
    let launcher_digest =
        agent_semantic_content_identity::file_content_digest_v1(&published.launcher)?;
    let scope = "state-home";
    let lock_path = super::core::canonical_provider_state_root()?
        .join("receipts")
        .join(format!("{language_id}.lock.toml"));
    super::core::write_provider_lock(
        &lock_path,
        &super::core::ProviderInstallLock {
            schema_id: "asp.provider-install-lock.v1",
            scope,
            language_id,
            provider_id,
            source_kind: "develop-workspace-tree",
            checkout_root: Some(&dev_root),
            provider_source_root: Some(&provider_source_root),
            repo: None,
            rev: None,
            target,
            binary: registered_binary,
            installed_path: &published.installed_path,
            package_path: &published.artifact_root,
            sha256: &installed_sha256,
            source: published.source_root.display().to_string(),
            source_snapshot_root: Some(&provenance.source_snapshot_root),
            source_snapshot_algorithm: Some("blake3-merkle-v1"),
            source_leaf_count: Some(provenance.source_leaf_count),
            provider_digest: Some(&provenance.provider_digest),
            build_recipe_digest: Some(&provenance.build_recipe_digest),
            artifact_digest: Some(&published.artifact_digest),
            artifact_leaf_count: Some(published.artifact_leaf_count),
            artifact_entrypoint: Some(&published.artifact_entrypoint),
            artifact_entrypoint_sha256: Some(&artifact_entrypoint_sha256),
            installed_entrypoint_digest: Some(&installed_entrypoint_digest),
            installed_entrypoint_metadata_digest: &installed_entrypoint_metadata_digest,
            execution_command_digest: &execution_command_digest,
            launcher_digest: Some(&launcher_digest),
        },
    )?;
    println!(
        "[asp-install] provider={} language={} scope={} installMode=develop-workspace-tree sourceKind=develop-workspace-tree devRoot={} target={} binary={} binaryContentDigest={} digestAlgorithm=blake3-256 artifactLeafCount={} artifactEntrypoint={} installedPath={} lock={} switch=atomic activeRuntimeBundleDigest={}",
        provider_id,
        language_id,
        scope,
        dev_root.display(),
        target,
        registered_binary,
        published.artifact_digest,
        published.artifact_leaf_count,
        published.artifact_entrypoint.display(),
        published.installed_path.display(),
        lock_path.display(),
        published.runtime_bundle_digest,
    );
    Ok(())
}

fn validate_registration_identity(
    language_id: &str,
    provider_id: &str,
    registered_binary: &str,
    registration: &super::super::provider_install_registry::ProviderInstallRegistration,
) -> Result<(), String> {
    if registration.provider_id == provider_id
        && registration.binary == registered_binary
        && registration.language_id == language_id
    {
        return Ok(());
    }
    Err(format!(
        "ProviderRegistry workspace-install identity drift: language={} provider={} binary={} expectedLanguage={language_id} expectedProvider={provider_id} expectedBinary={registered_binary}",
        registration.language_id, registration.provider_id, registration.binary
    ))
}
