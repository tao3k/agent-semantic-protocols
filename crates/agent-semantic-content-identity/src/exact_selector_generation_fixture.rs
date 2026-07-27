use std::cmp::Ordering;
use std::fmt;
use std::ops::Range;

pub const EXACT_SELECTOR_GENERATION_FIXTURE_SCHEMA_ID: &str =
    "agent.semantic-protocols.exact-selector-generation-fixture";
pub const EXACT_SELECTOR_GENERATION_FIXTURE_SCHEMA_VERSION: &str = "1";

const MAGIC: &[u8; 8] = b"ASPXGFV1";
const DIGEST_LEN: usize = 32;
const FIXED_HEADER_LEN: usize = 8 + 4 + 4 + 4 + 4 + 2 + 2 + (DIGEST_LEN * 6);
const FIXTURE_DIGEST_OFFSET: usize = FIXED_HEADER_LEN - DIGEST_LEN;
const RECORD_HEADER_LEN: usize = 4 + 4 + 4 + 1 + 8 + 8 + (DIGEST_LEN * 3);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum ExactSelectorProjectionModeV1 {
    Code = 1,
    Names = 2,
    Verbatim = 3,
    Skeleton = 4,
}

impl ExactSelectorProjectionModeV1 {
    fn from_byte(value: u8) -> Result<Self, ExactSelectorGenerationFixtureErrorV1> {
        match value {
            1 => Ok(Self::Code),
            2 => Ok(Self::Names),
            3 => Ok(Self::Verbatim),
            4 => Ok(Self::Skeleton),
            _ => Err(ExactSelectorGenerationFixtureErrorV1::InvalidProjectionMode(value)),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExactSelectorGenerationIdentityV1 {
    pub language_id: String,
    pub provider_id: String,
    pub workspace_root_digest: [u8; DIGEST_LEN],
    pub workspace_identity_digest: [u8; DIGEST_LEN],
    pub parser_identity_digest: [u8; DIGEST_LEN],
    pub query_pack_digest: [u8; DIGEST_LEN],
    pub generation_digest: [u8; DIGEST_LEN],
    pub selector_count: u32,
    pub owner_count: u32,
    pub leaf_count: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExactSelectorMerkleProofSideV1 {
    Left,
    Right,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExactSelectorMerkleProofStepV1 {
    pub side: ExactSelectorMerkleProofSideV1,
    pub digest: [u8; DIGEST_LEN],
}

/// Complete, parser-produced proof for one exact-selector materialization.
///
/// This is the sole in-process representation of the composed
/// `exact-selector-materialization-proof.v1` contract. Consumers must not
/// reconstruct any field from line numbers or by rereading source.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExactSelectorMaterializationProofV1 {
    pub language_id: String,
    pub provider_id: String,
    pub canonical_item_selector: crate::CanonicalItemSelectorV1,
    pub parser_identity_digest: [u8; DIGEST_LEN],
    pub query_pack_digest: [u8; DIGEST_LEN],
    pub workspace_root_digest: [u8; DIGEST_LEN],
    pub owner_path: String,
    pub owner_subtree_digest: [u8; DIGEST_LEN],
    pub owner_inclusion_proof: Vec<ExactSelectorMerkleProofStepV1>,
    pub source_blob_digest: [u8; DIGEST_LEN],
    pub normalized_parser_facts_digest: [u8; DIGEST_LEN],
    pub structural_selector: String,
    pub projection_mode: ExactSelectorProjectionModeV1,
    pub source_byte_start: u64,
    pub source_byte_end: u64,
    pub projection_digest: [u8; DIGEST_LEN],
    pub projection: Vec<u8>,
}

#[derive(serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ExactSelectorMaterializationProofWireV1 {
    schema_id: String,
    schema_version: String,
    projection_packet: ExactSelectorProjectionPacketWireV1,
    merkle_proof: ExactSelectorMerkleProofWireV1,
    source_byte_start: u64,
    source_byte_end: u64,
}

#[derive(serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ExactSelectorProjectionPacketWireV1 {
    schema_id: String,
    schema_version: String,
    digest_algorithm: String,
    language_id: String,
    provider_id: String,
    canonical_item_selector: crate::CanonicalItemSelectorV1,
    parser_identity_digest: String,
    query_pack_digest: String,
    owner_path: String,
    source_blob_digest: String,
    parser_fact_digest: String,
    structural_selector: String,
    projection_mode: String,
    projection_encoding: String,
    projection_payload_base64: String,
}

#[derive(serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ExactSelectorMerkleProofWireV1 {
    schema_id: String,
    schema_version: String,
    digest_algorithm: String,
    language_id: String,
    workspace_root_digest: String,
    owner_path: String,
    owner_subtree_digest: String,
    owner_inclusion_proof: Vec<ExactSelectorMerkleProofStepWireV1>,
    source_blob_digest: String,
    parser_identity_digest: String,
    query_pack_digest: String,
    parser_fact_digest: String,
    canonical_item_selector: crate::CanonicalItemSelectorV1,
    structural_selector: String,
    projection_mode: String,
    projection_digest: String,
}

#[derive(serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ExactSelectorMerkleProofStepWireV1 {
    side: String,
    digest: String,
}

fn decode_digest_v1<E: serde::de::Error>(value: &str) -> Result<[u8; DIGEST_LEN], E> {
    if value.len() != DIGEST_LEN * 2 {
        return Err(E::custom(
            "exact selector digest must contain 64 lowercase hex bytes",
        ));
    }
    let mut digest = [0_u8; DIGEST_LEN];
    for (index, pair) in value.as_bytes().chunks_exact(2).enumerate() {
        let hex = |byte: u8| match byte {
            b'0'..=b'9' => Some(byte - b'0'),
            b'a'..=b'f' => Some(byte - b'a' + 10),
            _ => None,
        };
        let high =
            hex(pair[0]).ok_or_else(|| E::custom("exact selector digest must be lowercase hex"))?;
        let low =
            hex(pair[1]).ok_or_else(|| E::custom("exact selector digest must be lowercase hex"))?;
        digest[index] = (high << 4) | low;
    }
    Ok(digest)
}

fn encode_digest_v1(digest: &[u8; DIGEST_LEN]) -> String {
    use std::fmt::Write as _;
    digest.iter().fold(
        String::with_capacity(DIGEST_LEN * 2),
        |mut encoded, byte| {
            let _ = write!(encoded, "{byte:02x}");
            encoded
        },
    )
}

fn projection_mode_name_v1(mode: ExactSelectorProjectionModeV1) -> &'static str {
    match mode {
        ExactSelectorProjectionModeV1::Code => "code",
        ExactSelectorProjectionModeV1::Names => "names",
        ExactSelectorProjectionModeV1::Verbatim => "verbatim",
        ExactSelectorProjectionModeV1::Skeleton => "skeleton",
    }
}

fn projection_mode_v1<E: serde::de::Error>(
    value: &str,
) -> Result<ExactSelectorProjectionModeV1, E> {
    match value {
        "code" => Ok(ExactSelectorProjectionModeV1::Code),
        "names" => Ok(ExactSelectorProjectionModeV1::Names),
        "verbatim" => Ok(ExactSelectorProjectionModeV1::Verbatim),
        "skeleton" => Ok(ExactSelectorProjectionModeV1::Skeleton),
        _ => Err(E::custom("unknown exact selector projection mode")),
    }
}

impl<'de> serde::Deserialize<'de> for ExactSelectorMaterializationProofV1 {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        use base64::Engine as _;

        let wire = ExactSelectorMaterializationProofWireV1::deserialize(deserializer)?;
        let projection = wire.projection_packet;
        let merkle = wire.merkle_proof;
        if wire.schema_id != "agent.semantic-protocols.exact-selector-materialization-proof"
            || wire.schema_version != "1"
            || projection.schema_id != "agent.semantic-protocols.exact-selector-projection-packet"
            || projection.schema_version != "1"
            || merkle.schema_id != "agent.semantic-protocols.exact-selector-merkle-proof"
            || merkle.schema_version != "1"
            || projection.digest_algorithm != "blake3-256"
            || merkle.digest_algorithm != "blake3-256"
            || projection.projection_encoding != "base64"
            || projection.language_id != merkle.language_id
            || projection.owner_path != merkle.owner_path
            || projection.source_blob_digest != merkle.source_blob_digest
            || projection.parser_identity_digest != merkle.parser_identity_digest
            || projection.query_pack_digest != merkle.query_pack_digest
            || projection.parser_fact_digest != merkle.parser_fact_digest
            || projection.canonical_item_selector != merkle.canonical_item_selector
            || projection.structural_selector != merkle.structural_selector
            || projection.projection_mode != merkle.projection_mode
        {
            return Err(serde::de::Error::custom(
                "exact selector materialization packet and Merkle proof disagree",
            ));
        }
        let proof = Self {
            language_id: projection.language_id,
            provider_id: projection.provider_id,
            canonical_item_selector: projection.canonical_item_selector,
            parser_identity_digest: decode_digest_v1::<D::Error>(
                &projection.parser_identity_digest,
            )?,
            query_pack_digest: decode_digest_v1::<D::Error>(&projection.query_pack_digest)?,
            workspace_root_digest: decode_digest_v1::<D::Error>(&merkle.workspace_root_digest)?,
            owner_path: projection.owner_path,
            owner_subtree_digest: decode_digest_v1::<D::Error>(&merkle.owner_subtree_digest)?,
            owner_inclusion_proof: merkle
                .owner_inclusion_proof
                .into_iter()
                .map(|step| {
                    Ok(ExactSelectorMerkleProofStepV1 {
                        side: match step.side.as_str() {
                            "left" => ExactSelectorMerkleProofSideV1::Left,
                            "right" => ExactSelectorMerkleProofSideV1::Right,
                            _ => {
                                return Err(<D::Error as serde::de::Error>::custom(
                                    "unknown exact selector Merkle proof side",
                                ));
                            }
                        },
                        digest: decode_digest_v1::<D::Error>(&step.digest)?,
                    })
                })
                .collect::<Result<Vec<_>, D::Error>>()?,
            source_blob_digest: decode_digest_v1::<D::Error>(&projection.source_blob_digest)?,
            normalized_parser_facts_digest: decode_digest_v1::<D::Error>(
                &projection.parser_fact_digest,
            )?,
            structural_selector: projection.structural_selector,
            projection_mode: projection_mode_v1::<D::Error>(&projection.projection_mode)?,
            source_byte_start: wire.source_byte_start,
            source_byte_end: wire.source_byte_end,
            projection_digest: decode_digest_v1::<D::Error>(&merkle.projection_digest)?,
            projection: base64::engine::general_purpose::STANDARD
                .decode(&projection.projection_payload_base64)
                .map_err(<D::Error as serde::de::Error>::custom)?,
        };
        ExactSelectorGenerationRecordV1::try_from(&proof)
            .map_err(<D::Error as serde::de::Error>::custom)?;
        Ok(proof)
    }
}

impl serde::Serialize for ExactSelectorMaterializationProofV1 {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use base64::Engine as _;

        let projection_mode = projection_mode_name_v1(self.projection_mode).to_string();
        ExactSelectorMaterializationProofWireV1 {
            schema_id: "agent.semantic-protocols.exact-selector-materialization-proof".to_string(),
            schema_version: "1".to_string(),
            projection_packet: ExactSelectorProjectionPacketWireV1 {
                schema_id: "agent.semantic-protocols.exact-selector-projection-packet".to_string(),
                schema_version: "1".to_string(),
                digest_algorithm: "blake3-256".to_string(),
                language_id: self.language_id.clone(),
                provider_id: self.provider_id.clone(),
                canonical_item_selector: self.canonical_item_selector.clone(),
                parser_identity_digest: encode_digest_v1(&self.parser_identity_digest),
                query_pack_digest: encode_digest_v1(&self.query_pack_digest),
                owner_path: self.owner_path.clone(),
                source_blob_digest: encode_digest_v1(&self.source_blob_digest),
                parser_fact_digest: encode_digest_v1(&self.normalized_parser_facts_digest),
                structural_selector: self.structural_selector.clone(),
                projection_mode: projection_mode.clone(),
                projection_encoding: "base64".to_string(),
                projection_payload_base64: base64::engine::general_purpose::STANDARD
                    .encode(&self.projection),
            },
            merkle_proof: ExactSelectorMerkleProofWireV1 {
                schema_id: "agent.semantic-protocols.exact-selector-merkle-proof".to_string(),
                schema_version: "1".to_string(),
                digest_algorithm: "blake3-256".to_string(),
                language_id: self.language_id.clone(),
                workspace_root_digest: encode_digest_v1(&self.workspace_root_digest),
                owner_path: self.owner_path.clone(),
                owner_subtree_digest: encode_digest_v1(&self.owner_subtree_digest),
                owner_inclusion_proof: self
                    .owner_inclusion_proof
                    .iter()
                    .map(|step| ExactSelectorMerkleProofStepWireV1 {
                        side: match step.side {
                            ExactSelectorMerkleProofSideV1::Left => "left",
                            ExactSelectorMerkleProofSideV1::Right => "right",
                        }
                        .to_string(),
                        digest: encode_digest_v1(&step.digest),
                    })
                    .collect(),
                source_blob_digest: encode_digest_v1(&self.source_blob_digest),
                parser_identity_digest: encode_digest_v1(&self.parser_identity_digest),
                query_pack_digest: encode_digest_v1(&self.query_pack_digest),
                parser_fact_digest: encode_digest_v1(&self.normalized_parser_facts_digest),
                canonical_item_selector: self.canonical_item_selector.clone(),
                structural_selector: self.structural_selector.clone(),
                projection_mode,
                projection_digest: encode_digest_v1(&self.projection_digest),
            },
            source_byte_start: self.source_byte_start,
            source_byte_end: self.source_byte_end,
        }
        .serialize(serializer)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExactSelectorMaterializationProofErrorV1 {
    EmptyIdentity,
    InvalidCanonicalSelector,
    InvalidOwnerPath,
    InvalidDigest,
    InvalidSourceByteRange,
    EmptyProjection,
    ProjectionDigestMismatch,
    WorkspaceRootDigestMismatch,
}

impl std::fmt::Display for ExactSelectorMaterializationProofErrorV1 {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::EmptyIdentity => "exact selector materialization identity is empty",
            Self::InvalidCanonicalSelector => {
                "exact selector materialization canonical selector mismatch"
            }
            Self::InvalidOwnerPath => "exact selector materialization owner path is invalid",
            Self::InvalidDigest => "exact selector materialization digest is invalid",
            Self::InvalidSourceByteRange => {
                "exact selector materialization source byte range is invalid"
            }
            Self::EmptyProjection => "exact selector materialization projection is empty",
            Self::ProjectionDigestMismatch => {
                "exact selector materialization projection digest mismatch"
            }
            Self::WorkspaceRootDigestMismatch => {
                "exact selector materialization Merkle root mismatch"
            }
        })
    }
}

impl std::error::Error for ExactSelectorMaterializationProofErrorV1 {}

impl TryFrom<&ExactSelectorMaterializationProofV1> for ExactSelectorGenerationRecordV1 {
    type Error = ExactSelectorMaterializationProofErrorV1;

    fn try_from(proof: &ExactSelectorMaterializationProofV1) -> Result<Self, Self::Error> {
        if proof.language_id.is_empty()
            || proof.provider_id.is_empty()
            || proof.structural_selector.is_empty()
        {
            return Err(Self::Error::EmptyIdentity);
        }
        let canonical = &proof.canonical_item_selector;
        if canonical.structural_selector != proof.structural_selector {
            return Err(Self::Error::InvalidCanonicalSelector);
        }
        if proof.owner_path.is_empty()
            || proof.owner_path.starts_with('/')
            || proof
                .owner_path
                .split('/')
                .any(|component| component == "..")
        {
            return Err(Self::Error::InvalidOwnerPath);
        }
        if [
            proof.parser_identity_digest,
            proof.query_pack_digest,
            proof.workspace_root_digest,
            proof.owner_subtree_digest,
            proof.source_blob_digest,
            proof.normalized_parser_facts_digest,
            proof.projection_digest,
        ]
        .iter()
        .any(|digest| digest.iter().all(|byte| *byte == 0))
            || proof
                .owner_inclusion_proof
                .iter()
                .any(|step| step.digest.iter().all(|byte| *byte == 0))
        {
            return Err(Self::Error::InvalidDigest);
        }
        if proof.source_byte_start >= proof.source_byte_end {
            return Err(Self::Error::InvalidSourceByteRange);
        }
        if proof.projection.is_empty() {
            return Err(Self::Error::EmptyProjection);
        }
        if blake3::hash(&proof.projection).as_bytes() != &proof.projection_digest {
            return Err(Self::Error::ProjectionDigestMismatch);
        }
        let mut root = proof.owner_subtree_digest;
        for step in &proof.owner_inclusion_proof {
            let (left, right) = match step.side {
                ExactSelectorMerkleProofSideV1::Left => (step.digest, root),
                ExactSelectorMerkleProofSideV1::Right => (root, step.digest),
            };
            let mut hasher = blake3::Hasher::new();
            hasher.update(b"asp.exact-selector.owner-merkle-node.v1\0");
            hasher.update(&left);
            hasher.update(&right);
            root = *hasher.finalize().as_bytes();
        }
        if root != proof.workspace_root_digest {
            return Err(Self::Error::WorkspaceRootDigestMismatch);
        }
        Ok(Self {
            structural_selector: proof.structural_selector.clone(),
            owner_path: proof.owner_path.clone(),
            owner_subtree_digest: proof.owner_subtree_digest,
            source_blob_digest: proof.source_blob_digest,
            normalized_parser_facts_digest: proof.normalized_parser_facts_digest,
            projection_mode: proof.projection_mode,
            source_byte_range: proof.source_byte_start..proof.source_byte_end,
            projection: proof.projection.clone(),
        })
    }
}

impl TryFrom<ExactSelectorMaterializationProofV1> for ExactSelectorGenerationRecordV1 {
    type Error = ExactSelectorMaterializationProofErrorV1;

    fn try_from(proof: ExactSelectorMaterializationProofV1) -> Result<Self, Self::Error> {
        Self::try_from(&proof)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExactSelectorGenerationRecordV1 {
    pub structural_selector: String,
    pub owner_path: String,
    pub owner_subtree_digest: [u8; DIGEST_LEN],
    pub source_blob_digest: [u8; DIGEST_LEN],
    pub normalized_parser_facts_digest: [u8; DIGEST_LEN],
    pub projection_mode: ExactSelectorProjectionModeV1,
    pub source_byte_range: Range<u64>,
    pub projection: Vec<u8>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExactSelectorGenerationRecordViewV1<'a> {
    pub structural_selector: &'a str,
    pub owner_path: &'a str,
    pub owner_subtree_digest: &'a [u8; DIGEST_LEN],
    pub source_blob_digest: &'a [u8; DIGEST_LEN],
    pub normalized_parser_facts_digest: &'a [u8; DIGEST_LEN],
    pub projection_mode: ExactSelectorProjectionModeV1,
    pub source_byte_range: Range<u64>,
    pub projection: &'a [u8],
}

#[derive(Clone, Copy, Debug)]
pub struct ExactSelectorGenerationFixtureViewV1<'a> {
    bytes: &'a [u8],
    language_id: &'a str,
    provider_id: &'a str,
    workspace_root_digest: &'a [u8; DIGEST_LEN],
    workspace_identity_digest: &'a [u8; DIGEST_LEN],
    parser_identity_digest: &'a [u8; DIGEST_LEN],
    query_pack_digest: &'a [u8; DIGEST_LEN],
    generation_digest: &'a [u8; DIGEST_LEN],
    fixture_digest: &'a [u8; DIGEST_LEN],
    record_count: u32,
    selector_count: u32,
    owner_count: u32,
    leaf_count: u32,
    offsets_start: usize,
    records_start: usize,
}

impl<'a> ExactSelectorGenerationFixtureViewV1<'a> {
    pub fn attach(
        bytes: &'a [u8],
        expected_generation_digest: &[u8; DIGEST_LEN],
        expected_fixture_digest: &[u8; DIGEST_LEN],
    ) -> Result<Self, ExactSelectorGenerationFixtureErrorV1> {
        if bytes.len() < FIXED_HEADER_LEN || bytes.get(..MAGIC.len()) != Some(MAGIC) {
            return Err(ExactSelectorGenerationFixtureErrorV1::InvalidHeader);
        }
        let record_count = read_u32(bytes, 8)?;
        let owner_count = read_u32(bytes, 12)?;
        let leaf_count = read_u32(bytes, 16)?;
        let selector_count = read_u32(bytes, 20)?;
        if record_count == 0
            || record_count != selector_count
            || owner_count == 0
            || owner_count != leaf_count
        {
            return Err(
                ExactSelectorGenerationFixtureErrorV1::IncompleteGeneration {
                    record_count,
                    selector_count,
                    owner_count,
                    leaf_count,
                },
            );
        }
        let language_len = usize::from(read_u16(bytes, 24)?);
        let provider_len = usize::from(read_u16(bytes, 26)?);
        let workspace_root_digest = digest_at(bytes, 28)?;
        let workspace_identity_digest = digest_at(bytes, 60)?;
        let parser_identity_digest = digest_at(bytes, 92)?;
        let query_pack_digest = digest_at(bytes, 124)?;
        let generation_digest = digest_at(bytes, 156)?;
        let fixture_digest = digest_at(bytes, FIXTURE_DIGEST_OFFSET)?;
        if generation_digest != expected_generation_digest {
            return Err(ExactSelectorGenerationFixtureErrorV1::GenerationMismatch);
        }
        if fixture_digest != expected_fixture_digest {
            return Err(ExactSelectorGenerationFixtureErrorV1::FixtureMismatch);
        }
        let language_start = FIXED_HEADER_LEN;
        let provider_start = language_start
            .checked_add(language_len)
            .ok_or(ExactSelectorGenerationFixtureErrorV1::InvalidHeader)?;
        let offsets_start = provider_start
            .checked_add(provider_len)
            .ok_or(ExactSelectorGenerationFixtureErrorV1::InvalidHeader)?;
        let offsets_len = usize::try_from(record_count)
            .ok()
            .and_then(|count| count.checked_add(1))
            .and_then(|count| count.checked_mul(8))
            .ok_or(ExactSelectorGenerationFixtureErrorV1::InvalidHeader)?;
        let records_start = offsets_start
            .checked_add(offsets_len)
            .ok_or(ExactSelectorGenerationFixtureErrorV1::InvalidHeader)?;
        if records_start > bytes.len() {
            return Err(ExactSelectorGenerationFixtureErrorV1::InvalidHeader);
        }
        let final_offset = read_u64(
            bytes,
            offsets_start
                .checked_add(
                    usize::try_from(record_count)
                        .map_err(|_| ExactSelectorGenerationFixtureErrorV1::InvalidHeader)?
                        .checked_mul(8)
                        .ok_or(ExactSelectorGenerationFixtureErrorV1::InvalidHeader)?,
                )
                .ok_or(ExactSelectorGenerationFixtureErrorV1::InvalidHeader)?,
        )?;
        let records_len = u64::try_from(bytes.len() - records_start)
            .map_err(|_| ExactSelectorGenerationFixtureErrorV1::InvalidHeader)?;
        if final_offset != records_len {
            return Err(ExactSelectorGenerationFixtureErrorV1::InvalidOffset);
        }
        let language_id = utf8_slice(bytes, language_start, provider_start)?;
        let provider_end = offsets_start;
        let provider_id = utf8_slice(bytes, provider_start, provider_end)?;
        if language_id.is_empty() || provider_id.is_empty() {
            return Err(ExactSelectorGenerationFixtureErrorV1::InvalidHeader);
        }
        Ok(Self {
            bytes,
            language_id,
            provider_id,
            workspace_root_digest,
            workspace_identity_digest,
            parser_identity_digest,
            query_pack_digest,
            generation_digest,
            fixture_digest,
            record_count,
            selector_count,
            owner_count,
            leaf_count,
            offsets_start,
            records_start,
        })
    }

    pub fn language_id(&self) -> &'a str {
        self.language_id
    }

    pub fn provider_id(&self) -> &'a str {
        self.provider_id
    }

    pub fn workspace_root_digest(&self) -> &'a [u8; DIGEST_LEN] {
        self.workspace_root_digest
    }

    pub fn workspace_identity_digest(&self) -> &'a [u8; DIGEST_LEN] {
        self.workspace_identity_digest
    }

    pub fn parser_identity_digest(&self) -> &'a [u8; DIGEST_LEN] {
        self.parser_identity_digest
    }

    pub fn query_pack_digest(&self) -> &'a [u8; DIGEST_LEN] {
        self.query_pack_digest
    }

    pub fn generation_digest(&self) -> &'a [u8; DIGEST_LEN] {
        self.generation_digest
    }

    pub fn fixture_digest(&self) -> &'a [u8; DIGEST_LEN] {
        self.fixture_digest
    }

    pub fn record_count(&self) -> u32 {
        self.record_count
    }

    pub fn owner_count(&self) -> u32 {
        self.owner_count
    }

    pub fn selector_count(&self) -> u32 {
        self.selector_count
    }

    pub fn leaf_count(&self) -> u32 {
        self.leaf_count
    }

    pub fn lookup(
        &self,
        structural_selector: &str,
    ) -> Result<
        Option<ExactSelectorGenerationRecordViewV1<'a>>,
        ExactSelectorGenerationFixtureErrorV1,
    > {
        let mut low = 0_u32;
        let mut high = self.record_count;
        while low < high {
            let middle = low + ((high - low) / 2);
            let record = self.record_at(middle)?;
            match record.structural_selector.cmp(structural_selector) {
                Ordering::Less => low = middle + 1,
                Ordering::Greater => high = middle,
                Ordering::Equal => return Ok(Some(record)),
            }
        }
        Ok(None)
    }

    fn record_at(
        &self,
        index: u32,
    ) -> Result<ExactSelectorGenerationRecordViewV1<'a>, ExactSelectorGenerationFixtureErrorV1>
    {
        if index >= self.record_count {
            return Err(ExactSelectorGenerationFixtureErrorV1::InvalidOffset);
        }
        let index = usize::try_from(index)
            .map_err(|_| ExactSelectorGenerationFixtureErrorV1::InvalidOffset)?;
        let start = usize::try_from(read_u64(self.bytes, self.offsets_start + (index * 8))?)
            .map_err(|_| ExactSelectorGenerationFixtureErrorV1::InvalidOffset)?;
        let end = usize::try_from(read_u64(
            self.bytes,
            self.offsets_start + ((index + 1) * 8),
        )?)
        .map_err(|_| ExactSelectorGenerationFixtureErrorV1::InvalidOffset)?;
        if start > end {
            return Err(ExactSelectorGenerationFixtureErrorV1::InvalidOffset);
        }
        let start = self
            .records_start
            .checked_add(start)
            .ok_or(ExactSelectorGenerationFixtureErrorV1::InvalidOffset)?;
        let end = self
            .records_start
            .checked_add(end)
            .ok_or(ExactSelectorGenerationFixtureErrorV1::InvalidOffset)?;
        let record = self
            .bytes
            .get(start..end)
            .ok_or(ExactSelectorGenerationFixtureErrorV1::InvalidOffset)?;
        decode_record(record)
    }
}

pub fn build_exact_selector_generation_fixture_v1(
    identity: &ExactSelectorGenerationIdentityV1,
    mut records: Vec<ExactSelectorGenerationRecordV1>,
) -> Result<Vec<u8>, ExactSelectorGenerationFixtureErrorV1> {
    if identity.language_id.is_empty()
        || identity.provider_id.is_empty()
        || identity.selector_count == 0
        || usize::try_from(identity.selector_count).ok() != Some(records.len())
        || identity.owner_count == 0
        || identity.owner_count != identity.leaf_count
        || records.is_empty()
    {
        return Err(
            ExactSelectorGenerationFixtureErrorV1::IncompleteGeneration {
                record_count: u32::try_from(records.len()).unwrap_or(u32::MAX),
                selector_count: identity.selector_count,
                owner_count: identity.owner_count,
                leaf_count: identity.leaf_count,
            },
        );
    }
    let language_len = u16::try_from(identity.language_id.len())
        .map_err(|_| ExactSelectorGenerationFixtureErrorV1::IdentityTooLong)?;
    let provider_len = u16::try_from(identity.provider_id.len())
        .map_err(|_| ExactSelectorGenerationFixtureErrorV1::IdentityTooLong)?;
    records
        .sort_unstable_by(|left, right| left.structural_selector.cmp(&right.structural_selector));
    if records
        .windows(2)
        .any(|window| window[0].structural_selector == window[1].structural_selector)
    {
        return Err(ExactSelectorGenerationFixtureErrorV1::DuplicateSelector);
    }
    let covered_owner_count = records
        .iter()
        .map(|record| record.owner_path.as_str())
        .collect::<std::collections::BTreeSet<_>>()
        .len();
    let materialization_evidence_is_complete = records.iter().all(|record| {
        !record.owner_path.is_empty()
            && record.source_byte_range.start < record.source_byte_range.end
            && !record.projection.is_empty()
            && record.owner_subtree_digest.iter().any(|byte| *byte != 0)
            && record.source_blob_digest.iter().any(|byte| *byte != 0)
            && record
                .normalized_parser_facts_digest
                .iter()
                .any(|byte| *byte != 0)
    });
    if u32::try_from(covered_owner_count).ok() != Some(identity.owner_count)
        || !materialization_evidence_is_complete
    {
        return Err(
            ExactSelectorGenerationFixtureErrorV1::IncompleteGeneration {
                record_count: u32::try_from(records.len()).unwrap_or(u32::MAX),
                selector_count: identity.selector_count,
                owner_count: identity.owner_count,
                leaf_count: identity.leaf_count,
            },
        );
    }
    let record_count = u32::try_from(records.len())
        .map_err(|_| ExactSelectorGenerationFixtureErrorV1::TooManyRecords)?;
    let mut encoded_records = Vec::with_capacity(records.len());
    for record in &records {
        encoded_records.push(encode_record(record)?);
    }
    let offsets_len = (encoded_records.len() + 1)
        .checked_mul(8)
        .ok_or(ExactSelectorGenerationFixtureErrorV1::TooManyRecords)?;
    let records_len = encoded_records.iter().try_fold(0_usize, |length, record| {
        length
            .checked_add(record.len())
            .ok_or(ExactSelectorGenerationFixtureErrorV1::TooManyRecords)
    })?;
    let capacity = FIXED_HEADER_LEN
        .checked_add(identity.language_id.len())
        .and_then(|length| length.checked_add(identity.provider_id.len()))
        .and_then(|length| length.checked_add(offsets_len))
        .and_then(|length| length.checked_add(records_len))
        .ok_or(ExactSelectorGenerationFixtureErrorV1::TooManyRecords)?;
    let mut bytes = Vec::with_capacity(capacity);
    bytes.extend_from_slice(MAGIC);
    bytes.extend_from_slice(&record_count.to_le_bytes());
    bytes.extend_from_slice(&identity.owner_count.to_le_bytes());
    bytes.extend_from_slice(&identity.leaf_count.to_le_bytes());
    bytes.extend_from_slice(&identity.selector_count.to_le_bytes());
    bytes.extend_from_slice(&language_len.to_le_bytes());
    bytes.extend_from_slice(&provider_len.to_le_bytes());
    bytes.extend_from_slice(&identity.workspace_root_digest);
    bytes.extend_from_slice(&identity.workspace_identity_digest);
    bytes.extend_from_slice(&identity.parser_identity_digest);
    bytes.extend_from_slice(&identity.query_pack_digest);
    bytes.extend_from_slice(&identity.generation_digest);
    bytes.extend_from_slice(&[0_u8; DIGEST_LEN]);
    bytes.extend_from_slice(identity.language_id.as_bytes());
    bytes.extend_from_slice(identity.provider_id.as_bytes());
    let mut offset = 0_u64;
    bytes.extend_from_slice(&offset.to_le_bytes());
    for record in &encoded_records {
        offset = offset
            .checked_add(
                u64::try_from(record.len())
                    .map_err(|_| ExactSelectorGenerationFixtureErrorV1::TooManyRecords)?,
            )
            .ok_or(ExactSelectorGenerationFixtureErrorV1::TooManyRecords)?;
        bytes.extend_from_slice(&offset.to_le_bytes());
    }
    for record in encoded_records {
        bytes.extend_from_slice(&record);
    }
    let fixture_digest = blake3::hash(&bytes);
    bytes[FIXTURE_DIGEST_OFFSET..FIXTURE_DIGEST_OFFSET + DIGEST_LEN]
        .copy_from_slice(fixture_digest.as_bytes());
    Ok(bytes)
}

pub fn fixture_digest_v1(
    bytes: &[u8],
) -> Result<&[u8; DIGEST_LEN], ExactSelectorGenerationFixtureErrorV1> {
    digest_at(bytes, FIXTURE_DIGEST_OFFSET)
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ExactSelectorGenerationFixtureErrorV1 {
    DuplicateSelector,
    FixtureMismatch,
    GenerationMismatch,
    IdentityTooLong,
    IncompleteGeneration {
        record_count: u32,
        selector_count: u32,
        owner_count: u32,
        leaf_count: u32,
    },
    InvalidHeader,
    InvalidOffset,
    InvalidProjectionMode(u8),
    InvalidUtf8,
    RangeOutsideProjection,
    TooManyRecords,
}

impl fmt::Display for ExactSelectorGenerationFixtureErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for ExactSelectorGenerationFixtureErrorV1 {}

fn encode_record(
    record: &ExactSelectorGenerationRecordV1,
) -> Result<Vec<u8>, ExactSelectorGenerationFixtureErrorV1> {
    if record.structural_selector.is_empty()
        || record.owner_path.is_empty()
        || record.source_byte_range.start > record.source_byte_range.end
    {
        return Err(ExactSelectorGenerationFixtureErrorV1::RangeOutsideProjection);
    }
    let selector_len = u32::try_from(record.structural_selector.len())
        .map_err(|_| ExactSelectorGenerationFixtureErrorV1::TooManyRecords)?;
    let owner_len = u32::try_from(record.owner_path.len())
        .map_err(|_| ExactSelectorGenerationFixtureErrorV1::TooManyRecords)?;
    let projection_len = u32::try_from(record.projection.len())
        .map_err(|_| ExactSelectorGenerationFixtureErrorV1::TooManyRecords)?;
    let mut encoded = Vec::with_capacity(
        RECORD_HEADER_LEN
            + record.structural_selector.len()
            + record.owner_path.len()
            + record.projection.len(),
    );
    encoded.extend_from_slice(&selector_len.to_le_bytes());
    encoded.extend_from_slice(&owner_len.to_le_bytes());
    encoded.extend_from_slice(&projection_len.to_le_bytes());
    encoded.push(record.projection_mode as u8);
    encoded.extend_from_slice(&record.source_byte_range.start.to_le_bytes());
    encoded.extend_from_slice(&record.source_byte_range.end.to_le_bytes());
    encoded.extend_from_slice(&record.owner_subtree_digest);
    encoded.extend_from_slice(&record.source_blob_digest);
    encoded.extend_from_slice(&record.normalized_parser_facts_digest);
    encoded.extend_from_slice(record.structural_selector.as_bytes());
    encoded.extend_from_slice(record.owner_path.as_bytes());
    encoded.extend_from_slice(&record.projection);
    Ok(encoded)
}

fn decode_record(
    record: &[u8],
) -> Result<ExactSelectorGenerationRecordViewV1<'_>, ExactSelectorGenerationFixtureErrorV1> {
    if record.len() < RECORD_HEADER_LEN {
        return Err(ExactSelectorGenerationFixtureErrorV1::InvalidOffset);
    }
    let selector_len = usize::try_from(read_u32(record, 0)?)
        .map_err(|_| ExactSelectorGenerationFixtureErrorV1::InvalidOffset)?;
    let owner_len = usize::try_from(read_u32(record, 4)?)
        .map_err(|_| ExactSelectorGenerationFixtureErrorV1::InvalidOffset)?;
    let projection_len = usize::try_from(read_u32(record, 8)?)
        .map_err(|_| ExactSelectorGenerationFixtureErrorV1::InvalidOffset)?;
    let projection_mode = ExactSelectorProjectionModeV1::from_byte(record[12])?;
    let source_start = read_u64(record, 13)?;
    let source_end = read_u64(record, 21)?;
    let owner_subtree_digest = digest_at(record, 29)?;
    let source_blob_digest = digest_at(record, 61)?;
    let normalized_parser_facts_digest = digest_at(record, 93)?;
    let selector_start = RECORD_HEADER_LEN;
    let selector_end = selector_start
        .checked_add(selector_len)
        .ok_or(ExactSelectorGenerationFixtureErrorV1::InvalidOffset)?;
    let owner_end = selector_end
        .checked_add(owner_len)
        .ok_or(ExactSelectorGenerationFixtureErrorV1::InvalidOffset)?;
    let projection_end = owner_end
        .checked_add(projection_len)
        .ok_or(ExactSelectorGenerationFixtureErrorV1::InvalidOffset)?;
    if projection_end != record.len() {
        return Err(ExactSelectorGenerationFixtureErrorV1::InvalidOffset);
    }
    Ok(ExactSelectorGenerationRecordViewV1 {
        structural_selector: utf8_slice(record, selector_start, selector_end)?,
        owner_path: utf8_slice(record, selector_end, owner_end)?,
        owner_subtree_digest,
        source_blob_digest,
        normalized_parser_facts_digest,
        projection_mode,
        source_byte_range: source_start..source_end,
        projection: record
            .get(owner_end..projection_end)
            .ok_or(ExactSelectorGenerationFixtureErrorV1::InvalidOffset)?,
    })
}

fn digest_at(
    bytes: &[u8],
    start: usize,
) -> Result<&[u8; DIGEST_LEN], ExactSelectorGenerationFixtureErrorV1> {
    bytes
        .get(start..start + DIGEST_LEN)
        .and_then(|digest| digest.try_into().ok())
        .ok_or(ExactSelectorGenerationFixtureErrorV1::InvalidHeader)
}

fn read_u16(bytes: &[u8], start: usize) -> Result<u16, ExactSelectorGenerationFixtureErrorV1> {
    bytes
        .get(start..start + 2)
        .and_then(|value| value.try_into().ok())
        .map(u16::from_le_bytes)
        .ok_or(ExactSelectorGenerationFixtureErrorV1::InvalidHeader)
}

fn read_u32(bytes: &[u8], start: usize) -> Result<u32, ExactSelectorGenerationFixtureErrorV1> {
    bytes
        .get(start..start + 4)
        .and_then(|value| value.try_into().ok())
        .map(u32::from_le_bytes)
        .ok_or(ExactSelectorGenerationFixtureErrorV1::InvalidHeader)
}

fn read_u64(bytes: &[u8], start: usize) -> Result<u64, ExactSelectorGenerationFixtureErrorV1> {
    bytes
        .get(start..start + 8)
        .and_then(|value| value.try_into().ok())
        .map(u64::from_le_bytes)
        .ok_or(ExactSelectorGenerationFixtureErrorV1::InvalidHeader)
}

fn utf8_slice(
    bytes: &[u8],
    start: usize,
    end: usize,
) -> Result<&str, ExactSelectorGenerationFixtureErrorV1> {
    std::str::from_utf8(
        bytes
            .get(start..end)
            .ok_or(ExactSelectorGenerationFixtureErrorV1::InvalidOffset)?,
    )
    .map_err(|_| ExactSelectorGenerationFixtureErrorV1::InvalidUtf8)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::CanonicalItemSelectorV1;

    fn digest(byte: u8) -> [u8; DIGEST_LEN] {
        [byte; DIGEST_LEN]
    }

    fn materialization_proof() -> ExactSelectorMaterializationProofV1 {
        let projection = b"fn run() {}".to_vec();
        let structural_selector = "rust://src/lib.rs#item/function/run".to_string();
        ExactSelectorMaterializationProofV1 {
            language_id: "rust".to_string(),
            provider_id: "rs-harness".to_string(),
            canonical_item_selector: CanonicalItemSelectorV1::parse(&structural_selector)
                .expect("canonical selector"),
            parser_identity_digest: digest(1),
            query_pack_digest: digest(2),
            workspace_root_digest: digest(3),
            owner_path: "src/lib.rs".to_string(),
            owner_subtree_digest: digest(3),
            owner_inclusion_proof: Vec::new(),
            source_blob_digest: digest(4),
            normalized_parser_facts_digest: digest(5),
            structural_selector,
            projection_mode: ExactSelectorProjectionModeV1::Code,
            source_byte_start: 0,
            source_byte_end: 11,
            projection_digest: *blake3::hash(&projection).as_bytes(),
            projection,
        }
    }

    #[test]
    fn materialization_proof_converts_without_source_or_line_reconstruction() {
        let proof = materialization_proof();
        let record = ExactSelectorGenerationRecordV1::try_from(&proof).expect("proof");
        assert_eq!(record.structural_selector, proof.structural_selector);
        assert_eq!(record.owner_path, proof.owner_path);
        assert_eq!(
            record.source_byte_range,
            proof.source_byte_start..proof.source_byte_end
        );
        assert_eq!(record.projection, proof.projection);
    }

    #[test]
    fn materialization_proof_rejects_selector_mismatch() {
        let mut proof = materialization_proof();
        proof.canonical_item_selector.structural_selector =
            "rust://src/lib.rs#item/function/other".to_string();
        assert_eq!(
            ExactSelectorGenerationRecordV1::try_from(&proof),
            Err(ExactSelectorMaterializationProofErrorV1::InvalidCanonicalSelector)
        );
    }

    #[test]
    fn materialization_proof_rejects_zero_digest() {
        let mut proof = materialization_proof();
        proof.source_blob_digest = [0_u8; DIGEST_LEN];
        assert_eq!(
            ExactSelectorGenerationRecordV1::try_from(&proof),
            Err(ExactSelectorMaterializationProofErrorV1::InvalidDigest)
        );
    }

    #[test]
    fn materialization_proof_rejects_invalid_source_byte_range() {
        let mut proof = materialization_proof();
        proof.source_byte_start = proof.source_byte_end;
        assert_eq!(
            ExactSelectorGenerationRecordV1::try_from(&proof),
            Err(ExactSelectorMaterializationProofErrorV1::InvalidSourceByteRange)
        );
    }

    #[test]
    fn materialization_proof_rejects_projection_digest_mismatch() {
        let mut proof = materialization_proof();
        proof.projection_digest = digest(9);
        assert_eq!(
            ExactSelectorGenerationRecordV1::try_from(&proof),
            Err(ExactSelectorMaterializationProofErrorV1::ProjectionDigestMismatch)
        );
    }

    #[test]
    fn materialization_proof_rejects_empty_projection() {
        let mut proof = materialization_proof();
        proof.projection.clear();
        proof.projection_digest = *blake3::hash(&proof.projection).as_bytes();
        assert_eq!(
            ExactSelectorGenerationRecordV1::try_from(&proof),
            Err(ExactSelectorMaterializationProofErrorV1::EmptyProjection)
        );
    }

    #[test]
    fn materialization_proof_rejects_workspace_root_mismatch() {
        let mut proof = materialization_proof();
        proof.workspace_root_digest = digest(8);
        assert_eq!(
            ExactSelectorGenerationRecordV1::try_from(&proof),
            Err(ExactSelectorMaterializationProofErrorV1::WorkspaceRootDigestMismatch)
        );
    }

    fn fixture() -> (Vec<u8>, [u8; DIGEST_LEN]) {
        let generation_digest = digest(4);
        let bytes = build_exact_selector_generation_fixture_v1(
            &ExactSelectorGenerationIdentityV1 {
                workspace_identity_digest: [9_u8; DIGEST_LEN],
                language_id: "rust".to_string(),
                provider_id: "rs-harness".to_string(),
                workspace_root_digest: digest(1),
                parser_identity_digest: digest(2),
                query_pack_digest: digest(3),
                generation_digest,
                selector_count: 1,
                owner_count: 1,
                leaf_count: 1,
            },
            vec![ExactSelectorGenerationRecordV1 {
                structural_selector: "rust://src/lib.rs#item/function/run".to_string(),
                owner_path: "src/lib.rs".to_string(),
                owner_subtree_digest: digest(5),
                source_blob_digest: digest(6),
                normalized_parser_facts_digest: digest(7),
                projection_mode: ExactSelectorProjectionModeV1::Code,
                source_byte_range: 0..11,
                projection: b"fn run() {}".to_vec(),
            }],
        )
        .expect("fixture");
        (bytes, generation_digest)
    }

    #[test]
    fn exact_lookup_reads_immutable_projection() {
        let (bytes, generation_digest) = fixture();
        let fixture_digest = *fixture_digest_v1(&bytes).expect("fixture digest");
        let view = ExactSelectorGenerationFixtureViewV1::attach(
            &bytes,
            &generation_digest,
            &fixture_digest,
        )
        .expect("attach");
        let record = view
            .lookup("rust://src/lib.rs#item/function/run")
            .expect("lookup")
            .expect("hit");
        assert_eq!(record.projection, b"fn run() {}");
        assert_eq!(record.owner_path, "src/lib.rs");
    }

    #[test]
    fn absent_selector_is_a_closed_miss() {
        let (bytes, generation_digest) = fixture();
        let fixture_digest = *fixture_digest_v1(&bytes).expect("fixture digest");
        let view = ExactSelectorGenerationFixtureViewV1::attach(
            &bytes,
            &generation_digest,
            &fixture_digest,
        )
        .expect("attach");
        assert_eq!(
            view.lookup("rust://src/lib.rs#item/function/missing")
                .expect("lookup"),
            None
        );
    }

    #[test]
    fn incomplete_generation_is_rejected() {
        let error = build_exact_selector_generation_fixture_v1(
            &ExactSelectorGenerationIdentityV1 {
                workspace_identity_digest: [9_u8; DIGEST_LEN],
                language_id: "rust".to_string(),
                provider_id: "rs-harness".to_string(),
                workspace_root_digest: digest(1),
                parser_identity_digest: digest(2),
                query_pack_digest: digest(3),
                generation_digest: digest(4),
                selector_count: 1,
                owner_count: 84,
                leaf_count: 85,
            },
            vec![ExactSelectorGenerationRecordV1 {
                structural_selector: "rust://src/lib.rs#item/function/run".to_string(),
                owner_path: "src/lib.rs".to_string(),
                owner_subtree_digest: digest(5),
                source_blob_digest: digest(6),
                normalized_parser_facts_digest: digest(7),
                projection_mode: ExactSelectorProjectionModeV1::Code,
                source_byte_range: 0..11,
                projection: b"fn run() {}".to_vec(),
            }],
        )
        .expect_err("incomplete generation");
        assert_eq!(
            error,
            ExactSelectorGenerationFixtureErrorV1::IncompleteGeneration {
                record_count: 1,
                selector_count: 1,
                owner_count: 84,
                leaf_count: 85,
            }
        );
    }

    #[test]
    fn partial_selector_materialization_is_rejected() {
        let error = build_exact_selector_generation_fixture_v1(
            &ExactSelectorGenerationIdentityV1 {
                workspace_identity_digest: [9_u8; DIGEST_LEN],
                language_id: "rust".to_string(),
                provider_id: "rs-harness".to_string(),
                workspace_root_digest: digest(1),
                parser_identity_digest: digest(2),
                query_pack_digest: digest(3),
                generation_digest: digest(4),
                selector_count: 2,
                owner_count: 1,
                leaf_count: 1,
            },
            vec![ExactSelectorGenerationRecordV1 {
                structural_selector: "rust://src/lib.rs#item/function/run".to_string(),
                owner_path: "src/lib.rs".to_string(),
                owner_subtree_digest: digest(5),
                source_blob_digest: digest(6),
                normalized_parser_facts_digest: digest(7),
                projection_mode: ExactSelectorProjectionModeV1::Code,
                source_byte_range: 0..11,
                projection: b"fn run() {}".to_vec(),
            }],
        )
        .expect_err("partial selector generation");
        assert_eq!(
            error,
            ExactSelectorGenerationFixtureErrorV1::IncompleteGeneration {
                record_count: 1,
                selector_count: 2,
                owner_count: 1,
                leaf_count: 1,
            }
        );
    }
}
