//! Resolves configured resident targets from compiled hook rules.

use super::{ConfiguredResidentTarget, implementation::ClientHookConfig};

impl ClientHookConfig {
    /// Resolve the configured resident target for a Codex agent type.
    pub fn configured_resident_target(
        &self,
        codex_agent_name: &str,
    ) -> Option<ConfiguredResidentTarget<'_>> {
        self.rules.iter().find_map(|rule| {
            let dispatch = rule.dispatch.as_ref()?;
            dispatch
                .resident_codex_agent_name
                .eq_ignore_ascii_case(codex_agent_name)
                .then_some(ConfiguredResidentTarget {
                    resident_name: &dispatch.resident_name,
                    codex_agent_name: &dispatch.resident_codex_agent_name,
                    role: &dispatch.resident_role,
                })
        })
    }
}
