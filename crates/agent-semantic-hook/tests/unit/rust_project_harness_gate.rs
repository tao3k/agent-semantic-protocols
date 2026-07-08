use std::path::Path;

#[test]
fn rust_project_harness_policy_applies_to_agent_semantic_hook() {
    let config = asp_rust_project_harness_policy::default_rust_harness_config();
    asp_rust_project_harness_policy::assert_rust_project_harness_clean_with_config(
        Path::new(env!("CARGO_MANIFEST_DIR")),
        &config,
    );
}
