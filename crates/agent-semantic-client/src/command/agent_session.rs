pub(crate) struct AgentSession {
    pub(crate) client: String,
    pub(crate) id: String,
}

impl AgentSession {
    pub(crate) fn recall_session_id(&self) -> &str {
        &self.id
    }
}

pub(crate) fn current_agent_session() -> Option<AgentSession> {
    agent_semantic_runtime::current_agent_runtime_session().map(|session| AgentSession {
        client: session.client,
        id: session.id,
    })
}
