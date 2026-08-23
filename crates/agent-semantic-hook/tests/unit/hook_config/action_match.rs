use agent_semantic_config::{
    HookClientActionKind, HookClientActionSubjectKind, HookClientCapabilityPolicyConfig,
};

use super::{AgentActionMatch, AgentActionMatchConfig};
use crate::HookRuntime;
use crate::tool_action::ToolAction;

fn runtime() -> HookRuntime {
    HookRuntime {
        project_root: ".".to_owned(),
        rankers: Vec::new(),
        providers: Vec::new(),
        policy_providers: Vec::new(),
    }
}

fn policy(id: &str, action_any: Vec<HookClientActionKind>) -> HookClientCapabilityPolicyConfig {
    HookClientCapabilityPolicyConfig {
        id: id.to_owned(),
        action_any,
        semantic_capability_any: Vec::new(),
        subject_kind_any: Vec::new(),
    }
}

fn semantic_policy(
    id: &str,
    semantic_capability_any: Vec<HookClientActionKind>,
) -> HookClientCapabilityPolicyConfig {
    HookClientCapabilityPolicyConfig {
        id: id.to_owned(),
        action_any: Vec::new(),
        semantic_capability_any,
        subject_kind_any: Vec::new(),
    }
}

#[test]
fn host_native_read_is_an_exact_semantic_capability() {
    let matcher = AgentActionMatch::new(AgentActionMatchConfig {
        action_any: vec![HookClientActionKind::Read],
        ..AgentActionMatchConfig::default()
    });
    let action = ToolAction::normalized_direct_policy_action("src/lib.rs".to_owned());

    assert!(matcher.matches(&runtime(), &action, None));
    let receipt = matcher
        .derive_agent_action_for_rule(&runtime(), &action, None, None)
        .expect("AgentAction receipt")
        .receipt_value();
    assert_eq!(receipt["hostInvocation"]["action"], "read");
    assert_eq!(receipt["semanticCapabilities"][0]["action"], "read");
    assert_eq!(
        receipt["semanticCapabilities"][0]["evidence"],
        "host-invocation"
    );
    assert!(
        receipt["semanticCapabilities"][0]
            .get("authority")
            .is_none()
    );
}

#[test]
fn host_native_edit_is_not_derived_from_shell_syntax() {
    let matcher = AgentActionMatch::new(AgentActionMatchConfig {
        action_any: vec![HookClientActionKind::Edit],
        ..AgentActionMatchConfig::default()
    });
    let actions = crate::tool_action::collect_tool_actions(
        "Edit",
        &serde_json::json!({
            "file_path": "src/lib.rs",
            "old_string": "old",
            "new_string": "new"
        }),
    );
    let action = actions.first().expect("native Edit action");
    assert!(matcher.matches(&runtime(), action, None));
    let receipt = matcher
        .derive_agent_action_for_rule(&runtime(), action, None, None)
        .expect("AgentAction")
        .receipt_value();
    assert_eq!(receipt["hostInvocation"]["action"], "edit");
    assert_eq!(
        receipt["hostInvocation"]["payload"]["file_path"],
        "src/lib.rs"
    );
    assert_eq!(
        receipt["semanticCapabilities"][0]["evidence"],
        "host-invocation"
    );
}

#[test]
fn opaque_shell_stage_does_not_invent_read_or_edit() {
    let read_matcher = AgentActionMatch::new(AgentActionMatchConfig {
        action_any: vec![HookClientActionKind::Read],
        ..AgentActionMatchConfig::default()
    });
    let execute_matcher = AgentActionMatch::new(AgentActionMatchConfig {
        policy_all: vec![policy(
            "opaque-shell-source-access",
            vec![HookClientActionKind::Execute],
        )],
        ..AgentActionMatchConfig::default()
    });
    let action = ToolAction::normalized_shell_command_action(
        "opaque-source-consumer src/lib.rs".to_owned(),
        "Bash".to_owned(),
    );

    assert!(!read_matcher.matches(&runtime(), &action, None));
    assert!(execute_matcher.matches(&runtime(), &action, None));
}

#[test]
fn host_execute_and_semantic_read_are_independent_predicate_axes() {
    let matcher = AgentActionMatch::new(AgentActionMatchConfig {
        policy_all: vec![HookClientCapabilityPolicyConfig {
            id: "execute-with-parser-read".to_owned(),
            action_any: vec![HookClientActionKind::Execute],
            semantic_capability_any: vec![HookClientActionKind::Read],
            subject_kind_any: Vec::new(),
        }],
        ..AgentActionMatchConfig::default()
    });
    let opaque = ToolAction::normalized_shell_command_action(
        "opaque src/lib.rs".to_owned(),
        "Bash".to_owned(),
    );
    let redirected = ToolAction::normalized_shell_command_action(
        "opaque < src/lib.rs".to_owned(),
        "Bash".to_owned(),
    );

    assert!(!matcher.matches(&runtime(), &opaque, None));
    assert!(matcher.matches(&runtime(), &redirected, None));
}

#[test]
fn shell_redirection_ast_projects_read_and_edit_capabilities() {
    for (command, expected) in [
        ("opaque < src/lib.rs", HookClientActionKind::Read),
        ("opaque > out.txt", HookClientActionKind::Edit),
        ("opaque <> state.db", HookClientActionKind::Read),
        ("opaque <> state.db", HookClientActionKind::Edit),
    ] {
        let matcher = AgentActionMatch::new(AgentActionMatchConfig {
            policy_all: vec![semantic_policy("shell-redirection", vec![expected])],
            ..AgentActionMatchConfig::default()
        });
        let action =
            ToolAction::normalized_shell_command_action(command.to_owned(), "Bash".to_owned());
        assert!(
            matcher.matches(&runtime(), &action, None),
            "{command} => {expected:?}"
        );
    }
}

#[test]
fn heredoc_is_not_projected_as_filesystem_read() {
    let matcher = AgentActionMatch::new(AgentActionMatchConfig {
        policy_all: vec![semantic_policy(
            "filesystem-read",
            vec![HookClientActionKind::Read],
        )],
        ..AgentActionMatchConfig::default()
    });
    let action =
        ToolAction::normalized_shell_command_action("opaque <<EOF".to_owned(), "Bash".to_owned());
    assert!(!matcher.matches(&runtime(), &action, None));
}

#[test]
fn subject_axis_remains_orthogonal_to_capability_axis() {
    let configured = HookClientCapabilityPolicyConfig {
        id: "registered-source".to_owned(),
        action_any: Vec::new(),
        semantic_capability_any: Vec::new(),
        subject_kind_any: vec![HookClientActionSubjectKind::RegisteredLanguageSource],
    };
    assert!(configured.action_any.is_empty());
    assert_eq!(configured.subject_kind_any.len(), 1);
}
