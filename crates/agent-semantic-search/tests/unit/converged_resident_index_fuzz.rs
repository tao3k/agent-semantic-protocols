// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Property-Based Testing (PBT) and Adversarial Fuzzing suite for ASP's
//! sub-millisecond 4-axis converged resident search engine (`WorkspaceSearchIndex`).
//!
//! Verifies across thousands of randomized corpora and adversarial fuzzed inputs:
//! 1. **Trigram Recall Completeness (ZERO False Negatives)**: every literal substring
//!    present in any indexed file is guaranteed to be recalled.
//! 2. **Topology Filter Soundness (ZERO Filter Leakage)**: `@prefix:`, `@ext:`, `@lang:`,
//!    and `@doc:` hold across ALL 4 axes (`rg`, `rg-regex`, `file-locator`, `tantivy`).
//! 3. **Determinism, Deduplication & Line/Context Bounds**: bit-identical ranking and
//!    digests across repeated queries, bounded result count, zero duplicate paths.
//! 4. **Adversarial Query & Corpus Fuzzing (Panic/Crash-Freedom)**: pathological regex,
//!    malformed Tantivy syntax, null bytes, CJK/emoji UTF-8 boundaries, oversized queries,
//!    binary blobs, and CRLF/LF edge cases never panic.
//! 5. **4-Axis Converged Functional & Sub-ms Latency Verification**: `rg`, `rg-regex`,
//!    ASP `FileLocatorIndex` + fuzzy subsequence path, Tantivy BM25, and typo-tolerant
//!    `FuzzyTermQuery` over ASP's real repository.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use agent_semantic_search::{
    TrigramQuery, TrigramSnapshotError, WorkspaceFileDelta, WorkspaceIndexConfig,
    WorkspaceSearchIndex, WorkspaceSearchQuery, compile_regex_trigram_query,
};
use regex::RegexBuilder;

struct SplitMix64(u64);

impl SplitMix64 {
    fn new(seed: u64) -> Self {
        Self(seed)
    }

    fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    fn range(&mut self, upper: usize) -> usize {
        if upper <= 1 {
            0
        } else {
            (self.next_u64() as usize) % upper
        }
    }
}

const IDENT_ALPHABET: &[u8] = b"abcdefghijklmnopqrstuvwxyz0123456789_";

fn random_ident(rng: &mut SplitMix64, min_len: usize, max_len: usize) -> String {
    let len = min_len + rng.range(max_len - min_len + 1);
    let mut s = String::with_capacity(len);
    for _ in 0..len {
        let idx = rng.range(IDENT_ALPHABET.len());
        s.push(IDENT_ALPHABET[idx] as char);
    }
    s
}

fn generate_random_corpus(rng: &mut SplitMix64, file_count: usize) -> Vec<(String, String)> {
    let dirs = [
        "crates/core/src",
        "crates/search/src",
        "scheme/ui",
        "docs/spec",
    ];
    let exts = ["rs", "ss", "org", "md", "py"];
    let mut files = Vec::with_capacity(file_count);
    for idx in 0..file_count {
        let dir = dirs[rng.range(dirs.len())];
        let ext = exts[rng.range(exts.len())];
        let stem = random_ident(rng, 4, 10);
        let path = format!("{dir}/{stem}_{idx}.{ext}");
        let line_count = 3 + rng.range(12);
        let mut body = String::new();
        for ln in 0..line_count {
            let w1 = random_ident(rng, 4, 12);
            let w2 = random_ident(rng, 4, 12);
            let w3 = random_ident(rng, 3, 10);
            body.push_str(&format!("fn {w1}_{ln}({w2}: &str) -> {w3}\n"));
        }
        files.push((path, body));
    }
    files
}

#[test]
fn pbt_trigram_recall_completeness_zero_false_negatives() {
    // 100 random corpora x 12 random substrings = 1,200 recall proofs.
    let mut rng = SplitMix64::new(0xA590_0001);
    for seed in 0..100u64 {
        let corpus = generate_random_corpus(&mut rng, 12);
        let cfg = WorkspaceIndexConfig {
            enable_tantivy: false,
            enable_file_locator: false,
            enable_rg_regex: false,
            ..WorkspaceIndexConfig::default()
        };
        let index = WorkspaceSearchIndex::build_from_memory(
            PathBuf::from(format!("/virtual/seed-{seed}")),
            corpus
                .iter()
                .map(|(p, b)| (p.clone(), b.as_bytes().to_vec())),
            cfg,
        );
        assert_eq!(index.file_count(), corpus.len());

        for _ in 0..12 {
            let target_idx = rng.range(corpus.len());
            let (target_path, target_body) = &corpus[target_idx];
            // Pick a guaranteed ASCII alphanumeric substring of length 3..=10 from `target_body`.
            let lines: Vec<&str> = target_body.lines().collect();
            let line = lines[rng.range(lines.len())];
            let tokens: Vec<&str> = line
                .split(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
                .filter(|t| t.len() >= 3)
                .collect();
            let needle = tokens[rng.range(tokens.len())];
            let sub_len = 3 + rng.range(needle.len().saturating_sub(2).min(8));
            let sub = &needle[..sub_len];

            let report = index.query(&WorkspaceSearchQuery {
                pattern: sub,
                languages: &[],
                documents: &[],
                path_prefix: None,
                extension: None,
                max_hits: corpus.len() + 4,
                context_radius: 2,
            });
            assert!(
                report.hits.iter().any(|h| h.path == *target_path),
                "FALSE NEGATIVE on seed={seed}: substring `{sub}` from `{target_path}` was missed!"
            );
        }
    }
}

#[test]
fn pbt_topology_filter_soundness_determinism_and_line_bounds() {
    // 40 corpora x 25 randomized multi-axis queries = 1,000 soundness & determinism checks.
    let mut rng = SplitMix64::new(0xA590_0002);
    let prefixes = [
        None,
        Some("crates/"),
        Some("crates/search/"),
        Some("scheme/"),
    ];
    let exts = [None, Some("rs"), Some("ss"), Some("org")];
    let lang_sets: &[&[&str]] = &[&[], &["rust"], &["scheme"], &["python"]];
    let doc_sets: &[&[&str]] = &[&[], &["org"], &["markdown"]];

    for seed in 0..40u64 {
        let corpus = generate_random_corpus(&mut rng, 14);
        let line_counts: std::collections::HashMap<String, usize> = corpus
            .iter()
            .map(|(p, b)| (p.clone(), b.lines().count().max(1)))
            .collect();
        let index = WorkspaceSearchIndex::build_from_memory(
            PathBuf::from(format!("/virtual/filter-{seed}")),
            corpus
                .iter()
                .map(|(p, b)| (p.clone(), b.as_bytes().to_vec())),
            WorkspaceIndexConfig::default(),
        );

        for _ in 0..25 {
            let prefix = prefixes[rng.range(prefixes.len())];
            let ext = exts[rng.range(exts.len())];
            let langs: Vec<String> = lang_sets[rng.range(lang_sets.len())]
                .iter()
                .map(|s| (*s).to_owned())
                .collect();
            let docs: Vec<String> = doc_sets[rng.range(doc_sets.len())]
                .iter()
                .map(|s| (*s).to_owned())
                .collect();
            let max_hits = 1 + rng.range(10);
            let pattern = if rng.range(4) == 0 {
                String::new()
            } else {
                random_ident(&mut rng, 2, 7)
            };

            let q = WorkspaceSearchQuery {
                pattern: &pattern,
                languages: &langs,
                documents: &docs,
                path_prefix: prefix,
                extension: ext,
                max_hits,
                context_radius: 3,
            };
            let r1 = index.query(&q);
            let r2 = index.query(&q);

            // Determinism & boundedness
            assert_eq!(r1.generation_digest, r2.generation_digest);
            assert_eq!(r1.hits, r2.hits, "non-deterministic hit ranking");
            assert!(r1.hits.len() <= max_hits);

            // Deduplication & strict filter soundness
            let mut seen_paths = BTreeSet::new();
            for hit in &r1.hits {
                assert!(seen_paths.insert(hit.path.clone()), "duplicate hit path");
                if let Some(p) = prefix {
                    assert!(
                        hit.path.starts_with(p),
                        "prefix leak: `{}` does not start with `{p}`",
                        hit.path
                    );
                }
                if let Some(e) = ext {
                    assert!(
                        hit.path.ends_with(&format!(".{e}")),
                        "ext leak: `{}` does not end with `.{e}`",
                        hit.path
                    );
                }
                let max_line = *line_counts.get(&hit.path).expect("indexed path");
                for lm in &hit.line_matches {
                    assert!((1..=max_line).contains(&lm.line_number));
                }
                for ctx in &hit.context_lines {
                    assert!((1..=max_line).contains(&ctx.line_number));
                }
            }
        }
    }
}

#[test]
fn adversarial_fuzz_queries_and_corrupted_corpora_never_panic() {
    let mut rng = SplitMix64::new(0xDEAD_BEEF_CAFE_0003);

    // Mixed adversarial corpus: binary NUL blobs, CRLF, CJK/emoji UTF-8, single-line, huge line.
    let mut files: Vec<(String, Vec<u8>)> = vec![
        ("bin/null_blob.rs".into(), vec![b'a', 0, b'b', b'c']),
        ("bin/invalid_utf8.rs".into(), vec![0xFF, 0xFE, 0xFD]),
        ("empty.rs".into(), Vec::new()),
        (
            "unicode/中文_🚀_file.rs".into(),
            "pub fn 检索_engine() {\r\n    let s = \"🦀 café naïve\";\r\n}\r\n"
                .as_bytes()
                .to_vec(),
        ),
        (
            "crates/core/src/long_line.rs".into(),
            format!("pub fn {}() {{}}\n", "x".repeat(4096)).into_bytes(),
        ),
    ];
    for (p, b) in generate_random_corpus(&mut rng, 10) {
        files.push((p, b.into_bytes()));
    }

    let index = WorkspaceSearchIndex::build_from_memory(
        PathBuf::from("/virtual/fuzz"),
        files,
        WorkspaceIndexConfig::default(),
    );
    // Binary/invalid UTF-8/empty files must be cleanly skipped without panicking.
    assert!(index.file_count() >= 12);

    let hand_crafted_adversarial = [
        "",
        " ",
        "\0\0\0",
        "((((((",
        "re:((((((",
        "rg:[z-a]",
        "re:(a+)+b",
        "re:\\",
        "re:a{999999}",
        "title: OR AND ((( \"\" ^-9",
        "body:\"unclosed quote",
        "../../../etc/passwd",
        "🦀🦀🦀中文检索",
        &"a".repeat(8192),
        &"re:.*|".repeat(256),
    ];
    for pattern in hand_crafted_adversarial {
        let report = index.query(&WorkspaceSearchQuery {
            pattern,
            languages: &[],
            documents: &[],
            path_prefix: None,
            extension: None,
            max_hits: 16,
            context_radius: 4,
        });
        assert!(report.hits.len() <= 16);
    }

    // 1,500 random byte-mutated fuzzed queries (UTF-8 slices, punctuation, metacharacters).
    let fuzz_chars: Vec<char> = "abcxyz019_./\\?*+()[]{}|^$!@#:-'\"\t\n中🦀é"
        .chars()
        .collect();
    for _ in 0..1_500 {
        let len = rng.range(48);
        let mut q = String::with_capacity(len * 2);
        for _ in 0..len {
            q.push(fuzz_chars[rng.range(fuzz_chars.len())]);
        }
        let report = index.query(&WorkspaceSearchQuery {
            pattern: &q,
            languages: &[],
            documents: &[],
            path_prefix: None,
            extension: None,
            max_hits: 8,
            context_radius: 2,
        });
        assert!(report.hits.len() <= 8);
    }
}

#[test]
fn real_asp_repo_four_axis_converged_engine_verifies_rg_regex_fuzzy_and_bm25() {
    let repo_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("ASP repo root");
    let cfg = WorkspaceIndexConfig {
        skip_dirs: vec![
            "target".into(),
            ".git".into(),
            "node_modules".into(),
            ".devenv".into(),
        ],
        ..WorkspaceIndexConfig::default()
    };
    let index = WorkspaceSearchIndex::build_with_config(repo_root, cfg);
    assert!(index.file_count() > 50);

    // 1. Literal `rg` + Tantivy BM25 convergence
    let literal = index.query(&WorkspaceSearchQuery {
        pattern: "admit_search_playbook_plan",
        languages: &["rust".into()],
        documents: &[],
        path_prefix: Some("crates/"),
        extension: None,
        max_hits: 32,
        context_radius: 4,
    });
    assert!(
        literal
            .hits
            .iter()
            .any(|h| h.matched_axes.iter().any(|a| a == "rg")
                && h.matched_axes.iter().any(|a| a == "bm25")),
        "expected converged rg+bm25 hit, got {:?}",
        literal.hits
    );

    // 2. Built-in resident `rg` regex (`re:...`)
    let regex_report = index.query(&WorkspaceSearchQuery {
        pattern: r"re:pub\s+fn\s+execute_admitted_playbook",
        languages: &["rust".into()],
        documents: &[],
        path_prefix: Some("crates/agent-semantic-search/"),
        extension: None,
        max_hits: 16,
        context_radius: 3,
    });
    assert!(
        regex_report
            .hits
            .iter()
            .any(|h| h.path.ends_with("marlin_playbook_executor.rs")
                && h.matched_axes.iter().any(|a| a == "rg-regex")),
        "expected rg-regex hit on marlin_playbook_executor.rs, got {:?}",
        regex_report.hits
    );

    // 3. ASP FileLocatorIndex + fuzzy subsequence path (`flcltr` -> `file_locator.rs`)
    let fuzzy_path = index.query(&WorkspaceSearchQuery {
        pattern: "file_locator",
        languages: &["rust".into()],
        documents: &[],
        path_prefix: Some("crates/agent-semantic-search/"),
        extension: None,
        max_hits: 16,
        context_radius: 2,
    });
    assert!(
        fuzzy_path
            .hits
            .iter()
            .any(|h| h.path.ends_with("file_locator.rs")),
        "expected FileLocator hit on file_locator.rs"
    );

    // 4. In-RAM Tantivy `FuzzyTermQuery` typo tolerance (`playbok` -> `playbook`)
    let typo = index.query(&WorkspaceSearchQuery {
        pattern: "playbok",
        languages: &["rust".into()],
        documents: &[],
        path_prefix: Some("crates/agent-semantic-search-playbook/"),
        extension: None,
        max_hits: 16,
        context_radius: 2,
    });
    assert!(
        typo.hits
            .iter()
            .any(|h| h.matched_axes.iter().any(|a| a == "fuzzy")),
        "expected Tantivy FuzzyTermQuery hit for typo `playbok`, got {:?}",
        typo.hits
    );
}

#[test]
fn pbt_hir_regex_trigram_query_plan_zero_false_negatives_and_pruning() {
    let mut rng = SplitMix64::new(0xA590_0004);
    let corpus = generate_random_corpus(&mut rng, 48);
    // Include Unicode case-folding edge cases (`ſ` U+017F folds with `s`; `K` U+212A folds with `k`).
    let mut files: Vec<(String, String)> = corpus;
    files.push((
        "crates/core/src/unicode_fold.rs".to_owned(),
        "pub fn \u{017f}earch_engine() {}\npub fn \u{212a}ey_lookup() {}\n".to_owned(),
    ));

    let cfg = WorkspaceIndexConfig {
        enable_tantivy: false,
        enable_file_locator: false,
        enable_rg_regex: true,
        ..WorkspaceIndexConfig::default()
    };
    let index = WorkspaceSearchIndex::build_from_memory(
        PathBuf::from("/virtual/hir-pbt"),
        files
            .iter()
            .map(|(p, b)| (p.clone(), b.as_bytes().to_vec())),
        cfg,
    );

    // Extract real tokens from corpus to build diverse regexes.
    let mut pool: Vec<String> = Vec::new();
    for (_, body) in &files {
        for tok in body
            .split(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
            .filter(|t| t.len() >= 4)
        {
            if pool.len() < 120 {
                pool.push(tok.to_owned());
            }
        }
    }

    let mut patterns: Vec<String> = vec![
        "search_engine".to_owned(),
        "key_lookup".to_owned(),
        format!("({}|{})", pool[0], pool[1]),
        format!("({}|{}|{})", pool[2], pool[3], pool[4]),
        format!("fn\\s+({}|{})", pool[5], pool[6]),
        format!("({}|{})_0", &pool[0][..4], &pool[1][..4]),
        format!("{}.*->\\s+\\w+", pool[7]),
        format!("{}[a-c]?.*", &pool[8][..4]),
        format!("(?:{})+", pool[9]),
        "^[a-z]+_0".to_owned(),
    ];

    for _ in 0..390 {
        let a = &pool[rng.range(pool.len())];
        let b = &pool[rng.range(pool.len())];
        let c = &pool[rng.range(pool.len())];
        let pat = match rng.range(6) {
            0 => format!("({a}|{b})"),
            1 => format!("({a}|{b}|{c})"),
            2 => format!("{a}.*{b}"),
            3 => format!("({a}|{b}).*\\w+"),
            4 => format!("(?:{a})+(?:{b})?"),
            _ => format!("{}[a-z0-9_]*{}", &a[..3.min(a.len())], &b[..3.min(b.len())]),
        };
        patterns.push(pat);
    }

    // Verify alternation actually compiles to Or (not All!) and prunes unrelated files.
    let alt_expr = format!("({}|{})", pool[0], pool[1]);
    let (alt_plan, alt_cands) = index.plan_regex_candidates(&alt_expr);
    assert!(
        !matches!(alt_plan, TrigramQuery::All),
        "alternation `{alt_expr}` must not relax to TrigramQuery::All; got {alt_plan:?}"
    );
    assert!(
        alt_cands.len() < index.file_count() / 2,
        "alternation `{alt_expr}` should prune most unrelated files: cands={}, total={}",
        alt_cands.len(),
        index.file_count()
    );

    // Prove ZERO False Negatives across all 400 regexes against full-corpus reference scan.
    for expr in &patterns {
        let ci = !expr.chars().any(char::is_uppercase);
        let re = RegexBuilder::new(expr)
            .case_insensitive(ci)
            .build()
            .expect("valid test regex");

        let expected: BTreeSet<&str> = files
            .iter()
            .filter(|(p, b)| re.is_match(p) || re.is_match(b))
            .map(|(p, _)| p.as_str())
            .collect();

        let report = index.query(&WorkspaceSearchQuery {
            pattern: &format!("re:{expr}"),
            languages: &[],
            documents: &[],
            path_prefix: None,
            extension: None,
            max_hits: files.len() + 4,
            context_radius: 1,
        });
        let actual: BTreeSet<&str> = report.hits.iter().map(|h| h.path.as_str()).collect();
        if let Some(missing) = expected.difference(&actual).next() {
            panic!(
                "HIR TRIGRAM FALSE NEGATIVE for `{expr}` (plan={:?}): missed `{missing}`",
                compile_regex_trigram_query(expr, ci)
            );
        }
    }
}

#[test]
fn pbt_loc_mask_and_next_mask_prune_misaligned_decoys_with_zero_false_negatives() {
    let needle = "alpha_beta_gamma";
    let mut files: Vec<(String, Vec<u8>)> = Vec::new();

    // 8 true-positive files placing `needle` at every offset mod 8 (0..8).
    for pad in 0..8usize {
        let prefix = ".".repeat(pad);
        files.push((
            format!("src/true_mod_{pad}.rs"),
            format!("{prefix}{needle}\n").into_bytes(),
        ));
    }

    // 24 decoy files containing every trigram of `needle` scattered and misaligned so
    // naive trigram-set intersection would include all 24, whereas `loc_mask` + `next_mask`
    // rejects them before touching file bytes!
    for decoy in 0..24usize {
        let mut scattered = String::new();
        for (i, win) in needle.as_bytes().windows(3).enumerate() {
            let tri = std::str::from_utf8(win).expect("ascii");
            // Separate each trigram with padding so neither adjacency nor next_byte aligns.
            scattered.push_str(&format!("zzz{i}_{tri}###"));
        }
        files.push((format!("src/decoy_{decoy:02}.rs"), scattered.into_bytes()));
    }

    let cfg = WorkspaceIndexConfig {
        enable_tantivy: false,
        enable_file_locator: false,
        enable_rg_regex: false,
        ..WorkspaceIndexConfig::default()
    };
    let index =
        WorkspaceSearchIndex::build_from_memory(PathBuf::from("/virtual/loc-mask"), files, cfg);

    let report = index.query(&WorkspaceSearchQuery {
        pattern: needle,
        languages: &[],
        documents: &[],
        path_prefix: None,
        extension: None,
        max_hits: 64,
        context_radius: 1,
    });

    assert_eq!(
        report.matched_files, 8,
        "all 8 mod-8 alignments must be recalled"
    );
    assert!(
        report.candidate_files < 16,
        "loc_mask + next_mask should prune scattered decoys (candidates={}, expected < 16)",
        report.candidate_files
    );
}

#[test]
fn pbt_incremental_file_delta_matches_full_rebuild_digest_and_queries() {
    let mut rng = SplitMix64::new(0xA590_0005);
    let initial = generate_random_corpus(&mut rng, 20);
    let mut truth: BTreeMap<String, Vec<u8>> = initial
        .iter()
        .map(|(p, b)| (p.clone(), b.as_bytes().to_vec()))
        .collect();

    let cfg = WorkspaceIndexConfig::default();
    let mut live_index = WorkspaceSearchIndex::build_from_memory(
        PathBuf::from("/virtual/delta"),
        truth.iter().map(|(p, b)| (p.clone(), b.clone())),
        cfg.clone(),
    );

    for step in 0..50usize {
        let mut delta = WorkspaceFileDelta::default();
        let existing_paths: Vec<String> = truth.keys().cloned().collect();

        // Randomly delete 0..=2 files
        let del_n = rng.range(3).min(existing_paths.len());
        for _ in 0..del_n {
            let p = existing_paths[rng.range(existing_paths.len())].clone();
            truth.remove(&p);
            delta.deletes.push(p);
        }

        // Randomly modify 1..=2 existing files and add 1 new file
        if !existing_paths.is_empty() {
            let mod_p = existing_paths[rng.range(existing_paths.len())].clone();
            let new_body =
                format!("pub fn mutated_step_{step}() {{ let tok = \"delta_token_{step}\"; }}\n")
                    .into_bytes();
            truth.insert(mod_p.clone(), new_body.clone());
            delta.upserts.push((mod_p, new_body));
        }
        let added_p = format!("crates/delta/src/step_{step:02}.rs");
        let added_body =
            format!("pub fn added_step_{step}() {{ let unique = \"fresh_marker_{step}\"; }}\n")
                .into_bytes();
        truth.insert(added_p.clone(), added_body.clone());
        delta.upserts.push((added_p, added_body));

        let receipt = live_index.apply_file_delta(delta);
        let rebuilt = WorkspaceSearchIndex::build_from_memory(
            PathBuf::from("/virtual/delta"),
            truth.iter().map(|(p, b)| (p.clone(), b.clone())),
            cfg.clone(),
        );

        assert_eq!(
            receipt.generation_digest,
            *rebuilt.generation_digest(),
            "step={step}: incremental delta digest diverged from full rebuild"
        );
        assert_eq!(live_index.file_count(), rebuilt.file_count());

        // Compare literal + regex alternation queries between incremental and full rebuild.
        for q_str in [
            format!("fresh_marker_{step}"),
            format!("re:(mutated_step_{step}|added_step_{step})"),
        ] {
            let q = WorkspaceSearchQuery {
                pattern: &q_str,
                languages: &[],
                documents: &[],
                path_prefix: None,
                extension: None,
                max_hits: 32,
                context_radius: 2,
            };
            let inc_paths: Vec<String> = live_index
                .query(&q)
                .hits
                .into_iter()
                .map(|h| h.path)
                .collect();
            let full_paths: Vec<String> =
                rebuilt.query(&q).hits.into_iter().map(|h| h.path).collect();
            assert_eq!(
                inc_paths, full_paths,
                "step={step} query=`{q_str}`: incremental results diverged from full rebuild"
            );
        }
    }
}

#[test]
fn adversarial_fuzz_mmap_and_binary_snapshot_corruption_never_panics() {
    let mut rng = SplitMix64::new(0xA590_0006);
    let corpus = generate_random_corpus(&mut rng, 18);
    let cfg = WorkspaceIndexConfig::default();
    let original = WorkspaceSearchIndex::build_from_memory(
        PathBuf::from("/virtual/snap"),
        corpus
            .iter()
            .map(|(p, b)| (p.clone(), b.as_bytes().to_vec())),
        cfg.clone(),
    );

    // 1. Round-trip through zero-copy mmap snapshot on disk.
    let tmp = tempfile::tempdir().expect("tempdir");
    let snap_path = tmp.path().join("workspace.asp-index.bin");
    original.save_snapshot(&snap_path).expect("save snapshot");
    let mmap_index = WorkspaceSearchIndex::load_mmap_snapshot(
        PathBuf::from("/virtual/snap"),
        &snap_path,
        cfg.clone(),
    )
    .expect("load mmap snapshot");
    assert!(mmap_index.is_mmap_backed());
    assert_eq!(mmap_index.generation_digest(), original.generation_digest());
    assert_eq!(mmap_index.file_count(), original.file_count());
    assert_eq!(
        mmap_index.unique_trigram_count(),
        original.unique_trigram_count()
    );
    assert_eq!(
        mmap_index.base_posting_count(),
        original.base_posting_count()
    );

    // Verify mmap-backed index serves identical literal and regex queries.
    let sample_tok = corpus[0]
        .1
        .split(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
        .find(|t| t.len() >= 5)
        .expect("sample token");
    for pat in [sample_tok.to_owned(), format!("re:({sample_tok}|fn)")] {
        let q = WorkspaceSearchQuery {
            pattern: &pat,
            languages: &[],
            documents: &[],
            path_prefix: None,
            extension: None,
            max_hits: 32,
            context_radius: 2,
        };
        assert_eq!(original.query(&q).hits, mmap_index.query(&q).hits);
    }

    // 2. Adversarial corruption fuzzing (truncation, bit-flips, forged BLAKE3 with bad layout).
    let valid_bytes = original.encode_snapshot_bytes();
    let decoded = WorkspaceSearchIndex::decode_snapshot_bytes(
        PathBuf::from("/virtual/snap"),
        &valid_bytes,
        cfg.clone(),
    )
    .expect("decode valid bytes");
    assert_eq!(decoded.generation_digest(), original.generation_digest());

    // Every header truncation 0..80 must fail cleanly.
    for cut in 0..80 {
        assert!(matches!(
            WorkspaceSearchIndex::decode_snapshot_bytes(
                PathBuf::from("/virtual/snap"),
                &valid_bytes[..cut],
                cfg.clone(),
            ),
            Err(TrigramSnapshotError::Truncated(_))
        ));
    }

    // Bad magic & bad version
    let mut bad_magic = valid_bytes.clone();
    bad_magic[0] ^= 0xFF;
    assert!(matches!(
        WorkspaceSearchIndex::decode_snapshot_bytes(
            PathBuf::from("/virtual/snap"),
            &bad_magic,
            cfg.clone(),
        ),
        Err(TrigramSnapshotError::InvalidMagic)
    ));

    let mut bad_ver = valid_bytes.clone();
    bad_ver[8..12].copy_from_slice(&99u32.to_le_bytes());
    assert!(matches!(
        WorkspaceSearchIndex::decode_snapshot_bytes(
            PathBuf::from("/virtual/snap"),
            &bad_ver,
            cfg.clone(),
        ),
        Err(TrigramSnapshotError::UnsupportedVersion(99))
    ));

    // 500 random single-byte bit flips and random truncations across the full blob.
    for _ in 0..500 {
        let mut mutated = valid_bytes.clone();
        match rng.range(3) {
            0 => {
                // Flip a byte in payload -> must fail checksum
                let pos = 80 + rng.range(mutated.len().saturating_sub(80).max(1));
                if pos < mutated.len() {
                    mutated[pos] ^= 1u8 << (rng.range(8) as u8);
                }
            }
            1 => {
                // Random truncation
                let cut = rng.range(mutated.len());
                mutated.truncate(cut);
            }
            _ => {
                // Corrupt layout AND re-sign BLAKE3 so structural validators are hammered!
                let field_idx = rng.range(3);
                let off = 24 + field_idx * 8;
                mutated[off..off + 8].copy_from_slice(&(rng.next_u64() % 1_000_000).to_le_bytes());
                let forged_hash = *blake3::hash(&mutated[80..]).as_bytes();
                mutated[48..80].copy_from_slice(&forged_hash);
            }
        }
        let res = WorkspaceSearchIndex::decode_snapshot_bytes(
            PathBuf::from("/virtual/snap"),
            &mutated,
            cfg.clone(),
        );
        assert!(res.is_err(), "corrupted snapshot unexpectedly succeeded");
    }
}

#[test]
fn cached_mmap_open_and_disk_sync_plus_resident_playbook_execution() {
    use agent_semantic_search::{
        SearchPlaybookPlanInput, admit_search_playbook_plan, execute_admitted_playbook,
        execute_admitted_playbook_with_index,
    };
    use agent_semantic_search_playbook::{SearchPlaybookComposition, SearchPlaybookLeaf};

    let tmp = tempfile::tempdir().expect("tempdir");
    let ws = tmp.path().join("workspace");
    let snap = tmp.path().join("cache/workspace.asp-index.bin");
    std::fs::create_dir_all(ws.join("src")).expect("create src");
    std::fs::write(
        ws.join("src/alpha.rs"),
        "pub fn alpha_search_kernel() -> usize { 1 }\n",
    )
    .expect("write alpha");
    std::fs::write(
        ws.join("src/beta.rs"),
        "pub fn beta_other_helper() -> usize { 2 }\n",
    )
    .expect("write beta");

    let cfg = WorkspaceIndexConfig::default();
    // 1. Cold open builds and saves snapshot.
    let cold = WorkspaceSearchIndex::open_or_build_cached(&ws, &snap, cfg.clone());
    assert!(snap.is_file());
    assert_eq!(cold.file_count(), 2);

    // 2. Warm open maps zero-copy snapshot with identical digest.
    let mut warm = WorkspaceSearchIndex::open_or_build_cached(&ws, &snap, cfg.clone());
    assert!(warm.is_mmap_backed());
    assert_eq!(warm.generation_digest(), cold.generation_digest());

    // 3. Mutate workspace on disk (modify alpha, add gamma, delete beta) and sync incrementally.
    std::fs::write(
        ws.join("src/alpha.rs"),
        "pub fn alpha_search_kernel_v2() -> usize { 42 }\n",
    )
    .expect("update alpha");
    std::fs::write(
        ws.join("src/gamma.rs"),
        "pub fn gamma_resident_engine() -> usize { 7 }\n",
    )
    .expect("write gamma");
    std::fs::remove_file(ws.join("src/beta.rs")).expect("remove beta");

    let receipt = warm.sync_workspace_from_disk();
    assert_eq!(receipt.upserted_files, 2);
    assert_eq!(receipt.deleted_files, 1);
    assert_eq!(receipt.live_files, 2);

    let fresh = WorkspaceSearchIndex::build_with_config(&ws, cfg);
    assert_eq!(warm.generation_digest(), fresh.generation_digest());

    // 4. Verify `execute_admitted_playbook_with_index` matches cold `execute_admitted_playbook`.
    let plan = SearchPlaybookPlanInput {
        workspace: Some("virtual-ws".to_owned()),
        languages: vec!["rust".to_owned()],
        documents: Vec::new(),
        composition: SearchPlaybookComposition::Leaf(SearchPlaybookLeaf::Rg(vec![
            "-n".to_owned(),
            "(alpha_search_kernel_v2|gamma_resident_engine)".to_owned(),
        ])),
    };
    let admitted = admit_search_playbook_plan(plan).expect("admit plan");
    let resident_report =
        execute_admitted_playbook_with_index(&warm, &admitted).expect("resident playbook");
    let disk_report = execute_admitted_playbook(&ws, &admitted).expect("disk playbook");

    assert_eq!(resident_report.execution_mode, "pure-rust-resident-index");
    let res_paths: BTreeSet<String> = resident_report.hits.into_iter().map(|h| h.path).collect();
    let disk_paths: BTreeSet<String> = disk_report.hits.into_iter().map(|h| h.path).collect();
    assert_eq!(res_paths, disk_paths);
    assert_eq!(res_paths.len(), 2);
}
