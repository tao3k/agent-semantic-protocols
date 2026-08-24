#![deny(dead_code)]

//! Language-neutral Provider Registration Protocol shared by ASP Server and providers.

mod install_register;
mod register;
mod route;
mod syntax_query;
mod workspace_install;
pub use install_register::*;

pub const PROVIDER_STREAM_SCHEMA_ID: &str = "agent.semantic-protocols.provider-stream";
pub const PROVIDER_STREAM_SCHEMA_VERSION: &str = "1";

pub fn validate_provider_stream_envelope(
    schema_id: &str,
    schema_version: &str,
    session_id: &str,
    request_id: &str,
    workspace_identity: &str,
    generation_digest: &str,
    provider_id: &str,
    language_id: &str,
    kind: &str,
    payload_schema_id: &str,
) -> Result<(), String> {
    if schema_id != PROVIDER_STREAM_SCHEMA_ID || schema_version != PROVIDER_STREAM_SCHEMA_VERSION {
        return Err("invalid provider stream schema identity".to_owned());
    }
    for (name, value) in [
        ("sessionId", session_id),
        ("requestId", request_id),
        ("workspaceIdentity", workspace_identity),
        ("generationDigest", generation_digest),
        ("providerId", provider_id),
        ("languageId", language_id),
        ("kind", kind),
        ("payloadSchemaId", payload_schema_id),
    ] {
        if value.is_empty() {
            return Err(format!("provider stream envelope {name} is empty"));
        }
    }
    Ok(())
}

pub use register::{
    PROVIDER_REGISTER_REQUEST_SCHEMA_ID, PROVIDER_REGISTER_RESPONSE_SCHEMA_ID,
    PROVIDER_REGISTER_SCHEMA_VERSION, ProviderDocumentInventory, ProviderProjectInventory,
    ProviderRegisterOperation, ProviderRegisterRequest, ProviderRegisterResponse,
    ProviderRegisterResult, ProviderRegisterSnapshot, ProviderRegistrationDocument,
    ProviderSourceInventory, builtin_provider_register_json, builtin_provider_registrations,
};
pub use route::{
    CompiledProviderRoute, PROVIDER_ROUTE_SCHEMA_ID, PROVIDER_ROUTE_SCHEMA_VERSION,
    ProviderRouteAccess, ProviderRouteAuthority, ProviderRouteCache, ProviderRouteCacheScope,
    ProviderRouteCardinality, ProviderRouteCompileError, ProviderRouteConcurrency,
    ProviderRouteEffects, ProviderRouteInputSlot, ProviderRouteInputSource, ProviderRouteOutput,
    ProviderRouteRequiredState, ProviderRouteRequirement, ProviderRouteSpec, ProviderRouteTarget,
    ProviderRouteTelemetry, ProviderRouteTelemetryPolicy, ProviderRouteValueType,
};
pub use syntax_query::{
    PROVIDER_SYNTAX_QUERY_OPERATION, PROVIDER_SYNTAX_QUERY_REQUEST_SCHEMA_ID,
    PROVIDER_SYNTAX_QUERY_RESPONSE_SCHEMA_ID, ProviderSyntaxQueryCapture,
    ProviderSyntaxQueryRequest, ProviderSyntaxQueryResponse, SyntaxQueryPattern, SyntaxQueryPlan,
    SyntaxQueryPredicate, SyntaxQueryPredicateOp, SyntaxQueryPredicateValue,
};
pub use workspace_install::*;
