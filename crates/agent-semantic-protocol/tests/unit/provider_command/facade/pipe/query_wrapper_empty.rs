use crate::provider_command::support::{asp_command, temp_project_root};

#[test]
fn asp_fd_query_empty_seeds_prints_compact_no_output_receipt() {
    let root = temp_project_root("asp-fd-query-empty-receipt");
    std::fs::create_dir_all(root.join("src")).expect("create src");
    std::fs::write(
        root.join("package.json"),
        r#"{"name":"query-wrapper-fixture"}"#,
    )
    .expect("write package json");
    std::fs::write(root.join("src/present.ts"), "export const Present = 1;\n")
        .expect("write source");

    let output = asp_command(&root)
        .args(["fd", "-query", "MissingOwner|MissingHelper", "."])
        .output()
        .expect("run asp fd -query with no candidates");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("stdout");
    assert!(stdout.starts_with("[search-fd]"), "{stdout}");
    assert!(stdout.contains("querySet=2"), "{stdout}");
    assert!(
        stdout.contains("query=MissingOwner|MissingHelper"),
        "{stdout}"
    );
    assert!(
        stdout.contains("noOutput reason=no-candidates sourceTrace=finder:empty"),
        "{stdout}"
    );
    assert!(
        stdout.contains("nextCommand=asp rg -query 'missingowner' -query 'missinghelper' '.'"),
        "{stdout}"
    );
    assert!(
        stdout.contains("avoid=repeat-flat-fd,workspace-wide-fd,raw-read"),
        "{stdout}"
    );
    assert!(!stdout.contains("queryPack="), "{stdout}");
    assert!(!stdout.contains("rankedEvidence="), "{stdout}");
    assert!(!stdout.contains("evidenceFrontier="), "{stdout}");
    assert!(!stdout.contains("ownerItems=-"), "{stdout}");
    assert!(!stdout.contains("actionFrontier="), "{stdout}");
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn asp_rg_query_empty_seeds_prints_compact_no_output_receipt() {
    let root = temp_project_root("asp-rg-query-empty-receipt");
    std::fs::create_dir_all(root.join("src")).expect("create src");
    std::fs::write(
        root.join("package.json"),
        r#"{"name":"query-wrapper-fixture"}"#,
    )
    .expect("write package json");
    std::fs::write(root.join("src/present.ts"), "export const Present = 1;\n")
        .expect("write source");

    let output = asp_command(&root)
        .args([
            "rg",
            "-query",
            "MissingOwner",
            "-query",
            "MissingHelper",
            ".",
        ])
        .output()
        .expect("run asp rg -query with no candidates");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("stdout");
    assert!(stdout.starts_with("[search-rg]"), "{stdout}");
    assert!(stdout.contains("querySet=2"), "{stdout}");
    assert!(
        stdout.contains("query=MissingOwner + MissingHelper"),
        "{stdout}"
    );
    assert!(
        stdout.contains("noOutput reason=no-candidates sourceTrace=finder:empty"),
        "{stdout}"
    );
    assert!(
        stdout.contains("nextCommand=asp fd -query 'missingowner|missinghelper' '.'"),
        "{stdout}"
    );
    assert!(
        stdout.contains("avoid=repeat-flat-rg,workspace-wide-rg,manual-window-scan,raw-read"),
        "{stdout}"
    );
    assert!(!stdout.contains("queryPack="), "{stdout}");
    assert!(!stdout.contains("rankedEvidence="), "{stdout}");
    assert!(!stdout.contains("evidenceFrontier="), "{stdout}");
    assert!(!stdout.contains("ownerItems=-"), "{stdout}");
    assert!(!stdout.contains("actionFrontier="), "{stdout}");
    let _ = std::fs::remove_dir_all(root);
}
