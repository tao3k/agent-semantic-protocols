use agent_semantic_command_match::{
    PrefixMatch, bash::parse_bash_command_candidates, command_stages_match_prefix,
};

fn tokens(values: &[&str]) -> Vec<String> {
    values.iter().map(|value| (*value).to_owned()).collect()
}

#[test]
fn typed_stage_matches_bare_and_wrapped_rg_without_wildcards() {
    let prefix = tokens(&["rg"]);
    for command in ["rg needle src/lib.rs", "env TRACE=1 rg needle src/lib.rs"] {
        let stages = parse_bash_command_candidates(command).expect("valid Bash command");
        assert_eq!(
            command_stages_match_prefix(&stages, &prefix),
            PrefixMatch::Matched
        );
    }
}
