use std::collections::BTreeSet;

use agent_semantic_hook::builtin_provider_manifests;
use serde_json::Value;

fn schema_enum<'a>(schema: &'a Value, pointer: &str) -> BTreeSet<&'a str> {
    schema
        .pointer(pointer)
        .and_then(Value::as_array)
        .expect("descriptor schema enum")
        .iter()
        .map(|value| value.as_str().expect("string enum member"))
        .collect()
}

#[test]
fn builtin_semantic_facts_descriptors_follow_shared_schema_contract() {
    let schema: Value = serde_json::from_str(include_str!(
        "../../../../../schemas/provider-semantic-facts-descriptor.v1.schema.json"
    ))
    .expect("valid semantic facts descriptor schema");
    let descriptor_version = schema
        .pointer("/properties/descriptorVersion/const")
        .and_then(Value::as_str)
        .expect("descriptor version contract");
    let fact_kinds = schema_enum(&schema, "/properties/factKinds/items/enum");
    let axes = schema_enum(&schema, "/$defs/intentAxis/properties/axis/enum");
    let roles = schema_enum(&schema, "/$defs/intentAxis/properties/roles/items/enum");

    let manifests = builtin_provider_manifests();
    let gerbil = manifests
        .iter()
        .find(|manifest| manifest.language_id == "gerbil-scheme")
        .expect("gerbil-scheme provider manifest");
    assert!(!gerbil.search_capabilities.semantic_facts);
    assert!(gerbil.semantic_facts_descriptor.is_none());
    let descriptors = manifests
        .iter()
        .filter_map(|manifest| {
            manifest
                .semantic_facts_descriptor
                .as_ref()
                .map(|descriptor| (manifest, descriptor))
        })
        .collect::<Vec<_>>();
    assert_eq!(
        descriptors
            .iter()
            .map(|(_, descriptor)| descriptor.descriptor_id.as_str())
            .collect::<BTreeSet<_>>(),
        BTreeSet::from([
            "julia.semantic-facts",
            "python.semantic-facts",
            "rust.semantic-facts",
            "typescript.semantic-facts",
        ])
    );

    for (manifest, descriptor) in descriptors {
        assert!(
            manifest.search_capabilities.semantic_facts,
            "{} has a semantic facts descriptor while semanticFacts is disabled",
            descriptor.descriptor_id
        );
        assert_eq!(descriptor.descriptor_version, descriptor_version);
        assert!(!descriptor.packet_schema_ids.is_empty());
        assert!(
            descriptor
                .fact_kinds
                .iter()
                .all(|fact_kind| fact_kinds.contains(fact_kind.as_str()))
        );
        assert!(descriptor.intent_axes.iter().all(|intent_axis| {
            axes.contains(intent_axis.axis.as_str())
                && !intent_axis.terms.is_empty()
                && intent_axis
                    .roles
                    .iter()
                    .all(|role| roles.contains(role.as_str()))
        }));
    }
}
