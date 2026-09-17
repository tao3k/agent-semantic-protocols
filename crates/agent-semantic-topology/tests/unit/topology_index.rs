// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use agent_semantic_topology::{
    TopologyHitV1, TopologyIndexV1, TopologyNodeKindV1, TopologyNodeV1, TopologyOwnerV1,
    ranked_text_topology_selector_carrier, topology_feature_terms, topology_navigation_features,
};

#[test]
fn indexes_repository_paths_symbols_and_document_headings_as_topology() {
    let index = TopologyIndexV1::build([
        owner(
            "src/runtime/lib.rs",
            "rust://src/runtime/lib.rs#item/function/commit",
            ["commit"],
        ),
        owner(
            "docs/design.org",
            "org://docs/design.org#item/heading/Topology%20Contract",
            ["Topology Contract"],
        ),
        owner(
            "docs/readme.md",
            "markdown://docs/readme.md#item/heading/Architecture",
            ["Architecture"],
        ),
    ])
    .expect("build language-neutral topology index");

    assert_eq!(
        index.query("commit", 8)[0].kind,
        parser_kind("rust", "function")
    );
    assert_eq!(
        index.query("Topology Contract", 8)[0].kind,
        parser_kind("org", "heading")
    );
    assert_eq!(
        index.query("Architecture", 8)[0].kind,
        parser_kind("markdown", "heading")
    );
    assert!(
        index
            .query("docs", 8)
            .iter()
            .any(|hit| hit.kind == TopologyNodeKindV1::Directory)
    );
    assert!(
        index
            .query("readme.md", 8)
            .iter()
            .any(|hit| hit.kind == TopologyNodeKindV1::Owner)
    );
    assert_eq!(index.parser_native_node_count(), 3);
    assert!(index.node_count() > index.parser_native_node_count());
}

#[test]
fn owner_content_digest_changes_topology_shard_identity() {
    let first = TopologyIndexV1::build([owner_with_digest('1')]).unwrap();
    let second = TopologyIndexV1::build([owner_with_digest('2')]).unwrap();
    assert_ne!(
        first.owner_shard_digest("src/lib.rs"),
        second.owner_shard_digest("src/lib.rs")
    );
}

#[test]
fn exact_byte_match_resolves_smallest_content_bound_anchor() {
    let mut outer = TopologyNodeV1::from_selector_with_anchor(
        "rust://src/lib.rs#item/module/runtime",
        vec!["runtime".to_owned()],
        0,
        80,
    )
    .unwrap();
    let inner = TopologyNodeV1::from_selector_with_anchor(
        "rust://src/lib.rs#item/function/commit",
        vec!["commit".to_owned()],
        12,
        40,
    )
    .unwrap();
    outer.features.push("module".to_owned());
    let digest = format!("blake3-256:{}", "1".repeat(64));
    let index = TopologyIndexV1::build([TopologyOwnerV1 {
        owner_path: "src/lib.rs".to_owned(),
        owner_content_digest: digest.clone(),
        nodes: vec![outer, inner],
    }])
    .unwrap();

    let hit = index
        .smallest_enclosing_anchor("src/lib.rs", &digest, 20, 26)
        .unwrap()
        .expect("current anchor");
    assert_eq!(
        hit.structural_selector,
        "rust://src/lib.rs#item/function/commit"
    );
    assert!(
        index
            .smallest_enclosing_anchor(
                "src/lib.rs",
                &format!("blake3-256:{}", "2".repeat(64)),
                20,
                26,
            )
            .is_err()
    );
}

#[test]
fn resident_anchor_lookup_scenario_is_sub_millisecond_without_owner_bodies() {
    let benchmark = toml::from_str::<toml::Value>(include_str!(
        "../scenarios/topology_index_v1/benchmark.toml"
    ))
    .expect("Topology Index benchmark receipt");
    let anchor_count = benchmark["work_metrics"]["parser_anchor_count"]
        .as_integer()
        .and_then(|value| usize::try_from(value).ok())
        .expect("anchor count");
    let sample_count = benchmark["work_metrics"]["sample_count"]
        .as_integer()
        .and_then(|value| usize::try_from(value).ok())
        .expect("sample count");
    let maximum_p99_nanos = benchmark["qualification"]["maximum_anchor_lookup_p99_nanos"]
        .as_integer()
        .and_then(|value| u128::try_from(value).ok())
        .expect("anchor p99 gate");
    let digest = format!("blake3-256:{}", "1".repeat(64));
    let nodes = (0..anchor_count)
        .map(|index| {
            TopologyNodeV1::from_selector_with_anchor(
                format!("rust://src/lib.rs#item/function/node_{index}"),
                vec![format!("node_{index}")],
                index * 16,
                index * 16 + 12,
            )
            .unwrap()
        })
        .collect();
    let index = TopologyIndexV1::build([TopologyOwnerV1 {
        owner_path: "src/lib.rs".to_owned(),
        owner_content_digest: digest.clone(),
        nodes,
    }])
    .unwrap();
    let byte_start = (anchor_count - 1) * 16 + 2;
    let mut observations = Vec::with_capacity(sample_count);
    for _ in 0..sample_count {
        let started = std::time::Instant::now();
        let hit = index
            .smallest_enclosing_anchor("src/lib.rs", &digest, byte_start, byte_start + 2)
            .unwrap();
        observations.push(started.elapsed().as_nanos());
        assert!(hit.is_some());
    }
    observations.sort_unstable();
    let p99 = observations[observations.len() * 99 / 100];
    eprintln!(
        "[topology-anchor] anchors={anchor_count} samples={sample_count} p99Nanos={p99} ownerSnapshotCopies=0 providerCalls=0"
    );
    assert!(p99 < maximum_p99_nanos, "anchor lookup p99={p99}ns");
}

#[test]
fn provider_node_kind_must_match_the_canonical_selector() {
    let mut owner = owner(
        "docs/design.org",
        "org://docs/design.org#item/heading/Topology",
        ["Topology"],
    );
    owner.nodes[0].kind = parser_kind("rust", "function");
    assert!(TopologyIndexV1::build([owner]).is_err());
}

#[test]
fn shared_feature_analyzer_retains_complete_snake_camel_and_path_keys() {
    assert_eq!(
        topology_feature_terms("HTTPRuntime_owner-id"),
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
        topology_navigation_features("crates/runtime_server/src/HTTPRouter.rs")
            .contains(&"crates/runtime_server/src/httprouter.rs".to_owned())
    );
}

#[test]
fn shard_identity_is_feature_order_independent_and_rejects_untyped_digests() {
    let mut first_owner = owner(
        "src/lib.rs",
        "rust://src/lib.rs#item/function/commit",
        ["write", "commit"],
    );
    let mut second_owner = first_owner.clone();
    second_owner.nodes[0].features.reverse();

    let first = TopologyIndexV1::build([first_owner.clone()]).unwrap();
    let second = TopologyIndexV1::build([second_owner]).unwrap();
    assert_eq!(
        first.owner_shard_digest("src/lib.rs"),
        second.owner_shard_digest("src/lib.rs")
    );

    first_owner.owner_content_digest = "blake3-256:ABC".to_owned();
    assert!(TopologyIndexV1::build([first_owner]).is_err());
}

#[test]
fn warm_lookup_over_4096_cross_language_nodes_is_sub_millisecond_p99() {
    let scenario =
        toml::from_str::<toml::Value>(include_str!("../scenarios/topology_index_v1/scenario.toml"))
            .expect("Topology Index Scenario");
    let benchmark = toml::from_str::<toml::Value>(include_str!(
        "../scenarios/topology_index_v1/benchmark.toml"
    ))
    .expect("Topology Index benchmark receipt");
    assert_eq!(
        scenario["gate"]["symbol_as_index_identity_count"].as_integer(),
        Some(0)
    );
    let owner_count = benchmark["work_metrics"]["owner_count"]
        .as_integer()
        .and_then(|value| usize::try_from(value).ok())
        .expect("owner count");
    let sample_count = benchmark["work_metrics"]["sample_count"]
        .as_integer()
        .and_then(|value| usize::try_from(value).ok())
        .expect("sample count");
    let maximum_p99_nanos = benchmark["qualification"]["maximum_p99_nanos"]
        .as_integer()
        .and_then(|value| u128::try_from(value).ok())
        .expect("maximum p99");
    let languages = ["rust", "typescript", "python", "julia", "scheme", "org"];
    let index = TopologyIndexV1::build((0..owner_count).map(|owner_id| {
        let language = languages[owner_id % languages.len()];
        let kind = if language == "org" {
            "heading"
        } else {
            "function"
        };
        owner(
            &format!("src/{owner_id}/module.txt"),
            &format!("{language}://src/{owner_id}/module.txt#item/{kind}/node_{owner_id}"),
            [format!("node_{owner_id}")],
        )
    }))
    .expect("build representative topology index");
    let mut elapsed = Vec::with_capacity(sample_count);
    for _ in 0..sample_count {
        let started = std::time::Instant::now();
        let hits = index.query("node_2048", 16);
        elapsed.push(started.elapsed());
        assert_eq!(hits.len(), 1);
    }
    elapsed.sort_unstable();
    let p99 = elapsed[elapsed.len() * 99 / 100];
    eprintln!(
        "[topology-index] owners={owner_count} parserNodes={owner_count} samples={sample_count} p99Nanos={} bodyInputs=0",
        p99.as_nanos()
    );
    assert!(p99.as_nanos() < maximum_p99_nanos, "p99={p99:?}");
}

#[test]
fn ranked_text_owner_scope_never_expands_unproven_sibling_selectors() {
    let owner_scope = ["src/lib.rs".to_owned()].into_iter().collect();
    let languages = ["rust".to_owned()].into_iter().collect();
    let candidates = ranked_text_topology_selector_carrier(
        &owner_scope,
        &languages,
        [carrier_hit("src/lib.rs", "rust", "function", "proven")],
        8,
    )
    .expect("project exact topology selector carrier");
    assert_eq!(candidates.len(), 1);
    assert_eq!(
        candidates[0].structural_selector,
        "rust://src/lib.rs#item/function/proven"
    );
    assert!(
        ranked_text_topology_selector_carrier(&owner_scope, &languages, [], 8)
            .expect("owner-only hit is not selector evidence")
            .is_empty()
    );
}

#[test]
fn ranked_text_topology_selector_carrier_is_scope_bound_and_fails_closed() {
    let owner_scope = ["src/lib.rs".to_owned()].into_iter().collect();
    let languages = ["rust".to_owned()].into_iter().collect();
    assert!(
        ranked_text_topology_selector_carrier(
            &owner_scope,
            &languages,
            [
                carrier_hit("src/other.rs", "rust", "function", "hidden"),
                carrier_hit("src/lib.rs", "python", "function", "hidden"),
            ],
            8,
        )
        .expect("filter non-admitted hits")
        .is_empty()
    );
    let error = ranked_text_topology_selector_carrier(
        &owner_scope,
        &languages,
        [
            carrier_hit("src/lib.rs", "rust", "function", "first"),
            carrier_hit("src/lib.rs", "rust", "function", "second"),
        ],
        1,
    )
    .expect_err("over-budget carrier fails closed");
    assert!(error.contains("topology selector carrier budget exceeded"));
}

#[test]
fn topology_selector_carrier_scenario_is_sub_millisecond_p99() {
    let scenario = toml::from_str::<toml::Value>(include_str!(
        "scenarios/topology_selector_carrier/scenario.toml"
    ))
    .expect("topology selector carrier Scenario");
    let benchmark = toml::from_str::<toml::Value>(include_str!(
        "scenarios/topology_selector_carrier/benchmark.toml"
    ))
    .expect("topology selector carrier benchmark receipt");
    assert!(
        scenario["query"]["scheme"]
            .as_str()
            .is_some_and(|query| query.starts_with("(search (producers"))
    );
    let candidate_count = scenario["model"]["candidate_count"]
        .as_integer()
        .and_then(|value| usize::try_from(value).ok())
        .expect("candidate count");
    let samples = scenario["benchmark"]["samples"]
        .as_integer()
        .and_then(|value| usize::try_from(value).ok())
        .expect("sample count");
    let p99_limit = scenario["benchmark"]["maximum_p99_nanos"]
        .as_integer()
        .and_then(|value| u128::try_from(value).ok())
        .expect("p99 limit");
    assert_eq!(
        benchmark["observed"]["unproven_sibling_results"].as_integer(),
        Some(0)
    );
    let owner_scope = ["src/lib.rs".to_owned()].into_iter().collect();
    let languages = ["rust".to_owned()].into_iter().collect();
    let hits = (0..candidate_count)
        .map(|index| carrier_hit("src/lib.rs", "rust", "function", &format!("node_{index}")))
        .collect::<Vec<_>>();
    let mut observations = Vec::with_capacity(samples);
    for _ in 0..samples {
        let started = std::time::Instant::now();
        let projected = ranked_text_topology_selector_carrier(
            &owner_scope,
            &languages,
            hits.clone(),
            candidate_count,
        )
        .expect("project bounded topology selector carrier");
        observations.push(started.elapsed().as_nanos());
        assert_eq!(projected.len(), candidate_count);
    }
    observations.sort_unstable();
    let p99 = observations[(observations.len() * 99 / 100).min(observations.len() - 1)];
    eprintln!(
        "[topology-selector-carrier] candidateCount={candidate_count} samples={samples} p99Nanos={p99}"
    );
    assert!(p99 < p99_limit, "topology selector carrier p99={p99}ns");
}

fn owner<I, S>(path: &str, selector: &str, features: I) -> TopologyOwnerV1
where
    I: IntoIterator<Item = S>,
    S: Into<String>,
{
    TopologyOwnerV1 {
        owner_path: path.to_owned(),
        owner_content_digest: format!("blake3-256:{}", "1".repeat(64)),
        nodes: vec![
            TopologyNodeV1::from_selector(selector, features.into_iter().map(Into::into).collect())
                .expect("canonical topology selector"),
        ],
    }
}

fn owner_with_digest(digit: char) -> TopologyOwnerV1 {
    let mut owner = owner(
        "src/lib.rs",
        "rust://src/lib.rs#item/function/commit",
        ["commit"],
    );
    owner.owner_content_digest = format!("blake3-256:{}", digit.to_string().repeat(64));
    owner
}

fn parser_kind(language: &str, kind: &str) -> TopologyNodeKindV1 {
    TopologyNodeKindV1::ParserNative {
        language_id: language.to_owned(),
        native_kind: kind.to_owned(),
    }
}

fn carrier_hit(owner: &str, language: &str, kind: &str, name: &str) -> TopologyHitV1 {
    TopologyHitV1 {
        owner_path: owner.to_owned(),
        owner_content_digest: format!("blake3-256:{}", "0".repeat(64)),
        topology_locator: format!("{language}://{owner}#item/{kind}/{name}"),
        kind: parser_kind(language, kind),
        structural_selector: Some(format!("{language}://{owner}#item/{kind}/{name}")),
        matched_features: vec![name.to_owned()],
    }
}
