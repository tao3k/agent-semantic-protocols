//! Reconcile registered provider binaries and their immutable install receipts.

use std::path::Path;

fn registered_provider_binary_exists(path: &Path) -> Result<bool, String> {
    match std::fs::metadata(path) {
        Ok(metadata) => Ok(metadata.is_file()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(format!(
            "failed to inspect registered provider binary {}: {error}",
            path.display()
        )),
    }
}

fn registered_provider_binary_is_canonical_lattice_entry(
    path: &Path,
    artifact_root: &Path,
) -> Result<bool, String> {
    match std::fs::symlink_metadata(path) {
        Ok(metadata) if !metadata.file_type().is_symlink() => Ok(false),
        Ok(_) => {
            let canonical = std::fs::canonicalize(path).map_err(|error| {
                format!(
                    "failed to resolve registered provider Lattice entry {}: {error}",
                    path.display()
                )
            })?;
            let canonical_artifact_root =
                std::fs::canonicalize(artifact_root).map_err(|error| {
                    format!(
                        "failed to resolve provider artifact root {}: {error}",
                        artifact_root.display()
                    )
                })?;
            Ok(canonical.file_name() == path.file_name()
                && canonical.starts_with(canonical_artifact_root.join("blake3-256"))
                && super::protocol_binary::protocol_binary_digest_from_canonical_artifact_path(
                    &canonical,
                )
                .is_some())
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(format!(
            "failed to inspect registered provider Lattice profile {}: {error}",
            path.display()
        )),
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct RegisteredProviderBinaryReconciliation {
    pub(super) registration_count: usize,
    pub(super) binary_identity_count: usize,
    pub(super) reconciled_count: usize,
    pub(super) changed_count: usize,
    pub(super) missing_count: usize,
    pub(super) receipt_reconciled_count: usize,
    pub(super) receipt_changed_count: usize,
    pub(super) receipt_missing_count: usize,
    pub(super) provider_receipts: Vec<agent_semantic_runtime::ProviderInstallReceipt>,
    pub(super) binary_byte_reads: usize,
}

pub(super) async fn reconcile_registered_provider_runtime_binaries(
    runtime_bin_dir: &Path,
    artifact_root: &Path,
    provider_lock_dir: &Path,
) -> Result<RegisteredProviderBinaryReconciliation, String> {
    let registrations = agent_semantic_hook::registered_provider_binaries_v1();
    reconcile_registered_provider_runtime_binaries_from(
        &registrations,
        runtime_bin_dir,
        artifact_root,
        provider_lock_dir,
    )
    .await
}

/// Reconciles the installed provider lattice and republishes its catalog before
/// the platform supervisor starts a new Runtime Server generation.
///
/// The daemon remains a read-only, fail-closed catalog consumer. Repair lives
/// in the explicit supervisor control path so a stale catalog cannot turn the
/// platform supervisor into an endless restart loop.
pub(crate) async fn reconcile_global_provider_catalog_for_runtime(
    state_home: &Path,
) -> Result<super::global_provider_catalog::GlobalProviderCatalogReadiness, String> {
    let runtime_root = state_home.join("runtime");
    let reconciliation = reconcile_registered_provider_runtime_binaries(
        &runtime_root.join("bin"),
        &runtime_root.join("artifacts"),
        &agent_semantic_runtime::provider_receipt_dir(state_home),
    )
    .await?;
    publish_registered_provider_runtime_reconciliation(state_home, &reconciliation).await
}

pub(crate) async fn publish_registered_provider_runtime_reconciliation(
    state_home: &Path,
    reconciliation: &RegisteredProviderBinaryReconciliation,
) -> Result<super::global_provider_catalog::GlobalProviderCatalogReadiness, String> {
    super::global_provider_catalog::publish_global_provider_catalog(
        state_home,
        &reconciliation.provider_receipts,
    )?;
    super::global_provider_catalog::read_global_provider_catalog_readiness(state_home)
}

pub async fn prepare_runtime_server_provider_catalog(state_home: &Path) -> Result<(), String> {
    reconcile_global_provider_catalog_for_runtime(state_home)
        .await
        .map(|_| ())
}

async fn reconcile_registered_provider_runtime_binaries_from(
    registrations: &[agent_semantic_hook::RegisteredProviderBinaryV1],
    runtime_bin_dir: &Path,
    artifact_root: &Path,
    provider_lock_dir: &Path,
) -> Result<RegisteredProviderBinaryReconciliation, String> {
    let binary_names = registrations
        .iter()
        .map(|registration| registration.binary().to_string())
        .collect::<std::collections::BTreeSet<_>>();
    prune_stale_registered_provider_leaves(registrations, runtime_bin_dir, provider_lock_dir)
        .await?;
    let state_home = artifact_root
        .parent()
        .and_then(Path::parent)
        .ok_or_else(|| {
            format!(
                "runtime artifact root does not belong to a State Home: {}",
                artifact_root.display()
            )
        })?;
    let developer_root =
        agent_semantic_runtime::runtime_artifact_catalog::load_runtime_developer_root(state_home)?;
    let mut reconciled_count = 0;
    let mut changed_count = 0;
    let mut missing_count = 0;
    let mut binary_byte_reads = 0;
    for binary_name in &binary_names {
        let target = runtime_bin_dir.join(binary_name);
        if let Some(developer_root) = developer_root.as_deref() {
            let mut verified_source = None;
            let mut repair_required = None;
            let mut generation_is_stale = false;
            let target_is_developer_owned = tokio::fs::canonicalize(&target)
                .await
                .is_ok_and(|target| target.starts_with(developer_root));
            for registered_binary in registrations
                .iter()
                .filter(|registration| registration.binary() == binary_name)
            {
                // Developer authority is resolved from the current Schema
                // Register/workspace descriptor before inspecting any
                // persisted target or receipt. Persisted binaries are never
                // candidates for developer publication.
                generation_is_stale = true;
                let registration = agent_semantic_hook::registered_provider_development_v1(
                    registered_binary.language_id().as_str(),
                )?;
                let Some(workspace_install) = registration.development.workspace_install.as_deref()
                else {
                    continue;
                };
                match agent_semantic_runtime::provider_workspace_artifact::resolve_verified_provider_workspace_artifact(
                    developer_root,
                    &registration.development.source_root,
                    workspace_install,
                    registration.language_id.as_str(),
                    registration.provider_id.as_str(),
                    &registration.binary,
                )
                .await
                {
                    Ok(artifact) => {
                        verified_source = Some(artifact.entrypoint().to_path_buf());
                        break;
                    }
                    Err(
                        agent_semantic_runtime::provider_workspace_artifact::ProviderWorkspaceArtifactError::RepairRequired(
                            message,
                        ),
                    ) => repair_required = Some(message),
                    Err(error) => return Err(error.to_string()),
                }
            }

            if !generation_is_stale {
                reconciled_count += 1;
                continue;
            }

            if let Some(source) = verified_source {
                let target_matches_source = tokio::fs::canonicalize(&target)
                    .await
                    .ok()
                    .zip(tokio::fs::canonicalize(&source).await.ok())
                    .is_some_and(|(target, source)| target == source);
                if target_matches_source {
                    reconciled_count += 1;
                    continue;
                }
                let binary_identity =
                    super::protocol_binary::RuntimeBinaryIdentityV1::from_registered_provider(
                        binary_name,
                    )?;
                super::protocol_binary::install_protocol_binary_target(
                    &source,
                    &target,
                    artifact_root,
                    &binary_identity,
                )
                .await?;
                changed_count += 1;
                reconciled_count += 1;
                continue;
            }

            if target_is_developer_owned {
                reconciled_count += 1;
                continue;
            }
            if repair_required.is_some() {
                missing_count += 1;
                continue;
            }
            missing_count += 1;
            continue;
        }
        if !registered_provider_binary_exists(&target)? {
            missing_count += 1;
            continue;
        }
        // Managed releases may only be migrated from the immutable artifact
        // lattice.  Never treat an existing registered symlink outside the
        // State Home as an install source; that would let an unmanaged path
        // cross the runtime trust boundary.
        if registered_provider_binary_is_canonical_lattice_entry(&target, artifact_root)? {
            reconciled_count += 1;
            continue;
        }
        let canonical_artifact_root = artifact_root.canonicalize().map_err(|error| {
            format!(
                "failed to canonicalize immutable artifact root {}: {error}",
                artifact_root.display()
            )
        })?;
        let canonical_target = target.canonicalize().map_err(|error| {
            format!(
                "escapes immutable artifact root: {} ({error})",
                target.display()
            )
        })?;
        if !canonical_target.starts_with(&canonical_artifact_root) {
            return Err(format!(
                "escapes immutable artifact root: {}",
                canonical_target.display()
            ));
        }
        let binary_identity =
            super::protocol_binary::RuntimeBinaryIdentityV1::from_registered_provider(binary_name)?;
        super::protocol_binary::install_protocol_binary_target(
            &target,
            &target,
            artifact_root,
            &binary_identity,
        )
        .await?;
        binary_byte_reads += 1;
        changed_count += 1;
        reconciled_count += 1;
    }
    if developer_root.is_some() {
        agent_semantic_runtime::developer_artifact_cleanup::remove_developer_artifact_lattice(
            runtime_bin_dir,
            artifact_root,
        )
        .await?;
    }
    // Capture receipt currency only after the atomic publication and Developer
    // lattice cleanup so stale artifact-backed receipts cannot enter the catalog.
    let current_receipts = registrations
        .iter()
        .filter_map(|registration| {
            let receipt = super::install_provider_reconcile::read_provider_install_receipt(
                registration.language_id().as_str(),
                provider_lock_dir,
            )
            .ok()?;
            let binary_path = runtime_bin_dir.join(registration.binary());
            if receipt.provider_id != registration.provider_id().as_str() {
                return None;
            }
            super::install_provider_reconcile::provider_install_receipt_matches_artifact(
                &receipt,
                &binary_path,
            )
            .ok()
            .filter(|current| *current)
            .map(|_| receipt)
        })
        .collect::<Vec<_>>();
    let receipt_registrations = registrations.to_vec();
    let receipt_runtime_bin_dir = runtime_bin_dir.to_path_buf();
    let receipt_provider_lock_dir = provider_lock_dir.to_path_buf();
    let receipt_current = current_receipts.clone();
    let receipt_reconciliation = tokio::task::spawn_blocking(move || {
        let mut reconciled_count = 0;
        let mut changed_count = 0;
        let mut missing_count = 0;
        let mut binary_reads = 0;
        let mut receipts = Vec::new();
        for registration in &receipt_registrations {
            let binary_path = receipt_runtime_bin_dir.join(registration.binary());
            if !registered_provider_binary_exists(&binary_path)? {
                continue;
            }
            let lock_path = receipt_provider_lock_dir
                .join(format!("{}.lock.toml", registration.language_id().as_str()));
            match std::fs::symlink_metadata(&lock_path) {
                Ok(_) => {}
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                    if registered_provider_receipt_covers_binary(
                        &receipt_current,
                        registration.binary(),
                    ) {
                        continue;
                    }
                    missing_count += 1;
                    continue;
                }
                Err(error) => {
                    return Err(format!(
                        "failed to inspect provider install receipt {}: {error}",
                        lock_path.display()
                    ));
                }
            }
            if let Some(receipt) = receipt_current
                .iter()
                .find(|receipt| receipt.language_id == registration.language_id().as_str())
            {
                reconciled_count += 1;
                receipts.push(receipt.clone());
                continue;
            }
            let changed =
                super::install_provider_reconcile::reconcile_provider_install_receipt_in_lock_dir(
                    registration.language_id().as_str(),
                    &receipt_provider_lock_dir,
                    false,
                )?;
            binary_reads += 1;
            reconciled_count += 1;
            if changed {
                changed_count += 1;
            }
            receipts.push(
                super::install_provider_reconcile::read_provider_install_receipt(
                    registration.language_id().as_str(),
                    &receipt_provider_lock_dir,
                )?,
            );
        }
        Ok::<_, String>((
            reconciled_count,
            changed_count,
            missing_count,
            binary_reads,
            receipts,
        ))
    })
    .await
    .map_err(|error| format!("join provider receipt reconciliation task: {error}"))??;
    let (
        receipt_reconciled_count,
        receipt_changed_count,
        receipt_missing_count,
        receipt_binary_byte_reads,
        provider_receipts,
    ) = receipt_reconciliation;
    binary_byte_reads += receipt_binary_byte_reads;
    Ok(RegisteredProviderBinaryReconciliation {
        registration_count: registrations.len(),
        binary_identity_count: binary_names.len(),
        reconciled_count,
        changed_count,
        missing_count,
        receipt_reconciled_count,
        receipt_changed_count,
        receipt_missing_count,
        provider_receipts,
        binary_byte_reads,
    })
}

/// Remove only persisted provider leaves whose identity is absent from the
/// current Schema Register.  We inspect directory entries, never symlink
/// targets, so a stale external link cannot cause deletion outside State Home.
async fn prune_stale_registered_provider_leaves(
    registrations: &[agent_semantic_hook::RegisteredProviderBinaryV1],
    runtime_bin_dir: &Path,
    provider_lock_dir: &Path,
) -> Result<(), String> {
    let desired_languages = registrations
        .iter()
        .map(|registration| registration.language_id().as_str().to_owned())
        .collect::<std::collections::BTreeSet<_>>();
    let mut receipts = tokio::fs::read_dir(provider_lock_dir)
        .await
        .map_err(|error| format!("read provider receipt directory: {error}"))?;
    while let Some(entry) = receipts
        .next_entry()
        .await
        .map_err(|error| format!("read provider receipt entry: {error}"))?
    {
        let name = entry.file_name().to_string_lossy().into_owned();
        let Some(language) = name.strip_suffix(".lock.toml") else {
            continue;
        };
        if !desired_languages.contains(language) {
            if let Ok(receipt) = super::install_provider_reconcile::read_provider_install_receipt(
                language,
                provider_lock_dir,
            ) {
                if let Some(binary) = receipt.installed_path.file_name() {
                    let stale_leaf = runtime_bin_dir.join(binary);
                    let _ = tokio::fs::remove_file(stale_leaf).await;
                }
            }
            tokio::fs::remove_file(entry.path())
                .await
                .map_err(|error| {
                    format!(
                        "prune stale provider receipt {}: {error}",
                        entry.path().display()
                    )
                })?;
        }
    }
    Ok(())
}

pub(super) fn registered_provider_receipt_covers_binary(
    receipts: &[agent_semantic_runtime::ProviderInstallReceipt],
    binary_name: &str,
) -> bool {
    receipts.iter().any(|receipt| {
        receipt
            .installed_path
            .file_name()
            .and_then(|name| name.to_str())
            == Some(binary_name)
    })
}

#[cfg(test)]
#[path = "../../tests/unit/install_provider_runtime_reconcile.rs"]
mod install_provider_runtime_reconcile_tests;
