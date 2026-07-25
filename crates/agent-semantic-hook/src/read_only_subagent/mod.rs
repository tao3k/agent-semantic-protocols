//! Read-only resident-child policy facade.

mod identity;
mod payload;
mod receipt;
mod write_guard;

pub use identity::{
    CodexHookAgentId, CodexHookAgentType, ConfiguredCodexAgentName, ConfiguredResidentRole,
    HookSubagentPermissionContext, ManagedChildName, ResidentChildIdentityProof,
    ResidentChildSessionId, ResidentConfiguration, ResidentEnabled, ResidentIdentityStatus,
    ResidentLiveIdentity, ResidentRootSessionId, ResidentSandboxMode,
};
pub use receipt::classify_read_only_subagent_receipt;
pub use write_guard::classify_read_only_subagent_write;
