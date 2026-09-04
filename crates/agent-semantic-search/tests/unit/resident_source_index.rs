use std::collections::BTreeMap;

use agent_semantic_content_identity::SourceSnapshotEvidence;
use agent_semantic_content_identity::SourceSnapshotKind;
use agent_semantic_content_identity::WorkspaceSnapshot;
use agent_semantic_content_identity::hash_blob;

use crate::ResidentIndexBuildResources;
use crate::ResidentIndexBuildStrategy;
use crate::ResidentSearchAuthority;
use crate::ResidentSourceDocument;
use crate::ResidentSourceIndex;
use crate::resident_lexical_coverage_keys;
use crate::resident_navigation_keys;

fn build_resources() -> ResidentIndexBuildResources {
    ResidentIndexBuildResources::new(
        1,
        32 * 1024 * 1024,
        ResidentIndexBuildStrategy::SingleSegmentBulk,
    )
    .unwrap()
}

#[test]
fn explicit_server_resources_are_validated_without_search_local_scheduling() {
    assert!(
        ResidentIndexBuildResources::new(
            1,
            8 * 1024 * 1024,
            ResidentIndexBuildStrategy::SingleSegmentBulk,
        )
        .expect_err("an unfunded Tantivy worker must fail before indexing")
        .contains("cannot fund")
    );
    assert!(
        ResidentIndexBuildResources::new(
            9,
            1024 * 1024 * 1024,
            ResidentIndexBuildStrategy::ParallelSegments,
        )
        .expect_err("Search must reject an impossible server worker envelope")
        .contains("worker limit")
    );
}

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

fn one_source_document() -> BTreeMap<String, ResidentSourceDocument> {
    BTreeMap::from([(
        "crates/runtime.rs".to_owned(),
        ResidentSourceDocument {
            owner_path: "crates/runtime.rs".to_owned(),
            owner_content_digest: hash_blob(b"runtime").value,
            line_count: 42,
            query_keys: vec!["runtime".to_owned(), "runtime-server".to_owned()],
            authority: Some(authority("rust", "asp-rust")),
        },
    )])
}

#[test]
fn published_tantivy_generation_opens_without_rebuilding_owner_terms() {
    let temporary = tempfile::tempdir().expect("temporary Tantivy generation");
    let generation_digest =
        "blake3-256:1111111111111111111111111111111111111111111111111111111111111111";
    let (built, artifact_digest) = ResidentSourceIndex::build_in_directory(
        temporary.path(),
        one_source_document(),
        source_snapshot(),
        generation_digest.to_owned(),
        build_resources(),
    )
    .expect("build immutable Tantivy artifact");
    assert_eq!(
        built
            .query("runtime-server", Some(&authority("rust", "asp-rust")), 10)
            .expect("built index query")
            .hits
            .len(),
        1
    );
    drop(built);

    let opened = ResidentSourceIndex::open_from_directory(
        temporary.path(),
        &artifact_digest,
        one_source_document(),
        source_snapshot(),
        generation_digest.to_owned(),
    )
    .expect("open immutable Tantivy artifact");
    assert_eq!(
        opened
            .query("runtime-server", Some(&authority("rust", "asp-rust")), 10)
            .expect("opened index query")
            .hits[0]
            .owner_path,
        "crates/runtime.rs"
    );

    assert!(
        ResidentSourceIndex::open_from_directory(
            temporary.path(),
            &hash_blob(b"wrong-artifact").value,
            one_source_document(),
            source_snapshot(),
            generation_digest.to_owned(),
        )
        .expect_err("artifact digest drift must fail")
        .contains("artifact digest mismatch")
    );
}

#[test]
fn exact_and_ranked_queries_use_only_the_resident_generation() {
    let index = ResidentSourceIndex::new(
        BTreeMap::from([(
            "crates/runtime.rs".to_owned(),
            ResidentSourceDocument {
                owner_path: "crates/runtime.rs".to_owned(),
                owner_content_digest: hash_blob(b"runtime").value,
                line_count: 42,
                query_keys: vec!["runtime".to_owned(), "runtime-server".to_owned()],
                authority: Some(authority("rust", "asp-rust")),
            },
        )]),
        source_snapshot(),
        "blake3-256:1111111111111111111111111111111111111111111111111111111111111111".to_owned(),
        build_resources(),
    )
    .unwrap();

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
        BTreeMap::from([
            (
                "crates/runtime.rs".to_owned(),
                ResidentSourceDocument {
                    owner_path: "crates/runtime.rs".to_owned(),
                    owner_content_digest: hash_blob(b"rust").value,
                    line_count: 1,
                    query_keys: vec!["runtime".to_owned()],
                    authority: Some(rust.clone()),
                },
            ),
            (
                "src/runtime.ss".to_owned(),
                ResidentSourceDocument {
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
        build_resources(),
    )
    .unwrap();

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
        source_snapshot(),
        "blake3-256:1111111111111111111111111111111111111111111111111111111111111111".to_owned(),
        build_resources(),
    )
    .unwrap();

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
        source_snapshot(),
        "blake3-256:1111111111111111111111111111111111111111111111111111111111111111".to_owned(),
        build_resources(),
    )
    .unwrap();

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
        BTreeMap::from([(
            "crates/runtime.rs".to_owned(),
            ResidentSourceDocument {
                owner_path: "crates/runtime.rs".to_owned(),
                owner_content_digest: hash_blob(b"runtime").value,
                line_count: 42,
                query_keys: vec!["runtime".to_owned()],
                authority: Some(rust.clone()),
            },
        )]),
        source_snapshot(),
        "blake3-256:1111111111111111111111111111111111111111111111111111111111111111".to_owned(),
        build_resources(),
    )
    .unwrap();

    let cold = index.query("runtime", Some(&rust), 100).unwrap();
    let warm = index.query("runtime", Some(&rust), 100).unwrap();

    assert!(std::sync::Arc::ptr_eq(&cold, &warm));
    assert_eq!(cold.generation_digest, warm.generation_digest);
}

#[test]
fn novel_dense_posting_queries_preserve_bounded_top_k_without_full_vocabulary_clones() {
    const OWNER_COUNT: usize = 2_048;
    const QUERY_COUNT: usize = 16;
    const LIMIT: u32 = 100;
    let rust = authority("rust", "asp-rust");
    let mut source_documents = BTreeMap::new();
    let owner_paths = (0..OWNER_COUNT)
        .map(|index| format!("src/owner_{index:04}.rs"))
        .collect::<Vec<_>>();
    for owner_path in &owner_paths {
        source_documents.insert(
            owner_path.clone(),
            ResidentSourceDocument {
                owner_path: owner_path.clone(),
                owner_content_digest: hash_blob(owner_path.as_bytes()).value,
                line_count: 1,
                query_keys: std::iter::once("common".to_owned())
                    .chain((0..QUERY_COUNT).map(|query| format!("term{query}")))
                    .chain((0..128).map(|key| format!("owner-key-{key}")))
                    .collect(),
                authority: Some(rust.clone()),
            },
        );
    }
    let index = ResidentSourceIndex::new(
        source_documents,
        source_snapshot(),
        "blake3-256:1111111111111111111111111111111111111111111111111111111111111111".to_owned(),
        build_resources(),
    )
    .unwrap();

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
        "[resident-dense-postings] owners={OWNER_COUNT} novelQueries={QUERY_COUNT} postingVisits={} topK={LIMIT} p99Nanos={} diagnosticOnly=true performanceAuthority=large_workspace_playbook_measures_cold_warm_and_concurrent_triad",
        OWNER_COUNT * 2,
        p99.as_nanos()
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
