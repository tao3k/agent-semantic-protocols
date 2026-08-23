#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum AgentActionSubjectKind {
    RegisteredLanguageSource,
    RegisteredLanguageSourcePattern,
    Directory,
    StructuralSelector,
    Other,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct AgentActionSubject {
    pub(crate) value: String,
    pub(crate) kind: AgentActionSubjectKind,
}
