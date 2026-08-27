use std::fmt::Write as _;
use std::path::{Path, PathBuf};

#[path = "../agent-semantic-schema-manager/src/build_support/provider_registry.rs"]
mod provider_registry;

fn main() {
    let manifest_dir = PathBuf::from(
        std::env::var_os("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR is defined"),
    );
    let agents_root = manifest_dir.join("../../agents");
    let source_root = manifest_dir.join("../..");
    let output_path = PathBuf::from(std::env::var_os("OUT_DIR").expect("OUT_DIR is defined"))
        .join("embedded_agent_assets.rs");

    println!("cargo:rerun-if-changed={}", agents_root.display());
    let mut asset_paths = std::fs::read_dir(&agents_root)
        .unwrap_or_else(|error| {
            panic!(
                "failed to read canonical ASP agents directory {}: {error}",
                agents_root.display()
            )
        })
        .map(|entry| entry.expect("canonical ASP agent asset entry").path())
        .filter(|path| is_registry_asset(path))
        .collect::<Vec<_>>();
    asset_paths.sort();

    let mut generated = String::from("&[\n");
    for path in asset_paths {
        println!("cargo:rerun-if-changed={}", path.display());
        let file_name = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or_else(|| panic!("agent asset has a non-UTF-8 file name: {}", path.display()));
        let bytes = std::fs::read(&path).unwrap_or_else(|error| {
            panic!("failed to read agent asset {}: {error}", path.display())
        });
        writeln!(
            generated,
            "    EmbeddedAgentAsset {{ file_name: {file_name:?}, contents: &{bytes:?} }},"
        )
        .expect("write embedded agent asset source");
    }

    generated.push_str("]\n");
    std::fs::write(&output_path, generated).unwrap_or_else(|error| {
        panic!(
            "failed to write embedded agent assets {}: {error}",
            output_path.display()
        )
    });

    let provider_register = provider_registry::resolve_provider_register(&source_root)
        .expect("resolve canonical provider identity register");
    for path in &provider_register.input_paths {
        println!("cargo:rerun-if-changed={}", path.display());
    }
    let mut identities = String::from("&[\n");
    for identity in provider_register.identities {
        writeln!(
            identities,
            "    ({:?}, {:?}),",
            identity.language_id, identity.provider_id
        )
        .expect("write embedded provider route identity");
    }
    identities.push_str("]\n");
    let identities_path = PathBuf::from(std::env::var_os("OUT_DIR").expect("OUT_DIR is defined"))
        .join("provider_route_identities.rs");
    std::fs::write(&identities_path, identities).unwrap_or_else(|error| {
        panic!(
            "failed to write provider route identities {}: {error}",
            identities_path.display()
        )
    });
}

fn is_registry_asset(path: &Path) -> bool {
    if !path.is_file() {
        return false;
    }
    let Some(file_name) = path.file_name().and_then(|name| name.to_str()) else {
        return false;
    };
    file_name == "config.toml" || file_name.ends_with(".toml") || file_name.ends_with(".md")
}
