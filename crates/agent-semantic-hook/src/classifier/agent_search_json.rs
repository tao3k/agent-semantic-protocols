//! Owns JSON-output search denial and registry-derived recovery routes.

use super::decision::{DenyForActionRequest, deny_for_action};
use crate::command::{asp_command_tokens, command_line, infer_query_from_path, search_json_route};
use crate::{
    ActivatedProvider, DecisionRoute, DecisionRouteKind, HookDecision, HookRuntime, ReasonKind,
    ToolAction, subject_for_action,
};

pub(crate) fn materialize_agent_search_json_decision(
    registry: &HookRuntime,
    platform: &str,
    event: &str,
    action: &ToolAction,
    tokens: &[String],
) -> Option<HookDecision> {
    if asp_command_tokens(tokens) || crate::command::is_asp_facade_command(tokens) {
        return None;
    }
    if !tokens.iter().any(|token| token == "--json") {
        return None;
    }
    let (provider, argv) = search_json_route(registry, tokens)?;
    if !provider.policy.blocks_agent_search_json() {
        return None;
    }
    let route = search_json_decision_route(provider, argv);
    let message = format!(
        "agent-search-json denied; route: {}",
        command_line(&route.argv)
    );
    Some(deny_for_action(
        platform,
        event,
        DenyForActionRequest {
            reason_kind: ReasonKind::AgentSearchJson,
            action,
            language_ids: vec![provider.language_id.clone()],
            subject: subject_for_action(action),
            routes: vec![route],
            message,
        },
    ))
}

fn search_json_decision_route(provider: &ActivatedProvider, argv: Vec<String>) -> DecisionRoute {
    if let Some(path) = search_json_owner_path(&argv).map(str::to_string) {
        let query = infer_query_from_path(&path);
        return provider.route_from_template(
            DecisionRouteKind::Owner,
            &provider.routes.owner,
            Some(&path),
            query.as_deref(),
        );
    }
    DecisionRoute {
        language_id: provider.language_id.clone(),
        provider_id: provider.provider_id.clone(),
        binary: "asp".to_string(),
        kind: DecisionRouteKind::Lexical,
        argv: provider.agent_facade_argv_from_provider_argv(argv),
        stdin_mode: None,
    }
}

fn search_json_owner_path(argv: &[String]) -> Option<&str> {
    if argv.get(1).map(String::as_str) != Some("search")
        || argv.get(2).map(String::as_str) != Some("owner")
    {
        return None;
    }
    let path = argv.get(3)?.as_str();
    (path != "." && !path.starts_with('-')).then_some(path)
}
