pub(crate) fn is_agent_session_control_json_command(args: &[String]) -> bool {
    matches!(
        args,
        [agent, session, ..] if agent == "agent" && session == "session"
    )
}
