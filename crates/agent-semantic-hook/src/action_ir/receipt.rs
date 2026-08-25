use super::{
    AgentActionKind, AgentActionSubject, AgentActionSubjectKind, HostInvocationFact,
    HostInvocationKind, SemanticCapability, SemanticCapabilityEvidence,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct AgentAction {
    pub(crate) host: HostInvocationFact,
    pub(crate) capabilities: Vec<SemanticCapability>,
    pub(crate) subjects: Vec<AgentActionSubject>,
}

impl AgentAction {
    pub(crate) fn add_capability(&mut self, capability: SemanticCapability) {
        if !self.capabilities.contains(&capability) {
            self.capabilities.push(capability);
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

const fn agent_action_kind_label(kind: AgentActionKind) -> &'static str {
    match kind {
        AgentActionKind::Read => "read",
        AgentActionKind::Edit => "edit",
        AgentActionKind::Search => "search",
        AgentActionKind::Enumerate => "enumerate",
        AgentActionKind::Execute => "execute",
        AgentActionKind::Unknown => "unknown",
    }
}

const fn host_invocation_kind_label(kind: HostInvocationKind) -> &'static str {
    match kind {
        HostInvocationKind::Read => "read",
        HostInvocationKind::Edit => "edit",
        HostInvocationKind::Search => "search",
        HostInvocationKind::Enumerate => "enumerate",
        HostInvocationKind::Execute => "execute",
        HostInvocationKind::Mcp => "mcp",
        HostInvocationKind::Unknown => "unknown",
    }
}

const fn semantic_capability_evidence_label(evidence: SemanticCapabilityEvidence) -> &'static str {
    match evidence {
        SemanticCapabilityEvidence::HostInvocation => "host-invocation",
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
