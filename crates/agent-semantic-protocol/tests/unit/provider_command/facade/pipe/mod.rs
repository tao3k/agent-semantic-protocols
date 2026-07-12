mod config;
mod guide_gerbil;
mod ingest;
mod lexical;
mod lexical_fallback;
mod lexical_query_deps;
mod lexical_workspace;
mod options;
mod owner_items_gerbil;
mod owner_items_language_harness;
mod pipe_frontier;
mod provider_ontology;
mod query_wrapper;
mod query_wrapper_empty;
mod query_wrapper_gerbil;
mod reasoning;
mod source_index;
mod suggest;
mod surface;

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
    let seed_ids = payload["seedIds"].as_array().expect("seedIds array");
    assert!(
        !seed_ids.is_empty()
            && seed_ids
                .iter()
                .all(|seed_id| seed_id.as_str().is_some_and(|seed_id| !seed_id.is_empty())),
        "seedIds must be non-empty strings: {payload}"
    );
    assert_graph_turbo_seed_plan_contract(payload, seed_ids);

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
                    | "source"
                    | "confidence"
                    | "matchText"
                    | "syntaxQuery"
                    | "name"
                    | "startLine"
                    | "endLine"
                    | "start"
                    | "end"
                    | "languageId"
                    | "target"
                    | "itemName"
                    | "itemKind"
                    | "displayLineRange"
                    | "sourceLocatorHint"
                    | "structuralSelector"
                    | "projection"
                    | "codePolicy"
                    | "requiresExact"
                    | "candidateState"
                    | "rankEligible"
                    | "exactReadCommand"
                    | "readinessNextAction"
                    | "recommendedNextCommand"
                    | "matchedTerm"
                    | "matchLine"
                    | "boundarySource"
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

fn assert_graph_turbo_seed_plan_contract(payload: &Value, seed_ids: &[Value]) {
    let seed_plan = payload["seedPlan"].as_object().expect("seedPlan object");
    assert_eq!(payload["seedPlan"]["phase"], "seed-query");
    assert_eq!(payload["seedPlan"]["algorithm"], "asp-search-pipe-v1");
    assert!(
        payload["seedPlan"]["reason"]
            .as_str()
            .is_some_and(|reason| matches!(reason, "query" | "fallback-owner" | "empty")),
        "seedPlan.reason must explain seed selection: {seed_plan:?}"
    );
    for field in [
        "seedQuality",
        "queryPresent",
        "querySeedPresent",
        "candidateCount",
        "candidateOwnerCount",
        "queryOwnerSeedCount",
        "fallbackOwnerSeedCount",
        "selectedSeedCount",
        "seedIds",
        "riskFactors",
        "recommendedActions",
    ] {
        assert!(
            seed_plan.contains_key(field),
            "seedPlan missing required field {field}: {seed_plan:?}"
        );
    }
    assert_eq!(
        payload["seedPlan"]["selectedSeedCount"].as_u64(),
        Some(seed_ids.len() as u64),
        "seedPlan.selectedSeedCount must match seedIds: {payload}"
    );
    assert_eq!(
        payload["seedPlan"]["seedIds"]
            .as_array()
            .expect("seedPlan.seedIds array"),
        seed_ids,
        "seedPlan.seedIds must mirror request seedIds"
    );
    assert!(
        payload["seedPlan"]["seedQuality"]
            .as_str()
            .is_some_and(|quality| matches!(quality, "good" | "review" | "fail")),
        "seedPlan.seedQuality must be an analyzer status: {payload}"
    );
    assert!(
        payload["seedPlan"]["riskFactors"].as_array().is_some(),
        "seedPlan.riskFactors must be an array: {payload}"
    );
    assert!(
        payload["seedPlan"]["recommendedActions"]
            .as_array()
            .is_some_and(|actions| !actions.is_empty()),
        "seedPlan.recommendedActions must name at least one analyzer action: {payload}"
    );
    if payload.get("route").is_some() {
        assert_graph_turbo_route_contract(payload);
    }
}

fn assert_graph_turbo_route_contract(payload: &Value) {
    let route = payload["route"].as_object().expect("route object");
    assert_eq!(payload["route"]["kind"], "graph-route");
    assert_eq!(payload["route"]["version"], "lexical-route-v1");
    assert_eq!(payload["route"]["algorithm"], "graph-owner-rank-v1");
    assert!(
        payload["route"]["relation"]
            .as_str()
            .is_some_and(|relation| matches!(relation, "cohesive" | "query-bundle-required")),
        "route relation must be explicit: {route:?}"
    );
    let covered = payload["route"]["coveredQueryCount"]
        .as_u64()
        .expect("coveredQueryCount");
    let query_count = payload["route"]["queryCount"].as_u64().expect("queryCount");
    assert!(
        covered <= query_count,
        "route coverage must not exceed query count: {route:?}"
    );
    for pointer in [
        "/route/owner/path",
        "/route/owner/score/total",
        "/route/nextAction/kind",
        "/route/nextAction/languageId",
        "/route/nextAction/ownerPath",
        "/route/nextAction/query",
    ] {
        assert!(
            payload.pointer(pointer).is_some(),
            "route missing {pointer}: {route:?}"
        );
    }
}

fn assert_graph_turbo_request_matches_shared_schema(payload: &Value) {
    let schema = shared_graph_turbo_request_schema();
    let payload = payload.as_object().expect("request object");
    assert_required_fields(payload, &schema["required"]);
    assert_allowed_keys(payload.keys(), &property_keys(&schema["properties"]));

    let graph = payload["graph"].as_object().expect("graph object");
    let graph_schema = &schema["$defs"]["graph"];
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
