use agent_semantic_hook::aot_evaluator::{evaluate_pre_tool, reader_probe_request};
use agent_semantic_hook::{
    ReaderProbeAccess, bind_reader_probe_observation, diagnose_reader_probe,
};

use super::aot_evaluator_contract::canonical_generation;

#[test]
fn every_declared_reader_pattern_accepts_minimal_and_extended_argv() {
    let generation = canonical_generation();
    let config = agent_semantic_config::default_hook_client_config_file()
        .expect("load canonical Hook Config V1");
    assert!(!config.reader_behavior_patterns.is_empty());

    for pattern in &config.reader_behavior_patterns {
        for tokens in pattern_witnesses(pattern, "src/lib.rs") {
            assert_reader_route(&generation, &config, pattern, tokens);
        }
    }
}

fn assert_reader_route(
    generation: &str,
    config: &agent_semantic_config::HookClientConfigFile,
    pattern: &[String],
    tokens: Vec<String>,
) {
    let command = tokens.join(" ");
    let mut payload = serde_json::json!({
        "session_id": "testkit-canonical-reader-catalog",
        "cwd": ".",
        "hook_event_name": "PreToolUse",
        "tool_name": "Bash",
        "tool_input": {"command": command}
    });
    let request = reader_probe_request(generation, &payload.to_string(), "Bash")
        .unwrap_or_else(|error| panic!("pattern={pattern:?} command={command:?}: {error}"))
        .unwrap_or_else(|| panic!("pattern={pattern:?} command={command:?}: no Reader request"));
    let observation = diagnose_reader_probe(
        request.command_tokens,
        request.subject,
        request.wrapped_command,
        request.reader_behavior_patterns,
    )
    .unwrap_or_else(|| panic!("pattern={pattern:?} command={command:?}: no observation"));
    assert_eq!(
        observation.access,
        ReaderProbeAccess::Read,
        "pattern={pattern:?} command={command:?} observation={observation:?}"
    );
    assert!(!observation.probe_process_launched);
    assert!(
        observation.elapsed_micros < 1_000,
        "static Reader catalog exceeded 1ms: pattern={pattern:?} command={command:?} observation={observation:?}"
    );
    bind_reader_probe_observation(&mut payload, Some(&observation)).expect("bind Reader fact");
    let observed_payload = payload.to_string();
    let decision = evaluate_pre_tool(generation, &observed_payload, "Bash")
        .unwrap_or_else(|error| panic!("pattern={pattern:?} command={command:?}: {error}"))
        .unwrap_or_else(|| panic!("pattern={pattern:?} command={command:?}: no deny"));
    assert_eq!(decision.decision, "deny", "pattern={pattern:?}");
    let base_rule = config
        .rules
        .iter()
        .find(|rule| rule.id == "route-read-to-asp-languages")
        .expect("canonical base Reader route");
    let selected_rule = config
        .rules
        .iter()
        .find(|rule| rule.id == decision.config_rule_id)
        .unwrap_or_else(|| panic!("unknown selected rule {}", decision.config_rule_id));
    assert!(
        selected_rule.priority >= base_rule.priority,
        "a lower-priority rule cannot dominate the confirmed Reader route: pattern={pattern:?} selected={}",
        selected_rule.id
    );
    if selected_rule.id == base_rule.id {
        assert_eq!(decision.profile, Some("rust"));
    } else {
        assert!(
            selected_rule.priority > base_rule.priority,
            "only a strictly dominant specialized policy may replace the profile Reader route"
        );
    }
    assert!(
        decision.route.is_some(),
        "selected deny must declare its Agent route"
    );
    assert_eq!(
        decision.access, "read",
        "pattern={pattern:?} command={command:?} selected={}",
        decision.config_rule_id
    );
    assert_eq!(
        decision.evidence, "reader-behavior-static-catalog",
        "pattern={pattern:?} command={command:?}"
    );
    assert!(decision.message.contains("collaboration.spawn_agent({"));
    assert!(decision.message.contains("collaboration.list_agents({"));
    assert!(decision.message.contains("standardized JSON"));
    assert!(decision.message.contains("diagnostic live-Agent snapshot"));
    assert!(
        decision
            .message
            .contains("start a new turn on that same canonical Agent path")
    );
    assert!(decision.message.contains("without starting a second turn"));
    for internal_field in ["testProcessLaunched", "failureLayer", "reasonKind"] {
        assert!(
            !decision.message.contains(internal_field),
            "natural-language guidance leaked receipt field {internal_field}: {}",
            decision.message
        );
    }
    let typed = serde_json::to_value(&decision).expect("serialize typed Reader decision");
    let host = agent_semantic_hook::render_codex_pre_tool_deny(&typed, &decision.message);
    assert_eq!(
        host["hookSpecificOutput"]["additionalContext"],
        decision.message
    );
}

fn pattern_witnesses(pattern: &[String], subject: &str) -> [Vec<String>; 2] {
    let minimal = materialize_pattern(pattern, subject, false);
    let extended = materialize_pattern(pattern, subject, true);
    [minimal, extended]
}

fn materialize_pattern(pattern: &[String], subject: &str, extended: bool) -> Vec<String> {
    let mut tokens = pattern
        .iter()
        .map(|token| glob_witness(token, subject, extended))
        .collect::<Vec<_>>();
    if extended {
        tokens.push("arbitrary-extra-argument".to_owned());
    }
    if !tokens.iter().any(|token| token.contains(subject)) {
        tokens.push(subject.to_owned());
    }
    tokens
}

fn glob_witness(pattern: &str, subject: &str, extended: bool) -> String {
    if !pattern.contains('*') && !pattern.contains('?') {
        return pattern.to_owned();
    }
    let matcher = globset::Glob::new(pattern)
        .expect("validated Config glob")
        .compile_matcher();
    let candidates = if extended {
        ["fixture".to_owned(), "HEAD:fixture.txt".to_owned()]
    } else {
        [subject.to_owned(), format!("HEAD:{subject}")]
    };
    candidates
        .into_iter()
        .find(|candidate| matcher.is_match(candidate))
        .unwrap_or_else(|| panic!("no deterministic witness for validated argv glob {pattern:?}"))
}
