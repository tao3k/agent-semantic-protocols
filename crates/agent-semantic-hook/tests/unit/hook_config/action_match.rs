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
        filesystem_permissions: Vec::new(),
        capabilities: vec![SemanticCapability {
            action: capability,
            evidence: SemanticCapabilityEvidence::HostMatcher,
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
                playbook_route: command,
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
fn canonical_apply_patch_and_official_aliases_share_host_semantics() {
    let matcher = AgentActionMatch::new(AgentActionMatchConfig {
        native_matcher_any: vec!["apply_patch".to_owned()],
        ..AgentActionMatchConfig::default()
    });
    let mut actions = crate::tool_action::collect_tool_actions(
        "apply_patch",
        &serde_json::json!({
            "command": "*** Begin Patch\n*** Update File: src/lib.rs\n@@\n-old\n+new\n*** End Patch"
        }),
    );
    actions[0].host_action = HostInvocationKind::Edit;
    let action = actions.first().expect("canonical apply_patch action");

    assert!(matcher.matches(&runtime(), "codex", &action, None));
    let receipt = matcher
        .derive_agent_action_for_rule(&runtime(), "codex", &action, None, None)
        .expect("AgentAction receipt")
        .receipt_value();
    assert_eq!(receipt["hostInvocation"]["action"], "edit");
    assert_eq!(receipt["semanticCapabilities"][0]["action"], "edit");
    assert_eq!(
        receipt["semanticCapabilities"][0]["evidence"],
        "host-matcher"
    );
    assert!(
        receipt["semanticCapabilities"][0]
            .get("authority")
            .is_none()
    );
    assert_eq!(receipt["filesystemPermissions"][0]["permission"], "write");
    assert_eq!(
        receipt["filesystemPermissions"][0]["source"],
        "host-matcher"
    );
    assert_eq!(receipt["filesystemPermissions"][0]["subject"], "src/lib.rs");

    let aliases = AgentActionMatch::new(AgentActionMatchConfig {
        native_matcher_any: vec!["Edit|Write".to_owned()],
        ..AgentActionMatchConfig::default()
    });
    assert!(aliases.matches(&runtime(), "codex", action, None));
}

#[test]
fn canonical_apply_patch_edit_is_not_derived_from_shell_syntax() {
    let matcher = AgentActionMatch::new(AgentActionMatchConfig {
        native_matcher_any: vec!["apply_patch".to_owned()],
        ..AgentActionMatchConfig::default()
    });
    let mut actions = crate::tool_action::collect_tool_actions(
        "apply_patch",
        &serde_json::json!({
            "file_path": "src/lib.rs",
            "old_string": "old",
            "new_string": "new"
        }),
    );
    actions[0].host_action = HostInvocationKind::Edit;
    let action = actions.first().expect("canonical apply_patch action");
    assert!(matcher.matches(&runtime(), "codex", action, None));
    let receipt = matcher
        .derive_agent_action_for_rule(&runtime(), "codex", action, None, None)
        .expect("AgentAction")
        .receipt_value();
    assert_eq!(receipt["hostInvocation"]["action"], "edit");
    assert_eq!(
        receipt["hostInvocation"]["payload"]["file_path"],
        "src/lib.rs"
    );
    assert_eq!(
        receipt["semanticCapabilities"][0]["evidence"],
        "host-matcher"
    );
    assert_eq!(receipt["filesystemPermissions"][0]["permission"], "write");
    assert_eq!(
        receipt["filesystemPermissions"][0]["source"],
        "host-matcher"
    );
    assert_eq!(receipt["filesystemPermissions"][0]["subject"], "src/lib.rs");
}

#[test]
fn filesystem_read_permission_projects_read_action_for_any_executable() {
    let read_matcher = AgentActionMatch::new(AgentActionMatchConfig {
        policy_all: vec![semantic_policy(
            "read-permission",
            vec![HookClientActionKind::Read],
        )],
        ..AgentActionMatchConfig::default()
    });
    let execute_matcher = AgentActionMatch::new(AgentActionMatchConfig {
        host_invocation_any: vec![HookClientHostInvocationKind::Execute],
        ..AgentActionMatchConfig::default()
    });
    let action = ToolAction::normalized_shell_command_action(
        "future-source-consumer < src/lib.rs".to_owned(),
        "Bash".to_owned(),
    );
    let paths = ["src/lib.rs".to_owned()];

    assert!(read_matcher.matches(&rust_runtime(), "codex", &action, Some(paths.as_slice())));
    assert!(execute_matcher.matches(&rust_runtime(), "codex", &action, Some(paths.as_slice())));
    let receipt = execute_matcher
        .derive_agent_action_for_rule(
            &rust_runtime(),
            "codex",
            &action,
            Some(paths.as_slice()),
            None,
        )
        .expect("AgentAction receipt")
        .receipt_value();
    assert_eq!(receipt["hostInvocation"]["action"], "execute");
    assert!(
        receipt["semanticCapabilities"]
            .as_array()
            .is_some_and(|capabilities| capabilities.iter().any(|capability| {
                capability["action"] == "read" && capability["evidence"] == "shell-redirection"
            }))
    );
    assert_eq!(receipt["filesystemPermissions"][0]["permission"], "read");
    assert_eq!(receipt["filesystemPermissions"][0]["subject"], "src/lib.rs");
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
        "opaque token-without-source".to_owned(),
        "Bash".to_owned(),
    );
    let redirected = ToolAction::normalized_shell_command_action(
        "opaque < src/lib.rs".to_owned(),
        "Bash".to_owned(),
    );

    assert!(!matcher.matches(&runtime(), "codex", &opaque, None));
    assert!(matcher.matches(&runtime(), "codex", &redirected, None));
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
            matcher.matches(&runtime(), "codex", &action, None),
            "{command} => {expected:?}"
        );
    }
}

#[test]
fn unresolved_source_access_projection_is_independent_of_the_executable_name() {
    let matcher = AgentActionMatch::new(AgentActionMatchConfig {
        native_matcher_any: vec!["Bash".to_owned()],
        ..AgentActionMatchConfig::default()
    });
    let action = ToolAction::normalized_shell_command_action(
        "future-source-consumer --mode opaque src/lib.rs".to_owned(),
        "Bash".to_owned(),
    );

    assert!(matcher.matches(&rust_runtime(), "codex", &action, None));
    let receipt = matcher
        .derive_agent_action_for_rule(&rust_runtime(), "codex", &action, None, None)
        .expect("filesystem permission action receipt")
        .receipt_value();
    assert_eq!(receipt["hostInvocation"]["action"], "execute");
    assert!(
        receipt["semanticCapabilities"]
            .as_array()
            .is_some_and(|capabilities| capabilities.iter().any(|capability| {
                capability["action"] == "unknown"
                    && capability["evidence"] == "registered-source-operand"
            }))
    );
    assert!(
        receipt["filesystemPermissions"]
            .as_array()
            .is_some_and(Vec::is_empty)
    );
}

#[test]
fn explicit_write_permission_prevents_a_source_operand_from_being_guessed_as_read() {
    let read_matcher = AgentActionMatch::new(AgentActionMatchConfig {
        policy_all: vec![semantic_policy(
            "read-permission",
            vec![HookClientActionKind::Read],
        )],
        ..AgentActionMatchConfig::default()
    });
    let edit_matcher = AgentActionMatch::new(AgentActionMatchConfig {
        policy_all: vec![semantic_policy(
            "write-permission",
            vec![HookClientActionKind::Edit],
        )],
        ..AgentActionMatchConfig::default()
    });
    let action = ToolAction::normalized_shell_command_action(
        "future-source-producer > src/generated.rs".to_owned(),
        "Bash".to_owned(),
    );

    assert!(!read_matcher.matches(&rust_runtime(), "codex", &action, None));
    assert!(edit_matcher.matches(&rust_runtime(), "codex", &action, None));
    let receipt = edit_matcher
        .derive_agent_action_for_rule(&rust_runtime(), "codex", &action, None, None)
        .expect("subject-bound write permission receipt")
        .receipt_value();
    assert!(
        receipt["filesystemPermissions"]
            .as_array()
            .is_some_and(|permissions| permissions.iter().any(|permission| {
                permission["permission"] == "write"
                    && permission["source"] == "shell-redirection"
                    && permission["subject"] == "src/generated.rs"
            }))
    );
}

#[test]
fn read_write_permission_preserves_both_facts_for_the_same_subject() {
    let action = ToolAction::normalized_shell_command_action(
        "future-source-transformer <> src/state.rs".to_owned(),
        "Bash".to_owned(),
    );
    let matcher = AgentActionMatch::new(AgentActionMatchConfig::default());
    let receipt = matcher
        .derive_agent_action_for_rule(&rust_runtime(), "codex", &action, None, None)
        .expect("read-write permission receipt")
        .receipt_value();
    let permissions = receipt["filesystemPermissions"]
        .as_array()
        .expect("filesystem permission facts");

    assert!(permissions.iter().any(|permission| {
        permission["permission"] == "read" && permission["subject"] == "src/state.rs"
    }));
    assert!(permissions.iter().any(|permission| {
        permission["permission"] == "write" && permission["subject"] == "src/state.rs"
    }));
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
    assert!(!matcher.matches(&runtime(), "codex", &action, None));
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
        native_matcher_any: vec!["test-tool".to_owned()],
        host_invocation_any: vec![HookClientHostInvocationKind::Mcp],
        policy_all: vec![semantic_policy(
            "read-capability",
            vec![HookClientActionKind::Read],
        )],
        ..AgentActionMatchConfig::default()
    });

    assert!(matcher.matches_envelope(&action(HostInvocationKind::Mcp, AgentActionKind::Read,)));
    assert!(
        !matcher.matches_envelope(&action(HostInvocationKind::Execute, AgentActionKind::Read,))
    );
    assert!(!matcher.matches_envelope(&action(HostInvocationKind::Mcp, AgentActionKind::Edit,)));
}
