#![deny(dead_code)]

//! Language-neutral Provider Registration Protocol shared by ASP Server and providers.

mod register;
mod syntax_query;

pub use register::{
    PROVIDER_REGISTER_REQUEST_SCHEMA_ID, PROVIDER_REGISTER_RESPONSE_SCHEMA_ID,
    PROVIDER_REGISTER_SCHEMA_VERSION, ProviderRegisterOperation, ProviderRegisterRequest,
    ProviderRegisterResponse, ProviderRegisterResult, ProviderRegisterSnapshot,
    ProviderRegistrationDocument, builtin_provider_register_json, builtin_provider_registrations,
};
pub use syntax_query::{
    PROVIDER_SYNTAX_QUERY_OPERATION, PROVIDER_SYNTAX_QUERY_REQUEST_SCHEMA_ID,
    PROVIDER_SYNTAX_QUERY_RESPONSE_SCHEMA_ID, ProviderSyntaxQueryCapture,
    ProviderSyntaxQueryRequest, ProviderSyntaxQueryResponse, SyntaxQueryPattern, SyntaxQueryPlan,
    SyntaxQueryPredicate, SyntaxQueryPredicateOp, SyntaxQueryPredicateValue,
};
