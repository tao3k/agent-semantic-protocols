use agent_semantic_hook::{builtin_provider_manifests, validate_provider_manifest_contract};
use std::path::{Path, PathBuf};

fn canonical_manifest() -> agent_semantic_hook::ProviderManifest {
    builtin_provider_manifests()
        .into_iter()
        .next()
        .expect("builtin provider registry must not be empty")
}

#[test]
fn builtin_manifest_requires_source_snapshot_descriptor() {
    let mut value =
        serde_json::to_value(canonical_manifest()).expect("serialize canonical manifest");
    value["searchCapabilities"]["sourceSnapshot"] = serde_json::Value::Null;
    let manifest =
        serde_json::from_value(value).expect("deserialize invalid source snapshot fixture");

    assert_eq!(
        validate_provider_manifest_contract(&manifest),
        vec![format!(
            "provider `{}` is missing required searchCapabilities.sourceSnapshot descriptor",
            manifest.language_id()
        )]
    );
}

#[test]
fn builtin_manifest_rejects_invalid_query_pack_descriptor_version() {
    let mut value =
        serde_json::to_value(canonical_manifest()).expect("serialize canonical manifest");
    value["queryPackDescriptor"]["descriptorVersion"] =
        serde_json::Value::String("invalid".to_string());
    let manifest = serde_json::from_value(value).expect("deserialize invalid query pack fixture");

    assert_eq!(
        validate_provider_manifest_contract(&manifest),
        vec![format!(
            "invalid-activation-config: provider manifest {} has an invalid queryPackDescriptor: identity, version, language, or recipes",
            manifest.manifest_id()
        )]
    );
}

#[test]
fn programming_providers_expose_only_project_resolution_without_workspace_identity() {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let workspace = manifest_dir
        .parent()
        .and_then(Path::parent)
        .expect("workspace root")
        .to_path_buf();
    let providers = [
        (
            "languages/rust-lang-project-harness",
            "provider/asp-provider-manifest.json",
            &["src"] as &[&str],
            "Cargo.toml",
            "rust.cargo-toml",
        ),
        (
            "languages/python-lang-project-harness",
            "provider/asp-provider-manifest.json",
            &["src"],
            "pyproject.toml",
            "python.pyproject-toml",
        ),
        (
            "languages/typescript-lang-project-harness",
            "provider/asp-provider-manifest.json",
            &["src"],
            "package.json",
            "typescript.package-json",
        ),
        (
            "languages/JuliaLangProjectHarness.jl",
            "juliac/asp-provider-manifest.json",
            &["src", "juliac"],
            "Project.toml",
            "julia.pkg-project-toml",
        ),
        (
            "languages/gerbil-scheme-language-project-harness",
            "provider/asp-provider-manifest.json",
            &["src"],
            "gerbil.pkg",
            "gerbil.package-spec",
        ),
    ];
    let forbidden = [
        "repositoryCandidates",
        "repositoryIdentity",
        "worktreeIdentity",
    ];

    for (provider_root, manifest_path, source_roots, project_entry, parser_id) in providers {
        let root = workspace.join(provider_root);
        let manifest: serde_json::Value = serde_json::from_slice(
            &std::fs::read(root.join(manifest_path)).expect("read provider manifest"),
        )
        .expect("parse provider manifest");
        let scope = manifest
            .get("projectResolution")
            .expect("programming provider owns ProjectResolution");
        assert_eq!(scope["capabilityId"], "project-resolution");
        assert_eq!(scope["commandBinding"], "project-resolution-stdin");
        assert_eq!(scope["entryMarkers"], serde_json::json!([project_entry]));
        assert_eq!(scope["parserId"], parser_id);
        assert!(manifest.get("documentResolution").is_none());

        for required_schema in [
            "provider-project-resolution-descriptor.v1.schema.json",
            "provider-project-resolution-request.v1.schema.json",
            "provider-project-resolution-response.v1.schema.json",
            "project-resolution.v1.schema.json",
        ] {
            assert!(
                root.join("schemas").join(required_schema).is_file(),
                "provider schema boundary is missing {provider_root}/schemas/{required_schema}"
            );
        }
        assert!(
            !root
                .join("schemas")
                .join("repository-candidate-snapshot.v1.schema.json")
                .exists(),
            "provider must not own ASP repository/worktree candidate identity: {provider_root}"
        );
        for source_root in source_roots {
            assert_provider_sources_exclude_workspace_identity(&root.join(source_root), &forbidden);
        }
    }
}

fn assert_provider_sources_exclude_workspace_identity(root: &Path, forbidden: &[&str]) {
    for entry in std::fs::read_dir(root).expect("read provider source root") {
        let path = entry.expect("read provider source entry").path();
        if path.is_dir() {
            assert_provider_sources_exclude_workspace_identity(&path, forbidden);
            continue;
        }
        let Some(extension) = path.extension().and_then(|value| value.to_str()) else {
            continue;
        };
        if !matches!(extension, "rs" | "py" | "ts" | "jl" | "ss" | "json") {
            continue;
        }
        let source = std::fs::read_to_string(&path).expect("read provider source");
        for term in forbidden {
            assert!(
                !source.contains(term),
                "provider source retained ASP-owned workspace identity term {term}: {}",
                path.display()
            );
        }
    }
}
