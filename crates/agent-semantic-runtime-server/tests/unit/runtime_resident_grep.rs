// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use grep_matcher::Matcher;

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
    let all_owners = corpus
        .owner_spans
        .iter()
        .map(|span| span.owner_path.clone())
        .collect::<Vec<_>>();
    execute_runtime_resident_grep_blocks_impl(corpus, blocks, limit, |plan, limit| {
        if plan.is_match_all() {
            return Ok((
                all_owners.clone(),
                agent_semantic_search::ResidentByteCoverageQueryReceipt {
                    requested_gram_count: 0,
                    decoded_posting_count: 0,
                    smallest_posting_count: 0,
                    candidate_count: all_owners.len(),
                    lookup_nanos: 0,
                },
            ));
        }
        index.candidate_owner_paths_for_grep_plan_with_receipt(plan, None, limit)
    })
}

fn digest(bytes: &[u8]) -> String {
    format!("blake3-256:{}", blake3::hash(bytes).to_hex())
}

#[derive(Debug, serde::Deserialize)]
struct ResidentGrepSemanticsScenario {
    schema_version: String,
    owners: Vec<ResidentGrepSemanticsOwner>,
    cases: Vec<ResidentGrepSemanticsCase>,
}

#[derive(Debug, serde::Deserialize)]
struct ResidentGrepSemanticsOwner {
    path: String,
    content: String,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, serde::Deserialize)]
struct ResidentGrepSemanticsExpectedMatch {
    owner_path: String,
    owner_line: u64,
}

#[derive(Debug, serde::Deserialize)]
struct ResidentGrepSemanticsCase {
    id: String,
    category: String,
    argv: Vec<String>,
    limit: u32,
    expected: Vec<ResidentGrepSemanticsExpectedMatch>,
    truncated: bool,
}

fn resident_grep_semantics_scenario() -> ResidentGrepSemanticsScenario {
    toml::from_str(include_str!(
        "scenarios/runtime_resident_grep_semantics/cases.toml"
    ))
    .expect("resident GREP semantics Scenario")
}

fn scenario_corpus(
    scenario: &ResidentGrepSemanticsScenario,
) -> agent_semantic_search::ResidentGrepCorpusArtifact {
    let digests = scenario
        .owners
        .iter()
        .map(|owner| digest(owner.content.as_bytes()))
        .collect::<Vec<_>>();
    agent_semantic_search::build_resident_grep_corpus(
        &digest(b"resident-grep-semantics-v1"),
        scenario
            .owners
            .iter()
            .zip(&digests)
            .map(
                |(owner, content_digest)| agent_semantic_search::ResidentGrepCorpusOwner {
                    owner_path: &owner.path,
                    content_digest,
                    bytes: owner.content.as_bytes(),
                },
            ),
    )
    .expect("resident GREP Scenario corpus")
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
fn case_flags_obey_last_occurrence() {
    for (flags, expected) in [
        (vec!["-i", "-s"], false),
        (vec!["-s", "-i"], true),
        (vec!["-i", "-S"], false),
    ] {
        let mut argv = flags.into_iter().map(str::to_owned).collect::<Vec<_>>();
        argv.extend(["-n", "-e", "Runtime", "."].map(str::to_owned));
        let analysis = agent_semantic_shell_parser::analyze_native_rg_argv(&argv);
        let matcher = super::compile_resident_matcher(&analysis, 0).unwrap();
        assert_eq!(
            matcher.expression.is_match(b"runtime").unwrap(),
            expected,
            "{argv:?}"
        );
    }
}

#[test]
fn grounding_anchors_are_deduplicated_and_bounded_by_lines() {
    let bytes = b"abc abc abc\nabc\nabc\n";
    let corpus = agent_semantic_search::build_resident_grep_corpus(
        &digest(b"generation"),
        [agent_semantic_search::ResidentGrepCorpusOwner {
            owner_path: "a.rs",
            content_digest: &digest(bytes),
            bytes,
        }],
    )
    .unwrap();
    let result = execute_runtime_resident_grep_blocks(
        &corpus,
        &[vec!["-n".into(), "abc".into(), ".".into()]],
        2,
    )
    .unwrap();
    assert!(result.truncated);
    assert_eq!(
        result.branch_matches[0]
            .iter()
            .map(|hit| hit.owner_line)
            .collect::<Vec<_>>(),
        [1, 2]
    );
    assert_eq!(result.block_receipts[0].resident_regex_scan_count, 1);
}

#[test]
fn one_owner_automaton_preserves_multiline_dotall_and_start_line_attribution() {
    let bytes = b"prefix\nstart\nmiddle\nend\nsuffix\n";
    let corpus = agent_semantic_search::build_resident_grep_corpus(
        &digest(b"generation"),
        [agent_semantic_search::ResidentGrepCorpusOwner {
            owner_path: "a.rs",
            content_digest: &digest(bytes),
            bytes,
        }],
    )
    .unwrap();
    let result = execute_runtime_resident_grep_blocks(
        &corpus,
        &[vec![
            "-U".into(),
            "--multiline-dotall".into(),
            "-n".into(),
            "start.*end".into(),
            ".".into(),
        ]],
        8,
    )
    .unwrap();

    assert_eq!(result.branch_matches[0][0].owner_line, 2);
    assert_eq!(result.block_receipts[0].resident_owner_read_count, 1);
    assert_eq!(result.block_receipts[0].resident_regex_scan_count, 1);
}

#[test]
fn zero_width_eof_is_not_projected_as_a_phantom_line() {
    let bytes = b"value\n\n";
    let empty = b"";
    let corpus = agent_semantic_search::build_resident_grep_corpus(
        &digest(b"generation"),
        [
            agent_semantic_search::ResidentGrepCorpusOwner {
                owner_path: "lines.rs",
                content_digest: &digest(bytes),
                bytes,
            },
            agent_semantic_search::ResidentGrepCorpusOwner {
                owner_path: "empty.rs",
                content_digest: &digest(empty),
                bytes: empty,
            },
        ],
    )
    .unwrap();
    let result = execute_runtime_resident_grep_blocks(
        &corpus,
        &[vec!["-n".into(), "^$".into(), ".".into()]],
        8,
    )
    .unwrap();

    assert_eq!(
        result.branch_matches[0]
            .iter()
            .map(|hit| (hit.owner_path.as_str(), hit.owner_line))
            .collect::<Vec<_>>(),
        [("lines.rs", 2)]
    );
}

#[test]
fn result_limit_is_applied_after_complete_candidate_verification() {
    let false_positive = b"bar appears before foo\n";
    let exact_match = b"foo appears before bar\n";
    let corpus = agent_semantic_search::build_resident_grep_corpus(
        &digest(b"generation"),
        [
            agent_semantic_search::ResidentGrepCorpusOwner {
                owner_path: "a-false-positive.rs",
                content_digest: &digest(false_positive),
                bytes: false_positive,
            },
            agent_semantic_search::ResidentGrepCorpusOwner {
                owner_path: "b-exact-match.rs",
                content_digest: &digest(exact_match),
                bytes: exact_match,
            },
        ],
    )
    .unwrap();
    let result = execute_runtime_resident_grep_blocks(
        &corpus,
        &[vec!["-n".into(), "foo.*bar".into(), ".".into()]],
        1,
    )
    .unwrap();

    assert_eq!(
        result.branch_matches[0]
            .iter()
            .map(|hit| (hit.owner_path.as_str(), hit.owner_line))
            .collect::<Vec<_>>(),
        [("b-exact-match.rs", 1)]
    );
    assert_eq!(result.block_receipts[0].candidate_owner_count, 2);
    assert!(!result.truncated);
}

#[test]
fn resident_grep_semantics_scenario_covers_v1_matrix_without_external_processes() {
    let scenario = resident_grep_semantics_scenario();
    let metadata = toml::from_str::<toml::Value>(include_str!(
        "scenarios/runtime_resident_grep_semantics/scenario.toml"
    ))
    .expect("resident GREP Scenario metadata");
    let benchmark = toml::from_str::<toml::Value>(include_str!(
        "scenarios/runtime_resident_grep_semantics/benchmark.toml"
    ))
    .expect("resident GREP Scenario benchmark metadata");
    assert_eq!(
        scenario.schema_version,
        "asp.runtime-resident-grep-semantics-scenario.v1"
    );
    assert_eq!(
        metadata["scenario"]["id"].as_str(),
        Some("runtime-resident-grep-semantics")
    );
    assert_eq!(
        benchmark["benchmark"]["semantic_case_count"].as_integer(),
        Some(scenario.cases.len() as i64)
    );
    let categories = scenario
        .cases
        .iter()
        .map(|case| case.category.as_str())
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(
        categories,
        std::collections::BTreeSet::from([
            "boundary",
            "crlf",
            "fixed",
            "glob",
            "limit",
            "multiline",
            "unicode",
            "zero-width",
        ])
    );
    let corpus = scenario_corpus(&scenario);
    for case in &scenario.cases {
        let result = execute_runtime_resident_grep_blocks(
            &corpus,
            std::slice::from_ref(&case.argv),
            case.limit,
        )
        .unwrap_or_else(|error| panic!("Scenario case {} failed: {error}", case.id));
        let mut actual = result.branch_matches[0]
            .iter()
            .map(|hit| ResidentGrepSemanticsExpectedMatch {
                owner_path: hit.owner_path.clone(),
                owner_line: hit.owner_line,
            })
            .collect::<Vec<_>>();
        let mut expected = case.expected.clone();
        actual.sort();
        expected.sort();
        assert_eq!(actual, expected, "Scenario mismatch for {}", case.id);
        assert_eq!(result.truncated, case.truncated, "case={}", case.id);
        assert!(result.block_receipts.iter().all(|receipt| {
            receipt.process_count == 0 && receipt.filesystem_operation_count == 0
        }));
    }
}

/// Reference processes exist only in this explicitly selected qualification test.
#[test]
#[ignore = "requires an installed rg reference binary"]
fn admitted_grep_matches_rg_reference_corpus() {
    let scenario = resident_grep_semantics_scenario();
    let directory = tempfile::tempdir().unwrap();
    for owner in &scenario.owners {
        std::fs::write(directory.path().join(&owner.path), &owner.content).unwrap();
    }
    let corpus = scenario_corpus(&scenario);
    let index = agent_semantic_search::ResidentByteCoverageIndex::new(scenario.owners.iter().map(
        |owner| agent_semantic_search::ResidentByteCoverageInput {
            owner_path: owner.path.clone(),
            authority: None,
            bytes: owner.content.as_bytes(),
        },
    ))
    .unwrap();
    for case in &scenario.cases {
        let reference = std::process::Command::new("rg")
            .args(&case.argv)
            .current_dir(directory.path())
            .output()
            .expect("run rg reference");
        assert!(
            matches!(reference.status.code(), Some(0 | 1)),
            "rg failed: {}",
            String::from_utf8_lossy(&reference.stderr)
        );
        let mut expected = String::from_utf8(reference.stdout)
            .unwrap()
            .lines()
            .map(|line| serde_json::from_str::<serde_json::Value>(line).unwrap())
            .filter(|event| event["type"] == "match")
            .map(|event| {
                (
                    event["data"]["path"]["text"]
                        .as_str()
                        .unwrap()
                        .trim_start_matches("./")
                        .to_owned(),
                    event["data"]["line_number"].as_u64().unwrap(),
                )
            })
            .collect::<Vec<_>>();
        let result = execute_runtime_resident_grep_blocks_impl(
            &corpus,
            std::slice::from_ref(&case.argv),
            case.limit,
            |plan, limit| {
                if plan.is_match_all() {
                    Ok((
                        scenario
                            .owners
                            .iter()
                            .map(|owner| owner.path.clone())
                            .collect(),
                        agent_semantic_search::ResidentByteCoverageQueryReceipt {
                            requested_gram_count: 0,
                            decoded_posting_count: 0,
                            smallest_posting_count: 0,
                            candidate_count: scenario.owners.len(),
                            lookup_nanos: 0,
                        },
                    ))
                } else {
                    index.candidate_owner_paths_for_grep_plan_with_receipt(plan, None, limit)
                }
            },
        )
        .unwrap();
        let mut actual = result.branch_matches[0]
            .iter()
            .map(|hit| (hit.owner_path.clone(), hit.owner_line))
            .collect::<Vec<_>>();
        expected.sort();
        actual.sort();
        assert_eq!(actual, expected, "reference mismatch for {}", case.id);
        assert_eq!(result.truncated, case.truncated, "case={}", case.id);
    }
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
            && block.resident_regex_scan_count == 1
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
    assert_eq!(path_only.grounding_matches[0].owner_path, "src/client.rs");
    assert_eq!(path_only.grounding_matches[0].owner_line, 1);
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
    assert_eq!(ordinary.grounding_matches[0].owner_path, "src/runtime.rs");
    assert_eq!(ordinary.grounding_matches[0].owner_line, 1);
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
    assert!(receipt.grounding_matches.is_empty());
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
fn resident_grep_4096_owner_reads_have_submillisecond_kernel_and_wall_p99() {
    const OWNER_COUNT: usize = 4096;
    const SAMPLE_COUNT: usize = 1_024;
    const P99_BUDGET_NANOS: u128 = 1_000_000;
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
    let mut samples = Vec::with_capacity(SAMPLE_COUNT);
    let mut lookup_samples = Vec::with_capacity(SAMPLE_COUNT);
    let mut non_lookup_samples = Vec::with_capacity(SAMPLE_COUNT);
    let mut candidate_gram_count = None;
    let mut decoded_posting_count = None;
    for _ in 0..SAMPLE_COUNT {
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
        let wall_nanos = started.elapsed().as_nanos();
        let lookup_nanos = u128::from(receipt.block_receipts[0].candidate_lookup_nanos);
        samples.push(wall_nanos);
        lookup_samples.push(lookup_nanos);
        non_lookup_samples.push(wall_nanos.saturating_sub(lookup_nanos));
        assert_eq!(receipt.candidate_owner_paths, ["src/generated/4095.rs"]);
        assert_eq!(receipt.block_receipts[0].resident_owner_read_count, 1);
        assert_eq!(receipt.block_receipts[0].resident_regex_scan_count, 1);
        let observed_candidate_gram_count = receipt.block_receipts[0].candidate_gram_count;
        let observed_decoded_posting_count = receipt.block_receipts[0].decoded_posting_count;
        assert!(observed_candidate_gram_count > 1);
        assert!(observed_decoded_posting_count > 0 && observed_decoded_posting_count < OWNER_COUNT);
        assert_eq!(
            *candidate_gram_count.get_or_insert(observed_candidate_gram_count),
            observed_candidate_gram_count
        );
        assert_eq!(
            *decoded_posting_count.get_or_insert(observed_decoded_posting_count),
            observed_decoded_posting_count
        );
        assert_eq!(receipt.block_receipts[0].process_count, 0);
        assert_eq!(receipt.block_receipts[0].filesystem_operation_count, 0);
    }
    samples.sort_unstable();
    lookup_samples.sort_unstable();
    non_lookup_samples.sort_unstable();
    let p50 = samples[SAMPLE_COUNT * 50 / 100 - 1];
    let p95 = samples[SAMPLE_COUNT * 95 / 100 - 1];
    let p99 = samples[SAMPLE_COUNT * 99 / 100 - 1];
    let lookup_p50 = lookup_samples[SAMPLE_COUNT * 50 / 100 - 1];
    let lookup_p95 = lookup_samples[SAMPLE_COUNT * 95 / 100 - 1];
    let lookup_p99 = lookup_samples[SAMPLE_COUNT * 99 / 100 - 1];
    let non_lookup_p50 = non_lookup_samples[SAMPLE_COUNT * 50 / 100 - 1];
    let non_lookup_p95 = non_lookup_samples[SAMPLE_COUNT * 95 / 100 - 1];
    let non_lookup_p99 = non_lookup_samples[SAMPLE_COUNT * 99 / 100 - 1];
    let candidate_gram_count = candidate_gram_count.expect("candidate gram receipt");
    let decoded_posting_count = decoded_posting_count.expect("decoded posting receipt");
    eprintln!(
        "resident-search-grep ownerCount={OWNER_COUNT} samples={SAMPLE_COUNT} candidateGramCount={candidate_gram_count} decodedPostingCount={decoded_posting_count} candidateOwnerReads=1 processCount=0 filesystemOperationCount=0 wallP50Nanos={p50} wallP95Nanos={p95} wallP99Nanos={p99} candidateLookupP50Nanos={lookup_p50} candidateLookupP95Nanos={lookup_p95} candidateLookupP99Nanos={lookup_p99} nonLookupP50Nanos={non_lookup_p50} nonLookupP95Nanos={non_lookup_p95} nonLookupP99Nanos={non_lookup_p99} budgetNanos={P99_BUDGET_NANOS}"
    );
    assert!(
        p99 < P99_BUDGET_NANOS,
        "resident 4096-owner GREP wall p99 exceeded 1ms: p99Nanos={p99}"
    );
}
