use agent_semantic_config::{
    AspCommandIntent, AspCommandRouteId, HookClientAspCommandIntentPolicyConfig,
    classify_asp_language_command,
};

fn tokens(values: &[&str]) -> Vec<String> {
    values.iter().map(|value| (*value).to_string()).collect()
}

#[test]
fn gerbil_selector_requires_the_exact_gerbil_scheme_language_id() {
    let policy = HookClientAspCommandIntentPolicyConfig::default();
    let matched = classify_asp_language_command(
        "gerbil-scheme".to_string(),
        &tokens(&[
            "query",
            "--selector",
            "gerbil-scheme://src/example.ss#item/def/example",
            "--code",
        ]),
        &policy,
    )
    .expect("classify canonical Gerbil selector");

    assert_eq!(matched.intent, AspCommandIntent::ExactEvidence);
    assert_eq!(matched.route, AspCommandRouteId::QuerySelector);
}

#[test]
fn document_verbatim_selector_is_exact_evidence() {
    let policy = HookClientAspCommandIntentPolicyConfig::default();
    let matched = classify_asp_language_command(
        "org".to_string(),
        &tokens(&[
            "query",
            "--selector",
            "org://docs/guide.org#paragraph/paragraph/document[1]/paragraph[1]",
            "--verbatim",
        ]),
        &policy,
    )
    .expect("classify exact Org selector");

    assert_eq!(matched.intent, AspCommandIntent::ExactEvidence);
    assert_eq!(matched.route, AspCommandRouteId::QuerySelector);
}

#[test]
fn generic_scheme_selector_is_cross_language_for_gerbil_scheme() {
    let policy = HookClientAspCommandIntentPolicyConfig::default();
    let matched = classify_asp_language_command(
        "gerbil-scheme".to_string(),
        &tokens(&[
            "query",
            "--selector",
            "scheme://src/example.ss#item/def/example",
            "--code",
        ]),
        &policy,
    )
    .expect("classify cross-language selector");

    assert_eq!(matched.intent, AspCommandIntent::InvalidEvidence);
    assert_eq!(matched.route, AspCommandRouteId::QuerySelector);
}

#[test]
fn configured_search_route_produces_the_canonical_route_id() {
    let policy = HookClientAspCommandIntentPolicyConfig::default();
    let matched = classify_asp_language_command(
        "rust".to_string(),
        &tokens(&["search", "pipe", "owner routing"]),
        &policy,
    )
    .expect("classify configured search route");

    assert_eq!(matched.route, AspCommandRouteId::Search("pipe".to_string()));
    assert_eq!(matched.route.wire_value(), "search-pipe");
}
