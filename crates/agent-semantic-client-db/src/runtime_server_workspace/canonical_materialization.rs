// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use serde::Serialize;

#[path = "canonical_materialization_model.rs"]
mod model;
pub use model::{
    ValidatedWorkspaceCanonicalMaterialization, WorkspaceCanonicalMaterialization,
    WorkspaceCanonicalMaterializationLoad,
};

#[path = "canonical_materialization_observability.rs"]
mod observability;

#[path = "canonical_materialization_validation.rs"]
mod validation;

use super::{
    WorkspaceMemoryGeneration, WorkspaceOwnerSnapshot, WorkspaceSelectorSnapshot,
    canonical_snapshot::{
        validate_auxiliary_snapshot_membership, validate_canonical_snapshot,
        validate_owner_snapshot_membership,
    },
    validate_owners,
};

pub const WORKSPACE_CANONICAL_MATERIALIZATION_SCHEMA_ID: &str =
    "agent.semantic-protocols.runtime-server-workspace-canonical-materialization.v2";

struct CanonicalMaterializationDerived {
    project_root: String,
    workspace_snapshot: agent_semantic_content_identity::WorkspaceSnapshot,
    workspace_generation: agent_semantic_content_identity::workspace_generation_evidence::WorkspaceGenerationEvidenceV1,
    provider_schema_digest: String,
    import_digest: String,
    selector_set_digest: String,
    projection_capability: crate::active_generation_projection_capability::ActiveGenerationProjectionCapabilityManifest,
    workspace_source_scope_generation: String,
    file_count: u32,
}

fn derive_canonical_materialization(
    workspace_snapshot: agent_semantic_content_identity::WorkspaceSnapshot,
    source_snapshot: &agent_semantic_content_identity::SourceSnapshotEvidence,
    import: &crate::ClientDbSourceIndexImport,
    root_depth: [u8; 2],
    owners: &[WorkspaceOwnerSnapshot],
    project_resolutions: &[agent_semantic_content_identity::AdmittedProjectResolution],
) -> Result<CanonicalMaterializationDerived, String> {
    let file_count = u32::try_from(owners.len())
        .map_err(|_| "workspace canonical materialization file count overflow".to_owned())?;
    validate_canonical_snapshot(&workspace_snapshot, source_snapshot)?;
    validate_owner_snapshot_membership(&workspace_snapshot, owners)?;
    let selector_rows = owners
        .iter()
        .map(|owner| (&owner.owner_path, &owner.selectors))
        .collect::<Vec<_>>();
    Ok(CanonicalMaterializationDerived {
        project_root: WorkspaceCanonicalMaterialization::canonical_project_root(
            &import.project_root,
        )?,
        workspace_snapshot,
        workspace_generation: WorkspaceCanonicalMaterialization::generation_evidence(
            source_snapshot,
            root_depth,
            owners.len(),
        )?,
        provider_schema_digest: agent_semantic_content_identity::project_resolution_schema_digest(),
        import_digest: WorkspaceCanonicalMaterialization::canonical_import_digest(import)?,
        selector_set_digest: WorkspaceCanonicalMaterialization::typed_digest(&selector_rows)?,
        projection_capability: crate::active_generation_projection_capability::ActiveGenerationProjectionCapabilityManifest::from_source_index(
            source_snapshot.provider_digest.clone(),
            &import.selectors,
        )?,
        workspace_source_scope_generation:
            agent_semantic_content_identity::workspace_source_scope_generation_digest(project_resolutions)?,
        file_count,
    })
}

#[expect(
    clippy::too_many_arguments,
    reason = "canonical generation assembly binds all independently verified V1 evidence"
)]
fn assemble_canonical_materialization(
    workspace_identity: String,
    source_snapshot: agent_semantic_content_identity::SourceSnapshotEvidence,
    import: &crate::ClientDbSourceIndexImport,
    root_depth: [u8; 2],
    owners: Vec<WorkspaceOwnerSnapshot>,
    auxiliary_owners: Vec<super::WorkspaceAuxiliaryOwnerSnapshot>,
    project_resolutions: Vec<agent_semantic_content_identity::AdmittedProjectResolution>,
    derived: CanonicalMaterializationDerived,
) -> WorkspaceCanonicalMaterialization {
    WorkspaceCanonicalMaterialization {
        schema_id: WORKSPACE_CANONICAL_MATERIALIZATION_SCHEMA_ID.to_owned(),
        schema_version: "2".to_owned(),
        workspace_identity,
        project_root: derived.project_root,
        workspace_snapshot: derived.workspace_snapshot,
        source_snapshot,
        workspace_generation: derived.workspace_generation,
        provider_schema_digest: derived.provider_schema_digest,
        import_digest: derived.import_digest,
        runtime_provider_execution_binding: None,
        selector_set_digest: derived.selector_set_digest,
        content_search_generation: None,
        projection_capability: derived.projection_capability,
        workspace_source_scope_generation: derived.workspace_source_scope_generation,
        project_resolutions,
        auxiliary_owners,
        relations: import.relations.clone(),
        file_count: derived.file_count,
        root_depth,
        owners,
    }
}

impl WorkspaceCanonicalMaterialization {
    pub fn attach_content_search_generation(
        &mut self,
        receipt: agent_semantic_search::ContentSearchGenerationReceipt,
    ) -> Result<(), String> {
        receipt.validate()?;
        let identity = receipt.identity();
        let source_root_digest =
            agent_semantic_search::canonical_blake3_digest(&self.source_snapshot.root_digest)?;
        if identity.workspace_id != self.workspace_identity
            || identity.source_root_digest != source_root_digest
            || identity.provider_digest
                != agent_semantic_search::canonical_blake3_digest(
                    &self.source_snapshot.provider_digest,
                )?
            || identity.schema_digest
                != agent_semantic_search::canonical_blake3_digest(
                    &agent_semantic_content_identity::project_resolution_schema_digest(),
                )?
        {
            return Err("content search generation materialization binding drift".to_owned());
        }
        self.content_search_generation = Some(receipt);
        Ok(())
    }

    pub fn require_content_search_generation(
        &self,
    ) -> Result<&agent_semantic_search::ContentSearchGenerationReceipt, String> {
        let receipt = self.content_search_generation.as_ref().ok_or_else(|| {
            "query-not-ready: canonical generation lacks content search construction receipt"
                .to_owned()
        })?;
        receipt.validate()?;
        Ok(receipt)
    }

    pub fn into_validated(
        self,
        workspace_identity: &str,
    ) -> Result<ValidatedWorkspaceCanonicalMaterialization, String> {
        ValidatedWorkspaceCanonicalMaterialization::new(self, workspace_identity)
    }
}

impl WorkspaceCanonicalMaterialization {
    fn canonical_import_digest(
        import: &crate::ClientDbSourceIndexImport,
    ) -> Result<String, String> {
        let mut canonical = import.clone();
        for selector in &mut canonical.selectors {
            selector.source = "".into();
        }
        Self::typed_digest(&canonical)
    }

    pub(crate) fn validate_complete_source_index_import_identity(
        &self,
        import: &crate::ClientDbSourceIndexImport,
    ) -> Result<(), String> {
        let observed = Self::canonical_import_digest(import)?;
        if self.import_digest != observed {
            return Err(format!(
                "workspace canonical materialization import identity drift: expected={} observed={observed}",
                self.import_digest
            ));
        }
        let snapshot_paths = self
            .workspace_snapshot
            .file_digests()
            .map(|(path, _)| path)
            .collect::<std::collections::BTreeSet<_>>();
        let blob_paths = import
            .source_blobs
            .iter()
            .map(|(path, _)| path)
            .collect::<std::collections::BTreeSet<_>>();
        let hash_paths = import
            .file_hashes
            .iter()
            .filter(|record| !record.path.starts_with("@scope/"))
            .map(|record| record.path.as_str())
            .collect::<std::collections::BTreeSet<_>>();
        if snapshot_paths != blob_paths || snapshot_paths != hash_paths {
            return Err(format!(
                "workspace canonical materialization complete source membership drift: snapshotCount={} blobCount={} hashCount={}",
                snapshot_paths.len(),
                blob_paths.len(),
                hash_paths.len()
            ));
        }
        Ok(())
    }

    pub(crate) fn canonical_project_root(
        project_root: impl AsRef<std::path::Path>,
    ) -> Result<String, String> {
        crate::types::normalized_project_root(project_root.as_ref())
    }

    /// Whether two materializations commit the same canonical owner generation.
    ///
    /// Source acquisition provenance may differ while the provider-bound
    /// content identity and every materialized owner remain equal.
    pub fn has_same_generation_identity(&self, other: &Self) -> bool {
        self.generation_identity_mismatch_fields(other).is_empty()
    }

    /// Names every canonical identity component that drifted across resident
    /// publication and durability. This is intentionally field-level: logs
    /// must diagnose the violated proof boundary without dumping source bytes.
    pub fn generation_identity_mismatch_fields(&self, other: &Self) -> Vec<&'static str> {
        let mut fields = Vec::new();
        macro_rules! compare {
            ($field:ident) => {
                if self.$field != other.$field {
                    fields.push(stringify!($field));
                }
            };
        }
        compare!(schema_id);
        compare!(schema_version);
        compare!(workspace_identity);
        compare!(project_root);
        compare!(workspace_snapshot);
        if !self
            .source_snapshot
            .has_same_content_identity(&other.source_snapshot)
        {
            fields.push("source_snapshot_content_identity");
        }
        compare!(workspace_generation);
        compare!(provider_schema_digest);
        compare!(import_digest);
        compare!(runtime_provider_execution_binding);
        compare!(selector_set_digest);
        compare!(workspace_source_scope_generation);
        compare!(project_resolutions);
        compare!(auxiliary_owners);
        compare!(file_count);
        compare!(root_depth);
        compare!(owners);
        fields
    }

    pub(crate) fn generation_identity_digest(&self) -> Result<String, String> {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct GenerationIdentity<'a> {
            schema_id: &'a str,
            schema_version: &'a str,
            workspace_identity: &'a str,
            project_root: &'a str,
            workspace_snapshot: &'a agent_semantic_content_identity::WorkspaceSnapshot,
            workspace_generation: &'a agent_semantic_content_identity::workspace_generation_evidence::WorkspaceGenerationEvidenceV1,
            provider_schema_digest: &'a str,
            import_digest: &'a str,
            runtime_provider_execution_binding: &'a Option<agent_semantic_artifacts::runtime_provider_execution_binding::RuntimeProviderExecutionBinding>,
            selector_set_digest: &'a str,
            workspace_source_scope_generation: &'a str,
            project_resolutions: &'a [agent_semantic_content_identity::AdmittedProjectResolution],
            file_count: u32,
            root_depth: [u8; 2],
            owners: &'a [WorkspaceOwnerSnapshot],
            auxiliary_owners: &'a [super::WorkspaceAuxiliaryOwnerSnapshot],
        }

        Self::typed_digest(&GenerationIdentity {
            schema_id: &self.schema_id,
            schema_version: &self.schema_version,
            workspace_identity: &self.workspace_identity,
            project_root: &self.project_root,
            workspace_snapshot: &self.workspace_snapshot,
            workspace_generation: &self.workspace_generation,
            provider_schema_digest: &self.provider_schema_digest,
            import_digest: &self.import_digest,
            runtime_provider_execution_binding: &self.runtime_provider_execution_binding,
            selector_set_digest: &self.selector_set_digest,
            workspace_source_scope_generation: &self.workspace_source_scope_generation,
            project_resolutions: &self.project_resolutions,
            file_count: self.file_count,
            root_depth: self.root_depth,
            owners: &self.owners,
            auxiliary_owners: &self.auxiliary_owners,
        })
    }

    fn generation_evidence(
        source_snapshot: &agent_semantic_content_identity::SourceSnapshotEvidence,
        root_depth: [u8; 2],
        owner_count: usize,
    ) -> Result<
        agent_semantic_content_identity::workspace_generation_evidence::WorkspaceGenerationEvidenceV1,
        String,
    >{
        let evidence =
            agent_semantic_content_identity::workspace_generation_evidence::WorkspaceGenerationEvidenceV1 {
                root_digest: source_snapshot.root_digest.clone(),
                root_depth: u32::from(root_depth[0]),
                leaf_count: u64::try_from(source_snapshot.leaf_count)
                    .map_err(|_| "workspace generation leaf count overflow".to_owned())?,
                owner_count: u64::try_from(owner_count)
                    .map_err(|_| "workspace generation owner count overflow".to_owned())?,
            };
        agent_semantic_content_identity::workspace_generation_evidence::ValidatedWorkspaceGenerationV1::new(
            evidence.clone(),
        )
        .map_err(|error| format!("workspace generation evidence is incomplete: {error}"))?;
        Ok(evidence)
    }

    pub fn from_source_index(
        workspace_identity: impl Into<String>,
        workspace_snapshot: &agent_semantic_content_identity::WorkspaceSnapshot,
        source_snapshot: &agent_semantic_content_identity::SourceSnapshotEvidence,
        import: &crate::ClientDbSourceIndexImport,
        source_blobs: &crate::ClientDbSourceIndexSourceBlobs,
        project_resolutions: Vec<agent_semantic_content_identity::AdmittedProjectResolution>,
    ) -> Result<Self, String> {
        Self::from_source_index_inner(
            workspace_identity.into(),
            workspace_snapshot,
            source_snapshot,
            import,
            source_blobs,
            project_resolutions,
        )
    }

    fn from_source_index_inner(
        workspace_identity: String,
        workspace_snapshot: &agent_semantic_content_identity::WorkspaceSnapshot,
        source_snapshot: &agent_semantic_content_identity::SourceSnapshotEvidence,
        import: &crate::ClientDbSourceIndexImport,
        source_blobs: &crate::ClientDbSourceIndexSourceBlobs,
        project_resolutions: Vec<agent_semantic_content_identity::AdmittedProjectResolution>,
    ) -> Result<Self, String> {
        let owner_authorities = import
            .owners
            .iter()
            .filter_map(|owner| {
                owner
                    .language_id
                    .as_ref()
                    .zip(owner.provider_id.as_ref())
                    .map(|(language_id, provider_id)| {
                        (
                            owner.owner_path.as_str(),
                            agent_semantic_search::ResidentSearchAuthority {
                                language_id: language_id.clone(),
                                provider_id: provider_id.clone(),
                            },
                        )
                    })
            })
            .collect::<std::collections::BTreeMap<_, _>>();
        let mut owners = std::collections::BTreeMap::new();
        for indexed_owner in &import.owners {
            let owner_path = indexed_owner.owner_path.as_str();
            let bytes = source_blobs.get(&indexed_owner.owner_path).ok_or_else(|| {
                format!("source-index owner omitted same-pass bytes: ownerPath={owner_path}")
            })?;
            owners.insert(
                owner_path.to_owned(),
                WorkspaceOwnerSnapshot {
                    owner_path: owner_path.to_owned(),
                    authority: owner_authorities.get(owner_path).cloned(),
                    content_digest: format!("blake3-256:{}", blake3::hash(bytes).to_hex()),
                    native_syntax_diagnostic: None,
                    bytes: bytes.to_vec(),
                    selectors: Vec::new(),
                },
            );
        }
        for selector in &import.selectors {
            let record = &selector.projection_record;
            let proof = &record.proof;
            let owner = owners.get_mut(proof.owner_path()).ok_or_else(|| {
                format!(
                    "source-index selector omitted exact owner bytes: ownerPath={} selector={}",
                    proof.owner_path(),
                    proof.structural_selector()
                )
            })?;
            // This digest was computed above from these exact immutable bytes.
            // Rehashing the owner for each selector multiplies byte work by
            // selector count; only the selectors are mutated during assembly.
            if owner.content_digest.strip_prefix("blake3-256:")
                != Some(proof.source_blob_digest().as_str())
            {
                return Err(format!(
                    "source-index selector source blob digest drift: ownerPath={} selector={}",
                    proof.owner_path(),
                    proof.structural_selector()
                ));
            }
            let byte_start = usize::try_from(record.source_byte_range.start).map_err(|_| {
                format!(
                    "source-index selector byte start overflow: ownerPath={} selector={}",
                    proof.owner_path(),
                    proof.structural_selector()
                )
            })?;
            let byte_end = usize::try_from(record.source_byte_range.end).map_err(|_| {
                format!(
                    "source-index selector byte end overflow: ownerPath={} selector={}",
                    proof.owner_path(),
                    proof.structural_selector()
                )
            })?;
            let projected = owner.bytes.get(byte_start..byte_end).ok_or_else(|| {
                format!(
                    "source-index selector range is outside exact owner bytes: ownerPath={} selector={} byteStart={} byteEnd={} ownerBytes={}",
                    proof.owner_path(),
                    proof.structural_selector(),
                    byte_start,
                    byte_end,
                    owner.bytes.len()
                )
            })?;
            if projected != record.projection_payload {
                return Err(format!(
                    "source-index selector projection drift: ownerPath={} selector={}",
                    proof.owner_path(),
                    proof.structural_selector()
                ));
            }
            let mut query_keys = selector
                .query_keys
                .iter()
                .map(|key| key.as_str().to_owned())
                .collect::<Vec<_>>();
            query_keys.sort();
            query_keys.dedup();
            owner.selectors.push(WorkspaceSelectorSnapshot {
                selector: proof.structural_selector().to_owned(),
                byte_start,
                byte_end,
                query_keys,
                derived_projections: selector.derived_projections.clone(),
            });
        }
        let mut owners = owners.into_values().collect::<Vec<_>>();
        for owner in &mut owners {
            owner
                .selectors
                .sort_by(|left, right| left.selector.cmp(&right.selector));
        }
        let indexed_owner_paths = import
            .owners
            .iter()
            .map(|owner| owner.owner_path.as_str())
            .collect::<std::collections::BTreeSet<_>>();
        let mut auxiliary_owners = source_blobs
            .iter()
            .filter(|(path, _)| !indexed_owner_paths.contains(*path))
            .map(|(path, bytes)| super::WorkspaceAuxiliaryOwnerSnapshot {
                owner_path: path.to_string(),
                content_digest: format!("blake3-256:{}", blake3::hash(bytes).to_hex()),
                bytes: bytes.to_vec(),
            })
            .collect::<Vec<_>>();
        auxiliary_owners.sort_by(|left, right| left.owner_path.cmp(&right.owner_path));
        Self::new_with_workspace_snapshot(
            workspace_identity,
            workspace_snapshot.clone(),
            source_snapshot.clone(),
            import,
            [1, 0],
            owners,
            auxiliary_owners,
            project_resolutions,
        )
    }

    pub fn new(
        workspace_identity: impl Into<String>,
        source_snapshot: agent_semantic_content_identity::SourceSnapshotEvidence,
        import: &crate::ClientDbSourceIndexImport,
        root_depth: [u8; 2],
        owners: Vec<WorkspaceOwnerSnapshot>,
        project_resolutions: Vec<agent_semantic_content_identity::AdmittedProjectResolution>,
    ) -> Result<Self, String> {
        let workspace_snapshot =
            agent_semantic_content_identity::WorkspaceSnapshot::from_file_bytes(
                owners
                    .iter()
                    .map(|owner| (owner.owner_path.as_str(), owner.bytes.as_slice())),
            );
        Self::new_with_workspace_snapshot(
            workspace_identity,
            workspace_snapshot,
            source_snapshot,
            import,
            root_depth,
            owners,
            Vec::new(),
            project_resolutions,
        )
    }

    #[expect(
        clippy::too_many_arguments,
        reason = "canonical generation construction binds all independently verified V1 evidence"
    )]
    pub(super) fn new_with_workspace_snapshot(
        workspace_identity: impl Into<String>,
        workspace_snapshot: agent_semantic_content_identity::WorkspaceSnapshot,
        source_snapshot: agent_semantic_content_identity::SourceSnapshotEvidence,
        import: &crate::ClientDbSourceIndexImport,
        root_depth: [u8; 2],
        owners: Vec<WorkspaceOwnerSnapshot>,
        auxiliary_owners: Vec<super::WorkspaceAuxiliaryOwnerSnapshot>,
        project_resolutions: Vec<agent_semantic_content_identity::AdmittedProjectResolution>,
    ) -> Result<Self, String> {
        let derived = derive_canonical_materialization(
            workspace_snapshot,
            &source_snapshot,
            import,
            root_depth,
            &owners,
            &project_resolutions,
        )?;
        Ok(assemble_canonical_materialization(
            workspace_identity.into(),
            source_snapshot,
            import,
            root_depth,
            owners,
            auxiliary_owners,
            project_resolutions,
            derived,
        ))
    }

    pub fn finalize_generation_evidence(
        &mut self,
        workspace_snapshot: agent_semantic_content_identity::WorkspaceSnapshot,
        source_snapshot: agent_semantic_content_identity::SourceSnapshotEvidence,
    ) -> Result<(), String> {
        if workspace_snapshot.root_digest() != source_snapshot.root_digest {
            return Err(format!(
                "workspace generation finalization snapshot drift: workspaceRoot={} sourceRoot={}",
                workspace_snapshot.root_digest(),
                source_snapshot.root_digest
            ));
        }
        self.workspace_generation =
            Self::generation_evidence(&source_snapshot, self.root_depth, self.owners.len())?;
        self.workspace_snapshot = workspace_snapshot;
        self.source_snapshot = source_snapshot;
        Ok(())
    }

    pub fn validate_against(
        &self,
        workspace_identity: &str,
        source_snapshot: &agent_semantic_content_identity::SourceSnapshotEvidence,
        import: &crate::ClientDbSourceIndexImport,
        owner_count: u32,
    ) -> Result<(), String> {
        self.validate_persisted(workspace_identity)?;
        if self.file_count != owner_count {
            return Err(format!(
                "workspace canonical materialization is incomplete: importOwnerCount={owner_count} materializedOwnerCount={}",
                self.file_count
            ));
        }
        if !self
            .source_snapshot
            .has_same_content_identity(source_snapshot)
        {
            return Err(format!(
                "workspace canonical materialization source snapshot drift: expected={source_snapshot:?} actual={:?}",
                self.source_snapshot
            ));
        }
        let expected_import_digest = Self::canonical_import_digest(import)?;
        let expected_project_root = Self::canonical_project_root(&import.project_root)?;
        let actual_project_root = Self::canonical_project_root(&self.project_root)?;
        if actual_project_root != expected_project_root {
            return Err(format!(
                "workspace canonical materialization project root drift: expected={} actual={}",
                expected_project_root, actual_project_root
            ));
        }
        if self.import_digest != expected_import_digest {
            return Err(format!(
                "workspace canonical materialization import drift: expected={expected_import_digest} actual={}",
                self.import_digest
            ));
        }
        self.validate_source_index_proofs(import)?;
        Ok(())
    }

    pub fn validate_refresh_request(
        &self,
        workspace_identity: &str,
        request: &crate::ClientDbSourceIndexRefreshRequest,
    ) -> Result<(), String> {
        let owner_count = u32::try_from(request.import.owners.len())
            .map_err(|_| "source-index owner count overflow".to_owned())?;
        self.validate_against(
            workspace_identity,
            &request.source_snapshot,
            &request.import,
            owner_count,
        )
    }

    pub fn validate_persisted(&self, workspace_identity: &str) -> Result<(), String> {
        self.validate_persisted_inner(workspace_identity)
    }

    fn validate_persisted_inner(&self, workspace_identity: &str) -> Result<(), String> {
        self.workspace_snapshot.validate()?;
        if self.schema_id != WORKSPACE_CANONICAL_MATERIALIZATION_SCHEMA_ID
            || self.schema_version != "2"
        {
            return Err("workspace canonical materialization schema identity mismatch".to_owned());
        }
        if self.workspace_identity != workspace_identity {
            return Err(format!(
                "workspace canonical materialization identity mismatch: requested={workspace_identity} materialized={}",
                self.workspace_identity
            ));
        }
        if self.project_root.trim().is_empty() {
            return Err(
                "workspace canonical materialization project root must be non-empty".to_owned(),
            );
        }
        if self.root_depth != [1, 0] {
            return Err("workspace canonical materialization rootDepth must be [1, 0]".to_owned());
        }
        if self.owners.len() != self.file_count as usize {
            return Err(format!(
                "workspace canonical materialization is incomplete: materializedFileCount={} ownerCount={}",
                self.file_count,
                self.owners.len()
            ));
        }
        Self::validate_materialized_owners(&self.owners)?;
        validate_owners(&self.owners)?;
        validate_auxiliary_snapshot_membership(&self.workspace_snapshot, &self.auxiliary_owners)?;
        if self.source_snapshot.root_digest.len() != 64
            || !self
                .source_snapshot
                .root_digest
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
        {
            return Err(
                "workspace canonical materialization source snapshot root digest is invalid"
                    .to_owned(),
            );
        }
        if self.workspace_snapshot.root_digest() != self.source_snapshot.root_digest {
            return Err(
                "workspace canonical materialization workspace/source snapshot drift".to_owned(),
            );
        }
        if self.workspace_generation.root_digest != self.source_snapshot.root_digest
            || self.workspace_generation.root_depth != u32::from(self.root_depth[0])
            || self.workspace_generation.leaf_count
                != u64::try_from(self.source_snapshot.leaf_count)
                    .map_err(|_| "workspace generation leaf count overflow".to_owned())?
            || self.workspace_generation.owner_count
                != u64::try_from(self.owners.len())
                    .map_err(|_| "workspace generation owner count overflow".to_owned())?
        {
            return Err("workspace canonical materialization generation evidence drift".to_owned());
        }
        agent_semantic_content_identity::workspace_generation_evidence::ValidatedWorkspaceGenerationV1::new(
            self.workspace_generation.clone(),
        )
        .map_err(|error| format!("workspace generation evidence is incomplete: {error}"))?;
        if !self.import_digest.starts_with("blake3-256:")
            || self.import_digest.len() != "blake3-256:".len() + 64
            || !self.import_digest["blake3-256:".len()..]
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
        {
            return Err("workspace canonical materialization import digest is invalid".to_owned());
        }
        if let Some(binding) = &self.runtime_provider_execution_binding {
            binding.validate()?;
            if binding.source_snapshot_digest != self.source_snapshot.root_integrity_reference()?
                || binding.source_index_digest != self.import_digest
            {
                return Err(
                    "workspace canonical materialization Runtime provider execution binding drift"
                        .to_owned(),
                );
            }
        }
        if self.workspace_source_scope_generation
            != agent_semantic_content_identity::workspace_source_scope_generation_digest(
                &self.project_resolutions,
            )?
        {
            return Err(
                "workspace canonical materialization ProjectResolution generation drift".to_owned(),
            );
        }
        Ok(())
    }

    pub fn into_generation(self, active_epoch: u64) -> Result<WorkspaceMemoryGeneration, String> {
        let content_search_generation =
            self.content_search_generation.clone().ok_or_else(|| {
                "query-not-ready: canonical generation lacks content search construction receipt"
                    .to_owned()
            })?;
        let target_epoch = active_epoch
            .checked_add(1)
            .ok_or_else(|| "workspace generation epoch overflow".to_owned())?;
        WorkspaceMemoryGeneration::try_from_build(super::WorkspaceGenerationBuild {
            projection_capability: self.projection_capability.clone(),
            workspace_identity: self.workspace_identity,
            project_root: self.project_root,
            active_epoch: target_epoch,
            workspace_snapshot: self.workspace_snapshot,
            source_snapshot: self.source_snapshot,
            module_graph_digest: self.import_digest,
            runtime_provider_execution_binding: self.runtime_provider_execution_binding,
            content_search_generation,
            project_resolutions: self.project_resolutions,
            auxiliary_owners: self.auxiliary_owners,
            relations: self.relations,
            owners: self.owners,
        })
    }

    pub fn bind_runtime_provider_execution(
        &mut self,
        binding: agent_semantic_artifacts::runtime_provider_execution_binding::RuntimeProviderExecutionBinding,
    ) -> Result<(), String> {
        binding.validate()?;
        if binding.source_snapshot_digest != self.source_snapshot.root_integrity_reference()?
            || binding.source_index_digest != self.import_digest
        {
            return Err(
                "workspace canonical materialization Runtime provider execution binding drift"
                    .to_owned(),
            );
        }
        self.runtime_provider_execution_binding = Some(binding);
        Ok(())
    }

    fn typed_digest<T: Serialize>(value: &T) -> Result<String, String> {
        let bytes = serde_json::to_vec(value)
            .map_err(|error| format!("encode typed materialization digest input: {error}"))?;
        Ok(format!("blake3-256:{}", blake3::hash(&bytes).to_hex()))
    }
}
