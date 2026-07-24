use agent_semantic_command_match::{
    PrefixMatch, bash::parse_bash_command_candidates, command_stages_match_prefix,
};

fn tokens(values: &[&str]) -> Vec<String> {
    values.iter().map(|value| (*value).to_owned()).collect()
}

fn assert_rg_is_routed(command: &str) {
    let stages = parse_bash_command_candidates(command).expect("valid Bash command");
    assert_eq!(
        command_stages_match_prefix(&stages, &tokens(&["rg"])),
        PrefixMatch::Matched,
        "command={command} stages={stages:?}"
    );
}

#[test]
fn bare_and_wrapped_rg_share_the_same_ast_matcher() {
    for command in [
        "rg needle src/lib.rs",
        "/opt/bin/rg needle src/lib.rs",
        "env TRACE=1 rg needle src/lib.rs",
        "direnv exec . rg needle src/lib.rs",
        "echo ready && rg needle src/lib.rs",
        "printf ready | rg needle src/lib.rs",
    ] {
        assert_rg_is_routed(command);
    }
}

#[test]
fn quoted_source_path_remains_one_ast_word() {
    let stages =
        parse_bash_command_candidates("rg needle 'src/file name.rs'").expect("valid Bash command");
    assert_eq!(stages.len(), 1);
    assert_eq!(
        stages[0].words(),
        tokens(&["rg", "needle", "src/file name.rs"])
    );
}
