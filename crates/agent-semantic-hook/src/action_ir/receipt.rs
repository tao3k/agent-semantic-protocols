use super::{
    AgentActionKind, AgentActionSubject, AgentActionSubjectKind, FilesystemPermissionFact,
    FilesystemPermissionKind, FilesystemPermissionSource, HostInvocationFact, HostInvocationKind,
    SemanticCapability, SemanticCapabilityEvidence,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct AgentAction {
    pub(crate) host: HostInvocationFact,
    pub(crate) filesystem_permissions: Vec<FilesystemPermissionFact>,
    pub(crate) capabilities: Vec<SemanticCapability>,
    pub(crate) subjects: Vec<AgentActionSubject>,
}

impl AgentAction {
    pub(crate) fn add_capability(&mut self, capability: SemanticCapability) {
        if !self.capabilities.contains(&capability) {
            self.capabilities.push(capability);
        }
    }

    pub(crate) fn add_filesystem_permission(&mut self, permission: FilesystemPermissionFact) {
        if !self.filesystem_permissions.contains(&permission) {
            self.add_capability(SemanticCapability::from_filesystem_permission_fact(
                &permission,
            ));
            self.filesystem_permissions.push(permission);
        }
    }

    pub(crate) fn receipt_value(&self) -> serde_json::Value {
        serde_json::json!({
            "hostInvocation": {
            "action": host_invocation_kind_label(self.host.action),
                "toolName": self.host.tool_name,
                "surface": self.host.surface,
                "payload": self.host.payload,
                "source": self.host.invocation_source,
            },
            "filesystemPermissions": self.filesystem_permissions.iter().map(|fact| serde_json::json!({
                "permission": filesystem_permission_kind_label(fact.permission),
                "source": filesystem_permission_source_label(fact.source),
                "subject": fact.subject.as_deref(),
            })).collect::<Vec<_>>(),
            "semanticCapabilities": self.capabilities.iter().map(|capability| serde_json::json!({
                "action": agent_action_kind_label(capability.action),
                "evidence": semantic_capability_evidence_label(capability.evidence),
            })).collect::<Vec<_>>(),
            "subjects": self.subjects.iter().map(|subject| serde_json::json!({
                "value": subject.value.as_str(),
                "kind": action_subject_kind_label(subject.kind),
            })).collect::<Vec<_>>(),
        })
    }
}

const fn filesystem_permission_kind_label(kind: FilesystemPermissionKind) -> &'static str {
    match kind {
        FilesystemPermissionKind::Read => "read",
        FilesystemPermissionKind::Write => "write",
    }
}

const fn filesystem_permission_source_label(source: FilesystemPermissionSource) -> &'static str {
    match source {
        FilesystemPermissionSource::HostMatcher => "host-matcher",
        FilesystemPermissionSource::ShellRedirection => "shell-redirection",
    }
}

const fn agent_action_kind_label(kind: AgentActionKind) -> &'static str {
    match kind {
        AgentActionKind::Read => "read",
        AgentActionKind::Edit => "edit",
        AgentActionKind::Execute => "execute",
        AgentActionKind::Mcp => "mcp",
        AgentActionKind::SpawnAgent => "spawn-agent",
        AgentActionKind::Unknown => "unknown",
    }
}

const fn host_invocation_kind_label(kind: HostInvocationKind) -> &'static str {
    match kind {
        HostInvocationKind::Read => "read",
        HostInvocationKind::Edit => "edit",
        HostInvocationKind::Execute => "execute",
        HostInvocationKind::Mcp => "mcp",
        HostInvocationKind::SpawnAgent => "spawn-agent",
        HostInvocationKind::Unknown => "unknown",
    }
}

const fn semantic_capability_evidence_label(evidence: SemanticCapabilityEvidence) -> &'static str {
    match evidence {
        SemanticCapabilityEvidence::HostMatcher => "host-matcher",
        SemanticCapabilityEvidence::RegisteredSourceOperand => "registered-source-operand",
        SemanticCapabilityEvidence::ShellRedirection => "shell-redirection",
    }
}

const fn action_subject_kind_label(kind: AgentActionSubjectKind) -> &'static str {
    match kind {
        AgentActionSubjectKind::RegisteredLanguageSource => "registered-language-source",
        AgentActionSubjectKind::RegisteredLanguageSourcePattern => {
            "registered-language-source-pattern"
        }
        AgentActionSubjectKind::Directory => "directory",
        AgentActionSubjectKind::StructuralSelector => "structural-selector",
        AgentActionSubjectKind::Other => "other",
    }
}
