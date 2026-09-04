use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;

use agent_semantic_content_identity::exact_selector_generation_fixture::ExactSelectorGenerationFixtureViewV1;
use agent_semantic_content_identity::exact_selector_generation_fixture::fixture_digest_v1;

use crate::LoadOnceGenerationV1;

#[derive(Debug, Clone)]
pub struct ExactSelectorFixtureArtifactV1 {
    pub fixture_bytes: Arc<[u8]>,
    pub generation_digest: [u8; 32],
    pub fixture_digest: [u8; 32],
}

#[derive(Debug, Clone)]
pub struct ExactSelectorFixtureProjectionV1 {
    fixture_bytes: Arc<[u8]>,
    projection_range: std::ops::Range<usize>,
    language_id: String,
    provider_id: String,
    workspace_root_digest: [u8; 32],
    workspace_identity_digest: [u8; 32],
    parser_identity_digest: [u8; 32],
    query_pack_digest: [u8; 32],
    generation_digest: [u8; 32],
    owner_path: String,
    source_blob_digest: [u8; 32],
    owner_subtree_digest: [u8; 32],
    normalized_parser_facts_digest: [u8; 32],
    projection_mode: agent_semantic_content_identity::ExactSelectorProjectionModeV1,
}

impl ExactSelectorFixtureProjectionV1 {
    pub fn as_bytes(&self) -> &[u8] {
        &self.fixture_bytes[self.projection_range.clone()]
    }

    pub fn owner_path(&self) -> &str {
        &self.owner_path
    }

    pub fn matches_generation_authority_v1(
        &self,
        language_id: &str,
        provider_id: &str,
        parser_identity_digest: &str,
        query_pack_digest: &str,
    ) -> bool {
        self.language_id == language_id
            && self.provider_id == provider_id
            && digest_matches_v1(parser_identity_digest, &self.parser_identity_digest)
            && digest_matches_v1(query_pack_digest, &self.query_pack_digest)
    }

    pub const fn workspace_root_digest(&self) -> &[u8; 32] {
        &self.workspace_root_digest
    }

    pub const fn workspace_identity_digest(&self) -> &[u8; 32] {
        &self.workspace_identity_digest
    }

    pub const fn generation_digest(&self) -> &[u8; 32] {
        &self.generation_digest
    }

    pub const fn source_blob_digest(&self) -> &[u8; 32] {
        &self.source_blob_digest
    }

    pub const fn owner_subtree_digest(&self) -> &[u8; 32] {
        &self.owner_subtree_digest
    }

    pub const fn normalized_parser_facts_digest(&self) -> &[u8; 32] {
        &self.normalized_parser_facts_digest
    }

    pub const fn projection_mode(
        &self,
    ) -> agent_semantic_content_identity::ExactSelectorProjectionModeV1 {
        self.projection_mode
    }

    pub fn matches_live_owner_v1(
        &self,
        owner_path: &str,
        source_bytes: &[u8],
        projection_mode: agent_semantic_content_identity::ExactSelectorProjectionModeV1,
    ) -> bool {
        self.owner_path == owner_path
            && self.source_blob_digest == *blake3::hash(source_bytes).as_bytes()
            && self.projection_mode == projection_mode
    }
}

pub trait ExactSelectorFixtureBackendV1: Send + Sync {
    fn load_fixture(&self) -> Result<ExactSelectorFixtureArtifactV1, String>;
}

#[derive(Debug)]
pub struct ExactSelectorFixtureResidentV1<B> {
    backend: B,
    fixture: LoadOnceGenerationV1<ExactSelectorFixtureArtifactV1>,
}

impl<B> ExactSelectorFixtureResidentV1<B>
where
    B: ExactSelectorFixtureBackendV1,
{
    pub const fn new(backend: B) -> Self {
        Self {
            backend,
            fixture: LoadOnceGenerationV1::new(),
        }
    }

    pub fn resolve(
        &self,
        structural_selector: &str,
    ) -> Result<Option<ExactSelectorFixtureProjectionV1>, String> {
        let fixture = self
            .fixture
            .get_or_try_init(|| self.backend.load_fixture())?;
        let projection = exact_selector_fixture_projection_metadata_v1(
            &fixture.fixture_bytes,
            &fixture.generation_digest,
            &fixture.fixture_digest,
            structural_selector,
        )?;
        Ok(
            projection.map(|projection| ExactSelectorFixtureProjectionV1 {
                fixture_bytes: Arc::clone(&fixture.fixture_bytes),
                projection_range: projection.projection_range,
                language_id: projection.language_id,
                provider_id: projection.provider_id,
                workspace_root_digest: projection.workspace_root_digest,
                workspace_identity_digest: projection.workspace_identity_digest,
                parser_identity_digest: projection.parser_identity_digest,
                query_pack_digest: projection.query_pack_digest,
                generation_digest: projection.generation_digest,
                owner_path: projection.owner_path,
                source_blob_digest: projection.source_blob_digest,
                owner_subtree_digest: projection.owner_subtree_digest,
                normalized_parser_facts_digest: projection.normalized_parser_facts_digest,
                projection_mode: projection.projection_mode,
            }),
        )
    }

    pub fn is_initialized(&self) -> bool {
        self.fixture.is_initialized()
    }
}

#[derive(Debug, Clone)]
pub struct ExactSelectorFixtureFileBackendV1 {
    fixture_path: PathBuf,
    expected_artifact_digest: String,
    expected_generation_digest: [u8; 32],
}

impl ExactSelectorFixtureFileBackendV1 {
    pub fn new(
        fixture_path: impl Into<PathBuf>,
        expected_artifact_digest: impl Into<String>,
        expected_generation_digest: [u8; 32],
    ) -> Self {
        Self {
            fixture_path: fixture_path.into(),
            expected_artifact_digest: expected_artifact_digest.into(),
            expected_generation_digest,
        }
    }

    pub fn fixture_path(&self) -> &Path {
        &self.fixture_path
    }
}

impl ExactSelectorFixtureBackendV1 for ExactSelectorFixtureFileBackendV1 {
    fn load_fixture(&self) -> Result<ExactSelectorFixtureArtifactV1, String> {
        let fixture_bytes = std::fs::read(&self.fixture_path).map_err(|error| {
            format!(
                "failed to read exact-selector fixture {}: {error}",
                self.fixture_path.display()
            )
        })?;
        let actual_artifact_digest = blake3::hash(&fixture_bytes).to_hex().to_string();
        if actual_artifact_digest != self.expected_artifact_digest {
            return Err(format!(
                "exact-selector fixture artifact digest mismatch: expected={} actual={} path={}",
                self.expected_artifact_digest,
                actual_artifact_digest,
                self.fixture_path.display()
            ));
        }
        let fixture_digest = *fixture_digest_v1(&fixture_bytes)
            .map_err(|error| format!("invalid exact-selector fixture: {error}"))?;
        Ok(ExactSelectorFixtureArtifactV1 {
            fixture_bytes: fixture_bytes.into(),
            generation_digest: self.expected_generation_digest,
            fixture_digest,
        })
    }
}

pub fn exact_selector_fixture_lookup_v1(
    fixture_bytes: &[u8],
    expected_generation_digest: &[u8; 32],
    expected_fixture_digest: &[u8; 32],
    structural_selector: &str,
) -> Result<bool, String> {
    let fixture = ExactSelectorGenerationFixtureViewV1::attach(
        fixture_bytes,
        expected_generation_digest,
        expected_fixture_digest,
    )
    .map_err(|error| error.to_string())?;
    Ok(fixture
        .lookup(structural_selector)
        .map_err(|error| error.to_string())?
        .is_some())
}

pub fn exact_selector_fixture_projection_v1(
    fixture_bytes: &[u8],
    expected_generation_digest: &[u8; 32],
    expected_fixture_digest: &[u8; 32],
    structural_selector: &str,
) -> Result<Option<Vec<u8>>, String> {
    let projection_range = exact_selector_fixture_projection_range_v1(
        fixture_bytes,
        expected_generation_digest,
        expected_fixture_digest,
        structural_selector,
    )?;
    Ok(projection_range.map(|range| fixture_bytes[range].to_vec()))
}

pub fn exact_selector_fixture_projection_range_v1(
    fixture_bytes: &[u8],
    expected_generation_digest: &[u8; 32],
    expected_fixture_digest: &[u8; 32],
    structural_selector: &str,
) -> Result<Option<std::ops::Range<usize>>, String> {
    Ok(exact_selector_fixture_projection_metadata_v1(
        fixture_bytes,
        expected_generation_digest,
        expected_fixture_digest,
        structural_selector,
    )?
    .map(|projection| projection.projection_range))
}

#[derive(Debug)]
struct ExactSelectorFixtureProjectionMetadataV1 {
    projection_range: std::ops::Range<usize>,
    language_id: String,
    provider_id: String,
    workspace_root_digest: [u8; 32],
    workspace_identity_digest: [u8; 32],
    parser_identity_digest: [u8; 32],
    query_pack_digest: [u8; 32],
    generation_digest: [u8; 32],
    owner_path: String,
    source_blob_digest: [u8; 32],
    owner_subtree_digest: [u8; 32],
    normalized_parser_facts_digest: [u8; 32],
    projection_mode: agent_semantic_content_identity::ExactSelectorProjectionModeV1,
}

fn exact_selector_fixture_projection_metadata_v1(
    fixture_bytes: &[u8],
    expected_generation_digest: &[u8; 32],
    expected_fixture_digest: &[u8; 32],
    structural_selector: &str,
) -> Result<Option<ExactSelectorFixtureProjectionMetadataV1>, String> {
    let fixture = ExactSelectorGenerationFixtureViewV1::attach(
        fixture_bytes,
        expected_generation_digest,
        expected_fixture_digest,
    )
    .map_err(|error| error.to_string())?;
    let record = fixture
        .lookup(structural_selector)
        .map_err(|error| error.to_string())?;
    record
        .map(|record| {
            let fixture_start = fixture_bytes.as_ptr() as usize;
            let projection_start = record.projection.as_ptr() as usize;
            let start = projection_start
                .checked_sub(fixture_start)
                .ok_or_else(|| "fixture projection starts before fixture bytes".to_owned())?;
            let end = start
                .checked_add(record.projection.len())
                .ok_or_else(|| "fixture projection range overflow".to_owned())?;
            if end > fixture_bytes.len() {
                return Err("fixture projection ends after fixture bytes".to_owned());
            }
            Ok(ExactSelectorFixtureProjectionMetadataV1 {
                projection_range: start..end,
                language_id: fixture.language_id().to_owned(),
                provider_id: fixture.provider_id().to_owned(),
                workspace_root_digest: *fixture.workspace_root_digest(),
                workspace_identity_digest: *fixture.workspace_identity_digest(),
                parser_identity_digest: *fixture.parser_identity_digest(),
                query_pack_digest: *fixture.query_pack_digest(),
                generation_digest: *fixture.generation_digest(),
                owner_path: record.owner_path.to_owned(),
                source_blob_digest: *record.source_blob_digest,
                owner_subtree_digest: *record.owner_subtree_digest,
                normalized_parser_facts_digest: *record.normalized_parser_facts_digest,
                projection_mode: record.projection_mode,
            })
        })
        .transpose()
}

fn digest_matches_v1(expected: &str, actual: &[u8; 32]) -> bool {
    blake3::Hash::from_hex(expected).is_ok_and(|digest| digest.as_bytes() == actual)
}
