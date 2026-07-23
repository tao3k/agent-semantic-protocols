//! Describes configured resident targets selected by compiled hook rules.

/// Borrowed identity for a configured resident dispatch target.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ConfiguredResidentTarget<'a> {
    /// Stable resident session name.
    pub resident_name: &'a str,
    /// Codex agent type used to route the resident.
    pub codex_agent_name: &'a str,
    /// Configured semantic role for the resident.
    pub role: &'a str,
}
