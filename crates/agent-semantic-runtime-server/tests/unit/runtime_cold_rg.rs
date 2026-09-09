// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use super::execute_runtime_native_rg_blocks;

fn digest(bytes: &[u8]) -> String {
    format!("blake3-256:{}", blake3::hash(bytes).to_hex())
}

fn two_owner_corpus() -> agent_semantic_search::ColdRgCorpusArtifact {
    let first = b"pub struct RuntimeServingEndpoint;\n";
    let second = b"pub enum ClientFrame {}\n";
    let first_digest = digest(first);
    let second_digest = digest(second);
    agent_semantic_search::build_cold_rg_corpus(
        &format!("blake3-256:{}", "a".repeat(64)),
        [
            agent_semantic_search::ColdRgCorpusOwner {
                owner_path: "src/runtime.rs",
                content_digest: &first_digest,
                bytes: first,
            },
            agent_semantic_search::ColdRgCorpusOwner {
                owner_path: "src/client.rs",
                content_digest: &second_digest,
                bytes: second,
            },
        ],
    )
    .expect("cold corpus")
}

#[test]
fn native_rg_executes_one_exact_process_per_independent_block() {
    let corpus = two_owner_corpus();
    let receipt = execute_runtime_native_rg_blocks(
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
    .expect("native rg blocks");
    assert_eq!(receipt.process_count, 2);
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
fn native_rg_preserves_complex_glob_type_and_short_cluster_argv() {
    let corpus = two_owner_corpus();
    let receipt = execute_runtime_native_rg_blocks(
        &corpus,
        &[vec![
            "-nFi".to_owned(),
            "-g*.rs".to_owned(),
            "--type".to_owned(),
            "rust".to_owned(),
            "-e".to_owned(),
            "runtime".to_owned(),
            "src".to_owned(),
        ]],
        8,
    )
    .expect("complex native rg argv");
    assert_eq!(receipt.process_count, 1);
    assert_eq!(
        receipt.processes[0].exact_argv,
        ["-nFi", "-g*.rs", "--type", "rust", "-e", "runtime", "src"]
    );
    assert!(receipt.processes[0].argv_digest.starts_with("blake3-256:"));
    assert_eq!(receipt.candidate_owner_paths, ["src/runtime.rs"]);
    assert_eq!(receipt.branch_matches[0][0].owner_line, 1);
}

#[test]
fn native_rg_json_and_filename_only_outputs_remain_attributable() {
    let corpus = two_owner_corpus();
    let json = execute_runtime_native_rg_blocks(
        &corpus,
        &[vec![
            "--json".to_owned(),
            "Runtime".to_owned(),
            ".".to_owned(),
        ]],
        8,
    )
    .expect("JSON native rg");
    assert_eq!(json.candidate_owner_paths, ["src/runtime.rs"]);
    assert_eq!(json.branch_matches[0][0].owner_line, 1);

    let path_only = execute_runtime_native_rg_blocks(
        &corpus,
        &[vec!["-l".to_owned(), "Client".to_owned(), ".".to_owned()]],
        8,
    )
    .expect("filename-only native rg");
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
        let receipt = execute_runtime_native_rg_blocks(&corpus, &[argv], 8)
            .expect("line-attributable native rg output");
        assert_eq!(receipt.candidate_owner_paths, ["src/runtime.rs"]);
        assert_eq!(receipt.branch_matches[0][0].owner_line, 1);
    }

    let ordinary =
        execute_runtime_native_rg_blocks(&corpus, &[vec!["Runtime".to_owned(), ".".to_owned()]], 8)
            .expect("ordinary path-attributable native rg output");
    assert_eq!(ordinary.candidate_owner_paths, ["src/runtime.rs"]);
    assert!(ordinary.branch_matches[0].is_empty());
}

#[test]
fn native_rg_exit_one_is_a_zero_match_receipt() {
    let corpus = two_owner_corpus();
    let receipt = execute_runtime_native_rg_blocks(
        &corpus,
        &[vec![
            "-n".to_owned(),
            "definitely_absent".to_owned(),
            ".".to_owned(),
        ]],
        8,
    )
    .expect("zero-match native rg");
    assert_eq!(receipt.process_count, 1);
    assert!(receipt.candidate_owner_paths.is_empty());
    assert!(receipt.branch_matches[0].is_empty());
}

#[test]
fn native_rg_rejects_process_escape_before_execution() {
    let corpus = two_owner_corpus();
    let error = execute_runtime_native_rg_blocks(
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
