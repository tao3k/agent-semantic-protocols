use super::{AgentActionMatch, AgentActionMatchConfig};
use crate::protocol_activation::protocol_activation_manifest::HookRuntime;
use crate::tool_action::collect_tool_actions;
use agent_semantic_config::{HookClientConfigFile, default_hook_client_config_template};
use serde_json::json;

fn raw_source_action_match() -> (
    AgentActionMatch,
    Vec<agent_semantic_config::AgentActionEffectRule>,
    Vec<agent_semantic_config::AgentActionAuthorityRule>,
) {
    let document = toml::from_str::<HookClientConfigFile>(&default_hook_client_config_template())
        .expect("production hook config parses");
    let rule = document
        .rules
        .iter()
        .find(|rule| rule.id == "deny-raw-registered-source-action")
        .expect("production raw-source action rule");
    let config = rule.match_config.clone();
    let resolve = |references: &[String]| {
        references
            .iter()
            .map(|reference| {
                document
                    .action_policies
                    .iter()
                    .find(|policy| policy.id == *reference)
                    .cloned()
                    .expect("production action policy reference")
            })
            .collect::<Vec<_>>()
    };
    let effect_rules = config.effect_rules.clone();
    let authority_rules = config.authority_rules.clone();
    (
        AgentActionMatch::new(AgentActionMatchConfig {
            authority_rules: config.authority_rules,
            effect_rules: config.effect_rules,
            action_any: config.action_any,
            effect_any: config.effect_any,
            subject_kind_any: config.subject_kind_any,
            authority_any: config.authority_any,
            authority_exclude_any: config.authority_exclude_any,
            policy_all: resolve(&config.action_policy_all),
            policy_any: resolve(&config.action_policy_any),
            policy_none: resolve(&config.action_policy_none),
        }),
        effect_rules,
        authority_rules,
    )
}

fn command_for_prefix(prefix: &[String]) -> String {
    let prefix = prefix
        .iter()
        .map(|token| {
            if token == "<registered-language>" {
                "rust"
            } else {
                token
            }
        })
        .collect::<Vec<_>>()
        .join(" ");
    match prefix.as_str() {
        "cp" | "mv" => format!("{prefix} src/app.rs src/app2.rs"),
        "git mv" => "git mv src/app.rs src/app2.rs".to_owned(),
        "git show" => "git show HEAD:src/app.rs".to_owned(),
        prefix if prefix.starts_with("asp ") => {
            format!("{prefix} query --term item --workspace .")
        }
        _ => format!("{prefix} src/app.rs"),
    }
}

fn runtime() -> HookRuntime {
    HookRuntime {
        project_root: ".".to_owned(),
        rankers: Vec::new(),
        providers: Vec::new(),
        policy_providers: Vec::new(),
    }
}

#[test]
fn production_raw_source_derivation_rules_have_low_level_witnesses() {
    let (matcher, effect_rules, authority_rules) = raw_source_action_match();
    let runtime = runtime();
    let match_paths = vec!["src/app.rs".to_owned()];
    let mut witnessed = 0usize;

    for rule in effect_rules {
        let command = command_for_prefix(&rule.argv_prefix);
        let input = json!({"cmd": command});
        let action = collect_tool_actions("exec_command", &input)
            .into_iter()
            .next()
            .expect("shell tool action");
        let derived =
            matcher.derive_agent_action(&runtime, &action, Some(&match_paths), None, true);
        assert_eq!(
            format!("{:?}", derived.effect),
            format!("{:?}", rule.effect),
            "effect derivation drift for prefix {:?}: action={derived:?}",
            rule.argv_prefix
        );
        if format!("{:?}", rule.effect) == "Edit" {
            assert!(
                !matcher.matches_non_subject_envelope(&derived),
                "edit derivation must fail the read-only envelope matcher: {derived:?}"
            );
        }
        witnessed += 1;
    }

    for rule in authority_rules {
        let command = command_for_prefix(&rule.argv_prefix);
        let input = json!({"cmd": command});
        let action = collect_tool_actions("exec_command", &input)
            .into_iter()
            .next()
            .expect("shell tool action");
        let derived =
            matcher.derive_agent_action(&runtime, &action, Some(&match_paths), None, true);
        assert_eq!(
            format!("{:?}", derived.authority),
            format!("{:?}", rule.authority),
            "authority derivation drift for prefix {:?}: action={derived:?}",
            rule.argv_prefix
        );
        witnessed += 1;
    }

    assert_eq!(
        witnessed, 14,
        "production raw-source derivation rule count drifted"
    );
}
