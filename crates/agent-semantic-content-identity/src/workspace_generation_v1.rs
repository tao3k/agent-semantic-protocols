use serde::{Deserialize, Serialize};

use crate::exact_selector_merkle::blake3_content_digest_v1;

pub const WORKSPACE_GENERATION_EVIDENCE_SCHEMA_ID: &str = "asp.workspace-generation.v1";
pub const WORKSPACE_GENERATION_DIGEST_ALGORITHM: &str = "blake3-256";

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum WorkspaceMaterializationStateV1 {
    ResidentMemory,
    ArtifactComplete,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum WorkspaceOwnerCoverageV1 {
    Complete,
    Partial,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceGenerationEvidenceV1 {
    pub schema_id: String,
    pub digest_algorithm: String,
    pub generation_digest: String,
    pub source_root_digest: String,
    pub root_depth: usize,
    pub leaf_count: usize,
    pub materialization_state: WorkspaceMaterializationStateV1,
    pub module_graph_digest: String,
    pub selector_generation_digest: String,
    pub tombstone_digest: String,
    pub package_graph_digest: String,
    pub envelope_digest: String,
    pub owner_coverage: WorkspaceOwnerCoverageV1,
    pub provider_digest: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkspaceGenerationComponentsV1 {
    pub source_root_digest: String,
    pub root_depth: usize,
    pub leaf_count: usize,
    pub materialization_state: WorkspaceMaterializationStateV1,
    pub module_graph_digest: String,
    pub selector_generation_digest: String,
    pub tombstone_digest: String,
    pub package_graph_digest: String,
    pub envelope_digest: String,
    pub owner_coverage: WorkspaceOwnerCoverageV1,
    pub provider_digest: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum WorkspaceGenerationEvidenceErrorV1 {
    SchemaMismatch,
    DigestAlgorithmMismatch,
    InvalidDigest { field: &'static str },
    InvalidRootShape,
    PartialArtifactCoverage,
    GenerationDigestMismatch,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WorkspaceGenerationColdReasonV1 {
    MissingGeneration,
    PackageGraphDrift,
    IncompleteOwnerCoverage,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WorkspaceGenerationRouteV1 {
    ResidentMemory,
    ArtifactComplete,
    ColdRebuild(WorkspaceGenerationColdReasonV1),
}

impl WorkspaceGenerationEvidenceV1 {
    pub fn from_components(
        components: WorkspaceGenerationComponentsV1,
    ) -> Result<Self, WorkspaceGenerationEvidenceErrorV1> {
        let generation_digest = derive_workspace_generation_digest_v1(&components)?;
        let evidence = Self {
            schema_id: WORKSPACE_GENERATION_EVIDENCE_SCHEMA_ID.to_owned(),
            digest_algorithm: WORKSPACE_GENERATION_DIGEST_ALGORITHM.to_owned(),
            generation_digest,
            source_root_digest: components.source_root_digest,
            root_depth: components.root_depth,
            leaf_count: components.leaf_count,
            materialization_state: components.materialization_state,
            module_graph_digest: components.module_graph_digest,
            selector_generation_digest: components.selector_generation_digest,
            tombstone_digest: components.tombstone_digest,
            package_graph_digest: components.package_graph_digest,
            envelope_digest: components.envelope_digest,
            owner_coverage: components.owner_coverage,
            provider_digest: components.provider_digest,
        };
        evidence.validate()?;
        Ok(evidence)
    }

    pub fn validate(&self) -> Result<(), WorkspaceGenerationEvidenceErrorV1> {
        if self.schema_id != WORKSPACE_GENERATION_EVIDENCE_SCHEMA_ID {
            return Err(WorkspaceGenerationEvidenceErrorV1::SchemaMismatch);
        }
        if self.digest_algorithm != WORKSPACE_GENERATION_DIGEST_ALGORITHM {
            return Err(WorkspaceGenerationEvidenceErrorV1::DigestAlgorithmMismatch);
        }
        for (field, digest) in [
            ("generationDigest", self.generation_digest.as_str()),
            ("sourceRootDigest", self.source_root_digest.as_str()),
            ("moduleGraphDigest", self.module_graph_digest.as_str()),
            (
                "selectorGenerationDigest",
                self.selector_generation_digest.as_str(),
            ),
            ("tombstoneDigest", self.tombstone_digest.as_str()),
            ("packageGraphDigest", self.package_graph_digest.as_str()),
            ("envelopeDigest", self.envelope_digest.as_str()),
            ("providerDigest", self.provider_digest.as_str()),
        ] {
            validate_digest(field, digest)?;
        }
        if (self.leaf_count <= 1 && self.root_depth != 0)
            || (self.leaf_count > 1 && self.root_depth == 0)
        {
            return Err(WorkspaceGenerationEvidenceErrorV1::InvalidRootShape);
        }
        if self.materialization_state == WorkspaceMaterializationStateV1::ArtifactComplete
            && self.owner_coverage != WorkspaceOwnerCoverageV1::Complete
        {
            return Err(WorkspaceGenerationEvidenceErrorV1::PartialArtifactCoverage);
        }
        let expected = derive_workspace_generation_digest_v1(&WorkspaceGenerationComponentsV1 {
            source_root_digest: self.source_root_digest.clone(),
            root_depth: self.root_depth,
            leaf_count: self.leaf_count,
            materialization_state: self.materialization_state,
            module_graph_digest: self.module_graph_digest.clone(),
            selector_generation_digest: self.selector_generation_digest.clone(),
            tombstone_digest: self.tombstone_digest.clone(),
            package_graph_digest: self.package_graph_digest.clone(),
            envelope_digest: self.envelope_digest.clone(),
            owner_coverage: self.owner_coverage,
            provider_digest: self.provider_digest.clone(),
        })?;
        if self.generation_digest != expected {
            return Err(WorkspaceGenerationEvidenceErrorV1::GenerationDigestMismatch);
        }
        Ok(())
    }

    pub fn package_graph_matches(&self, live_package_graph_digest: &str) -> bool {
        self.package_graph_digest == live_package_graph_digest
    }

    pub fn is_memory_authority(&self) -> bool {
        self.materialization_state == WorkspaceMaterializationStateV1::ResidentMemory
    }

    pub fn has_complete_relocation_coverage(&self) -> bool {
        self.owner_coverage == WorkspaceOwnerCoverageV1::Complete
    }
}

pub fn route_workspace_generation_v1(
    evidence: Option<&WorkspaceGenerationEvidenceV1>,
    live_package_graph_digest: &str,
) -> Result<WorkspaceGenerationRouteV1, WorkspaceGenerationEvidenceErrorV1> {
    let Some(evidence) = evidence else {
        return Ok(WorkspaceGenerationRouteV1::ColdRebuild(
            WorkspaceGenerationColdReasonV1::MissingGeneration,
        ));
    };
    evidence.validate()?;
    validate_digest("livePackageGraphDigest", live_package_graph_digest)?;
    if !evidence.package_graph_matches(live_package_graph_digest) {
        return Ok(WorkspaceGenerationRouteV1::ColdRebuild(
            WorkspaceGenerationColdReasonV1::PackageGraphDrift,
        ));
    }
    match evidence.materialization_state {
        WorkspaceMaterializationStateV1::ResidentMemory => {
            Ok(WorkspaceGenerationRouteV1::ResidentMemory)
        }
        WorkspaceMaterializationStateV1::ArtifactComplete
            if evidence.has_complete_relocation_coverage() =>
        {
            Ok(WorkspaceGenerationRouteV1::ArtifactComplete)
        }
        WorkspaceMaterializationStateV1::ArtifactComplete => {
            Ok(WorkspaceGenerationRouteV1::ColdRebuild(
                WorkspaceGenerationColdReasonV1::IncompleteOwnerCoverage,
            ))
        }
    }
}

pub fn derive_workspace_generation_digest_v1(
    components: &WorkspaceGenerationComponentsV1,
) -> Result<String, WorkspaceGenerationEvidenceErrorV1> {
    for (field, digest) in [
        ("sourceRootDigest", components.source_root_digest.as_str()),
        ("moduleGraphDigest", components.module_graph_digest.as_str()),
        (
            "selectorGenerationDigest",
            components.selector_generation_digest.as_str(),
        ),
        ("tombstoneDigest", components.tombstone_digest.as_str()),
        (
            "packageGraphDigest",
            components.package_graph_digest.as_str(),
        ),
        ("envelopeDigest", components.envelope_digest.as_str()),
        ("providerDigest", components.provider_digest.as_str()),
    ] {
        validate_digest(field, digest)?;
    }
    let canonical = format!(
        "schema={}\nalgorithm={}\nsourceRoot={}\nrootDepth={}\nleafCount={}\nmaterialization={:?}\nmoduleGraph={}\nselectors={}\ntombstones={}\npackageGraph={}\nenvelope={}\ncoverage={:?}\nprovider={}\n",
        WORKSPACE_GENERATION_EVIDENCE_SCHEMA_ID,
        WORKSPACE_GENERATION_DIGEST_ALGORITHM,
        components.source_root_digest,
        components.root_depth,
        components.leaf_count,
        components.materialization_state,
        components.module_graph_digest,
        components.selector_generation_digest,
        components.tombstone_digest,
        components.package_graph_digest,
        components.envelope_digest,
        components.owner_coverage,
        components.provider_digest,
    );
    Ok(blake3_content_digest_v1(canonical.as_bytes())
        .as_str()
        .to_owned())
}

fn validate_digest(
    field: &'static str,
    digest: &str,
) -> Result<(), WorkspaceGenerationEvidenceErrorV1> {
    if digest.len() == 64
        && digest
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        Ok(())
    } else {
        Err(WorkspaceGenerationEvidenceErrorV1::InvalidDigest { field })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn digest(byte: char) -> String {
        std::iter::repeat_n(byte, 64).collect()
    }

    fn components() -> WorkspaceGenerationComponentsV1 {
        WorkspaceGenerationComponentsV1 {
            source_root_digest: digest('1'),
            root_depth: 2,
            leaf_count: 4,
            materialization_state: WorkspaceMaterializationStateV1::ArtifactComplete,
            module_graph_digest: digest('2'),
            selector_generation_digest: digest('3'),
            tombstone_digest: digest('4'),
            package_graph_digest: digest('5'),
            envelope_digest: digest('6'),
            owner_coverage: WorkspaceOwnerCoverageV1::Complete,
            provider_digest: digest('7'),
        }
    }

    #[test]
    fn generation_digest_binds_every_atomic_component() {
        let evidence = WorkspaceGenerationEvidenceV1::from_components(components()).unwrap();
        evidence.validate().unwrap();

        let mut drifted = evidence.clone();
        drifted.package_graph_digest = digest('8');
        assert_eq!(
            drifted.validate(),
            Err(WorkspaceGenerationEvidenceErrorV1::GenerationDigestMismatch)
        );
    }

    #[test]
    fn artifact_generation_rejects_partial_owner_coverage() {
        let mut partial = components();
        partial.owner_coverage = WorkspaceOwnerCoverageV1::Partial;
        assert_eq!(
            WorkspaceGenerationEvidenceV1::from_components(partial),
            Err(WorkspaceGenerationEvidenceErrorV1::PartialArtifactCoverage)
        );
    }

    #[test]
    fn zero_depth_memory_generation_is_explicit() {
        let mut memory = components();
        memory.root_depth = 0;
        memory.leaf_count = 0;
        memory.materialization_state = WorkspaceMaterializationStateV1::ResidentMemory;
        memory.owner_coverage = WorkspaceOwnerCoverageV1::Partial;
        let evidence = WorkspaceGenerationEvidenceV1::from_components(memory).unwrap();
        assert!(evidence.is_memory_authority());
        assert!(!evidence.has_complete_relocation_coverage());
    }

    #[test]
    fn package_drift_is_a_terminal_cold_route() {
        let evidence = WorkspaceGenerationEvidenceV1::from_components(components()).unwrap();
        assert_eq!(
            route_workspace_generation_v1(Some(&evidence), &digest('8')).unwrap(),
            WorkspaceGenerationRouteV1::ColdRebuild(
                WorkspaceGenerationColdReasonV1::PackageGraphDrift
            )
        );
    }

    #[test]
    fn matching_complete_generation_routes_without_search_refinement() {
        let evidence = WorkspaceGenerationEvidenceV1::from_components(components()).unwrap();
        assert_eq!(
            route_workspace_generation_v1(Some(&evidence), evidence.package_graph_digest.as_str())
                .unwrap(),
            WorkspaceGenerationRouteV1::ArtifactComplete
        );
    }

    #[test]
    fn absent_generation_is_a_terminal_cold_route() {
        assert_eq!(
            route_workspace_generation_v1(None, &digest('1')).unwrap(),
            WorkspaceGenerationRouteV1::ColdRebuild(
                WorkspaceGenerationColdReasonV1::MissingGeneration
            )
        );
    }
}
