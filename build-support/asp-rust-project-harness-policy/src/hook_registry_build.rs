//! Build-time projection of the shared semantic language registry for the hook crate.

use std::collections::BTreeSet;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

const REGISTRY_INDEX: &str = "semantic-language-registry.providers.v1.json";
const RESOLVED_REGISTRY: &str = "semantic-language-registry.providers.resolved.v1.json";

/// Materialize the hook crate's embedded provider registry from its Cargo build environment.
pub fn generate_agent_semantic_hook_registry_from_env() {
    let manifest_dir = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").expect("manifest dir"));
    let schema_dir = manifest_dir.join("../../schemas");
    let mut registry = load_registry_index(&schema_dir);
    resolve_language_descriptors(&schema_dir, &mut registry);
    write_registry_projections(&registry);
}

fn load_registry_index(schema_dir: &Path) -> serde_json::Value {
    let registry_path = schema_dir.join(REGISTRY_INDEX);
    println!("cargo:rerun-if-changed={}", registry_path.display());
    let registry_source = fs::read_to_string(&registry_path).expect("read semantic registry index");
    serde_json::from_str(&registry_source).expect("parse semantic registry index")
}

fn resolve_language_descriptors(schema_dir: &Path, registry: &mut serde_json::Value) {
    let languages = registry
        .get_mut("languages")
        .and_then(serde_json::Value::as_array_mut)
        .expect("semantic registry index must declare languages");

    let mut references = BTreeSet::new();
    for registration in languages {
        let reference = registration
            .get("descriptor")
            .and_then(|descriptor| descriptor.get("$ref"))
            .and_then(serde_json::Value::as_str)
            .expect("semantic registry index entry must declare descriptor.$ref");
        assert!(
            references.insert(reference.to_owned()),
            "duplicate semantic language descriptor reference `{reference}`"
        );
        let language_id = required_string(registration, "languageId");
        let provider_id = required_string(registration, "providerId");
        let descriptor_path = resolve_reference(schema_dir, reference);
        println!("cargo:rerun-if-changed={}", descriptor_path.display());
        let descriptor_source = fs::read_to_string(&descriptor_path).unwrap_or_else(|error| {
            panic!(
                "read semantic language descriptor `{}`: {error}",
                descriptor_path.display()
            )
        });
        let descriptor: serde_json::Value = serde_json::from_str(&descriptor_source)
            .unwrap_or_else(|error| panic!("parse descriptor `{reference}`: {error}"));
        assert_eq!(
            required_string(&descriptor, "languageId"),
            language_id,
            "languageId drift in `{reference}`"
        );
        assert_eq!(
            required_string(&descriptor, "providerId"),
            provider_id,
            "providerId drift in `{reference}`"
        );
        *registration = descriptor;
    }
}

fn write_registry_projections(registry: &serde_json::Value) {
    let output_dir = PathBuf::from(env::var_os("OUT_DIR").expect("out dir"));
    fs::write(
        output_dir.join(RESOLVED_REGISTRY),
        serde_json::to_vec_pretty(&registry).expect("serialize resolved semantic registry"),
    )
    .expect("write resolved semantic registry");
    write_registered_language_ids(registry, &output_dir);
}

fn write_registered_language_ids(registry: &serde_json::Value, output_dir: &Path) {
    let mut language_ids = registry["languages"]
        .as_array()
        .expect("resolved semantic registry languages")
        .iter()
        .map(|registration| required_string(registration, "languageId").to_owned())
        .collect::<Vec<_>>();
    language_ids.sort();
    language_ids.dedup();
    let generated = format!(
        "pub(crate) const REGISTERED_LANGUAGE_ID_STRINGS: &[&str] = &[{}];\n",
        language_ids
            .iter()
            .map(|language_id| serde_json::to_string(language_id).expect("encode language id"))
            .collect::<Vec<_>>()
            .join(",")
    );
    fs::write(output_dir.join("registered_language_ids.rs"), generated)
        .expect("write registered language id projection");
}

fn required_string<'a>(value: &'a serde_json::Value, field: &str) -> &'a str {
    value
        .get(field)
        .and_then(serde_json::Value::as_str)
        .unwrap_or_else(|| panic!("semantic registry entry must declare string `{field}`"))
}

fn resolve_reference(schema_dir: &Path, reference: &str) -> PathBuf {
    let relative = Path::new(reference);
    assert!(
        !relative.is_absolute()
            && relative
                .components()
                .all(|component| matches!(component, std::path::Component::Normal(_))),
        "semantic language descriptor reference must stay inside schemas/: `{reference}`"
    );
    schema_dir.join(relative)
}
