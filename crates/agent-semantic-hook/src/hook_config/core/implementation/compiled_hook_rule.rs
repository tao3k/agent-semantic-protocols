use super::{
    CompiledHookRule, DurableRuleMatcherArtifact, HookRuntime, RuleRoute, canonical_event,
    collect_source_selector_matches, compiled_rule_message,
};
use crate::protocol::DecisionRoute;
use crate::tool_action::ToolAction;

impl CompiledHookRule {
    pub(super) fn canonical_event_key(&self) -> Option<String> {
        self.event.as_deref().map(canonical_event)
    }
    pub(super) fn canonical_platform_key(&self) -> Option<String> {
        self.platform
            .as_ref()
            .map(|value| value.to_ascii_lowercase())
    }
    pub(super) fn durable_matcher_artifact(&self) -> DurableRuleMatcherArtifact {
        self.match_config.durable_matcher_artifact()
    }
    pub(super) fn rendered_message(&self) -> String {
        compiled_rule_message::render(self)
    }
    pub(super) fn needs_decision_paths(&self) -> bool {
        self.match_config.needs_source_paths()
    }
    pub(super) fn matches_before_paths(
        &self,
        runtime: &HookRuntime,
        platform: &str,
        event: &str,
        action: &ToolAction,
        command_tokens: Option<&[String]>,
    ) -> bool {
        self.platform
            .as_deref()
            .is_none_or(|expected| expected.eq_ignore_ascii_case(platform))
            && self
                .event
                .as_deref()
                .is_none_or(|expected| canonical_event(expected) == canonical_event(event))
            && self.match_config.matches_before_paths(
                runtime,
                action,
                command_tokens,
                Some(action.paths.as_slice()),
            )
    }
    pub(super) fn agent_action_receipt(
        &self,
        runtime: &HookRuntime,
        action: &ToolAction,
        paths: &[String],
        structured_source_operands: Option<&[String]>,
    ) -> Option<serde_json::Value> {
        self.match_config
            .agent_action
            .derive_agent_action_for_rule(runtime, action, Some(paths), structured_source_operands)
            .map(|agent_action| agent_action.receipt_value())
    }
    pub(super) fn matches_language(&self, runtime: &HookRuntime, paths: &[String]) -> bool {
        if self.language_ids.is_empty() {
            return true;
        }
        if paths.is_empty() {
            return false;
        }
        !collect_source_selector_matches(runtime, paths.iter().map(String::as_str), |provider| {
            self.language_ids
                .iter()
                .any(|language_id| language_id == &provider.language_id)
        })
        .is_empty()
    }
}

impl RuleRoute {
    pub(super) fn decision_route(&self, runtime: &HookRuntime) -> DecisionRoute {
        let provider = runtime
            .providers
            .iter()
            .find(|provider| provider.provider_id == self.provider_id);
        DecisionRoute {
            language_id: self.language_id.clone(),
            provider_id: self.provider_id.clone(),
            binary: self
                .binary
                .clone()
                .or_else(|| provider.map(|_| "asp".to_owned()))
                .unwrap_or_default(),
            kind: self.kind,
            argv: self.argv.clone(),
            stdin_mode: self.stdin_mode,
        }
    }
}
