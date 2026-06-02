use semantic_agent_hook::{PROFILE_REGISTRY_SCHEMA_ID, merge_profile_registries, parse_profiles};
use serde_json::json;

use super::registry_value;

#[test]
fn profile_registry_protocol_identity_is_validated() {
    let mut value = registry_value();
    value["schemaId"] = json!("agent.semantic-protocols.wrong-profile-registry");

    let error = parse_profiles(&value.to_string()).unwrap_err();

    assert!(format!("{error:?}").contains("schemaId"));
}

#[test]
fn profile_registry_requires_command_text() {
    let mut value = registry_value();
    value["profiles"][0]["commands"]["prime"]
        .as_object_mut()
        .unwrap()
        .remove("text");

    let error = parse_profiles(&value.to_string()).unwrap_err();

    assert!(format!("{error:?}").contains("missing field `text`"));
}

#[test]
fn profile_registry_rejects_legacy_text_route() {
    let mut value = registry_value();
    value["profiles"][0]["commands"]["text"] = json!({
        "text": "ts-harness search text {query} owner tests --view seeds .",
        "argv": ["ts-harness", "search", "text", "{query}", "owner", "tests", "--view", "seeds", "."]
    });

    let error = parse_profiles(&value.to_string()).unwrap_err();

    assert!(format!("{error:?}").contains("unknown field `text`"));
}

#[test]
fn profile_registry_rejects_null_stdin_mode() {
    let mut value = registry_value();
    value["profiles"][0]["commands"]["prime"]["stdinMode"] = json!(null);

    let error = parse_profiles(&value.to_string()).unwrap_err();

    assert!(format!("{error:?}").contains("stdinMode must be omitted"));
}

#[test]
fn profile_registry_merge_replaces_same_provider_profile() {
    let mut replacement = registry_value();
    replacement["profiles"][0]["sourceRoots"] = json!(["src", "packages"]);

    let merged = merge_profile_registries(vec![
        parse_profiles(&registry_value().to_string()).unwrap(),
        parse_profiles(&replacement.to_string()).unwrap(),
    ]);

    assert_eq!(merged.schema_id, PROFILE_REGISTRY_SCHEMA_ID);
    assert_eq!(merged.profiles.len(), 1);
    assert_eq!(
        merged.profiles[0].source_roots,
        ["src".to_string(), "packages".to_string()]
    );
}
