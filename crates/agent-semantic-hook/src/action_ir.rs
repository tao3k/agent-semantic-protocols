//! Host-action facts and parser-derived semantic capabilities used by Hook rules.

#[derive(Clone, Debug, Copy, Eq, PartialEq)]
pub(crate) enum AgentActionKind {
    Read,
    Edit,
    Search,
    Enumerate,
    Execute,
    Test,
    Build,
    Delete,
    Unknown,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum AgentActionSubjectKind {
    RegisteredLanguageSource,
    RegisteredLanguageSourcePattern,
    Directory,
    StructuralSelector,
    Other,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum AgentActionAuthority {
    RawHostAction,
    RawShell,
    ParserOwnedExactEvidence,
    ParserOwnedSearch,
    AstPatchEvidence,
    Unknown,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SemanticCapabilityCertainty {
    Exact,
    Possible,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SemanticCapabilityEvidence {
    HostAction,
    ParserEffect,
    ShellRedirection,
    SubjectAccess,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct SemanticCapability {
    pub(crate) action: AgentActionKind,
    pub(crate) certainty: SemanticCapabilityCertainty,
    pub(crate) authority: AgentActionAuthority,
    pub(crate) evidence: SemanticCapabilityEvidence,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct AgentActionSubject {
    pub(crate) value: String,
    pub(crate) kind: AgentActionSubjectKind,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct AgentAction {
    pub(crate) host_action: AgentActionKind,
    pub(crate) host_tool_name: String,
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
            "hostAction": agent_action_kind_label(self.host_action),
            "hostToolName": self.host_tool_name,
            "semanticCapabilities": self.capabilities.iter().map(|capability| serde_json::json!({
                "action": agent_action_kind_label(capability.action),
                "certainty": semantic_capability_certainty_label(capability.certainty),
                "authority": action_authority_label(capability.authority),
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
        AgentActionKind::Test => "test",
        AgentActionKind::Build => "build",
        AgentActionKind::Delete => "delete",
        AgentActionKind::Unknown => "unknown",
    }
}

const fn semantic_capability_certainty_label(
    certainty: SemanticCapabilityCertainty,
) -> &'static str {
    match certainty {
        SemanticCapabilityCertainty::Exact => "exact",
        SemanticCapabilityCertainty::Possible => "possible",
    }
}

const fn semantic_capability_evidence_label(
    evidence: SemanticCapabilityEvidence,
) -> &'static str {
    match evidence {
        SemanticCapabilityEvidence::HostAction => "host-action",
        SemanticCapabilityEvidence::ParserEffect => "parser-effect",
        SemanticCapabilityEvidence::ShellRedirection => "shell-redirection",
        SemanticCapabilityEvidence::SubjectAccess => "subject-access",
    }
}

const fn action_authority_label(authority: AgentActionAuthority) -> &'static str {
    match authority {
        AgentActionAuthority::RawHostAction => "raw-host-action",
        AgentActionAuthority::RawShell => "raw-shell",
        AgentActionAuthority::ParserOwnedExactEvidence => "parser-owned-exact-evidence",
        AgentActionAuthority::ParserOwnedSearch => "parser-owned-search",
        AgentActionAuthority::AstPatchEvidence => "ast-patch-evidence",
        AgentActionAuthority::Unknown => "unknown",
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

pub(crate) fn action_kind_matches(
    candidate: AgentActionKind,
    configured: agent_semantic_config::HookClientActionKind,
) -> bool {
    use agent_semantic_config::HookClientActionKind as Configured;
    matches!(
        (candidate, configured),
        (AgentActionKind::Read, Configured::Read)
            | (AgentActionKind::Edit, Configured::Edit)
            | (AgentActionKind::Search, Configured::Search)
            | (AgentActionKind::Enumerate, Configured::Enumerate)
            | (AgentActionKind::Execute, Configured::Execute)
            | (AgentActionKind::Test, Configured::Test)
            | (AgentActionKind::Build, Configured::Build)
            | (AgentActionKind::Delete, Configured::Delete)
            | (AgentActionKind::Unknown, Configured::Unknown)
    )
}

pub(crate) fn action_kind_from_config(
    configured: agent_semantic_config::HookClientActionKind,
) -> Option<AgentActionKind> {
    use agent_semantic_config::HookClientActionKind as Configured;
    match configured {
        Configured::Read => Some(AgentActionKind::Read),
        Configured::Edit => Some(AgentActionKind::Edit),
        Configured::Search => Some(AgentActionKind::Search),
        Configured::Enumerate => Some(AgentActionKind::Enumerate),
        Configured::Execute => Some(AgentActionKind::Execute),
        Configured::Test => Some(AgentActionKind::Test),
        Configured::Build => Some(AgentActionKind::Build),
        Configured::Delete => Some(AgentActionKind::Delete),
        Configured::Unknown => Some(AgentActionKind::Unknown),
    }
}

pub(crate) fn subject_kind_matches(
    candidate: AgentActionSubjectKind,
    configured: agent_semantic_config::HookClientActionSubjectKind,
) -> bool {
    use agent_semantic_config::HookClientActionSubjectKind as Configured;
    matches!(
        (candidate, configured),
        (
            AgentActionSubjectKind::RegisteredLanguageSource,
            Configured::RegisteredLanguageSource
        ) | (
            AgentActionSubjectKind::RegisteredLanguageSourcePattern,
            Configured::RegisteredLanguageSourcePattern
        ) | (AgentActionSubjectKind::Directory, Configured::Directory)
            | (
                AgentActionSubjectKind::StructuralSelector,
                Configured::StructuralSelector
            )
            | (AgentActionSubjectKind::Other, Configured::Other)
    )
}

pub(crate) fn authority_matches(
    candidate: AgentActionAuthority,
    configured: agent_semantic_config::HookClientActionAuthority,
) -> bool {
    candidate == action_authority_from_config(configured)
}

pub(crate) fn action_authority_from_config(
    configured: agent_semantic_config::HookClientActionAuthority,
) -> AgentActionAuthority {
    use agent_semantic_config::HookClientActionAuthority as Configured;
    match configured {
        Configured::RawHostAction => AgentActionAuthority::RawHostAction,
        Configured::RawShell => AgentActionAuthority::RawShell,
        Configured::ParserOwnedExactEvidence => AgentActionAuthority::ParserOwnedExactEvidence,
        Configured::ParserOwnedSearch => AgentActionAuthority::ParserOwnedSearch,
        Configured::AstPatchEvidence => AgentActionAuthority::AstPatchEvidence,
        Configured::Unknown => AgentActionAuthority::Unknown,
    }
}
