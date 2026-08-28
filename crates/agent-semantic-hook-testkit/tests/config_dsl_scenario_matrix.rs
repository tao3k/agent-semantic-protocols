use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use agent_semantic_hook::{ClientHookConfig, HookRuntime};
use agent_semantic_hook_testkit::classify_codex_plugin_scenario;
use serde_json::{Value, json};

const MATRIX: &str = include_str!("../../agent-semantic-config/templates/hooks/config-test.toml");
const MATRIX_SCHEMA: &str =
    include_str!("../../../schemas/semantic-hook-match-config-test.v1.schema.json");

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("canonical workspace root")
}

fn runtime() -> HookRuntime {
    HookRuntime {
        project_root: workspace_root().to_string_lossy().into_owned(),
        rankers: Vec::new(),
        providers: Vec::new(),
        policy_providers: Vec::new(),
    }
}

fn config() -> ClientHookConfig {
    let root = tempfile::tempdir().expect("temporary config root");
    let path = root.path().join("config.toml");
    std::fs::write(&path, agent_semantic_hook::default_client_config_template())
        .expect("materialize canonical config");
    agent_semantic_hook::load_client_config(&path).expect("compile canonical config")
}

fn classify_command(runtime: &HookRuntime, config: &ClientHookConfig, command: &str) -> Value {
    classify_codex_plugin_scenario(
        runtime,
        config,
        "pre-tool",
        &json!({"tool_name": "Bash", "tool_input": {"command": command}}),
        Some("Bash"),
        None,
    )
    .expect("classify canonical Bash scenario")
}

fn classify_tool(
    runtime: &HookRuntime,
    config: &ClientHookConfig,
    tool_name: &str,
    tool_input: Value,
) -> Value {
    let exact = matches!(tool_name, "apply_patch" | "Bash" | "spawn_agent").then_some(tool_name);
    let prefix = tool_name.starts_with("mcp__").then_some("mcp__");
    let payload = json!({"tool_name": tool_name, "tool_input": tool_input});
    if exact.is_some() || prefix.is_some() {
        classify_codex_plugin_scenario(runtime, config, "pre-tool", &payload, exact, prefix)
            .expect("classify canonical native-tool scenario")
    } else {
        agent_semantic_hook_testkit::classify_hook_scenario(
            runtime, config, "codex", "pre-tool", &payload,
        )
        .expect("classify non-installed negative tool scenario")
    }
}

fn rule_id(decision: &Value) -> Option<&str> {
    decision
        .pointer("/fields/configRuleId")
        .and_then(Value::as_str)
}

fn executable_is_available(binary: &str) -> bool {
    std::env::var_os("PATH").is_some_and(|path| {
        std::env::split_paths(&path)
            .map(|directory| directory.join(binary))
            .any(|candidate| candidate.is_file())
    })
}

fn render_wrapper(template: &str, command: &str) -> String {
    if template.contains("\"{command}\"") {
        let escaped = command
            .replace('\\', "\\\\")
            .replace('"', "\\\"")
            .replace('$', "\\$")
            .replace('`', "\\`");
        return template.replace("{command}", &escaped);
    }
    if template.contains("'{command}'") {
        return template.replace("{command}", &command.replace('\'', "'\"'\"'"));
    }
    template.replace("{command}", command)
}

fn assert_declared_winner(decision: &Value, rule: &toml::Value, case: &str) {
    let expected_rule = rule["id"].as_str().expect("production rule id");
    assert_eq!(rule_id(decision), Some(expected_rule), "case={case}");
    assert_eq!(
        decision["decision"].as_str(),
        rule["decision"].as_str(),
        "case={case}"
    );
    if let Some(reason) = rule.get("reasonKind").and_then(toml::Value::as_str) {
        assert_eq!(decision["reasonKind"].as_str(), Some(reason), "case={case}");
    }
    if let Some(dispatch) = rule.get("dispatch") {
        assert_eq!(
            decision
                .pointer("/fields/targetAgent")
                .and_then(Value::as_str),
            dispatch.get("agent").and_then(toml::Value::as_str),
            "case={case}"
        );
        assert_eq!(
            decision
                .pointer("/fields/receiptKind")
                .and_then(Value::as_str),
            dispatch.get("receiptKind").and_then(toml::Value::as_str),
            "case={case}"
        );
        assert_eq!(
            decision
                .pointer("/fields/choicePlaneOwner")
                .and_then(Value::as_str),
            Some("org-contract:agent-interactive"),
            "case={case}"
        );
        if decision["reasonKind"] == "subagent-receipt-required" {
            let symbol = decision
                .pointer("/fields/targetAgentSymbol")
                .and_then(Value::as_str)
                .expect("native agent symbol");
            let message = decision["message"].as_str().expect("dispatch message");
            assert!(message.contains(symbol), "case={case} message={message}");
            assert!(
                message.contains("asp session --agents choice-plane"),
                "case={case} message={message}"
            );
        }
        for legacy in [
            "residentName",
            "targetAgentName",
            "targetAgentKind",
            "targetAgentDisplayRole",
            "targetAgentDescription",
        ] {
            assert!(decision["fields"].get(legacy).is_none(), "case={case}");
        }
    }
}

#[test]
fn canonical_dsl_has_one_scenario_contract_for_every_rule_and_no_legacy_action_axis() {
    let production =
        toml::from_str::<toml::Value>(&agent_semantic_hook::default_client_config_template())
            .expect("parse canonical config");
    let matrix = toml::from_str::<toml::Value>(MATRIX).expect("parse canonical scenario matrix");
    let schema = serde_json::from_str::<Value>(MATRIX_SCHEMA).expect("parse matrix schema");
    let matrix_json = serde_json::to_value(&matrix).expect("project matrix JSON");
    let errors = jsonschema::validator_for(&schema)
        .expect("compile matrix schema")
        .iter_errors(&matrix_json)
        .map(|error| error.to_string())
        .collect::<Vec<_>>();
    assert!(errors.is_empty(), "matrix schema errors={errors:#?}");

    let production_rules = production["rules"].as_array().expect("production rules");
    let matrix_rules = matrix["rules"].as_array().expect("matrix rules");
    let production_ids = production_rules
        .iter()
        .map(|rule| rule["id"].as_str().expect("production rule id"))
        .collect::<BTreeSet<_>>();
    let matrix_ids = matrix_rules
        .iter()
        .map(|rule| rule["id"].as_str().expect("matrix rule id"))
        .collect::<BTreeSet<_>>();
    assert_eq!(matrix_ids, production_ids);
    assert_eq!(
        matrix_ids.len(),
        matrix_rules.len(),
        "duplicate matrix rule id"
    );

    for rule in production_rules {
        assert!(rule.get("hostAction").is_none());
        assert!(rule.get("semanticAction").is_none());
        assert!(rule.get("hostInvocations").is_none());
        assert!(rule["match"].get("actionAny").is_none());
        assert!(rule["match"].get("hostInvocationAny").is_none());
    }

    for witness in matrix_rules {
        let id = witness["id"].as_str().expect("matrix rule id");
        let production_rule = production_rules
            .iter()
            .find(|rule| rule["id"].as_str() == Some(id))
            .expect("matrix rule must resolve");
        let production_json =
            serde_json::to_value(production_rule).expect("project production rule JSON");
        for predicate in witness["ruleComposition"]
            .as_array()
            .expect("composition predicates")
        {
            let pointer = predicate["pointer"].as_str().expect("composition pointer");
            let actual = production_json
                .pointer(pointer)
                .unwrap_or_else(|| panic!("{id} unresolved pointer {pointer}"));
            if let Some(expected) = predicate.get("equals") {
                assert_eq!(
                    actual,
                    &serde_json::to_value(expected).expect("equals value")
                );
            }
            if let Some(expected) = predicate.get("contains") {
                let expected = serde_json::to_value(expected).expect("contains value");
                assert!(
                    actual
                        .as_array()
                        .is_some_and(|items| items.contains(&expected)),
                    "{id} pointer={pointer} expected={expected} actual={actual}"
                );
            }
        }
    }
}

#[test]
fn canonical_dsl_positive_negative_and_wrapper_witnesses_select_only_the_declared_rule() {
    let runtime = runtime();
    let config = config();
    let matrix = toml::from_str::<toml::Value>(MATRIX).expect("parse scenario matrix");
    let production =
        toml::from_str::<toml::Value>(&agent_semantic_hook::default_client_config_template())
            .expect("parse canonical config");
    let production_rules = production["rules"].as_array().expect("production rules");
    let wrappers = matrix["wrapperTemplates"]
        .as_array()
        .expect("wrapper templates");

    for witness in matrix["rules"].as_array().expect("matrix rules") {
        let expected_rule = witness["id"].as_str().expect("matrix rule id");
        let production_rule = production_rules
            .iter()
            .find(|rule| rule["id"].as_str() == Some(expected_rule))
            .expect("production rule");
        let projection_binary = witness["ruleComposition"]
            .as_array()
            .expect("composition")
            .iter()
            .find_map(|predicate| {
                (predicate["pointer"].as_str() == Some("/match/structuredProjection/binary"))
                    .then(|| predicate["equals"].as_str())
                    .flatten()
            });
        let capability_available = projection_binary.is_none_or(executable_is_available);

        if capability_available {
            for command in witness
                .get("positiveCommands")
                .and_then(toml::Value::as_array)
                .into_iter()
                .flatten()
                .map(|command| command.as_str().expect("positive command"))
            {
                let decision = classify_command(&runtime, &config, command);
                assert_declared_winner(&decision, production_rule, command);
                assert_eq!(decision.pointer("/subject/command"), Some(&json!(command)));
                assert!(decision.get("interactiveCommand").is_none());
                if witness.get("wrapperExemption").is_none() {
                    for wrapper in wrappers {
                        let wrapped =
                            render_wrapper(wrapper.as_str().expect("wrapper template"), command);
                        let decision = classify_command(&runtime, &config, &wrapped);
                        assert_declared_winner(&decision, production_rule, &wrapped);
                    }
                }
            }
        }
        for command in witness
            .get("negativeCommands")
            .and_then(toml::Value::as_array)
            .into_iter()
            .flatten()
            .map(|command| command.as_str().expect("negative command"))
        {
            assert_ne!(
                rule_id(&classify_command(&runtime, &config, command)),
                Some(expected_rule),
                "negative command={command}"
            );
        }
        if capability_available {
            for tool in witness
                .get("positiveTools")
                .and_then(toml::Value::as_array)
                .into_iter()
                .flatten()
            {
                let name = tool["name"].as_str().expect("positive tool name");
                let input = serde_json::to_value(&tool["input"]).expect("positive tool input");
                let decision = classify_tool(&runtime, &config, name, input);
                assert_declared_winner(&decision, production_rule, name);
            }
        }
        for tool in witness
            .get("negativeTools")
            .and_then(toml::Value::as_array)
            .into_iter()
            .flatten()
        {
            let name = tool["name"].as_str().expect("negative tool name");
            let input = serde_json::to_value(&tool["input"]).expect("negative tool input");
            assert_ne!(
                rule_id(&classify_tool(&runtime, &config, name, input)),
                Some(expected_rule),
                "negative tool={name}"
            );
        }
    }
}

#[cfg(unix)]
fn current_thread_cpu_nanos() -> u128 {
    let mut time = libc::timespec {
        tv_sec: 0,
        tv_nsec: 0,
    };
    let status = unsafe { libc::clock_gettime(libc::CLOCK_THREAD_CPUTIME_ID, &mut time) };
    assert_eq!(status, 0, "read current-thread CPU clock");
    (time.tv_sec as u128) * 1_000_000_000 + (time.tv_nsec as u128)
}

#[cfg(not(unix))]
fn current_thread_cpu_nanos() -> u128 {
    static STARTED: std::sync::OnceLock<std::time::Instant> = std::sync::OnceLock::new();
    STARTED
        .get_or_init(std::time::Instant::now)
        .elapsed()
        .as_nanos()
}

#[test]
fn canonical_dsl_matcher_cpu_cost_stays_inside_the_declared_gate() {
    const SAMPLES: u128 = 64;

    let runtime = runtime();
    let config = config();
    let matrix = toml::from_str::<toml::Value>(MATRIX).expect("parse scenario matrix");
    let maximum = matrix["performance"]["maxMatcherMicros"]
        .as_integer()
        .expect("matcher CPU gate") as u128;
    for witness in matrix["rules"].as_array().expect("matrix rules") {
        let Some(command) = witness
            .get("positiveCommands")
            .and_then(toml::Value::as_array)
            .and_then(|commands| commands.first())
            .and_then(toml::Value::as_str)
        else {
            continue;
        };
        let projection_binary = witness["ruleComposition"]
            .as_array()
            .expect("composition")
            .iter()
            .find_map(|predicate| {
                (predicate["pointer"].as_str() == Some("/match/structuredProjection/binary"))
                    .then(|| predicate["equals"].as_str())
                    .flatten()
            });
        if projection_binary.is_some_and(|binary| !executable_is_available(binary)) {
            continue;
        }
        // Exclude one-time matcher/config compilation from the declared
        // steady-state CPU gate; the measured loop is the real hot path.
        let _ = classify_command(&runtime, &config, command);
        let started = current_thread_cpu_nanos();
        for _ in 0..SAMPLES {
            let _ = classify_command(&runtime, &config, command);
        }
        let mean_micros = (current_thread_cpu_nanos() - started) / 1_000 / SAMPLES;
        assert!(
            mean_micros <= maximum,
            "rule={} meanMicros={mean_micros} maxMatcherMicros={maximum}",
            witness["id"].as_str().expect("rule id")
        );
    }
}
