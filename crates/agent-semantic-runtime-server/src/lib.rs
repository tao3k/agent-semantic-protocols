#![deny(unsafe_code)]

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
pub use runtime_query_generation_authority::{
    RuntimeQueryGenerationAuthority, RuntimeQueryGenerationState,
};
mod runtime_cold_rg;
mod runtime_query_generation_key;
mod runtime_search_graph;

pub use agent_semantic_client_server::{
    AspClientGrpcTransport, bind_asp_client_grpc_tcp, serve_asp_client_grpc_tcp,
};
pub use provider_stream::{bind_provider_stream_tcp, serve_provider_stream_tcp};
pub use runtime_asp_client::{RuntimeAspClientDispatcher, build_frame_service};
pub use schema_bundle::RuntimeSchemaBundleCatalog;
pub mod artifact_activation;
pub mod asp_python_graphs_artifact;
pub mod asp_python_graphs_transport;
