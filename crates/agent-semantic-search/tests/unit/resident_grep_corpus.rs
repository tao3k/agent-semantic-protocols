// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use std::io::Write;
use std::sync::Arc;

use super::ResidentGrepCorpusOwner;
use super::ResidentGrepMappedCorpusOwner;
use super::build_resident_grep_corpus;
use super::open_mapped_resident_grep_corpus;
use super::owner_for_corpus_line;

fn digest(bytes: &[u8]) -> String {
    format!("blake3-256:{}", blake3::hash(bytes).to_hex())
}

#[test]
fn corpus_is_sorted_content_bound_and_maps_lines_to_owners() {
    let a = b"fn alpha() {}\n";
    let b = b"fn beta() {}\nfn gamma() {}\n";
    let artifact = build_resident_grep_corpus(
        &digest(b"generation"),
        [
            ResidentGrepCorpusOwner {
                owner_path: "src/b.rs",
                content_digest: &digest(b),
                bytes: b,
            },
            ResidentGrepCorpusOwner {
                owner_path: "src/a.rs",
                content_digest: &digest(a),
                bytes: a,
            },
        ],
    )
    .expect("resident GREP corpus");
    let expected_byte_count = a.len() + b.len();
    assert_eq!(artifact.corpus_byte_count(), expected_byte_count);
    assert_eq!(artifact.corpus_heap_bytes(), expected_byte_count);
    assert_eq!(
        owner_for_corpus_line(&artifact.owner_spans, 1)
            .unwrap()
            .owner_path,
        "src/a.rs"
    );
    assert_eq!(
        owner_for_corpus_line(&artifact.owner_spans, 3)
            .unwrap()
            .owner_path,
        "src/b.rs"
    );
}

#[test]
fn corpus_rejects_content_drift_before_publication() {
    assert!(
        build_resident_grep_corpus(
            &digest(b"generation"),
            [ResidentGrepCorpusOwner {
                owner_path: "src/lib.rs",
                content_digest: &digest(b"other"),
                bytes: b"fn actual() {}\n",
            }],
        )
        .expect_err("content drift")
        .contains("content digest drift")
    );
}

#[test]
fn mapped_corpus_reads_owner_ranges_without_workspace_heap_copy() {
    let a = b"fn alpha() {}\n";
    let b = b"fn beta() {}\n";
    let mut file = tempfile::tempfile().unwrap();
    file.write_all(a).unwrap();
    file.write_all(b).unwrap();
    file.flush().unwrap();
    let mapping = Arc::new(unsafe { memmap2::MmapOptions::new().map(&file).unwrap() });
    let artifact = open_mapped_resident_grep_corpus(
        &digest(b"generation"),
        Arc::clone(&mapping),
        [
            ResidentGrepMappedCorpusOwner {
                owner_path: "src/a.rs".to_owned(),
                content_digest: digest(a),
                byte_range: 0..a.len(),
            },
            ResidentGrepMappedCorpusOwner {
                owner_path: "src/b.rs".to_owned(),
                content_digest: digest(b),
                byte_range: a.len()..a.len() + b.len(),
            },
        ],
    )
    .unwrap();
    let owned = build_resident_grep_corpus(
        &digest(b"generation"),
        [
            ResidentGrepCorpusOwner {
                owner_path: "src/a.rs",
                content_digest: &digest(a),
                bytes: a,
            },
            ResidentGrepCorpusOwner {
                owner_path: "src/b.rs",
                content_digest: &digest(b),
                bytes: b,
            },
        ],
    )
    .unwrap();
    assert_eq!(artifact.corpus_heap_bytes(), 0);
    assert_eq!(artifact.corpus_byte_count(), a.len() + b.len());
    assert_eq!(artifact.receipt, owned.receipt);
    assert_eq!(artifact.owner_bytes("src/b.rs"), Some(b.as_slice()));
}
