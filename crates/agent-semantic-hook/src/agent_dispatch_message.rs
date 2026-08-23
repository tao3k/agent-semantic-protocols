#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct AgentDispatchMessageFields<'a> {
    pub(crate) role: &'a str,
    pub(crate) receipt_kind: &'a str,
}

pub(crate) const CHOICE_PLANE_COMMAND: &str = "asp session --agents choice-plane";

pub(crate) fn render_choice_plane_instruction(fields: AgentDispatchMessageFields<'_>) -> String {
    format!(
        "This operation is denied only in the current Agent; ASP remains available. Use `{CHOICE_PLANE_COMMAND}` to select the host Agent for role `{}` and require receipt `{}` before retrying the exact operation.",
        fields.role, fields.receipt_kind
    )
}

#[cfg(test)]
#[path = "../tests/unit/agent_dispatch_message.rs"]
mod tests;
