//! Language-neutral semantic route declarations compiled by ASP Server.

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt::{Display, Formatter};

use serde::{Deserialize, Serialize};

pub const PROVIDER_ROUTE_SCHEMA_ID: &str = "agent.semantic-protocols.provider-route";
pub const PROVIDER_ROUTE_SCHEMA_VERSION: &str = "1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProviderRouteSpec {
    pub schema_id: String,
    pub schema_version: String,
    pub route_id: String,
    pub operation: String,
    pub authority: ProviderRouteAuthority,
    pub target: ProviderRouteTarget,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub request_schema_id: Option<String>,
    pub inputs: Vec<ProviderRouteInputSlot>,
    pub requirements: Vec<ProviderRouteRequirement>,
    pub effects: ProviderRouteEffects,
    pub output: ProviderRouteOutput,
    pub failure_schema_ids: Vec<String>,
    pub cache: ProviderRouteCache,
    pub telemetry: ProviderRouteTelemetry,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ProviderRouteAuthority {
    #[default]
    AspServer,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProviderRouteTarget {
    pub language_id: String,
    pub provider_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProviderRouteInputSlot {
    pub name: String,
    pub value_type: ProviderRouteValueType,
    pub cardinality: ProviderRouteCardinality,
    pub source: ProviderRouteInputSource,
    #[serde(default)]
    pub telemetry: ProviderRouteTelemetryPolicy,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ProviderRouteValueType {
    String,
    WorkspaceRelativePath,
    StructuralSelector,
    Presentation,
    Boolean,
    UnsignedInteger,
    Json,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ProviderRouteCardinality {
    #[default]
    Required,
    Optional,
    Many,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ProviderRouteInputSource {
    Request,
    RuntimeContext,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ProviderRouteTelemetryPolicy {
    #[default]
    Omit,
    Identity,
    Measurement,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub enum ProviderRouteRequirement {
    Capability { capability: String },
    State { state: ProviderRouteRequiredState },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ProviderRouteRequiredState {
    ProviderReady,
    TerminalGeneration,
    LiveOwner,
    RegisteredWorkspace,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProviderRouteEffects {
    pub access: ProviderRouteAccess,
    pub idempotent: bool,
    pub cancellable: bool,
    pub concurrency: ProviderRouteConcurrency,
    pub streaming: bool,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ProviderRouteAccess {
    #[default]
    Read,
    Write,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ProviderRouteConcurrency {
    Isolated,
    #[default]
    SharedRead,
    Exclusive,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProviderRouteOutput {
    pub schema_id: String,
    pub media_type: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub projection_kind: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProviderRouteCache {
    pub authority: ProviderRouteAuthority,
    pub scope: ProviderRouteCacheScope,
    pub key_slots: Vec<String>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ProviderRouteCacheScope {
    #[default]
    None,
    Request,
    Workspace,
    Generation,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProviderRouteTelemetry {
    pub span_name: String,
    pub attribute_slots: Vec<String>,
}

/// Validated server-side route IR. It contains no executable, argv, shell, or
/// transport representation; those are selected by ASP Server after admission.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompiledProviderRoute {
    spec: ProviderRouteSpec,
    input_slots: BTreeMap<String, ProviderRouteInputSlot>,
}

impl CompiledProviderRoute {
    #[must_use]
    pub fn spec(&self) -> &ProviderRouteSpec {
        &self.spec
    }

    #[must_use]
    pub fn input_slot(&self, name: &str) -> Option<&ProviderRouteInputSlot> {
        self.input_slots.get(name)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderRouteCompileError {
    message: String,
}

impl ProviderRouteCompileError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl Display for ProviderRouteCompileError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl Error for ProviderRouteCompileError {}

impl ProviderRouteSpec {
    pub fn compile(self) -> Result<CompiledProviderRoute, ProviderRouteCompileError> {
        require_equal("schemaId", &self.schema_id, PROVIDER_ROUTE_SCHEMA_ID)?;
        require_equal(
            "schemaVersion",
            &self.schema_version,
            PROVIDER_ROUTE_SCHEMA_VERSION,
        )?;
        require_dotted_id("routeId", &self.route_id)?;
        require_semantic_id("operation", &self.operation)?;
        require_kebab_id("languageId", &self.target.language_id)?;
        if self
            .target
            .provider_id
            .strip_prefix("asp-")
            .is_none_or(|provider| require_kebab_id("providerId", provider).is_err())
        {
            return Err(ProviderRouteCompileError::new(
                "providerId must use the asp-<language> identity",
            ));
        }
        require_schema_id("output.schemaId", &self.output.schema_id)?;
        if let Some(request_schema_id) = &self.request_schema_id {
            require_schema_id("requestSchemaId", request_schema_id)?;
        }
        if self.output.media_type != "application/json" {
            return Err(ProviderRouteCompileError::new(
                "output.mediaType must be `application/json`",
            ));
        }
        if self
            .output
            .projection_kind
            .as_ref()
            .is_some_and(|kind| kind.is_empty())
        {
            return Err(ProviderRouteCompileError::new(
                "output.projectionKind must not be empty",
            ));
        }
        let telemetry_suffix = self.telemetry.span_name.strip_prefix("asp.route.");
        if !telemetry_suffix.is_some_and(|suffix| {
            !suffix.is_empty()
                && suffix.bytes().all(|byte| {
                    byte.is_ascii_lowercase()
                        || byte.is_ascii_digit()
                        || matches!(byte, b'.' | b'-')
                })
        }) {
            return Err(ProviderRouteCompileError::new(
                "telemetry.spanName must start with `asp.route.`",
            ));
        }

        if self.failure_schema_ids.is_empty() {
            return Err(ProviderRouteCompileError::new(
                "failureSchemaIds must not be empty",
            ));
        }
        for schema_id in &self.failure_schema_ids {
            require_schema_id("failureSchemaIds", schema_id)?;
        }
        require_unique("failureSchemaIds", &self.failure_schema_ids)?;
        require_unique("cache.keySlots", &self.cache.key_slots)?;
        require_unique("telemetry.attributeSlots", &self.telemetry.attribute_slots)?;
        for requirement in &self.requirements {
            if let ProviderRouteRequirement::Capability { capability } = requirement {
                require_dotted_id("requirements.capability", capability)?;
            }
        }
        let mut requirements = BTreeSet::new();
        for requirement in &self.requirements {
            if !requirements.insert(requirement) {
                return Err(ProviderRouteCompileError::new(
                    "requirements contains a duplicate requirement",
                ));
            }
        }

        let mut input_slots = BTreeMap::new();
        for slot in &self.inputs {
            require_identifier("inputs.name", &slot.name)?;
            if input_slots
                .insert(slot.name.clone(), slot.clone())
                .is_some()
            {
                return Err(ProviderRouteCompileError::new(format!(
                    "duplicate input slot `{}`",
                    slot.name
                )));
            }
        }

        let mut referenced = BTreeSet::new();
        for name in &self.cache.key_slots {
            referenced.insert(("cache.keySlots", name));
        }
        for name in &self.telemetry.attribute_slots {
            referenced.insert(("telemetry.attributeSlots", name));
        }
        for (owner, name) in referenced {
            let Some(slot) = input_slots.get(name) else {
                return Err(ProviderRouteCompileError::new(format!(
                    "{owner} references unknown input slot `{name}`"
                )));
            };
            if owner == "telemetry.attributeSlots"
                && slot.telemetry == ProviderRouteTelemetryPolicy::Omit
            {
                return Err(ProviderRouteCompileError::new(format!(
                    "telemetry.attributeSlots references omitted input slot `{name}`"
                )));
            }
        }

        Ok(CompiledProviderRoute {
            spec: self,
            input_slots,
        })
    }
}

fn require_equal(
    field: &str,
    actual: &str,
    expected: &str,
) -> Result<(), ProviderRouteCompileError> {
    if actual == expected {
        Ok(())
    } else {
        Err(ProviderRouteCompileError::new(format!(
            "{field} must be `{expected}`, got `{actual}`"
        )))
    }
}

fn require_identifier(field: &str, value: &str) -> Result<(), ProviderRouteCompileError> {
    let valid = value.as_bytes().first().is_some_and(u8::is_ascii_lowercase)
        && value.bytes().all(|byte| byte.is_ascii_alphanumeric());
    if valid {
        Ok(())
    } else {
        Err(ProviderRouteCompileError::new(format!(
            "{field} must be a non-empty semantic identifier"
        )))
    }
}

fn require_kebab_id(field: &str, value: &str) -> Result<(), ProviderRouteCompileError> {
    let valid = value.as_bytes().first().is_some_and(u8::is_ascii_lowercase)
        && value
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-');
    if valid {
        Ok(())
    } else {
        Err(ProviderRouteCompileError::new(format!(
            "{field} must be a lower-case kebab identifier"
        )))
    }
}

fn require_semantic_id(field: &str, value: &str) -> Result<(), ProviderRouteCompileError> {
    if value.split('.').all(|segment| {
        !segment.is_empty()
            && segment.as_bytes()[0].is_ascii_lowercase()
            && segment
                .bytes()
                .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
    }) {
        Ok(())
    } else {
        Err(ProviderRouteCompileError::new(format!(
            "{field} must be a semantic identifier"
        )))
    }
}

fn require_dotted_id(field: &str, value: &str) -> Result<(), ProviderRouteCompileError> {
    if value.contains('.')
        && value.split('.').all(|segment| {
            !segment.is_empty()
                && segment.as_bytes()[0].is_ascii_lowercase()
                && segment
                    .bytes()
                    .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
        })
    {
        Ok(())
    } else {
        Err(ProviderRouteCompileError::new(format!(
            "{field} must be a dotted semantic identifier"
        )))
    }
}

fn require_schema_id(field: &str, value: &str) -> Result<(), ProviderRouteCompileError> {
    let valid = value
        .as_bytes()
        .first()
        .is_some_and(u8::is_ascii_alphabetic)
        && value.bytes().all(|byte| {
            byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b':' | b'/' | b'-')
        });
    if valid {
        Ok(())
    } else {
        Err(ProviderRouteCompileError::new(format!(
            "{field} must be a semantic schema identifier"
        )))
    }
}

fn require_unique(field: &str, values: &[String]) -> Result<(), ProviderRouteCompileError> {
    let mut seen = BTreeSet::new();
    for value in values {
        if !seen.insert(value) {
            return Err(ProviderRouteCompileError::new(format!(
                "{field} contains duplicate value `{value}`"
            )));
        }
    }
    Ok(())
}

#[cfg(test)]
mod semantic_identifier_tests {
    use super::{require_dotted_id, require_semantic_id};

    #[test]
    fn provider_operations_accept_single_or_dotted_semantic_ids() {
        for operation in ["query", "search", "projection-batch", "search.owner"] {
            require_semantic_id("operation", operation).unwrap();
        }
    }

    #[test]
    fn route_ids_remain_globally_dotted() {
        assert!(require_dotted_id("routeId", "query").is_err());
        require_dotted_id("routeId", "rust.query").unwrap();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn route() -> ProviderRouteSpec {
        ProviderRouteSpec {
            schema_id: PROVIDER_ROUTE_SCHEMA_ID.into(),
            schema_version: PROVIDER_ROUTE_SCHEMA_VERSION.into(),
            route_id: "rust.search.owner".into(),
            operation: "search.owner".into(),
            authority: ProviderRouteAuthority::AspServer,
            target: ProviderRouteTarget {
                language_id: "rust".into(),
                provider_id: "asp-rust".into(),
            },
            request_schema_id: Some("agent.semantic-protocols.search-owner-request".into()),
            inputs: vec![ProviderRouteInputSlot {
                name: "query".into(),
                value_type: ProviderRouteValueType::String,
                cardinality: ProviderRouteCardinality::Required,
                source: ProviderRouteInputSource::Request,
                telemetry: ProviderRouteTelemetryPolicy::Identity,
            }],
            requirements: vec![ProviderRouteRequirement::State {
                state: ProviderRouteRequiredState::ProviderReady,
            }],
            effects: ProviderRouteEffects {
                access: ProviderRouteAccess::Read,
                idempotent: true,
                cancellable: true,
                concurrency: ProviderRouteConcurrency::SharedRead,
                streaming: false,
            },
            output: ProviderRouteOutput {
                schema_id: "agent.semantic-protocols.search-packet".into(),
                media_type: "application/json".into(),
                projection_kind: None,
            },
            failure_schema_ids: vec!["agent.semantic-protocols.route-failure".into()],
            cache: ProviderRouteCache {
                authority: ProviderRouteAuthority::AspServer,
                scope: ProviderRouteCacheScope::Workspace,
                key_slots: vec!["query".into()],
            },
            telemetry: ProviderRouteTelemetry {
                span_name: "asp.route.provider".into(),
                attribute_slots: vec!["query".into()],
            },
        }
    }

    #[test]
    fn compiles_a_semantic_route_without_an_argv_projection() {
        let compiled = route().compile().expect("route should compile");
        assert_eq!(compiled.spec().route_id, "rust.search.owner");
        assert!(compiled.input_slot("query").is_some());
    }

    #[test]
    fn rejects_unknown_cache_slots() {
        let mut route = route();
        route.cache.key_slots = vec!["missing".into()];
        assert!(
            route
                .compile()
                .unwrap_err()
                .to_string()
                .contains("unknown input slot")
        );
    }

    #[test]
    fn rejects_telemetry_for_omitted_slots() {
        let mut route = route();
        route.inputs[0].telemetry = ProviderRouteTelemetryPolicy::Omit;
        assert!(
            route
                .compile()
                .unwrap_err()
                .to_string()
                .contains("omitted input slot")
        );
    }

    #[test]
    fn rejects_duplicate_slots() {
        let mut route = route();
        route.inputs.push(route.inputs[0].clone());
        assert!(
            route
                .compile()
                .unwrap_err()
                .to_string()
                .contains("duplicate input slot")
        );
    }
}
