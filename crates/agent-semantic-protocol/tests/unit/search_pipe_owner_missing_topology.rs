use super::{OwnerMissingDiagnosticRequest, render_owner_missing_diagnostic};

#[test]
fn owner_missing_diagnostic_preserves_language_generation_and_navigation_action() {
    let diagnostic = render_owner_missing_diagnostic(OwnerMissingDiagnosticRequest {
        language_id: "rust",
        workspace: ".",
        owner_path: "crates/agent-semantic-hook/src/policy.rs",
        generation_digest: "generation-42",
        root_digest: "root-42",
        project_resolutions: &[],
    });

    assert!(diagnostic.starts_with(
        "owner search state=owner-missing reasonKind=owner-not-in-workspace ownerPath=crates/agent-semantic-hook/src/policy.rs languageId=rust generation=generation-42 rootDigest=root-42\n"
    ));
    let packet = diagnostic
        .lines()
        .find_map(|line| line.strip_prefix("searchTopology="))
        .expect("typed topology line");
    let packet: serde_json::Value = serde_json::from_str(packet).expect("typed topology JSON");
    let schema: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../schemas/search-owner-missing-topology.v1.schema.json"
    ))
    .expect("owner-missing topology schema");
    jsonschema::validator_for(&schema)
        .expect("compile owner-missing topology schema")
        .validate(&packet)
        .expect("Rust owner-missing packet validates against shared schema");
    assert_eq!(
        packet["schemaId"],
        "agent.semantic-protocols.search-owner-missing-topology"
    );
    assert_eq!(packet["schemaVersion"], "1");
    assert_eq!(packet["languageId"], "rust");
    assert_eq!(packet["state"], "owner-missing");
    assert_eq!(packet["reasonKind"], "owner-not-in-workspace");
    assert_eq!(
        packet["actionFrontier"],
        serde_json::json!(["A1.lexical-owner-evidence"])
    );
    assert_eq!(
        packet["recommendedNext"],
        "asp rust search lexical --query crates/agent-semantic-hook/src/policy.rs --workspace . --view seeds"
    );
    assert!(
        packet["edges"]
            .as_array()
            .expect("topology edges")
            .iter()
            .any(|edge| edge["relation"] == "omits_owner")
    );
}
