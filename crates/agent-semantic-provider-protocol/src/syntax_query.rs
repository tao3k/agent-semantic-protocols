use serde::Deserialize;
use serde::Serialize;

pub const PROVIDER_SYNTAX_QUERY_OPERATION: &str = "syntax-query";
pub const PROVIDER_SYNTAX_QUERY_REQUEST_SCHEMA_ID: &str =
    "agent.semantic-protocols.provider-syntax-query-request";
pub const PROVIDER_SYNTAX_QUERY_RESPONSE_SCHEMA_ID: &str =
    "agent.semantic-protocols.provider-syntax-query-response";

#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SyntaxQueryPlan {
    pub patterns: Vec<SyntaxQueryPattern>,
    pub captures: Vec<String>,
    pub node_types: Vec<String>,
    pub fields: Vec<String>,
    pub predicates: Vec<SyntaxQueryPredicate>,
}

impl SyntaxQueryPlan {
    #[must_use]
    pub fn pattern_count(&self) -> usize {
        self.patterns.len()
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SyntaxQueryPattern {
    pub index: usize,
    pub captures: Vec<String>,
    pub node_types: Vec<String>,
    pub fields: Vec<String>,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum SyntaxQueryPredicateOp {
    Eq,
    AnyEq,
    AnyOf,
    Match,
    AnyMatch,
    NotEq,
    NotMatch,
}

impl SyntaxQueryPredicateOp {
    #[must_use]
    pub fn as_abi_str(&self) -> &'static str {
        match self {
            Self::Eq => "eq",
            Self::AnyEq => "any-eq",
            Self::AnyOf => "any-of",
            Self::Match => "match",
            Self::AnyMatch => "any-match",
            Self::NotEq => "not-eq",
            Self::NotMatch => "not-match",
        }
    }
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Deserialize, Serialize)]
#[serde(tag = "kind", content = "value", rename_all = "camelCase")]
pub enum SyntaxQueryPredicateValue {
    String(String),
    Capture(String),
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SyntaxQueryPredicate {
    pub op: SyntaxQueryPredicateOp,
    pub capture: String,
    pub values: Vec<SyntaxQueryPredicateValue>,
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProviderSyntaxQueryRequest {
    pub schema_id: String,
    pub schema_version: String,
    pub language_id: String,
    pub provider_id: String,
    pub owner_path: String,
    pub source_content_digest: String,
    pub query_digest: String,
    pub source: String,
    pub plan: SyntaxQueryPlan,
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProviderSyntaxQueryResponse {
    pub schema_id: String,
    pub schema_version: String,
    pub language_id: String,
    pub provider_id: String,
    pub owner_path: String,
    pub source_content_digest: String,
    pub query_digest: String,
    pub parsed: bool,
    pub captures: Vec<ProviderSyntaxQueryCapture>,
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProviderSyntaxQueryCapture {
    pub pattern_index: usize,
    pub capture_name: String,
    pub native_fact_ref: String,
    pub source_byte_start: u64,
    pub source_byte_end: u64,
}
