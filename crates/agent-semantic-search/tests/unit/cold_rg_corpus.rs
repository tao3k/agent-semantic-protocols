// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use super::ColdRgCorpusOwner;
use super::build_cold_rg_corpus;
use super::owner_for_corpus_line;

fn digest(bytes: &[u8]) -> String {
    format!("blake3-256:{}", blake3::hash(bytes).to_hex())
}

#[test]
fn corpus_is_sorted_content_bound_and_maps_lines_to_owners() {
    let a = b"fn alpha() {}\n";
    let b = b"fn beta() {}\nfn gamma() {}\n";
    let artifact = build_cold_rg_corpus(
        &digest(b"generation"),
        [
            ColdRgCorpusOwner {
                owner_path: "src/b.rs",
                content_digest: &digest(b),
                bytes: b,
            },
            ColdRgCorpusOwner {
                owner_path: "src/a.rs",
                content_digest: &digest(a),
                bytes: a,
            },
        ],
    )
    .expect("cold rg corpus");
    assert_eq!(
        artifact.bytes,
        b"fn alpha() {}\nfn beta() {}\nfn gamma() {}\n"
    );
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
        build_cold_rg_corpus(
            &digest(b"generation"),
            [ColdRgCorpusOwner {
                owner_path: "src/lib.rs",
                content_digest: &digest(b"other"),
                bytes: b"fn actual() {}\n",
            }],
        )
        .expect_err("content drift")
        .contains("content digest drift")
    );
}
