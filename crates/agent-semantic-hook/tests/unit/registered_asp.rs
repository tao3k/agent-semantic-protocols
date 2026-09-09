// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use agent_semantic_config::LanguageId;
use agent_semantic_config::ProviderId;

use super::match_registered_asp_command;
use crate::HookPolicy;
use crate::HookProviderProjection;
use crate::HookRuntime;
use crate::tool_action::ToolAction;

fn rust_runtime() -> HookRuntime {
    HookRuntime {
        project_root: ".".to_owned(),
        policy_providers: vec![HookProviderProjection {
            language_id: LanguageId::new("rust"),
            provider_id: ProviderId::new("asp-rust"),
            package_roots: vec!["crates".to_owned()],
            source_extensions: vec![".rs".to_owned()],
            config_files: Vec::new(),
            policy: HookPolicy::default(),
        }],
    }
}

#[test]
fn direct_registered_asp_search_matches_the_declarative_language_pattern() {
    let action = ToolAction::normalized_shell_command_action(
        "asp search playbook --language 'rust|python' --rg --files -g src/lib.rs --tantivy 'title:[* TO *] OR body:[* TO *]'".to_owned(),
        "Bash".to_owned(),
    );
    let patterns = vec![vec![
        "asp".to_owned(),
        "search".to_owned(),
        "playbook".to_owned(),
    ]];

    let runtime = rust_runtime();
    let matched = match_registered_asp_command(&patterns, &runtime, &action)
        .expect("direct registered ASP search match");
    assert_eq!(matched.language_id.as_str(), "rust");
}

#[test]
fn root_query_playbook_requires_matching_language_and_selector_scheme() {
    let action = ToolAction::normalized_shell_command_action(
        "asp query playbook --language rust --selector rust://src/lib.rs#item/function/run --projection source"
            .to_owned(),
        "Bash".to_owned(),
    );
    let patterns = vec![vec![
        "asp".to_owned(),
        "query".to_owned(),
        "playbook".to_owned(),
    ]];

    let runtime = rust_runtime();
    let matched = match_registered_asp_command(&patterns, &runtime, &action)
        .expect("root Query selector scheme matches its registered provider");
    assert_eq!(matched.language_id.as_str(), "rust");
}
