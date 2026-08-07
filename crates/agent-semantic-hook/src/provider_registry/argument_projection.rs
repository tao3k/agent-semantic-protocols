//! Closed provider method argument projection.

use serde::Deserialize;

use super::catalog::schema_registry;

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

/// Materialize deterministic provider argv from a registered closed projection.
pub fn registered_provider_method_projected_argv_v1(
    language_id: &str,
    provider_id: &str,
    method: &str,
    values: &ProviderMethodArgumentValuesV1,
) -> Result<Vec<String>, String> {
    let unavailable = |detail: &str| {
        format!(
            "reasonKind=provider-native-argument-projection-unavailable languageId={language_id} providerId={provider_id} method={method} detail={detail}"
        )
    };
    let registry = schema_registry();
    let language = registry
        .languages
        .iter()
        .find(|language| language.language_id == language_id)
        .ok_or_else(|| unavailable("language-not-registered"))?;
    if language.provider_id != provider_id {
        return Err(unavailable("provider-identity-drift"));
    }
    let descriptor = language
        .method_descriptors
        .iter()
        .find(|descriptor| descriptor.method == method)
        .ok_or_else(|| unavailable("method-not-registered"))?;
    let projection = descriptor
        .argument_projection
        .as_ref()
        .ok_or_else(|| unavailable("projection-not-declared"))?;
    if projection.schema_version != "1" {
        return Err(unavailable("unsupported-schema-version"));
    }

    projection
        .tokens
        .iter()
        .map(|token| match token {
            ProviderMethodArgumentTokenV1::Literal(literal) => Ok(literal.value.clone()),
            ProviderMethodArgumentTokenV1::Slot(slot) => {
                let value = match (&slot.name, &slot.value_type) {
                    (
                        ProviderMethodArgumentSlotNameV1::Query,
                        ProviderMethodArgumentValueTypeV1::String,
                    ) => values.query.as_ref(),
                    (
                        ProviderMethodArgumentSlotNameV1::Workspace,
                        ProviderMethodArgumentValueTypeV1::Path,
                    ) => values.workspace.as_ref(),
                    (
                        ProviderMethodArgumentSlotNameV1::Presentation,
                        ProviderMethodArgumentValueTypeV1::Presentation,
                    ) => values.presentation.as_ref(),
                    (
                        ProviderMethodArgumentSlotNameV1::Owner,
                        ProviderMethodArgumentValueTypeV1::Path,
                    ) if method.starts_with("search/owner") => values.owner.as_ref(),
                    _ => return Err(unavailable("slot-type-or-method-mismatch")),
                };
                value
                    .filter(|value| !value.is_empty())
                    .cloned()
                    .ok_or_else(|| unavailable(&format!("missing-slot-{}", slot.name.as_str())))
            }
        })
        .collect()
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
    value: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct ProviderMethodSlotArgumentTokenV1 {
    pub(crate) name: ProviderMethodArgumentSlotNameV1,
    value_type: ProviderMethodArgumentValueTypeV1,
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
    fn as_str(self) -> &'static str {
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
