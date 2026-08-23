#![deny(dead_code)]

//! Language-neutral Provider Registration Protocol shared by ASP Server and providers.

mod register;
mod route;
mod syntax_query;

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
