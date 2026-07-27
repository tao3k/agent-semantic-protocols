use std::path::Path;

use super::c_family_native::{NativeFact, NativeParseResult, NativeSourceRange};
use super::c_family_native_projection::{
    NativeProjectionRequest, native_library_available, native_projection_json,
    normalize_compile_args, split_shell_words, try_native_projection,
};

#[test]
fn shell_command_preserves_quoted_compile_definitions() {
    let words = split_shell_words(r#"clang++ -DNAME="hello world" -I'include dir' -c src/main.cc"#)
        .expect("split compile command");

    assert_eq!(
        words,
        [
            "clang++",
            "-DNAME=hello world",
            "-Iinclude dir",
            "-c",
            "src/main.cc"
        ]
    );
}

#[test]
fn compile_args_remove_driver_input_and_output() {
    let args = normalize_compile_args(
        vec![
            "clang".into(),
            "-std=c17".into(),
            "-c".into(),
            "src/main.c".into(),
            "-o".into(),
            "main.o".into(),
            "-DVALUE=1".into(),
        ],
        Path::new("/workspace"),
        Path::new("/workspace/src/main.c"),
    );

    assert_eq!(args, ["-std=c17", "-DVALUE=1"]);
}

#[test]
fn native_facts_shape_the_existing_projection_v1_contract() {
    let request = NativeProjectionRequest {
        activation_root: Path::new("/activation"),
        project_root: Path::new("/workspace"),
        owner: Path::new("src/main.c"),
        language_id: "c",
        provider_id: "ccls-asp",
        artifact_stem: "ccls-asp-parser",
        abi_version: 1,
        parse_symbol: "parse",
        free_symbol: "free",
    };
    let result = NativeParseResult {
        facts: vec![NativeFact {
            name: "main".into(),
            qualified_name: "main".into(),
            symbol_id: "symbol-main".into(),
            semantic_variant_id: "variant-main".into(),
            kind: "function".into(),
            role: "definition".into(),
            visibility: "public".into(),
            r#type: "int ()".into(),
            target: String::new(),
            target_symbol_id: String::new(),
            container_symbol_id: String::new(),
            translation_unit: "src/main.c".into(),
            compile_context_digest: "compile-context".into(),
            location: NativeSourceRange {
                path: "src/main.c".into(),
                start_line: 1,
                end_line: 1,
                start_column: 1,
                end_column: 11,
                start_offset: 0,
                end_offset: 10,
                structural_selector: "c://src/main.c#item/function/main".into(),
            },
        }],
        dependency_usages: Vec::new(),
        compile_contexts: Vec::new(),
        translation_units: vec!["src/main.c".into()],
        errors: Vec::new(),
    };

    let bytes = native_projection_json(&request, &result).expect("shape projection");
    let json = std::str::from_utf8(&bytes).expect("UTF-8 projection");
    let projection = agent_semantic_client_db::ClientDbLanguageProjection::from_json(json)
        .expect("valid shared projection v1");

    assert_eq!(projection.language_id(), "c");
    assert_eq!(projection.sources()[0].path, "src/main.c");
}

#[test]
#[ignore = "requires a freshly built ccls-asp shared library"]
fn native_output_validates_structural_index_v1() {
    let source_library =
        std::env::var_os("CCLS_ASP_TEST_LIBRARY").expect("set CCLS_ASP_TEST_LIBRARY");
    let root = std::env::temp_dir().join(format!(
        "asp-c-family-structural-smoke-{}",
        std::process::id()
    ));
    let workspace = root.join("workspace");
    let activation_root = root.join("activation");
    std::fs::create_dir_all(&workspace).expect("create workspace");
    let installed_library =
        super::c_family_native::installed_library_path(&activation_root, "ccls-asp-parser");
    std::fs::create_dir_all(installed_library.parent().expect("library parent"))
        .expect("create activation lib");
    std::fs::copy(&source_library, &installed_library).expect("install test shared library");
    assert!(native_library_available(
        &activation_root,
        "ccls-asp-parser"
    ));
    std::fs::write(
        workspace.join("dep.h"),
        "static inline int dep_value(void) { return 42; }\n",
    )
    .expect("write dependency header");
    std::fs::write(
        workspace.join("main.c"),
        "#include \"dep.h\"\nint answer(void) { return dep_value(); }\n",
    )
    .expect("write translation unit");
    let compile_commands = serde_json::json!([{
        "directory": workspace,
        "file": "main.c",
        "arguments": ["clang", "-I.", "-std=c17", "-c", "main.c", "-o", "main.o"],
    }]);
    std::fs::write(
        workspace.join("compile_commands.json"),
        serde_json::to_vec_pretty(&compile_commands).expect("encode compile commands"),
    )
    .expect("write compile commands");

    use sha2::{Digest, Sha256};

    let main_bytes = std::fs::read(workspace.join("main.c")).expect("read translation unit");
    let dependency_bytes = std::fs::read(workspace.join("dep.h")).expect("read dependency header");
    let workspace_snapshot =
        agent_semantic_content_identity::WorkspaceSnapshot::from_file_hashes([
            (
                "main.c".to_string(),
                format!("{:x}", Sha256::digest(&main_bytes)),
            ),
            (
                "dep.h".to_string(),
                format!("{:x}", Sha256::digest(&dependency_bytes)),
            ),
        ]);
    let source_blobs =
        agent_semantic_client_db::ClientDbSourceIndexSourceBlobs::from_normalized(vec![
            (
                agent_semantic_client_db::ClientDbSourceIndexPath::new("main.c"),
                main_bytes,
            ),
            (
                agent_semantic_client_db::ClientDbSourceIndexPath::new("dep.h"),
                dependency_bytes,
            ),
        ]);
    let source_evidence = workspace_snapshot.evidence(
        agent_semantic_content_identity::SourceSnapshotKind::Filesystem,
        agent_semantic_content_identity::provider_digest(b"c-family-native-test"),
    );
    let source_snapshot = agent_semantic_client::source_index::CurrentSourceIndexSnapshot {
        workspace_snapshot,
        source_snapshot: source_evidence,
        source_blobs,
    };
    let output = try_native_projection(
        NativeProjectionRequest {
            activation_root: &activation_root,
            project_root: &workspace,
            owner: Path::new("main.c"),
            language_id: "c",
            provider_id: "ccls-asp",
            artifact_stem: "ccls-asp-parser",
            abi_version: 1,
            parse_symbol: "ccls_asp_parse_translation_unit_with_args_v1",
            free_symbol: "ccls_asp_result_free_v1",
        },
        &source_snapshot,
    )
    .expect("run native projection")
    .expect("native projection selected");
    assert!(!output.language_projection.is_empty());
    let client_dir = root.join("client");
    agent_semantic_client_db::ClientDbEngine::import_semantic_structural_index_refresh_packet_from_client_dir(
        &client_dir,
        &output.generation,
        &output.structural_index_packet,
        &output.source_snapshot,
    )
    .expect("validate and persist stable structural-index v1");
    let structural: serde_json::Value =
        serde_json::from_slice(&output.structural_index_packet).expect("decode structural packet");

    assert!(
        structural["symbols"]
            .as_array()
            .expect("symbol rows")
            .iter()
            .any(|symbol| symbol["name"] == "answer")
    );
    assert!(
        structural["dependencyUsages"]
            .as_array()
            .expect("dependency rows")
            .iter()
            .any(|dependency| dependency["importPath"] == "dep.h")
    );
    assert!(
        agent_semantic_client_db::ClientDbEngine::turso_path_for_client_dir(&client_dir).exists()
    );

    std::fs::remove_dir_all(root).expect("remove structural smoke workspace");
}
