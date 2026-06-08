mod config;
mod fzf;
mod fzf_query_deps;
mod fzf_workspace;
mod ingest;
mod pipe_frontier;
mod provider_ontology;
mod reasoning;
mod suggest;

use std::collections::BTreeSet;
use std::path::PathBuf;

use serde_json::Value;

fn assert_graph_turbo_request_contract(payload: &Value) {
    assert_graph_turbo_request_matches_shared_schema(payload);
    assert_eq!(
        payload["schemaId"],
        "agent.semantic-protocols.semantic-graph-turbo-request"
    );
    assert_eq!(payload["schemaVersion"], "1");
    assert_eq!(
        payload["protocolId"],
        "agent.semantic-protocols.semantic-language"
    );
    assert_eq!(payload["protocolVersion"], "1");
    assert_eq!(payload["packetKind"], "graph-turbo-request");
    assert_eq!(payload["algorithm"], "typed-ppr-diverse");
    assert!(
        payload["profile"]
            .as_str()
            .is_some_and(|profile| !profile.is_empty()),
        "profile must be a non-empty string: {payload}"
    );
    assert!(
        payload["budget"].as_u64().is_some_and(|budget| budget > 0),
        "budget must be a positive integer: {payload}"
    );
    assert!(
        payload["seedIds"]
            .as_array()
            .is_some_and(|seed_ids| !seed_ids.is_empty()
                && seed_ids
                    .iter()
                    .all(|seed_id| seed_id.as_str().is_some_and(|seed_id| !seed_id.is_empty()))),
        "seedIds must be non-empty strings: {payload}"
    );

    let graph = payload["graph"].as_object().expect("graph object");
    assert!(
        graph
            .keys()
            .all(|key| matches!(key.as_str(), "nodes" | "edges")),
        "graph contains schema-unknown keys: {graph:?}"
    );
    let nodes = graph
        .get("nodes")
        .and_then(Value::as_array)
        .expect("graph.nodes array");
    let edges = graph
        .get("edges")
        .and_then(Value::as_array)
        .expect("graph.edges array");
    assert!(!nodes.is_empty(), "graph.nodes should not be empty");

    for node in nodes {
        let node = node.as_object().expect("node object");
        assert!(
            node.keys().all(|key| matches!(
                key.as_str(),
                "id" | "kind"
                    | "role"
                    | "value"
                    | "action"
                    | "weight"
                    | "locator"
                    | "location"
                    | "path"
                    | "owner"
                    | "ownerPath"
                    | "symbol"
                    | "matchText"
                    | "syntaxQuery"
                    | "name"
                    | "startLine"
                    | "endLine"
                    | "start"
                    | "end"
                    | "fields"
            )),
            "node contains schema-unknown keys: {node:?}"
        );
        for field in ["id", "kind", "role", "value"] {
            assert!(
                node.get(field)
                    .and_then(Value::as_str)
                    .is_some_and(|value| !value.is_empty()),
                "node.{field} must be a non-empty string: {node:?}"
            );
        }
    }

    for edge in edges {
        let edge = edge.as_object().expect("edge object");
        assert!(
            edge.keys().all(|key| matches!(
                key.as_str(),
                "source" | "target" | "relation" | "weight" | "fields"
            )),
            "edge contains schema-unknown keys: {edge:?}"
        );
        for field in ["source", "target", "relation"] {
            assert!(
                edge.get(field)
                    .and_then(Value::as_str)
                    .is_some_and(|value| !value.is_empty()),
                "edge.{field} must be a non-empty string: {edge:?}"
            );
        }
    }
}

fn assert_graph_turbo_request_matches_shared_schema(payload: &Value) {
    let schema = shared_graph_turbo_request_schema();
    let payload = payload.as_object().expect("request object");
    assert_required_fields(payload, &schema["required"]);
    assert_allowed_keys(payload.keys(), &property_keys(&schema["properties"]));

    let graph = payload["graph"].as_object().expect("graph object");
    let graph_schema = &schema["properties"]["graph"];
    assert_required_fields(graph, &graph_schema["required"]);
    assert_allowed_keys(graph.keys(), &property_keys(&graph_schema["properties"]));

    let node_keys = property_keys(&schema["$defs"]["node"]["properties"]);
    for node in graph["nodes"].as_array().expect("graph.nodes") {
        assert_allowed_keys(node.as_object().expect("node object").keys(), &node_keys);
    }
    let edge_keys = property_keys(&schema["$defs"]["edge"]["properties"]);
    for edge in graph["edges"].as_array().expect("graph.edges") {
        assert_allowed_keys(edge.as_object().expect("edge object").keys(), &edge_keys);
    }
}

fn shared_graph_turbo_request_schema() -> Value {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../schemas/semantic-graph-turbo-request.v1.schema.json");
    serde_json::from_slice(&std::fs::read(path).expect("read graph turbo request schema"))
        .expect("parse graph turbo request schema")
}

fn assert_required_fields(map: &serde_json::Map<String, Value>, required: &Value) {
    for field in string_array(required) {
        assert!(
            map.contains_key(&field),
            "missing required schema field {field}"
        );
    }
}

fn assert_allowed_keys<'a>(keys: impl Iterator<Item = &'a String>, allowed: &BTreeSet<String>) {
    for key in keys {
        assert!(allowed.contains(key), "schema-unknown key {key}");
    }
}

fn property_keys(properties: &Value) -> BTreeSet<String> {
    properties
        .as_object()
        .expect("schema properties object")
        .keys()
        .cloned()
        .collect()
}

fn string_array(value: &Value) -> Vec<String> {
    value
        .as_array()
        .expect("schema string array")
        .iter()
        .map(|item| item.as_str().expect("schema string").to_string())
        .collect()
}
