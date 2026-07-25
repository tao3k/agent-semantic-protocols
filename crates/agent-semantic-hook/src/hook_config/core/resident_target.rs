//! Describes configured resident targets selected by compiled hook rules.

/// Borrowed identity for a configured resident dispatch target.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ConfiguredResidentTarget<'a> {
    /// Stable resident session name.
    resident_name: crate::ManagedChildName<'a>,
    /// Codex agent type used to route the resident.
    codex_agent_name: crate::ConfiguredCodexAgentName<'a>,
    /// Configured semantic role for the resident.
    role: crate::ConfiguredResidentRole<'a>,
}

impl<'a> ConfiguredResidentTarget<'a> {
    pub fn new(
        resident_name: crate::ManagedChildName<'a>,
        codex_agent_name: crate::ConfiguredCodexAgentName<'a>,
        role: crate::ConfiguredResidentRole<'a>,
    ) -> Self {
        Self {
            resident_name,
            codex_agent_name,
            role,
        }
    }

    pub fn resident_name(&self) -> &str {
        self.resident_name.as_str()
    }

    pub fn codex_agent_name(&self) -> &str {
        self.codex_agent_name.as_str()
    }

    pub fn role(&self) -> &str {
        self.role.as_str()
    }
}
