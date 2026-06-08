use agent_semantic_hook::{
    ActivatedProvider, HookClassificationRequest, HookRuntime, classify_hook,
    classify_hook_with_config, load_client_config, render_platform_response,
};
use serde_json::json;
use std::{
    fs,
    time::{SystemTime, UNIX_EPOCH},
};

use super::{command, provider, provider_routes, registry};

#[test]
fn platform_response_wraps_denied_decision_for_codex_hooks() {
    let decision = classify_hook(
        &registry(),
        "codex",
        "pre-tool",
        &json!({
            "tool_name": "Read",
            "tool_input": {"path": "src/cli/agent-hooks.ts"}
        }),
    );

    let response = render_platform_response(&decision).unwrap();

    assert_eq!(
        response["hookSpecificOutput"]["hookEventName"],
        "PreToolUse"
    );
    assert_eq!(response["hookSpecificOutput"]["permissionDecision"], "deny");
    assert!(response.get("agentHookDecision").is_none());
    let context = response["hookSpecificOutput"]["additionalContext"]
        .as_str()
        .expect("decision context");
    assert!(context.starts_with("[agent-hook-decision] "));
    assert!(context.contains("\"decision\":\"deny\""));
    let reason = response["hookSpecificOutput"]["permissionDecisionReason"]
        .as_str()
        .expect("permission decision reason");
    assert!(reason.starts_with("ASP hook denied `"), "{reason}");
    assert!(
        reason.contains("@.agents/skills/agent-semantic-protocols/SKILL.md"),
        "{reason}"
    );
    assert!(reason.contains("## ASP Hook Recovery"), "{reason}");
    let system_message = response["systemMessage"].as_str().expect("system message");
    assert_eq!(system_message, reason);
    assert!(system_message.contains("## Stop"), "{system_message}");
    assert!(system_message.contains("## Run Next"), "{system_message}");
    assert!(
        system_message.contains("## Detected Binaries"),
        "{system_message}"
    );
    assert!(system_message.contains("## Agent Flow"), "{system_message}");
    assert!(
        system_message.contains("Do not retry `Read`, `cat`, `sed`, `rg`, or source-dump commands"),
        "{system_message}"
    );
    assert!(
        system_message.contains(
            "asp typescript query --from-hook direct-source-read --selector src/cli/agent-hooks.ts --workspace . --code"
        ),
        "{system_message}"
    );
    assert!(
        system_message.contains("`asp typescript guide .`"),
        "{system_message}"
    );
    assert!(
        system_message.contains(
            "- language=typescript provider=ts-harness command=`ts-harness` facade=`asp typescript`"
        ),
        "{system_message}"
    );
    assert!(
        system_message.contains("asp typescript search prime --view seeds ."),
        "{system_message}"
    );
    assert!(
        system_message.contains("asp typescript query guide treesitter ."),
        "{system_message}"
    );
    assert!(system_message.contains("apply_patch"), "{system_message}");
    assert!(
        system_message.contains(
            "`ast-patch` is available for structural/mechanical edits after a provider dry-run receipt"
        ),
        "{system_message}"
    );
    assert!(!system_message.contains("|run-next"), "{system_message}");
    assert!(
        !system_message.contains("protocol=asp-hook-recovery.v1"),
        "{system_message}"
    );
}

#[test]
fn platform_response_keeps_multi_language_agent_flows_separate() {
    let runtime = HookRuntime {
        project_root: ".".to_string(),
        providers: vec![
            provider_for_language(
                "rust",
                "rs-harness",
                "rs-harness",
                &[".rs"],
                &["Cargo.toml"],
                &["target"],
            ),
            provider_for_language(
                "typescript",
                "ts-harness",
                "ts-harness",
                &[".ts", ".tsx", ".js", ".jsx"],
                &["package.json", "tsconfig.json"],
                &["node_modules", "dist"],
            ),
            provider_for_language(
                "python",
                "py-harness",
                "py-harness",
                &[".py"],
                &["pyproject.toml"],
                &[".venv", "__pycache__"],
            ),
        ],
    };
    let decision = classify_hook(
        &runtime,
        "codex",
        "pre-tool",
        &json!({
            "tool_name": "Bash",
            "tool_input": {
                "command": "cat crates/agent-semantic-hook/src/classifier.rs languages/typescript-lang-project-harness/src/config.ts languages/python-lang-project-harness/src/python_lang_project_harness/_project_config.py"
            }
        }),
    );

    let response = render_platform_response(&decision).unwrap();
    let reason = response["hookSpecificOutput"]["permissionDecisionReason"]
        .as_str()
        .expect("permission decision reason");
    let rust_flow = language_flow(reason, "Rust", "TypeScript");
    let typescript_flow = language_flow(reason, "TypeScript", "Python");
    let python_flow = language_flow(reason, "Python", "Rules");

    assert!(reason.contains("## Detected Binaries"), "{reason}");
    assert!(
        reason
            .contains("- language=rust provider=rs-harness command=`rs-harness` facade=`asp rust`"),
        "{reason}"
    );
    assert!(
        reason.contains(
            "- language=typescript provider=ts-harness command=`ts-harness` facade=`asp typescript`"
        ),
        "{reason}"
    );
    assert!(
        reason.contains(
            "- language=python provider=py-harness command=`py-harness` facade=`asp python`"
        ),
        "{reason}"
    );
    assert!(
        rust_flow.contains("1. Start from the language guide"),
        "{reason}"
    );
    assert!(rust_flow.contains("`asp rust guide .`"), "{reason}");
    assert!(
        rust_flow.contains("`asp rust check --changed .`"),
        "{reason}"
    );
    assert!(!rust_flow.contains("asp typescript"), "{reason}");
    assert!(!rust_flow.contains("asp python"), "{reason}");
    assert!(
        typescript_flow.contains("1. Start from the language guide"),
        "{reason}"
    );
    assert!(
        typescript_flow.contains("`asp typescript guide .`"),
        "{reason}"
    );
    assert!(
        typescript_flow.contains("`asp typescript check --changed .`"),
        "{reason}"
    );
    assert!(
        python_flow.contains("1. Start from the language guide"),
        "{reason}"
    );
    assert!(python_flow.contains("`asp python guide .`"), "{reason}");
    assert!(
        python_flow.contains("`asp python check --changed .`"),
        "{reason}"
    );
}

#[test]
fn platform_response_reflects_disabled_semantic_ast_patch_config() {
    let config_dir = std::env::temp_dir().join(format!(
        "asp-hook-test-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock")
            .as_nanos()
    ));
    fs::create_dir_all(&config_dir).expect("config dir");
    let config_path = config_dir.join("config.toml");
    fs::write(
        &config_path,
        r#"
schemaId = "agent.semantic-protocols.hook.client-config"
schemaVersion = "1"
protocolId = "agent.semantic-protocols.hook"
protocolVersion = "1"

[experimental.semanticAstPatch]
enabled = false
"#,
    )
    .expect("write hook config");
    let config = load_client_config(&config_path).expect("load hook config");
    let runtime = registry();
    let decision = classify_hook_with_config(HookClassificationRequest {
        registry: &runtime,
        config: &config,
        platform: "codex",
        event: "pre-tool",
        payload: &json!({
            "tool_name": "Read",
            "tool_input": {"path": "src/cli/agent-hooks.ts"}
        }),
    });
    let response = render_platform_response(&decision).unwrap();
    let reason = response["hookSpecificOutput"]["permissionDecisionReason"]
        .as_str()
        .expect("permission decision reason");

    assert!(
        reason.contains("experimental.semanticAstPatch.enabled = false"),
        "{reason}"
    );
    assert!(reason.contains("patch with `apply_patch`"), "{reason}");
    assert!(
        reason.contains("`ast-patch` is disabled by hook config"),
        "{reason}"
    );
    assert!(
        !reason.contains(
            "`ast-patch` is available for structural/mechanical edits after a provider dry-run receipt"
        ),
        "{reason}"
    );

    let _ = fs::remove_dir_all(config_dir);
}

fn provider_for_language(
    language_id: &str,
    provider_id: &str,
    binary: &str,
    source_extensions: &[&str],
    config_files: &[&str],
    ignored_path_prefixes: &[&str],
) -> ActivatedProvider {
    let namespace = format!("agent.semantic-protocols.languages.{language_id}.{provider_id}");
    provider(
        language_id,
        provider_id,
        binary,
        &namespace,
        source_extensions,
        config_files,
        &["src", "tests"],
        ignored_path_prefixes,
        provider_routes(
            binary,
            Some(command(&[
                "asp",
                language_id,
                "query",
                "--from-hook",
                "direct-source-read",
                "--selector",
                "{selector}",
                "{termArgs}",
                "--surface",
                "owners,tests",
                "--view",
                "seeds",
                ".",
            ])),
        ),
    )
}

fn language_flow<'a>(message: &'a str, heading: &str, next_heading: &str) -> &'a str {
    let start = message
        .find(&format!("### {heading}"))
        .unwrap_or_else(|| panic!("missing {heading} flow:\n{message}"));
    let end = message[start..]
        .find(&format!("### {next_heading}"))
        .map(|offset| start + offset)
        .or_else(|| {
            message[start..]
                .find("## Rules")
                .map(|offset| start + offset)
        })
        .unwrap_or_else(|| panic!("missing {next_heading} boundary:\n{message}"));
    &message[start..end]
}
