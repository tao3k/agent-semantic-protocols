use std::collections::BTreeMap;

use agent_semantic_content_identity::{
    SourceSnapshotEvidence, SourceSnapshotKind, WorkspaceSnapshot, hash_blob,
};

use crate::{
    ResidentSearchAuthority, ResidentSourceIndex, ResidentSourceIndexSeed, resident_navigation_keys,
};

fn authority(language_id: &str, provider_id: &str) -> ResidentSearchAuthority {
    ResidentSearchAuthority {
        language_id: language_id.into(),
        provider_id: provider_id.into(),
    }
}

fn source_snapshot() -> SourceSnapshotEvidence {
    WorkspaceSnapshot::from_file_hashes([("crates/runtime.rs", hash_blob(b"runtime").value)])
        .evidence(SourceSnapshotKind::Filesystem, hash_blob(b"asp-rust").value)
}

#[test]
fn exact_and_ranked_queries_use_only_the_resident_generation() {
    let index = ResidentSourceIndex::new(
        BTreeMap::from([
            (
                "runtime-server".to_owned(),
                vec!["crates/runtime.rs".to_owned()],
            ),
            ("runtime".to_owned(), vec!["crates/runtime.rs".to_owned()]),
        ]),
        BTreeMap::from([(
            "crates/runtime.rs".to_owned(),
            ResidentSourceIndexSeed {
                owner_path: "crates/runtime.rs".to_owned(),
                owner_content_digest: hash_blob(b"runtime").value,
                line_count: 42,
                query_keys: vec!["runtime".to_owned(), "runtime-server".to_owned()],
                authority: Some(authority("rust", "asp-rust")),
            },
        )]),
        source_snapshot(),
        "blake3-256:1111111111111111111111111111111111111111111111111111111111111111".to_owned(),
    );

    let rust = authority("rust", "asp-rust");
    let exact = index.query("runtime-server", Some(&rust), 10).unwrap();
    assert_eq!(
        exact.state,
        agent_semantic_search_projection::ResidentSearchReadyState::Ready
    );
    assert_eq!(exact.hits[0].owner_path, "crates/runtime.rs");
    assert_eq!(exact.hits[0].language_id.as_deref(), Some("rust"));

    let ranked = index.query("runtime", Some(&rust), 10).unwrap();
    assert_eq!(ranked.hits[0].owner_path, "crates/runtime.rs");
}

#[test]
fn provider_authority_filters_before_limit_and_never_relabels_hits() {
    let rust = authority("rust", "asp-rust");
    let gerbil = authority("gerbil-scheme", "asp-gerbil-scheme");
    let index = ResidentSourceIndex::new(
        BTreeMap::from([(
            "runtime".to_owned(),
            vec!["crates/runtime.rs".to_owned(), "src/runtime.ss".to_owned()],
        )]),
        BTreeMap::from([
            (
                "crates/runtime.rs".to_owned(),
                ResidentSourceIndexSeed {
                    owner_path: "crates/runtime.rs".to_owned(),
                    owner_content_digest: hash_blob(b"rust").value,
                    line_count: 1,
                    query_keys: vec!["runtime".to_owned()],
                    authority: Some(rust.clone()),
                },
            ),
            (
                "src/runtime.ss".to_owned(),
                ResidentSourceIndexSeed {
                    owner_path: "src/runtime.ss".to_owned(),
                    owner_content_digest: hash_blob(b"gerbil").value,
                    line_count: 1,
                    query_keys: vec!["runtime".to_owned()],
                    authority: Some(gerbil.clone()),
                },
            ),
        ]),
        source_snapshot(),
        "blake3-256:1111111111111111111111111111111111111111111111111111111111111111".to_owned(),
    );

    let result = index.query("runtime", Some(&gerbil), 1).unwrap();
    assert_eq!(result.hits.len(), 1);
    assert_eq!(result.hits[0].owner_path, "src/runtime.ss");
    assert_eq!(result.hits[0].language_id.as_deref(), Some("gerbil-scheme"));
    assert!(
        index.query("runtime", Some(&rust), 1).unwrap().hits[0]
            .owner_path
            .ends_with(".rs")
    );
}

#[test]
fn a_published_empty_generation_is_a_miss_not_a_cold_or_db_state() {
    let index = ResidentSourceIndex::new(
        BTreeMap::new(),
        BTreeMap::new(),
        source_snapshot(),
        "blake3-256:1111111111111111111111111111111111111111111111111111111111111111".to_owned(),
    );

    let result = index.query("missing", None, 10).unwrap();
    assert_eq!(
        result.state,
        agent_semantic_search_projection::ResidentSearchReadyState::Ready
    );
    assert!(result.hits.is_empty());
}

#[test]
fn durable_navigation_keys_are_path_shallow_and_never_source_text() {
    let keys = resident_navigation_keys("packages/runtime/search/src/router.rs");

    assert!(keys.iter().any(|key| key == "router"));
    assert!(
        keys.iter()
            .any(|key| key == "packages/runtime/search/src/router.rs")
    );
    assert!(!keys.iter().any(|key| key == "DynamicOverlaySearch"));
}
