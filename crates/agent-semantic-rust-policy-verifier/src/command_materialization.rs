use agent_semantic_rust_policy_types::MemberPolicyRegistry;

pub fn prepare_command(registry: &MemberPolicyRegistry, package_name: &str) -> String {
    registry
        .harness_execution
        .prepare_command
        .iter()
        .map(|token| {
            if token == "{cargo-package-name}" {
                package_name
            } else {
                token
            }
        })
        .collect::<Vec<_>>()
        .into_iter()
        .map(|argument| {
            if argument == "{package}" {
                package_name
            } else {
                argument
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}
