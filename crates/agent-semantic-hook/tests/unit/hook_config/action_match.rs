use agent_semantic_command_match::normalize_bash_command_invocations;
use agent_semantic_config::{
    HookClientActionAuthority, HookClientActionKind, HookClientActionSubjectKind,
    HookClientCommandWrapper, HookClientFlagPresence, HookClientInvocationShape,
    HookClientWrapperMatch,
};

use super::SemanticActionMatch;
use crate::tool_action::{
    ActionAuthority, ActionSubject, ActionSubjectKind, HostActionKind, SemanticHostAction,
};

fn wrapped_matcher() -> SemanticActionMatch {
    SemanticActionMatch::new(
        vec![HookClientCommandWrapper {
            executable: "rtk".to_string(),
        }],
        vec![HookClientInvocationShape::WrappedCommand],
        vec![HookClientWrapperMatch::Matched],
        vec![
            HookClientFlagPresence::Present,
            HookClientFlagPresence::Absent,
        ],
        Vec::new(),
        Vec::new(),
        Vec::new(),
        Vec::new(),
        Vec::new(),
        Vec::new(),
        Vec::new(),
    )
}

#[test]
fn wrapper_match_accepts_flags_or_no_flags_without_inner_command_registry() {
    let matcher = wrapped_matcher();

    for command in ["rtk read *.rs", "rtk -q read --number *.rs"] {
        let invocations = normalize_bash_command_invocations(command, &matcher.command_wrappers)
            .expect("bash invocation should parse");
        assert!(matcher.matches_invocation_facts(HostActionKind::Execute, &invocations));
    }
}

#[test]
fn wrapper_match_is_registry_driven() {
    let matcher = wrapped_matcher();
    let invocations = normalize_bash_command_invocations("reader *.rs", &matcher.command_wrappers)
        .expect("direct bash invocation should parse");

    assert!(!matcher.matches_invocation_facts(HostActionKind::Execute, &invocations));
}

#[test]
fn source_expansion_requires_read_effect_even_for_registered_source_patterns() {
    let matcher = SemanticActionMatch::new(
        vec![HookClientCommandWrapper {
            executable: "rtk".to_string(),
        }],
        vec![HookClientInvocationShape::WrappedCommand],
        vec![HookClientWrapperMatch::Matched],
        vec![
            HookClientFlagPresence::Present,
            HookClientFlagPresence::Absent,
        ],
        vec![HookClientActionKind::Execute],
        vec![HookClientActionKind::Read],
        vec![HookClientActionSubjectKind::RegisteredLanguageSourcePattern],
        vec![HookClientActionAuthority::RawShell],
        Vec::new(),
        Vec::new(),
        Vec::new(),
    );
    let invocations =
        normalize_bash_command_invocations("rtk git mv old.rs new.rs", &matcher.command_wrappers)
            .expect("wrapped command should parse");
    let mut semantic = SemanticHostAction {
        action: HostActionKind::Execute,
        effect: HostActionKind::Edit,
        authority: ActionAuthority::RawShell,
        subjects: vec![ActionSubject {
            value: "old.rs".to_string(),
            kind: ActionSubjectKind::RegisteredLanguageSourcePattern,
        }],
    };

    assert!(!matcher.matches_envelope(&semantic, &invocations));

    semantic.effect = HostActionKind::Read;
    assert!(matcher.matches_envelope(&semantic, &invocations));
}

#[test]
fn wrapped_execute_with_projected_read_effect_matches_registered_source_pattern() {
    let matcher = SemanticActionMatch::new(
        vec![HookClientCommandWrapper {
            executable: "rtk".to_string(),
        }],
        vec![
            HookClientInvocationShape::HostNative,
            HookClientInvocationShape::Command,
            HookClientInvocationShape::WrappedCommand,
        ],
        vec![
            HookClientWrapperMatch::Matched,
            HookClientWrapperMatch::Unmatched,
            HookClientWrapperMatch::Unknown,
        ],
        vec![
            HookClientFlagPresence::Present,
            HookClientFlagPresence::Absent,
        ],
        vec![HookClientActionKind::Read, HookClientActionKind::Execute],
        vec![HookClientActionKind::Read],
        vec![HookClientActionSubjectKind::RegisteredLanguageSourcePattern],
        vec![HookClientActionAuthority::RawShell],
        Vec::new(),
        Vec::new(),
        vec![agent_semantic_config::HookClientEffectProjection {
            argv_prefix: vec!["read".to_string()],
            effect: HookClientActionKind::Read,
        }],
    );
    let invocations =
        normalize_bash_command_invocations("rtk read *.rs", &matcher.command_wrappers)
            .expect("wrapped command should parse");
    let mut semantic = SemanticHostAction {
        action: HostActionKind::Execute,
        effect: HostActionKind::Unknown,
        authority: ActionAuthority::RawShell,
        subjects: vec![ActionSubject {
            value: "*.rs".to_string(),
            kind: ActionSubjectKind::RegisteredLanguageSourcePattern,
        }],
    };

    semantic.effect = matcher
        .projected_effect(&invocations)
        .expect("registered wrapper must expose the inner read projection");
    assert_eq!(semantic.effect, HostActionKind::Read);
    assert!(matcher.matches_envelope(&semantic, &invocations));
}

#[test]
fn semantic_action_and_invocation_schemas_are_valid_json_objects() {
    for schema in [
        include_str!("../../../../../schemas/semantic-host-action.v1.schema.json"),
        include_str!("../../../../../schemas/semantic-action-match.v1.schema.json"),
        include_str!("../../../../../schemas/semantic-command-invocation.v1.schema.json"),
        include_str!("../../../../../schemas/semantic-invocation-match.v1.schema.json"),
        include_str!("../../../../../schemas/semantic-command-wrapper-registry.v1.schema.json"),
    ] {
        let document = serde_json::from_str::<serde_json::Value>(schema)
            .expect("semantic action schema should contain valid JSON");
        assert_eq!(document["type"], "object");
        assert_eq!(document["additionalProperties"], false);
        assert!(document["$id"].as_str().is_some_and(|id| !id.is_empty()));
    }
}

#[test]
fn host_native_read_matches_registered_source_without_command_parsing() {
    let matcher = SemanticActionMatch::new(
        Vec::new(),
        vec![HookClientInvocationShape::HostNative],
        vec![HookClientWrapperMatch::Unmatched],
        vec![HookClientFlagPresence::Absent],
        vec![HookClientActionKind::Read],
        vec![HookClientActionKind::Read],
        vec![HookClientActionSubjectKind::RegisteredLanguageSource],
        vec![HookClientActionAuthority::RawHostAction],
        Vec::new(),
        Vec::new(),
        Vec::new(),
    );
    let semantic = SemanticHostAction {
        action: HostActionKind::Read,
        effect: HostActionKind::Read,
        authority: ActionAuthority::RawHostAction,
        subjects: vec![ActionSubject {
            value: "src/lib.rs".to_string(),
            kind: ActionSubjectKind::RegisteredLanguageSource,
        }],
    };

    assert!(matcher.matches_envelope(&semantic, &[]));
}

#[test]
fn parser_owned_authority_does_not_match_raw_shell_safety_rule() {
    let matcher = SemanticActionMatch::new(
        Vec::new(),
        vec![HookClientInvocationShape::Command],
        vec![HookClientWrapperMatch::Unmatched],
        vec![HookClientFlagPresence::Absent],
        vec![HookClientActionKind::Execute],
        vec![HookClientActionKind::Unknown],
        vec![HookClientActionSubjectKind::RegisteredLanguageSource],
        vec![
            HookClientActionAuthority::RawShell,
            HookClientActionAuthority::Unknown,
        ],
        Vec::new(),
        Vec::new(),
        Vec::new(),
    );
    let invocations = normalize_bash_command_invocations("provider file.rs", &[])
        .expect("parser-owned command should parse");
    let semantic = SemanticHostAction {
        action: HostActionKind::Execute,
        effect: HostActionKind::Unknown,
        authority: ActionAuthority::ParserOwnedExactEvidence,
        subjects: vec![ActionSubject {
            value: "file.rs".to_string(),
            kind: ActionSubjectKind::RegisteredLanguageSource,
        }],
    };

    assert!(!matcher.matches_envelope(&semantic, &invocations));
}
