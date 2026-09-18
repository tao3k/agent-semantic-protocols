// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Atomic bundle publication separated from registry and closure resolution.

use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::fs;
use std::io::Write;
use std::path::Path;
use std::path::PathBuf;

use crate::receipt::BUNDLE_MEMBERSHIP_FILE;
use crate::receipt::SchemaBundleMembership;
use crate::receipt::read_receipt_if_present;
use crate::registry::BUNDLE_RECEIPT_FILE;
use crate::registry::LanguageSchemaBundleReceipt;
use crate::registry::LanguageSchemaProfile;
use crate::registry::SchemaBundleReport;

pub(super) fn write_package_projection(
    profile: &LanguageSchemaProfile,
    receipt: &LanguageSchemaBundleReceipt,
    documents: &BTreeMap<String, Vec<u8>>,
    schema_root: &Path,
) -> Result<SchemaBundleReport, String> {
    let receipt_path = schema_root.join(BUNDLE_RECEIPT_FILE);
    let previous = read_receipt_if_present(&receipt_path).unwrap_or_default();
    let protected_names = profile
        .provider_owned
        .iter()
        .chain(&profile.bootstrap)
        .cloned()
        .collect::<BTreeSet<_>>();
    let mut changed_count = write_bootstrap_documents(profile, documents, schema_root)?;
    let mut removed_count = remove_stale_documents(previous, &protected_names, schema_root)?;
    let membership_path = schema_root.join(BUNDLE_MEMBERSHIP_FILE);
    if membership_path.is_file() {
        fs::remove_file(&membership_path).map_err(|error| {
            format!(
                "remove package schema membership {}: {error}",
                membership_path.display()
            )
        })?;
        removed_count += 1;
    }
    changed_count += write_public_receipt(receipt, &receipt_path)?;
    sync_directory(schema_root)?;
    Ok(report(
        profile,
        receipt,
        changed_count,
        removed_count,
        receipt_path,
    ))
}

pub(super) fn write_bundle(
    profile: &LanguageSchemaProfile,
    receipt: &LanguageSchemaBundleReceipt,
    documents: &BTreeMap<String, Vec<u8>>,
    schema_root: &Path,
    protected_names: &BTreeSet<String>,
) -> Result<SchemaBundleReport, String> {
    fs::create_dir_all(schema_root).map_err(|error| {
        format!(
            "create schema bundle root {}: {error}",
            schema_root.display()
        )
    })?;
    let receipt_path = schema_root.join(BUNDLE_RECEIPT_FILE);
    let previous = read_receipt_if_present(&receipt_path).unwrap_or_default();
    let expected_names = documents.keys().cloned().collect::<BTreeSet<_>>();
    let mut changed_count = write_documents(documents, schema_root)?;
    let retained_names = expected_names
        .union(protected_names)
        .cloned()
        .collect::<BTreeSet<_>>();
    let removed_count = remove_stale_documents(previous, &retained_names, schema_root)?;
    changed_count += write_public_receipt(receipt, &receipt_path)?;
    changed_count += write_membership(receipt, schema_root)?;
    sync_directory(schema_root)?;
    Ok(report(
        profile,
        receipt,
        changed_count,
        removed_count,
        receipt_path,
    ))
}

fn write_bootstrap_documents(
    profile: &LanguageSchemaProfile,
    documents: &BTreeMap<String, Vec<u8>>,
    schema_root: &Path,
) -> Result<usize, String> {
    let selected = profile
        .bootstrap
        .iter()
        .map(|name| {
            documents
                .get(name)
                .map(|bytes| (name.clone(), bytes.clone()))
                .ok_or_else(|| {
                    format!(
                        "bootstrap schema is outside canonical closure for {}: {name}",
                        profile.language_id
                    )
                })
        })
        .collect::<Result<BTreeMap<_, _>, _>>()?;
    write_documents(&selected, schema_root)
}

fn write_documents(
    documents: &BTreeMap<String, Vec<u8>>,
    schema_root: &Path,
) -> Result<usize, String> {
    let mut changed_count = 0;
    for (name, bytes) in documents {
        let target = schema_root.join(name);
        if fs::read(&target).ok().as_deref() != Some(bytes.as_slice()) {
            atomic_write(&target, bytes)?;
            changed_count += 1;
        }
    }
    Ok(changed_count)
}

fn remove_stale_documents(
    previous: Option<LanguageSchemaBundleReceipt>,
    retained_names: &BTreeSet<String>,
    schema_root: &Path,
) -> Result<usize, String> {
    let mut removed_count = 0;
    for stale in previous
        .into_iter()
        .flat_map(|receipt| receipt.schemas)
        .map(|entry| entry.name)
        .filter(|name| !retained_names.contains(name))
    {
        let stale_path = schema_root.join(&stale);
        if stale_path.is_file() {
            fs::remove_file(&stale_path).map_err(|error| {
                format!(
                    "remove stale managed schema {}: {error}",
                    stale_path.display()
                )
            })?;
            removed_count += 1;
        }
    }
    Ok(removed_count)
}

fn write_public_receipt(
    receipt: &LanguageSchemaBundleReceipt,
    receipt_path: &Path,
) -> Result<usize, String> {
    let bytes = serde_json::to_vec_pretty(receipt)
        .map_err(|error| format!("encode schema bundle receipt: {error}"))?;
    write_if_changed(receipt_path, &bytes)
}

fn write_membership(
    receipt: &LanguageSchemaBundleReceipt,
    schema_root: &Path,
) -> Result<usize, String> {
    let membership = SchemaBundleMembership {
        language_id: receipt.language_id.clone(),
        profile_digest: receipt.profile_digest.clone(),
        bundle_digest: receipt.bundle_digest.clone(),
        schemas: receipt.schemas.clone(),
    };
    let bytes = serde_json::to_vec_pretty(&membership)
        .map_err(|error| format!("encode schema bundle membership: {error}"))?;
    write_if_changed(&schema_root.join(BUNDLE_MEMBERSHIP_FILE), &bytes)
}

fn write_if_changed(path: &Path, bytes: &[u8]) -> Result<usize, String> {
    if fs::read(path).ok().as_deref() == Some(bytes) {
        return Ok(0);
    }
    atomic_write(path, bytes)?;
    Ok(1)
}

pub(super) fn report(
    profile: &LanguageSchemaProfile,
    receipt: &LanguageSchemaBundleReceipt,
    changed_count: usize,
    removed_count: usize,
    receipt_path: PathBuf,
) -> SchemaBundleReport {
    SchemaBundleReport {
        language_id: profile.language_id.clone(),
        schema_count: receipt.schemas.len(),
        changed_count,
        removed_count,
        receipt_path,
        bundle_digest: receipt.bundle_digest.clone(),
    }
}

fn atomic_write(target: &Path, bytes: &[u8]) -> Result<(), String> {
    let parent = target
        .parent()
        .ok_or_else(|| format!("schema target has no parent: {}", target.display()))?;
    let mut staged = tempfile::NamedTempFile::new_in(parent)
        .map_err(|error| format!("stage schema bundle file in {}: {error}", parent.display()))?;
    staged
        .write_all(bytes)
        .and_then(|()| staged.as_file().sync_all())
        .map_err(|error| format!("write staged schema bundle file: {error}"))?;
    let staged_path = staged.into_temp_path();
    fs::rename(&staged_path, target)
        .map_err(|error| format!("atomically publish schema {}: {error}", target.display()))?;
    Ok(())
}

fn sync_directory(path: &Path) -> Result<(), String> {
    fs::File::open(path)
        .and_then(|directory| directory.sync_all())
        .map_err(|error| format!("sync schema bundle directory {}: {error}", path.display()))
}
