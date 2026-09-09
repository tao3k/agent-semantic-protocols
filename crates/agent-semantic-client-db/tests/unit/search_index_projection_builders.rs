// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use super::add_parser_owner_contains_relations;

#[test]
fn parser_containment_is_complete_and_coalesces_provider_duplicates() {
    let mut relations = vec![
        agent_semantic_content_identity::provider_projection_relation::ProviderProjectedRelation {
            from: agent_semantic_content_identity::provider_projection_relation::ProviderProjectedRelationEndpoint {
                kind: agent_semantic_content_identity::ProviderRelationEndpointKindV1::Owner,
                id: "src/lib.rs".to_owned(),
            },
            kind: agent_semantic_content_identity::ProviderRelationKindV1::from("contains"),
            to: agent_semantic_content_identity::provider_projection_relation::ProviderProjectedRelationEndpoint {
                kind: agent_semantic_content_identity::ProviderRelationEndpointKindV1::Item,
                id: "rust://src/lib.rs#item/function/run".to_owned(),
            },
        },
    ];
    add_parser_owner_contains_relations(
        &mut relations,
        [
            (
                "src/lib.rs".to_owned(),
                "rust://src/lib.rs#item/function/run".to_owned(),
            ),
            (
                "src/lib.rs".to_owned(),
                "rust://src/lib.rs#item/type/Runtime".to_owned(),
            ),
        ],
    );
    relations.sort();
    assert_eq!(relations.len(), 2);
    assert!(relations.iter().any(|relation| {
        relation.kind.as_str() == "CONTAINS"
            && relation.to.id == "rust://src/lib.rs#item/type/Runtime"
    }));
}
