// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Builds structured-projection decision templates inside the compiled-rule owner.

use super::CompiledHookRule;
use super::HookDecision;
use super::HookRuntime;
use super::ToolAction;

impl CompiledHookRule {
    pub(super) fn structured_projection_decision_template(
        &self,
        runtime: &HookRuntime,
        action: &ToolAction,
        placeholder: &str,
    ) -> Option<(
        agent_semantic_config::HookClientStructuredProjectionMatchConfig,
        HookDecision,
    )> {
        let projection = self.match_config.structured_projection.as_ref()?;
        let paths = vec![placeholder.to_owned()];
        Some((
            projection.config.clone(),
            self.decision(runtime, "codex", "pre-tool", action, &paths, Some(&paths)),
        ))
    }

    pub(super) fn structured_projection_rejection_template(
        &self,
        runtime: &HookRuntime,
        action: &ToolAction,
        projector_binary: &str,
        placeholder: &str,
    ) -> Option<(i64, HookDecision)> {
        let matches_projector_family = self.match_config.structured_projection.is_none()
            && self.match_config.argv_structured_document_file
            && self
                .match_config
                .command_any
                .iter()
                .any(|command| command == projector_binary);
        if !matches_projector_family {
            return None;
        }
        let paths = vec![placeholder.to_owned()];
        Some((
            self.priority,
            self.decision(runtime, "codex", "pre-tool", action, &paths, None),
        ))
    }
}
