use crate::provider_command::support::{asp_command, temp_project_root};

use super::support::write_org_elements_fixture;

#[test]
fn org_facade_search_pipe_uses_document_element_chain() {
    let root = temp_project_root("org-document-search-pipe");
    write_org_elements_fixture(&root);

    let output = asp_command(&root)
        .args([
            "org",
            "search",
            "pipe",
            "execution mode",
            "--workspace",
            ".",
            "--view",
            "graph-turbo-request",
        ])
        .output()
        .expect("run asp org search pipe");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let packet: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("parse graph-turbo request");
    assert_document_pipe_packet(&packet, "org", "execution mode");

    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn md_facade_search_pipe_uses_document_element_chain() {
    let root = temp_project_root("md-document-search-pipe");
    std::fs::write(
        root.join("guide.md"),
        "# Guide\n\nRuntime activation keeps document facts available.\n\n## Hooks\n\nPipe search should return content selectors.\n",
    )
    .expect("write guide markdown");

    let output = asp_command(&root)
        .args([
            "md",
            "search",
            "pipe",
            "runtime activation",
            "--workspace",
            ".",
            "--view",
            "graph-turbo-request",
        ])
        .output()
        .expect("run asp md search pipe");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let packet: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("parse graph-turbo request");
    assert_document_pipe_packet(&packet, "md", "Runtime activation");

    let _ = std::fs::remove_dir_all(root);
}

fn assert_document_pipe_packet(packet: &serde_json::Value, language_id: &str, matched_text: &str) {
    assert_eq!(packet["surface"], "search-pipe");
    assert_eq!(
        packet["candidateSources"],
        serde_json::json!(["document-element"])
    );
    assert_eq!(packet["sourceTrace"][0]["source"], "document-element");
    assert_eq!(packet["sourceTrace"][0]["status"], "used");
    let nodes = packet["graph"]["nodes"].as_array().expect("graph nodes");
    assert!(
        nodes.iter().any(|node| {
            node["source"] == "document-element"
                && node["locator"]
                    .as_str()
                    .is_some_and(|locator| locator.starts_with(&format!("{language_id}://")))
                && node["sourceLocatorHint"]
                    .as_str()
                    .is_some_and(|locator| locator.contains(':'))
                && node["displayLineRange"].as_str().is_some()
                && node["matchText"]
                    .as_str()
                    .is_some_and(|text| text.contains(matched_text))
        }),
        "{packet:#}"
    );
    assert!(
        packet
            .get("actionFrontier")
            .is_none_or(serde_json::Value::is_null),
        "{packet:#}"
    );
    assert!(
        nodes.iter().any(|node| {
            node["kind"] == "provider-root"
                && node["fields"]["languageId"] == language_id
                && node["path"].as_str().is_some()
        }),
        "{packet:#}"
    );
    assert!(
        nodes
            .iter()
            .any(|node| { node["kind"] == "owner" && node["path"].as_str().is_some() }),
        "{packet:#}"
    );
}

#[test]
fn graph_turbo_schema_allows_document_pipe_candidate_sources() {
    let schema_path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../schemas/semantic-graph-turbo-request.v1.schema.json");
    let schema: serde_json::Value =
        serde_json::from_slice(&std::fs::read(schema_path).expect("read graph-turbo schema"))
            .expect("parse graph-turbo schema");

    assert_enum_contains(
        &schema["properties"]["candidateSources"]["items"]["enum"],
        "document-element",
    );
    assert_enum_contains(
        &schema["$defs"]["sourceTraceEntry"]["properties"]["source"]["enum"],
        "document-element",
    );
    assert_enum_contains(
        &schema["$defs"]["node"]["properties"]["source"]["enum"],
        "document-element",
    );
    assert_enum_contains(
        &schema["$defs"]["node"]["properties"]["confidence"]["enum"],
        "parser",
    );
}

fn assert_enum_contains(values: &serde_json::Value, expected: &str) {
    assert!(
        values
            .as_array()
            .expect("schema enum array")
            .iter()
            .any(|value| value.as_str() == Some(expected)),
        "missing `{expected}` in schema enum {values:#}"
    );
}
