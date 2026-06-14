use crate::provider_command::support::{
    asp_command, make_executable, prepend_path, temp_project_root,
};

#[test]
fn md_facade_uses_native_orgize_dependency() {
    let root = temp_project_root("md-document-facade");
    let bin_dir = root.join(".bin");
    std::fs::create_dir_all(&bin_dir).expect("create bin dir");
    let orgize = bin_dir.join("orgize");
    std::fs::write(&orgize, "#!/bin/sh\nexit 42\n").expect("write orgize");
    make_executable(&orgize);

    let output = asp_command(&root)
        .env("PATH", prepend_path(&bin_dir))
        .args(["md", "search", "prime", "--view", "seeds", "."])
        .output()
        .expect("run asp md search");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("stdout");
    assert!(stdout.contains("[search-prime] lang=md"), "stdout={stdout}");
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn md_facade_search_fzf_toc_returns_toc_for_keyword_matched_documents() {
    let root = temp_project_root("md-document-fzf-toc");
    std::fs::write(
        root.join("guide.md"),
        "# Guide\n\nTree facts live here.\n\n## Syntax\n\nSitter details live in this section.\n",
    )
    .expect("write guide markdown");
    std::fs::write(
        root.join("other.md"),
        "# Other\n\nThis document should not match both query terms.\n",
    )
    .expect("write other markdown");

    let output = asp_command(&root)
        .args([
            "md", "search", "fzf", "Tree", "Sitter", "--view", "toc", ".",
        ])
        .output()
        .expect("run asp md fzf toc");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("stdout");
    assert!(stdout.contains("[search-fzf-toc] lang=md"), "{stdout}");
    assert!(stdout.contains("q=Tree Sitter"), "{stdout}");
    assert!(
        stdout.contains("|doc path=\"./guide.md\" heading=2"),
        "{stdout}"
    );
    assert!(stdout.contains("level=1 title=\"Guide\""), "{stdout}");
    assert!(stdout.contains("level=2 title=\"Syntax\""), "{stdout}");
    assert!(!stdout.contains("./other.md"), "{stdout}");

    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn md_facade_content_projection_deduplicates_list_children() {
    let root = temp_project_root("md-document-content-deduplicates-list");
    std::fs::write(
        root.join("guide.md"),
        "# Guide\n\nWrapped content spans\nmultiple markdown lines.\n\n- [x] ship element map\n- plain list item\n\n- repeated item\n- repeated item\n",
    )
    .expect("write markdown list fixture");

    let paragraph_output = asp_command(&root)
        .args(["md", "query", "--term", "Wrapped", "--content"])
        .output()
        .expect("run asp md paragraph content query");
    assert!(
        paragraph_output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&paragraph_output.stderr)
    );
    let paragraph_content = String::from_utf8(paragraph_output.stdout).expect("stdout");
    assert_eq!(
        paragraph_content.trim(),
        "Wrapped content spans multiple markdown lines.",
        "{paragraph_content}"
    );

    let output = asp_command(&root)
        .args(["md", "query", "--selector", "guide.md:6-7", "--content"])
        .output()
        .expect("run asp md content query");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let content = String::from_utf8(output.stdout).expect("stdout");
    assert_eq!(content.matches("ship element map").count(), 1, "{content}");
    assert_eq!(content.matches("plain list item").count(), 1, "{content}");

    let repeated_output = asp_command(&root)
        .args(["md", "query", "--selector", "guide.md:9-10", "--content"])
        .output()
        .expect("run asp md repeated content query");
    assert!(
        repeated_output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&repeated_output.stderr)
    );
    let repeated = String::from_utf8(repeated_output.stdout).expect("stdout");
    assert_eq!(repeated.matches("repeated item").count(), 2, "{repeated}");

    let missing_output = asp_command(&root)
        .args([
            "md",
            "query",
            "--term",
            "__asp_missing_content_probe__",
            "--content",
        ])
        .output()
        .expect("run asp md missing content query");
    assert!(
        missing_output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&missing_output.stderr)
    );
    assert_eq!(missing_output.stdout, b"", "{missing_output:?}");

    let _ = std::fs::remove_dir_all(root);
}
