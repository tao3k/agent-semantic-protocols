use agent_semantic_content_identity::provider_projection_relation::PROVIDER_RELATION_ITEM_ENDPOINT_KIND;
use agent_semantic_content_identity::provider_projection_relation::ProviderProjectedRelation;
use agent_semantic_content_identity::provider_projection_relation::ProviderProjectedRelationEndpoint;

fn relation() -> ProviderProjectedRelation {
    ProviderProjectedRelation {
        from: ProviderProjectedRelationEndpoint {
            kind: PROVIDER_RELATION_ITEM_ENDPOINT_KIND,
            id: "item:caller".into(),
        },
        kind: "calls".into(),
        to: ProviderProjectedRelationEndpoint {
            kind: PROVIDER_RELATION_ITEM_ENDPOINT_KIND,
            id: "item:callee".into(),
        },
    }
}

#[test]
fn provider_projected_relation_accepts_schema_v1_shape() {
    let relation = relation();
    relation.validate().expect("typed relation must validate");
    let encoded = serde_json::to_value(&relation).expect("relation must encode");
    assert_eq!(encoded["from"]["id"], "item:caller");
    assert_eq!(encoded["kind"], "calls");
    assert_eq!(encoded["to"]["id"], "item:callee");
}

#[test]
fn provider_projected_relation_rejects_empty_endpoint_or_kind() {
    for invalid in [
        ProviderProjectedRelation {
            kind: "".into(),
            ..relation()
        },
        ProviderProjectedRelation {
            to: ProviderProjectedRelationEndpoint {
                kind: PROVIDER_RELATION_ITEM_ENDPOINT_KIND,
                id: "".into(),
            },
            ..relation()
        },
    ] {
        assert!(invalid.validate().is_err());
    }
}

#[test]
fn provider_projected_relation_denies_unknown_fields() {
    let error = serde_json::from_value::<ProviderProjectedRelation>(serde_json::json!({
        "from": {"kind": "item", "id": "item:caller"},
        "kind": "calls",
        "to": {"kind": "item", "id": "item:callee"},
        "legacyTarget": "item:callee"
    }))
    .expect_err("legacy relation fields must fail closed");
    assert!(error.to_string().contains("unknown field"));
}
