use crate::provider_command::support::{asp_command, temp_project_root};

use crate::provider_command::facade::document::support::{
    asp_org_query, write_org_elements_fixture,
};

#[test]
fn org_facade_query_covers_org_element_kinds() {
    let root = temp_project_root("org-document-elements-query");
    let path = write_org_elements_fixture(&root);

    for (kind, row, source_kind) in [
        ("heading", "|heading", "sourceKind=\"Headline\""),
        ("property", "|property", "sourceKind=\"PropertyDrawer\""),
        ("planning", "|planning", "sourceKind=\"SyntaxPlanning\""),
        ("table", "|table", "sourceKind=\"OrgTable\""),
        ("paragraph", "|paragraph", "sourceKind=\"Paragraph\""),
        ("block", "|block", "sourceKind=\"SourceBlock\""),
        ("list", "|list", "sourceKind=\"SyntaxList\""),
        ("task", "|task", "sourceKind=\"Headline\""),
        ("listItem", "|listItem", "sourceKind=\"SyntaxListItem\""),
        ("link", "|link", "sourceKind=\"SyntaxLink\""),
        ("image", "|image", "sourceKind=\"SyntaxLink\""),
    ] {
        let stdout = asp_org_query(
            &root,
            &[
                "query",
                "--kind",
                kind,
                "--workspace",
                ".",
                "--view",
                "metadata",
            ],
        );
        assert!(stdout.contains(row), "kind={kind} stdout={stdout}");
        assert!(stdout.contains(source_kind), "kind={kind} stdout={stdout}");
    }

    let property = asp_org_query(
        &root,
        &[
            "query",
            "--field",
            "key=CUSTOM_ID",
            "--workspace",
            ".",
            "--view",
            "metadata",
        ],
    );
    assert!(property.contains("|property"), "{property}");
    assert!(property.contains("value=\"task-1\""), "{property}");

    let source_block = asp_org_query(
        &root,
        &[
            "query",
            "--field",
            "kind=source",
            "--workspace",
            ".",
            "--view",
            "metadata",
        ],
    );
    assert!(source_block.contains("|block"), "{source_block}");
    assert!(source_block.contains("lang=\"rust\""), "{source_block}");

    let export_block = asp_org_query(
        &root,
        &[
            "query",
            "--field",
            "kind=export",
            "--workspace",
            ".",
            "--view",
            "metadata",
        ],
    );
    assert!(export_block.contains("|block"), "{export_block}");
    assert!(export_block.contains("backend=\"html\""), "{export_block}");

    let paragraph_content = asp_org_query(
        &root,
        &[
            "query",
            "--term",
            "embedded",
            "--workspace",
            ".",
            "--content",
        ],
    );
    assert_eq!(
        paragraph_content.trim(),
        "Provider activation carries execution mode. Document providers stay embedded inside ASP."
    );

    let selector = format!("{}:1-5", path.display());
    let selector_frontier = asp_org_query(
        &root,
        &[
            "query",
            "--selector",
            &selector,
            "--workspace",
            ".",
            "--view",
            "metadata",
        ],
    );
    assert!(selector_frontier.contains("[query-selector] lang=org"));
    assert!(
        selector_frontier.contains("|heading"),
        "{selector_frontier}"
    );
    assert!(
        selector_frontier.contains("|property"),
        "{selector_frontier}"
    );

    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn org_facade_content_projection_deduplicates_list_children() {
    let root = temp_project_root("org-document-content-deduplicates-list");
    let path = write_org_elements_fixture(&root);
    let selector = format!("{}:15-16", path.display());

    let content = asp_org_query(
        &root,
        &[
            "query",
            "--selector",
            &selector,
            "--workspace",
            ".",
            "--content",
        ],
    );

    assert_eq!(content.matches("ship element map").count(), 1, "{content}");
    assert_eq!(content.matches("plain list item").count(), 1, "{content}");
    assert!(content.contains("- [X] ship element map"), "{content}");
    assert!(content.contains("- plain list item"), "{content}");

    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn org_facade_direct_read_accepts_content_projection_for_hook_recovery() {
    let root = temp_project_root("org-document-direct-read-content");
    std::fs::write(
        root.join("plan.org"),
        "* Guide\n\nHook recovery keeps raw Org source.\n",
    )
    .expect("write org fixture");

    let output = asp_command(&root)
        .args([
            "org",
            "query",
            "--from-hook",
            "direct-source-read",
            "--selector",
            "plan.org:1-3",
            "--workspace",
            ".",
            "--content",
        ])
        .output()
        .expect("run asp org direct-read content query");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let content = String::from_utf8(output.stdout).expect("stdout");
    assert_eq!(content, "* Guide\n\nHook recovery keeps raw Org source.\n");

    let _ = std::fs::remove_dir_all(root);
}
