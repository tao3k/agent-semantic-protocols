// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use agent_semantic_symbol_index::{
    SymbolSkeletonIndexV1, SymbolSkeletonOwnerV1, SymbolSkeletonRecordV1,
};

#[test]
fn indexes_paths_and_symbols_across_languages_without_body_input() {
    let index = SymbolSkeletonIndexV1::build([
        owner(
            "src/lib.rs",
            "rust",
            "rust://src/lib.rs#item/function/commit",
            "commit",
        ),
        owner(
            "src/main.py",
            "python",
            "python://src/main.py#item/function/commit",
            "commit",
        ),
    ])
    .expect("build language-neutral symbol index");

    let hits = index.query("commit", 8);
    assert_eq!(hits.len(), 2);
    assert_eq!(hits[0].language_id.as_deref(), Some("rust"));
    assert_eq!(hits[1].language_id.as_deref(), Some("python"));
    assert_eq!(index.query("lib.rs", 8)[0].structural_selector, None);
    assert_eq!(index.symbol_count(), 2);
}

#[test]
fn owner_content_digest_changes_symbol_shard_identity() {
    let first = SymbolSkeletonIndexV1::build([owner_with_digest('1')]).unwrap();
    let second = SymbolSkeletonIndexV1::build([owner_with_digest('2')]).unwrap();
    assert_ne!(
        first.owner_shard_digest("src/lib.rs"),
        second.owner_shard_digest("src/lib.rs")
    );
}

#[test]
fn shared_analyzer_retains_complete_snake_and_camel_symbol_keys() {
    assert_eq!(
        agent_semantic_symbol_index::symbol_skeleton_terms("HTTPRuntime_owner-id"),
        [
            "http",
            "httpruntime",
            "httpruntime_owner-id",
            "id",
            "owner",
            "runtime"
        ]
    );
    assert!(
        agent_semantic_symbol_index::symbol_skeleton_navigation_keys(
            "crates/runtime_server/src/HTTPRouter.rs"
        )
        .contains(&"crates/runtime_server/src/httprouter.rs".to_owned())
    );
}

#[test]
fn shard_identity_is_independent_of_parser_key_order_and_rejects_untyped_digests() {
    let mut first_owner = owner(
        "src/lib.rs",
        "rust",
        "rust://src/lib.rs#item/function/commit",
        "commit",
    );
    first_owner.symbols[0].keys = vec!["write".to_owned(), "commit".to_owned()];
    let mut second_owner = first_owner.clone();
    second_owner.symbols[0].keys.reverse();

    let first = SymbolSkeletonIndexV1::build([first_owner]).unwrap();
    let second = SymbolSkeletonIndexV1::build([second_owner]).unwrap();
    assert_eq!(
        first.owner_shard_digest("src/lib.rs"),
        second.owner_shard_digest("src/lib.rs")
    );

    let mut malformed = owner_with_digest('1');
    malformed.owner_content_digest = "blake3-256:ABC".to_owned();
    assert!(SymbolSkeletonIndexV1::build([malformed]).is_err());
}

#[test]
fn warm_lookup_over_4096_cross_language_owners_is_sub_millisecond_p99() {
    let languages = ["rust", "typescript", "python", "julia", "scheme", "org"];
    let index = SymbolSkeletonIndexV1::build((0..4_096).map(|owner_id| {
        owner(
            &format!("src/{owner_id}/module.rs"),
            languages[owner_id % languages.len()],
            &format!("lang://src/{owner_id}/module.rs#item/function/symbol_{owner_id}"),
            &format!("symbol_{owner_id}"),
        )
    }))
    .expect("build representative symbol index");
    let mut elapsed = Vec::with_capacity(512);
    for _ in 0..512 {
        let started = std::time::Instant::now();
        let hits = index.query("symbol_2048", 16);
        elapsed.push(started.elapsed());
        assert_eq!(hits.len(), 1);
    }
    elapsed.sort_unstable();
    let p99 = elapsed[elapsed.len() * 99 / 100];
    eprintln!(
        "[symbol-skeleton-index] owners=4096 symbols=4096 samples=512 p99Nanos={} bodyInputs=0",
        p99.as_nanos()
    );
    assert!(p99 < std::time::Duration::from_millis(1), "p99={p99:?}");
}

fn owner(path: &str, language: &str, selector: &str, symbol: &str) -> SymbolSkeletonOwnerV1 {
    SymbolSkeletonOwnerV1 {
        owner_path: path.to_owned(),
        owner_content_digest: format!("blake3-256:{}", "1".repeat(64)),
        language_id: Some(language.to_owned()),
        symbols: vec![SymbolSkeletonRecordV1 {
            structural_selector: selector.to_owned(),
            keys: vec![symbol.to_owned()],
        }],
    }
}

fn owner_with_digest(digit: char) -> SymbolSkeletonOwnerV1 {
    let mut owner = owner(
        "src/lib.rs",
        "rust",
        "rust://src/lib.rs#item/function/commit",
        "commit",
    );
    owner.owner_content_digest = format!("blake3-256:{}", digit.to_string().repeat(64));
    owner
}
