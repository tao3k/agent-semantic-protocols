use std::collections::BTreeSet;

use super::registered_provider_binaries_v1;

#[test]
fn registered_binary_identities_are_materialized_from_the_v1_registry_schema() {
    let registry: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../schemas/semantic-language-registry.providers.v1.json"
    ))
    .expect("v1 provider registry must be valid JSON");
    let expected = registry["languages"]
        .as_array()
        .expect("v1 provider registry must declare languages")
        .iter()
        .map(|registration| {
            (
                registration["languageId"]
                    .as_str()
                    .expect("registration must declare languageId")
                    .to_string(),
                registration["providerId"]
                    .as_str()
                    .expect("registration must declare providerId")
                    .to_string(),
                registration["binary"]
                    .as_str()
                    .expect("registration must declare binary")
                    .to_string(),
            )
        })
        .collect::<Vec<_>>();
    let actual = registered_provider_binaries_v1()
        .into_iter()
        .map(|identity| {
            (
                identity.language_id().as_str().to_string(),
                identity.provider_id().as_str().to_string(),
                identity.binary().to_string(),
            )
        })
        .collect::<Vec<_>>();

    assert_eq!(actual, expected);
}

#[test]
fn shared_provider_binaries_are_not_duplicated_by_publication_planners() {
    let registrations = registered_provider_binaries_v1();
    let binaries = registrations
        .iter()
        .map(|identity| identity.binary())
        .collect::<BTreeSet<_>>();

    assert!(registrations.len() >= binaries.len());
    assert!(binaries.iter().all(|binary| {
        !binary.is_empty()
            && !binary.contains('/')
            && !binary.contains('\\')
            && *binary != "."
            && *binary != ".."
    }));
}
