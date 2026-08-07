use agent_semantic_hook::{
    DecisionKind, HookClassificationRequest, ReasonKind, classify_hook_with_config,
    default_client_config_template, evaluate_match_policy_conformance,
    load_client_config_for_project,
};
use serde_json::{Value, json};
use std::{collections::BTreeSet, fs, path::PathBuf, process::Command};

use super::classifier::{builtin_programming_runtime, registry};

const CAPABILITY_CHILD_ENV: &str = "ASP_HOOK_MATCH_POLICY_CAPABILITY_CHILD";
const CONTRACT_TEST_NAME: &str = "match_policy_contract::production_match_policy_contract";

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
fn canonical_config_covers_builtin_programming_native_read_matrix() {
    let root = temp_project_root();
    fs::create_dir_all(&root).expect("matrix root");
    let config_path = root.join("config.toml");
    fs::write(&config_path, default_client_config_template()).expect("write config");
    let config = load_client_config_for_project(&config_path, &root).expect("compile config");
    let mut runtime = builtin_programming_runtime();
    runtime.project_root = root.to_string_lossy().into_owned();
    let mut count = 0usize;
    for provider in &runtime.providers {
        for extension in &provider.source_extensions {
            let path = format!("src/witness{extension}");
            let mut inputs = vec![json!({"type":"read", "path": path})];
            for key in [
                "path",
                "file",
                "file_path",
                "filePath",
                "absolute_path",
                "absolutePath",
                "relative_path",
                "relativePath",
                "uri",
            ] {
                inputs.push(json!({"toolName":"Read", "toolInput":{key: path}}));
            }
            for (tool_name, input_key) in [("toolName", "toolInput"), ("tool_name", "tool_input")] {
                inputs.push(json!({tool_name:"Read", input_key:{"path":path}}));
            }
            for input in inputs {
                let payload =
                    json!({"toolName":"functions.exec", "toolInput":{"commandActions":[input]}});
                let decision = classify_hook_with_config(HookClassificationRequest {
                    registry: &runtime,
                    config: &config,
                    platform: "codex",
                    event: "pre-tool",
                    payload: &payload,
                });
                assert_eq!(
                    decision.fields.get("configRuleId").and_then(Value::as_str),
                    Some("materialize-registered-source-read-action"),
                    "{payload}"
                );
                assert_eq!(decision.decision, DecisionKind::Deny);
                assert_eq!(decision.reason_kind, ReasonKind::DirectSourceRead);
                assert_eq!(decision.language_ids, [provider.language_id.as_str()]);
                assert!(
                    decision
                        .fields
                        .get("normalizedActions")
                        .is_some_and(|v| v.to_string().contains("direct-read"))
                );
                assert!(
                    decision
                        .routes
                        .iter()
                        .any(|route| route.provider_id == provider.provider_id)
                );
                count += 1;
            }
        }
    }
    assert!(count > 0);
    fs::remove_dir_all(root).expect("cleanup matrix root");
}

#[test]
fn bundled_plugin_matcher_covers_every_host_tool_action() {
    let hooks: Value = serde_json::from_str(include_str!(
        "../../../../asp-codex-plugin/hooks/hooks.json"
    ))
    .expect("plugin hooks JSON");
    let matcher = hooks["hooks"]["PreToolUse"][0]["matcher"]
        .as_str()
        .expect("PreToolUse matcher");
    assert_eq!(
        matcher, "*",
        "PreToolUse must deliver every host tool action to the internal policy matcher"
    );
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

fn run_with_projection_capabilities() {
    run_with_projection_capabilities_for(CONTRACT_TEST_NAME);
}

fn run_with_projection_capabilities_for(test_name: &str) {
    // Each fixture temporarily projects the test binary as a capability probe.
    // Serialize the process tree so parallel tests cannot multiply the same
    // executable/probe lifecycle and starve the Hook contract runner.
    let _capability_fixture = crate::match_policy_fixture::capability_guard();
    let capability_root = std::env::temp_dir().join(format!(
        "agent-semantic-hook-match-policy-capabilities-{}-{}",
        std::process::id(),
        test_name.replace("::", "-")
    ));
    let bin_dir = capability_root.join("bin");
    fs::create_dir_all(&bin_dir).expect("create match-policy capability bin");
    let current_exe = std::env::current_exe().expect("resolve current test executable");
    let yq = bin_dir.join("yq");
    if fs::hard_link(&current_exe, &yq).is_err() {
        fs::copy(&current_exe, &yq).expect("materialize executable yq capability");
    }
    let existing_path = std::env::var_os("PATH").unwrap_or_default();
    let path = std::env::join_paths(
        std::iter::once(bin_dir.clone()).chain(std::env::split_paths(&existing_path)),
    )
    .expect("compose isolated capability PATH");
    let output = Command::new(current_exe)
        .args(["--exact", test_name, "--nocapture"])
        .env(CAPABILITY_CHILD_ENV, "1")
        .env("PATH", path)
        .output()
        .expect("run isolated match-policy capability child");
    fs::remove_dir_all(capability_root).expect("cleanup match-policy capability root");
    assert!(
        output.status.success(),
        "isolated production match-policy contract failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn shell(command: &str) -> Value {
    json!({
        "tool_name": "exec_command",
        "tool_input": {"cmd": command},
    })
}

fn classify<'a>(
    runtime: &'a agent_semantic_hook::HookRuntime,
    config: &'a agent_semantic_hook::ClientHookConfig,
    payload: &'a Value,
) -> agent_semantic_hook::HookDecision {
    classify_hook_with_config(HookClassificationRequest {
        registry: runtime,
        config,
        platform: "codex",
        event: "pre-tool",
        payload,
    })
}

struct MatchCase {
    name: &'static str,
    payload: Value,
    rule_id: &'static str,
    decision: DecisionKind,
    reason: ReasonKind,
}

#[test]
fn production_match_policy_contract() {
    if std::env::var_os(CAPABILITY_CHILD_ENV).is_none() {
        run_with_projection_capabilities();
        return;
    }

    let root = temp_project_root();
    fs::create_dir_all(&root).expect("create match-policy contract root");
    let config_path = root.join("config.toml");
    let template = default_client_config_template();
    fs::write(&config_path, template).expect("write production hook config");
    fs::write(
        root.join("package.json"),
        "{\"package\":{\"name\":\"hook\"}}\n",
    )
    .expect("write JSON projection fixture");
    fs::write(root.join("Cargo.toml"), "[package]\nname = \"hook\"\n")
        .expect("write TOML projection fixture");

    let config =
        load_client_config_for_project(&config_path, &root).expect("load production hook config");
    let rule_ids = config.rule_ids().collect::<BTreeSet<_>>();
    assert_eq!(config.rule_count(), rule_ids.len());

    let mut runtime = registry();
    runtime.project_root = root.to_string_lossy().into_owned();
    let cases = vec![
        MatchCase {
            name: "registered reasoning search",
            payload: shell("asp rust search lexical --query classify_hook --workspace ."),
            rule_id: "registered-asp-reasoning-search",
            decision: DecisionKind::Deny,
            reason: ReasonKind::SubagentReceiptRequired,
        },
        MatchCase {
            name: "testing dispatch",
            payload: shell("cargo test --workspace"),
            rule_id: "resident-testing-dispatch",
            decision: DecisionKind::Deny,
            reason: ReasonKind::SubagentReceiptRequired,
        },
        MatchCase {
            name: "raw registered source search",
            payload: json!({
                "tool_name": "Grep",
                "tool_input": {
                    "pattern": "classify_hook",
                    "path": "src/app.ts"
                },
            }),
            rule_id: "deny-raw-registered-source-search-action",
            decision: DecisionKind::Deny,
            reason: ReasonKind::RawBroadSearch,
        },
        MatchCase {
            name: "javascript inline source materialization",
            payload: shell("node -e 'require(\"fs\").readFileSync(\"src/app.ts\", \"utf8\")'"),
            rule_id: "materialize-source-access-policy",
            decision: DecisionKind::Deny,
            reason: ReasonKind::BulkSourceDump,
        },
        MatchCase {
            name: "legacy python inline source materialization",
            payload: shell("python -c 'print(open(\"src/app.ts\").read())'"),
            rule_id: "materialize-source-access-policy",
            decision: DecisionKind::Deny,
            reason: ReasonKind::BulkSourceDump,
        },
        MatchCase {
            name: "source materialization command",
            payload: shell("sed -n '1,8p' src/app.ts"),
            rule_id: "deny-uncontrolled-source-materialization-commands",
            decision: DecisionKind::Deny,
            reason: ReasonKind::BulkSourceDump,
        },
        MatchCase {
            name: "source search command",
            payload: shell("rg classify_hook src/app.ts"),
            rule_id: "deny-uncontrolled-source-search-commands",
            decision: DecisionKind::Deny,
            reason: ReasonKind::RawBroadSearch,
        },
        MatchCase {
            name: "git source read",
            payload: shell("git show HEAD:src/app.ts"),
            rule_id: "deny-uncontrolled-git-source-reads",
            decision: DecisionKind::Deny,
            reason: ReasonKind::BulkSourceDump,
        },
        MatchCase {
            name: "git metadata read",
            payload: shell("git diff --check"),
            rule_id: "deny-uncontrolled-git-metadata-reads",
            decision: DecisionKind::Deny,
            reason: ReasonKind::RawBroadSearch,
        },
        MatchCase {
            name: "raw registered source execute action",
            payload: shell("read src/app.ts"),
            rule_id: "deny-raw-registered-source-action",
            decision: DecisionKind::Deny,
            reason: ReasonKind::BulkSourceDump,
        },
        MatchCase {
            name: "registered source read materializer",
            payload: json!({
                "tool_name": "Read",
                "tool_input": {"file_path": "src/app.ts"},
            }),
            rule_id: "materialize-registered-source-read-action",
            decision: DecisionKind::Deny,
            reason: ReasonKind::DirectSourceRead,
        },
        MatchCase {
            name: "structured document read materializer",
            payload: json!({
                "tool_name": "Read",
                "tool_input": {"file_path": "package.json"},
            }),
            rule_id: "materialize-structured-document-read-action",
            decision: DecisionKind::Deny,
            reason: ReasonKind::StructuredSourceRead,
        },
        MatchCase {
            name: "bounded JSON projection",
            payload: shell("jq -c '.package.name' package.json"),
            rule_id: "allow-bounded-json-projection",
            decision: DecisionKind::Allow,
            reason: ReasonKind::None,
        },
        MatchCase {
            name: "bounded TOML projection",
            payload: shell("yq eval '.package.name' Cargo.toml"),
            rule_id: "allow-bounded-toml-projection",
            decision: DecisionKind::Allow,
            reason: ReasonKind::None,
        },
        MatchCase {
            name: "unbounded structured projection",
            payload: shell("jq '.' package.json"),
            rule_id: "deny-unbounded-structured-projection",
            decision: DecisionKind::Deny,
            reason: ReasonKind::BulkSourceDump,
        },
        MatchCase {
            name: "agent search JSON",
            payload: shell("ts-harness search lexical projectRoot owner tests --json ."),
            rule_id: "deny-agent-search-json",
            decision: DecisionKind::Deny,
            reason: ReasonKind::AgentSearchJson,
        },
        MatchCase {
            name: "apply patch materializer",
            payload: json!({
                "tool_name": "apply_patch",
                "tool_input": {
                    "patch": "*** Begin Patch\n*** Update File: src/app.ts\n@@\n-old\n+new\n*** End Patch\n"
                },
            }),
            rule_id: "materialize-apply-patch-policy",
            decision: DecisionKind::Deny,
            reason: ReasonKind::SemanticAstPatchRequired,
        },
        MatchCase {
            name: "source access materializer",
            payload: json!({
                "tool_name": "functions.exec_command",
                "tool_input": {"cmd": "custom-reader '.read_text(' src/app.ts"},
            }),
            rule_id: "materialize-source-access-policy",
            decision: DecisionKind::Deny,
            reason: ReasonKind::BulkSourceDump,
        },
    ];
    let matrix_rule_ids = cases
        .iter()
        .map(|case| case.rule_id)
        .collect::<BTreeSet<_>>();
    let mut failures = Vec::new();
    if matrix_rule_ids != rule_ids {
        failures.push(format!(
            "production config and match-policy matrix rule IDs differ: config_only={:?} matrix_only={:?}",
            rule_ids.difference(&matrix_rule_ids).collect::<Vec<_>>(),
            matrix_rule_ids.difference(&rule_ids).collect::<Vec<_>>()
        ));
    }
    let mut matched = BTreeSet::new();
    for case in cases {
        let decision = classify(&runtime, &config, &case.payload);
        let rule_id = decision
            .fields
            .get("configRuleId")
            .and_then(Value::as_str)
            .unwrap_or("<none>");
        if rule_id != "<none>" {
            matched.insert(rule_id.to_owned());
        }
        if rule_id != case.rule_id
            || decision.decision != case.decision
            || decision.reason_kind != case.reason
        {
            failures.push(format!(
                "{}: expected rule={} decision={:?} reason={:?}; actual rule={} decision={:?} reason={:?}; normalizedActions={} hookMatchReceipt={}; expected rule is unreachable or shadowed",
                case.name,
                case.rule_id,
                case.decision,
                case.reason,
                rule_id,
                decision.decision,
                decision.reason_kind,
                decision
                    .fields
                    .get("normalizedActions")
                    .unwrap_or(&Value::Null),
                decision
                    .fields
                    .get("hookMatchReceipt")
                    .unwrap_or(&Value::Null)
            ));
        }
    }
    let configured = rule_ids
        .iter()
        .map(|rule_id| (*rule_id).to_owned())
        .collect::<BTreeSet<_>>();
    let report = evaluate_match_policy_conformance(&runtime, &config, "codex");
    if !report.is_complete() {
        failures.push(format!(
            "production conformance receipt incomplete: {report:?}"
        ));
    }
    if let Err(error) = agent_semantic_hook::validate_match_policy_rule_coverage(&config) {
        failures.push(format!(
            "runtime-free mmap rule coverage gate rejected production config: {error}"
        ));
    }
    fs::remove_dir_all(root).expect("cleanup match-policy contract root");
    if matched != configured {
        failures.push(format!(
            "configured rule IDs without an end-to-end witness: {:?}",
            configured.difference(&matched).collect::<Vec<_>>()
        ));
    }
    assert!(
        failures.is_empty(),
        "production match-policy contract failed:\n{}",
        failures.join("\n")
    );
}

#[path = "match_policy_contract/branch_coverage.rs"]
mod branch_coverage;
