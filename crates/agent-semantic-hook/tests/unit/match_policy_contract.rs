// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use agent_semantic_hook::DecisionKind;
use agent_semantic_hook::HookClassificationRequest;
use agent_semantic_hook::ReasonKind;
use agent_semantic_hook::classify_hook_with_config;
use agent_semantic_hook::default_client_config_template;
use agent_semantic_hook::load_client_config_for_project_with_executable_capabilities;
use serde_json::Value;
use serde_json::json;
use std::collections::BTreeSet;
use std::fs;
use std::path::PathBuf;

use super::classifier::builtin_programming_runtime;
use super::classifier::registry;

fn temp_project_root() -> PathBuf {
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("match-policy fixture clock")
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "agent-semantic-hook-match-policy-contract-{}-{nonce}",
        std::process::id()
    ));
    let agents_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("agents");
    let fixture_agents = root.join("agents");
    fs::create_dir_all(&fixture_agents).expect("create match-policy fixture agent registry");
    for name in [
        "config.toml",
        "asp_explorer_codex.toml",
        "asp_explorer_claude.md",
        "asp_testing_codex.toml",
        "asp_testing_claude.md",
    ] {
        fs::copy(agents_root.join(name), fixture_agents.join(name))
            .expect("copy match-policy fixture agent registry entry");
    }
    root
}

#[test]
fn canonical_config_covers_registered_source_bash_matrix() {
    let root = temp_project_root();
    fs::create_dir_all(&root).expect("matrix root");
    let config_path = root.join("config.toml");
    fs::write(&config_path, default_client_config_template()).expect("write config");
    fs::write(root.join("package.json"), r#"{"name":"fixture"}"#)
        .expect("write structured projection fixture");
    let profiles =
        agent_semantic_config::default_hook_client_config_file().expect("canonical config");
    let registered_source_profiles = profiles
        .rules
        .iter()
        .find(|rule| rule.id == "route-read-to-asp-languages")
        .expect("registered source Bash rule")
        .profiles_list
        .iter()
        .collect::<BTreeSet<_>>();
    let config = agent_semantic_hook::load_client_config_for_project(&config_path, &root)
        .expect("compile config");
    let mut runtime = builtin_programming_runtime();
    runtime.project_root = root.to_string_lossy().into_owned();
    let mut count = 0usize;
    for (profile_id, profile) in &profiles.profiles {
        if !registered_source_profiles.contains(profile_id) {
            continue;
        }
        for extension in &profile.extension_any {
            let path = format!("src/witness.{extension}");
            let mut payload = json!({
                "tool_name": "Bash",
                "tool_input": {"command": format!("head {path}")}
            });
            agent_semantic_hook::bind_plugin_host_matcher(&mut payload, "Bash")
                .expect("bind canonical Bash matcher");
            let observation = agent_semantic_hook::ReaderProbeObservation {
                subject: path.clone(),
                access: agent_semantic_hook::ReaderProbeAccess::Read,
                backend: "hook-policy-bundle-reader-catalog".to_owned(),
                terminal: "reader-behavior-catalog-hit".to_owned(),
                elapsed_micros: 0,
                probe_process_launched: false,
                cleanup_verified: true,
                cache_hit: false,
                behavior_key: None,
            };
            agent_semantic_hook::bind_reader_probe_observation(&mut payload, Some(&observation))
                .expect("bind confirmed Reader observation");
            {
                let decision = classify_hook_with_config(HookClassificationRequest {
                    registry: &runtime,
                    config: &config,
                    platform: "codex",
                    event: "pre-tool",
                    payload: &payload,
                });
                assert_eq!(
                    decision.fields.get("configRuleId").and_then(Value::as_str),
                    Some("route-read-to-asp-languages"),
                    "{payload}"
                );
                assert_eq!(decision.decision, DecisionKind::Deny);
                assert_eq!(
                    decision.reason_kind,
                    ReasonKind::RegisteredSourceRouteRequired
                );
                assert_eq!(decision.language_ids, [profile.language_id.as_str()]);
                assert!(
                    decision
                        .routes
                        .iter()
                        .any(|route| route.provider_id == profile.provider_id.as_str())
                );
                for route in &decision.routes {
                    assert_eq!(
                        route.argv.len(),
                        4,
                        "Search route must carry one expression"
                    );
                    agent_semantic_search::parse_progressive_search_playbook_args(&route.argv[1..])
                        .expect("materialized Search route must satisfy one-expression admission");
                }
                count += 1;
            }
        }
    }
    assert!(count > 0);
    fs::remove_dir_all(root).expect("cleanup matrix root");
}

#[test]
fn bundled_plugin_matchers_preserve_one_host_action_identity_per_entry() {
    let hooks: Value = serde_json::from_str(include_str!(
        "../../../../asp-codex-plugin/hooks/hooks.json"
    ))
    .expect("plugin hooks JSON");
    let matchers = hooks["hooks"]["PreToolUse"]
        .as_array()
        .expect("PreToolUse entries")
        .iter()
        .map(|entry| entry["matcher"].as_str().expect("native matcher"))
        .collect::<Vec<_>>();
    assert_eq!(&matchers[..3], ["^apply_patch$", "Bash", "spawn_agent"]);
    assert!(
        matchers[3..]
            .iter()
            .all(|matcher| matcher.starts_with("mcp__codex_app__"))
    );
    assert!(matchers.contains(&"mcp__codex_app__send_message_to_thread"));
    assert!(matchers.contains(&"mcp__codex_app__automation_update"));
    assert!(!matchers.contains(&"mcp__codex_app__consume_usage_reset"));
    assert!(!matchers.contains(&"mcp__codex_app__uninstall_plugin"));
    assert!(!matchers.contains(&"*"));
    assert!(!matchers.contains(&"^mcp__.*$"));
}

#[test]
fn embedded_hook_policy_does_not_require_project_agent_routes() {
    let project_root = std::env::temp_dir().join(format!(
        "asp-hook-policy-without-agent-routes-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system clock")
            .as_nanos()
    ));
    std::fs::create_dir_all(&project_root).expect("create project root without agents config");

    let config = agent_semantic_hook::load_embedded_client_config_for_project(&project_root)
        .expect("embedded hook policy must load without project agent routes");
    agent_semantic_hook::validate_match_policy_rule_coverage(&config)
        .expect("embedded hook policy remains conformant");

    let config_path = project_root.join("hook-config.toml");
    std::fs::write(
        &config_path,
        agent_semantic_hook::default_client_config_template(),
    )
    .expect("write installed hook policy fixture");
    let config = agent_semantic_hook::load_client_config_for_project(&config_path, &project_root)
        .expect("installed hook policy must load without project agent routes");
    agent_semantic_hook::validate_match_policy_rule_coverage(&config)
        .expect("installed hook policy remains conformant");

    std::fs::remove_dir_all(project_root).ok();
}

fn configured_executable_capabilities(template: &str) -> BTreeSet<String> {
    let config: agent_semantic_config::HookClientConfigFile =
        toml::from_str(template).expect("production Hook template parses");
    config
        .rules
        .into_iter()
        .filter_map(|rule| rule.match_config.structured_projection)
        .map(|projection| projection.binary)
        .collect()
}

fn load_policy_with_configured_capabilities(
    config_path: &std::path::Path,
    project_root: &std::path::Path,
    template: &str,
) -> agent_semantic_hook::ClientHookConfig {
    load_client_config_for_project_with_executable_capabilities(
        config_path,
        project_root,
        configured_executable_capabilities(template),
    )
    .expect("compile policy with config-derived executable capability snapshot")
}

fn shell(command: &str) -> Value {
    json!({
        "tool_name": "Bash",
        "tool_input": {"command": command},
    })
}

fn classify<'a>(
    runtime: &'a agent_semantic_hook::HookRuntime,
    config: &'a agent_semantic_hook::ClientHookConfig,
    payload: &'a Value,
) -> agent_semantic_hook::HookDecision {
    let mut payload = payload.clone();
    let tool_name = payload
        .get("tool_name")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_owned();
    let binding = match tool_name.as_str() {
        "apply_patch" | "Bash" | "spawn_agent" => {
            agent_semantic_hook::bind_plugin_host_matcher(&mut payload, tool_name.as_str())
        }
        name if name.starts_with("mcp__") => {
            agent_semantic_hook::bind_plugin_host_matcher(&mut payload, name)
        }
        _ => Ok(()),
    };
    binding.expect("bind canonical Host matcher in production scenario");
    classify_hook_with_config(HookClassificationRequest {
        registry: runtime,
        config,
        platform: "codex",
        event: "pre-tool",
        payload: &payload,
    })
}

#[test]
fn production_match_policy_contract() {
    let root = temp_project_root();
    fs::create_dir_all(&root).expect("create match-policy contract root");
    let config_path = root.join("config.toml");
    let template = default_client_config_template();
    fs::write(&config_path, &template).expect("write production hook config");
    fs::write(
        root.join("package.json"),
        "{\"package\":{\"name\":\"hook\"}}\n",
    )
    .expect("write JSON projection fixture");
    fs::write(root.join("Cargo.toml"), "[package]\nname = \"hook\"\n")
        .expect("write TOML projection fixture");

    let config = load_policy_with_configured_capabilities(&config_path, &root, &template);
    let rule_ids = config.rule_ids().collect::<BTreeSet<_>>();
    assert_eq!(config.rule_count(), rule_ids.len());

    let mut runtime = registry();
    runtime.project_root = root.to_string_lossy().into_owned();
    config
        .apply_language_provider_projection(&mut runtime)
        .expect("apply declarative language profile projection");
    assert!(
        runtime
            .policy_providers
            .iter()
            .any(|provider| provider.language_id == "typescript"),
        "typescript profile must be projected from the declarative config"
    );

    let syntax_only_config = load_client_config_for_project_with_executable_capabilities(
        &config_path,
        &root,
        BTreeSet::<String>::new(),
    )
    .expect("compile policy without projector executables");
    let bounded_json_probe = classify(
        &runtime,
        &syntax_only_config,
        &shell("jq -c '.package.name' package.json"),
    );
    assert_eq!(
        bounded_json_probe
            .fields
            .get("configRuleId")
            .and_then(Value::as_str),
        Some("allow-bounded-json-projection"),
        "bounded projection authorization must not depend on the pre-hook PATH snapshot"
    );

    agent_semantic_hook::validate_match_policy_rule_coverage(&config)
        .expect("runtime-free mmap rule coverage gate");
    fs::remove_dir_all(root).expect("cleanup match-policy contract root");
}

#[path = "match_policy_contract/branch_coverage.rs"]
mod branch_coverage;
