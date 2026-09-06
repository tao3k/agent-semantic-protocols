//! Resolve ASP `State Core` identity and durable state paths.

mod identity;
mod layout;
mod resolution;

pub use crate::git::RemoteUrl;
pub use identity::RepoId;
pub use identity::RepoIdentity;
pub use identity::RepoPersistence;
pub use identity::ScopeId;
pub use identity::WorkspaceId;
pub use identity::WorkspaceIdentity;
pub use identity::WorkspaceLifecycle;
pub use identity::is_temporary_checkout_path;
pub use layout::ASP_STATE_HOME_ENV;
pub use layout::CLIENT_DB_FILE;
pub use layout::DEFAULT_SCOPE_ID;
pub use layout::DEFAULT_STATE_HOME_DIR;
pub use layout::STATE_LAYOUT_VERSION;
pub use layout::STATE_MANIFEST_FILE;
pub use layout::StateHomeResolution;
pub use layout::StateHomeResolutionSource;
pub use layout::TURSO_BACKEND;
pub use layout::resolve_state_home;
pub use layout::resolve_state_home_from;
pub use layout::resolve_state_home_projection;
pub use layout::resolve_state_home_projection_from;
pub use resolution::ResolvedState;
pub use resolution::StateLocateReport;
pub use resolution::locate_state;
