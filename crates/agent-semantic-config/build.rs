use std::fmt::Write as _;
use std::path::{Path, PathBuf};

fn main() {
    let manifest_dir = PathBuf::from(
        std::env::var_os("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR is defined"),
    );
    let agents_root = manifest_dir.join("../../agents");
    let workspace_root = manifest_dir.join("../..");
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

    let mut provider_manifest_paths = Vec::new();
    collect_provider_manifest_paths(
        &workspace_root.join("languages"),
        0,
        &mut provider_manifest_paths,
    );
    collect_provider_manifest_paths(
        &workspace_root.join("analyzers"),
        0,
        &mut provider_manifest_paths,
    );
    provider_manifest_paths.sort();
    for path in provider_manifest_paths {
        println!("cargo:rerun-if-changed={}", path.display());
    }
    generated.push_str("]\n");
    std::fs::write(&output_path, generated).unwrap_or_else(|error| {
        panic!(
            "failed to write embedded agent assets {}: {error}",
            output_path.display()
        )
    });
}

fn collect_provider_manifest_paths(root: &Path, depth: usize, output: &mut Vec<PathBuf>) {
    const MAX_PROVIDER_MANIFEST_DEPTH: usize = 3;

    let mut entries = std::fs::read_dir(root)
        .unwrap_or_else(|error| {
            panic!(
                "failed to read provider workspace directory {}: {error}",
                root.display()
            )
        })
        .map(|entry| entry.expect("provider workspace entry").path())
        .collect::<Vec<_>>();
    entries.sort();

    for path in entries {
        let file_type = std::fs::symlink_metadata(&path)
            .unwrap_or_else(|error| panic!("failed to inspect {}: {error}", path.display()))
            .file_type();
        if file_type.is_symlink() {
            continue;
        }
        if file_type.is_dir() && depth < MAX_PROVIDER_MANIFEST_DEPTH {
            let directory_name = path.file_name().and_then(|name| name.to_str());
            if matches!(
                directory_name,
                Some(".devenv" | ".direnv" | ".git" | "node_modules" | "target")
            ) {
                continue;
            }
            collect_provider_manifest_paths(&path, depth + 1, output);
        } else if path.file_name().and_then(|name| name.to_str())
            == Some("asp-provider-manifest.json")
        {
            output.push(path);
        }
    }
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
