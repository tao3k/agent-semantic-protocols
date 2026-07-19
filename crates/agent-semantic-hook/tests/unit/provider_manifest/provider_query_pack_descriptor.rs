use std::collections::BTreeSet;

use agent_semantic_hook::builtin_provider_manifests;
use serde_json::Value;

#[test]
fn every_builtin_language_owns_a_query_pack_descriptor() {
    let schema: Value = serde_json::from_str(include_str!(
        "../../../../../schemas/provider-query-pack-descriptor.v1.schema.json"
    ))
    .expect("valid provider query pack descriptor schema");
    let descriptor_version = schema
        .pointer("/properties/descriptorVersion/const")
        .and_then(Value::as_str)
        .expect("query pack descriptor version contract");
    let manifests = builtin_provider_manifests();

    assert_eq!(
        manifests
            .iter()
            .map(|manifest| manifest.language_id.as_str())
            .collect::<BTreeSet<_>>(),
        BTreeSet::from(["gerbil-scheme", "julia", "python", "rust", "typescript"])
    );

    let mut descriptor_ids = BTreeSet::new();
    for manifest in manifests {
        let descriptor = manifest
            .query_pack_descriptor
            .as_ref()
            .unwrap_or_else(|| panic!("{} must own a queryPackDescriptor", manifest.language_id));
        assert_eq!(descriptor.language_id, manifest.language_id);
        assert_eq!(descriptor.descriptor_version, descriptor_version);
        assert!(descriptor_ids.insert(descriptor.descriptor_id.as_str()));
        assert!(!descriptor.recipes.is_empty());
        assert!(descriptor.term_role_overrides.iter().all(|override_| {
            !override_.term.is_empty()
                && matches!(override_.role.as_str(), "context" | "symbol" | "concept")
        }));
        assert!(descriptor.recipes.iter().all(|recipe| {
            !recipe.recipe_id.is_empty()
                && !recipe.trigger.terms.is_empty()
                && matches!(recipe.trigger.r#match.as_str(), "all" | "any")
                && !recipe.clauses.is_empty()
                && recipe.clauses.iter().all(|clause| {
                    !clause.terms.is_empty()
                        && clause
                            .roles
                            .iter()
                            .all(|role| matches!(role.as_str(), "context" | "symbol" | "concept"))
                })
        }));
    }
}
