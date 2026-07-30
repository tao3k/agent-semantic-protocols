use std::path::PathBuf;

fn main() {
    let manifest_dir = PathBuf::from(
        std::env::var_os("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR must be set"),
    );
    let registry_path =
        manifest_dir.join("../../schemas/semantic-language-registry.providers.v1.json");
    println!("cargo:rerun-if-changed={}", registry_path.display());

    let registry_source =
        std::fs::read_to_string(&registry_path).expect("read semantic language registry");
    let registry: serde_json::Value =
        serde_json::from_str(&registry_source).expect("parse semantic language registry");
    let mut language_ids = registry["languages"]
        .as_array()
        .expect("semantic language registry languages")
        .iter()
        .map(|registration| {
            registration["languageId"]
                .as_str()
                .expect("semantic language registry languageId")
                .to_string()
        })
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
    let output_path = PathBuf::from(std::env::var_os("OUT_DIR").expect("OUT_DIR must be set"))
        .join("registered_language_ids.rs");
    std::fs::write(output_path, generated).expect("write registered language id projection");
}
