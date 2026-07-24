use std::path::{Component, Path};

use serde::{Deserialize, Serialize};

use crate::exact_selector_cache::ExactSelectorProjectionRecordV1;
use crate::exact_selector_merkle::{
    ContentDigestV1, ExactProjectionModeV1, ExactSelectorMerkleProofV1, canonical_digest_v1,
    derive_projection_digest_v1,
};
use crate::workspace_merkle_v1::{WorkspacePathMerkleTreeV1, derive_owner_subtree_digest_v1};

pub const EXACT_SELECTOR_PROJECTION_PACKET_SCHEMA_ID: &str =
    "agent.semantic-protocols.exact-selector-projection-packet";
pub const EXACT_SELECTOR_PROJECTION_PACKET_SCHEMA_VERSION: &str = "1";
pub const EXACT_SELECTOR_PROJECTION_PACKET_DIGEST_ALGORITHM: &str = "blake3-256";
const PARSER_IDENTITY_DOMAIN: &[u8] = b"asp.parser-identity.v1";
const QUERY_PACK_IDENTITY_DOMAIN: &[u8] = b"asp.query-pack-identity.v1";

macro_rules! projection_packet_text_v1 {
    ($(#[$meta:meta])* $name:ident) => {
        $(#[$meta])*
        #[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
        pub struct $name(String);

        impl $name {
            pub fn as_str(&self) -> &str {
                &self.0
            }

            pub fn into_string(self) -> String {
                self.0
            }
        }

        impl From<String> for $name {
            fn from(value: String) -> Self {
                Self(value)
            }
        }

        impl From<&str> for $name {
            fn from(value: &str) -> Self {
                Self(value.to_owned())
            }
        }
    };
}

projection_packet_text_v1!(
    /// Provider id used to derive a parser identity digest.
    ProjectionPacketProviderIdV1
);
projection_packet_text_v1!(
    /// Language id used in an exact selector projection packet.
    ProjectionPacketLanguageIdV1
);
projection_packet_text_v1!(
    /// Execution command digest used to derive a parser identity digest.
    ProjectionPacketExecutionCommandDigestV1
);
projection_packet_text_v1!(
    /// Semantic registry digest used to derive a parser identity digest.
    ProjectionPacketSemanticRegistryDigestV1
);
projection_packet_text_v1!(
    /// Owner path used in an exact selector projection packet.
    ProjectionPacketOwnerPathV1
);
projection_packet_text_v1!(
    /// Structural selector used in an exact selector projection packet.
    ProjectionPacketStructuralSelectorV1
);
projection_packet_text_v1!(
    /// Base64 encoded projection payload carried by an exact selector packet.
    ProjectionPacketPayloadBase64V1
);

pub fn derive_parser_identity_digest_v1(
    provider_id: &ProjectionPacketProviderIdV1,
    execution_command_digest: &ProjectionPacketExecutionCommandDigestV1,
    semantic_registry_digest: &ProjectionPacketSemanticRegistryDigestV1,
) -> ContentDigestV1 {
    canonical_digest_v1(
        PARSER_IDENTITY_DOMAIN,
        &[
            provider_id.as_str().as_bytes(),
            execution_command_digest.as_str().as_bytes(),
            semantic_registry_digest.as_str().as_bytes(),
        ],
    )
}

pub fn derive_query_pack_identity_digest_v1(canonical_descriptor_json: &[u8]) -> ContentDigestV1 {
    canonical_digest_v1(QUERY_PACK_IDENTITY_DOMAIN, &[canonical_descriptor_json])
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ExactSelectorProjectionEncodingV1 {
    Base64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExactSelectorProjectionPacketV1 {
    schema_id: String,
    schema_version: String,
    digest_algorithm: String,
    pub language_id: ProjectionPacketLanguageIdV1,
    pub provider_id: ProjectionPacketProviderIdV1,
    canonical_item_selector: crate::canonical_item_identity::CanonicalItemSelectorV1,
    parser_identity_digest: ContentDigestV1,
    query_pack_digest: ContentDigestV1,
    pub owner_path: ProjectionPacketOwnerPathV1,
    source_blob_digest: ContentDigestV1,
    parser_fact_digest: ContentDigestV1,
    pub structural_selector: ProjectionPacketStructuralSelectorV1,
    projection_mode: ExactProjectionModeV1,
    projection_encoding: ExactSelectorProjectionEncodingV1,
    pub projection_payload_base64: ProjectionPacketPayloadBase64V1,
}

pub fn build_exact_selector_projection_packet_v1(
    language_id: &ProjectionPacketLanguageIdV1,
    provider_id: &ProjectionPacketProviderIdV1,
    canonical_item_selector: crate::canonical_item_identity::CanonicalItemSelectorV1,
    parser_identity_digest: &ContentDigestV1,
    query_pack_digest: &ContentDigestV1,
    owner_path: &ProjectionPacketOwnerPathV1,
    structural_selector: &ProjectionPacketStructuralSelectorV1,
    projection_mode: ExactProjectionModeV1,
    source: &[u8],
    normalized_parser_facts: &[u8],
    projection: &[u8],
) -> ExactSelectorProjectionPacketV1 {
    let source_blob_digest = crate::exact_selector_merkle::blake3_content_digest_v1(source);
    let parser_fact_digest = crate::exact_selector_merkle::canonical_content_digest_v1(
        b"asp.parser-fact.v1",
        &[
            language_id.as_str().as_bytes(),
            parser_identity_digest.as_str().as_bytes(),
            query_pack_digest.as_str().as_bytes(),
            source_blob_digest.as_str().as_bytes(),
            normalized_parser_facts,
        ],
    );
    ExactSelectorProjectionPacketV1 {
        schema_id: EXACT_SELECTOR_PROJECTION_PACKET_SCHEMA_ID.to_owned(),
        schema_version: EXACT_SELECTOR_PROJECTION_PACKET_SCHEMA_VERSION.to_owned(),
        digest_algorithm: EXACT_SELECTOR_PROJECTION_PACKET_DIGEST_ALGORITHM.to_owned(),
        language_id: language_id.clone(),
        provider_id: provider_id.clone(),
        canonical_item_selector,
        parser_identity_digest: parser_identity_digest.clone(),
        query_pack_digest: query_pack_digest.clone(),
        owner_path: owner_path.clone(),
        source_blob_digest,
        parser_fact_digest,
        structural_selector: structural_selector.clone(),
        projection_mode,
        projection_encoding: ExactSelectorProjectionEncodingV1::Base64,
        projection_payload_base64: ProjectionPacketPayloadBase64V1::from(
            encode_projection_payload_base64_v1(projection),
        ),
    }
}

fn encode_projection_payload_base64_v1(bytes: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut encoded = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let first = chunk[0];
        let second = chunk.get(1).copied().unwrap_or(0);
        let third = chunk.get(2).copied().unwrap_or(0);
        encoded.push(TABLE[(first >> 2) as usize] as char);
        encoded.push(TABLE[(((first & 0x03) << 4) | (second >> 4)) as usize] as char);
        encoded.push(if chunk.len() > 1 {
            TABLE[(((second & 0x0f) << 2) | (third >> 6)) as usize] as char
        } else {
            '='
        });
        encoded.push(if chunk.len() > 2 {
            TABLE[(third & 0x3f) as usize] as char
        } else {
            '='
        });
    }
    encoded
}

#[cfg(test)]
#[path = "../tests/unit/exact_selector_projection_packet.rs"]
mod packet_builder_tests;

impl ExactSelectorProjectionPacketV1 {
    pub fn language_id(&self) -> &ProjectionPacketLanguageIdV1 {
        &self.language_id
    }

    pub fn provider_id(&self) -> &ProjectionPacketProviderIdV1 {
        &self.provider_id
    }

    pub fn owner_path(&self) -> &ProjectionPacketOwnerPathV1 {
        &self.owner_path
    }

    pub fn structural_selector(&self) -> &ProjectionPacketStructuralSelectorV1 {
        &self.structural_selector
    }

    pub fn validate_shape(&self) -> Result<(), ExactSelectorProjectionPacketV1Error> {
        if self.schema_id != EXACT_SELECTOR_PROJECTION_PACKET_SCHEMA_ID
            || self.schema_version != EXACT_SELECTOR_PROJECTION_PACKET_SCHEMA_VERSION
            || self.digest_algorithm != EXACT_SELECTOR_PROJECTION_PACKET_DIGEST_ALGORITHM
        {
            return Err(ExactSelectorProjectionPacketV1Error::ContractIdentity);
        }
        if self.language_id.as_str().is_empty()
            || self.provider_id.as_str().is_empty()
            || self.structural_selector.as_str().is_empty()
        {
            return Err(ExactSelectorProjectionPacketV1Error::RequiredIdentity);
        }
        self.canonical_item_selector
            .validate()
            .map_err(|_| ExactSelectorProjectionPacketV1Error::CanonicalItemIdentity)?;
        if self.canonical_item_selector.language_id.as_str() != self.language_id.as_str()
            || self.canonical_item_selector.structural_selector.as_str()
                != self.structural_selector.as_str()
        {
            return Err(ExactSelectorProjectionPacketV1Error::CanonicalItemIdentity);
        }
        let owner_path = Path::new(self.owner_path.as_str());
        if self.owner_path.as_str().trim().is_empty()
            || owner_path.is_absolute()
            || owner_path.components().any(|component| {
                matches!(
                    component,
                    Component::ParentDir | Component::RootDir | Component::Prefix(_)
                )
            })
        {
            return Err(ExactSelectorProjectionPacketV1Error::OwnerPath);
        }
        if !is_canonical_base64(self.projection_payload_base64.as_str()) {
            return Err(ExactSelectorProjectionPacketV1Error::ProjectionPayload);
        }
        Ok(())
    }

    pub fn enrich_projection_record(
        self,
        workspace_tree: &WorkspacePathMerkleTreeV1,
    ) -> Result<ExactSelectorProjectionRecordV1, ExactSelectorProjectionPacketV1Error> {
        self.validate_shape()?;
        let projection_payload = decode_canonical_base64(self.projection_payload_base64.as_str())
            .ok_or(ExactSelectorProjectionPacketV1Error::ProjectionPayload)?;
        let owner_subtree_digest = workspace_tree
            .owner_subtree_digest(self.owner_path.as_str())
            .ok_or(ExactSelectorProjectionPacketV1Error::OwnerNotInSnapshot)?;
        let expected_owner_subtree_digest =
            derive_owner_subtree_digest_v1(self.owner_path.as_str(), &self.source_blob_digest);
        if owner_subtree_digest != &expected_owner_subtree_digest {
            return Err(ExactSelectorProjectionPacketV1Error::SourceSnapshotMismatch);
        }
        let owner_inclusion_proof = workspace_tree
            .inclusion_proof(self.owner_path.as_str())
            .ok_or(ExactSelectorProjectionPacketV1Error::OwnerNotInSnapshot)?;
        let projection_digest = derive_projection_digest_v1(
            &self.canonical_item_selector,
            self.structural_selector.as_str(),
            self.projection_mode,
            &self.parser_fact_digest,
            &projection_payload,
        );
        Ok(ExactSelectorProjectionRecordV1 {
            proof: ExactSelectorMerkleProofV1::from_input(
                crate::exact_selector_merkle::ExactSelectorMerkleProofInputV1 {
                    language_id: self.language_id.as_str().into(),
                    workspace_root_digest: workspace_tree.root_digest().clone(),
                    owner_path: self.owner_path.as_str().to_string(),
                    owner_subtree_digest: expected_owner_subtree_digest,
                    owner_inclusion_proof,
                    source_blob_digest: self.source_blob_digest,
                    parser_identity_digest: self.parser_identity_digest,
                    query_pack_digest: self.query_pack_digest,
                    parser_fact_digest: self.parser_fact_digest,
                    canonical_item_selector: self.canonical_item_selector,
                    structural_selector: self.structural_selector.as_str().to_string(),
                    projection_mode: self.projection_mode,
                    projection_digest,
                },
            ),
            projection_payload,
        })
    }
}

fn is_canonical_base64(value: &str) -> bool {
    decode_canonical_base64(value).is_some()
}

fn decode_canonical_base64(value: &str) -> Option<Vec<u8>> {
    if value.len() % 4 != 0 {
        return None;
    }
    let chunk_count = value.len() / 4;
    let mut output = Vec::with_capacity(chunk_count * 3);
    for (index, chunk) in value.as_bytes().chunks_exact(4).enumerate() {
        let last = index + 1 == chunk_count;
        let first = base64_value(chunk[0])?;
        let second = base64_value(chunk[1])?;
        output.push((first << 2) | (second >> 4));

        if chunk[2] == b'=' {
            if !last || chunk[3] != b'=' || second & 0x0f != 0 {
                return None;
            }
            continue;
        }
        let third = base64_value(chunk[2])?;
        output.push((second << 4) | (third >> 2));
        if chunk[3] == b'=' {
            if !last || third & 0x03 != 0 {
                return None;
            }
            continue;
        }
        output.push((third << 6) | base64_value(chunk[3])?);
    }
    Some(output)
}

fn base64_value(byte: u8) -> Option<u8> {
    match byte {
        b'A'..=b'Z' => Some(byte - b'A'),
        b'a'..=b'z' => Some(byte - b'a' + 26),
        b'0'..=b'9' => Some(byte - b'0' + 52),
        b'+' => Some(62),
        b'/' => Some(63),
        _ => None,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExactSelectorProjectionPacketV1Error {
    ContractIdentity,
    RequiredIdentity,
    CanonicalItemIdentity,
    OwnerPath,
    ProjectionPayload,
    OwnerNotInSnapshot,
    SourceSnapshotMismatch,
}
