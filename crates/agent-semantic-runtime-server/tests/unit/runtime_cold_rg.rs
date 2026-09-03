use std::time::Duration;

use super::execute_runtime_cold_rg;

fn digest(bytes: &[u8]) -> String {
    format!("blake3-256:{}", blake3::hash(bytes).to_hex())
}

#[tokio::test]
async fn cold_rg_reads_one_immutable_corpus_and_maps_hits_to_owners() {
    let first = b"pub fn cold_owner() {}\n";
    let second = b"pub fn unrelated() {}\n";
    let first_digest = digest(first);
    let second_digest = digest(second);
    let corpus = agent_semantic_search::build_cold_rg_corpus(
        &format!("blake3-256:{}", "a".repeat(64)),
        [
            agent_semantic_search::ColdRgCorpusOwner {
                owner_path: "src/cold.rs",
                content_digest: &first_digest,
                bytes: first,
            },
            agent_semantic_search::ColdRgCorpusOwner {
                owner_path: "src/other.rs",
                content_digest: &second_digest,
                bytes: second,
            },
        ],
    )
    .expect("cold corpus");

    let receipt = execute_runtime_cold_rg(&corpus, "cold_owner", 8, Duration::from_millis(500))
        .await
        .expect("bounded rg");
    assert_eq!(receipt.process_count, 1);
    assert_eq!(receipt.candidate_owner_paths, ["src/cold.rs"]);
    assert!(receipt.elapsed_micros > 0);
    assert!(receipt.coverage_input_digest.starts_with("blake3-256:"));
}

#[tokio::test]
async fn cold_rg_timeout_kills_and_reaps_the_only_process() {
    let bytes = b"pub fn cold_owner() {}\n";
    let content_digest = digest(bytes);
    let corpus = agent_semantic_search::build_cold_rg_corpus(
        &format!("blake3-256:{}", "a".repeat(64)),
        [agent_semantic_search::ColdRgCorpusOwner {
            owner_path: "src/cold.rs",
            content_digest: &content_digest,
            bytes,
        }],
    )
    .expect("cold corpus");

    let error = execute_runtime_cold_rg(&corpus, "cold_owner", 8, Duration::from_nanos(1))
        .await
        .expect_err("expired deadline must fail closed");
    assert!(error.contains("process tree killed and reaped"), "{error}");
}

#[tokio::test]
async fn cold_rg_4096_owner_corpus_stays_below_the_cold_budget() {
    const OWNER_COUNT: usize = 4096;
    const SAMPLE_COUNT: usize = 16;
    let owner_bytes = (0..OWNER_COUNT)
        .map(|index| format!("pub fn owner_{index}() {{}}\n").into_bytes())
        .collect::<Vec<_>>();
    let owner_digests = owner_bytes
        .iter()
        .map(|bytes| digest(bytes))
        .collect::<Vec<_>>();
    let owner_paths = (0..OWNER_COUNT)
        .map(|index| format!("src/owner_{index}.rs"))
        .collect::<Vec<_>>();
    let corpus = agent_semantic_search::build_cold_rg_corpus(
        &format!("blake3-256:{}", "b".repeat(64)),
        owner_bytes.iter().enumerate().map(|(index, bytes)| {
            agent_semantic_search::ColdRgCorpusOwner {
                owner_path: &owner_paths[index],
                content_digest: &owner_digests[index],
                bytes,
            }
        }),
    )
    .expect("large cold corpus");

    let mut samples = Vec::with_capacity(SAMPLE_COUNT);
    for _ in 0..SAMPLE_COUNT {
        let receipt = execute_runtime_cold_rg(&corpus, "owner_4095", 8, Duration::from_millis(250))
            .await
            .expect("bounded large cold rg");
        assert_eq!(receipt.candidate_owner_paths, ["src/owner_4095.rs"]);
        samples.push(receipt.elapsed_micros);
    }
    samples.sort_unstable();
    let p95 = samples[(SAMPLE_COUNT * 95).div_ceil(100) - 1];
    eprintln!("cold-rg owners={OWNER_COUNT} samples={SAMPLE_COUNT} p95={p95}us");
    assert!(p95 < 100_000, "cold rg p95={p95}us");
}
