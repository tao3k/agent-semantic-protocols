//! Typed, framed language-projection batches for generation-time provider work.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::ops::Range;

use serde::{Deserialize, Serialize};

pub const PROJECTION_BATCH_REQUEST_SCHEMA_ID: &str =
    "agent.semantic-protocols.provider-language-projection-batch-request";
pub const PROJECTION_BATCH_RESPONSE_SCHEMA_ID: &str =
    "agent.semantic-protocols.provider-language-projection-batch-response";
pub const CANONICAL_LANGUAGE_ITEM_IDENTITY_SCHEMA_ID: &str =
    "agent.semantic-protocols.canonical-language-item-identity";
/// Maximum owners admitted to one provider projection wire frame.
pub const MAX_PROVIDER_PROJECTION_BATCH_OWNERS: usize = 32;
/// Maximum aggregate unencoded source bytes admitted to one provider projection wire frame.
///
/// The shared client-server contract deliberately stays below the smallest
/// supported HTTP request-body ceiling.  A generation is a stream of these
/// independently validated frames; it is never serialized as one corpus-sized
/// request.  The HTTP transport separately validates the final encoded frame.
pub const MAX_PROVIDER_PROJECTION_BATCH_SOURCE_BYTES: usize = 384 * 1024;

/// Plans ordered provider wire frames without splitting an individual owner.
pub fn provider_projection_batch_ranges(owner_sizes: &[usize]) -> Vec<Range<usize>> {
    let mut ranges = Vec::new();
    let mut start = 0;
    while start < owner_sizes.len() {
        let mut end = start;
        let mut source_bytes = 0usize;
        while end < owner_sizes.len() && end - start < MAX_PROVIDER_PROJECTION_BATCH_OWNERS {
            let next_source_bytes = source_bytes.saturating_add(owner_sizes[end]);
            if end > start && next_source_bytes > MAX_PROVIDER_PROJECTION_BATCH_SOURCE_BYTES {
                break;
            }
            source_bytes = next_source_bytes;
            end += 1;
        }
        ranges.push(start..end);
        start = end;
    }
    ranges
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct ProviderProjectionOwner {
    pub owner_path: String,
    pub source_leaf_digest: String,
    pub source_bytes: Vec<u8>,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct ProviderProjectionBatchRequest {
    pub language_id: String,
    pub provider_id: String,
    pub workspace_identity: String,
    pub generation_root_digest: String,
    pub parser_identity_digest: String,
    pub query_pack_digest: String,
    pub base_generation_root_digest: Option<String>,
    pub owners: Vec<ProviderProjectionOwner>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProviderProjectionBatchResponse {
    pub schema_id: String,
    pub schema_version: String,
    pub language_id: String,
    pub provider_id: String,
    pub generation_root_digest: String,
    pub owners: Vec<ProviderProjectedOwner>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProviderProjectedOwner {
    pub owner_path: String,
    pub source_leaf_digest: String,
    pub items: Vec<ProviderProjectedItem>,
    pub relations: Vec<
        agent_semantic_content_identity::provider_projection_relation::ProviderProjectedRelation,
    >,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProviderProjectedItem {
    pub item_id: String,
    pub owner_id: String,
    pub kind: String,
    pub name: String,
    pub selector: String,
    pub source_byte_start: usize,
    pub source_byte_end: usize,
    pub identity: ProviderProjectedItemIdentity,
    pub projections: Vec<ProviderDerivedProjection>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProviderDerivedProjection {
    pub projection_kind: String,
    pub payload: serde_json::Value,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProviderProjectedItemIdentity {
    pub schema_id: String,
    pub schema_version: String,
    pub language_id: String,
    pub kind: String,
    pub symbol: String,
    pub scopes: Vec<ProviderProjectedItemScope>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProviderProjectedItemScope {
    pub relation: String,
    pub kind: String,
    pub symbol: String,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct ProviderProjectionBatchError(String);

impl fmt::Display for ProviderProjectionBatchError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for ProviderProjectionBatchError {}

impl ProviderProjectionBatchRequest {
    pub fn encode(&self) -> Result<Vec<u8>, ProviderProjectionBatchError> {
        self.validate()?;
        let owners = self
            .owners
            .iter()
            .map(|owner| {
                let source_text = std::str::from_utf8(&owner.source_bytes).map_err(|error| {
                    ProviderProjectionBatchError(format!(
                        "projection owner is not UTF-8 {}: {error}",
                        owner.owner_path
                    ))
                })?;
                Ok(serde_json::json!({
                    "ownerPath": owner.owner_path,
                    "sourceLeafDigest": owner.source_leaf_digest,
                    "sourceText": source_text,
                }))
            })
            .collect::<Result<Vec<_>, ProviderProjectionBatchError>>()?;
        let mut payload = serde_json::json!({
            "schemaId": PROJECTION_BATCH_REQUEST_SCHEMA_ID,
            "schemaVersion": "1",
            "languageId": self.language_id,
            "providerId": self.provider_id,
            "workspaceIdentity": self.workspace_identity,
            "generationRootDigest": self.generation_root_digest,
            "parserIdentityDigest": self.parser_identity_digest,
            "queryPackDigest": self.query_pack_digest,
            "owners": owners,
        });
        if let Some(base_digest) = &self.base_generation_root_digest {
            payload["baseGenerationRootDigest"] = serde_json::Value::String(base_digest.clone());
        }
        serde_json::to_vec(&payload).map_err(|error| {
            ProviderProjectionBatchError(format!("encode projection batch request: {error}"))
        })
    }

    fn validate(&self) -> Result<(), ProviderProjectionBatchError> {
        require_text("languageId", &self.language_id)?;
        require_text("providerId", &self.provider_id)?;
        require_text("workspaceIdentity", &self.workspace_identity)?;
        require_text("generationRootDigest", &self.generation_root_digest)?;
        require_text("parserIdentityDigest", &self.parser_identity_digest)?;
        require_text("queryPackDigest", &self.query_pack_digest)?;
        if let Some(base_digest) = self.base_generation_root_digest.as_deref() {
            require_text("baseGenerationRootDigest", base_digest)?;
        }
        let mut owner_paths = BTreeSet::new();
        for owner in &self.owners {
            require_text("ownerPath", &owner.owner_path)?;
            require_text("sourceLeafDigest", &owner.source_leaf_digest)?;
            if !owner_paths.insert(owner.owner_path.as_str()) {
                return Err(ProviderProjectionBatchError(format!(
                    "duplicate projection owner path: {}",
                    owner.owner_path
                )));
            }
        }
        Ok(())
    }
}

impl ProviderProjectionBatchResponse {
    pub fn decode_for(
        request: &ProviderProjectionBatchRequest,
        bytes: &[u8],
    ) -> Result<Self, ProviderProjectionBatchError> {
        let response: Self = serde_json::from_slice(bytes).map_err(|error| {
            ProviderProjectionBatchError(format!("decode projection batch response: {error}"))
        })?;
        validate_response(request, &response)?;
        Ok(response)
    }
}

fn validate_response(
    request: &ProviderProjectionBatchRequest,
    response: &ProviderProjectionBatchResponse,
) -> Result<(), ProviderProjectionBatchError> {
    if response.schema_id != PROJECTION_BATCH_RESPONSE_SCHEMA_ID
        || response.schema_version != "1"
        || response.language_id != request.language_id
        || response.provider_id != request.provider_id
        || response.generation_root_digest != request.generation_root_digest
    {
        return Err(ProviderProjectionBatchError(
            "projection batch response identity mismatch".to_string(),
        ));
    }
    let expected = request
        .owners
        .iter()
        .map(|owner| (owner.owner_path.as_str(), owner.source_leaf_digest.as_str()))
        .collect::<BTreeMap<_, _>>();
    let actual = response
        .owners
        .iter()
        .map(|owner| (owner.owner_path.as_str(), owner.source_leaf_digest.as_str()))
        .collect::<BTreeMap<_, _>>();
    if expected.len() != request.owners.len()
        || actual.len() != response.owners.len()
        || actual != expected
    {
        return Err(ProviderProjectionBatchError(
            "projection batch owner coverage mismatch".to_string(),
        ));
    }
    for projected_owner in &response.owners {
        let requested_owner = request
            .owners
            .iter()
            .find(|owner| owner.owner_path == projected_owner.owner_path)
            .expect("owner coverage was checked above");
        let expected_owner_id = format!("owner:{}", projected_owner.owner_path);
        let expected_selector_prefix =
            format!("{}://{}#", request.language_id, projected_owner.owner_path);
        let mut selectors = BTreeSet::new();
        for item in &projected_owner.items {
            let canonical = agent_semantic_content_identity::CanonicalItemSelector::parse(
                item.selector.as_str(),
            )
            .map_err(|error| {
                ProviderProjectionBatchError(format!(
                    "projection batch selector is not canonical: ownerPath={} itemId={} error={error}",
                    projected_owner.owner_path, item.item_id
                ))
            })?;
            let identity_scopes_match = canonical.scopes.len() == item.identity.scopes.len()
                && canonical.scopes.iter().zip(&item.identity.scopes).all(
                    |(canonical, projected)| {
                        canonical.relation.as_str() == projected.relation
                            && canonical.kind.as_str() == projected.kind
                            && canonical.symbol.as_str() == projected.symbol
                    },
                );
            let mut proof_mismatches = Vec::new();
            if item.owner_id != expected_owner_id {
                proof_mismatches.push("ownerId");
            }
            if item.identity.schema_id != CANONICAL_LANGUAGE_ITEM_IDENTITY_SCHEMA_ID {
                proof_mismatches.push("identity.schemaId");
            }
            if item.identity.schema_version != "1" {
                proof_mismatches.push("identity.schemaVersion");
            }
            if item.identity.language_id != request.language_id {
                proof_mismatches.push("identity.languageId");
            }
            if item.identity.kind != item.kind {
                proof_mismatches.push("identity.kind");
            }
            if item.identity.symbol != item.name {
                proof_mismatches.push("identity.symbol");
            }
            if canonical.language_id.as_str() != item.identity.language_id {
                proof_mismatches.push("selector.languageId");
            }
            if canonical.kind.as_str() != item.identity.kind {
                proof_mismatches.push("selector.kind");
            }
            if canonical.symbol.as_str() != item.identity.symbol {
                proof_mismatches.push("selector.symbol");
            }
            if !identity_scopes_match {
                proof_mismatches.push("selector.scopes");
            }
            if !item.selector.starts_with(&expected_selector_prefix) {
                proof_mismatches.push("selector.ownerPrefix");
            }
            if item.source_byte_start >= item.source_byte_end {
                proof_mismatches.push("sourceByteRange.emptyOrReversed");
            }
            if item.source_byte_end > requested_owner.source_bytes.len() {
                proof_mismatches.push("sourceByteRange.outOfBounds");
            }
            if !selectors.insert(item.selector.as_str()) {
                proof_mismatches.push("selector.duplicate");
            }
            if !proof_mismatches.is_empty() {
                return Err(ProviderProjectionBatchError(format!(
                    "projection batch item proof mismatch: ownerPath={} itemId={} proofMismatch={} sourceByteStart={} sourceByteEnd={} sourceBytes={}",
                    projected_owner.owner_path,
                    item.item_id,
                    proof_mismatches.join(","),
                    item.source_byte_start,
                    item.source_byte_end,
                    requested_owner.source_bytes.len(),
                )));
            }
        }
        let mut relations = BTreeSet::new();
        for relation in &projected_owner.relations {
            if relation.validate().is_err() || !relations.insert(relation) {
                return Err(ProviderProjectionBatchError(format!(
                    "projection batch relation proof mismatch: ownerPath={} from={}:{} kind={} to={}:{}",
                    projected_owner.owner_path,
                    relation.from.kind,
                    relation.from.id,
                    relation.kind,
                    relation.to.kind,
                    relation.to.id
                )));
            }
        }
    }
    Ok(())
}

fn require_text(field: &str, value: &str) -> Result<(), ProviderProjectionBatchError> {
    if value.trim().is_empty() {
        return Err(ProviderProjectionBatchError(format!(
            "projection batch {field} must be non-empty"
        )));
    }
    Ok(())
}

#[cfg(test)]
#[path = "../tests/unit/projection_batch.rs"]
mod tests;
