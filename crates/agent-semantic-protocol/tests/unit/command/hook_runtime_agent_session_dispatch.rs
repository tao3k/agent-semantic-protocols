use std::collections::BTreeMap;

use serde_json::Value;

#[test]
fn choice_plane_projection_removes_legacy_lifecycle_tokens() {
    let mut fields = BTreeMap::from([
        (
            "completionReceipt".to_owned(),
            Value::String("legacy".to_owned()),
        ),
        (
            "requiredAction".to_owned(),
            Value::String("legacy".to_owned()),
        ),
        (
            "targetAgentName".to_owned(),
            Value::String("@asp_testing".to_owned()),
        ),
        (
            "targetAgentRole".to_owned(),
            Value::String("asp_testing".to_owned()),
        ),
        (
            "targetAgentSelectionSource".to_owned(),
            Value::String("command-tags".to_owned()),
        ),
    ]);

    super::materialize_org_choice_plane_fields(&mut fields);

    assert!(!fields.contains_key("completionReceipt"));
    assert!(!fields.contains_key("requiredAction"));
    assert_eq!(
        fields.get("targetAgentName").and_then(Value::as_str),
        Some("@asp_testing")
    );
    assert_eq!(
        fields.get("targetAgentRole").and_then(Value::as_str),
        Some("asp_testing")
    );
    assert_eq!(
        fields
            .get("targetAgentSelectionSource")
            .and_then(Value::as_str),
        Some("command-tags")
    );
    assert_eq!(
        fields.get("agentWindowCommand").and_then(Value::as_str),
        Some("asp session --agents choice-plane")
    );
    assert!(
        fields
            .get("nextAction")
            .and_then(Value::as_str)
            .is_some_and(|instruction| instruction.contains("Open the Agent ChoicePlane"))
    );
}
