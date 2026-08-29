use agent_semantic_shell_parser::{
    MAX_COMMAND_CANDIDATES, PrefixMatch, bash::parse_bash_command_candidates,
    command_stages_match_process_environment_assignment, command_stages_match_wrapped_prefix,
};

#[test]
fn process_environment_assignment_is_parser_owned_and_stage_bounded() {
    let expected = vec!["BUILD_MODE=ci".to_owned()];
    for command in [
        "BUILD_MODE=ci cargo test",
        "TRACE=1 BUILD_MODE=ci cargo test",
        "env BUILD_MODE=ci cargo test",
        "/usr/bin/env BUILD_MODE=ci cargo test",
        "export BUILD_MODE=ci; exec cargo test",
    ] {
        let stages = parse_bash_command_candidates(command).expect("valid Bash command");
        assert!(
            command_stages_match_process_environment_assignment(&stages, &expected),
            "command={command:?} stages={stages:?}"
        );
    }
    for command in [
        "NOT_BUILD_MODE=ci cargo test",
        "printf warmup && BUILD_MODE=ci cargo test",
        "bash -lc 'BUILD_MODE=ci cargo test'",
        "export BUILD_MODE=ci; cargo test",
        "export BUILD_MODE=ci; printf warmup; exec cargo test",
    ] {
        let stages = parse_bash_command_candidates(command).expect("valid Bash command");
        assert!(
            !command_stages_match_process_environment_assignment(&stages, &expected),
            "{command}"
        );
    }
}

fn tokens(values: &[&str]) -> Vec<String> {
    values.iter().map(|value| (*value).to_owned()).collect()
}

fn cargo_test_prefix() -> Vec<String> {
    tokens(&["cargo", "test"])
}

fn cargo_check_prefix() -> Vec<String> {
    tokens(&["cargo", "check"])
}

#[test]
fn wrapper_scenarios_share_one_prefix_contract() {
    let prefix = cargo_test_prefix();
    for command in [
        "cargo test -p policy",
        "/Users/example/.cargo/bin/cargo test -p policy",
        "env RUST_BACKTRACE=1 cargo test",
        "direnv exec . cargo test",
        "rtk cargo test",
        "echo ready && /opt/rust/bin/cargo test",
    ] {
        let stages = parse_bash_command_candidates(command).expect("valid Bash command");
        assert_eq!(
            command_stages_match_wrapped_prefix(&stages, &prefix),
            PrefixMatch::Matched,
            "command={command:?} stages={stages:?}"
        );
    }
}

#[test]
fn cargo_check_prefix_is_independent_of_downstream_package_names() {
    let prefix = cargo_check_prefix();
    for command in [
        "cargo check -p downstream-alpha -p downstream-beta",
        "/opt/rust/bin/cargo check -p consumer-core",
        "direnv exec . cargo check --workspace",
        "rtk cargo check -p arbitrary-package",
    ] {
        let stages = parse_bash_command_candidates(command).expect("valid Bash command");
        assert_eq!(
            command_stages_match_wrapped_prefix(&stages, &prefix),
            PrefixMatch::Matched,
            "command={command:?} stages={stages:?}"
        );
    }
}

#[test]
fn quoted_text_and_partial_prefixes_do_not_match() {
    let prefix = cargo_test_prefix();
    for command in ["echo 'cargo test'", "cargo testing", "echo cargo"] {
        let stages = parse_bash_command_candidates(command).expect("valid Bash command");
        assert_eq!(
            command_stages_match_wrapped_prefix(&stages, &prefix),
            PrefixMatch::NotMatched,
            "command={command:?} stages={stages:?}"
        );
    }
}

#[test]
fn bounded_scan_routes_protected_on_exhaustion() {
    let command = (0..=MAX_COMMAND_CANDIDATES)
        .map(|index| format!("wrapper-{index} noop"))
        .chain(std::iter::once("cargo test".to_string()))
        .collect::<Vec<_>>()
        .join(" && ");
    let stages = parse_bash_command_candidates(&command).expect("valid Bash command");
    let result = command_stages_match_wrapped_prefix(&stages, &cargo_test_prefix());
    assert_eq!(result, PrefixMatch::BudgetExceeded);
    assert!(result.routes_protected());
}
