#![deny(unsafe_code)]
// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

pub mod provider_readiness;
pub mod provider_stream;
pub mod query_generation;
mod query_generation_calibration;
pub mod readiness;
pub mod resident_install;
pub mod resident_publication;
pub mod schema_bundle;

#[path = "runtime_asp_client.rs"]
mod runtime_asp_client;
mod runtime_query_generation;
mod runtime_query_generation_authority;
mod runtime_query_generation_model;
mod runtime_query_materialization;
pub(crate) use runtime_query_generation::RuntimeQueryGeneration;
pub(crate) use runtime_query_generation::RuntimeQueryTerminalState;
pub use runtime_query_generation_authority::RuntimeQueryGenerationAuthority;
pub use runtime_query_generation_authority::RuntimeQueryGenerationState;
pub use runtime_query_generation_key::RuntimeProjectWorkspaceKey;
pub mod runtime_evidence_graph;
mod runtime_query_generation_key;
mod runtime_search_execution;
mod runtime_search_execution_budget;
mod runtime_search_graph;
pub mod runtime_server_opentelemetry;

pub use agent_semantic_client_server::AspClientGrpcTransport;
pub use agent_semantic_client_server::bind_asp_client_grpc_tcp;
pub use agent_semantic_client_server::serve_asp_client_grpc_tcp;
pub use provider_stream::bind_provider_stream_tcp;
pub use provider_stream::serve_provider_stream_tcp;
pub use runtime_asp_client::HostWorkspaceInitializationBindingResolver;
pub use runtime_asp_client::RuntimeAspClientDispatcher;
pub use runtime_asp_client::build_frame_service;
pub use runtime_asp_client::workspace_search_providers_from_provider_register;
pub use schema_bundle::RuntimeSchemaBundleCatalog;
pub mod artifact_activation;
pub mod asp_python_graphs_artifact;
pub mod asp_python_graphs_transport;
