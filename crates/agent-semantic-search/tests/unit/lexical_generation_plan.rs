// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use agent_semantic_search::AdmittedLexicalOwner;
use agent_semantic_search::LexicalOwnerFact;
use agent_semantic_search::LexicalShardArtifact;
use agent_semantic_search::LexicalShardDisposition;
use agent_semantic_search::plan_lexical_generation;

fn digest(value: &str) -> String {
    format!("blake3-256:{}", blake3::hash(value.as_bytes()).to_hex())
}

fn facts<'a>(owners: &'a [(&'a str, String, Vec<String>)]) -> Vec<LexicalOwnerFact<'a>> {
    owners
        .iter()
        .map(|(path, content_digest, keys)| LexicalOwnerFact {
            owner_path: path,
            content_digest,
            query_keys: keys,
        })
        .collect()
}

#[test]
fn bootstrap_builds_every_content_addressed_lexical_shard() {
    let owners = vec![
        ("src/a.rs", digest("a"), vec!["alpha".to_owned()]),
        ("src/b.rs", digest("b"), vec!["beta".to_owned()]),
    ];
    let plan = plan_lexical_generation(
        &digest("analyzer"),
        ["src/b.rs", "src/a.rs"],
        owners
            .iter()
            .map(|(path, content_digest, _)| AdmittedLexicalOwner {
                owner_path: path,
                content_digest,
            }),
        facts(&owners),
        [],
    )
    .expect("bootstrap lexical plan");
    assert_eq!(plan.reused_shard_count(), 0);
    assert_eq!(plan.rebuilt_shard_count(), 2);
    assert!(plan.retired_artifact_digests.is_empty());
}

#[test]
fn delta_reuses_unchanged_shard_and_rebuilds_only_changed_owner() {
    let analyzer = digest("analyzer");
    let base_owners = vec![
        ("src/a.rs", digest("a"), vec!["alpha".to_owned()]),
        ("src/b.rs", digest("b"), vec!["beta".to_owned()]),
    ];
    let base = plan_lexical_generation(
        &analyzer,
        ["src/a.rs", "src/b.rs"],
        base_owners
            .iter()
            .map(|(path, content_digest, _)| AdmittedLexicalOwner {
                owner_path: path,
                content_digest,
            }),
        facts(&base_owners),
        [],
    )
    .expect("base plan");
    let prior = base
        .entries
        .iter()
        .map(|entry| LexicalShardArtifact {
            shard_key: entry.shard_key.clone(),
            artifact_digest: digest(&entry.shard_key),
            owner_path: entry.owner_path.clone(),
            content_digest: entry.content_digest.clone(),
            analyzer_digest: analyzer.clone(),
        })
        .collect::<Vec<_>>();
    let changed = vec![
        ("src/a.rs", digest("a"), vec!["alpha".to_owned()]),
        ("src/b.rs", digest("b-next"), vec!["beta-next".to_owned()]),
    ];
    let plan = plan_lexical_generation(
        &analyzer,
        ["src/a.rs", "src/b.rs"],
        changed
            .iter()
            .map(|(path, content_digest, _)| AdmittedLexicalOwner {
                owner_path: path,
                content_digest,
            }),
        facts(&changed),
        prior,
    )
    .expect("delta plan");
    assert_eq!(plan.reused_shard_count(), 1);
    assert_eq!(plan.rebuilt_shard_count(), 1);
    assert_eq!(plan.retired_artifact_digests.len(), 1);
    assert_eq!(plan.entries[0].disposition, LexicalShardDisposition::Reuse);
    assert_eq!(
        plan.entries[1].disposition,
        LexicalShardDisposition::Rebuild
    );
}

#[test]
fn missing_inventory_or_lexical_fact_fails_closed() {
    let owner = ("src/a.rs", digest("a"), vec!["alpha".to_owned()]);
    let admitted = || {
        [AdmittedLexicalOwner {
            owner_path: owner.0,
            content_digest: &owner.1,
        }]
    };
    assert!(
        plan_lexical_generation(
            &digest("analyzer"),
            [],
            admitted(),
            facts(&[owner.clone()]),
            []
        )
        .expect_err("missing fd inventory must fail")
        .contains("absent from fd inventory")
    );
    assert!(
        plan_lexical_generation(&digest("analyzer"), [owner.0], admitted(), [], [],)
            .expect_err("missing lexical fact must fail")
            .contains("do not cover")
    );
}

#[test]
fn lexical_content_drift_cannot_reuse_a_prior_tantivy_shard() {
    let admitted_digest = digest("current");
    let stale_digest = digest("stale");
    let keys = vec!["owner".to_owned()];
    let error = plan_lexical_generation(
        &digest("analyzer"),
        ["src/a.rs"],
        [AdmittedLexicalOwner {
            owner_path: "src/a.rs",
            content_digest: &admitted_digest,
        }],
        [LexicalOwnerFact {
            owner_path: "src/a.rs",
            content_digest: &stale_digest,
            query_keys: &keys,
        }],
        [],
    )
    .expect_err("stale lexical fact must fail");
    assert!(error.contains("content digest drift"));
}
