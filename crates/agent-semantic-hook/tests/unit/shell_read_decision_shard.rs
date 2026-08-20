use serde_json::json;

use super::CommandDecisionShard;

fn decision(label: &str) -> crate::HookDecision {
    let mut decision = crate::classify_hook(
        &crate::HookRuntime {
            project_root: ".".to_owned(),
            rankers: Vec::new(),
            providers: Vec::new(),
            policy_providers: Vec::new(),
        },
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
fn leading_environment_assignment_terminal_entry_precedes_longer_command_profile() {
    let shard = CommandDecisionShard::new_with_leading_environment_assignments(
        vec![(
            vec!["cargo".to_owned(), "test".to_owned()],
            decision("testing"),
        )],
        vec![(vec!["BYPASS=1".to_owned()], decision("terminal-bypass"))],
    )
    .expect("compile declarative environment matcher shard")
    .to_binary_bytes()
    .expect("encode declarative environment matcher shard");
    let command = "TRACE=1 BYPASS=1 cargo test";
    let tokens = ["TRACE=1", "BYPASS=1", "cargo", "test"].map(str::to_owned);
    let selected = CommandDecisionShard::select_for_command(&shard, command, &tokens)
        .expect("select declarative environment matcher")
        .expect("terminal environment decision");
    assert_eq!(selected.message, "terminal-bypass");

    let nested = "env BYPASS=1 cargo test";
    let nested_tokens = ["env", "BYPASS=1", "cargo", "test"].map(str::to_owned);
    let selected = CommandDecisionShard::select_for_command(&shard, nested, &nested_tokens)
        .expect("select nested command profile")
        .expect("testing decision");
    assert_eq!(selected.message, "testing");
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
