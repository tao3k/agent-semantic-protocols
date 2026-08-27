//! Host facts and parser-derived semantic capabilities consumed by Hook rules.

mod capability;
mod host;
mod matching;
mod projection;
mod receipt;
mod subject;

pub(crate) use capability::{
    FilesystemPermissionFact, FilesystemPermissionKind, FilesystemPermissionSource,
    SemanticCapability, SemanticCapabilityEvidence,
};
pub(crate) use host::{AgentActionKind, HostInvocationFact, HostInvocationKind};
pub(crate) use matching::{action_kind_matches, host_invocation_kind_matches};
pub(crate) use projection::project_agent_action;
pub(crate) use receipt::AgentAction;
pub(crate) use subject::{AgentActionSubject, AgentActionSubjectKind};
