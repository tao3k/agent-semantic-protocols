use std::path::Path;

#[test]
fn rust_project_harness_policy_is_atomic_to_the_hook_package() {
    let config = asp_rust_project_harness_policy::default_rust_harness_config();
    let report = asp_rust_project_harness_policy::assert_rust_project_harness_clean_with_config(
        Path::new(env!("CARGO_MANIFEST_DIR")),
        &config,
    );
    assert!(
        report
            .root_paths
            .iter()
            .all(|path| path.starts_with(env!("CARGO_MANIFEST_DIR"))),
        "downstream package gate escaped into workspace siblings"
    );
}
