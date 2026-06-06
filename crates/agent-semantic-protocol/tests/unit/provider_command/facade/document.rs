use crate::provider_command::support::{
    asp_command, make_executable, prepend_path, temp_project_root,
};
use std::path::{Path, PathBuf};

#[test]
fn top_level_usage_lists_document_facades() {
    let root = temp_project_root("document-facade-top-level-help");

    let output = asp_command(&root)
        .arg("--help")
        .output()
        .expect("run top-level help");

    assert_eq!(output.status.code(), Some(2));
    let stderr = String::from_utf8(output.stderr).expect("stderr");
    assert!(
        stderr.contains("rust|typescript|python|julia|org|md"),
        "stderr={stderr}"
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn document_facade_help_does_not_spawn_orgize() {
    let root = temp_project_root("document-facade-help");

    for language in ["org", "md"] {
        let output = asp_command(&root)
            .env("PATH", "")
            .args([language, "--help"])
            .output()
            .expect("run document help");

        assert!(
            output.status.success(),
            "language={language} stderr={}",
            String::from_utf8_lossy(&output.stderr)
        );
        let stdout = String::from_utf8(output.stdout).expect("stdout");
        assert!(
            stdout.contains(&format!("usage: asp {language} <guide|search|query> ...")),
            "stdout={stdout}"
        );
    }
    let _ = std::fs::remove_dir_all(root);
}

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

fn write_org_elements_fixture(root: &Path) -> PathBuf {
    let path = root.join("plan.org");
    std::fs::write(
        &path,
        "* TODO [#A] Task :work:\nSCHEDULED: <2026-06-06 Sat>\n:PROPERTIES:\n:CUSTOM_ID: task-1\n:END:\n\nProvider activation carries execution mode.\nDocument providers stay embedded inside ASP.\n\n| Key | Value |\n| Foo | Bar |\n\n- [X] ship element map\n- plain list item\n[[https://example.com][site]]\n[[file:diagram.png]]\n\n#+begin_src rust\nfn main() {}\n#+end_src\n\n#+begin_export html\n<div>exported</div>\n#+end_export\n",
    )
    .expect("write org elements fixture");
    path
}

fn asp_org_query(root: &Path, args: &[&str]) -> String {
    let output = asp_command(root)
        .arg("org")
        .args(args)
        .output()
        .expect("run asp org query");
    assert!(
        output.status.success(),
        "args={args:?} stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).expect("stdout")
}
