// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use std::fs;
use std::path::Path;

use crate::provider_core::{
    LANGUAGE_CONFORMANCE_PACK_SCHEMA_ID, LanguageConformancePack, PROVIDER_CORE_MANIFEST_SCHEMA_ID,
    PROVIDER_CORE_MANIFEST_SCHEMA_VERSION, PROVIDER_CORE_SCHEMA_VERSION, PROVIDER_CORE_SCHEMAS,
    ProviderCapabilities, ProviderCoreManifest, ProviderLanguageId, ProviderPackageId,
    ProviderSyntaxSurface, ProviderTransport, canonical_provider_core_schemas,
};

#[test]
fn schemars_generated_provider_core_schemas_match_committed_files_byte_for_byte() {
    let schemas_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../schemas");
    let generated =
        canonical_provider_core_schemas().expect("generate Provider Core canonical schemas");
    assert_eq!(generated.len(), 4);
    for (filename, expected_value) in generated {
        assert!(
            !filename.contains(".v1.") && !filename.contains(".v2."),
            "generated schema filename must carry version inside schema, not filename: {filename}"
        );
        let on_disk_path = schemas_dir.join(filename);
        let raw = fs::read_to_string(&on_disk_path).unwrap_or_else(|err| {
            panic!(
                "missing generated provider core schema {}: {err}",
                on_disk_path.display()
            )
        });
        let actual_value: serde_json::Value =
            serde_json::from_str(&raw).expect("parse on-disk provider core schema");
        assert_eq!(
            actual_value, expected_value,
            "Provider Core schema drift detected in {filename}; regenerate from Rust SSoT"
        );
    }
    for name in PROVIDER_CORE_SCHEMAS {
        assert!(
            schemas_dir.join(name).is_file(),
            "Provider Core schema missing on disk: {name}"
        );
    }
}

#[test]
fn provider_core_manifest_and_conformance_pack_round_trip() {
    let manifest = ProviderCoreManifest {
        schema_id: PROVIDER_CORE_MANIFEST_SCHEMA_ID.to_owned(),
        schema_version: PROVIDER_CORE_MANIFEST_SCHEMA_VERSION.to_owned(),
        language_id: ProviderLanguageId::new("julia").expect("julia id"),
        provider_id: ProviderPackageId::new("asp-julia").expect("asp-julia id"),
        display_name: "Julia".to_owned(),
        extensions: vec![".jl".to_owned()],
        project_markers: vec!["Project.toml".to_owned()],
        syntax_surface: ProviderSyntaxSurface {
            line_comment: Some("#".to_owned()),
            block_comment: Some(["#=".to_owned(), "=#".to_owned()]),
            delimiter_pairs: vec![
                ["(".to_owned(), ")".to_owned()],
                ["[".to_owned(), "]".to_owned()],
                ["{".to_owned(), "}".to_owned()],
            ],
            statement_terminator: None,
            string_delimiters: vec!["\"".to_owned()],
        },
        transport: ProviderTransport {
            transport: "http-json".to_owned(),
            command: vec!["serve".to_owned()],
            health_path: "/health".to_owned(),
            request_path: "/v1/provider-runtime".to_owned(),
            shutdown_path: "/shutdown".to_owned(),
        },
        capabilities: ProviderCapabilities {
            ast_inspect: true,
            ast_verify: true,
            open_edb: true,
            project_resolution: true,
        },
    };
    let encoded = serde_json::to_string(&manifest).expect("encode provider core manifest");
    let decoded: ProviderCoreManifest =
        serde_json::from_str(&encoded).expect("decode provider core manifest");
    assert_eq!(decoded, manifest);
    assert_eq!(decoded.syntax_surface.line_comment.as_deref(), Some("#"));

    let pack_json = serde_json::json!({
        "schemaId": LANGUAGE_CONFORMANCE_PACK_SCHEMA_ID,
        "schemaVersion": PROVIDER_CORE_SCHEMA_VERSION,
        "languageId": "julia",
        "providerLanguageId": "julia",
        "extensions": [".jl"],
        "projectMarkers": ["Project.toml"],
        "kindMappings": [
            {
                "nativeLanguageKind": "function",
                "generalKind": "function",
                "description": "Julia function definition"
            }
        ],
        "specialRelations": ["julia_macro"],
        "probes": [
            {
                "probeId": "julia-fn",
                "path": "src/sample.jl",
                "source": "function add(x, y)\n    x + y\nend\n",
                "expectValid": true,
                "expectedNodes": [
                    { "kind": "function", "languageKind": "function", "name": "add" }
                ]
            }
        ]
    });
    let pack: LanguageConformancePack =
        serde_json::from_value(pack_json).expect("decode conformance pack");
    assert_eq!(pack.language_id.as_str(), "julia");
    assert_eq!(pack.probes.len(), 1);
}
