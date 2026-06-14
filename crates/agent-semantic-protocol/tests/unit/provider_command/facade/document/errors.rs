use crate::provider_command::support::{asp_command, temp_project_root};

#[test]
fn document_facade_rejects_non_document_commands() {
    let root = temp_project_root("document-facade-rejects-check");

    let output = asp_command(&root)
        .args(["org", "check", "."])
        .output()
        .expect("run asp org check");

    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).expect("stderr");
    assert!(
        stderr.contains("unsupported document command `check`"),
        "stderr={stderr}"
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn document_query_rejects_code_projection_with_content_guidance() {
    let root = temp_project_root("document-query-rejects-code-projection");

    let output = asp_command(&root)
        .args(["md", "query", "--selector", "README.md:1-4", "--code"])
        .output()
        .expect("run asp md query --code");

    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).expect("stderr");
    assert!(
        stderr.contains("document providers use --content for query projection"),
        "stderr={stderr}"
    );
    assert!(
        stderr.contains("--code is reserved for source-language providers"),
        "stderr={stderr}"
    );
    let _ = std::fs::remove_dir_all(root);
}
