use crate::provider_command::support::{
    asp_command, make_executable, prepend_path, temp_project_root,
};

use super::support::{asp_org_query, write_org_elements_fixture};

#[test]
fn org_facade_uses_native_orgize_dependency() {
    let root = temp_project_root("org-document-facade");
    let bin_dir = root.join(".bin");
    std::fs::create_dir_all(&bin_dir).expect("create bin dir");
    let orgize = bin_dir.join("orgize");
    std::fs::write(&orgize, "#!/bin/sh\nexit 42\n").expect("write orgize");
    make_executable(&orgize);

    let output = asp_command(&root)
        .env("PATH", prepend_path(&bin_dir))
        .args(["org", "search", "prime", "--view", "seeds", "."])
        .output()
        .expect("run asp org search");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("stdout");
    assert!(
        stdout.contains("[search-prime] lang=org"),
        "stdout={stdout}"
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn org_facade_guide_explains_element_query_axes() {
    let root = temp_project_root("org-document-guide-query-axes");

    let output = asp_command(&root)
        .args(["org", "guide", "."])
        .output()
        .expect("run asp org guide");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("stdout");
    assert!(
        stdout.contains("|query-axis field matches=key-or-key=value value-match=contains"),
        "{stdout}"
    );
    assert!(
        stdout.contains("|field-map heading fields=level,title,todo,todoType,priority,tag"),
        "{stdout}"
    );
    assert!(
        stdout.contains("|field-map block fields=kind=source|export,lang,backend"),
        "{stdout}"
    );
    assert!(
        stdout.contains(
            "|recipe paragraph-content=asp org query --kind paragraph --term <term> --content ."
        ),
        "{stdout}"
    );

    let query_output = asp_command(&root)
        .args(["org", "query", "guide", "."])
        .output()
        .expect("run asp org query guide");
    assert!(
        query_output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&query_output.stderr)
    );
    let query_stdout = String::from_utf8(query_output.stdout).expect("stdout");
    assert!(
        query_stdout
            .contains("|combine all=--selector+--term+--kind+--field semantics=intersection"),
        "{query_stdout}"
    );
    assert!(
        query_stdout.contains(
            "|content-rule requires=--selector|--term|--kind|--field forbids=--from-hook"
        ),
        "{query_stdout}"
    );

    let _ = std::fs::remove_dir_all(root);
}

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
        ("task", "|task", "sourceKind=\"SyntaxListItem\""),
        ("listItem", "|listItem", "sourceKind=\"SyntaxListItem\""),
        ("link", "|link", "sourceKind=\"SyntaxLink\""),
        ("image", "|image", "sourceKind=\"SyntaxLink\""),
    ] {
        let stdout = asp_org_query(&root, &["query", "--kind", kind, "--view", "metadata", "."]);
        assert!(stdout.contains(row), "kind={kind} stdout={stdout}");
        assert!(stdout.contains(source_kind), "kind={kind} stdout={stdout}");
    }

    let property = asp_org_query(
        &root,
        &[
            "query",
            "--field",
            "key=CUSTOM_ID",
            "--view",
            "metadata",
            ".",
        ],
    );
    assert!(property.contains("|property"), "{property}");
    assert!(property.contains("value=\"task-1\""), "{property}");

    let source_block = asp_org_query(
        &root,
        &["query", "--field", "kind=source", "--view", "metadata", "."],
    );
    assert!(source_block.contains("|block"), "{source_block}");
    assert!(source_block.contains("lang=\"rust\""), "{source_block}");

    let export_block = asp_org_query(
        &root,
        &["query", "--field", "kind=export", "--view", "metadata", "."],
    );
    assert!(export_block.contains("|block"), "{export_block}");
    assert!(export_block.contains("backend=\"html\""), "{export_block}");

    let paragraph_content =
        asp_org_query(&root, &["query", "--term", "embedded", "--content", "."]);
    assert_eq!(
        paragraph_content.trim(),
        "Provider activation carries execution mode.\nDocument providers stay embedded inside ASP."
    );

    let selector = format!("{}:1-5", path.display());
    let selector_frontier = asp_org_query(
        &root,
        &["query", "--selector", &selector, "--view", "metadata"],
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

    let content = asp_org_query(&root, &["query", "--selector", &selector, "--content"]);

    assert_eq!(content.matches("ship element map").count(), 1, "{content}");
    assert_eq!(content.matches("plain list item").count(), 1, "{content}");
    assert!(content.contains("- [X] ship element map"), "{content}");
    assert!(content.contains("- plain list item"), "{content}");

    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn org_facade_search_toc_returns_heading_outline() {
    let root = temp_project_root("org-document-toc-search");
    write_org_elements_fixture(&root);

    let output = asp_command(&root)
        .args(["org", "search", "toc", "."])
        .output()
        .expect("run asp org toc");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("stdout");
    assert!(stdout.contains("[search-toc] lang=org"), "{stdout}");
    assert!(stdout.contains("heading=3"), "{stdout}");
    assert!(stdout.contains("maxLevel=3"), "{stdout}");
    assert!(
        stdout.contains("|doc path=\"./plan.org\" heading=3"),
        "{stdout}"
    );
    assert!(
        stdout.contains("level=1 title=\"Task\" todo=\"TODO\""),
        "{stdout}"
    );
    assert!(
        stdout.contains("level=2 title=\"Repository Map\""),
        "{stdout}"
    );
    assert!(stdout.contains("level=3 title=\"Docs\""), "{stdout}");
    assert!(
        stdout.contains("next=\"asp org query --selector ./plan.org:"),
        "{stdout}"
    );

    let json_output = asp_command(&root)
        .args(["org", "search", "toc", "--json", "."])
        .output()
        .expect("run asp org toc json");
    assert!(
        json_output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&json_output.stderr)
    );
    let packet: serde_json::Value =
        serde_json::from_slice(&json_output.stdout).expect("parse toc packet");
    assert_eq!(packet["method"], "search/toc");
    assert_eq!(packet["view"], "toc");
    assert!(
        packet["documentFacts"]
            .as_array()
            .expect("document facts")
            .iter()
            .all(|fact| fact["kind"] == "heading"),
        "{packet:#}"
    );

    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn org_facade_search_fzf_toc_returns_toc_for_keyword_matched_documents() {
    let root = temp_project_root("org-document-fzf-toc-search");
    write_org_elements_fixture(&root);
    std::fs::write(
        root.join("other.org"),
        "* Other\nThis document should not match the activation keyword.\n",
    )
    .expect("write other org fixture");

    let output = asp_command(&root)
        .args(["org", "search", "fzf", "Provider", "--view", "toc", "."])
        .output()
        .expect("run asp org fzf toc");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("stdout");
    assert!(stdout.contains("[search-fzf-toc] lang=org"), "{stdout}");
    assert!(stdout.contains("q=Provider"), "{stdout}");
    assert!(
        stdout.contains("|doc path=\"./plan.org\" heading=3"),
        "{stdout}"
    );
    assert!(
        stdout.contains("level=2 title=\"Repository Map\""),
        "{stdout}"
    );
    assert!(!stdout.contains("./other.org"), "{stdout}");

    let tail_view_output = asp_command(&root)
        .args(["org", "search", "fzf", "Provider", "toc", "."])
        .output()
        .expect("run asp org fzf trailing toc");
    assert!(
        tail_view_output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&tail_view_output.stderr)
    );
    let tail_stdout = String::from_utf8(tail_view_output.stdout).expect("stdout");
    assert!(
        tail_stdout.contains("[search-fzf-toc] lang=org"),
        "{tail_stdout}"
    );

    let json_output = asp_command(&root)
        .args([
            "org", "search", "fzf", "Provider", "--view", "toc", "--json", ".",
        ])
        .output()
        .expect("run asp org fzf toc json");
    assert!(
        json_output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&json_output.stderr)
    );
    let packet: serde_json::Value =
        serde_json::from_slice(&json_output.stdout).expect("parse fzf toc packet");
    assert_eq!(packet["method"], "search/fzf-toc");
    assert_eq!(packet["view"], "fzf-toc");
    assert_eq!(packet["query"], "Provider");
    assert!(
        packet["documentFacts"]
            .as_array()
            .expect("document facts")
            .iter()
            .all(|fact| fact["kind"] == "heading"),
        "{packet:#}"
    );

    let _ = std::fs::remove_dir_all(root);
}
