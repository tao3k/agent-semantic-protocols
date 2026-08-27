use agent_semantic_config::{LanguageId, ProviderId};

use super::match_registered_asp_command;
use crate::tool_action::ToolAction;
use crate::{CommandTemplate, HookPolicy, HookProviderProjection, HookRuntime};

fn route() -> CommandTemplate {
    CommandTemplate {
        argv: Vec::new(),
        stdin_mode: None,
    }
}

fn rust_runtime() -> HookRuntime {
    HookRuntime {
        project_root: ".".to_owned(),
        rankers: Vec::new(),
        providers: Vec::new(),
        policy_providers: vec![HookProviderProjection {
            language_id: LanguageId::new("rust"),
            provider_id: ProviderId::new("asp-rust"),
            package_roots: vec!["crates".to_owned()],
            source_extensions: vec![".rs".to_owned()],
            config_files: Vec::new(),
            policy: HookPolicy::default(),
            owner_route: route(),
            lexical_route: route(),
            ingest_route: route(),
        }],
    }
}

#[test]
fn direct_registered_asp_search_matches_the_declarative_language_pattern() {
    let action = ToolAction::normalized_shell_command_action(
        "asp rust search pipe 'HookDecision' --workspace . --view seeds".to_owned(),
        "Bash".to_owned(),
    );
    let patterns = vec![vec![
        "asp".to_owned(),
        "<registered-language>".to_owned(),
        "search".to_owned(),
    ]];

    let runtime = rust_runtime();
    let matched = match_registered_asp_command(&patterns, &runtime, &action)
        .expect("direct registered ASP search match");
    assert_eq!(matched.language_id.as_str(), "rust");
}
