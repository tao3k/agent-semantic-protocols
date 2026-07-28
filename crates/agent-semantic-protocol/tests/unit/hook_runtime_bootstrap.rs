use super::{AspNoAgentSource, asp_no_agent_source};
use serde_json::{Value, json};
use std::time::{Duration, Instant};

fn payload(command: &str) -> Value {
    json!({"tool_input": {"command": command}})
}

#[test]
fn inherited_opt_out_wins_before_command_inspection() {
    assert_eq!(
        asp_no_agent_source(&payload("cargo test"), true),
        Some(AspNoAgentSource::InheritedEnvironment)
    );
}

#[test]
fn parsed_leading_inline_assignment_opts_out_for_one_command() {
    assert_eq!(
        asp_no_agent_source(&payload("ASP_NO_AGENT=1 cargo test"), false),
        Some(AspNoAgentSource::InlineCommandEnvironment)
    );
    assert_eq!(asp_no_agent_source(&payload("cargo test"), false), None);
    assert_eq!(
        asp_no_agent_source(&payload("cargo test; ASP_NO_AGENT=1 echo late"), false),
        None
    );
}

#[test]
fn inline_assignment_detection_stays_sub_millisecond_on_hot_path() {
    let payload = payload("ASP_NO_AGENT=1 cargo test");
    let started = Instant::now();
    for _ in 0..1_000 {
        assert!(asp_no_agent_source(&payload, false).is_some());
    }
    assert!(
        started.elapsed() < Duration::from_secs(1),
        "1,000 inline opt-out classifications exceeded one second"
    );
}
