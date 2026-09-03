use std::fs;

use serde_json::{Value, json};

use super::{
    classify, default_client_config_template, load_policy_with_configured_capabilities,
    normalized_agent_action, registry, shell, shell_surface, temp_project_root,
};

struct Scenario {
    name: &'static str,
    payload: Value,
    expected_rule: Option<&'static str>,
    forbidden_rule: Option<&'static str>,
}

#[test]
fn registered_asp_search_intent_is_stable_across_native_wrapped_and_nested_surfaces() {
    let command = "asp rust search playbook 'HookDecision' --workspace .";
    run_scenarios(
        "registered ASP search surfaces",
        &[
            Scenario {
                name: "native exec command",
                payload: shell_surface("exec_command", "cmd", command),
                expected_rule: Some("registered-asp-reasoning-search"),
                forbidden_rule: None,
            },
            Scenario {
                name: "wrapped Bash command",
                payload: shell_surface(
                    "Bash",
                    "command",
                    "asp rust search playbook 'HookDecision' --workspace .",
                ),
                expected_rule: Some("registered-asp-reasoning-search"),
                forbidden_rule: None,
            },
            Scenario {
                name: "testing lane wrapped command",
                payload: shell_surface(
                    "Bash",
                    "command",
                    "rtk --ultra-compact err asp rust search playbook 'HookDecision' --workspace .",
                ),
                expected_rule: Some("registered-asp-reasoning-search"),
                forbidden_rule: None,
            },
            Scenario {
                name: "nested functions exec command",
                payload: serde_json::json!({
                    "tool_name": "functions.exec",
                    "tool_input": {
                        "code": "const r = await tools.exec_command({cmd: \"asp rust search playbook 'HookDecision' --workspace .\"});"
                    }
                }),
                expected_rule: Some("registered-asp-reasoning-search"),
                forbidden_rule: None,
            },
        ],
    );
}

#[test]
fn registered_asp_query_projects_structured_projection_across_codex_surfaces() {
    let command = "asp rust query --selector rust://crates/example.rs#item/function/example --workspace . --projection source";
    run_scenarios(
        "registered ASP structured projection surfaces",
        &[
            Scenario {
                name: "native exact query",
                payload: shell_surface("exec_command", "cmd", command),
                expected_rule: Some("registered-asp-structured-projection"),
                forbidden_rule: None,
            },
            Scenario {
                name: "wrapped exact query",
                payload: shell_surface(
                    "Bash",
                    "command",
                    "rtk --ultra-compact err asp rust query --selector rust://crates/example.rs#item/function/example --workspace . --projection source",
                ),
                expected_rule: Some("registered-asp-structured-projection"),
                forbidden_rule: None,
            },
            Scenario {
                name: "nested exact query",
                payload: serde_json::json!({
                    "tool_name": "functions.exec",
                    "tool_input": {
                        "code": "const r = await tools.exec_command({cmd: \"asp rust query --selector rust://crates/example.rs#item/function/example --workspace . --projection source\"});"
                    }
                }),
                expected_rule: Some("registered-asp-structured-projection"),
                forbidden_rule: None,
            },
        ],
    );
}

fn run_scenarios(_test_name: &str, scenarios: &[Scenario]) {
    let root = temp_project_root();
    fs::create_dir_all(root.join("src")).expect("create scenario source root");
    fs::write(root.join("src/app.ts"), "export const value = 1;\n")
        .expect("write registered source fixture");
    fs::write(root.join("src/app.py"), "value = 1\n").expect("write Python source fixture");
    fs::write(root.join("src/app.jl"), "value = 1\n").expect("write Julia source fixture");
    fs::write(root.join("src/app.ss"), "(define value 1)\n")
        .expect("write Gerbil Scheme source fixture");
    fs::write(
        root.join("package.json"),
        "{\"package\":{\"name\":\"hook\"}}\n",
    )
    .expect("write JSON projection fixture");
    fs::write(root.join("Cargo.toml"), "[package]\nname = \"hook\"\n")
        .expect("write TOML projection fixture");
    let config_path = root.join("config.toml");
    let template = default_client_config_template();
    fs::write(&config_path, &template).expect("write production hook config");
    let config = load_policy_with_configured_capabilities(&config_path, &root, &template);
    let mut runtime = registry();
    runtime.project_root = root.to_string_lossy().into_owned();

    let mut failures = Vec::new();
    for scenario in scenarios {
        let decision = classify(&runtime, &config, &scenario.payload);
        let actual = decision.fields.get("configRuleId").and_then(Value::as_str);
        if scenario.expected_rule.is_some() && actual != scenario.expected_rule {
            failures.push(format!(
                "{} expected={:?} actual={actual:?} action={} normalized={} payload={}",
                scenario.name,
                scenario.expected_rule,
                normalized_agent_action(&decision),
                decision
                    .fields
                    .get("normalizedActions")
                    .unwrap_or(&Value::Null),
                scenario.payload
            ));
        }
        if scenario.forbidden_rule.is_some() && actual == scenario.forbidden_rule {
            failures.push(format!(
                "{} unexpectedly matched forbidden rule={actual:?}; payload={}",
                scenario.name, scenario.payload
            ));
        }
    }
    fs::remove_dir_all(root).expect("cleanup scenario root");
    assert!(
        failures.is_empty(),
        "production match-policy scenario contract failed:\n{}",
        failures.join("\n")
    );
}

#[test]
fn only_canonical_host_matchers_activate_native_policy_rules() {
    const TEST_NAME: &str =
        "match_policy_contract::branch_coverage::scenarios::codex_payload_surfaces_are_equivalent";
    let command = "unknown-consumer < src/app.ts";
    let scenarios = [
        Scenario {
            name: "exec_command cmd",
            payload: shell_surface("exec_command", "cmd", command),
            expected_rule: None,
            forbidden_rule: None,
        },
        Scenario {
            name: "functions.exec_command cmd",
            payload: shell_surface("functions.exec_command", "cmd", command),
            expected_rule: None,
            forbidden_rule: None,
        },
        Scenario {
            name: "Bash command",
            payload: shell_surface("Bash", "command", command),
            expected_rule: Some("route-read-to-asp-languages"),
            forbidden_rule: None,
        },
        Scenario {
            name: "functions.exec nested exec_command cmd",
            payload: json!({
                "tool_name": "functions.exec",
                "tool_input": {
            "code": "await tools.exec_command({cmd: \"unknown-consumer < src/app.ts\"})"
                },
            }),
            expected_rule: None,
            forbidden_rule: None,
        },
        Scenario {
            name: "functions.exec nested exec_command command",
            payload: json!({
                "tool_name": "functions.exec",
                "tool_input": {
            "code": "await tools.exec_command({command: \"unknown-consumer < src/app.ts\"});"
                },
            }),
            expected_rule: None,
            forbidden_rule: None,
        },
        Scenario {
            name: "functions.exec nested single-quoted cmd",
            payload: json!({
                "tool_name": "functions.exec",
                "tool_input": {
            "code": "await tools.exec_command({cmd: 'unknown-consumer < src/app.ts'});"
                },
            }),
            expected_rule: None,
            forbidden_rule: None,
        },
        Scenario {
            name: "functions.exec current codex envelope",
            payload: json!({
                "tool_name": "functions.exec",
                "tool_input": {
            "code": "const r = await tools.exec_command({cmd: \"unknown-consumer < src/app.ts\", workdir: \"/workspace\", yield_time_ms: 10000}); text(JSON.stringify(r));"
                },
            }),
            expected_rule: None,
            forbidden_rule: None,
        },
        Scenario {
            name: "functions.exec freeform codex envelope",
            payload: json!({
                    "tool_name": "functions.exec",
            "tool_input": "const r = await tools.exec_command({cmd: \"unknown-consumer < src/app.ts\", workdir: \"/workspace\", yield_time_ms: 10000}); text(r);",
                }),
            expected_rule: None,
            forbidden_rule: None,
        },
        Scenario {
            name: "functions.exec multiple nested calls",
            payload: json!({
                "tool_name": "functions.exec",
                "tool_input": {
            "code": "await tools.exec_command({cmd: \"true\"}); await tools.exec_command({cmd: \"unknown-consumer < src/app.ts\"});"
                },
            }),
            expected_rule: None,
            forbidden_rule: None,
        },
        Scenario {
            name: "TypeScript unresolved Bash source access",
            payload: shell("opaque-source-consumer src/app.ts"),
            expected_rule: None,
            forbidden_rule: None,
        },
        Scenario {
            name: "Python unresolved Bash source access",
            payload: shell("opaque-source-consumer src/app.py"),
            expected_rule: None,
            forbidden_rule: None,
        },
        Scenario {
            name: "Julia unresolved Bash source access",
            payload: shell("opaque-source-consumer src/app.jl"),
            expected_rule: None,
            forbidden_rule: None,
        },
        Scenario {
            name: "Gerbil Scheme unresolved Bash source access",
            payload: shell("opaque-source-consumer src/app.ss"),
            expected_rule: None,
            forbidden_rule: None,
        },
        Scenario {
            name: "JSON input redirection stays on structured projector route",
            payload: shell("opaque-source-consumer < package.json"),
            expected_rule: Some("route-shell-structured-document-read"),
            forbidden_rule: Some("route-read-to-asp-languages"),
        },
        Scenario {
            name: "structured Grep",
            payload: json!({
                "tool_name": "Grep",
                "tool_input": {"pattern": "value", "path": "src/app.ts"},
            }),
            expected_rule: None,
            forbidden_rule: None,
        },
        Scenario {
            name: "structured apply_patch",
            payload: json!({
                "tool_name": "apply_patch",
                "tool_input": {
                    "patch": "*** Begin Patch\n*** Update File: src/app.ts\n@@\n-old\n+new\n*** End Patch\n"
                },
            }),
            expected_rule: None,
            forbidden_rule: None,
        },
    ];
    run_scenarios(TEST_NAME, &scenarios);
}

#[test]
fn functions_exec_code_parser_rejects_near_misses() {
    const TEST_NAME: &str = "match_policy_contract::branch_coverage::scenarios::functions_exec_code_parser_rejects_near_misses";
    let snippets = [
        (
            "call text inside a string",
            "\"await tools.exec_command({cmd: \\\"sed -n '1,8p' src/app.ts\\\"})\"",
        ),
        (
            "dynamic cmd",
            "await tools.exec_command({cmd: sourceCommand})",
        ),
        (
            "non tools receiver",
            "await wrapper.exec_command({cmd: \"sed -n '1,8p' src/app.ts\"})",
        ),
        (
            "non object argument",
            "await tools.exec_command(\"sed -n '1,8p' src/app.ts\")",
        ),
    ];
    let scenarios = snippets.map(|(name, code)| Scenario {
        name,
        payload: json!({
            "tool_name": "functions.exec",
            "tool_input": {"code": code},
        }),
        expected_rule: None,
        forbidden_rule: Some("route-read-to-asp-languages"),
    });
    run_scenarios(TEST_NAME, &scenarios);
}

#[test]
fn priority_overlaps_have_explicit_winners() {
    const TEST_NAME: &str = "match_policy_contract::branch_coverage::scenarios::priority_overlaps_have_explicit_winners";
    let scenarios = [
        (
            "reasoning over json",
            shell("asp rust search --json --language rust"),
            "deny-agent-search-json",
        ),
        (
            "action-first parser read over python profile",
            shell("unknown-consumer < src/app.py"),
            "route-read-to-asp-languages",
        ),
        (
            "parser read over typescript profile",
            shell("unknown-consumer < src/app.ts"),
            "route-read-to-asp-languages",
        ),
        (
            "parser read over rust profile",
            shell("unknown-consumer < src/lib.rs"),
            "route-read-to-asp-languages",
        ),
        (
            "repository history intent dominates embedded source read",
            shell("git show HEAD:src/app.ts"),
            "git-history-inspection-dispatch",
        ),
        (
            "bounded allow over unbounded deny",
            shell("jq -c '.package.name' package.json"),
            "allow-bounded-json-projection",
        ),
    ]
    .map(|(name, payload, winner)| Scenario {
        name,
        payload,
        expected_rule: Some(winner),
        forbidden_rule: None,
    });
    run_scenarios(TEST_NAME, &scenarios);
}

#[test]
fn structured_projection_boundaries_are_explicit() {
    const TEST_NAME: &str = "match_policy_contract::branch_coverage::scenarios::structured_projection_boundaries_are_explicit";
    let scenarios = [
        ("identity", shell("jq '.' package.json")),
        ("recursive", shell("jq '..' package.json")),
        ("compound", shell("jq '.package, .name' package.json")),
        (
            "multiple input",
            shell("jq '.package.name' package.json package.json"),
        ),
        (
            "unknown option",
            shell("jq --unknown '.package.name' package.json"),
        ),
        ("multi stage", shell("jq '.package | .name' package.json")),
    ]
    .map(|(name, payload)| Scenario {
        name,
        payload,
        expected_rule: Some("deny-unbounded-structured-projection"),
        forbidden_rule: None,
    });
    let yq = [
        Scenario {
            name: "yq eval",
            payload: shell("yq eval '.package.name' Cargo.toml"),
            expected_rule: Some("allow-bounded-toml-projection"),
            forbidden_rule: None,
        },
        Scenario {
            name: "yq e",
            payload: shell("yq e '.package.name' Cargo.toml"),
            expected_rule: Some("allow-bounded-toml-projection"),
            forbidden_rule: None,
        },
        Scenario {
            name: "yq no subcommand",
            payload: shell("yq '.package.name' Cargo.toml"),
            expected_rule: Some("allow-bounded-toml-projection"),
            forbidden_rule: None,
        },
    ];
    let all = scenarios.into_iter().chain(yq).collect::<Vec<_>>();
    run_scenarios(TEST_NAME, &all);
}
