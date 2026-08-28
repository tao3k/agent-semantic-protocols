use super::compile_aot_hook_generation_projection;

#[test]
fn projection_compiler_is_deterministic_and_profile_driven() {
    let projection = serde_json::json!({
        "profiles": {"rust": {"extensions": ["rs", ".rs", "rsx"]}},
        "rules": [{
            "id": "route-source",
            "matcher": "Bash",
            "profilesList": ["rust"],
            "decision": "deny",
            "reasonKind": "registered-source-route-required",
            "message": "Use ASP.",
            "route": "asp languages"
        }]
    });
    let first = compile_aot_hook_generation_projection(&projection, "digest:g1")
        .expect("compile generation");
    let second = compile_aot_hook_generation_projection(&projection, "digest:g1")
        .expect("compile generation");
    assert_eq!(first, second);
    let text = String::from_utf8(first).expect("UTF-8 generation");
    assert!(text.contains(r#""registeredExtensions":["rs","rsx"]"#));
    assert!(!text.contains("cat"));
    assert!(!text.contains("head"));
}
