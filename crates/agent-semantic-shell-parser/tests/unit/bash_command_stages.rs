use agent_semantic_shell_parser::{
    PrefixMatch, bash::parse_bash_command_candidates, command_stages_match_wrapped_prefix,
};

fn tokens(values: &[&str]) -> Vec<String> {
    values.iter().map(|value| (*value).to_owned()).collect()
}

fn assert_rg_is_routed(command: &str) {
    let stages = parse_bash_command_candidates(command).expect("valid Bash command");
    assert_eq!(
        command_stages_match_wrapped_prefix(&stages, &tokens(&["rg"])),
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

#[test]
fn generic_run_wrapper_expands_compound_shell_program_into_inner_stages() {
    let first = "crates/agent-semantic-content-identity/src/content_binding.rs";
    let second = "crates/agent-semantic-client-db/src/runtime_server_workspace/content_binding.rs";
    let command = format!(
        "/workspace/.devenv/devenv-profile-exec rtk run 'rg -n . {first} | sed -n \"238,300p\"; rg -n . {second} | head -n 90'"
    );
    let stages = parse_bash_command_candidates(&command).expect("valid wrapped Bash command");

    for expected in [
        tokens(&["rg", "-n", ".", first]),
        tokens(&["sed", "-n", "238,300p"]),
        tokens(&["rg", "-n", ".", second]),
        tokens(&["head", "-n", "90"]),
    ] {
        assert!(
            stages.iter().any(|stage| stage.words() == expected),
            "missing inner stage {expected:?}: stages={stages:?}"
        );
    }
}

#[test]
fn powershell_command_wrapper_exposes_reader_stage() {
    let stages = agent_semantic_shell_parser::parse_bash_command_candidates(
        "pwsh -NoProfile -Command 'Get-Content crates/example/src/lib.rs'",
    )
    .expect("parse PowerShell command wrapper");

    assert!(stages.iter().any(|stage| {
        stage.words()
            == [
                "Get-Content".to_owned(),
                "crates/example/src/lib.rs".to_owned(),
            ]
    }));
}

#[test]
fn quoted_data_with_pipeline_character_is_not_a_nested_stage_without_run() {
    let stages = parse_bash_command_candidates("rg 'alpha|beta' src/lib.rs")
        .expect("valid quoted pattern command");
    assert_eq!(stages.len(), 1, "quoted data is not a shell program");
    assert_eq!(
        stages[0].words(),
        tokens(&["rg", "alpha|beta", "src/lib.rs"])
    );
}
