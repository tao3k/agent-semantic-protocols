use serde_json::json;

use super::CommandDecisionShard;

fn decision(label: &str) -> crate::HookDecision {
    let mut decision = crate::asp_no_agent_passthrough_decision(
        "codex",
        "pre-tool",
        &json!({"tool_name":"semantic-shell", "tool_input":{"command":label}}),
    );
    decision.message = label.to_owned();
    decision
}

#[test]
fn normalized_longest_prefix_selects_the_config_compiled_winner() {
    let shard = CommandDecisionShard::new(vec![
        (Vec::new(), decision("empty-must-never-match")),
        (
            vec!["cargo".to_owned(), "test".to_owned()],
            decision("testing"),
        ),
        (vec!["cargo".to_owned()], decision("cargo")),
    ])
    .expect("compile decision shard")
    .to_binary_bytes()
    .expect("encode decision shard");
    let command = ["wrapper", "cargo", "test", "--workspace"].map(str::to_owned);
    let selected = CommandDecisionShard::select(&shard, &command)
        .expect("select decision")
        .expect("matching decision");
    assert_eq!(selected.message, "testing");
}

#[test]
fn unknown_or_corrupt_tables_fail_closed_without_inventing_a_winner() {
    let shard = CommandDecisionShard::new(vec![(
        vec!["cargo".to_owned(), "test".to_owned()],
        decision("testing"),
    )])
    .expect("compile decision shard")
    .to_binary_bytes()
    .expect("encode decision shard");
    assert!(
        CommandDecisionShard::select(&shard, &["cargo".to_owned(), "build".to_owned()])
            .expect("decode valid shard")
            .is_none()
    );
    assert!(CommandDecisionShard::select(&shard[..shard.len() - 1], &[]).is_err());
}

#[test]
fn wrapped_shell_source_candidates_do_not_depend_on_first_or_last_dotted_argument() {
    let payload = json!({
        "tool_name": "Bash",
        "tool_input": {
            "command": "wrapper --activation state.json read src/owner.rs --projection .fields.value"
        }
    });

    let keys = crate::shell_read_source_keys(&payload);
    let candidates = keys
        .iter()
        .map(|key| (key.extension.as_str(), key.path.as_str()))
        .collect::<Vec<_>>();

    assert!(
        candidates.contains(&(".json", "state.json")),
        "{candidates:?}"
    );
    assert!(
        candidates.contains(&(".rs", "src/owner.rs")),
        "{candidates:?}"
    );
    assert!(
        candidates.contains(&(".value", ".fields.value")),
        "{candidates:?}"
    );
}
