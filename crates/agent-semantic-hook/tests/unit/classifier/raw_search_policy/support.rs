use agent_semantic_hook::{
    ActivatedProvider, DecisionKind, HookDecision, HookRuntime, ReasonKind, StdinMode,
    classify_hook,
};
use serde_json::json;

use crate::classifier::{command, command_with_stdin, provider, provider_routes};

pub(super) fn assert_raw_search_denied(command: &str, provider_id: &str) {
    let decision = classify_hook(
        &polyglot_registry(),
        "codex",
        "pre-tool",
        &json!({
            "tool_name": "functions.exec_command",
            "tool_input": {"cmd": command}
        }),
    );

    assert_eq!(decision.decision, DecisionKind::Deny, "{command}");
    assert_eq!(decision.reason_kind, ReasonKind::RawBroadSearch);
    assert_agent_facade_decision(&decision, command);
    assert_eq!(decision.routes[0].provider_id, provider_id, "{command}");
    for route in &decision.routes {
        assert!(
            !route
                .argv
                .windows(2)
                .any(|window| window[0] == "search" && window[1] == "query"),
            "{command}: {:?}",
            route.argv
        );
        if let Some(surface_index) = route.argv.iter().position(|arg| arg == "--surface") {
            assert_eq!(
                route.argv.get(surface_index + 1).map(String::as_str),
                Some("owners,tests"),
                "{command}: {:?}",
                route.argv
            );
        }
    }
}

pub(super) fn assert_bulk_source_dump_denied(command: &str, provider_id: &str) {
    let decision = classify_hook(
        &polyglot_registry(),
        "codex",
        "pre-tool",
        &json!({
            "tool_name": "functions.exec_command",
            "tool_input": {"cmd": command}
        }),
    );

    assert_eq!(decision.decision, DecisionKind::Deny, "{command}");
    assert_eq!(decision.reason_kind, ReasonKind::BulkSourceDump);
    assert_agent_facade_decision(&decision, command);
    assert_eq!(decision.routes[0].provider_id, provider_id, "{command}");
}

pub(super) fn assert_direct_read_denied(command: &str, provider_id: &str) {
    let decision = classify_hook(
        &polyglot_registry(),
        "codex",
        "pre-tool",
        &json!({
            "tool_name": "functions.exec_command",
            "tool_input": {"cmd": command}
        }),
    );

    assert_eq!(decision.decision, DecisionKind::Deny, "{command}");
    assert_eq!(decision.reason_kind, ReasonKind::DirectSourceRead);
    assert_agent_facade_decision(&decision, command);
    assert_eq!(decision.routes[0].provider_id, provider_id, "{command}");
}

pub(super) fn assert_content_dump_denied(command: &str, provider_id: &str) {
    let decision = classify_hook(
        &polyglot_registry(),
        "codex",
        "pre-tool",
        &json!({
            "tool_name": "functions.exec_command",
            "tool_input": {"cmd": command}
        }),
    );

    assert_eq!(decision.decision, DecisionKind::Deny, "{command}");
    assert_eq!(decision.reason_kind, ReasonKind::BulkSourceDump);
    assert_agent_facade_decision(&decision, command);
    assert_eq!(decision.routes[0].provider_id, provider_id, "{command}");
}

fn assert_agent_facade_decision(decision: &HookDecision, command: &str) {
    assert!(
        decision.message.starts_with("# ASP Hook Recovery"),
        "{command}: {}",
        decision.message
    );
    assert!(
        decision.message.contains(&format!(
            "blocked `{}`",
            reason_kind_label(decision.reason_kind)
        )),
        "{command}: {}",
        decision.message
    );
    assert!(
        decision.message.contains("## Stop")
            && decision.message.contains("## Run Next")
            && decision.message.contains("## Agent Flow")
            && decision.message.contains("## Rules"),
        "{command}: {}",
        decision.message
    );
    assert!(
        decision
            .message
            .contains("Do not retry `Read`, `cat`, `sed`, `rg`, or source-dump commands"),
        "{command}: {}",
        decision.message
    );
    for route in &decision.routes {
        assert_eq!(route.binary, "asp", "{command}: {:?}", route.argv);
        assert_eq!(
            route.argv.first().map(String::as_str),
            Some("asp"),
            "{command}"
        );
        assert!(
            decision
                .message
                .contains(&format!("asp {} query", route.language_id))
                || decision
                    .message
                    .contains(&format!("asp {} search", route.language_id)),
            "{command}: {}",
            decision.message
        );
        assert!(
            decision
                .message
                .contains(&format!("asp {} guide .", route.language_id)),
            "{command}: {}",
            decision.message
        );
    }
    for stale in [
        "rs-harness agent guide",
        "ts-harness agent guide",
        "py-harness agent guide",
        "provider guide:",
        "blocked=",
        "protocol=asp-hook-recovery.v1",
        "|run-next",
        "|guide",
    ] {
        assert!(
            !decision.message.contains(stale),
            "{command}: {}",
            decision.message
        );
    }
}

fn reason_kind_label(reason_kind: ReasonKind) -> &'static str {
    match reason_kind {
        ReasonKind::None => "none",
        ReasonKind::DirectSourceRead => "direct-source-read",
        ReasonKind::BulkSourceDump => "bulk-source-dump",
        ReasonKind::RawBroadSearch => "raw-broad-search",
        ReasonKind::SourceDirectoryEnumeration => "source-directory-enumeration",
        ReasonKind::AgentSearchJson => "agent-search-json",
        ReasonKind::SemanticAstPatchRequired => "semantic-ast-patch-required",
        ReasonKind::SubagentReceiptRequired => "subagent-receipt-required",
    }
}

pub(super) fn assert_allowed(command: &str) {
    let decision = classify_hook(
        &polyglot_registry(),
        "codex",
        "pre-tool",
        &json!({
            "tool_name": "functions.exec_command",
            "tool_input": {"cmd": command}
        }),
    );

    assert_eq!(decision.decision, DecisionKind::Allow, "{command}");
}

pub(super) fn rust_registry() -> HookRuntime {
    HookRuntime {
        project_root: ".".to_string(),
        providers: vec![rust_provider()],
    }
}

pub(super) fn polyglot_registry() -> HookRuntime {
    HookRuntime {
        project_root: ".".to_string(),
        providers: vec![typescript_provider(), rust_provider(), python_provider()],
    }
}

pub(super) fn typescript_provider() -> ActivatedProvider {
    provider(
        "typescript",
        "ts-harness",
        "ts-harness",
        "agent.semantic-protocols.languages.typescript.ts-harness",
        &[".ts", ".tsx", ".mts", ".cts", ".js", ".jsx", ".mjs", ".cjs"],
        &["package.json", "tsconfig.json"],
        &["src", "test", "tests", "__tests__", "packages", "apps"],
        &["node_modules", "dist", ".git"],
        provider_routes(
            "ts-harness",
            Some(command(&[
                "asp",
                "typescript",
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

pub(super) fn rust_provider() -> ActivatedProvider {
    let mut routes = provider_routes(
        "rs-harness",
        Some(command(&[
            "asp",
            "rust",
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
    );
    routes.owner = command(&[
        "asp",
        "rust",
        "query",
        "--from-hook",
        "direct-source-read",
        "--selector",
        "{path}",
        ".",
    ]);
    routes.ingest = command_with_stdin(
        &[
            "asp", "rust", "search", "ingest", "items", "tests", "--view", "seeds", ".",
        ],
        StdinMode::PipeCandidates,
    );
    provider(
        "rust",
        "rs-harness",
        "rs-harness",
        "agent.semantic-protocols.languages.rust.rs-harness",
        &[".rs"],
        &["Cargo.toml", "Cargo.lock"],
        &["src", "tests", "crates", "examples", "benches"],
        &["target", ".git"],
        routes,
    )
}

pub(super) fn python_provider() -> ActivatedProvider {
    let mut routes = provider_routes(
        "py-harness",
        Some(command(&[
            "asp",
            "python",
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
    );
    routes.owner = command(&["py-harness", "search", "owner", "{path}", "."]);
    routes.ingest = command_with_stdin(
        &["py-harness", "search", "ingest", "."],
        StdinMode::PipeCandidates,
    );
    provider(
        "python",
        "py-harness",
        "py-harness",
        "agent.semantic-protocols.languages.python.py-harness",
        &[".py", ".pyi"],
        &["pyproject.toml", "setup.py", "setup.cfg"],
        &["src", "test", "tests", "packages"],
        &[".venv", "__pycache__", ".git"],
        routes,
    )
}
