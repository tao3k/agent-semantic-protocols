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

fn run_scenarios(_test_name: &str, scenarios: &[Scenario]) {
    let root = temp_project_root();
    fs::create_dir_all(root.join("src")).expect("create scenario source root");
    fs::write(root.join("src/app.ts"), "export const value = 1;\n")
        .expect("write registered source fixture");
    fs::write(root.join("README.md"), "# fixture\n").expect("write unregistered source fixture");
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
fn codex_payload_surfaces_are_equivalent() {
    const TEST_NAME: &str =
        "match_policy_contract::branch_coverage::scenarios::codex_payload_surfaces_are_equivalent";
    let command = "sed -n '1,8p' src/app.ts";
    let scenarios = [
        Scenario {
            name: "exec_command cmd",
            payload: shell_surface("exec_command", "cmd", command),
            expected_rule: Some("deny-uncontrolled-source-materialization-commands"),
            forbidden_rule: None,
        },
        Scenario {
            name: "functions.exec_command cmd",
            payload: shell_surface("functions.exec_command", "cmd", command),
            expected_rule: Some("deny-uncontrolled-source-materialization-commands"),
            forbidden_rule: None,
        },
        Scenario {
            name: "Bash command",
            payload: shell_surface("Bash", "command", command),
            expected_rule: Some("deny-uncontrolled-source-materialization-commands"),
            forbidden_rule: None,
        },
        Scenario {
            name: "functions.exec nested exec_command cmd",
            payload: json!({
                "tool_name": "functions.exec",
                "tool_input": {
                    "code": "await tools.exec_command({cmd: \"sed -n '1,8p' src/app.ts\"})"
                },
            }),
            expected_rule: Some("deny-uncontrolled-source-materialization-commands"),
            forbidden_rule: None,
        },
        Scenario {
            name: "functions.exec nested exec_command command",
            payload: json!({
                "tool_name": "functions.exec",
                "tool_input": {
                    "code": "await tools.exec_command({command: \"sed -n '1,8p' src/app.ts\"});"
                },
            }),
            expected_rule: Some("deny-uncontrolled-source-materialization-commands"),
            forbidden_rule: None,
        },
        Scenario {
            name: "functions.exec nested single-quoted cmd",
            payload: json!({
                "tool_name": "functions.exec",
                "tool_input": {
                    "code": "await tools.exec_command({cmd: 'sed -n 1,8p src/app.ts'});"
                },
            }),
            expected_rule: Some("deny-uncontrolled-source-materialization-commands"),
            forbidden_rule: None,
        },
        Scenario {
            name: "functions.exec current codex envelope",
            payload: json!({
                "tool_name": "functions.exec",
                "tool_input": {
                    "code": "const r = await tools.exec_command({cmd: \"sed -n '1,8p' src/app.ts\", workdir: \"/workspace\", yield_time_ms: 10000}); text(JSON.stringify(r));"
                },
            }),
            expected_rule: Some("deny-uncontrolled-source-materialization-commands"),
            forbidden_rule: None,
        },
        Scenario {
            name: "functions.exec freeform codex envelope",
            payload: json!({
                "tool_name": "functions.exec",
                "tool_input": "const r = await tools.exec_command({cmd: \"sed -n '1,8p' src/app.ts\", workdir: \"/workspace\", yield_time_ms: 10000}); text(r);",
            }),
            expected_rule: Some("deny-uncontrolled-source-materialization-commands"),
            forbidden_rule: None,
        },
        Scenario {
            name: "functions.exec multiple nested calls",
            payload: json!({
                "tool_name": "functions.exec",
                "tool_input": {
                    "code": "await tools.exec_command({cmd: \"true\"}); await tools.exec_command({cmd: \"cat src/app.ts\"});"
                },
            }),
            expected_rule: Some("deny-uncontrolled-source-materialization-commands"),
            forbidden_rule: None,
        },
        Scenario {
            name: "structured Read",
            payload: json!({
                "tool_name": "Read",
                "tool_input": {"file_path": "src/app.ts"},
            }),
            expected_rule: Some("materialize-registered-source-read-action"),
            forbidden_rule: None,
        },
        Scenario {
            name: "structured Grep",
            payload: json!({
                "tool_name": "Grep",
                "tool_input": {"pattern": "value", "path": "src/app.ts"},
            }),
            expected_rule: Some("deny-raw-registered-source-search-action"),
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
            expected_rule: Some("materialize-apply-patch-policy"),
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
        forbidden_rule: Some("deny-uncontrolled-source-materialization-commands"),
    });
    run_scenarios(TEST_NAME, &scenarios);
}

#[test]
fn every_rule_has_a_near_miss() {
    const TEST_NAME: &str =
        "match_policy_contract::branch_coverage::scenarios::every_rule_has_a_near_miss";
    let scenarios = [
        (
            "registered-asp-reasoning-search",
            shell("asp rust query --selector item"),
        ),
        (
            "resident-testing-dispatch",
            shell("cargo metadata --no-deps"),
        ),
        (
            "deny-raw-registered-source-search-action",
            json!({"tool_name":"Grep","tool_input":{"pattern":"value","path":"README.md"}}),
        ),
        ("deny-raw-registered-source-action", shell("read README.md")),
        (
            "deny-agent-search-json",
            shell("ts-harness search lexical projectRoot owner tests ."),
        ),
        (
            "materialize-apply-patch-policy",
            json!({"tool_name":"apply_patch_preview","tool_input":{"path":"src/app.ts"}}),
        ),
        (
            "materialize-registered-source-read-action",
            json!({"tool_name":"Read","tool_input":{"file_path":"README.md"}}),
        ),
        (
            "materialize-source-access-policy",
            shell("custom-reader '.read_text(' README.md"),
        ),
        (
            "deny-uncontrolled-source-search-commands",
            shell("printf 'rg value src/app.ts'"),
        ),
        (
            "allow-bounded-json-projection",
            shell("jq '.' package.json"),
        ),
        ("allow-bounded-toml-projection", shell("yq '.' Cargo.toml")),
        (
            "deny-unbounded-structured-projection",
            shell("jq -c '.package.name' package.json"),
        ),
        (
            "deny-uncontrolled-source-materialization-commands",
            shell("sed -n '1,8p' README.md"),
        ),
        ("deny-uncontrolled-git-source-reads", shell("git status")),
    ]
    .map(|(rule, payload)| Scenario {
        name: rule,
        payload,
        expected_rule: None,
        forbidden_rule: Some(rule),
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
            "registered-asp-reasoning-search",
        ),
        (
            "Grep action over raw shell search",
            json!({"tool_name":"Grep","tool_input":{"pattern":"value","path":"src/app.ts"}}),
            "deny-raw-registered-source-search-action",
        ),
        (
            "action-first read over legacy python inline materialization",
            shell("python -c 'from pathlib import Path; print(Path(\"src/app.ts\").read_text())'"),
            "materialize-source-access-policy",
        ),
        (
            "javascript inline over generic materialization",
            shell("node -e 'require(\"fs\").readFileSync(\"src/app.ts\", \"utf8\")'"),
            "materialize-source-access-policy",
        ),
        (
            "materialization over raw action",
            shell("sed -n '1,8p' src/app.ts"),
            "deny-uncontrolled-source-materialization-commands",
        ),
        (
            "git over raw action",
            shell("git show HEAD:src/app.ts"),
            "deny-uncontrolled-git-source-reads",
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
