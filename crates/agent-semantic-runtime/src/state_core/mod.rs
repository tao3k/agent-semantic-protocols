//! Resolve ASP `State Core` identity and durable state paths.

mod identity;
mod layout;
mod migration;
mod registry_gc;
mod resolution;

#[cfg(test)]
#[path = "../../tests/unit/state_core_registry_gc.rs"]
mod registry_gc_tests;

pub use crate::git::RemoteUrl;
pub use identity::{
    RepoId, RepoIdentity, RepoPersistence, ScopeId, WorkspaceId, WorkspaceIdentity,
};
pub use layout::{
    ASP_STATE_HOME_ENV, CLIENT_DB_FILE, DEFAULT_SCOPE_ID, DEFAULT_STATE_HOME_DIR,
    STATE_LAYOUT_VERSION, STATE_MANIFEST_FILE, StatePaths, TURSO_BACKEND, resolve_state_home,
    resolve_state_home_from,
};
pub use registry_gc::{
    ProjectRegistryGcCandidate, ProjectRegistryGcOptions, ProjectRegistryGcReport,
};
pub use resolution::{ResolvedState, StateLocateReport, locate_state};
