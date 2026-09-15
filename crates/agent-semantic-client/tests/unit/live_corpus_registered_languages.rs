// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use std::collections::BTreeSet;

use agent_semantic_schema_manager::SchemaManager;

#[test]
fn live_corpus_plan_covers_every_registered_language_profile() {
    let workspace_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let registered = SchemaManager::new(&workspace_root)
        .registered_language_profiles()
        .expect("registered language profiles")
        .into_iter()
        .map(|profile| profile.language_id)
        .collect::<BTreeSet<_>>();
    let plan: toml::Value = toml::from_str(include_str!(
        "../../../../benchmarks/live-corpus-scheme-scenarios.v1.toml"
    ))
    .expect("live corpus qualification plan");
    let cases = plan
        .get("cases")
        .and_then(toml::Value::as_array)
        .expect("qualification cases");
    let covered = cases
        .iter()
        .filter_map(|case| case.get("language_id").and_then(toml::Value::as_str))
        .map(str::to_owned)
        .collect::<BTreeSet<_>>();

    assert_eq!(
        covered, registered,
        "Live Corpus cases must be generated from the central registered-language profile set"
    );
}

#[test]
fn live_corpus_v1_protocol_count_is_plan_wide_not_shard_local() {
    let plan: toml::Value = toml::from_str(include_str!(
        "../../../../benchmarks/live-corpus-scheme-scenarios.v1.toml"
    ))
    .expect("live corpus qualification plan");
    let schema: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../schemas/asp.live-corpus-search-query-qualification-receipt.schema.json"
    ))
    .expect("live corpus qualification receipt schema");
    let plan_count = plan
        .get("client_protocol")
        .and_then(|value| value.get("applies_to_case_count"))
        .and_then(toml::Value::as_integer)
        .and_then(|value| u64::try_from(value).ok())
        .expect("V1 plan-wide protocol count");
    let case_count = plan
        .get("cases")
        .and_then(toml::Value::as_array)
        .map(|cases| cases.len() as u64)
        .expect("V1 plan cases");
    let receipt_count = schema
        .pointer("/$defs/clientProtocolReceipt/properties/qualifiedCaseCount/const")
        .and_then(serde_json::Value::as_u64)
        .expect("V1 receipt protocol count");

    assert_eq!(plan_count, case_count);
    assert_eq!(receipt_count, plan_count);
}
