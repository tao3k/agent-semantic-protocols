use super::compile_aot_hook_policy_bundle_projection;

#[test]
fn projection_compiler_is_deterministic_and_profile_driven() {
    let projection = serde_json::json!({
        "agentCalling": {"defaultPattern": "@{name}"},
        "readerBehaviorPatterns": [["head"], ["cat"]],
        "profiles": {"rust": {"languageId": "rust", "extensionAny": ["rs", "rsx"]}},
        "rules": [{
            "id": "route-source",
            "matcher": "Bash",
            "actions": ["read"],
            "profilesList": ["rust"],
            "decision": "deny",
            "reasonKind": "registered-source-route-required",
            "message": "Use ASP.",
            "dispatch": {"agent": "asp_explorer"}
        }]
    });
    let first = compile_aot_hook_policy_bundle_projection(&projection, "digest:g1")
        .expect("compile generation");
    let second = compile_aot_hook_policy_bundle_projection(&projection, "digest:g1")
        .expect("compile generation");
    assert_eq!(first, second);
    let text = String::from_utf8(first).expect("UTF-8 generation");
    assert!(text.contains(r#""registeredExtensions":["rs","rsx"]"#));
    assert!(text.contains(r#""readerBehaviorPatterns":[["cat"],["head"]]"#));
    assert!(text.contains(r#""actions":["read"]"#));
    assert!(
        text.contains(r#""wrappedCommand":true"#),
        "Read actions enable wrapped command observation by default"
    );
    assert!(text.contains(r#""route":"asp_explorer""#));
    assert!(text.contains(r#""agentCallingPattern":"@{name}""#));
}

#[test]
fn profile_list_expands_to_language_specific_rules_and_dispatch_route() {
    let projection = serde_json::json!({
        "agentCalling": {"defaultPattern": "@{name}"},
        "profiles": {
            "rust": {"languageId": "rust", "extensionAny": ["rs"]},
            "typescript": {"languageId": "typescript", "extensionAny": ["ts", "tsx"]}
        },
        "rules": [{
            "id": "route-read",
            "matcher": "Bash",
            "actions": ["read"],
            "profilesList": ["rust", "typescript"],
            "decision": "deny",
            "reasonKind": "registered-source-route-required",
            "message": "Use ASP.",
            "dispatch": {"agent": "asp_explorer"}
        }]
    });
    let bytes = compile_aot_hook_policy_bundle_projection(&projection, "digest:g1")
        .expect("compile multi-profile generation");
    let generation: serde_json::Value =
        serde_json::from_slice(&bytes).expect("decode compiled generation");
    let rules = generation["rules"].as_array().expect("compiled rules");
    assert_eq!(rules.len(), 2);
    let by_profile = rules
        .iter()
        .map(|rule| (rule["profile"].as_str().expect("compiled profile"), rule))
        .collect::<std::collections::BTreeMap<_, _>>();
    let rust = by_profile.get("rust").expect("Rust rule");
    assert_eq!(rust["language"], "rust");
    assert_eq!(rust["registeredExtensions"], serde_json::json!(["rs"]));
    assert_eq!(rust["route"], "asp_explorer");
    assert_eq!(rust["actions"], serde_json::json!(["read"]));
    let typescript = by_profile.get("typescript").expect("TypeScript rule");
    assert_eq!(typescript["language"], "typescript");
    assert_eq!(
        typescript["registeredExtensions"],
        serde_json::json!(["ts", "tsx"])
    );
}

#[test]
fn canonical_config_rules_are_all_projected_into_the_aot_generation() {
    let config = agent_semantic_config::default_hook_client_config_file()
        .expect("load canonical Hook Config V1");
    let source = serde_json::to_value(&config).expect("project canonical Hook config");
    let expected = source["rules"]
        .as_array()
        .expect("canonical rules")
        .iter()
        .map(|rule| rule["id"].as_str().expect("rule id"))
        .collect::<std::collections::BTreeSet<_>>();
    let compiled = super::compile_aot_hook_policy_bundle(&config, "digest:canonical")
        .expect("compile canonical AOT generation");
    let generation: serde_json::Value =
        serde_json::from_slice(&compiled).expect("decode canonical AOT generation");
    let actual = generation["rules"]
        .as_array()
        .expect("compiled rules")
        .iter()
        .map(|rule| rule["id"].as_str().expect("compiled rule id"))
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(
        actual, expected,
        "AOT compiler silently dropped Config rules"
    );
    assert!(
        generation["rules"]
            .as_array()
            .expect("compiled rules")
            .iter()
            .filter(|rule| rule["id"] == "route-read-to-asp-languages")
            .all(|rule| rule["wrappedCommand"] == true),
        "registered-source Read routes must preserve wrapped_command as a required compiled fact"
    );
    assert_eq!(
        generation["registeredLanguages"],
        serde_json::json!([
            "gerbil-scheme",
            "julia",
            "md",
            "org",
            "python",
            "rust",
            "typescript"
        ])
    );
}

#[test]
fn embedded_identity_projection_matches_the_compiled_generation() {
    let expected = super::embedded_hook_policy_content_digest()
        .expect("project embedded Hook policy content identity");
    let bundle =
        super::compile_embedded_hook_policy_bundle().expect("compile embedded Hook policy bundle");
    let bundle: serde_json::Value =
        serde_json::from_slice(&bundle).expect("decode embedded Hook policy bundle");
    assert_eq!(bundle["generationDigest"], expected);
}
