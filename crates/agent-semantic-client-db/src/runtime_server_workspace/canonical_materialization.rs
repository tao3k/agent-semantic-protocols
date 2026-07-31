use serde::{Deserialize, Serialize};

use super::{
    WorkspaceGenerationState, WorkspaceMemoryGeneration, WorkspaceOwnerSnapshot,
    WorkspaceSelectorSnapshot, validate_owners,
};

pub const WORKSPACE_CANONICAL_MATERIALIZATION_SCHEMA_ID: &str =
    "agent.semantic-protocols.runtime-server-workspace-canonical-materialization.v1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceCanonicalMaterialization {
    pub schema_id: String,
    pub schema_version: String,
    pub workspace_identity: String,
    pub workspace_snapshot: agent_semantic_content_identity::WorkspaceSnapshot,
    pub source_snapshot: agent_semantic_content_identity::SourceSnapshotEvidence,
    pub workspace_generation: agent_semantic_content_identity::workspace_generation_evidence::WorkspaceGenerationEvidenceV1,
    pub import_digest: String,
    pub file_count: u32,
    pub root_depth: [u8; 2],
    pub owners: Vec<WorkspaceOwnerSnapshot>,
}

impl WorkspaceCanonicalMaterialization {
    /// Whether two materializations commit the same canonical owner generation.
    ///
    /// Source acquisition provenance may differ while the provider-bound
    /// content identity and every materialized owner remain equal.
    pub fn has_same_generation_identity(&self, other: &Self) -> bool {
        self.schema_id == other.schema_id
            && self.schema_version == other.schema_version
            && self.workspace_identity == other.workspace_identity
            && self.workspace_snapshot == other.workspace_snapshot
            && self
                .source_snapshot
                .has_same_content_identity(&other.source_snapshot)
            && self.workspace_generation == other.workspace_generation
            && self.import_digest == other.import_digest
            && self.file_count == other.file_count
            && self.root_depth == other.root_depth
            && self.owners == other.owners
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
        source_snapshot: &agent_semantic_content_identity::SourceSnapshotEvidence,
        import: &crate::ClientDbSourceIndexImport,
        source_blobs: &crate::ClientDbSourceIndexSourceBlobs,
    ) -> Result<Self, String> {
        let mut owners = std::collections::BTreeMap::new();
        for (owner_path, bytes) in source_blobs.iter() {
            owners.insert(
                owner_path.to_owned(),
                WorkspaceOwnerSnapshot {
                    owner_path: owner_path.to_owned(),
                    content_digest: format!("blake3-256:{}", blake3::hash(bytes).to_hex()),
                    bytes: bytes.to_vec(),
                    selectors: Vec::new(),
                },
            );
        }
        for selector in &import.selectors {
            let proof = &selector.materialization_proof;
            let owner = owners.get_mut(&proof.owner_path).ok_or_else(|| {
                format!(
                    "source-index selector omitted exact owner bytes: ownerPath={} selector={}",
                    proof.owner_path, proof.structural_selector
                )
            })?;
            if proof.source_blob_digest != *blake3::hash(&owner.bytes).as_bytes() {
                return Err(format!(
                    "source-index selector source blob digest drift: ownerPath={} selector={}",
                    proof.owner_path, proof.structural_selector
                ));
            }
            let byte_start = usize::try_from(proof.source_byte_start).map_err(|_| {
                format!(
                    "source-index selector byte start overflow: ownerPath={} selector={}",
                    proof.owner_path, proof.structural_selector
                )
            })?;
            let byte_end = usize::try_from(proof.source_byte_end).map_err(|_| {
                format!(
                    "source-index selector byte end overflow: ownerPath={} selector={}",
                    proof.owner_path, proof.structural_selector
                )
            })?;
            let projected = owner.bytes.get(byte_start..byte_end).ok_or_else(|| {
                format!(
                    "source-index selector range is outside exact owner bytes: ownerPath={} selector={} byteStart={} byteEnd={} ownerBytes={}",
                    proof.owner_path,
                    proof.structural_selector,
                    byte_start,
                    byte_end,
                    owner.bytes.len()
                )
            })?;
            if projected != proof.projection {
                return Err(format!(
                    "source-index selector projection drift: ownerPath={} selector={}",
                    proof.owner_path, proof.structural_selector
                ));
            }
            owner.selectors.push(WorkspaceSelectorSnapshot {
                selector: proof.structural_selector.clone(),
                byte_start,
                byte_end,
            });
        }
        let mut owners = owners.into_values().collect::<Vec<_>>();
        for owner in &mut owners {
            owner
                .selectors
                .sort_by(|left, right| left.selector.cmp(&right.selector));
        }
        Self::new(
            workspace_identity,
            source_snapshot.clone(),
            import,
            [1, 0],
            owners,
        )
    }

    pub fn new(
        workspace_identity: impl Into<String>,
        source_snapshot: agent_semantic_content_identity::SourceSnapshotEvidence,
        import: &crate::ClientDbSourceIndexImport,
        root_depth: [u8; 2],
        owners: Vec<WorkspaceOwnerSnapshot>,
    ) -> Result<Self, String> {
        let file_count = u32::try_from(owners.len())
            .map_err(|_| "workspace canonical materialization file count overflow".to_owned())?;
        let file_hashes = import
            .file_hashes
            .iter()
            .map(|file| (file.path.as_str(), file.sha256.as_str()))
            .collect::<std::collections::BTreeMap<_, _>>();
        let source_file_hashes = owners
            .iter()
            .map(|owner| {
                let hash = file_hashes.get(owner.owner_path.as_str()).ok_or_else(|| {
                    format!(
                        "workspace canonical materialization missing source hash: ownerPath={}",
                        owner.owner_path
                    )
                })?;
                Ok((owner.owner_path.as_str(), *hash))
            })
            .collect::<Result<Vec<_>, String>>()?;
        let workspace_snapshot =
            agent_semantic_content_identity::WorkspaceSnapshot::from_file_hashes(
                source_file_hashes,
            );
        if workspace_snapshot.root_digest() != source_snapshot.root_digest {
            return Err(format!(
                "workspace canonical materialization source snapshot drift: expected={} actual={}",
                workspace_snapshot.root_digest(),
                source_snapshot.root_digest
            ));
        }
        let workspace_generation =
            Self::generation_evidence(&source_snapshot, root_depth, owners.len())?;
        Ok(Self {
            schema_id: WORKSPACE_CANONICAL_MATERIALIZATION_SCHEMA_ID.to_owned(),
            schema_version: "1".to_owned(),
            workspace_identity: workspace_identity.into(),
            workspace_snapshot,
            source_snapshot,
            workspace_generation,
            import_digest: Self::typed_digest(import)?,
            file_count,
            root_depth,
            owners,
        })
    }

    pub fn finalize_generation_evidence(
        mut self,
        workspace_snapshot: agent_semantic_content_identity::WorkspaceSnapshot,
        source_snapshot: agent_semantic_content_identity::SourceSnapshotEvidence,
    ) -> Result<Self, String> {
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
        Ok(self)
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
        let expected_import_digest = Self::typed_digest(import)?;
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
        self.workspace_snapshot.validate()?;
        if self.schema_id != WORKSPACE_CANONICAL_MATERIALIZATION_SCHEMA_ID
            || self.schema_version != "1"
        {
            return Err("workspace canonical materialization schema identity mismatch".to_owned());
        }
        if self.workspace_identity != workspace_identity {
            return Err(format!(
                "workspace canonical materialization identity mismatch: requested={workspace_identity} materialized={}",
                self.workspace_identity
            ));
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
        Ok(())
    }

    pub fn into_generation(self, active_epoch: u64) -> Result<WorkspaceMemoryGeneration, String> {
        let target_epoch = active_epoch
            .checked_add(1)
            .ok_or_else(|| "workspace generation epoch overflow".to_owned())?;
        let memory_backend_digest = Self::typed_digest(&self.owners)?;
        let generation_digest = Self::typed_digest(&(
            &self.workspace_identity,
            &self.workspace_snapshot,
            &self.source_snapshot,
            &self.workspace_generation,
            &self.import_digest,
            &memory_backend_digest,
        ))?;
        Ok(WorkspaceMemoryGeneration {
            workspace_identity: self.workspace_identity,
            state: WorkspaceGenerationState::Ready,
            active_epoch: target_epoch,
            generation_digest,
            root_depth: self.root_depth,
            workspace_snapshot: self.workspace_snapshot,
            source_snapshot: self.source_snapshot,
            workspace_generation: self.workspace_generation,
            memory_backend_digest,
            owners: self.owners,
        })
    }

    fn typed_digest<T: Serialize>(value: &T) -> Result<String, String> {
        let bytes = serde_json::to_vec(value)
            .map_err(|error| format!("encode typed materialization digest input: {error}"))?;
        Ok(format!("blake3-256:{}", blake3::hash(&bytes).to_hex()))
    }

    fn validate_materialized_owners(owners: &[WorkspaceOwnerSnapshot]) -> Result<(), String> {
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

    fn validate_source_index_proofs(
        &self,
        import: &crate::ClientDbSourceIndexImport,
    ) -> Result<(), String> {
        let owners = self
            .owners
            .iter()
            .map(|owner| (owner.owner_path.as_str(), owner))
            .collect::<std::collections::BTreeMap<_, _>>();
        let mut expected_selectors = std::collections::BTreeSet::new();
        for selector in &import.selectors {
            let proof = &selector.materialization_proof;
            let owner = owners.get(proof.owner_path.as_str()).ok_or_else(|| {
                format!(
                    "workspace canonical materialization omitted proof owner: ownerPath={} selector={}",
                    proof.owner_path, proof.structural_selector
                )
            })?;
            if proof.source_blob_digest != *blake3::hash(&owner.bytes).as_bytes() {
                return Err(format!(
                    "workspace canonical materialization proof source drift: ownerPath={} selector={}",
                    proof.owner_path, proof.structural_selector
                ));
            }
            let byte_start = usize::try_from(proof.source_byte_start).map_err(|_| {
                format!(
                    "workspace canonical materialization proof start overflow: ownerPath={} selector={}",
                    proof.owner_path, proof.structural_selector
                )
            })?;
            let byte_end = usize::try_from(proof.source_byte_end).map_err(|_| {
                format!(
                    "workspace canonical materialization proof end overflow: ownerPath={} selector={}",
                    proof.owner_path, proof.structural_selector
                )
            })?;
            let materialized_selector = owner
                .selectors
                .iter()
                .find(|candidate| candidate.selector == proof.structural_selector)
                .ok_or_else(|| {
                    format!(
                        "workspace canonical materialization omitted proof selector: ownerPath={} selector={}",
                        proof.owner_path, proof.structural_selector
                    )
                })?;
            if materialized_selector.byte_start != byte_start
                || materialized_selector.byte_end != byte_end
                || owner.bytes.get(byte_start..byte_end) != Some(proof.projection.as_slice())
            {
                return Err(format!(
                    "workspace canonical materialization proof projection drift: ownerPath={} selector={}",
                    proof.owner_path, proof.structural_selector
                ));
            }
            expected_selectors.insert((
                proof.owner_path.as_str(),
                proof.structural_selector.as_str(),
            ));
        }
        let actual_selectors =
            self.owners
                .iter()
                .flat_map(|owner| {
                    owner.selectors.iter().map(move |selector| {
                        (owner.owner_path.as_str(), selector.selector.as_str())
                    })
                })
                .collect::<std::collections::BTreeSet<_>>();
        if actual_selectors != expected_selectors {
            return Err(
                "workspace canonical materialization selector set differs from parser proofs"
                    .to_owned(),
            );
        }
        Ok(())
    }
}
