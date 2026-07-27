use serde_json::Value;

const C_FAMILY_MANIFESTS: &[(&str, &str)] = &[
    ("c", include_str!("../provider-manifests/c.json")),
    ("cpp", include_str!("../provider-manifests/cpp.json")),
    (
        "objective-c",
        include_str!("../provider-manifests/objective-c.json"),
    ),
];

#[test]
fn c_family_manifests_declare_the_stable_native_library_v1() {
    for (language_id, source) in C_FAMILY_MANIFESTS {
        let manifest: Value = serde_json::from_str(source)
            .unwrap_or_else(|error| panic!("{language_id} manifest is invalid: {error}"));
        let descriptor = manifest
            .get("nativeLibrary")
            .unwrap_or_else(|| panic!("{language_id} manifest must declare nativeLibrary"));

        assert_eq!(
            descriptor.get("descriptorId").and_then(Value::as_str),
            Some("c-family.native-parser-library"),
            "{language_id} native library descriptor id"
        );
        assert_eq!(
            descriptor.get("descriptorVersion").and_then(Value::as_str),
            Some("1"),
            "{language_id} native library descriptor version"
        );
        assert_eq!(
            descriptor.get("abiVersion").and_then(Value::as_u64),
            Some(1),
            "{language_id} native ABI version"
        );
        assert_eq!(
            descriptor.get("artifactStem").and_then(Value::as_str),
            Some("ccls-asp-parser"),
            "{language_id} native library artifact"
        );
        assert_eq!(
            descriptor
                .get("parseTranslationUnitSymbol")
                .and_then(Value::as_str),
            Some("ccls_asp_parse_translation_unit_with_args_v1"),
            "{language_id} single-TU entrypoint"
        );
        assert_eq!(
            descriptor.get("fallbackExecution").and_then(Value::as_str),
            Some("external-process"),
            "{language_id} migration fallback"
        );
    }
}

#[test]
fn root_native_manifest_test_never_embeds_a_languages_checkout() {
    let source = include_str!("c_family_native_library_manifest.rs");
    assert!(
        !source.contains("include_str!(\"../../../languages/"),
        "root tests must not compile in a languages/* checkout"
    );
}
