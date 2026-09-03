//! Typed, framed language-projection batches for generation-time provider work.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::ops::Range;

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use serde::{Deserialize, Serialize};

pub const PROJECTION_BATCH_REQUEST_SCHEMA_ID: &str =
    "agent.semantic-protocols.provider-language-projection-batch-request";
pub const PROJECTION_BATCH_RESPONSE_SCHEMA_ID: &str =
    "agent.semantic-protocols.provider-language-projection-batch-response";
pub const CANONICAL_LANGUAGE_ITEM_IDENTITY_SCHEMA_ID: &str =
    "agent.semantic-protocols.canonical-language-item-identity";
pub const PROJECTION_DIAGNOSTIC_SCHEMA_ID: &str =
    "agent.semantic-protocols.provider-language-projection-diagnostic";
pub const SOURCE_SYNTAX_UNAVAILABLE_REASON_KIND: &str = "source-syntax-unavailable";
pub const MAX_PROVIDER_PROJECTION_DIAGNOSTIC_CHARS: usize = 4096;
/// Maximum owners admitted to one provider projection wire frame.
pub const MAX_PROVIDER_PROJECTION_BATCH_OWNERS: usize = 32;
/// Maximum immutable config owners admitted alongside one source-owner frame.
pub const MAX_PROVIDER_PROJECTION_BATCH_AUXILIARY_OWNERS: usize = 128;
/// Maximum aggregate unencoded source bytes admitted to one provider projection wire frame.
///
/// The shared client-server contract deliberately stays below the smallest
/// supported HTTP request-body ceiling.  A generation is a stream of these
/// independently validated frames; it is never serialized as one corpus-sized
/// request.  The HTTP transport separately validates the final encoded frame.
pub const MAX_PROVIDER_PROJECTION_BATCH_SOURCE_BYTES: usize = 384 * 1024;
/// Maximum source bytes for a frame containing exactly one indivisible owner.
///
/// Native parsers require a complete owner.  Large generated sources therefore
/// travel alone instead of forcing the entire generation into one request.  The
/// ceiling matches the transport's bounded streamed-operation budget; the
/// HTTP actor splits the encoded request into independently admitted chunks.
pub const MAX_PROVIDER_PROJECTION_SINGLE_OWNER_SOURCE_BYTES: usize = 16 * 1024 * 1024;

/// Plans ordered provider wire frames without splitting an individual owner.
pub fn provider_projection_batch_ranges(owner_sizes: &[usize]) -> Vec<Range<usize>> {
    provider_projection_batch_ranges_with_auxiliary_bytes(owner_sizes, 0)
}

/// Plans frames while reserving the repeated immutable config context bytes.
pub fn provider_projection_batch_ranges_with_auxiliary_bytes(
    owner_sizes: &[usize],
    auxiliary_source_bytes: usize,
) -> Vec<Range<usize>> {
    let mut ranges = Vec::new();
    let mut start = 0;
    while start < owner_sizes.len() {
        let mut end = start;
        let mut source_bytes = auxiliary_source_bytes;
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
    pub auxiliary_owners: Vec<ProviderProjectionOwner>,
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
    pub projection_state: ProviderProjectionState,
    pub diagnostic: Option<ProviderProjectionDiagnostic>,
    pub items: Vec<ProviderProjectedItem>,
    pub relations: Vec<
        agent_semantic_content_identity::provider_projection_relation::ProviderProjectedRelation,
    >,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, Eq, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub enum ProviderProjectionState {
    Ready,
    SyntaxUnavailable,
}

#[derive(Debug, Clone, Deserialize, Serialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProviderProjectionDiagnostic {
    pub schema_id: String,
    pub schema_version: String,
    pub reason_kind: String,
    pub message: String,
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
        let owners = self.owners.iter().map(encode_owner).collect::<Vec<_>>();
        let auxiliary_owners = self
            .auxiliary_owners
            .iter()
            .map(encode_owner)
            .collect::<Vec<_>>();
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
        if !auxiliary_owners.is_empty() {
            payload["auxiliaryOwners"] = serde_json::Value::Array(auxiliary_owners);
        }
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
        if self.owners.is_empty() || self.owners.len() > MAX_PROVIDER_PROJECTION_BATCH_OWNERS {
            return Err(ProviderProjectionBatchError(format!(
                "projection batch owner count is outside the admitted range: {}",
                self.owners.len()
            )));
        }
        if self.auxiliary_owners.len() > MAX_PROVIDER_PROJECTION_BATCH_AUXILIARY_OWNERS {
            return Err(ProviderProjectionBatchError(format!(
                "projection batch auxiliary owner count exceeds the admitted limit: {}",
                self.auxiliary_owners.len()
            )));
        }
        let mut owner_paths = BTreeSet::new();
        let mut source_bytes = 0usize;
        for owner in &self.owners {
            require_text("ownerPath", &owner.owner_path)?;
            require_text("sourceLeafDigest", &owner.source_leaf_digest)?;
            if !owner_paths.insert(owner.owner_path.as_str()) {
                return Err(ProviderProjectionBatchError(format!(
                    "duplicate projection owner path: {}",
                    owner.owner_path
                )));
            }
            source_bytes = source_bytes.saturating_add(owner.source_bytes.len());
        }
        for owner in &self.auxiliary_owners {
            require_text("auxiliaryOwners.ownerPath", &owner.owner_path)?;
            require_text(
                "auxiliaryOwners.sourceLeafDigest",
                &owner.source_leaf_digest,
            )?;
            if !owner_paths.insert(owner.owner_path.as_str()) {
                return Err(ProviderProjectionBatchError(format!(
                    "duplicate projection or auxiliary owner path: {}",
                    owner.owner_path
                )));
            }
            source_bytes = source_bytes.saturating_add(owner.source_bytes.len());
        }
        let admitted_source_bytes = if self.owners.len() == 1 {
            MAX_PROVIDER_PROJECTION_SINGLE_OWNER_SOURCE_BYTES
        } else {
            MAX_PROVIDER_PROJECTION_BATCH_SOURCE_BYTES
        };
        if source_bytes > admitted_source_bytes {
            return Err(ProviderProjectionBatchError(format!(
                "projection batch immutable source bytes exceed the admitted limit: {source_bytes} > {admitted_source_bytes}"
            )));
        }
        Ok(())
    }
}

fn encode_owner(owner: &ProviderProjectionOwner) -> serde_json::Value {
    let mut payload = serde_json::json!({
        "ownerPath": owner.owner_path,
        "sourceLeafDigest": owner.source_leaf_digest,
    });
    if let Ok(source_text) = std::str::from_utf8(&owner.source_bytes) {
        payload["sourceEncoding"] = serde_json::Value::String("utf8".to_string());
        payload["sourceText"] = serde_json::Value::String(source_text.to_string());
    } else {
        payload["sourceEncoding"] = serde_json::Value::String("base64".to_string());
        payload["sourceBytesBase64"] =
            serde_json::Value::String(BASE64_STANDARD.encode(&owner.source_bytes));
    }
    payload
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
        validate_owner_projection_state(projected_owner)?;
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
            let canonical_owner_path = canonical.owner_path().map_err(|error| {
                ProviderProjectionBatchError(format!(
                    "projection batch selector owner is not canonical: ownerPath={} itemId={} error={error}",
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
            if canonical_owner_path != projected_owner.owner_path {
                proof_mismatches.push("selector.ownerPath");
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

fn validate_owner_projection_state(
    owner: &ProviderProjectedOwner,
) -> Result<(), ProviderProjectionBatchError> {
    match (&owner.projection_state, &owner.diagnostic) {
        (ProviderProjectionState::Ready, None) => Ok(()),
        (ProviderProjectionState::Ready, Some(_)) => Err(ProviderProjectionBatchError(format!(
            "ready projection owner carries a diagnostic: ownerPath={}",
            owner.owner_path
        ))),
        (ProviderProjectionState::SyntaxUnavailable, None) => {
            Err(ProviderProjectionBatchError(format!(
                "syntax-unavailable projection owner omitted its diagnostic: ownerPath={}",
                owner.owner_path
            )))
        }
        (ProviderProjectionState::SyntaxUnavailable, Some(diagnostic)) => {
            if !owner.items.is_empty() || !owner.relations.is_empty() {
                return Err(ProviderProjectionBatchError(format!(
                    "syntax-unavailable projection owner carries semantic facts: ownerPath={}",
                    owner.owner_path
                )));
            }
            if diagnostic.schema_id != PROJECTION_DIAGNOSTIC_SCHEMA_ID
                || diagnostic.schema_version != "1"
                || diagnostic.reason_kind != SOURCE_SYNTAX_UNAVAILABLE_REASON_KIND
                || diagnostic.message.trim().is_empty()
                || diagnostic.message.chars().count() > MAX_PROVIDER_PROJECTION_DIAGNOSTIC_CHARS
            {
                return Err(ProviderProjectionBatchError(format!(
                    "syntax-unavailable projection owner has an invalid diagnostic: ownerPath={}",
                    owner.owner_path
                )));
            }
            Ok(())
        }
    }
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
