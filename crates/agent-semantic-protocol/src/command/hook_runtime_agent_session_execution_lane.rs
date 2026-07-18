use super::{HookClientExecutionTransport, command_prefix_matches_wrapped};

pub(super) struct ResidentExecutionLane {
    pub(super) name: String,
    pub(super) transport: HookClientExecutionTransport,
    pub(super) resident_child_name: String,
    pub(super) resident_agent_role: String,
    pub(super) resident_codex_agent_name: String,
    pub(super) command_prefixes: Vec<Vec<String>>,
    pub(super) receipt_kind: String,
}

impl ResidentExecutionLane {
    pub(super) fn name(&self) -> &str {
        &self.name
    }

    pub(super) fn transport(&self) -> HookClientExecutionTransport {
        self.transport
    }

    pub(super) fn resident_child_name(&self) -> &str {
        &self.resident_child_name
    }

    pub(super) fn resident_agent_role(&self) -> &str {
        &self.resident_agent_role
    }

    pub(super) fn resident_codex_agent_name(&self) -> &str {
        &self.resident_codex_agent_name
    }

    pub(super) fn receipt_kind(&self) -> &str {
        &self.receipt_kind
    }

    pub(super) fn matching_prefix_len(&self, command_tokens: &[String]) -> Option<usize> {
        self.command_prefixes
            .iter()
            .filter(|prefix| command_prefix_matches_wrapped(command_tokens, prefix))
            .map(Vec::len)
            .max()
    }
}
