#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct AgentDispatchMessageFields<'a> {
    pub(crate) agent: &'a str,
    pub(crate) symbol: Option<&'a str>,
    pub(crate) receipt_kind: &'a str,
}

pub(crate) const CHOICE_PLANE_COMMAND: &str = "asp session --agents choice-plane";

pub(crate) fn render_choice_plane_instruction(fields: AgentDispatchMessageFields<'_>) -> String {
    let target = fields.symbol.unwrap_or(fields.agent);
    format!(
        "This operation is denied only in the current Agent; ASP remains available. Invoke `{target}` for registered Agent `{}` through `{CHOICE_PLANE_COMMAND}` and require receipt `{}` before retrying the exact operation.",
        fields.agent, fields.receipt_kind
    )
}

#[cfg(test)]
#[path = "../tests/unit/agent_dispatch_message.rs"]
mod tests;
