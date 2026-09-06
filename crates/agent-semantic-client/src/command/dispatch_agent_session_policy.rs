// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-only

pub(crate) fn is_agent_session_control_json_command(args: &[String]) -> bool {
    matches!(
        args,
        [agent, session, ..] if agent == "agent" && session == "session"
    )
}
