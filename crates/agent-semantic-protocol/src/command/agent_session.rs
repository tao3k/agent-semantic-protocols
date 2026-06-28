pub(crate) struct AgentSession {
    pub(crate) client: String,
    pub(crate) id: String,
    pub(crate) root_id: Option<String>,
}

impl AgentSession {
    pub(crate) fn recall_session_id(&self) -> &str {
        self.root_id.as_deref().unwrap_or(&self.id)
    }
}

pub(crate) fn current_agent_session() -> Option<AgentSession> {
    let mut sessions = [
        ("CODEX_THREAD_ID", "codex"),
        ("CLAUDE_CODE_SESSION_ID", "claude-code"),
        ("CLAUDE_CODE_REMOTE_SESSION_ID", "claude-code"),
    ]
    .into_iter()
    .filter_map(|(name, client)| {
        env_value(name).map(|id| AgentSession {
            client: client.to_string(),
            root_id: root_session_id(&id),
            id,
        })
    })
    .collect::<Vec<_>>();

    let session = sessions.pop()?;
    if sessions.iter().all(|candidate| candidate.id == session.id) {
        Some(session)
    } else {
        None
    }
}

fn root_session_id(current_session_id: &str) -> Option<String> {
    env_value("ASP_ROOT_SESSION_ID").filter(|root_id| root_id != current_session_id)
}

fn env_value(name: &str) -> Option<String> {
    std::env::var(name).ok().and_then(non_empty_value)
}

fn non_empty_value(value: String) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}
