//! Closed provider method argument projection.

use serde::Deserialize;

/// Typed facade values that may be projected into provider-native argv.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ProviderMethodArgumentValuesV1 {
    /// Facade query terms in their canonical deterministic order.
    pub query: Option<String>,
    /// Authoritatively resolved workspace root.
    pub workspace: Option<String>,
    /// Facade presentation mode, such as `seeds`.
    pub presentation: Option<String>,
    /// Explicit owner scope; lexical methods never consume this field.
    pub owner: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct ProviderMethodArgumentProjectionV1 {
    pub(crate) schema_version: String,
    pub(crate) tokens: Vec<ProviderMethodArgumentTokenV1>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub(crate) enum ProviderMethodArgumentTokenV1 {
    Literal(ProviderMethodLiteralArgumentTokenV1),
    Slot(ProviderMethodSlotArgumentTokenV1),
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct ProviderMethodLiteralArgumentTokenV1 {
    pub(crate) value: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct ProviderMethodSlotArgumentTokenV1 {
    pub(crate) name: ProviderMethodArgumentSlotNameV1,
    pub(crate) value_type: ProviderMethodArgumentValueTypeV1,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub(crate) enum ProviderMethodArgumentSlotNameV1 {
    Query,
    Workspace,
    Presentation,
    Owner,
}

impl ProviderMethodArgumentSlotNameV1 {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Query => "query",
            Self::Workspace => "workspace",
            Self::Presentation => "presentation",
            Self::Owner => "owner",
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum ProviderMethodArgumentValueTypeV1 {
    String,
    Path,
    Presentation,
}
