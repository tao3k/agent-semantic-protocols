//! Integration tests for the ASP Rust workspace policy adapter.

#[path = "integration/publication_admission.rs"]
mod publication_admission;

#[cfg(feature = "workspace-policy")]
#[path = "integration/workspace_policy.rs"]
mod workspace_policy;
