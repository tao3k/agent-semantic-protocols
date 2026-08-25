#![deny(unsafe_code)]

pub mod provider_stream;
pub mod query_generation;
pub mod readiness;
pub mod resident_install;
pub mod resident_publication;

#[path = "runtime_asp_client.rs"]
mod runtime_asp_client;

pub use provider_stream::{bind_provider_stream_listener, serve_provider_stream};
pub use runtime_asp_client::{bind_http_listener, build_http_service};
pub mod artifact_activation;
