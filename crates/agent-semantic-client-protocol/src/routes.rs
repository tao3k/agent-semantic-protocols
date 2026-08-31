//! Typed provider route request and response bindings for the ASP Client Protocol.

use serde::{Deserialize, Serialize};
use serde_json::Value;

const SEARCH_REQUEST: &str = "agent.semantic-protocols.runtime-provider-search-request";
const CLIENT_SEARCH_REQUEST: &str = "agent.semantic-protocols.asp-client-search-request";
const CLIENT_SOURCE_INDEX_LOOKUP_REQUEST: &str =
    "agent.semantic-protocols.asp-client-source-index-lookup-request";
const CLIENT_EXACT_QUERY_REQUEST: &str = "agent.semantic-protocols.asp-client-exact-query-request";
const CLIENT_EXACT_QUERY_RESPONSE: &str =
    "agent.semantic-protocols.asp-client-exact-query-response";
const CLIENT_EXACT_QUERY_FAILURE: &str = "agent.semantic-protocols.asp-client-exact-query-failure";
const CLIENT_OWNER_SEARCH_REQUEST: &str =
    "agent.semantic-protocols.asp-client-owner-search-request";
const CLIENT_OWNER_SEARCH_RESPONSE: &str =
    "agent.semantic-protocols.asp-client-owner-search-response";
const CLIENT_GRAPHS_TIMELINE_REQUEST: &str =
    "agent.semantic-protocols.asp-client-graphs-timeline-request";
const EXACT_REQUEST: &str = "agent.semantic-protocols.provider-native-exact-request";
const EXACT_RESPONSE: &str = "agent.semantic-protocols.provider-native-exact-projection";
const OWNER_REQUEST: &str = "agent.semantic-protocols.provider-native-owner-search-request";
const OWNER_RESPONSE: &str = "agent.semantic-protocols.provider-native-owner-search-response";

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AspClientSearchRequest {
    pub schema_id: String,
    pub schema_version: String,
    pub operation: String,
    pub query: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AspClientSourceIndexLookupRequest {
    pub schema_id: String,
    pub schema_version: String,
    pub query: String,
    pub index_root: String,
    pub limit: u32,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AspClientExactQueryRequest {
    pub schema_id: String,
    pub schema_version: String,
    pub selector: String,
    pub projection: String,
}

/// Server-owned graph timeline request used by history/audit clients.
///
/// Graph-Turbo is an algorithm identity carried by the event packet; the
/// transport and lifecycle owner is ASP Server's `asp-python-graphs` service.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AspClientGraphsTimelineRequest {
    pub schema_id: String,
    pub schema_version: String,
    pub event_packet: Value,
    pub arguments: Vec<String>,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AspClientRuntimeWorkCounters {
    pub database_read_count: u64,
    pub filesystem_read_count: u64,
    pub provider_process_count: u64,
    pub scheduler_task_count: u64,
    pub socket_operation_count: u64,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AspClientExactQueryResponse {
    pub schema_id: String,
    pub schema_version: String,
    pub operation_id: String,
    pub language_id: String,
    pub provider_id: String,
    pub generation_digest: String,
    pub root_digest: String,
    pub result: Value,
    pub resident_read_elapsed_micros: u64,
    pub service_elapsed_micros: u64,
    pub elapsed_micros: u64,
    pub work_counters: AspClientRuntimeWorkCounters,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AspClientExactQueryFailure {
    pub schema_id: String,
    pub schema_version: String,
    pub state: String,
    pub operation_id: String,
    pub language_id: String,
    pub provider_id: String,
    pub requested_selector: Option<String>,
    pub resolved_selector: Option<String>,
    pub projection_kind: Option<String>,
    pub phase: String,
    pub reason_kind: String,
    pub generation_digest: Option<String>,
    pub root_digest: Option<String>,
    pub recommended_next: Value,
    pub resident_read_elapsed_micros: u64,
    pub service_elapsed_micros: u64,
    pub elapsed_micros: u64,
    pub work_counters: AspClientRuntimeWorkCounters,
    pub details: Value,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AspClientOwnerSearchRequest {
    pub schema_id: String,
    pub schema_version: String,
    pub owner_path: String,
    pub query: String,
    pub view: String,
}

/// Compact structural seed returned by owner-local search.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AspClientOwnerSearchSeed {
    pub selector: String,
    pub byte_start: usize,
    pub byte_end: usize,
}

/// Versioned bounded owner-local search response.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AspClientOwnerSearchResponse {
    pub schema_id: String,
    pub schema_version: String,
    pub state: String,
    pub generation_digest: String,
    pub root_digest: String,
    pub owner_path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content_digest: Option<String>,
    pub query: String,
    pub view: String,
    pub candidate_count: usize,
    pub returned_count: usize,
    pub selectors: Vec<AspClientOwnerSearchSeed>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RuntimeProviderSearchRequest {
    pub schema_id: String,
    pub schema_version: String,
    pub operation_id: String,
    pub workspace_identity: String,
    pub language_id: String,
    pub scope: String,
    pub query_plan: Value,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProviderNativeExactRequest {
    pub schema_id: String,
    pub schema_version: String,
    pub language_id: String,
    pub provider_id: String,
    pub projection_kind: String,
    pub structural_selector: String,
    pub owner_path: String,
    pub generation_identity_digest: String,
    pub parser_identity_digest: String,
    pub query_pack_digest: String,
    pub source_digest: String,
    pub source_byte_length: usize,
    pub source_encoding: String,
    pub source_bytes_base64: String,
    pub transport: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProviderNativeExactProjection {
    pub schema_id: String,
    pub schema_version: String,
    pub language_id: String,
    pub provider_id: String,
    pub owner_path: String,
    pub requested_structural_selector: String,
    #[serde(default)]
    pub resolution_state: Option<String>,
    #[serde(default)]
    pub reason_kind: Option<String>,
    #[serde(default)]
    pub active_generation_digest: Option<String>,
    #[serde(default)]
    pub root_digest: Option<String>,
    #[serde(default)]
    pub item_kind: Option<String>,
    #[serde(default)]
    pub item_name: Option<String>,
    #[serde(default)]
    pub candidates: Option<Vec<String>>,
    #[serde(default)]
    pub actual_kinds: Option<Vec<String>>,
    #[serde(default)]
    pub recommended_next: Option<Value>,
    #[serde(default)]
    pub structural_selector: Option<String>,
    #[serde(default)]
    pub projection_mode: Option<String>,
    #[serde(default)]
    pub normalized_parser_facts: Option<Value>,
    #[serde(default)]
    pub projection_text: Option<String>,
    #[serde(default)]
    pub projection_payload: Option<Value>,
    #[serde(default)]
    pub source_content_digest: Option<String>,
    #[serde(default)]
    pub source_byte_start: Option<usize>,
    #[serde(default)]
    pub source_byte_end: Option<usize>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProviderNativeOwnerSearchRequest {
    pub schema_id: String,
    pub schema_version: String,
    pub language_id: String,
    pub provider_id: String,
    pub workspace_identity: String,
    pub provider_workspace_identity_digest: String,
    pub owner_path: String,
    pub source_fingerprint: Value,
    pub source_encoding: String,
    pub source_bytes_base64: String,
    pub projection_mode: String,
    pub transport: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProviderNativeOwnerSearchResponse {
    pub schema_id: String,
    pub schema_version: String,
    pub language_id: String,
    pub provider_id: String,
    pub requested_owner_path: String,
    pub requested_projection_mode: String,
    pub source_content_digest: String,
    pub parsed_owner_count: usize,
    pub projection_completeness: String,
    pub projections: Vec<Value>,
}

fn check(id: &str, schema_id: &str, schema_version: &str) -> Result<(), String> {
    if id != schema_id {
        return Err(format!(
            "route schema identity drift: expected={schema_id} actual={id}"
        ));
    }
    if schema_version != "1" {
        return Err(format!(
            "route schema version unsupported: {schema_version}"
        ));
    }
    Ok(())
}

macro_rules! validate_schema_identity {
    ($name:ident, $id:expr) => {
        impl $name {
            pub fn validate_schema_identity(&self) -> Result<(), String> {
                check(&self.schema_id, $id, &self.schema_version)
            }
        }
    };
}
validate_schema_identity!(RuntimeProviderSearchRequest, SEARCH_REQUEST);
validate_schema_identity!(AspClientSearchRequest, CLIENT_SEARCH_REQUEST);
validate_schema_identity!(
    AspClientSourceIndexLookupRequest,
    CLIENT_SOURCE_INDEX_LOOKUP_REQUEST
);
validate_schema_identity!(AspClientExactQueryRequest, CLIENT_EXACT_QUERY_REQUEST);
validate_schema_identity!(AspClientOwnerSearchRequest, CLIENT_OWNER_SEARCH_REQUEST);
validate_schema_identity!(AspClientOwnerSearchResponse, CLIENT_OWNER_SEARCH_RESPONSE);
validate_schema_identity!(
    AspClientGraphsTimelineRequest,
    CLIENT_GRAPHS_TIMELINE_REQUEST
);
validate_schema_identity!(ProviderNativeExactRequest, EXACT_REQUEST);
validate_schema_identity!(ProviderNativeExactProjection, EXACT_RESPONSE);
validate_schema_identity!(ProviderNativeOwnerSearchRequest, OWNER_REQUEST);
validate_schema_identity!(ProviderNativeOwnerSearchResponse, OWNER_RESPONSE);

impl AspClientOwnerSearchResponse {
    pub fn validate(&self) -> Result<(), String> {
        self.validate_schema_identity()?;
        if self.owner_path.trim().is_empty()
            || self.generation_digest.trim().is_empty()
            || self.root_digest.trim().is_empty()
            || self.view != "seeds"
        {
            return Err("owner-search response identity is incomplete".to_owned());
        }
        if self.returned_count != self.selectors.len()
            || self.returned_count > 100
            || self.returned_count > self.candidate_count
        {
            return Err("owner-search response counts are inconsistent".to_owned());
        }
        match self.state.as_str() {
            "owner" if self.content_digest.is_some() => {}
            "owner-missing"
                if self.content_digest.is_none()
                    && self.candidate_count == 0
                    && self.selectors.is_empty() => {}
            _ => return Err("owner-search response state is inconsistent".to_owned()),
        }
        if self
            .selectors
            .iter()
            .any(|seed| seed.selector.trim().is_empty() || seed.byte_end < seed.byte_start)
            || self
                .selectors
                .windows(2)
                .any(|pair| pair[0].selector >= pair[1].selector)
        {
            return Err("owner-search response selector seeds are not canonical".to_owned());
        }
        Ok(())
    }
}

impl AspClientExactQueryResponse {
    pub fn validate(&self) -> Result<(), String> {
        check(
            &self.schema_id,
            CLIENT_EXACT_QUERY_RESPONSE,
            &self.schema_version,
        )?;
        if self.operation_id.trim().is_empty()
            || self.language_id.trim().is_empty()
            || self.provider_id.trim().is_empty()
        {
            return Err("exact-query response identity fields must be non-empty".to_owned());
        }
        validate_digest("generationDigest", &self.generation_digest, true)?;
        validate_digest("rootDigest", &self.root_digest, false)?;
        if !self.result.is_object() {
            return Err("exact-query response result must be an object".to_owned());
        }
        let state = self
            .result
            .get("state")
            .and_then(Value::as_str)
            .ok_or_else(|| "exact-query response result state is missing".to_owned())?;
        match state {
            "projection" | "provider-projection" => {
                let bytes = self
                    .result
                    .get("bytes")
                    .and_then(Value::as_array)
                    .ok_or_else(|| "exact-query Ready result has no byte payload".to_owned())?;
                if bytes.is_empty() {
                    return Err("exact-query Ready result has an empty byte payload".to_owned());
                }
            }
            _ => {
                return Err(format!(
                    "exact-query response result state {state} is not terminal"
                ));
            }
        }
        if self.elapsed_micros
            != self
                .resident_read_elapsed_micros
                .saturating_add(self.service_elapsed_micros)
        {
            return Err("exact-query response elapsedMicros is inconsistent".to_owned());
        }
        Ok(())
    }
}

impl AspClientExactQueryFailure {
    pub fn validate(&self) -> Result<(), String> {
        check(
            &self.schema_id,
            CLIENT_EXACT_QUERY_FAILURE,
            &self.schema_version,
        )?;
        if self.state != "failed" {
            return Err("exact-query failure state must be failed".to_owned());
        }
        if self.operation_id.trim().is_empty()
            || self.language_id.trim().is_empty()
            || self.provider_id.trim().is_empty()
            || self.phase.trim().is_empty()
            || self.reason_kind.trim().is_empty()
        {
            return Err("exact-query failure identity fields must be non-empty".to_owned());
        }
        if let Some(generation_digest) = &self.generation_digest {
            validate_digest("generationDigest", generation_digest, true)?;
        }
        if let Some(root_digest) = &self.root_digest {
            validate_digest("rootDigest", root_digest, false)?;
        }
        if self.recommended_next.is_null() || !self.details.is_object() {
            return Err(
                "exact-query failure requires recommendedNext and object details".to_owned(),
            );
        }
        if self.elapsed_micros
            != self
                .resident_read_elapsed_micros
                .saturating_add(self.service_elapsed_micros)
        {
            return Err("exact-query failure elapsedMicros is inconsistent".to_owned());
        }
        Ok(())
    }
}

fn validate_digest(field: &str, value: &str, algorithm_prefix: bool) -> Result<(), String> {
    let hex = if algorithm_prefix {
        value
            .strip_prefix("blake3-256:")
            .ok_or_else(|| format!("exact-query response {field} uses an unsupported digest"))?
    } else {
        value
    };
    if hex.len() != 64
        || !hex
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    {
        return Err(format!("exact-query response {field} is invalid"));
    }
    Ok(())
}
