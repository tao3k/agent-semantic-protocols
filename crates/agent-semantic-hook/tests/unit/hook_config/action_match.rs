use agent_semantic_command_match::parse_bash_command_candidates;
use agent_semantic_config::{
    HookClientActionAuthority, HookClientActionKind, HookClientActionSubjectKind,
};

use super::{AgentActionMatch, AgentActionMatchConfig};
use crate::HookRuntime;
use crate::tool_action::{
    AgentAction, AgentActionAuthority, AgentActionKind, AgentActionSubject, AgentActionSubjectKind,
};

#[test]
fn source_expansion_requires_read_effect_even_for_registered_source_patterns() {
    let matcher = AgentActionMatch::new(AgentActionMatchConfig {
        action_any: vec![HookClientActionKind::Execute],
        effect_any: vec![HookClientActionKind::Read],
        subject_kind_any: vec![HookClientActionSubjectKind::RegisteredLanguageSourcePattern],
        authority_any: vec![HookClientActionAuthority::RawShell],
        ..Default::default()
    });
    let mut agent_action = AgentAction {
        action: AgentActionKind::Execute,
        effect: AgentActionKind::Edit,
        authority: AgentActionAuthority::RawShell,
        subjects: vec![AgentActionSubject {
            value: "old.rs".to_string(),
            kind: AgentActionSubjectKind::RegisteredLanguageSourcePattern,
        }],
    };

    assert!(!matcher.matches_envelope(&agent_action));

    agent_action.effect = AgentActionKind::Read;
    assert!(matcher.matches_envelope(&agent_action));
}

#[test]
fn arbitrary_wrapper_with_inferred_read_effect_matches_registered_source_pattern() {
    let matcher = AgentActionMatch::new(AgentActionMatchConfig {
        action_any: vec![HookClientActionKind::Read, HookClientActionKind::Execute],
        effect_any: vec![HookClientActionKind::Read],
        subject_kind_any: vec![HookClientActionSubjectKind::RegisteredLanguageSourcePattern],
        authority_any: vec![HookClientActionAuthority::RawShell],
        effect_rules: vec![agent_semantic_config::AgentActionEffectRule {
            argv_prefix: vec!["read".to_string()],
            command_contains_any: Vec::new(),
            effect: HookClientActionKind::Read,
        }],
        ..Default::default()
    });
    let command_stages = parse_bash_command_candidates("any-wrapper --mode safe read *.rs")
        .expect("wrapped command should parse");
    let mut agent_action = AgentAction {
        action: AgentActionKind::Execute,
        effect: AgentActionKind::Unknown,
        authority: AgentActionAuthority::RawShell,
        subjects: vec![AgentActionSubject {
            value: "*.rs".to_string(),
            kind: AgentActionSubjectKind::RegisteredLanguageSourcePattern,
        }],
    };

    agent_action.effect = matcher
        .infer_effect(&command_stages, Some("any-wrapper --mode safe read *.rs"))
        .expect("generic wrapper match must expose the inner read projection");
    assert_eq!(agent_action.effect, AgentActionKind::Read);
    assert!(matcher.matches_envelope(&agent_action));
}

#[test]
fn structured_query_program_is_not_projected_as_a_shell_subject() {
    let registry = HookRuntime {
        project_root: ".".to_string(),
        providers: Vec::new(),
    };
    let matcher = AgentActionMatch::new(AgentActionMatchConfig::default());
    let schema_path = "schemas/semantic-search-packet.v1.schema.json".to_string();
    let filter = "{properties: (.properties | to_entries[:32] | map({key, type: .value.type})), required: (.required[:32] // [])}".to_string();
    let action = crate::tool_action::ToolAction {
        tool_name: "Bash".to_string(),
        surface: crate::tool_action::ToolSurface::CodexShell,
        operation: crate::tool_action::OperationIntent::ShellCommand,
        command: Some(format!("jq '{filter}' {schema_path}")),
        command_tokens: None,
        paths: vec![filter, schema_path.clone()],
    };
    let structured_source_operands = [schema_path.clone()];

    let agent_action = matcher
        .derive_agent_action_for_rule(
            &registry,
            &action,
            Some(&action.paths),
            Some(&structured_source_operands),
        )
        .expect("configured action rule must derive a shell action");

    assert_eq!(
        agent_action
            .subjects
            .iter()
            .map(|subject| subject.value.as_str())
            .collect::<Vec<_>>(),
        [schema_path]
    );
}

#[test]
fn slash_operator_does_not_create_path_authority() {
    let registry = HookRuntime {
        project_root: ".".to_string(),
        providers: Vec::new(),
    };
    let matcher = AgentActionMatch::new(AgentActionMatchConfig::default());
    let schema_path = "schemas/semantic-search-packet.v1.schema.json".to_string();
    let filter = "{properties: (.properties | to_entries[:32]), required: (.required[:32] // [])}"
        .to_string();
    let action = crate::tool_action::ToolAction {
        tool_name: "Bash".to_string(),
        surface: crate::tool_action::ToolSurface::CodexShell,
        operation: crate::tool_action::OperationIntent::ShellCommand,
        command: Some(format!("jq '{filter}' {schema_path}")),
        command_tokens: None,
        paths: vec![filter, schema_path.clone()],
    };

    let agent_action = matcher
        .derive_agent_action_for_rule(&registry, &action, Some(&action.paths), None)
        .expect("configured action rule must derive a shell action");

    assert_eq!(
        agent_action
            .subjects
            .iter()
            .map(|subject| subject.value.as_str())
            .collect::<Vec<_>>(),
        [schema_path]
    );
}

#[test]
fn registered_source_path_remains_a_typed_shell_subject() {
    let registry = HookRuntime {
        project_root: ".".to_string(),
        providers: Vec::new(),
    };
    let source = "crates/agent-semantic-hook/src/tool_action.rs".to_string();
    let projected = crate::source_selector::project_shell_subject_paths(
        &registry,
        std::slice::from_ref(&source),
    );
    assert_eq!(projected, [source]);
}

#[test]
fn agent_action_and_invocation_schemas_are_valid_json_objects() {
    for schema in [
        include_str!("../../../../../schemas/agent-action.v1.schema.json"),
        include_str!("../../../../../schemas/agent-action-match.v1.schema.json"),
    ] {
        let document = serde_json::from_str::<serde_json::Value>(schema)
            .expect("agent action schema should contain valid JSON");
        assert_eq!(document["type"], "object");
        assert_eq!(document["additionalProperties"], false);
        assert!(document["$id"].as_str().is_some_and(|id| !id.is_empty()));
    }
}

#[test]
fn host_native_read_matches_registered_source_without_command_parsing() {
    let matcher = AgentActionMatch::new(AgentActionMatchConfig {
        action_any: vec![HookClientActionKind::Read],
        effect_any: vec![HookClientActionKind::Read],
        subject_kind_any: vec![HookClientActionSubjectKind::RegisteredLanguageSource],
        authority_any: vec![HookClientActionAuthority::RawHostAction],
        ..Default::default()
    });
    let agent_action = AgentAction {
        action: AgentActionKind::Read,
        effect: AgentActionKind::Read,
        authority: AgentActionAuthority::RawHostAction,
        subjects: vec![AgentActionSubject {
            value: "src/lib.rs".to_string(),
            kind: AgentActionSubjectKind::RegisteredLanguageSource,
        }],
    };

    assert!(matcher.matches_envelope(&agent_action));
}

#[test]
fn parser_owned_authority_does_not_match_raw_shell_safety_rule() {
    let matcher = AgentActionMatch::new(AgentActionMatchConfig {
        action_any: vec![HookClientActionKind::Execute],
        effect_any: vec![HookClientActionKind::Unknown],
        subject_kind_any: vec![HookClientActionSubjectKind::RegisteredLanguageSource],
        authority_any: vec![
            HookClientActionAuthority::RawShell,
            HookClientActionAuthority::Unknown,
        ],
        ..Default::default()
    });
    let agent_action = AgentAction {
        action: AgentActionKind::Execute,
        effect: AgentActionKind::Unknown,
        authority: AgentActionAuthority::ParserOwnedExactEvidence,
        subjects: vec![AgentActionSubject {
            value: "file.rs".to_string(),
            kind: AgentActionSubjectKind::RegisteredLanguageSource,
        }],
    };

    assert!(!matcher.matches_envelope(&agent_action));
}
