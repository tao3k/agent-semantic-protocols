pub fn semantic_shell_tokens(command: &str) -> Vec<String> {
    agent_semantic_command_match::parse_bash_command_candidates(command)
        .unwrap_or_default()
        .into_iter()
        .flat_map(|stage| stage.words().to_vec())
        .collect()
}
