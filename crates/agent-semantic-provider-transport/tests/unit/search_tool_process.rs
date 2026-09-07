// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::time::Duration;
use std::time::Instant;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;

use super::MAX_INVENTORY_BYTES;
use super::MAX_RG_OUTPUT_BYTES;
use super::ValidatedColdRgCorpus;
use super::parse_fd_inventory;
use super::run_fd_inventory;
use super::run_reference_search_tool;
use super::run_rg_cold_query;

fn fixture_root(name: &str) -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock")
        .as_nanos();
    let root = std::env::temp_dir().join(format!("asp-search-tool-{name}-{nonce}"));
    fs::create_dir_all(root.join("src")).expect("create fixture root");
    root
}

#[tokio::test]
async fn fd_inventory_and_rg_cold_query_remain_separate_bounded_processes() {
    let root = fixture_root("real-tools");
    fs::write(root.join("src/read.rs"), "fn needle_owner() {}\n").expect("write Rust fixture");
    fs::write(root.join("src/ignored.py"), "def needle_owner(): pass\n")
        .expect("write Python fixture");
    let corpus = root.with_extension("cold-rg-corpus");
    let corpus_bytes = b"fn needle_owner() {}\n";
    fs::write(&corpus, corpus_bytes).expect("write exact corpus");
    let corpus_digest = format!(
        "blake3-256:{}",
        agent_semantic_content_identity::ArtifactHash::blake3(corpus_bytes).value
    );
    let corpus = ValidatedColdRgCorpus::open(&corpus, &corpus_digest).expect("validate corpus");
    let inventory = run_fd_inventory(&root, Duration::from_secs(1))
        .await
        .expect("real fd inventory");
    assert_eq!(
        inventory.owner_paths,
        ["src/ignored.py".to_owned(), "src/read.rs".to_owned()]
    );
    assert_eq!(inventory.receipt.backend, "resident-fd-ignore-parallel");
    let fd_parity = run_reference_search_tool(
        Path::new("fd"),
        &root,
        vec![
            "--type".to_owned(),
            "f".to_owned(),
            "--hidden".to_owned(),
            "--exclude".to_owned(),
            ".git".to_owned(),
            "--print0".to_owned(),
            ".".to_owned(),
        ],
        Duration::from_secs(1),
        MAX_INVENTORY_BYTES,
    )
    .await
    .expect("reference fd inventory");
    assert_eq!(
        inventory.owner_paths,
        parse_fd_inventory(&fd_parity.stdout).expect("parse reference fd inventory")
    );

    let query = run_rg_cold_query(&corpus, "needle_owner", Duration::from_secs(1))
        .await
        .expect("real rg query");
    let output = std::str::from_utf8(&query.output).expect("rg output is UTF-8");
    assert!(output.contains("needle_owner"));
    let parity = run_reference_search_tool(
        Path::new("rg"),
        corpus.path().parent().expect("corpus parent"),
        vec![
            "--hidden".to_owned(),
            "--null".to_owned(),
            "--line-number".to_owned(),
            "--column".to_owned(),
            "--color".to_owned(),
            "never".to_owned(),
            "--fixed-strings".to_owned(),
            "--no-heading".to_owned(),
            "--".to_owned(),
            "needle_owner".to_owned(),
            corpus.path().display().to_string(),
        ],
        Duration::from_secs(1),
        MAX_RG_OUTPUT_BYTES,
    )
    .await
    .expect("reference ripgrep query");
    assert_eq!(query.output, parity.stdout, "resident output must match rg");

    fs::remove_dir_all(root).expect("remove fixture root");
    fs::remove_file(corpus.path()).expect("remove cold rg corpus");
}

#[tokio::test]
async fn rg_rejects_corpus_drift_before_spawning_a_process() {
    let root = fixture_root("invalid-path");
    let corpus = root.with_extension("cold-rg-corpus");
    fs::write(&corpus, b"needle\n").expect("write corpus");
    let error = ValidatedColdRgCorpus::open(&corpus, &format!("blake3-256:{}", "0".repeat(64)))
        .expect_err("corpus drift must fail before process admission");
    assert!(error.contains("corpus digest mismatch"));
    fs::remove_dir_all(root).expect("remove fixture root");
    fs::remove_file(corpus).expect("remove cold rg corpus");
}

#[tokio::test]
async fn validated_corpus_is_immutable_after_source_path_drift() {
    let root = fixture_root("immutable-corpus");
    let corpus_path = root.with_extension("cold-rg-corpus");
    let original = b"fn original_needle() {}\n";
    fs::write(&corpus_path, original).expect("write original corpus");
    let digest = format!(
        "blake3-256:{}",
        agent_semantic_content_identity::ArtifactHash::blake3(original).value
    );
    let corpus = ValidatedColdRgCorpus::open(&corpus_path, &digest).expect("validate corpus");
    fs::write(&corpus_path, b"fn drifted_needle() {}\n").expect("mutate source path");
    let original_result = run_rg_cold_query(&corpus, "original_needle", Duration::from_millis(50))
        .await
        .expect("validated bytes remain readable");
    assert_eq!(original_result.receipt.matched_lines, 1);
    let drifted_result = run_rg_cold_query(&corpus, "drifted_needle", Duration::from_millis(50))
        .await
        .expect("drifted path cannot alter resident corpus");
    assert_eq!(drifted_result.receipt.matched_lines, 0);

    fs::remove_dir_all(root).expect("remove immutable fixture root");
    fs::remove_file(corpus_path).expect("remove immutable corpus path");
}

#[tokio::test]
async fn resident_cold_query_deadline_fails_without_spawning() {
    let root = fixture_root("resident-deadline");
    let corpus_path = root.with_extension("cold-rg-corpus");
    let corpus_bytes = b"fn deadline_needle() {}\n";
    fs::write(&corpus_path, corpus_bytes).expect("write deadline corpus");
    let digest = format!(
        "blake3-256:{}",
        agent_semantic_content_identity::ArtifactHash::blake3(corpus_bytes).value
    );
    let corpus = ValidatedColdRgCorpus::open(&corpus_path, &digest).expect("validate corpus");
    let error = run_rg_cold_query(&corpus, "deadline_needle", Duration::ZERO)
        .await
        .expect_err("zero deadline must fail before evidence emission");
    assert!(error.contains("resident cold rg timed out after 0ms"));

    fs::remove_dir_all(root).expect("remove deadline fixture root");
    fs::remove_file(corpus_path).expect("remove deadline corpus path");
}

#[cfg(unix)]
#[tokio::test]
async fn leaf_deadline_kills_and_reaps_before_returning() {
    let root = fixture_root("leaf-timeout");
    let started = Instant::now();
    let error = run_reference_search_tool(
        Path::new("/bin/sleep"),
        &root,
        vec!["5".to_owned()],
        Duration::from_millis(20),
        64,
    )
    .await
    .expect_err("deadline must terminate the leaf process");
    assert!(error.contains("timed out after 20ms"));
    assert!(started.elapsed() < Duration::from_millis(500));
    fs::remove_dir_all(root).expect("remove timeout fixture root");
}

#[tokio::test]
async fn large_workspace_fd_and_cold_rg_are_independent_subsecond_lanes() {
    const OWNER_COUNT: usize = 4_096;
    const SAMPLE_COUNT: usize = 32;
    let root = fixture_root("large-workspace");
    let mut corpus = Vec::new();
    for owner in 0..OWNER_COUNT {
        let name = format!("owner_{owner:04}.rs");
        fs::write(
            root.join(&name),
            format!("pub fn needle_owner_{owner:04}() {{}}\n"),
        )
        .expect("write large-workspace owner");
        corpus.extend_from_slice(format!("pub fn needle_owner_{owner:04}() {{}}\n").as_bytes());
    }
    let corpus_path = root.with_extension("cold-rg-corpus");
    fs::write(&corpus_path, &corpus).expect("write large-workspace exact corpus");
    let corpus_digest = format!(
        "blake3-256:{}",
        agent_semantic_content_identity::ArtifactHash::blake3(&corpus).value
    );
    let corpus = ValidatedColdRgCorpus::open(&corpus_path, &corpus_digest)
        .expect("validate large-workspace corpus");
    let inventory_started = Instant::now();
    let inventory = run_fd_inventory(&root, Duration::from_secs(1))
        .await
        .expect("large-workspace fd inventory");
    let inventory_elapsed = inventory_started.elapsed();
    assert_eq!(inventory.owner_paths.len(), OWNER_COUNT);

    let mut samples = Vec::with_capacity(SAMPLE_COUNT);
    for _ in 0..SAMPLE_COUNT {
        let started = Instant::now();
        let result = run_rg_cold_query(&corpus, "needle_owner_2048", Duration::from_secs(1))
            .await
            .expect("large-workspace cold rg query");
        samples.push(started.elapsed());
        assert_eq!(result.receipt.backend, "resident-ripgrep-fixed-string");
        assert_eq!(result.receipt.corpus_digest, corpus.digest());
        assert_eq!(result.receipt.matched_lines, 1);
        assert!(
            std::str::from_utf8(&result.output)
                .expect("cold rg output")
                .contains("needle_owner_2048")
        );
    }
    samples.sort_unstable();
    let p50 = samples[(samples.len() - 1) * 50 / 100];
    let p95 = samples[(samples.len() - 1) * 95 / 100];
    let p99 = samples[(samples.len() - 1) * 99 / 100];
    eprintln!(
        "fd+rg cold receipt: owners={OWNER_COUNT} fdMicros={} fdBackend={} fdProcesses=0 rgP50Micros={} rgP95Micros={} rgP99Micros={} rgEngineQueries={SAMPLE_COUNT} rgProcesses=0 tantivyBuilds=0",
        inventory_elapsed.as_micros(),
        inventory.receipt.backend,
        p50.as_micros(),
        p95.as_micros(),
        p99.as_micros(),
    );
    assert!(inventory_elapsed < Duration::from_secs(1));
    assert!(p95 < Duration::from_millis(10));
    let mut concurrent = tokio::task::JoinSet::new();
    for _ in 0..SAMPLE_COUNT {
        let corpus = corpus.clone();
        concurrent.spawn(async move {
            let started = Instant::now();
            let result = run_rg_cold_query(&corpus, "needle_owner_2048", Duration::from_secs(1))
                .await
                .expect("concurrent resident cold rg query");
            assert_eq!(result.receipt.matched_lines, 1);
            started.elapsed()
        });
    }
    let mut concurrent_samples = Vec::with_capacity(SAMPLE_COUNT);
    while let Some(result) = concurrent.join_next().await {
        concurrent_samples.push(result.expect("join concurrent cold rg query"));
    }
    concurrent_samples.sort_unstable();
    let concurrent_p99 = concurrent_samples[(concurrent_samples.len() - 1) * 99 / 100];
    eprintln!(
        "resident cold rg concurrency receipt: n={SAMPLE_COUNT} p99Micros={}",
        concurrent_p99.as_micros()
    );
    assert!(concurrent_p99 < Duration::from_millis(100));

    fs::remove_dir_all(root).expect("remove large-workspace fixture");
    fs::remove_file(corpus.path()).expect("remove large-workspace corpus");
}
