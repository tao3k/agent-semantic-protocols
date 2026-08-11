#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct AgentDispatchMessageFields<'a> {
    pub(crate) agent_kind: &'a str,
    pub(crate) call_target: &'a str,
    pub(crate) role: &'a str,
    pub(crate) description: &'a str,
}

pub(crate) const CHOICE_PLANE_COMMAND: &str = "asp session --agents choice-plane";

pub(crate) fn render_choice_plane_instruction(fields: AgentDispatchMessageFields<'_>) -> String {
    let call_target = format!("@{}", fields.call_target.trim_start_matches('@'));
    format!(
        "Please use `{CHOICE_PLANE_COMMAND}` to create or resume the {} `{}` ({}; {}).",
        fields.agent_kind, call_target, fields.role, fields.description
    )
}
