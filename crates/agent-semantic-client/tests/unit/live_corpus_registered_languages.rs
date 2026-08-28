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
    let plan: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../benchmarks/live-corpus-search-query-qualification.json"
    ))
    .expect("live corpus qualification plan");
    let cases = plan
        .get("cases")
        .and_then(serde_json::Value::as_array)
        .expect("qualification cases");
    let covered = cases
        .iter()
        .filter_map(|case| {
            case.get("languageId")
                .or_else(|| case.get("language_id"))
                .and_then(serde_json::Value::as_str)
        })
        .map(str::to_owned)
        .collect::<BTreeSet<_>>();

    assert_eq!(
        covered, registered,
        "Live Corpus cases must be generated from the central registered-language profile set"
    );
}
