use std::path::PathBuf;

use super::runtime_profiles_path_for_activation;

#[test]
fn runtime_profiles_path_for_generated_activation_uses_state_cache_home() {
    let activation_path =
        PathBuf::from("/tmp/project/.cache/agent-semantic-protocol/hooks/activation.json");

    assert_eq!(
        runtime_profiles_path_for_activation(&activation_path),
        PathBuf::from("/tmp/project/.cache/agent-semantic-protocol/runtime/profiles.json")
    );
}

#[test]
fn runtime_profiles_path_for_manual_activation_uses_parent_local_cache() {
    let activation_path = PathBuf::from("/tmp/project/activation.json");

    assert_eq!(
        runtime_profiles_path_for_activation(&activation_path),
        PathBuf::from("/tmp/project/.cache/agent-semantic-protocol/runtime/profiles.json")
    );
}
