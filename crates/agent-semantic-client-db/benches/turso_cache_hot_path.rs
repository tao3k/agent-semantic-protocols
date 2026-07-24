use std::time::{SystemTime, UNIX_EPOCH};

use agent_semantic_client_db::ClientDbEngine;
use criterion::{Criterion, criterion_group, criterion_main};

fn turso_cache_hot_path(c: &mut Criterion) {
    use agent_semantic_content_identity::canonical_item_identity::{
        CanonicalItemIdentityV1, CanonicalItemSelectorV1,
    };
    use agent_semantic_content_identity::exact_selector_cache::ExactSelectorMerkleLookupKeyV1;
    use agent_semantic_content_identity::exact_selector_merkle::{
        ExactProjectionModeV1, blake3_content_digest_v1, canonical_content_digest_v1,
    };
    use agent_semantic_content_identity::exact_selector_projection_packet::build_exact_selector_projection_packet_v1;
    use agent_semantic_content_identity::workspace_merkle_v1::WorkspacePathMerkleTreeV1;

    c.bench_function("turso_engine_inspect_client_dir", |b| {
        b.iter(|| {
            let client_dir = std::env::temp_dir().join(format!(
                "asp-turso-cache-hot-path-{}",
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .expect("system time before unix epoch")
                    .as_nanos()
            ));
            let report = ClientDbEngine::inspect_client_dir(&client_dir);
            assert_eq!(report.db_path, client_dir.join("facts.turso"));
            let _ = std::fs::remove_dir_all(client_dir);
        });
    });

    let owner_path = "src/lib.rs";
    let selector = "rust://src/lib.rs#item/function/warm_symbol";
    let source = b"fn warm_symbol() -> usize { 7 }\n";
    let source_blob_digest = blake3_content_digest_v1(source);
    let parser_identity_digest = canonical_content_digest_v1(b"parser", &[b"rs-harness"]);
    let query_pack_digest = canonical_content_digest_v1(b"query-pack", &[b"rust"]);
    let tree = WorkspacePathMerkleTreeV1::from_file_digests([(
        owner_path.to_string(),
        source_blob_digest.clone(),
    )])
    .expect("build benchmark workspace tree");
    let owner_subtree_digest = tree
        .owner_subtree_digest(owner_path)
        .expect("resolve benchmark owner subtree");
    let projection_mode = ExactProjectionModeV1::Code;
    let canonical_item_selector = CanonicalItemSelectorV1::new(
        CanonicalItemIdentityV1::new("rust", "function", "warm_symbol"),
        selector,
    );
    let packet = build_exact_selector_projection_packet_v1(
        "rust",
        "rs-harness",
        canonical_item_selector,
        &parser_identity_digest,
        &query_pack_digest,
        owner_path,
        selector,
        projection_mode,
        source,
        br#"{"kind":"fn","name":"warm_symbol"}"#,
        source,
    );
    let record = packet
        .enrich_projection_record(&tree)
        .expect("enrich benchmark exact-selector projection");
    let warm_key = ExactSelectorMerkleLookupKeyV1 {
        language_id: "rust",
        workspace_root_digest: tree.root_digest(),
        owner_path,
        owner_subtree_digest,
        source_blob_digest: &source_blob_digest,
        parser_identity_digest: &parser_identity_digest,
        query_pack_digest: &query_pack_digest,
        structural_selector: selector,
        projection_mode,
    };
    let warm_dir = std::env::temp_dir().join(format!(
        "asp-turso-exact-selector-warm-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time before unix epoch")
            .as_nanos()
    ));
    let _ = std::fs::remove_dir_all(&warm_dir);
    std::fs::create_dir_all(&warm_dir).expect("create warm exact-selector bench dir");
    ClientDbEngine::persist_exact_selector_projection_v1_from_client_dir(
        &warm_dir, &warm_key, &record,
    )
    .expect("seed warm exact-selector projection");

    c.bench_function("turso_exact_selector_warm_lookup", |b| {
        b.iter(|| {
            let hit = ClientDbEngine::lookup_exact_selector_projection_v1_from_client_dir(
                std::hint::black_box(&warm_dir),
                std::hint::black_box(&warm_key),
            )
            .expect("lookup warm exact-selector projection")
            .expect("warm exact-selector projection");
            std::hint::black_box(hit);
        });
    });

    let missing_selector = "rust://src/lib.rs#item/function/missing_symbol";
    let missing_key = ExactSelectorMerkleLookupKeyV1 {
        structural_selector: missing_selector,
        ..warm_key
    };
    c.bench_function("turso_exact_selector_missing_lookup", |b| {
        b.iter(|| {
            let miss = ClientDbEngine::lookup_exact_selector_projection_v1_from_client_dir(
                std::hint::black_box(&warm_dir),
                std::hint::black_box(&missing_key),
            )
            .expect("lookup missing exact-selector projection");
            assert!(miss.is_none());
        });
    });

    c.bench_function("turso_exact_selector_fresh_client_dir_miss", |b| {
        b.iter(|| {
            let client_dir = std::env::temp_dir().join(format!(
                "asp-turso-exact-selector-fresh-{}",
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .expect("system time before unix epoch")
                    .as_nanos()
            ));
            let _ = std::fs::remove_dir_all(&client_dir);
            std::fs::create_dir_all(&client_dir).expect("create fresh exact-selector bench dir");
            let miss = ClientDbEngine::lookup_exact_selector_projection_v1_from_client_dir(
                std::hint::black_box(&client_dir),
                std::hint::black_box(&missing_key),
            )
            .expect("lookup exact-selector projection in fresh client dir");
            assert!(miss.is_none());
            let _ = std::fs::remove_dir_all(&client_dir);
        });
    });

    let persist_dir = std::env::temp_dir().join(format!(
        "asp-turso-exact-selector-persist-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time before unix epoch")
            .as_nanos()
    ));
    let _ = std::fs::remove_dir_all(&persist_dir);
    std::fs::create_dir_all(&persist_dir).expect("create persist exact-selector bench dir");
    c.bench_function("turso_exact_selector_persist_replace", |b| {
        b.iter(|| {
            ClientDbEngine::persist_exact_selector_projection_v1_from_client_dir(
                std::hint::black_box(&persist_dir),
                std::hint::black_box(&warm_key),
                std::hint::black_box(&record),
            )
            .expect("persist exact-selector projection");
        });
    });
}

criterion_group!(benches, turso_cache_hot_path);
criterion_main!(benches);
