use super::source_access;
use super::support::{
    args, temp_root, write_activation_with_languages, write_activation_with_package_roots,
};

#[test]
fn read_file_command_selects_provider_by_source_extension() {
    let root = temp_root();
    let activation = write_activation_with_languages(&root, &["rust", "typescript"]);
    let rust_decision = source_access::source_access_decision_from_args(&args(&[
        "read-file",
        "--activation",
        activation.to_str().expect("path"),
        "src/lib.rs",
    ]))
    .expect("decision")
    .expect("rust decision");
    let typescript_decision = source_access::source_access_decision_from_args(&args(&[
        "read-file",
        "--activation",
        activation.to_str().expect("path"),
        "src/index.ts",
    ]))
    .expect("decision")
    .expect("typescript decision");
    let rust_value = serde_json::to_value(&rust_decision).expect("json");
    let typescript_value = serde_json::to_value(&typescript_decision).expect("json");
    assert_eq!(rust_value["providerId"], "rs-harness");
    assert_eq!(rust_decision.routes[0].argv[1], "rust");
    assert_eq!(typescript_value["providerId"], "ts-harness");
    assert_eq!(typescript_decision.routes[0].argv[1], "typescript");
}

#[test]
fn read_file_command_rewrites_selector_to_package_root() {
    let root = temp_root();
    let activation =
        write_activation_with_package_roots(&root, "rust", &["packages/rust-crate", "."]);
    let decision = source_access::source_access_decision_from_args(&args(&[
        "read-file",
        "--activation",
        activation.to_str().expect("path"),
        "packages/rust-crate/src/lib.rs",
    ]))
    .expect("decision")
    .expect("source decision");
    assert_eq!(
        decision.routes[0].argv,
        args(&[
            "asp",
            "rust",
            "query",
            "--from-hook",
            "direct-source-read",
            "--selector",
            "src/lib.rs",
            "--workspace",
            "packages/rust-crate",
            "--code",
        ])
    );
}
