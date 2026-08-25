use agent_semantic_config::{
    HookClientActionKind, HookClientActionSubjectKind, HookClientCapabilityPolicyConfig,
    HookClientHostInvocationKind, LanguageId, ProviderId,
};

use super::{AgentActionMatch, AgentActionMatchConfig};
use crate::HookRuntime;
use crate::action_ir::{
    AgentAction, AgentActionKind, HostInvocationFact, HostInvocationKind, SemanticCapability,
    SemanticCapabilityEvidence,
};
use crate::tool_action::ToolAction;

fn runtime() -> HookRuntime {
    HookRuntime {
        project_root: ".".to_owned(),
        rankers: Vec::new(),
        providers: Vec::new(),
        policy_providers: Vec::new(),
    }
}

fn action(host: HostInvocationKind, capability: AgentActionKind) -> AgentAction {
    AgentAction {
        host: HostInvocationFact {
            action: host,
            tool_name: "test-tool".to_owned(),
            surface: "test-surface".to_owned(),
            payload: serde_json::Value::Null,
            invocation_source: None,
        },
        capabilities: vec![SemanticCapability {
            action: capability,
            evidence: SemanticCapabilityEvidence::HostInvocation,
        }],
        subjects: Vec::new(),
    }
}

fn rust_runtime() -> HookRuntime {
    let command = crate::protocol::CommandTemplate {
        argv: vec!["asp".to_owned()],
        stdin_mode: None,
    };
    HookRuntime {
        project_root: ".".to_owned(),
        rankers: Vec::new(),
        providers: Vec::new(),
        policy_providers: vec![
            crate::protocol_activation::protocol_activation_manifest::HookProviderProjection {
                language_id: LanguageId::new("rust"),
                provider_id: ProviderId::new("asp-rust"),
                package_roots: vec![".".to_owned()],
                source_extensions: vec![".rs".to_owned()],
                config_files: Vec::new(),
                policy: crate::protocol::HookPolicy {
                    direct_source_read: crate::protocol::ActionPolicy::Block,
                    bulk_source_dump: crate::protocol::ActionPolicy::Block,
                    raw_source_search: crate::protocol::ActionPolicy::Block,
                    agent_search_json: crate::protocol::ActionPolicy::Block,
                },
                owner_route: command.clone(),
                lexical_route: command.clone(),
                ingest_route: command,
            },
        ],
    }
}

fn semantic_policy(
    id: &str,
    semantic_capability_any: Vec<HookClientActionKind>,
) -> HookClientCapabilityPolicyConfig {
    HookClientCapabilityPolicyConfig {
        id: id.to_owned(),
        host_invocation_any: Vec::new(),
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
fn registered_shell_source_operand_does_not_invent_read_capability() {
    let read_matcher = AgentActionMatch::new(AgentActionMatchConfig {
        action_any: vec![HookClientActionKind::Read],
        ..AgentActionMatchConfig::default()
    });
    let execute_matcher = AgentActionMatch::new(AgentActionMatchConfig {
        host_invocation_any: vec![HookClientHostInvocationKind::Execute],
        ..AgentActionMatchConfig::default()
    });
    let action = ToolAction::normalized_shell_command_action(
        "future-source-consumer src/lib.rs".to_owned(),
        "Bash".to_owned(),
    );
    let paths = ["src/lib.rs".to_owned()];

    assert!(!read_matcher.matches(&rust_runtime(), &action, Some(&paths)));
    assert!(execute_matcher.matches(&rust_runtime(), &action, Some(&paths)));
    let receipt = execute_matcher
        .derive_agent_action_for_rule(&rust_runtime(), &action, Some(&paths), None)
        .expect("AgentAction receipt")
        .receipt_value();
    assert_eq!(receipt["hostInvocation"]["action"], "execute");
    assert!(
        receipt["semanticCapabilities"]
            .as_array()
            .is_some_and(|capabilities| capabilities
                .iter()
                .all(|capability| capability["action"] != "read"))
    );
}

#[test]
fn host_execute_and_semantic_read_are_independent_predicate_axes() {
    let matcher = AgentActionMatch::new(AgentActionMatchConfig {
        policy_all: vec![HookClientCapabilityPolicyConfig {
            id: "execute-with-parser-read".to_owned(),
            host_invocation_any: vec![HookClientHostInvocationKind::Execute],
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
        host_invocation_any: Vec::new(),
        semantic_capability_any: Vec::new(),
        subject_kind_any: vec![HookClientActionSubjectKind::RegisteredLanguageSource],
    };
    assert!(configured.host_invocation_any.is_empty());
    assert_eq!(configured.subject_kind_any.len(), 1);
}

#[test]
fn rule_actions_and_host_invocations_are_independent_conjunctive_axes() {
    let matcher = AgentActionMatch::new(AgentActionMatchConfig {
        action_any: vec![HookClientActionKind::Read],
        host_invocation_any: vec![HookClientHostInvocationKind::Mcp],
        ..AgentActionMatchConfig::default()
    });

    assert!(matcher.matches_envelope(&action(HostInvocationKind::Mcp, AgentActionKind::Read,)));
    assert!(
        !matcher.matches_envelope(&action(HostInvocationKind::Execute, AgentActionKind::Read,))
    );
    assert!(!matcher.matches_envelope(&action(HostInvocationKind::Mcp, AgentActionKind::Edit,)));
}
