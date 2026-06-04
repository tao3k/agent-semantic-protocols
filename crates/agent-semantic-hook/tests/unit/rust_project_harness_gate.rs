use std::path::Path;

#[test]
fn rust_project_harness_policy_applies_to_agent_semantic_hook() {
    let config = rust_lang_project_harness::default_rust_harness_config();
    rust_lang_project_harness::assert_rust_project_harness_clean_with_config(
        Path::new(env!("CARGO_MANIFEST_DIR")),
        &config,
    );
}
