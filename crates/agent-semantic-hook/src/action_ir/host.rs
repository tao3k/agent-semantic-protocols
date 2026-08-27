#[derive(Clone, Debug, Copy, Eq, PartialEq)]
pub(crate) enum AgentActionKind {
    Read,
    Edit,
    Execute,
    Mcp,
    SpawnAgent,
    Unknown,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum HostInvocationKind {
    Read,
    Edit,
    Execute,
    Mcp,
    SpawnAgent,
    Unknown,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct HostInvocationFact {
    pub(crate) action: HostInvocationKind,
    pub(crate) tool_name: String,
    pub(crate) surface: String,
    pub(crate) payload: serde_json::Value,
    /// Codex does not currently expose `ToolInvocation.source` in the Hook payload.
    /// Absence remains a Host fact and is never inferred from shell behavior.
    pub(crate) invocation_source: Option<String>,
}
