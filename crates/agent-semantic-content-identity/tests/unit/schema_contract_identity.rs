use serde_json::json;

use crate::schema_contract_identities;

#[test]
fn contract_identity_prefers_explicit_schema_version() {
    let identities = schema_contract_identities(
        "contract.schema.json",
        &json!({
            "properties": {
                "schemaId": { "const": "agent.example.contract" },
                "schemaVersion": { "const": "7" }
            }
        }),
    )
    .expect("identity");
    assert_eq!(identities[0].schema_id, "agent.example.contract");
    assert_eq!(identities[0].schema_version, "7");
}

#[test]
fn contract_identity_accepts_typed_version_suffixes() {
    let from_id = schema_contract_identities(
        "contract.schema.json",
        &json!({"properties": {"schemaId": {"const": "agent.example.contract.v2"}}}),
    )
    .expect("id suffix");
    assert_eq!(from_id[0].schema_version, "2");

    let from_name = schema_contract_identities(
        "contract.v3.schema.json",
        &json!({"properties": {"schemaId": {"const": "agent.example.contract"}}}),
    )
    .expect("name suffix");
    assert_eq!(from_name[0].schema_version, "3");
}

#[test]
fn contract_identity_projects_every_public_contract_in_one_schema() {
    let identities = schema_contract_identities(
        "evidence.v1.schema.json",
        &json!({
            "$defs": {
                "first": {"properties": {"schemaId": {"const": "agent.first.v1"}}},
                "second": {"properties": {"schemaId": {"const": "agent.second.v1"}}}
            }
        }),
    )
    .expect("identities");
    assert_eq!(identities.len(), 2);
}

#[test]
fn contract_identity_rejects_unversioned_public_contracts() {
    let error = schema_contract_identities(
        "contract.schema.json",
        &json!({"properties": {"schemaId": {"const": "agent.example.contract"}}}),
    )
    .expect_err("unversioned contract must fail");
    assert!(error.contains("no explicit or .v<digits> version"));
}
