// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use super::RuleMatch;
use super::structured_document_format;
use crate::tool_action::ToolAction;

impl RuleMatch {
    pub(super) fn matches_configured_document_format(&self, candidate: &std::path::Path) -> bool {
        if self.argv_structured_document_file && structured_document_format(candidate).is_none() {
            return false;
        }
        self.matches_structured_projection_format(candidate)
    }
    pub(super) fn matches_structured_projection_format(&self, candidate: &std::path::Path) -> bool {
        let Some(projection) = self.structured_projection.as_ref() else {
            return true;
        };
        let format = structured_document_format(candidate);
        format.is_some_and(|format| format == projection.config.document_format)
    }
    pub(super) fn matches_paths(&self, paths: &[String]) -> bool {
        self.matches_path(paths)
    }
    pub(super) fn matches_tool(&self, action: &ToolAction) -> bool {
        self.tool_any.is_empty()
            || self
                .tool_any
                .iter()
                .any(|tool| tool.eq_ignore_ascii_case(&action.tool_name))
    }
    pub(super) fn needs_command_tokens(&self) -> bool {
        !self.command_any.is_empty()
            || !self.argv_prefix_any.is_empty()
            || !self.argv_token_all.is_empty()
            || !self.process_environment_assignment_any.is_empty()
            || !self.argv_source_any.is_empty()
            || !self.argv_source_glob_any.is_empty()
            || self.argv_workspace_regular_file
            || self.argv_structured_document_file
            || self.argv_registered_source_file
            || self.structured_projection.is_some()
    }
    pub(super) fn needs_path_match(&self) -> bool {
        !self.path_any.is_empty()
            || !self.path_glob_any.is_empty()
            || !self.profile_extension_any.is_empty()
            || self.needs_profile_subjects()
    }

    pub(super) fn needs_profile_subjects(&self) -> bool {
        !self.profile_any.is_empty()
            && self.agent_action.needs_profile_subjects()
            && !self.needs_command_tokens()
    }
    pub(super) fn needs_source_paths(&self) -> bool {
        self.agent_action.needs_subjects()
            || self.needs_path_match()
            || !self.argv_source_any.is_empty()
            || !self.argv_source_glob_any.is_empty()
            || self.argv_workspace_regular_file
            || self.argv_structured_document_file
            || self.argv_registered_source_file
    }
    pub(super) fn needs_argv_source_match(&self) -> bool {
        !self.argv_source_any.is_empty()
            || !self.argv_source_glob_any.is_empty()
            || self.argv_workspace_regular_file
            || self.argv_structured_document_file
            || self.argv_registered_source_file
    }
}
