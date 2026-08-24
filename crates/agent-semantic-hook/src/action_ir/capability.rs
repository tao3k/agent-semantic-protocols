use super::AgentActionKind;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SemanticCapabilityEvidence {
    HostInvocation,
    ShellRedirection,
    ShellPathOperand,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct SemanticCapability {
    pub(crate) action: AgentActionKind,
    pub(crate) evidence: SemanticCapabilityEvidence,
}
