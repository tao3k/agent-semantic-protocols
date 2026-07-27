use crate::command::c_family_native::{installed_library_path, platform_library_filename};
use std::path::Path;

#[test]
fn native_library_path_is_platform_specific_and_install_relative() {
    let filename = platform_library_filename("ccls-asp-parser");
    assert!(filename.contains("ccls-asp-parser"));
    assert_eq!(
        installed_library_path(Path::new("/activation"), "ccls-asp-parser"),
        Path::new("/activation").join("lib").join(filename)
    );
}

#[cfg(unix)]
#[test]
#[ignore = "requires a freshly built ccls-asp shared library"]
fn rust_loader_parses_one_explicit_translation_unit() {
    let library_path =
        std::env::var_os("CCLS_ASP_TEST_LIBRARY").expect("set CCLS_ASP_TEST_LIBRARY");
    let workspace =
        std::env::temp_dir().join(format!("asp-c-family-native-smoke-{}", std::process::id()));
    std::fs::create_dir_all(&workspace).expect("create smoke workspace");
    let source_path = workspace.join("main.c");
    std::fs::write(&source_path, "int answer(void) { return 42; }\n")
        .expect("write C translation unit");

    let library = super::c_family_native::NativeParserLibrary::open(
        Path::new(&library_path),
        1,
        "ccls_asp_parse_translation_unit_with_args_v1",
        "ccls_asp_result_free_v1",
    )
    .expect("open ccls-asp shared library");
    let result = library
        .parse_translation_unit(
            &workspace,
            "main.c",
            "c",
            &["-xc".to_string(), "-std=c17".to_string()],
        )
        .expect("parse one explicit translation unit");

    assert!(result.errors.is_empty(), "{:?}", result.errors);
    assert_eq!(result.translation_units, ["main.c"]);
    assert!(
        result
            .facts
            .iter()
            .any(|fact| fact.name == "answer" && fact.kind == "function")
    );

    std::fs::remove_file(source_path).expect("remove smoke source");
    std::fs::remove_dir(workspace).expect("remove smoke workspace");
}
