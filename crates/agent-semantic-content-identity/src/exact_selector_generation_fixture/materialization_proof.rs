use super::{DIGEST_LEN, ExactSelectorGenerationRecordV1, ExactSelectorProjectionModeV1};

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
    pub canonical_item_selector: crate::CanonicalItemSelector,
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
    canonical_item_selector: crate::CanonicalItemSelector,
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
    canonical_item_selector: crate::CanonicalItemSelector,
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
        ExactSelectorProjectionModeV1::Source => "source",
        ExactSelectorProjectionModeV1::CallableSkeleton => "callable-skeleton",
    }
}

fn projection_mode_v1<E: serde::de::Error>(
    value: &str,
) -> Result<ExactSelectorProjectionModeV1, E> {
    match value {
        "source" => Ok(ExactSelectorProjectionModeV1::Source),
        "callable-skeleton" => Ok(ExactSelectorProjectionModeV1::CallableSkeleton),
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
