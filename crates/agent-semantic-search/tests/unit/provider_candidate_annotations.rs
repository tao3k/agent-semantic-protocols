use serde_json::json;

use crate::{
    compact_provider_fact_nodes, compact_provider_fact_value, provider_candidate_annotation_nodes,
    provider_facts_envelope_from_stdout, provider_facts_envelope_from_value,
};

#[test]
fn provider_facts_envelope_parses_nodes_edges_and_candidate_annotations() {
    let envelope = provider_facts_envelope_from_value(json!({
        "nodes": [{"id": "field:src/model.py-items"}],
        "edges": [{"source": "field:src/model.py-items", "target": "owner:src/model.py"}],
        "candidateAnnotations": [{
            "path": "src/generated/lib.rs",
            "attributes": ["generated"]
        }]
    }));

    assert_eq!(envelope.nodes.len(), 1);
    assert_eq!(envelope.edges.len(), 1);
    assert_eq!(envelope.candidate_annotations.len(), 1);
    assert_eq!(
        envelope.candidate_annotations[0]["path"],
        "src/generated/lib.rs"
    );
}

#[test]
fn provider_facts_envelope_extracts_json_from_provider_stdout() {
    let stdout = br#"[agent-semantic-client] syncing generated activation
{"nodes":[{"id":"field:src/model.py-items"}],"edges":[],"candidateAnnotations":[{"path":"src/generated/lib.rs","attributes":["generated"]}]}
"#;

    let envelope = provider_facts_envelope_from_stdout(stdout).expect("provider facts envelope");

    assert_eq!(envelope.nodes.len(), 1);
    assert_eq!(envelope.candidate_annotations.len(), 1);
}

#[test]
fn provider_fact_compaction_is_search_owned() {
    let compacted = compact_provider_fact_nodes(&[json!({
        "id": "field:src/model.py-items",
        "kind": "field",
        "role": "class-field",
        "value": "items: list[str]",
        "matchText": "Bag.items: list[str]\nextra detail"
    })]);

    assert_eq!(compact_provider_fact_value("items: list[str]"), "items");
    assert_eq!(compacted[0]["value"], "items");
    assert_eq!(compacted[0]["matchText"], "Bag.items");
}

#[test]
fn provider_candidate_annotations_project_to_graph_nodes_without_path_heuristics() {
    let annotations = vec![json!({
        "path": "src/generated/lib.rs",
        "attributes": ["generated", "schema-generated"],
        "source": "rust-harness",
        "reason": "provider-parser-fact"
    })];

    let nodes = provider_candidate_annotation_nodes(&annotations);

    assert_eq!(nodes.len(), 1);
    assert_eq!(nodes[0]["kind"], "provider-candidate-annotation");
    assert_eq!(nodes[0]["role"], "file-attributes");
    assert_eq!(nodes[0]["path"], "src/generated/lib.rs");
    assert_eq!(nodes[0]["fields"]["attributes"][0], "generated");
    assert_eq!(nodes[0]["fields"]["attributes"][1], "schema-generated");
}

#[test]
fn provider_candidate_annotations_ignore_incomplete_entries() {
    let annotations = vec![
        json!({"path": "", "attributes": ["generated"]}),
        json!({"path": "src/lib.rs", "attributes": []}),
        json!({"attributes": ["generated"]}),
    ];

    let nodes = provider_candidate_annotation_nodes(&annotations);

    assert!(nodes.is_empty());
}
