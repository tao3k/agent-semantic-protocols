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
    pub(super) provider_receipts: Vec<super::install_provider_reconcile::ProviderInstallReceipt>,
    pub(super) binary_byte_reads: usize,
}

pub(super) fn reconcile_registered_provider_runtime_binaries(
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
}

fn reconcile_registered_provider_runtime_binaries_from(
    registrations: &[agent_semantic_hook::RegisteredProviderBinaryV1],
    runtime_bin_dir: &Path,
    artifact_root: &Path,
    provider_lock_dir: &Path,
) -> Result<RegisteredProviderBinaryReconciliation, String> {
    let binary_names = registrations
        .iter()
        .map(|registration| registration.binary().to_string())
        .collect::<std::collections::BTreeSet<_>>();
    let mut reconciled_count = 0;
    let changed_count = 0;
    let mut missing_count = 0;
    let mut receipt_reconciled_count = 0;
    let mut receipt_changed_count = 0;
    let mut receipt_missing_count = 0;
    let mut provider_receipts = Vec::new();
    let mut binary_byte_reads = 0;
    let current_receipts = registrations
        .iter()
        .filter_map(|registration| {
            let receipt = super::install_provider_reconcile::read_provider_install_receipt(
                registration.language_id().as_str(),
                provider_lock_dir,
            )
            .ok()?;
            let binary_path = runtime_bin_dir.join(registration.binary());
            super::install_provider_reconcile::provider_install_receipt_matches_artifact(
                &receipt,
                &binary_path,
            )
            .ok()
            .filter(|current| *current)
            .map(|_| receipt)
        })
        .collect::<Vec<_>>();
    for binary_name in &binary_names {
        let target = runtime_bin_dir.join(binary_name);
        if !registered_provider_binary_exists(&target)? {
            missing_count += 1;
            continue;
        }
        if registered_provider_binary_is_canonical_lattice_entry(&target, artifact_root)? {
            reconciled_count += 1;
            continue;
        }
        return Err(format!(
            "registered provider runtime entry {} is not a canonical digest-lattice symlink",
            target.display()
        ));
    }
    for registration in registrations {
        let binary_path = runtime_bin_dir.join(registration.binary());
        if !registered_provider_binary_exists(&binary_path)? {
            continue;
        }
        let lock_path =
            provider_lock_dir.join(format!("{}.lock.toml", registration.language_id().as_str()));
        match std::fs::symlink_metadata(&lock_path) {
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                if registered_provider_receipt_covers_binary(
                    &current_receipts,
                    registration.binary(),
                ) {
                    continue;
                }
                receipt_missing_count += 1;
                continue;
            }
            Err(error) => {
                return Err(format!(
                    "failed to inspect provider install receipt {}: {error}",
                    lock_path.display()
                ));
            }
        }
        if let Some(receipt) = current_receipts
            .iter()
            .find(|receipt| receipt.language_id == registration.language_id().as_str())
        {
            receipt_reconciled_count += 1;
            provider_receipts.push(receipt.clone());
            continue;
        }
        let changed =
            super::install_provider_reconcile::reconcile_provider_install_receipt_in_lock_dir(
                registration.language_id().as_str(),
                provider_lock_dir,
                false,
            )?;
        binary_byte_reads += 1;
        receipt_reconciled_count += 1;
        if changed {
            receipt_changed_count += 1;
        }
        provider_receipts.push(
            super::install_provider_reconcile::read_provider_install_receipt(
                registration.language_id().as_str(),
                provider_lock_dir,
            )?,
        );
    }
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

pub(super) fn registered_provider_receipt_covers_binary(
    receipts: &[super::install_provider_reconcile::ProviderInstallReceipt],
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
