use std::collections::BTreeMap;

use agent_semantic_content_identity::{
    SourceSnapshotEvidence, SourceSnapshotKind, WorkspaceSnapshot, hash_blob,
};

use crate::{
    ResidentSearchAuthority, ResidentSourceIndex, ResidentSourceIndexSeed,
    resident_lexical_coverage_keys, resident_navigation_keys,
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
    assert_eq!(exact.hits[0].query_keys, vec!["runtime-server"]);

    let ranked = index.query("runtime", Some(&rust), 10).unwrap();
    assert_eq!(ranked.hits[0].owner_path, "crates/runtime.rs");
    assert_eq!(ranked.hits[0].query_keys, vec!["runtime"]);
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
fn query_limit_is_fail_closed_at_the_shared_top_k_boundary() {
    let index = ResidentSourceIndex::new(
        BTreeMap::new(),
        BTreeMap::new(),
        source_snapshot(),
        "blake3-256:1111111111111111111111111111111111111111111111111111111111111111".to_owned(),
    );

    assert_eq!(
        index.query("runtime", None, 0).unwrap_err(),
        "resident source-index query limit must be in 1..=100: limit=0"
    );
    assert_eq!(
        index.query("runtime", None, 101).unwrap_err(),
        "resident source-index query limit must be in 1..=100: limit=101"
    );
}

#[test]
fn warm_query_cache_reuses_the_generation_bound_result_without_payload_clone() {
    let rust = authority("rust", "asp-rust");
    let index = ResidentSourceIndex::new(
        BTreeMap::from([("runtime".to_owned(), vec!["crates/runtime.rs".to_owned()])]),
        BTreeMap::from([(
            "crates/runtime.rs".to_owned(),
            ResidentSourceIndexSeed {
                owner_path: "crates/runtime.rs".to_owned(),
                owner_content_digest: hash_blob(b"runtime").value,
                line_count: 42,
                query_keys: vec!["runtime".to_owned()],
                authority: Some(rust.clone()),
            },
        )]),
        source_snapshot(),
        "blake3-256:1111111111111111111111111111111111111111111111111111111111111111".to_owned(),
    );

    let cold = index.query("runtime", Some(&rust), 100).unwrap();
    let warm = index.query("runtime", Some(&rust), 100).unwrap();

    assert!(std::sync::Arc::ptr_eq(&cold, &warm));
    assert_eq!(cold.generation_digest, warm.generation_digest);
}

#[test]
fn novel_dense_posting_queries_stay_bounded_without_full_vocabulary_clones() {
    const OWNER_COUNT: usize = 2_048;
    const QUERY_COUNT: usize = 16;
    const LIMIT: u32 = 100;
    let rust = authority("rust", "asp-rust");
    let mut candidate_seeds = BTreeMap::new();
    let owner_paths = (0..OWNER_COUNT)
        .map(|index| format!("src/owner_{index:04}.rs"))
        .collect::<Vec<_>>();
    for owner_path in &owner_paths {
        candidate_seeds.insert(
            owner_path.clone(),
            ResidentSourceIndexSeed {
                owner_path: owner_path.clone(),
                owner_content_digest: hash_blob(owner_path.as_bytes()).value,
                line_count: 1,
                query_keys: (0..128).map(|key| format!("owner-key-{key}")).collect(),
                authority: Some(rust.clone()),
            },
        );
    }
    let mut lexical_index = BTreeMap::from([("common".to_owned(), owner_paths.clone())]);
    for query in 0..QUERY_COUNT {
        lexical_index.insert(format!("term{query}"), owner_paths.clone());
    }
    let index = ResidentSourceIndex::new(
        lexical_index,
        candidate_seeds,
        source_snapshot(),
        "blake3-256:1111111111111111111111111111111111111111111111111111111111111111".to_owned(),
    );

    let mut elapsed = Vec::with_capacity(QUERY_COUNT);
    for query in 0..QUERY_COUNT {
        let started = std::time::Instant::now();
        let result = index
            .query(&format!("term{query} common"), Some(&rust), LIMIT)
            .unwrap();
        elapsed.push(started.elapsed());
        assert_eq!(result.hits.len(), LIMIT as usize);
        assert_eq!(result.hits[0].owner_path, "src/owner_0000.rs");
        assert_eq!(
            result.hits[0].query_keys,
            vec!["common".to_owned(), format!("term{query}")]
        );
        assert!(result.hits.iter().all(|hit| hit.query_keys.len() == 2));
    }
    elapsed.sort_unstable();
    let p99 = elapsed[elapsed.len() - 1];
    eprintln!(
        "[resident-dense-postings] owners={OWNER_COUNT} novelQueries={QUERY_COUNT} postingVisits={} topK={LIMIT} p99Nanos={} budgetNanos=1000000",
        OWNER_COUNT * 2,
        p99.as_nanos()
    );
    assert!(
        p99 < std::time::Duration::from_millis(1),
        "novel dense-posting query exceeded one millisecond: {p99:?}"
    );
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

#[test]
fn admitted_owner_bytes_and_parser_keys_share_one_lexical_coverage() {
    let keys = resident_lexical_coverage_keys(
        "src/runtime_server_admission.rs",
        b"impl WorkspaceGenerationAdmission { fn compare_candidate(&self) {} }",
        ["rust://src/runtime_server_admission.rs#item/method/compare_candidate".to_owned()],
    );

    assert!(keys.contains(&"workspacegenerationadmission".to_owned()));
    assert!(keys.contains(&"compare_candidate".to_owned()));
    assert!(keys.contains(&"workspace".to_owned()));
    assert!(keys.contains(&"generation".to_owned()));
    assert!(keys.contains(&"admission".to_owned()));
    assert!(keys.contains(&"compare".to_owned()));
    assert!(keys.contains(&"candidate".to_owned()));
    assert!(keys.contains(&"runtime_server_admission".to_owned()));
}

#[test]
fn parser_keys_survive_a_saturated_source_coverage_budget() {
    let source = (0..5_000)
        .map(|index| format!("identifier_{index}"))
        .collect::<Vec<_>>()
        .join(" ");
    let keys = resident_lexical_coverage_keys(
        "src/large.rs",
        source.as_bytes(),
        ["zzzz_parser_authority".to_owned()],
    );

    assert_eq!(keys.len(), 4_096);
    assert!(keys.contains(&"zzzz_parser_authority".to_owned()));
    assert!(keys.contains(&"large".to_owned()));
}
