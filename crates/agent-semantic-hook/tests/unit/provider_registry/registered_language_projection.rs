use super::{REGISTERED_LANGUAGE_ID_STRINGS, schema_registry};

#[test]
fn build_projection_matches_the_registry_schema() {
    let mut schema_language_ids = schema_registry()
        .languages
        .iter()
        .map(|registration| registration.language_id.as_str())
        .collect::<Vec<_>>();
    schema_language_ids.sort_unstable();
    schema_language_ids.dedup();
    assert_eq!(schema_language_ids, REGISTERED_LANGUAGE_ID_STRINGS);
}
