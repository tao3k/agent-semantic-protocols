use agent_semantic_command_match::{command_stage_matches_prefix, PrefixMatch, MAX_PREFIX_WINDOWS};

fn tokens(values: &[&str]) -> Vec<String> {
    values.iter().map(|value| (*value).to_owned()).collect()
}

fn cargo_test_prefix() -> Vec<String> {
    tokens(&["cargo", "test"])
}

#[test]
fn wrapper_scenarios_share_one_prefix_contract() {
    let prefix = cargo_test_prefix();
    for command in [
        tokens(&["cargo", "test", "-p", "policy"]),
        tokens(&["/Users/example/.cargo/bin/cargo", "test", "-p", "policy"]),
        tokens(&["env", "RUST_BACKTRACE=1", "cargo", "test"]),
        tokens(&["direnv", "exec", ".", "cargo", "test"]),
        tokens(&["rtk", "cargo", "test"]),
        tokens(&["rtk", "test", "/opt/rust/bin/cargo", "test"]),
        tokens(&["echo", "ready", "&&", "/opt/rust/bin/cargo", "test"]),
    ] {
        assert_eq!(
            command_stage_matches_prefix(&command, &prefix),
            PrefixMatch::Matched,
            "command={command:?}"
        );
    }
}

#[test]
fn quoted_text_and_partial_prefixes_do_not_match() {
    let prefix = cargo_test_prefix();
    for command in [
        tokens(&["echo", "cargo test"]),
        tokens(&["cargo", "testing"]),
        tokens(&["echo", "cargo"]),
    ] {
        assert_eq!(
            command_stage_matches_prefix(&command, &prefix),
            PrefixMatch::NotMatched,
            "command={command:?}"
        );
    }
}

#[test]
fn bounded_scan_routes_protected_on_exhaustion() {
    let command = (0..=MAX_PREFIX_WINDOWS + 1)
        .map(|index| format!("wrapper-{index}"))
        .collect::<Vec<_>>();
    let result = command_stage_matches_prefix(&command, &cargo_test_prefix());
    assert_eq!(result, PrefixMatch::BudgetExceeded);
    assert!(result.routes_protected());
}
