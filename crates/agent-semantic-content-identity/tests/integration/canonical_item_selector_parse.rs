use agent_semantic_content_identity::canonical_item_identity::CanonicalItemSelector;

#[test]
fn canonical_selector_parse_is_language_neutral() {
    for (language_id, selector) in [
        (
            "rust",
            "rust://src/lib.rs#item/method/parse/scope/implementation-owner/type/Parser",
        ),
        (
            "typescript",
            "typescript://src/index.ts#item/function/parse",
        ),
        ("python", "python://src/parser.py#item/function/parse"),
        (
            "gerbil-scheme",
            "gerbil-scheme://src/parser.ss#item/function/parse",
        ),
        ("julia", "julia://src/Parser.jl#item/function/parse"),
    ] {
        let parsed = CanonicalItemSelector::parse(selector)
            .unwrap_or_else(|error| panic!("{language_id} selector rejected: {error}"));

        assert_eq!(parsed.language_id.as_str(), language_id);
        assert_eq!(parsed.structural_selector(), selector);
        parsed.validate().expect("parsed selector must validate");
    }
}

#[test]
fn canonical_selector_parse_rejects_non_item_and_noncanonical_identity_paths() {
    for selector in [
        "",
        "src/lib.rs:1:20",
        "rust://#item/function/parse",
        "rust://src/lib.rs#function/parse",
        "rust://src/lib.rs#item/function/parse/scope/type",
        "rust://src/lib.rs#item/function/%70arse",
    ] {
        assert!(
            CanonicalItemSelector::parse(selector).is_err(),
            "invalid selector was accepted: {selector}"
        );
    }
}
