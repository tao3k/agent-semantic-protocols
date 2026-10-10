// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use super::compile_aot_hook_policy_bundle_projection;
use super::compile_serving_hook_policy_bundle_at;

#[test]
fn projection_compiler_is_deterministic_and_profile_driven() {
    let projection = serde_json::json!({
        "agentCalling": {"defaultPattern": "@{name}"},
        "commandActionPatterns": [
            {"action": "read", "argvPatternAny": [["head"], ["cat"]]}
        ],
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
    assert!(text.contains(
        r#""commandActionPatterns":[{"action":"read","argvPatternAny":[["cat"],["head"]]}]"#
    ));
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
        "registered-source Bash route must preserve wrapped_command as a required compiled fact"
    );
    assert!(
        generation["rules"]
            .as_array()
            .expect("compiled rules")
            .iter()
            .filter(|rule| rule["id"] == "route-read-to-asp-languages")
            .all(|rule| rule["matchers"] == serde_json::json!(["Bash"])),
        "registered-source Bash route must bind only the Bash Host action"
    );
    for (rule_id, matcher) in [
        ("route-org-document-read-to-asp-explorer", "Bash"),
        ("route-markdown-document-read-to-asp-explorer", "Bash"),
    ] {
        assert!(
            generation["rules"]
                .as_array()
                .expect("compiled rules")
                .iter()
                .any(|rule| {
                    rule["id"] == rule_id && rule["matchers"] == serde_json::json!([matcher])
                }),
            "document route {rule_id} must bind only Host action {matcher}"
        );
    }
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

#[test]
fn serving_compiler_applies_a_valid_state_home_config_overlay() {
    let root = tempfile::tempdir().expect("create State Home fixture");
    let config_path = root.path().join("config.toml");
    let source = agent_semantic_config::default_hook_client_config_template().replace(
        "Agent-facing search JSON is denied; use the compact ASP search route.",
        "State Home overlay policy is active.",
    );
    std::fs::write(&config_path, source).expect("write valid full Hook config");

    let bundle = compile_serving_hook_policy_bundle_at(Some(&config_path))
        .expect("compile State Home Hook config");
    let bundle: serde_json::Value = serde_json::from_slice(&bundle).expect("decode bundle");
    assert_ne!(
        bundle["generationDigest"],
        super::embedded_hook_policy_content_digest().expect("embedded identity"),
        "an admitted State Home overlay must have its own serving identity"
    );
    assert!(
        bundle["rules"]
            .as_array()
            .expect("rules")
            .iter()
            .any(|rule| {
                rule["id"] == "deny-agent-search-json"
                    && rule["message"] == "State Home overlay policy is active."
            })
    );
}

#[test]
fn serving_compiler_applies_state_home_profile_list_overlay() {
    let root = tempfile::tempdir().expect("create State Home fixture");
    let config_path = root.path().join("config.toml");
    let source = agent_semantic_config::default_hook_client_config_template().replace(
        "profilesList = [\"rust\", \"typescript\", \"python\", \"julia\", \"gerbil-scheme\"]",
        "profilesList = [\"rust\"]",
    );
    std::fs::write(&config_path, source).expect("write State Home profile-list config");

    let bundle = compile_serving_hook_policy_bundle_at(Some(&config_path))
        .expect("compile State Home Hook config");
    let bundle: serde_json::Value = serde_json::from_slice(&bundle).expect("decode bundle");
    let route_profiles = bundle["rules"]
        .as_array()
        .expect("rules")
        .iter()
        .filter(|rule| rule["id"] == "route-read-to-asp-languages")
        .map(|rule| rule["profile"].as_str().expect("route profile"))
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(route_profiles, std::collections::BTreeSet::from(["rust"]));
}
