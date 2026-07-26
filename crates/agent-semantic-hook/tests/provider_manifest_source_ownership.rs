use std::path::Path;

const ROOT_MANIFESTS: &[&str] = &[
    "rust.json",
    "typescript.json",
    "python.json",
    "gerbil-scheme.json",
    "julia.json",
    "org.json",
    "md.json",
    "c.json",
    "cpp.json",
    "objective-c.json",
];

#[test]
fn builtin_manifests_are_owned_by_the_hook_crate() {
    let crate_root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let registry = std::fs::read_to_string(crate_root.join("src/provider_registry.rs"))
        .expect("read provider registry");
    assert!(
        !registry.contains("languages/"),
        "provider_registry.rs must not compile-time embed files from languages/*"
    );

    for manifest_name in ROOT_MANIFESTS {
        let path = crate_root.join("provider-manifests").join(manifest_name);
        let bytes = std::fs::read(&path)
            .unwrap_or_else(|error| panic!("read root-owned manifest {}: {error}", path.display()));
        let packet: serde_json::Value = serde_json::from_slice(&bytes)
            .unwrap_or_else(|error| panic!("parse root-owned manifest {}: {error}", path.display()));
        assert_eq!(
            packet.get("schemaId").and_then(serde_json::Value::as_str),
            Some("agent.semantic-protocols.hook.provider-manifest"),
            "{}",
            path.display()
        );
    }
}

#[test]
fn checked_out_c_family_release_copies_match_root_ownership() {
    let crate_root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let workspace_root = crate_root.join("../..");
    for (root_name, release_name) in [
        ("c.json", "asp-c-provider-manifest.json"),
        ("cpp.json", "asp-cpp-provider-manifest.json"),
        (
            "objective-c.json",
            "asp-objective-c-provider-manifest.json",
        ),
    ] {
        let root_manifest = crate_root.join("provider-manifests").join(root_name);
        let release_manifest = workspace_root
            .join("languages/ccls-asp/provider")
            .join(release_name);
        if !release_manifest.exists() {
            continue;
        }
        assert_eq!(
            std::fs::read(&root_manifest).expect("read root-owned C-family manifest"),
            std::fs::read(&release_manifest).expect("read C-family release manifest"),
            "release copy drifted from root-owned manifest: {}",
            release_manifest.display()
        );
    }
}

#[test]
fn c_family_facades_are_present_in_the_root_locked_release_manifest() {
    let crate_root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let releases = std::fs::read_to_string(
        crate_root.join("../agent-semantic-protocol/pinned-language-releases.toml"),
    )
    .expect("read root-owned pinned language releases");
    for language in ["c", "cpp", "objective-c"] {
        assert!(
            releases.contains(&format!("[languages.{language}]")),
            "missing locked release entry for {language}"
        );
    }
}

#[test]
fn c_family_structural_schema_stays_on_root_owned_v1() {
    let crate_root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let workspace_root = crate_root.join("../..");
    let root_schema = workspace_root.join("schemas/semantic-structural-index.v1.schema.json");
    let release_schema =
        workspace_root.join("languages/ccls-asp/schemas/semantic-structural-index.v1.schema.json");
    let root_bytes = std::fs::read(&root_schema).expect("read root-owned structural-index v1 schema");
    let schema: serde_json::Value =
        serde_json::from_slice(&root_bytes).expect("parse root-owned structural-index v1 schema");
    assert_eq!(
        schema.get("$id").and_then(serde_json::Value::as_str),
        Some(
            "https://agent.semantic-protocols/schemas/semantic-structural-index.v1.schema.json"
        )
    );
    assert!(
        !workspace_root
            .join("schemas/semantic-structural-index.v2.schema.json")
            .exists(),
        "C-family must keep the single stable structural-index v1 contract"
    );
    if release_schema.exists() {
        assert_eq!(
            root_bytes,
            std::fs::read(&release_schema).expect("read C-family schema release copy"),
            "C-family schema release copy drifted from the root-owned v1 schema"
        );
    }
}
