use std::collections::BTreeSet;

use agent_semantic_content_identity::canonical_item_identity::CANONICAL_ITEM_SELECTOR_SCHEMA_ID;
use agent_semantic_hook::{builtin_provider_manifests, registered_language_ids};

#[test]
fn every_registered_language_declares_the_canonical_item_selector_v1_contract() {
    let registered = registered_language_ids()
        .into_iter()
        .collect::<BTreeSet<_>>();
    let manifests = builtin_provider_manifests();
    let manifested = manifests
        .iter()
        .map(|manifest| manifest.language_id().clone())
        .collect::<BTreeSet<_>>();

    assert_eq!(
        manifested, registered,
        "the canonical identity gate must be generated from the complete registered-language set"
    );

    for manifest in manifests {
        let source_snapshot = manifest
            .search_capabilities()
            .source_snapshot
            .as_ref()
            .unwrap_or_else(|| {
                panic!(
                    "{} must register source-snapshot exact identity capabilities",
                    manifest.language_id()
                )
            });
        assert_eq!(
            source_snapshot.canonical_item_selector_schema_id(),
            CANONICAL_ITEM_SELECTOR_SCHEMA_ID,
            "{} must register the shared canonical item selector contract",
            manifest.language_id(),
        );
        assert_eq!(
            source_snapshot.exact_source_packet_schema_id(),
            "asp.exact-source-query-result.v1",
            "{} must return the shared typed exact result",
            manifest.language_id(),
        );
        assert_eq!(
            source_snapshot.source_snapshot_envelope_schema_id(),
            "asp.exact-source-snapshot-envelope.v1",
            "{} must consume the shared exact envelope",
            manifest.language_id(),
        );
        assert_eq!(
            source_snapshot.exact_selector_resolution(),
            "pinned-live-module-graph",
            "{} must resolve exact selectors from parser-owned identity",
            manifest.language_id(),
        );
    }
}
