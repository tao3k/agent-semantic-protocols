use super::AgentActionKind;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum FilesystemPermissionKind {
    Read,
    Write,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum FilesystemPermissionSource {
    HostMatcher,
    ShellRedirection,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct FilesystemPermissionFact {
    pub(crate) permission: FilesystemPermissionKind,
    pub(crate) source: FilesystemPermissionSource,
    pub(crate) subject: Option<String>,
}

impl FilesystemPermissionFact {
    pub(crate) fn new(
        permission: FilesystemPermissionKind,
        source: FilesystemPermissionSource,
        subject: Option<String>,
    ) -> Self {
        Self {
            permission,
            source,
            subject,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SemanticCapabilityEvidence {
    HostMatcher,
    RegisteredSourceOperand,
    ShellRedirection,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct SemanticCapability {
    pub(crate) action: AgentActionKind,
    pub(crate) evidence: SemanticCapabilityEvidence,
}

impl SemanticCapability {
    pub(crate) const fn from_filesystem_permission_fact(fact: &FilesystemPermissionFact) -> Self {
        let action = match fact.permission {
            FilesystemPermissionKind::Read => AgentActionKind::Read,
            FilesystemPermissionKind::Write => AgentActionKind::Edit,
        };
        let evidence = match fact.source {
            FilesystemPermissionSource::HostMatcher => SemanticCapabilityEvidence::HostMatcher,
            FilesystemPermissionSource::ShellRedirection => {
                SemanticCapabilityEvidence::ShellRedirection
            }
        };
        Self { action, evidence }
    }
}
