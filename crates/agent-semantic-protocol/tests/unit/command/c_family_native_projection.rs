use std::path::Path;

use super::c_family_native::{NativeFact, NativeParseResult, NativeSourceRange};
use super::c_family_native_projection::{
    NativeProjectionRequest, native_projection_json, normalize_compile_args, split_shell_words,
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

    let bytes = native_projection_json(request, result).expect("shape projection");
    let json = std::str::from_utf8(&bytes).expect("UTF-8 projection");
    let projection = agent_semantic_client_db::ClientDbLanguageProjection::from_json(json)
        .expect("valid shared projection v1");

    assert_eq!(projection.language_id(), "c");
    assert_eq!(projection.sources()[0].path, "src/main.c");
}
