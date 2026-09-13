#![deny(dead_code)]
// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Language-neutral Provider Registration Protocol shared by ASP Server and providers.

mod install_register;
mod provider_capabilities;
mod provider_stream;
mod register;
mod release_catalog;
mod route;
mod syntax_query;
mod workspace_install;
pub use install_register::ProviderInstallArtifactDomain;
pub use install_register::ProviderInstallRegister;
pub use install_register::ProviderInstallRegistration;
pub use install_register::parse_provider_install_register;
pub use provider_capabilities::ProviderQueryPackClause;
pub use provider_capabilities::ProviderQueryPackDescriptor;
pub use provider_capabilities::ProviderQueryPackRecipe;
pub use provider_capabilities::ProviderQueryPackTermRole;
pub use provider_capabilities::ProviderQueryPackTermRoleOverride;
pub use provider_capabilities::ProviderQueryPackTrigger;
pub use provider_capabilities::ProviderSearchCapabilities;
pub use provider_capabilities::ProviderSemanticFactsDescriptor;
pub use provider_capabilities::ProviderSemanticFactsIntentAxis;
pub use provider_capabilities::ProviderSourceSnapshotDescriptor;
pub use provider_stream::PROVIDER_STREAM_SCHEMA_ID;
pub use provider_stream::PROVIDER_STREAM_SCHEMA_VERSION;
pub use provider_stream::ProviderStreamEnvelopeIdentity;
pub use provider_stream::validate_provider_stream_envelope;
pub use release_catalog::PROVIDER_RELEASE_CATALOG_SCHEMA_ID;
pub use release_catalog::PROVIDER_RELEASE_CATALOG_SCHEMA_VERSION;
pub use release_catalog::ProviderReleaseCatalog;
pub use release_catalog::ProviderReleaseRegistration;
pub use release_catalog::builtin_provider_release_catalog;
pub use release_catalog::parse_provider_release_catalog;

pub use register::PROVIDER_REGISTER_REQUEST_SCHEMA_ID;
pub use register::PROVIDER_REGISTER_RESPONSE_SCHEMA_ID;
pub use register::PROVIDER_REGISTER_SCHEMA_VERSION;
pub use register::ProviderDocumentInventory;
pub use register::ProviderProjectInventory;
pub use register::ProviderRegisterOperation;
pub use register::ProviderRegisterRequest;
pub use register::ProviderRegisterResponse;
pub use register::ProviderRegisterResult;
pub use register::ProviderRegisterSnapshot;
pub use register::ProviderRegistrationDocument;
pub use register::ProviderSourceInventory;
pub use register::builtin_provider_register_json;
pub use register::builtin_provider_registrations;
pub use route::CompiledProviderRoute;
pub use route::PROVIDER_ROUTE_SCHEMA_ID;
pub use route::PROVIDER_ROUTE_SCHEMA_VERSION;
pub use route::ProviderRouteAccess;
pub use route::ProviderRouteAuthority;
pub use route::ProviderRouteCache;
pub use route::ProviderRouteCacheScope;
pub use route::ProviderRouteCardinality;
pub use route::ProviderRouteCompileError;
pub use route::ProviderRouteConcurrency;
pub use route::ProviderRouteEffects;
pub use route::ProviderRouteInputSlot;
pub use route::ProviderRouteInputSource;
pub use route::ProviderRouteOutput;
pub use route::ProviderRouteRequiredState;
pub use route::ProviderRouteRequirement;
pub use route::ProviderRouteSpec;
pub use route::ProviderRouteTarget;
pub use route::ProviderRouteTelemetry;
pub use route::ProviderRouteTelemetryPolicy;
pub use route::ProviderRouteValueType;
pub use route::ProviderSchemaReference;
pub use syntax_query::PROVIDER_SYNTAX_QUERY_OPERATION;
pub use syntax_query::PROVIDER_SYNTAX_QUERY_REQUEST_SCHEMA_ID;
pub use syntax_query::PROVIDER_SYNTAX_QUERY_RESPONSE_SCHEMA_ID;
pub use syntax_query::ProviderSyntaxQueryCapture;
pub use syntax_query::ProviderSyntaxQueryRequest;
pub use syntax_query::ProviderSyntaxQueryResponse;
pub use syntax_query::SyntaxQueryPattern;
pub use syntax_query::SyntaxQueryPlan;
pub use syntax_query::SyntaxQueryPredicate;
pub use syntax_query::SyntaxQueryPredicateOp;
pub use syntax_query::SyntaxQueryPredicateValue;
pub use workspace_install::PROVIDER_WORKSPACE_INSTALL_SCHEMA_AUTHORITY;
pub use workspace_install::PROVIDER_WORKSPACE_INSTALL_SCHEMA_FILE;
pub use workspace_install::PROVIDER_WORKSPACE_INSTALL_SCHEMA_ID;
pub use workspace_install::PROVIDER_WORKSPACE_INSTALL_SCHEMA_VERSION;
pub use workspace_install::ProviderWorkspaceInstallDescriptor;
pub use workspace_install::WorkspaceArtifactDescriptor;
pub use workspace_install::WorkspaceBuildDescriptor;
pub use workspace_install::WorkspaceCommandDescriptor;
pub use workspace_install::WorkspaceLaunchDescriptor;
pub use workspace_install::WorkspaceRuntimeDependencyDescriptor;

#[cfg(test)]
#[path = "../tests/unit/provider_capabilities.rs"]
mod provider_capabilities_tests;
#[cfg(test)]
#[path = "../tests/unit/release_catalog.rs"]
mod release_catalog_tests;
#[cfg(test)]
#[path = "../tests/unit/route.rs"]
mod route_tests;
#[cfg(test)]
#[path = "../tests/unit/workspace_install.rs"]
mod workspace_install_tests;
