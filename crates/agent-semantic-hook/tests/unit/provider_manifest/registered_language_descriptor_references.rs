use std::collections::BTreeSet;
use std::fs;
use std::path::{Component, Path, PathBuf};

#[test]
fn semantic_language_registry_is_a_bounded_reference_index() {
    let schema_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../schemas");
    let registry_path = schema_dir.join("semantic-language-registry.providers.v1.json");
    let registry_source = fs::read_to_string(&registry_path).expect("read registry index");
    assert!(
        registry_source.lines().count() <= 80,
        "root Provider Register must remain a bounded reference index"
    );
    let registry: serde_json::Value =
        serde_json::from_str(&registry_source).expect("parse registry index");
    let registrations = registry["languages"]
        .as_array()
        .expect("registry index languages");
    let mut references = BTreeSet::new();

    for registration in registrations {
        let object = registration
            .as_object()
            .expect("registration reference object");
        assert_eq!(
            object.keys().map(String::as_str).collect::<BTreeSet<_>>(),
            BTreeSet::from(["descriptor", "languageId", "providerId"]),
            "root registration may only carry identity plus descriptor reference"
        );
        let language_id = registration["languageId"]
            .as_str()
            .expect("reference languageId");
        let provider_id = registration["providerId"]
            .as_str()
            .expect("reference providerId");
        if matches!(language_id, "rust" | "python" | "gerbil-scheme") {
            let projection_binding =
                agent_semantic_hook::registered_provider_projection_command_binding(
                    language_id,
                    provider_id,
                )
                .expect("projection binding lookup")
                .unwrap_or_else(|| {
                    panic!("exact language must declare the projection runtime operation: {language_id}")
                });
            assert_eq!(projection_binding, "projection-batch-stdin");
        }
        let reference = registration["descriptor"]["$ref"]
            .as_str()
            .expect("descriptor $ref");
        let relative = Path::new(reference);
        assert!(
            !relative.is_absolute()
                && relative
                    .components()
                    .all(|component| matches!(component, Component::Normal(_))),
            "descriptor reference must stay inside schemas/: {reference}"
        );
        assert!(
            references.insert(reference),
            "duplicate descriptor reference"
        );

        let descriptor_source = fs::read_to_string(schema_dir.join(relative))
            .unwrap_or_else(|error| panic!("read descriptor {reference}: {error}"));
        let descriptor: serde_json::Value = serde_json::from_str(&descriptor_source)
            .unwrap_or_else(|error| panic!("parse descriptor {reference}: {error}"));
        assert_eq!(descriptor["languageId"], language_id);
        assert_eq!(descriptor["providerId"], provider_id);
        let methods = descriptor["methods"]
            .as_array()
            .expect("descriptor methods")
            .iter()
            .map(|method| method.as_str().expect("method string"))
            .collect::<BTreeSet<_>>();
        let descriptor_methods = descriptor["methodDescriptors"]
            .as_array()
            .expect("descriptor methodDescriptors")
            .iter()
            .map(|method| method["method"].as_str().expect("descriptor method"))
            .collect::<BTreeSet<_>>();
        assert_eq!(methods, descriptor_methods, "method inventory drift");
    }
}
