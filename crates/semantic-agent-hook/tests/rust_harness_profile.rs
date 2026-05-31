use semantic_agent_hook::{
    classify_hook, CommandTemplate, DecisionKind, HookCommands, LanguageProfile, ProfileRegistry,
    ReasonKind,
};
use serde_json::json;

fn rust_harness_profile_registry() -> ProfileRegistry {
    let config = rust_lang_project_harness::default_rust_harness_config();
    let mut ignored_path_prefixes = config.ignored_dir_names.into_iter().collect::<Vec<_>>();
    ignored_path_prefixes.sort();

    let mut source_roots = config.source_dir_names;
    source_roots.extend(config.test_dir_names);
    source_roots.extend(["examples".to_string(), "benches".to_string()]);

    ProfileRegistry {
        project_root: ".".to_string(),
        profiles: vec![LanguageProfile {
            language_id: "rust".to_string(),
            provider_id: "rs-harness".to_string(),
            binary: "rs-harness".to_string(),
            namespace: "agent.semantic-protocols.languages.rust.rs-harness".to_string(),
            source_extensions: vec![".rs".to_string()],
            config_files: vec!["Cargo.toml".to_string(), "Cargo.lock".to_string()],
            source_roots,
            ignored_path_prefixes,
            commands: HookCommands {
                prime: command(["rs-harness", "search", "prime", "--view", "seeds", "."]),
                owner: command([
                    "rs-harness",
                    "search",
                    "owner",
                    "{path}",
                    "items",
                    "--view",
                    "seeds",
                    ".",
                ]),
                text: command([
                    "rs-harness",
                    "search",
                    "text",
                    "{query}",
                    "owner",
                    "tests",
                    "--view",
                    "seeds",
                    ".",
                ]),
                ingest: CommandTemplate {
                    argv: vec![
                        "rs-harness".to_string(),
                        "search".to_string(),
                        "ingest".to_string(),
                        "items".to_string(),
                        "tests".to_string(),
                        "--view".to_string(),
                        "seeds".to_string(),
                        ".".to_string(),
                    ],
                    stdin_mode: Some("pipe-candidates".to_string()),
                },
                check_changed: command(["rs-harness", "check", "--changed", "."]),
            },
        }],
    }
}

fn command<const N: usize>(argv: [&str; N]) -> CommandTemplate {
    CommandTemplate {
        argv: argv.into_iter().map(str::to_string).collect(),
        stdin_mode: None,
    }
}

#[test]
fn build_script_uses_rust_harness_source_roots() {
    assert_eq!(env!("SEMANTIC_AGENT_HOOK_RUST_SOURCE_ROOTS"), "src");
}

#[test]
fn rust_harness_profile_routes_direct_reads_to_owner_search() {
    let decision = classify_hook(
        &rust_harness_profile_registry(),
        "codex",
        "pre-tool",
        &json!({
            "tool_name": "Read",
            "tool_input": {"path": "src/lib.rs"}
        }),
    );

    assert_eq!(decision.decision, DecisionKind::Deny);
    assert_eq!(decision.reason_kind, ReasonKind::DirectSourceRead);
    assert_eq!(
        decision.routes[0].argv,
        [
            "rs-harness",
            "search",
            "owner",
            "src/lib.rs",
            "items",
            "--view",
            "seeds",
            "."
        ]
    );
}

#[test]
fn rust_harness_profile_routes_raw_root_search_to_ingest() {
    let decision = classify_hook(
        &rust_harness_profile_registry(),
        "codex",
        "pre-tool",
        &json!({
            "tool_name": "functions.exec_command",
            "tool_input": {"cmd": "rg -n \"HookDecision\" ."}
        }),
    );

    assert_eq!(decision.decision, DecisionKind::Deny);
    assert_eq!(decision.reason_kind, ReasonKind::RawBroadSearch);
    assert_eq!(decision.routes[0].kind, "ingest");
    assert_eq!(
        decision.routes[0].stdin_mode.as_deref(),
        Some("pipe-candidates")
    );
}
