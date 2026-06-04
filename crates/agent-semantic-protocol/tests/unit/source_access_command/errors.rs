use super::source_access;
use super::support::{args, temp_root, write_activation};

#[test]
fn source_access_command_requires_explicit_activation() {
    let error =
        source_access::source_access_decision_from_args(&args(&["read-file", "src/lib.rs"]))
            .expect_err("missing activation should fail");
    assert!(error.contains("requires --activation"));
}

#[test]
fn source_access_command_rejects_unknown_flags_and_extra_paths() {
    let root = temp_root();
    let activation = write_activation(&root, "rust");
    let unknown = source_access::source_access_decision_from_args(&args(&[
        "read-file",
        "--activation",
        activation.to_str().expect("path"),
        "--mcp-resource",
        "src/lib.rs",
    ]))
    .expect_err("unknown flag should fail");
    let extra_paths = source_access::source_access_decision_from_args(&args(&[
        "read-file",
        "--activation",
        activation.to_str().expect("path"),
        "src/lib.rs",
        "src/main.rs",
    ]))
    .expect_err("extra paths should fail");
    assert!(unknown.contains("unknown source-access flag"));
    assert!(extra_paths.contains("accepts exactly one path"));
}

#[test]
fn shell_egress_command_requires_command_and_output_digest() {
    let root = temp_root();
    let activation = write_activation(&root, "rust");
    let missing_command = source_access::source_access_decision_from_args(&args(&[
        "shell-egress",
        "--activation",
        activation.to_str().expect("path"),
        "--output-digest",
        "sha256:source-like-output",
        "src/lib.rs",
    ]))
    .expect_err("missing command should fail");
    let missing_digest = source_access::source_access_decision_from_args(&args(&[
        "shell-egress",
        "--activation",
        activation.to_str().expect("path"),
        "--command",
        "sed -n '1,120p' src/lib.rs",
        "src/lib.rs",
    ]))
    .expect_err("missing output digest should fail");
    assert!(missing_command.contains("requires --command"));
    assert!(missing_digest.contains("requires --output-digest"));
}
