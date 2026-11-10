// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use crate::ProviderQueryPackDescriptor;
use crate::ProviderQueryPackTermRole;
use crate::ProviderSearchCapabilities;
use crate::ProviderSemanticFactsDescriptor;
use crate::builtin_provider_registrations;
use serde_json::json;

#[test]
fn provider_capability_descriptors_round_trip_under_protocol_ownership() {
    let search_json = json!({
        "ownerItems": true,
        "semanticFacts": true,
        "dependencyTopology": true,
        "dependencyTopologyMetadata": false,
        "enhancedSyntaxQueryCapability": {
            "schemaId": "agent.semantic-protocols.enhanced-tree-sitter-query-capability-table",
            "schemaVersion": "1",
            "languageId": "rust",
            "providerId": "asp-rust",
            "tableDigest": "blake3-256:test"
        },
        "sourceSnapshot": {
            "descriptorId": "rust.source-snapshot",
            "descriptorVersion": "1",
            "languageId": "rust",
            "packetSchemaId": "rust.source-packet",
            "exactSourcePacketSchemaId": "rust.exact-source-packet",
            "canonicalItemSelectorSchemaId": "rust.item-selector",
            "sourceSnapshotEnvelopeSchemaId": "rust.source-snapshot-envelope",
            "derivedArtifactEvidenceSchemaId": "rust.derived-artifact-evidence",
            "algorithm": "blake3-256",
            "authority": "provider",
            "exactSelectorResolution": "provider-owned",
            "overlayMode": "immutable-generation"
        }
    });
    let search: ProviderSearchCapabilities =
        serde_json::from_value(search_json.clone()).expect("search capabilities");
    let snapshot = search.source_snapshot.as_ref().expect("source snapshot");
    assert_eq!(snapshot.descriptor_id(), "rust.source-snapshot");
    assert_eq!(snapshot.language_id(), "rust");
    assert_eq!(snapshot.algorithm(), "blake3-256");
    assert_eq!(
        search
            .enhanced_syntax_query_capability
            .as_ref()
            .and_then(|value| value.get("providerId"))
            .and_then(serde_json::Value::as_str),
        Some("asp-rust")
    );
    assert_eq!(serde_json::to_value(&search).unwrap(), search_json);

    let facts_json = json!({
        "descriptorId": "rust.semantic-facts",
        "descriptorVersion": "1",
        "packetSchemaIds": ["rust.semantic-facts-packet"],
        "factKinds": ["function"],
        "intentAxes": [{
            "axis": "symbol",
            "terms": ["RuntimeProvider"],
            "roles": ["symbol"]
        }]
    });
    let facts: ProviderSemanticFactsDescriptor =
        serde_json::from_value(facts_json.clone()).expect("semantic facts descriptor");
    assert_eq!(facts.intent_axes[0].axis(), "symbol");
    assert_eq!(
        facts.intent_axes[0].terms().collect::<Vec<_>>(),
        ["RuntimeProvider"]
    );
    assert_eq!(
        facts.intent_axes[0].roles(),
        [ProviderQueryPackTermRole::Symbol]
    );
    assert_eq!(serde_json::to_value(&facts).unwrap(), facts_json);

    let query_pack_json = json!({
        "descriptorId": "rust.query-pack",
        "descriptorVersion": "1",
        "languageId": "rust",
        "semanticFactsDescriptorId": "rust.semantic-facts",
        "termRoleOverrides": [{
            "term": "RuntimeProvider",
            "role": "symbol",
            "caseSensitive": true
        }],
        "recipes": [{
            "recipeId": "runtime-provider",
            "trigger": {"terms": ["runtime", "provider"], "match": "all"},
            "clauses": [{
                "terms": ["RuntimeProvider"],
                "roles": ["symbol"],
                "intentAxes": ["definition"]
            }]
        }]
    });
    let query_pack: ProviderQueryPackDescriptor =
        serde_json::from_value(query_pack_json.clone()).expect("query pack descriptor");
    assert_eq!(query_pack.descriptor_id(), "rust.query-pack");
    assert_eq!(query_pack.language_id(), "rust");
    assert_eq!(query_pack.recipes()[0].recipe_id, "runtime-provider");
    assert_eq!(serde_json::to_value(&query_pack).unwrap(), query_pack_json);
}

#[test]
fn provider_capability_descriptors_reject_hook_private_extensions() {
    let error = serde_json::from_value::<ProviderSearchCapabilities>(json!({
        "ownerItems": true,
        "semanticFacts": true,
        "dependencyTopology": false,
        "dependencyTopologyMetadata": false,
        "hookRoute": "asp languages search playbook"
    }))
    .expect_err("provider protocol must reject Hook-private fields");
    assert!(error.to_string().contains("unknown field `hookRoute`"));
}

#[test]
fn builtin_rust_enhanced_query_capability_survives_the_provider_protocol_projection() {
    let rust = builtin_provider_registrations()
        .expect("built-in provider registrations")
        .into_iter()
        .find(|provider| provider.provider_id == "asp-rust")
        .expect("Rust provider registration");
    let raw = rust
        .registration_field("searchCapabilities")
        .expect("Rust search capabilities")
        .clone();
    let decoded: ProviderSearchCapabilities =
        serde_json::from_value(raw.clone()).expect("provider protocol projection");
    let capability = decoded
        .enhanced_syntax_query_capability
        .as_ref()
        .expect("enhanced Query capability");

    assert_eq!(serde_json::to_value(&decoded).unwrap(), raw);
    assert_eq!(
        capability
            .get("schemaVersion")
            .and_then(serde_json::Value::as_str),
        Some("1")
    );
    assert!(capability.get("tableDigest").is_some());
    assert!(capability.get("$ref").is_none());
}
