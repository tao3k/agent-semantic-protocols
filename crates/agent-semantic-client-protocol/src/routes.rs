//! Typed provider route request and response bindings for the ASP Client Protocol.

use serde::{Deserialize, Serialize};
use serde_json::Value;

const SEARCH_REQUEST: &str = "agent.semantic-protocols.runtime-provider-search-request";
const CLIENT_SEARCH_REQUEST: &str = "agent.semantic-protocols.asp-client-search-request";
const CLIENT_EXACT_QUERY_REQUEST: &str = "agent.semantic-protocols.asp-client-exact-query-request";
const CLIENT_OWNER_SEARCH_REQUEST: &str =
    "agent.semantic-protocols.asp-client-owner-search-request";
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
pub struct AspClientExactQueryRequest {
    pub schema_id: String,
    pub schema_version: String,
    pub selector: String,
    pub projection: String,
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
validate_schema_identity!(AspClientExactQueryRequest, CLIENT_EXACT_QUERY_REQUEST);
validate_schema_identity!(AspClientOwnerSearchRequest, CLIENT_OWNER_SEARCH_REQUEST);
validate_schema_identity!(ProviderNativeExactRequest, EXACT_REQUEST);
validate_schema_identity!(ProviderNativeExactProjection, EXACT_RESPONSE);
validate_schema_identity!(ProviderNativeOwnerSearchRequest, OWNER_REQUEST);
validate_schema_identity!(ProviderNativeOwnerSearchResponse, OWNER_RESPONSE);
