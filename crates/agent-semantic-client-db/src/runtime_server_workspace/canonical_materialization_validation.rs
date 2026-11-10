// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Exact owner and selector proof validation for canonical workspace materializations.

use super::{WorkspaceCanonicalMaterialization, WorkspaceOwnerSnapshot};

impl WorkspaceCanonicalMaterialization {
    pub(super) fn validate_materialized_owners(
        owners: &[WorkspaceOwnerSnapshot],
    ) -> Result<(), String> {
        let mut owner_paths = std::collections::HashSet::with_capacity(owners.len());
        for owner in owners {
            if !owner_paths.insert(owner.owner_path.as_str()) {
                return Err(format!(
                    "workspace canonical materialization contains duplicate owner: {}",
                    owner.owner_path
                ));
            }
            let expected_content_digest =
                format!("blake3-256:{}", blake3::hash(&owner.bytes).to_hex());
            if owner.content_digest != expected_content_digest {
                return Err(format!(
                    "workspace canonical materialization owner digest drift: ownerPath={} expected={} actual={}",
                    owner.owner_path, expected_content_digest, owner.content_digest
                ));
            }
            if let Some(diagnostic) = &owner.native_syntax_diagnostic
                && (diagnostic.owner_path != owner.owner_path
                    || diagnostic.content_digest != owner.content_digest
                    || diagnostic.reason_kind != "source-syntax-unavailable"
                    || diagnostic.message.trim().is_empty()
                    || !owner.selectors.is_empty())
            {
                return Err(format!(
                    "workspace canonical materialization native syntax diagnostic drift: ownerPath={}",
                    owner.owner_path
                ));
            }
            for selector in &owner.selectors {
                if selector.byte_start > selector.byte_end || selector.byte_end > owner.bytes.len()
                {
                    return Err(format!(
                        "workspace canonical materialization selector range is outside owner bytes: ownerPath={} selector={} byteStart={} byteEnd={} ownerBytes={}",
                        owner.owner_path,
                        selector.selector,
                        selector.byte_start,
                        selector.byte_end,
                        owner.bytes.len()
                    ));
                }
            }
        }
        Ok(())
    }

    pub(crate) fn validate_incremental_source_index_proofs(
        &self,
        import: &crate::ClientDbSourceIndexImport,
    ) -> Result<(), String> {
        let owners = self.owner_membership_index();
        let complete_file_hashes = import
            .file_hashes
            .iter()
            .map(|file_hash| (file_hash.path.as_str(), file_hash))
            .collect::<std::collections::BTreeMap<_, _>>();
        let selectors = self.selector_membership_index();
        for imported_owner in &import.owners {
            let owner_path = imported_owner.owner_path.as_str();
            let materialized_owner = owners.get(owner_path).ok_or_else(|| {
                format!(
                    "workspace canonical materialization omitted incremental owner: ownerPath={owner_path}"
                )
            })?;
            let file_hash = complete_file_hashes.get(owner_path).ok_or_else(|| {
                format!(
                    "workspace canonical materialization incremental owner has no file hash: ownerPath={owner_path}"
                )
            })?;
            let materialized_sha256 =
                <sha2::Sha256 as sha2::Digest>::digest(materialized_owner.bytes.as_slice());
            let actual_sha256 = format!("{materialized_sha256:x}");
            if file_hash.sha256 != actual_sha256 {
                return Err(format!(
                    "workspace canonical materialization incremental owner digest drift: ownerPath={owner_path} expected={} actual={actual_sha256}",
                    file_hash.sha256
                ));
            }
        }
        for selector in &import.selectors {
            self.validate_selector_proof(selector, &owners, &selectors)?;
        }
        Ok(())
    }

    pub(super) fn validate_source_index_proofs(
        &self,
        import: &crate::ClientDbSourceIndexImport,
    ) -> Result<(), String> {
        let owners = self.owner_membership_index();
        let selectors = self.selector_membership_index();
        let mut expected_selectors = std::collections::BTreeSet::new();
        for selector in &import.selectors {
            let proof = &selector.projection_record.proof;
            self.validate_selector_proof(selector, &owners, &selectors)?;
            expected_selectors.insert((proof.owner_path(), proof.structural_selector()));
        }
        let actual_selectors = selectors
            .keys()
            .copied()
            .collect::<std::collections::BTreeSet<_>>();
        if actual_selectors != expected_selectors {
            return Err(
                "workspace canonical materialization selector set differs from parser proofs"
                    .to_owned(),
            );
        }
        Ok(())
    }

    fn validate_selector_proof(
        &self,
        selector: &crate::ClientDbSourceIndexSelector,
        owners: &std::collections::BTreeMap<&str, &WorkspaceOwnerSnapshot>,
        selectors: &std::collections::BTreeMap<
            (&str, &str),
            &crate::runtime_server_workspace::WorkspaceSelectorSnapshot,
        >,
    ) -> Result<(), String> {
        let record = &selector.projection_record;
        let proof = &record.proof;
        let owner = owners.get(proof.owner_path()).ok_or_else(|| {
            format!(
                "workspace canonical materialization omitted proof owner: ownerPath={} selector={}",
                proof.owner_path(),
                proof.structural_selector()
            )
        })?;
        if proof.source_blob_digest()
            != &agent_semantic_content_identity::exact_selector_merkle::blake3_content_digest_v1(
                &owner.bytes,
            )
        {
            return Err(format!(
                "workspace canonical materialization proof source drift: ownerPath={} selector={}",
                proof.owner_path(),
                proof.structural_selector()
            ));
        }
        let byte_start = usize::try_from(record.source_byte_range.start).map_err(|_| {
            format!(
                "workspace canonical materialization proof start overflow: ownerPath={} selector={}",
                proof.owner_path(),
                proof.structural_selector()
            )
        })?;
        let byte_end = usize::try_from(record.source_byte_range.end).map_err(|_| {
            format!(
                "workspace canonical materialization proof end overflow: ownerPath={} selector={}",
                proof.owner_path(),
                proof.structural_selector()
            )
        })?;
        let materialized_selector = selectors
            .get(&(proof.owner_path(), proof.structural_selector()))
            .ok_or_else(|| {
                format!(
                    "workspace canonical materialization omitted proof selector: ownerPath={} selector={}",
                    proof.owner_path(),
                    proof.structural_selector()
                )
            })?;
        if materialized_selector.byte_start != byte_start
            || materialized_selector.byte_end != byte_end
            || owner.bytes.get(byte_start..byte_end) != Some(record.projection_payload.as_slice())
        {
            return Err(format!(
                "workspace canonical materialization proof projection drift: ownerPath={} selector={}",
                proof.owner_path(),
                proof.structural_selector()
            ));
        }
        Ok(())
    }

    fn owner_membership_index(&self) -> std::collections::BTreeMap<&str, &WorkspaceOwnerSnapshot> {
        self.owners
            .iter()
            .map(|owner| (owner.owner_path.as_str(), owner))
            .collect()
    }

    fn selector_membership_index(
        &self,
    ) -> std::collections::BTreeMap<
        (&str, &str),
        &crate::runtime_server_workspace::WorkspaceSelectorSnapshot,
    > {
        self.owners
            .iter()
            .flat_map(|owner| {
                owner.selectors.iter().map(move |selector| {
                    (
                        (owner.owner_path.as_str(), selector.selector.as_str()),
                        selector,
                    )
                })
            })
            .collect()
    }
}
