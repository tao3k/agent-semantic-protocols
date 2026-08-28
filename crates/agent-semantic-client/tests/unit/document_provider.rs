use crate::provider_command::support::{asp_command, temp_project_root};

#[test]
fn markdown_document_surface_bypasses_language_activation_and_external_provider_process() {
    let root = temp_project_root("md-document-surface");
    std::fs::create_dir(root.join(".git")).expect("create Git scope marker");
    std::fs::write(
        root.join("project.md"),
        "# Project\n\nDocument surface evidence.\n",
    )
    .expect("write Markdown document");

    let output = asp_command(&root)
        .args([
            "md",
            "query",
            "--term",
            "Project",
            "--workspace",
            ".",
            "--view",
            "metadata",
        ])
        .output()
        .expect("run asp md document query");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("query stdout");
    assert!(stdout.contains("md://"), "{stdout}");
    assert!(stdout.contains("structuralSelector="), "{stdout}");
    assert!(
        !root
            .join("home")
            .join(".agent-semantic-protocols")
            .join("activation.json")
            .exists(),
        "document query manufactured a language activation"
    );

    let _ = std::fs::remove_dir_all(root);
}
