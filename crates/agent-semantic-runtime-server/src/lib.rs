#![deny(unsafe_code)]

pub mod provider_readiness;
pub mod provider_stream;
pub mod query_generation;
pub mod readiness;
pub mod resident_install;
pub mod resident_publication;
pub mod schema_bundle;

#[path = "runtime_asp_client.rs"]
mod runtime_asp_client;

pub use agent_semantic_client_server::{
    ASP_CLIENT_HTTP_FRAME_PATH, AspClientGrpcTransport, bind_asp_client_grpc_unix,
    serve_asp_client_grpc_unix, serve_asp_client_http_json as serve_http_json,
};
pub use provider_stream::{bind_provider_stream_listener, serve_provider_stream};
pub use runtime_asp_client::{RuntimeAspClientDispatcher, build_frame_service};
pub use schema_bundle::RuntimeSchemaBundleCatalog;
pub mod artifact_activation;
pub mod asp_python_graphs_transport;
pub mod asp_python_graphs_artifact;
