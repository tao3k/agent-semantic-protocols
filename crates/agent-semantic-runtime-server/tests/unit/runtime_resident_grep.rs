// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use super::execute_runtime_resident_grep_blocks as execute_runtime_resident_grep_blocks_impl;

fn execute_runtime_resident_grep_blocks(
    corpus: &agent_semantic_search::ResidentGrepCorpusArtifact,
    blocks: &[Vec<String>],
    limit: u32,
) -> Result<super::RuntimeResidentGrepAxisReceipt, String> {
    let index =
        agent_semantic_search::ResidentByteCoverageIndex::new(corpus.owner_spans.iter().map(
            |span| agent_semantic_search::ResidentByteCoverageInput {
                owner_path: span.owner_path.clone(),
                authority: None,
                bytes: corpus.owner_bytes(&span.owner_path).unwrap(),
            },
        ))?;
    execute_runtime_resident_grep_blocks_impl(corpus, blocks, limit, |plan, limit| {
        index.candidate_owner_paths_for_grep_plan_with_receipt(plan, None, limit)
    })
}

fn digest(bytes: &[u8]) -> String {
    format!("blake3-256:{}", blake3::hash(bytes).to_hex())
}

fn two_owner_corpus() -> agent_semantic_search::ResidentGrepCorpusArtifact {
    let first = b"pub struct RuntimeServingEndpoint;\n";
    let second = b"pub enum ClientFrame {}\n";
    let first_digest = digest(first);
    let second_digest = digest(second);
    agent_semantic_search::build_resident_grep_corpus(
        &format!("blake3-256:{}", "a".repeat(64)),
        [
            agent_semantic_search::ResidentGrepCorpusOwner {
                owner_path: "src/runtime.rs",
                content_digest: &first_digest,
                bytes: first,
            },
            agent_semantic_search::ResidentGrepCorpusOwner {
                owner_path: "src/client.rs",
                content_digest: &second_digest,
                bytes: second,
            },
        ],
    )
    .expect("cold corpus")
}

#[test]
fn native_rg_reads_each_independent_block_without_process_or_filesystem_work() {
    let corpus = two_owner_corpus();
    let receipt = execute_runtime_resident_grep_blocks(
        &corpus,
        &[
            vec![
                "-n".to_owned(),
                "-e".to_owned(),
                "Runtime".to_owned(),
                ".".to_owned(),
            ],
            vec![
                "-n".to_owned(),
                "-e".to_owned(),
                "Client".to_owned(),
                ".".to_owned(),
            ],
        ],
        4096,
    )
    .expect("resident GREP blocks");
    assert_eq!(receipt.block_receipts.len(), 2);
    assert!(receipt.block_receipts.iter().all(|block| {
        block.execution_engine == "asp-resident-grep-v1"
            && block.candidate_source == "trigram"
            && block.process_count == 0
            && block.filesystem_operation_count == 0
            && block.resident_owner_read_count == 1
            && block.candidate_owner_count == 1
            && block.candidate_gram_count > 0
            && block.decoded_posting_count > 0
    }));
    assert_eq!(receipt.branch_candidate_owner_paths[0], ["src/runtime.rs"]);
    assert_eq!(receipt.branch_candidate_owner_paths[1], ["src/client.rs"]);
    assert_eq!(receipt.branch_matches[0][0].owner_path, "src/runtime.rs");
    assert_eq!(receipt.branch_matches[0][0].owner_line, 1);
    assert_eq!(receipt.branch_matches[1][0].owner_path, "src/client.rs");
    assert_eq!(receipt.branch_matches[1][0].owner_line, 1);
    assert_eq!(
        receipt.candidate_owner_paths,
        ["src/client.rs", "src/runtime.rs"]
    );
    assert!(!receipt.truncated);
}

#[test]
fn native_rg_preserves_resident_fixed_glob_and_short_cluster_argv() {
    let corpus = two_owner_corpus();
    let receipt = execute_runtime_resident_grep_blocks(
        &corpus,
        &[vec![
            "-nF".to_owned(),
            "-g*.rs".to_owned(),
            "-e".to_owned(),
            "Runtime".to_owned(),
            "src".to_owned(),
        ]],
        8,
    )
    .expect("complex resident GREP argv");
    assert_eq!(
        receipt.block_receipts[0].exact_argv,
        ["-nF", "-g*.rs", "-e", "Runtime", "src"]
    );
    assert!(
        receipt.block_receipts[0]
            .argv_digest
            .starts_with("blake3-256:")
    );
    assert_eq!(receipt.block_receipts[0].process_count, 0);
    assert_eq!(receipt.block_receipts[0].filesystem_operation_count, 0);
    assert_eq!(receipt.block_receipts[0].candidate_owner_count, 1);
    assert!(receipt.block_receipts[0].decoded_posting_count > 0);
    assert_eq!(receipt.candidate_owner_paths, ["src/runtime.rs"]);
    assert_eq!(receipt.branch_matches[0][0].owner_line, 1);
}

#[test]
fn native_rg_json_and_filename_only_outputs_remain_attributable() {
    let corpus = two_owner_corpus();
    let json = execute_runtime_resident_grep_blocks(
        &corpus,
        &[vec![
            "--json".to_owned(),
            "Runtime".to_owned(),
            ".".to_owned(),
        ]],
        8,
    )
    .expect("JSON resident GREP");
    assert_eq!(json.candidate_owner_paths, ["src/runtime.rs"]);
    assert_eq!(json.branch_matches[0][0].owner_line, 1);

    let path_only = execute_runtime_resident_grep_blocks(
        &corpus,
        &[vec!["-l".to_owned(), "Client".to_owned(), ".".to_owned()]],
        8,
    )
    .expect("filename-only resident GREP");
    assert_eq!(path_only.candidate_owner_paths, ["src/client.rs"]);
    assert!(path_only.branch_matches[0].is_empty());
}

#[test]
fn native_rg_heading_vimgrep_and_ordinary_output_remain_attributable() {
    let corpus = two_owner_corpus();
    for argv in [
        vec![
            "--heading".to_owned(),
            "-n".to_owned(),
            "Runtime".to_owned(),
            ".".to_owned(),
        ],
        vec!["--vimgrep".to_owned(), "Runtime".to_owned(), ".".to_owned()],
    ] {
        let receipt = execute_runtime_resident_grep_blocks(&corpus, &[argv], 8)
            .expect("line-attributable resident GREP output");
        assert_eq!(receipt.candidate_owner_paths, ["src/runtime.rs"]);
        assert_eq!(receipt.branch_matches[0][0].owner_line, 1);
    }

    let ordinary = execute_runtime_resident_grep_blocks(
        &corpus,
        &[vec!["Runtime".to_owned(), ".".to_owned()]],
        8,
    )
    .expect("ordinary path-attributable resident GREP output");
    assert_eq!(ordinary.candidate_owner_paths, ["src/runtime.rs"]);
    assert!(ordinary.branch_matches[0].is_empty());
}

#[test]
fn native_rg_exit_one_is_a_zero_match_receipt() {
    let corpus = two_owner_corpus();
    let receipt = execute_runtime_resident_grep_blocks(
        &corpus,
        &[vec![
            "-n".to_owned(),
            "definitely_absent".to_owned(),
            ".".to_owned(),
        ]],
        8,
    )
    .expect("zero-match resident GREP");
    assert_eq!(receipt.block_receipts[0].process_count, 0);
    assert!(receipt.candidate_owner_paths.is_empty());
    assert!(receipt.branch_matches[0].is_empty());
}

#[test]
fn native_rg_rejects_process_escape_before_execution() {
    let corpus = two_owner_corpus();
    let error = execute_runtime_resident_grep_blocks(
        &corpus,
        &[vec![
            "--pre".to_owned(),
            "cat".to_owned(),
            "Runtime".to_owned(),
            ".".to_owned(),
        ]],
        8,
    )
    .expect_err("secondary process must fail admission");
    assert!(error.contains("not admitted"), "{error}");
}

#[test]
fn resident_grep_executes_regex_with_mandatory_trigram_anchors() {
    let corpus = two_owner_corpus();
    let receipt = execute_runtime_resident_grep_blocks(
        &corpus,
        &[vec!["-n".to_owned(), "Run.*Serving".to_owned()]],
        8,
    )
    .expect("regex with mandatory trigrams must execute");
    assert_eq!(receipt.candidate_owner_paths, ["src/runtime.rs"]);
}

#[test]
fn resident_grep_4096_owner_reads_have_submillisecond_kernel_and_wall_p95() {
    const OWNER_COUNT: usize = 4096;
    let owners = (0..OWNER_COUNT)
        .map(|index| {
            let path = format!("src/generated/{index:04}.rs");
            let bytes =
                format!("pub fn owner_{index:04}() {{ /* needle_{index:04} */ }}\n").into_bytes();
            let digest = digest(&bytes);
            (path, bytes, digest)
        })
        .collect::<Vec<_>>();
    let corpus = agent_semantic_search::build_resident_grep_corpus(
        &format!("blake3-256:{}", "c".repeat(64)),
        owners.iter().map(
            |(path, bytes, digest)| agent_semantic_search::ResidentGrepCorpusOwner {
                owner_path: path,
                content_digest: digest,
                bytes,
            },
        ),
    )
    .expect("4096-owner resident corpus");
    let byte_index = agent_semantic_search::ResidentByteCoverageIndex::new(owners.iter().map(
        |(path, bytes, _)| agent_semantic_search::ResidentByteCoverageInput {
            owner_path: path.clone(),
            authority: None,
            bytes,
        },
    ))
    .expect("resident byte index");
    let argv = vec!["-n".to_owned(), "needle_4095".to_owned(), ".".to_owned()];
    let mut samples = Vec::new();
    for _ in 0..20 {
        let started = std::time::Instant::now();
        let receipt = execute_runtime_resident_grep_blocks_impl(
            &corpus,
            std::slice::from_ref(&argv),
            OWNER_COUNT as u32,
            |plan, limit| {
                byte_index.candidate_owner_paths_for_grep_plan_with_receipt(plan, None, limit)
            },
        )
        .expect("resident 4096-owner rg read");
        samples.push(started.elapsed());
        assert_eq!(receipt.candidate_owner_paths, ["src/generated/4095.rs"]);
        assert_eq!(receipt.block_receipts[0].resident_owner_read_count, 1);
        assert!(receipt.block_receipts[0].candidate_lookup_nanos < 1_000_000);
        assert_eq!(receipt.block_receipts[0].process_count, 0);
        assert_eq!(receipt.block_receipts[0].filesystem_operation_count, 0);
    }
    samples.sort_unstable();
    let p95 = samples[18];
    assert!(
        p95.as_micros() < 1_000,
        "resident 4096-owner GREP wall p95 exceeded 1ms: p95={p95:?} samples={samples:?}"
    );
}
