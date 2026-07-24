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
