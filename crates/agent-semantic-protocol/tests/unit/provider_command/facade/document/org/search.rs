use crate::provider_command::support::{asp_command, temp_project_root};

use crate::provider_command::facade::document::support::write_org_elements_fixture;

#[test]
fn org_facade_search_toc_returns_heading_outline() {
    let root = temp_project_root("org-document-toc-search");
    write_org_elements_fixture(&root);

    let output = asp_command(&root)
        .args(["org", "search", "toc", "--workspace", "."])
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
        stdout.contains("next=\"asp org query --selector org://./plan.org#"),
        "{stdout}"
    );

    let json_output = asp_command(&root)
        .args(["org", "search", "toc", "--json", "--workspace", "."])
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
fn org_facade_search_toc_returns_toc_for_keyword_matched_documents() {
    let root = temp_project_root("org-document-toc-search");
    write_org_elements_fixture(&root);
    std::fs::write(
        root.join("other.org"),
        "* Other\nThis document should not match the activation keyword.\n",
    )
    .expect("write other org fixture");

    let output = asp_command(&root)
        .args(["org", "search", "toc", "Provider", "--workspace", "."])
        .output()
        .expect("run asp org toc");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("stdout");
    assert!(stdout.contains("[search-toc] lang=org"), "{stdout}");
    assert!(
        stdout.contains("|doc path=\"./plan.org\" heading=3"),
        "{stdout}"
    );
    assert!(
        stdout.contains("level=2 title=\"Repository Map\""),
        "{stdout}"
    );

    let tail_view_output = asp_command(&root)
        .args(["org", "search", "toc", "Provider", "--workspace", "."])
        .output()
        .expect("run asp org trailing toc");
    assert!(
        tail_view_output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&tail_view_output.stderr)
    );
    let tail_stdout = String::from_utf8(tail_view_output.stdout).expect("stdout");
    assert!(
        tail_stdout.contains("[search-toc] lang=org"),
        "{tail_stdout}"
    );

    let json_output = asp_command(&root)
        .args(["org", "search", "toc", "Provider", "--workspace"])
        .arg(&root)
        .arg("--json")
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
