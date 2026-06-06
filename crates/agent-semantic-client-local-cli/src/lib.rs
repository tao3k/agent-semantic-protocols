#![deny(dead_code)]

//! Local native-provider process backend for `agent-semantic-client`.

pub mod backend;

pub use backend::{LocalNativeCliBackend, LocalNativeCommand, LocalNativeOutput};
