// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Optional host session context forwarded by Marlin.

pub(crate) struct AgentSession {
    pub(crate) client: String,
    pub(crate) id: String,
}

pub(crate) fn current_agent_session() -> Option<AgentSession> {
    let id = std::env::var("MARLIN_SESSION_ID")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .or_else(|| {
            std::env::var("AGENT_SESSION_ID")
                .ok()
                .filter(|value| !value.trim().is_empty())
        })?;
    let client = std::env::var("MARLIN_HOST_CLIENT")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| "marlin".to_owned());
    Some(AgentSession { client, id })
}
